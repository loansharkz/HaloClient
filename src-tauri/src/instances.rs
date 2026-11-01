//! Instances: config, content (mods/resource packs/shaders), screenshots, worlds.
//!
//! Layout: instances/<id>/instance.json, instances/<id>/content.json (what Halo
//! installed and from where), instances/<id>/minecraft/ (the game directory).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::core::{now_ms, read_json, write_json, Paths};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootSample {
    pub ms: u64,
    pub at: i64,
    pub cds: bool,
    pub perf: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String, // vanilla | fabric | quilt | forge | neoforge
    pub loader_version: Option<String>,
    /// Installed version id to launch (e.g. "fabric-loader-0.16.10-1.21.1").
    pub version_id: Option<String>,
    pub icon: String,
    pub icon_server: Option<String>,
    pub icon_path: Option<String>,
    pub hue: u32,
    pub group: Option<String>,
    pub created: i64,
    pub last_played: Option<i64>,
    pub playtime_secs: u64,
    pub ram_mb: Option<u32>,
    pub java_path: Option<String>,
    pub jvm_args: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pre_launch: Option<String>,
    pub post_exit: Option<String>,
    pub account_id: Option<String>,
    pub perf_profile: bool,
    pub cds: bool,
    pub tuned_flags: bool,
    pub last_boot_ms: Option<u64>,
    pub boot_history: Vec<BootSample>,
}

impl Default for Instance {
    fn default() -> Self {
        Instance {
            id: String::new(),
            name: String::new(),
            mc_version: String::new(),
            loader: "vanilla".into(),
            loader_version: None,
            version_id: None,
            icon: "box".into(),
            icon_server: None,
            icon_path: None,
            hue: 275,
            group: None,
            created: now_ms(),
            last_played: None,
            playtime_secs: 0,
            ram_mb: None,
            java_path: None,
            jvm_args: String::new(),
            width: None,
            height: None,
            pre_launch: None,
            post_exit: None,
            account_id: None,
            perf_profile: false,
            cds: true,
            tuned_flags: true,
            last_boot_ms: None,
            boot_history: vec![],
        }
    }
}

impl Instance {
    /// Median of the last few boot times for a given configuration.
    pub fn median_boot(&self, cds: bool) -> Option<u64> {
        let mut v: Vec<u64> = self.boot_history.iter().rev().filter(|b| b.cds == cds).take(5).map(|b| b.ms).collect();
        if v.is_empty() {
            return None;
        }
        v.sort();
        Some(v[v.len() / 2])
    }
}

pub fn game_dir(paths: &Paths, id: &str) -> PathBuf {
    paths.instance(id).join("minecraft")
}

fn instance_file(paths: &Paths, id: &str) -> PathBuf {
    paths.instance(id).join("instance.json")
}

pub fn load(paths: &Paths, id: &str) -> anyhow::Result<Instance> {
    read_json(&instance_file(paths, id)).with_context(|| format!("instance {id} not found"))
}

pub fn save(paths: &Paths, inst: &Instance) -> anyhow::Result<()> {
    write_json(&instance_file(paths, &inst.id), inst)
}

pub fn list(paths: &Paths) -> Vec<Instance> {
    let mut out: Vec<Instance> = std::fs::read_dir(paths.instances())
        .map(|rd| rd.flatten().filter_map(|e| read_json(&e.path().join("instance.json")).ok()).collect())
        .unwrap_or_default();
    out.sort_by_key(|i| std::cmp::Reverse(i.last_played.unwrap_or(i.created)));
    out
}

pub fn new_id(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() { "instance".to_string() } else { slug.chars().take(32).collect() };
    format!("{slug}-{}", &uuid::Uuid::new_v4().simple().to_string()[..6])
}

pub fn create(paths: &Paths, mut inst: Instance) -> anyhow::Result<Instance> {
    if inst.name.trim().is_empty() {
        bail!("Give the instance a name.");
    }
    inst.id = new_id(&inst.name);
    inst.created = now_ms();
    for d in ["mods", "resourcepacks", "shaderpacks", "saves", "screenshots", "config"] {
        std::fs::create_dir_all(game_dir(paths, &inst.id).join(d))?;
    }
    save(paths, &inst)?;
    Ok(inst)
}

