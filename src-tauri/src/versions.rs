//! Mojang version manifest, version JSON (with loader inheritance), rule
//! evaluation, and installing libraries / natives / assets / client jar.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::core::{get_json_cached, os_name, read_json, write_json, Paths};
use crate::download::{maven_path, DlItem, Downloader};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net";
const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<ManifestVersion>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub release_time: String,
    pub sha1: Option<String>,
}

pub async fn manifest(http: &reqwest::Client, paths: &Paths) -> anyhow::Result<Manifest> {
    get_json_cached(http, paths, MANIFEST_URL, "version_manifest_v2.json", 60 * 30).await
}

// ---------------------------------------------------------------------------
// Version JSON
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_version: Option<JavaVersionRef>,
    /// Forge/old versions sometimes name the jar to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jar: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<ArgValue>,
    #[serde(default)]
    pub jvm: Vec<ArgValue>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgValue {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: StringOrVec },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum StringOrVec {
    One(String),
    Many(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionDownloads {
    pub client: Option<Artifact>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionRef {
    pub major_version: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Library {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibDownloads>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Artifact {
    pub path: Option<String>,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub struct Features {
    pub custom_resolution: bool,
    pub quick_play_multiplayer: bool,
}

pub fn rules_allow(rules: Option<&Vec<Rule>>, f: &Features) -> bool {
    let Some(rules) = rules else { return true };
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for r in rules {
        if rule_matches(r, f) {
            allowed = r.action == "allow";
        }
    }
    allowed
}

fn rule_matches(r: &Rule, f: &Features) -> bool {
    if let Some(os) = &r.os {
        if let Some(name) = &os.name {
            if name != os_name() {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            let ours = if cfg!(target_arch = "aarch64") { "arm64" } else if cfg!(target_pointer_width = "64") { "x86_64" } else { "x86" };
            // "x86" in Mojang JSON means a 32-bit JVM.
            let matches = match arch.as_str() {
                "x86" => ours == "x86",
                "arm64" | "aarch64" => ours == "arm64",
                "x86_64" | "x64" | "amd64" => ours == "x86_64",
                _ => false,
            };
            if !matches {
                return false;
            }
        }
    }
    if let Some(features) = &r.features {
        for (k, want) in features {
            let have = match k.as_str() {
                "has_custom_resolution" => f.custom_resolution,
                "is_quick_play_multiplayer" => f.quick_play_multiplayer,
                _ => false, // demo user, quick play singleplayer/realms, etc.
            };
            if have != *want {
                return false;
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Loading + inheritance
// ---------------------------------------------------------------------------

pub fn version_json_path(paths: &Paths, id: &str) -> PathBuf {
    paths.versions().join(id).join(format!("{id}.json"))
}

/// Ensures the vanilla version JSON is on disk (hash-verified) and returns it.
pub async fn ensure_vanilla_json(http: &reqwest::Client, paths: &Paths, mc: &str) -> anyhow::Result<VersionJson> {
    let path = version_json_path(paths, mc);
    if let Ok(v) = read_json::<VersionJson>(&path) {
        return Ok(v);
    }
    let m = manifest(http, paths).await?;
    let entry = m.versions.iter().find(|v| v.id == mc).with_context(|| format!("Minecraft {mc} isn't in Mojang's version list"))?;
    let bytes = http.get(&entry.url).send().await?.error_for_status()?.bytes().await?;
    if let Some(expected) = &entry.sha1 {
        use sha1::Digest;
        let actual = hex::encode(sha1::Sha1::digest(&bytes));
        if &actual != expected {
            bail!("version JSON hash mismatch for {mc}");
        }
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, &bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Loads `id` and merges it onto its parents (`inheritsFrom`), child wins.
pub async fn resolve(http: &reqwest::Client, paths: &Paths, id: &str) -> anyhow::Result<VersionJson> {
    let path = version_json_path(paths, id);
    let child: VersionJson = match read_json(&path) {
        Ok(v) => v,
        Err(_) => ensure_vanilla_json(http, paths, id).await?,
    };
    match child.inherits_from.clone() {
        None => Ok(child),
        Some(parent_id) => {
            let parent = Box::pin(resolve(http, paths, &parent_id)).await?;
            Ok(merge(parent, child))
        }
    }
}

fn lib_key(name: &str) -> String {
    // group:artifact[:classifier] identifies a library regardless of version.
    let parts: Vec<&str> = name.split('@').next().unwrap_or(name).split(':').collect();
    match parts.len() {
        0..=2 => name.to_string(),
        3 => format!("{}:{}", parts[0], parts[1]),
        _ => format!("{}:{}:{}", parts[0], parts[1], parts[3]),
    }
}

fn merge(parent: VersionJson, child: VersionJson) -> VersionJson {
    let mut seen = HashSet::new();
    let mut libraries = Vec::new();
    for lib in child.libraries.into_iter().chain(parent.libraries) {
        if seen.insert(lib_key(&lib.name)) {
            libraries.push(lib);
        }
    }
    let arguments = match (parent.arguments, child.arguments) {
        (Some(mut p), Some(c)) => {
            p.game.extend(c.game);
            p.jvm.extend(c.jvm);
            Some(p)
        }
        (p, c) => c.or(p),
    };
    VersionJson {
        id: child.id,
        inherits_from: parent.inherits_from,
        kind: child.kind.or(parent.kind),
        main_class: child.main_class.or(parent.main_class),
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        arguments,
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        libraries,
        java_version: child.java_version.or(parent.java_version),
        jar: child.jar.or(parent.jar).or(Some(parent.id)),
    }
}

// ---------------------------------------------------------------------------
// Installing files
// ---------------------------------------------------------------------------

#[derive(Deserialize, Serialize)]
struct AssetIndex {
    objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "map_to_resources")]
    map_to_resources: bool,
    #[serde(default, rename = "virtual")]
    is_virtual: bool,
}

#[derive(Deserialize, Serialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

/// Everything a launch needs, after files are in place.
pub struct Prepared {
    pub version: VersionJson,
    pub classpath: Vec<PathBuf>,
    pub natives_dir: PathBuf,
    #[allow(dead_code)]
    pub client_jar: PathBuf,
    pub assets_index: String,
    /// For pre-1.7 versions that read assets from a virtual/legacy dir.
    pub game_assets_dir: PathBuf,
    pub java_major: u32,
}

pub fn library_downloads(lib: &Library, paths: &Paths, f: &Features) -> (Option<DlItem>, Option<DlItem>) {
    if !rules_allow(lib.rules.as_ref(), f) {
        return (None, None);
    }
    let libdir = paths.libraries();
    // Main artifact
    let main = match lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        Some(a) => {
            let rel = a.path.clone().or_else(|| maven_path(&lib.name));
            rel.map(|rel| {
                let item = DlItem::new(a.url.clone(), libdir.join(&rel)).sha1(a.sha1.clone()).size(a.size);
                // Some Forge libraries ship with an empty URL: they're produced by the installer.
                DlItem { urls: if a.url.is_empty() { vec![] } else { item.urls.clone() }, ..item }
            })
        }
        None if lib.natives.is_none() || lib.downloads.is_none() => maven_path(&lib.name).map(|rel| {
            let base = lib.url.clone().unwrap_or_else(|| LIBRARIES_URL.to_string());
            let base = if base.ends_with('/') { base } else { format!("{base}/") };
            DlItem::new(format!("{base}{rel}"), libdir.join(&rel)).sha1(lib.sha1.clone()).size(lib.size)
        }),
        None => None,
    };
    // Legacy natives (pre-1.19): a classifier jar to extract.
    let natives = lib.natives.as_ref().and_then(|n| n.get(os_name())).and_then(|classifier| {
        let classifier = classifier.replace("${arch}", if cfg!(target_pointer_width = "64") { "64" } else { "32" });
        let a = lib.downloads.as_ref()?.classifiers.as_ref()?.get(&classifier)?;
        let rel = a.path.clone().or_else(|| maven_path(&format!("{}:{}", lib.name, classifier)))?;
        Some(DlItem::new(a.url.clone(), libdir.join(rel)).sha1(a.sha1.clone()).size(a.size))
    });
    (main, natives)
}

pub async fn prepare(dl: &Downloader<'_>, version_id: &str, game_dir: &Path, f: &Features) -> anyhow::Result<Prepared> {
    let http = dl.http;
    let paths = dl.paths;
    let version = resolve(http, paths, version_id).await?;

    // --- client jar (from the root vanilla version)
    let jar_id = version.jar.clone().unwrap_or_else(|| version.id.clone());
    let client_jar = paths.versions().join(&jar_id).join(format!("{jar_id}.jar"));
    let client_dl = version.downloads.as_ref().and_then(|d| d.client.clone()).context("version has no client download")?;

    // --- libraries
    let mut items = vec![DlItem::new(client_dl.url.clone(), client_jar.clone()).sha1(client_dl.sha1.clone()).size(client_dl.size)];
    let mut classpath = Vec::new();
    let mut native_jars = Vec::new();
    for lib in &version.libraries {
        let (main, natives) = library_downloads(lib, paths, f);
        if let Some(m) = main {
            classpath.push(m.path.clone());
            if !m.urls.is_empty() {
                items.push(m);
            }
        }
        if let Some(n) = natives {
            native_jars.push(n.path.clone());
            items.push(n);
        }
    }
    classpath.push(client_jar.clone());

    // --- asset index
    let ai = version.asset_index.clone().context("version has no asset index")?;
    let index_path = paths.assets().join("indexes").join(format!("{}.json", ai.id));
    dl.run("libraries", "Downloading game files", vec![DlItem::new(ai.url.clone(), index_path.clone()).sha1(ai.sha1.clone()).size(ai.size)])
        .await?;
    let index: AssetIndex = read_json(&index_path)?;
    let objects_dir = paths.assets().join("objects");
    items.extend(index.objects.values().map(|o| {
        let p = format!("{}/{}", &o.hash[..2], o.hash);
        DlItem::new(format!("{RESOURCES_URL}/{p}"), objects_dir.join(&p)).sha1(Some(o.hash.clone())).size(Some(o.size))
    }));

    dl.run("game-files", "Downloading game files", items).await?;

    // --- legacy asset layouts
    let mut game_assets_dir = paths.assets().to_path_buf();
    if index.is_virtual || index.map_to_resources {
        let target = if index.map_to_resources { game_dir.join("resources") } else { paths.assets().join("virtual").join(&ai.id) };
        for (name, o) in &index.objects {
            let dest = crate::core::safe_join(&target, name)?;
            if !dest.exists() {
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(objects_dir.join(&o.hash[..2]).join(&o.hash), &dest)?;
            }
        }
        game_assets_dir = target;
    }

    // --- extract legacy natives
    let natives_dir = paths.natives(&version.id);
    if !native_jars.is_empty() && !natives_dir.join(".extracted").exists() {
        std::fs::create_dir_all(&natives_dir)?;
        for jar in &native_jars {
            extract_natives(jar, &natives_dir).with_context(|| format!("extracting natives from {}", jar.display()))?;
        }
        std::fs::write(natives_dir.join(".extracted"), b"")?;
    }
    std::fs::create_dir_all(&natives_dir)?;

    // Like the official launcher, a loader version gets its own copy of the
    // client jar named after it (Forge's module ignore-list relies on this).
    let mut client_jar = client_jar;
    if jar_id != version.id {
        let own = paths.versions().join(&version.id).join(format!("{}.jar", version.id));
        let same = std::fs::metadata(&own).map(|m| Some(m.len()) == std::fs::metadata(&client_jar).ok().map(|c| c.len())).unwrap_or(false);
        if !same {
            std::fs::create_dir_all(own.parent().unwrap())?;
            let _ = std::fs::remove_file(&own);
            if std::fs::hard_link(&client_jar, &own).is_err() {
                std::fs::copy(&client_jar, &own)?;
            }
        }
        if let Some(last) = classpath.last_mut() {
            *last = own.clone();
        }
        client_jar = own;
    }

    let java_major = version.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
    write_json(&paths.versions().join(&version.id).join("halo-resolved.json"), &version).ok();
    Ok(Prepared { assets_index: ai.id, version, classpath, natives_dir, client_jar, game_assets_dir, java_major })
}

fn extract_natives(jar: &Path, dest: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(jar)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with("META-INF/") || entry.is_dir() {
            continue;
        }
        let out = crate::core::safe_join(dest, &name)?;
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(out)?;
        std::io::copy(&mut entry, &mut f)?;
    }
    Ok(())
}

/// Release + snapshot list for the UI.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameVersion {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}

pub async fn game_versions(http: &reqwest::Client, paths: &Paths) -> anyhow::Result<Vec<GameVersion>> {
    let m = manifest(http, paths).await?;
    Ok(m.versions.into_iter().map(|v| GameVersion { id: v.id, kind: v.kind, release_time: v.release_time }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(action: &str, os: Option<&str>) -> Rule {
        Rule { action: action.into(), os: os.map(|n| OsRule { name: Some(n.into()), arch: None, version: None }), features: None }
    }

    #[test]
    fn rules_evaluate_last_match() {
        let f = Features::default();
        assert!(rules_allow(None, &f));
        let allow_all_but_osx = vec![rule("allow", None), rule("disallow", Some("osx"))];
        assert_eq!(rules_allow(Some(&allow_all_but_osx), &f), os_name() != "osx");
        let only_ours = vec![rule("allow", Some(os_name()))];
        assert!(rules_allow(Some(&only_ours), &f));
        let only_other = vec![rule("allow", Some(if os_name() == "windows" { "linux" } else { "windows" }))];
        assert!(!rules_allow(Some(&only_other), &f));
    }

    #[test]
    fn feature_rules() {
        let r = vec![Rule { action: "allow".into(), os: None, features: Some(HashMap::from([("has_custom_resolution".into(), true)])) }];
        assert!(!rules_allow(Some(&r), &Features::default()));
        assert!(rules_allow(Some(&r), &Features { custom_resolution: true, ..Default::default() }));
    }

    #[test]
    fn merge_child_wins_and_dedupes() {
        let lib = |n: &str| Library { name: n.into(), downloads: None, url: None, rules: None, natives: None, sha1: None, size: None };
        let parent = VersionJson { id: "1.21.1".into(), main_class: Some("net.minecraft.client.main.Main".into()), libraries: vec![lib("org.ow2.asm:asm:9.6"), lib("a:b:1")], ..Default::default() };
        let child = VersionJson { id: "fabric".into(), inherits_from: Some("1.21.1".into()), main_class: Some("net.fabricmc.loader.impl.launch.knot.KnotClient".into()), libraries: vec![lib("org.ow2.asm:asm:9.8")], ..Default::default() };
        let m = merge(parent, child);
        assert_eq!(m.main_class.unwrap(), "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(m.libraries.len(), 2);
        assert_eq!(m.libraries[0].name, "org.ow2.asm:asm:9.8");
        assert_eq!(m.jar.unwrap(), "1.21.1");
    }

    #[test]
    fn parses_modern_arguments() {
        let json = r#"{"id":"x","arguments":{"game":["--username","${auth_player_name}",{"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}"]}],"jvm":[{"rules":[{"action":"allow","os":{"name":"osx"}}],"value":"-XstartOnFirstThread"},"-cp","${classpath}"]},"libraries":[]}"#;
        let v: VersionJson = serde_json::from_str(json).unwrap();
        assert_eq!(v.arguments.unwrap().game.len(), 3);
    }
}
