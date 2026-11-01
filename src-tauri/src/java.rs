//! Managed Java runtimes from Eclipse Adoptium (Temurin), SHA-256 verified.
//! One runtime per major version lives in `java/<major>/`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::core::{emit_progress, sha256_file, Paths};

#[derive(Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
    release_name: String,
}

#[derive(Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Deserialize)]
struct AdoptiumPackage {
    link: String,
    checksum: String,
    name: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    pub major: u32,
    pub path: String,
    pub managed: bool,
    pub release: Option<String>,
}

fn adoptium_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn adoptium_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}

fn exe_name() -> &'static str {
    // javaw has no console window on Windows; stdout/stderr pipes still work.
    if cfg!(target_os = "windows") {
        "javaw.exe"
    } else {
        "java"
    }
}

/// Finds `bin/<java>` inside an extracted runtime (handles macOS Contents/Home).
fn find_exe(dir: &Path) -> Option<PathBuf> {
    [dir.join("bin").join(exe_name()), dir.join("Contents/Home/bin").join(exe_name())].into_iter().find(|c| c.is_file())
}

/// Which Java a Minecraft version needs. Old versions say nothing (Java 8).
/// Java 16 has no Adoptium JRE; 17 runs those versions fine.
pub fn effective_major(required: u32) -> u32 {
    match required {
        0..=8 => 8,
        9..=17 => 17,
        18..=21 => 21,
        n => n,
    }
}

pub fn installed(paths: &Paths) -> Vec<JavaRuntime> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(paths.java()) {
        for e in rd.flatten() {
            let Ok(major) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
            if let Some(exe) = find_exe(&e.path()) {
                let release = std::fs::read_to_string(e.path().join(".halo-release")).ok();
                out.push(JavaRuntime { major, path: exe.to_string_lossy().into(), managed: true, release });
            }
        }
    }
    out.sort_by_key(|j| j.major);
    out
}

/// Returns the java executable for `major`, downloading it if needed.
pub async fn ensure(http: &reqwest::Client, paths: &Paths, app: &AppHandle, major: u32) -> anyhow::Result<PathBuf> {
    let major = effective_major(major);
    let dir = paths.java().join(major.to_string());
    if let Some(exe) = find_exe(&dir) {
        return Ok(exe);
    }
    emit_progress(app, "java", &format!("Downloading Java {major}"), 0, 1);

    let mut asset = None;
    for image in ["jre", "jdk"] {
        let url = format!(
            "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={}&image_type={image}&os={}&vendor=eclipse",
            adoptium_arch(),
            adoptium_os()
        );
        if let Ok(list) = crate::core::get_json::<Vec<AdoptiumAsset>>(http, &url).await {
            if let Some(a) = list.into_iter().next() {
                asset = Some(a);
                break;
            }
        }
    }
    let asset = asset.with_context(|| format!("Adoptium has no Java {major} build for this system"))?;

    let tmp_dir = paths.java().join(format!(".download-{major}"));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir)?;
    let archive = tmp_dir.join(&asset.binary.package.name);
    let dl = crate::download::Downloader { http, paths, app, threads: 1, verify_existing: false };
    dl.run("java", &format!("Downloading Java {major}"), vec![crate::download::DlItem::new(asset.binary.package.link.clone(), archive.clone())])
        .await?;
    let actual = sha256_file(&archive).await?;
    if !actual.eq_ignore_ascii_case(&asset.binary.package.checksum) {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        bail!("Java {major} download failed its SHA-256 check");
    }

    emit_progress(app, "java", &format!("Installing Java {major}"), 0, 1);
    let extract_to = tmp_dir.join("x");
    let archive2 = archive.clone();
    let extract2 = extract_to.clone();
    tokio::task::spawn_blocking(move || extract_archive(&archive2, &extract2)).await??;

    // Archives contain one top-level folder (e.g. jdk-21.0.5+11-jre/); move its contents up.
    let inner = std::fs::read_dir(&extract_to)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .unwrap_or(extract_to.clone());
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&inner, &dir).or_else(|_| crate::core::copy_dir(&inner, &dir))?;
    std::fs::write(dir.join(".halo-release"), &asset.release_name)?;
    let _ = std::fs::remove_dir_all(&tmp_dir);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(rd) = std::fs::read_dir(dir.join("bin")) {
            for e in rd.flatten() {
                let _ = std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o755));
            }
        }
    }
    emit_progress(app, "java", &format!("Java {major} ready"), 1, 1);
    find_exe(&dir).with_context(|| format!("Java {major} installed but no executable was found"))
}

fn extract_archive(archive: &Path, dest: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest)?;
    let name = archive.to_string_lossy().to_lowercase();
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let out = crate::core::safe_join(dest, entry.name())?;
            if entry.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut f = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut f)?;
        }
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let gz = flate2::read::GzDecoder::new(std::fs::File::open(archive)?);
        let mut tar = tar::Archive::new(gz);
        // `unpack` rejects entries that escape `dest`.
        tar.unpack(dest)?;
    } else {
        bail!("unknown archive format: {name}");
    }
    Ok(())
}

/// Reads the major version from `java -version` output for user-picked runtimes.
pub async fn probe_major(java: &Path) -> anyhow::Result<u32> {
    let java = if cfg!(target_os = "windows") && java.file_name().map(|n| n == "javaw.exe").unwrap_or(false) {
        java.with_file_name("java.exe")
    } else {
        java.to_path_buf()
    };
    let mut cmd = tokio::process::Command::new(&java);
    cmd.arg("-version");
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let out = cmd.output().await.context("couldn't run java -version")?;
    let text = String::from_utf8_lossy(&out.stderr);
    parse_java_version(&text).context("couldn't read Java version")
}

pub fn parse_java_version(text: &str) -> Option<u32> {
    let re = regex::Regex::new(r#"version "(\d+)(?:\.(\d+))?"#).ok()?;
    let c = re.captures(text)?;
    let first: u32 = c.get(1)?.as_str().parse().ok()?;
    if first == 1 {
        c.get(2)?.as_str().parse().ok()
    } else {
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_versions() {
        assert_eq!(parse_java_version(r#"openjdk version "21.0.5" 2024-10-15"#), Some(21));
        assert_eq!(parse_java_version(r#"java version "1.8.0_421""#), Some(8));
        assert_eq!(effective_major(16), 17);
        assert_eq!(effective_major(25), 25);
        assert_eq!(effective_major(0), 8);
    }
}