pub fn delete(paths: &Paths, id: &str) -> anyhow::Result<()> {
    let dir = paths.instance(id);
    if !dir.starts_with(paths.instances()) || id.is_empty() || id.contains("..") {
        bail!("invalid instance id");
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

pub fn duplicate(paths: &Paths, id: &str) -> anyhow::Result<Instance> {
    let mut inst = load(paths, id)?;
    inst.name = format!("{} (copy)", inst.name);
    inst.id = new_id(&inst.name);
    inst.created = now_ms();
    inst.last_played = None;
    inst.playtime_secs = 0;
    inst.boot_history.clear();
    inst.last_boot_ms = None;
    crate::core::copy_dir(&paths.instance(id), &paths.instance(&inst.id))?;
    save(paths, &inst)?;
    Ok(inst)
}

// ---------------------------------------------------------------------------
// Content tracking
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentMeta {
    pub source: String, // modrinth | curseforge | halo | local
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub slug: Option<String>,
    pub title: Option<String>,
    pub version_number: Option<String>,
    pub icon_url: Option<String>,
    pub perf: bool,
    pub hud: bool,
    pub dependency: bool,
}

pub type ContentIndex = HashMap<String, ContentMeta>; // key: "<kind-dir>/<filename without .disabled>"

pub fn content_index(paths: &Paths, id: &str) -> ContentIndex {
    read_json(&paths.instance(id).join("content.json")).unwrap_or_default()
}

pub fn save_content_index(paths: &Paths, id: &str, idx: &ContentIndex) -> anyhow::Result<()> {
    write_json(&paths.instance(id).join("content.json"), idx)
}

pub fn kind_dir(kind: &str) -> anyhow::Result<&'static str> {
    Ok(match kind {
        "mod" => "mods",
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => bail!("unknown content kind {kind}"),
    })
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    pub key: String,
    pub kind: String,
    pub file_name: String,
    pub name: String,
    pub version: Option<String>,
    pub enabled: bool,
    pub size: u64,
    pub meta: Option<ContentMeta>,
}

pub fn list_content(paths: &Paths, id: &str) -> Vec<ContentItem> {
    let idx = content_index(paths, id);
    let gd = game_dir(paths, id);
    let mut out = vec![];
    for (kind, dir) in [("mod", "mods"), ("resourcepack", "resourcepacks"), ("shader", "shaderpacks")] {
        let Ok(rd) = std::fs::read_dir(gd.join(dir)) else { continue };
        for e in rd.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            let path = e.path();
            let enabled = !fname.ends_with(".disabled");
            let base = fname.trim_end_matches(".disabled").to_string();
            let ok_ext = base.ends_with(".jar") || base.ends_with(".zip") || (kind != "mod" && path.is_dir());
            if !ok_ext {
                continue;
            }
            let key = format!("{dir}/{base}");
            let meta = idx.get(&key).cloned();
            let (mut name, mut version) = (meta.as_ref().and_then(|m| m.title.clone()), meta.as_ref().and_then(|m| m.version_number.clone()));
            if name.is_none() && kind == "mod" && path.is_file() {
                if let Some((n, v)) = read_mod_info(&path) {
                    name = Some(n);
                    version = version.or(v);
                }
            }
            out.push(ContentItem {
                key,
                kind: kind.into(),
                name: name.unwrap_or_else(|| base.trim_end_matches(".jar").trim_end_matches(".zip").to_string()),
                file_name: fname,
                version,
                enabled,
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                meta,
            });
        }
    }
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

/// Reads display name + version from fabric.mod.json / quilt.mod.json / mods.toml.
pub fn read_mod_info(jar: &Path) -> Option<(String, Option<String>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut read = |name: &str| -> Option<String> {
        let mut f = zip.by_name(name).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    };
    if let Some(s) = read("fabric.mod.json") {
        let v: serde_json::Value = serde_json::from_str(&s).ok()?;
        return Some((v["name"].as_str().or(v["id"].as_str())?.to_string(), v["version"].as_str().map(String::from)));
    }
    if let Some(s) = read("quilt.mod.json") {
        let v: serde_json::Value = serde_json::from_str(&s).ok()?;
        let l = &v["quilt_loader"];
        return Some((l["metadata"]["name"].as_str().or(l["id"].as_str())?.to_string(), l["version"].as_str().map(String::from)));
    }
    for toml in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(s) = read(toml) {
            let name = regex::Regex::new(r#"(?m)^\s*displayName\s*=\s*"([^"]+)""#).ok()?.captures(&s).map(|c| c[1].to_string());
            let ver = regex::Regex::new(r#"(?m)^\s*version\s*=\s*"([^"]+)""#).ok()?.captures(&s).map(|c| c[1].to_string()).filter(|v| !v.contains("${"));
            if let Some(n) = name {
                return Some((n, ver));
            }
        }
    }
    None
}

fn content_path(paths: &Paths, id: &str, key: &str) -> anyhow::Result<PathBuf> {
    crate::core::safe_join(&game_dir(paths, id), key)
}

pub fn set_enabled(paths: &Paths, id: &str, key: &str, enabled: bool) -> anyhow::Result<()> {
    let on = content_path(paths, id, key)?;
    let off = PathBuf::from(format!("{}.disabled", on.display()));
    if enabled && off.exists() {
        std::fs::rename(off, on)?;
    } else if !enabled && on.exists() {
        std::fs::rename(on, off)?;
    }
    Ok(())
}

pub fn remove_content(paths: &Paths, id: &str, key: &str) -> anyhow::Result<()> {
    let on = content_path(paths, id, key)?;
    let off = PathBuf::from(format!("{}.disabled", on.display()));
    for p in [on, off] {
        if p.is_dir() {
            std::fs::remove_dir_all(&p)?;
        } else if p.exists() {
            std::fs::remove_file(&p)?;
        }
    }
    let mut idx = content_index(paths, id);
    idx.remove(key);
    save_content_index(paths, id, &idx)
}

// ---------------------------------------------------------------------------
// Screenshots & worlds
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub path: String,
    pub name: String,
    pub taken: i64,
}

pub fn screenshots(paths: &Paths, id: &str) -> Vec<Screenshot> {
    let mut v: Vec<Screenshot> = std::fs::read_dir(game_dir(paths, id).join("screenshots"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".png"))
                .map(|e| Screenshot {
                    path: e.path().to_string_lossy().into(),
                    name: e.file_name().to_string_lossy().into(),
                    taken: e
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|s| std::cmp::Reverse(s.taken));
    v
}

pub fn worlds(paths: &Paths, id: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(game_dir(paths, id).join("saves"))
        .map(|rd| rd.flatten().filter(|e| e.path().join("level.dat").exists()).map(|e| e.file_name().to_string_lossy().into()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_slugged_and_unique() {
        let a = new_id("Halo PvP!");
        assert!(a.starts_with("halo-pvp-"));
        assert_ne!(a, new_id("Halo PvP!"));
        assert!(new_id("???").starts_with("instance-"));
    }
}
