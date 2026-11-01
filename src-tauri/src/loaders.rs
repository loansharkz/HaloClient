//! Mod loaders. Fabric and Quilt come from their meta APIs as version JSONs
//! that inherit from vanilla. Forge and NeoForge run the official installer
//! headless against the launcher's data root (which mirrors .minecraft).

use std::path::Path;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::core::{emit_progress, get_json, get_json_cached, write_json, Paths};
use crate::download::{DlItem, Downloader};
use crate::versions::{self, version_json_path, VersionJson};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub version: String,
    pub recommended: bool,
}

#[derive(Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoader,
}
#[derive(Deserialize)]
struct FabricLoader {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

/// Loader versions for a Minecraft version, newest first.
pub async fn versions(http: &reqwest::Client, paths: &Paths, loader: &str, mc: &str) -> anyhow::Result<Vec<LoaderVersion>> {
    match loader {
        "fabric" | "quilt" => {
            let url = if loader == "fabric" {
                format!("https://meta.fabricmc.net/v2/versions/loader/{mc}")
            } else {
                format!("https://meta.quiltmc.org/v3/versions/loader/{mc}")
            };
            let list: Vec<FabricLoaderEntry> = get_json(http, &url).await?;
            if list.is_empty() {
                bail!("{} doesn't support Minecraft {mc} yet", title(loader));
            }
            let first_stable = list.iter().position(|e| e.loader.stable.unwrap_or(!e.loader.version.contains("beta")));
            Ok(list
                .into_iter()
                .enumerate()
                .map(|(i, e)| LoaderVersion { version: e.loader.version, recommended: Some(i) == first_stable })
                .collect())
        }
        "forge" => {
            #[derive(Deserialize, Serialize)]
            struct Promos {
                promos: std::collections::HashMap<String, String>,
            }
            let promos: Promos =
                get_json_cached(http, paths, "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json", "forge-promos.json", 3600).await?;
            let rec = promos.promos.get(&format!("{mc}-recommended")).or(promos.promos.get(&format!("{mc}-latest"))).cloned();
            let xml = http
                .get("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml")
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;
            let prefix = format!("{mc}-");
            let re = regex::Regex::new(r"<version>([^<]+)</version>")?;
            let mut v: Vec<LoaderVersion> = re
                .captures_iter(&xml)
                .map(|c| c[1].to_string())
                .filter(|s| s.starts_with(&prefix))
                .map(|s| {
                    let short = s[prefix.len()..].to_string();
                    LoaderVersion { recommended: rec.as_deref() == Some(short.as_str()), version: short }
                })
                .collect();
            v.reverse();
            if v.is_empty() {
                bail!("Forge has no builds for Minecraft {mc}");
            }
            Ok(v)
        }
        "neoforge" => {
            #[derive(Deserialize)]
            struct Versions {
                versions: Vec<String>,
            }
            let list: Versions = get_json(http, "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge").await?;
            let prefix = neoforge_prefix(mc);
            let mut v: Vec<String> = list.versions.into_iter().filter(|s| s.starts_with(&prefix)).collect();
            v.reverse();
            if v.is_empty() {
                bail!("NeoForge has no builds for Minecraft {mc} (NeoForge starts at 1.20.2)");
            }
            let first_stable = v.iter().position(|s| !s.contains("beta"));
            Ok(v.into_iter().enumerate().map(|(i, version)| LoaderVersion { version, recommended: Some(i) == first_stable }).collect())
        }
        _ => Ok(vec![]),
    }
}

/// NeoForge versions encode the Minecraft version: 1.21.1 → 21.1.x, 1.21 → 21.0.x, 26.1 → 26.1.x
pub fn neoforge_prefix(mc: &str) -> String {
    let rest = mc.strip_prefix("1.").unwrap_or(mc);
    if rest.contains('.') {
        format!("{rest}.")
    } else {
        format!("{rest}.0.")
    }
}

fn title(loader: &str) -> &'static str {
    match loader {
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        _ => "Vanilla",
    }
}

/// Installs the loader (if needed) and returns the version id to launch.
pub async fn install(dl: &Downloader<'_>, java: &Path, loader: &str, mc: &str, loader_version: Option<&str>) -> anyhow::Result<String> {
    let http = dl.http;
    let paths = dl.paths;
    // Vanilla JSON is always needed (parent of every loader profile).
    versions::ensure_vanilla_json(http, paths, mc).await?;
    let lv = match loader_version {
        Some(v) if !v.is_empty() => v.to_string(),
        _ if loader == "vanilla" => String::new(),
        _ => {
            let list = versions(http, paths, loader, mc).await?;
            list.iter().find(|v| v.recommended).or(list.first()).map(|v| v.version.clone()).context("no loader version")?
        }
    };
    match loader {
        "vanilla" => Ok(mc.to_string()),
        "fabric" | "quilt" => {
            let url = if loader == "fabric" {
                format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{lv}/profile/json")
            } else {
                format!("https://meta.quiltmc.org/v3/versions/loader/{mc}/{lv}/profile/json")
            };
            let profile: VersionJson = get_json(http, &url).await.with_context(|| format!("getting the {} profile", title(loader)))?;
            write_json(&version_json_path(paths, &profile.id), &profile)?;
            Ok(profile.id)
        }
        "forge" | "neoforge" => install_forge_like(dl, java, loader, mc, &lv).await,
        other => bail!("unknown loader {other}"),
    }
}

async fn install_forge_like(dl: &Downloader<'_>, java: &Path, loader: &str, mc: &str, lv: &str) -> anyhow::Result<String> {
    let paths = dl.paths;
    let (url, expected_id) = if loader == "forge" {
        let full = format!("{mc}-{lv}");
        (
            format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"),
            format!("{mc}-forge-{lv}"),
        )
    } else {
        (format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar"), format!("neoforge-{lv}"))
    };
    if version_json_path(paths, &expected_id).exists() {
        return Ok(expected_id);
    }

    // The installer patches the vanilla client jar, so it must exist first.
    let vanilla = versions::ensure_vanilla_json(dl.http, paths, mc).await?;
    if let Some(c) = vanilla.downloads.as_ref().and_then(|d| d.client.clone()) {
        let jar = paths.versions().join(mc).join(format!("{mc}.jar"));
        dl.run("loader", "Preparing loader install", vec![DlItem::new(c.url, jar).sha1(c.sha1).size(c.size)]).await?;
    }

    let installer = paths.cache().join("installers").join(url.rsplit('/').next().unwrap());
    // Verify the installer against the .sha1 the maven publishes next to it.
    let sha1 = dl.http.get(format!("{url}.sha1")).send().await.ok().filter(|r| r.status().is_success());
    let sha1 = match sha1 {
        Some(r) => r.text().await.ok().map(|s| s.trim().split_whitespace().next().unwrap_or("").to_string()).filter(|s| s.len() == 40),
        None => None,
    };
    dl.run("loader", &format!("Downloading {} installer", title(loader)), vec![DlItem::new(url.clone(), installer.clone()).sha1(sha1)])
        .await?;

    // Installers refuse to run without a launcher_profiles.json in the target.
    let profiles = paths.root.join("launcher_profiles.json");
    if !profiles.exists() {
        std::fs::write(&profiles, br#"{"profiles":{}}"#)?;
    }

    emit_progress(dl.app, "loader", &format!("Installing {} {lv}", title(loader)), 0, 1);
    let flags: &[&str] = if loader == "forge" { &["--installClient", "--install-client"] } else { &["--install-client", "--installClient"] };
    let mut last_output = String::new();
    for flag in flags {
        let (ok, output) = run_installer(dl.app, java, &installer, flag, &paths.root).await?;
        if ok && version_json_path(paths, &expected_id).exists() {
            emit_progress(dl.app, "loader", &format!("{} installed", title(loader)), 1, 1);
            return Ok(expected_id);
        }
        last_output = output;
        // Only retry with the other flag spelling when the option wasn't recognized.
        if !last_output.to_lowercase().contains("not a recognized option") && !last_output.contains("UnrecognizedOption") {
            break;
        }
    }
    // Some installers name the version differently: find the newest json that inherits from mc.
    if let Some(id) = find_installed_loader_version(paths, mc, loader) {
        return Ok(id);
    }
    let tail: String = last_output.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    bail!("The {} installer failed.\n{tail}", title(loader))
}

async fn run_installer(app: &AppHandle, java: &Path, installer: &Path, flag: &str, root: &Path) -> anyhow::Result<(bool, String)> {
    // Use java (console) rather than javaw so the installer's output is captured on every OS.
    let java = if java.file_name().map(|n| n == "javaw.exe").unwrap_or(false) { java.with_file_name("java.exe") } else { java.to_path_buf() };
    let mut cmd = tokio::process::Command::new(java);
    cmd.arg("-jar").arg(installer).arg(flag).arg(root).current_dir(root).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut child = cmd.spawn().context("couldn't start the loader installer")?;
    let mut out = String::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let readers: [Box<dyn tokio::io::AsyncRead + Unpin + Send>; 2] = [Box::new(child.stdout.take().unwrap()), Box::new(child.stderr.take().unwrap())];
    for r in readers {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(r).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);
    let mut n = 0u64;
    while let Some(line) = rx.recv().await {
        n += 1;
        if n % 20 == 0 {
            emit_progress(app, "loader", line.chars().take(90).collect::<String>().as_str(), 0, 1);
        }
        out.push_str(&line);
        out.push('\n');
    }
    let status = child.wait().await?;
    Ok((status.success(), out))
}

fn find_installed_loader_version(paths: &Paths, mc: &str, loader: &str) -> Option<String> {
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for e in std::fs::read_dir(paths.versions()).ok()?.flatten() {
        let id = e.file_name().to_string_lossy().to_string();
        if !id.to_lowercase().contains(loader) {
            continue;
        }
        let p = version_json_path(paths, &id);
        let Ok(v) = crate::core::read_json::<VersionJson>(&p) else { continue };
        if v.inherits_from.as_deref() != Some(mc) {
            continue;
        }
        let t = std::fs::metadata(&p).and_then(|m| m.modified()).ok()?;
        if best.as_ref().map(|(bt, _)| t > *bt).unwrap_or(true) {
            best = Some((t, id));
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neoforge_prefixes() {
        assert_eq!(neoforge_prefix("1.21.1"), "21.1.");
        assert_eq!(neoforge_prefix("1.21"), "21.0.");
        assert_eq!(neoforge_prefix("1.20.4"), "20.4.");
        assert_eq!(neoforge_prefix("1.21.10"), "21.10.");
        assert_eq!(neoforge_prefix("26.1"), "26.1.");
    }
}
