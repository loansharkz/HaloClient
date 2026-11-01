//! Content: Modrinth (search, projects, install with dependency resolution,
//! updates), CurseForge (behind an API key, respecting allowModDistribution),
//! the Performance profile, Halo HUD, and modpack import/export.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::core::{get_json, safe_join, sha1_file, sha512_file, Paths};
use crate::download::{DlItem, Downloader};
use crate::instances::{self, content_index, kind_dir, save_content_index, ContentMeta, Instance};

const MODRINTH: &str = "https://api.modrinth.com/v2";
const CURSEFORGE: &str = "https://api.curseforge.com/v1";

/// Fabric performance stack (all client-side, server-safe).
pub const PERF_STACK: &[&str] = &["sodium", "lithium", "ferrite-core", "modernfix", "immediatelyfast", "entityculling"];

// ---------------------------------------------------------------------------
// Shared shapes for the UI (both sources map into these)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub source: String,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub updated: String,
    pub categories: Vec<String>,
    pub client_side: String,
    pub server_side: String,
    pub project_type: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub total: u64,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub source: String, // modrinth | curseforge
    pub query: String,
    pub project_type: String, // mod | modpack | resourcepack | shader | datapack
    pub game_version: Option<String>,
    pub loader: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub environment: Option<String>, // client | server
    pub sort: String,                // relevance | downloads | follows | newest | updated
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "twenty")]
    pub limit: u32,
}
fn twenty() -> u32 {
    20
}

// ---------------------------------------------------------------------------
// Modrinth
// ---------------------------------------------------------------------------

pub async fn search(http: &reqwest::Client, q: &SearchQuery) -> anyhow::Result<SearchResult> {
    if q.source == "curseforge" {
        return cf_search(http, q).await;
    }
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", q.project_type)]];
    if let Some(v) = q.game_version.as_ref().filter(|v| !v.is_empty()) {
        facets.push(vec![format!("versions:{v}")]);
    }
    if let Some(l) = q.loader.as_ref().filter(|l| !l.is_empty() && *l != "vanilla") {
        if q.project_type == "mod" || q.project_type == "modpack" {
            // Quilt runs Fabric mods.
            if l == "quilt" {
                facets.push(vec!["categories:quilt".into(), "categories:fabric".into()]);
            } else {
                facets.push(vec![format!("categories:{l}")]);
            }
        }
    }
    for c in &q.categories {
        facets.push(vec![format!("categories:{c}")]);
    }
    match q.environment.as_deref() {
        Some("client") => facets.push(vec!["client_side:required".into(), "client_side:optional".into()]),
        Some("server") => facets.push(vec!["server_side:required".into(), "server_side:optional".into()]),
        _ => {}
    }
    let index = match q.sort.as_str() {
        "downloads" | "follows" | "newest" | "updated" => q.sort.as_str(),
        _ => "relevance",
    };
    let url = reqwest::Url::parse_with_params(
        &format!("{MODRINTH}/search"),
        &[
            ("query", q.query.as_str()),
            ("facets", &serde_json::to_string(&facets)?),
            ("index", index),
            ("offset", &q.offset.to_string()),
            ("limit", &q.limit.min(100).to_string()),
        ],
    )?;
    let v: Value = get_json(http, url.as_str()).await?;
    let hits = v["hits"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|h| SearchHit {
                    source: "modrinth".into(),
                    id: s(&h["project_id"]),
                    slug: s(&h["slug"]),
                    title: s(&h["title"]),
                    author: s(&h["author"]),
                    description: s(&h["description"]),
                    icon_url: h["icon_url"].as_str().filter(|s| !s.is_empty()).map(String::from),
                    banner_url: h["featured_gallery"].as_str().or(h["gallery"][0].as_str()).map(String::from),
                    downloads: h["downloads"].as_u64().unwrap_or(0),
                    follows: h["follows"].as_u64().unwrap_or(0),
                    updated: s(&h["date_modified"]),
                    categories: h["display_categories"].as_array().map(|c| c.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
                    client_side: s(&h["client_side"]),
                    server_side: s(&h["server_side"]),
                    project_type: s(&h["project_type"]),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(SearchResult { hits, total: v["total_hits"].as_u64().unwrap_or(0) })
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

/// Full project page data, passed through (the UI renders the markdown body safely).
pub async fn project(http: &reqwest::Client, source: &str, id: &str) -> anyhow::Result<Value> {
    if source == "curseforge" {
        let key = cf_key()?;
        let mut v: Value = http.get(format!("{CURSEFORGE}/mods/{id}")).header("x-api-key", &key).send().await?.error_for_status()?.json().await?;
        let desc: Value = http
            .get(format!("{CURSEFORGE}/mods/{id}/description"))
            .header("x-api-key", &key)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .unwrap_or_default();
        v["data"]["bodyHtml"] = desc["data"].clone();
        return Ok(v["data"].clone());
    }
    let mut p: Value = get_json(http, &format!("{MODRINTH}/project/{id}")).await?;
    // Team members → author name
    if let Ok(members) = get_json::<Value>(http, &format!("{MODRINTH}/project/{id}/members")).await {
        if let Some(owner) = members.as_array().and_then(|m| m.iter().find(|x| x["role"] == "Owner").or(m.first())) {
            p["author"] = owner["user"]["username"].clone();
        }
    }
    Ok(p)
}

pub async fn project_versions(http: &reqwest::Client, id: &str, loaders: &[String], game_versions: &[String]) -> anyhow::Result<Vec<Value>> {
    let mut params = vec![];
    if !loaders.is_empty() {
        params.push(("loaders", serde_json::to_string(loaders)?));
    }
    if !game_versions.is_empty() {
        params.push(("game_versions", serde_json::to_string(game_versions)?));
    }
    let url = reqwest::Url::parse_with_params(&format!("{MODRINTH}/project/{id}/version"), &params)?;
    get_json(http, url.as_str()).await
}

/// Modrinth loader filters for a content kind on an instance.
fn loader_filter(kind: &str, inst: &Instance) -> Vec<String> {
    match kind {
        "mod" => match inst.loader.as_str() {
            "quilt" => vec!["quilt".into(), "fabric".into()],
            "vanilla" => vec![],
            l => vec![l.to_string()],
        },
        "datapack" => vec!["datapack".into()],
        _ => vec![],
    }
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub installed: Vec<String>,
    pub warnings: Vec<String>,
    /// Shader pack installed without Iris/Oculus present.
    pub needs_shader_loader: Option<String>,
    /// CurseForge projects that disallow third-party downloads: open this page instead.
    pub manual_url: Option<String>,
}

pub struct InstallCtx<'a> {
    pub dl: &'a Downloader<'a>,
    pub inst: &'a Instance,
    /// For data packs: the world folder name.
    pub world: Option<String>,
}

/// Installs a Modrinth project (best matching version unless `version_id` is
/// given) plus its required dependencies.
pub async fn install_modrinth(ctx: &InstallCtx<'_>, project_id: &str, version_id: Option<&str>, kind_hint: Option<&str>) -> anyhow::Result<InstallResult> {
    let mut result = InstallResult::default();
    let mut visited = HashSet::new();
    install_rec(ctx, project_id, version_id, kind_hint, false, &mut visited, &mut result, 0).await?;
    // Shader packs need a shader loader mod.
    if kind_hint == Some("shader") {
        let has = instances::list_content(ctx.dl.paths, &ctx.inst.id)
            .iter()
            .any(|c| c.kind == "mod" && ["iris", "oculus"].iter().any(|n| c.name.to_lowercase().contains(n)));
        if !has {
            result.needs_shader_loader = Some(if matches!(ctx.inst.loader.as_str(), "forge" | "neoforge") { "oculus" } else { "iris" }.into());
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn install_rec<'a>(
    ctx: &'a InstallCtx<'a>,
    project_id: &'a str,
    version_id: Option<&'a str>,
    kind_hint: Option<&'a str>,
    is_dependency: bool,
    visited: &'a mut HashSet<String>,
    result: &'a mut InstallResult,
    depth: u32,
) -> futures::future::BoxFuture<'a, anyhow::Result<()>> {
    Box::pin(async move {
        if depth > 6 || !visited.insert(project_id.to_string()) {
            return Ok(());
        }
        let http = ctx.dl.http;
        let paths = ctx.dl.paths;
        let inst = ctx.inst;
        let proj: Value = get_json(http, &format!("{MODRINTH}/project/{project_id}")).await?;
        let kind = match kind_hint.filter(|_| !is_dependency) {
            Some(k) => k.to_string(),
            None => match proj["project_type"].as_str() {
                Some("resourcepack") => "resourcepack".into(),
                Some("shader") => "shader".into(),
                Some("modpack") => bail!("Modpacks are installed as a new instance (Instances → Import)."),
                _ => "mod".into(),
            },
        };
        if kind == "mod" && inst.loader == "vanilla" {
            bail!("{} is a mod. Vanilla instances can't load mods. Create a Fabric, Quilt, Forge or NeoForge instance.", s(&proj["title"]));
        }

        let version: Value = match version_id {
            Some(v) => get_json(http, &format!("{MODRINTH}/version/{v}")).await?,
            None => {
                let list = project_versions(http, project_id, &loader_filter(&kind, inst), &[inst.mc_version.clone()]).await?;
                let pick = list.iter().find(|v| v["version_type"] == "release").or(list.first()).cloned();
                match pick {
                    Some(v) => v,
                    None => {
                        let msg = format!("{} has no version for {} {}", s(&proj["title"]), loader_label(&inst.loader), inst.mc_version);
                        if is_dependency {
                            result.warnings.push(msg);
                            return Ok(());
                        }
                        bail!(msg);
                    }
                }
            }
        };

        let file = version["files"].as_array().and_then(|f| f.iter().find(|x| x["primary"] == true).or(f.first())).context("version has no files")?.clone();
        let filename = s(&file["filename"]);
        if filename.is_empty() || filename.contains('/') || filename.contains('\\') || filename.contains("..") {
            bail!("unsafe file name from Modrinth");
        }

        // Where it goes
        let gd = instances::game_dir(paths, &inst.id);
        let (dir_key, dest_dir) = if kind == "datapack" {
            let world = ctx.world.clone().context("Pick a world to add this data pack to")?;
            let d = safe_join(&gd, &format!("saves/{world}/datapacks"))?;
            (format!("saves/{world}/datapacks"), d)
        } else {
            let k = kind_dir(&kind)?;
            (k.to_string(), gd.join(k))
        };

        // Remove an older version of the same project (update / reinstall).
        let mut idx = content_index(paths, &inst.id);
        let old: Vec<String> = idx.iter().filter(|(_, m)| m.project_id.as_deref() == Some(project_id)).map(|(k, _)| k.clone()).collect();
        for k in old {
            if k != format!("{dir_key}/{filename}") {
                let _ = instances::remove_content(paths, &inst.id, &k);
            }
        }
        idx = content_index(paths, &inst.id);

        let item = DlItem::new(s(&file["url"]), dest_dir.join(&filename))
            .sha1(file["hashes"]["sha1"].as_str().map(String::from))
            .sha512(file["hashes"]["sha512"].as_str().map(String::from))
            .size(file["size"].as_u64())
            .cached();
        ctx.dl.run("content", &format!("Installing {}", s(&proj["title"])), vec![item]).await?;

        idx.insert(
            format!("{dir_key}/{filename}"),
            ContentMeta {
                source: "modrinth".into(),
                project_id: Some(project_id.to_string()),
                version_id: version["id"].as_str().map(String::from),
                slug: proj["slug"].as_str().map(String::from),
                title: proj["title"].as_str().map(String::from),
                version_number: version["version_number"].as_str().map(String::from),
                icon_url: proj["icon_url"].as_str().map(String::from),
                perf: PERF_STACK.contains(&proj["slug"].as_str().unwrap_or("")),
                hud: false,
                dependency: is_dependency,
            },
        );
        save_content_index(paths, &inst.id, &idx)?;
        result.installed.push(s(&proj["title"]));

        // Dependencies
        if let Some(deps) = version["dependencies"].as_array() {
            for d in deps.clone() {
                match d["dependency_type"].as_str() {
                    Some("required") => {
                        let Some(pid) = d["project_id"].as_str().map(String::from) else { continue };
                        let installed = content_index(paths, &inst.id).values().any(|m| m.project_id.as_deref() == Some(pid.as_str()));
                        if installed {
                            continue;
                        }
                        let vid = d["version_id"].as_str().map(String::from);
                        if let Err(e) = install_rec(ctx, &pid, vid.as_deref(), None, true, visited, result, depth + 1).await {
                            result.warnings.push(format!("Dependency not installed: {e:#}"));
                        }
                    }
                    Some("incompatible") => {
                        if let Some(pid) = d["project_id"].as_str() {
                            if content_index(paths, &inst.id).values().any(|m| m.project_id.as_deref() == Some(pid)) {
                                result.warnings.push(format!("{} is marked incompatible with another installed mod.", s(&proj["title"])));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })
}

fn loader_label(l: &str) -> &str {
    match l {
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        _ => "Vanilla",
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub key: String,
    pub project_id: String,
    pub current: String,
    pub latest: String,
    pub latest_version_id: String,
}

/// Checks Modrinth-tracked content for newer versions matching the instance.
pub async fn check_updates(http: &reqwest::Client, paths: &Paths, inst: &Instance) -> anyhow::Result<Vec<UpdateInfo>> {
    let gd = instances::game_dir(paths, &inst.id);
    let idx = content_index(paths, &inst.id);
    let mut by_hash: HashMap<String, (String, ContentMeta)> = HashMap::new();
    for (key, meta) in &idx {
        if meta.source != "modrinth" {
            continue;
        }
        let p = gd.join(key);
        let p = if p.exists() { p } else { PathBuf::from(format!("{}.disabled", p.display())) };
        if let Ok(h) = sha1_file(&p).await {
            by_hash.insert(h, (key.clone(), meta.clone()));
        }
    }
    if by_hash.is_empty() {
        return Ok(vec![]);
    }
    let kind_loaders = loader_filter("mod", inst);
    let body = json!({
        "hashes": by_hash.keys().collect::<Vec<_>>(),
        "algorithm": "sha1",
        "loaders": kind_loaders,
        "game_versions": [inst.mc_version],
    });
    let resp: HashMap<String, Value> = http.post(format!("{MODRINTH}/version_files/update")).json(&body).send().await?.error_for_status()?.json().await?;
    let mut out = vec![];
    for (hash, latest) in resp {
        let Some((key, meta)) = by_hash.get(&hash) else { continue };
        let latest_id = s(&latest["id"]);
        if Some(latest_id.as_str()) != meta.version_id.as_deref() && !latest_id.is_empty() {
            out.push(UpdateInfo {
                key: key.clone(),
                project_id: meta.project_id.clone().unwrap_or_default(),
                current: meta.version_number.clone().unwrap_or_default(),
                latest: s(&latest["version_number"]),
                latest_version_id: latest_id,
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Performance profile + Halo HUD
// ---------------------------------------------------------------------------

pub async fn install_perf_profile(ctx: &InstallCtx<'_>, only: Option<Vec<String>>) -> anyhow::Result<InstallResult> {
    if !matches!(ctx.inst.loader.as_str(), "fabric" | "quilt") {
        bail!("The Performance profile is for Fabric and Quilt instances.");
    }
    let mut result = InstallResult::default();
    for slug in PERF_STACK {
        if let Some(o) = &only {
            if !o.iter().any(|x| x == slug) {
                continue;
            }
        }
        match install_modrinth(ctx, slug, None, Some("mod")).await {
            Ok(r) => {
                result.installed.extend(r.installed);
                result.warnings.extend(r.warnings);
            }
            Err(e) => result.warnings.push(format!("{slug}: {e:#}")),
        }
    }
    // Mark as perf so the UI can tag them.
    let mut idx = content_index(ctx.dl.paths, &ctx.inst.id);
    for m in idx.values_mut() {
        if PERF_STACK.contains(&m.slug.as_deref().unwrap_or("")) {
            m.perf = true;
        }
    }
    save_content_index(ctx.dl.paths, &ctx.inst.id, &idx)?;
    Ok(result)
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HudInfo {
    pub version: String,
    pub minecraft: Vec<String>,
    pub file: String,
}

/// Bundled Halo HUD build (CI places it in resources/halo-hud/).
pub fn hud_info(app: &AppHandle) -> Option<(HudInfo, PathBuf)> {
    let dir = app.path().resource_dir().ok()?.join("resources").join("halo-hud");
    let info: HudInfo = crate::core::read_json(&dir.join("halo-hud.json")).ok()?;
    let jar = dir.join(&info.file);
    jar.exists().then_some((info, jar))
}

pub fn hud_supported(app: &AppHandle, inst: &Instance) -> bool {
    inst.loader == "fabric" && hud_info(app).map(|(i, _)| i.minecraft.iter().any(|v| v == &inst.mc_version)).unwrap_or(false)
}

pub fn install_halo_hud(app: &AppHandle, paths: &Paths, inst: &Instance) -> anyhow::Result<()> {
    let (info, jar) = hud_info(app).context("Halo HUD isn't bundled in this build")?;
    if !hud_supported(app, inst) {
        bail!("Halo HUD supports Fabric on Minecraft {}", info.minecraft.join(", "));
    }
    let mods = instances::game_dir(paths, &inst.id).join("mods");
    std::fs::create_dir_all(&mods)?;
    let mut idx = content_index(paths, &inst.id);
    // Remove older Halo HUD builds.
    let old: Vec<String> = idx.iter().filter(|(_, m)| m.hud).map(|(k, _)| k.clone()).collect();
    for k in old {
        let _ = instances::remove_content(paths, &inst.id, &k);
        idx.remove(&k);
    }
    std::fs::copy(&jar, mods.join(&info.file))?;
    idx.insert(
        format!("mods/{}", info.file),
        ContentMeta { source: "halo".into(), title: Some("Halo HUD".into()), version_number: Some(info.version.clone()), hud: true, ..Default::default() },
    );
    save_content_index(paths, &inst.id, &idx)
}

/// Keeps an installed Halo HUD in step with the bundled build.
pub fn sync_halo_hud(app: &AppHandle, paths: &Paths, inst: &Instance) -> anyhow::Result<()> {
    let idx = content_index(paths, &inst.id);
    let Some((key, meta)) = idx.iter().find(|(_, m)| m.hud) else { return Ok(()) };
    let Some((info, _)) = hud_info(app) else { return Ok(()) };
    if meta.version_number.as_deref() != Some(info.version.as_str()) {
        let enabled = instances::game_dir(paths, &inst.id).join(key).exists();
        if enabled {
            install_halo_hud(app, paths, inst)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// CurseForge (requires CURSEFORGE_API_KEY; the key never leaves the backend)
// ---------------------------------------------------------------------------

pub fn cf_key() -> anyhow::Result<String> {
    std::env::var("CURSEFORGE_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
        .or_else(|| option_env!("CURSEFORGE_API_KEY").map(String::from).filter(|k| !k.is_empty()))
        .context("CurseForge isn't set up in this build (CURSEFORGE_API_KEY)")
}

pub fn cf_available() -> bool {
    cf_key().is_ok()
}

fn cf_class(project_type: &str) -> u32 {
    match project_type {
        "modpack" => 4471,
        "resourcepack" => 12,
        "shader" => 6552,
        "datapack" => 6945,
        _ => 6,
    }
}

fn cf_loader(l: &str) -> Option<u32> {
    match l {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

async fn cf_search(http: &reqwest::Client, q: &SearchQuery) -> anyhow::Result<SearchResult> {
    let key = cf_key()?;
    let sort = match q.sort.as_str() {
        "downloads" => "6",
        "updated" | "newest" => "3",
        "follows" => "2",
        _ => "1",
    };
    let mut params = vec![
        ("gameId", "432".to_string()),
        ("classId", cf_class(&q.project_type).to_string()),
        ("searchFilter", q.query.clone()),
        ("sortField", sort.into()),
        ("sortOrder", "desc".into()),
        ("index", q.offset.to_string()),
        ("pageSize", q.limit.min(50).to_string()),
    ];
    if let Some(v) = q.game_version.as_ref().filter(|v| !v.is_empty()) {
        params.push(("gameVersion", v.clone()));
    }
    if let Some(l) = q.loader.as_deref().and_then(cf_loader) {
        if q.project_type == "mod" || q.project_type == "modpack" {
            params.push(("modLoaderType", l.to_string()));
        }
    }
    let url = reqwest::Url::parse_with_params(&format!("{CURSEFORGE}/mods/search"), &params)?;
    let v: Value = http.get(url).header("x-api-key", key).send().await?.error_for_status()?.json().await?;
    let hits = v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|m| SearchHit {
                    source: "curseforge".into(),
                    id: m["id"].as_u64().unwrap_or(0).to_string(),
                    slug: s(&m["slug"]),
                    title: s(&m["name"]),
                    author: s(&m["authors"][0]["name"]),
                    description: s(&m["summary"]),
                    icon_url: m["logo"]["thumbnailUrl"].as_str().map(String::from),
                    banner_url: m["screenshots"][0]["thumbnailUrl"].as_str().map(String::from),
                    downloads: m["downloadCount"].as_f64().unwrap_or(0.0) as u64,
                    follows: m["thumbsUpCount"].as_u64().unwrap_or(0),
                    updated: s(&m["dateModified"]),
                    categories: m["categories"].as_array().map(|c| c.iter().filter_map(|x| x["name"].as_str().map(String::from)).collect()).unwrap_or_default(),
                    client_side: String::new(),
                    server_side: String::new(),
                    project_type: q.project_type.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(SearchResult { hits, total: v["pagination"]["totalCount"].as_u64().unwrap_or(0) })
}

pub async fn install_curseforge(ctx: &InstallCtx<'_>, mod_id: &str, kind: &str) -> anyhow::Result<InstallResult> {
    let mut result = InstallResult::default();
    let mut seen = HashSet::new();
    cf_install_rec(ctx, mod_id, kind, false, &mut seen, &mut result, 0).await?;
    Ok(result)
}

fn cf_install_rec<'a>(
    ctx: &'a InstallCtx<'a>,
    mod_id: &'a str,
    kind: &'a str,
    dep: bool,
    seen: &'a mut HashSet<String>,
    result: &'a mut InstallResult,
    depth: u32,
) -> futures::future::BoxFuture<'a, anyhow::Result<()>> {
    Box::pin(async move {
        if depth > 6 || !seen.insert(mod_id.to_string()) {
            return Ok(());
        }
        let key = cf_key()?;
        let http = ctx.dl.http;
        let paths = ctx.dl.paths;
        let inst = ctx.inst;
        let m: Value = http.get(format!("{CURSEFORGE}/mods/{mod_id}")).header("x-api-key", &key).send().await?.error_for_status()?.json().await?;
        let m = &m["data"];
        let mut params = vec![("gameVersion", inst.mc_version.clone())];
        if kind == "mod" {
            if let Some(l) = cf_loader(&inst.loader) {
                params.push(("modLoaderType", l.to_string()));
            }
        }
        let url = reqwest::Url::parse_with_params(&format!("{CURSEFORGE}/mods/{mod_id}/files"), &params)?;
        let files: Value = http.get(url).header("x-api-key", &key).send().await?.error_for_status()?.json().await?;
        let file = files["data"].as_array().and_then(|a| a.iter().find(|f| f["releaseType"] == 1).or(a.first())).cloned();
        let Some(file) = file else {
            let msg = format!("{} has no file for {} {}", s(&m["name"]), loader_label(&inst.loader), inst.mc_version);
            if dep {
                result.warnings.push(msg);
                return Ok(());
            }
            bail!(msg);
        };
        let allowed = m["allowModDistribution"].as_bool().unwrap_or(true);
        let download_url = file["downloadUrl"].as_str().map(String::from);
        if !allowed || download_url.is_none() {
            // Respect the author's choice: send the user to CurseForge instead.
            let class_path = match kind {
                "resourcepack" => "texture-packs",
                "shader" => "shaders",
                "datapack" => "data-packs",
                _ => "mc-mods",
            };
            let page = format!("https://www.curseforge.com/minecraft/{class_path}/{}/download/{}", s(&m["slug"]), file["id"]);
            if dep {
                result.warnings.push(format!("{} must be downloaded from CurseForge: {page}", s(&m["name"])));
            } else {
                result.manual_url = Some(page);
            }
            return Ok(());
        }
        let filename = s(&file["fileName"]);
        if filename.is_empty() || filename.contains('/') || filename.contains('\\') || filename.contains("..") {
            bail!("unsafe file name from CurseForge");
        }
        let sha1 = file["hashes"].as_array().and_then(|h| h.iter().find(|x| x["algo"] == 1)).and_then(|x| x["value"].as_str()).map(String::from);
        let k = kind_dir(kind)?;
        let dest = instances::game_dir(paths, &inst.id).join(k).join(&filename);
        ctx.dl.run("content", &format!("Installing {}", s(&m["name"])), vec![DlItem::new(download_url.unwrap(), dest).sha1(sha1).size(file["fileLength"].as_u64()).cached()]).await?;
        let mut idx = content_index(paths, &inst.id);
        idx.insert(
            format!("{k}/{filename}"),
            ContentMeta {
                source: "curseforge".into(),
                project_id: Some(mod_id.to_string()),
                version_id: Some(file["id"].to_string()),
                slug: m["slug"].as_str().map(String::from),
                title: m["name"].as_str().map(String::from),
                version_number: file["displayName"].as_str().map(String::from),
                icon_url: m["logo"]["thumbnailUrl"].as_str().map(String::from),
                dependency: dep,
                ..Default::default()
            },
        );
        save_content_index(paths, &inst.id, &idx)?;
        result.installed.push(s(&m["name"]));
        if let Some(deps) = file["dependencies"].as_array() {
            for d in deps.clone() {
                if d["relationType"] == 3 {
                    let id = d["modId"].to_string();
                    if let Err(e) = cf_install_rec(ctx, &id, "mod", true, seen, result, depth + 1).await {
                        result.warnings.push(format!("Dependency not installed: {e:#}"));
                    }
                }
            }
        }
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// .mrpack import / export
// ---------------------------------------------------------------------------

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MrIndex {
    format_version: u32,
    game: String,
    version_id: String,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    files: Vec<MrFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MrFile {
    path: String,
    hashes: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    env: Option<HashMap<String, String>>,
    downloads: Vec<String>,
    file_size: u64,
}

/// Download hosts allowed in .mrpack files (per the Modrinth spec).
fn mrpack_host_ok(url: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else { return false };
    u.scheme() == "https" && matches!(u.host_str(), Some("cdn.modrinth.com" | "github.com" | "raw.githubusercontent.com" | "gitlab.com"))
}

/// Reads a .mrpack, creates the instance and downloads its files.
pub async fn import_mrpack(dl: &Downloader<'_>, pack: &Path, mut base: Instance) -> anyhow::Result<Instance> {
    let paths = dl.paths;
    let pack = pack.to_path_buf();
    let index: MrIndex = {
        let p = pack.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<MrIndex> {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&p)?)?;
            let mut s = String::new();
            zip.by_name("modrinth.index.json").context("not a Modrinth pack (no modrinth.index.json)")?.read_to_string(&mut s)?;
            Ok(serde_json::from_str(&s)?)
        })
        .await??
    };
    if index.game != "minecraft" {
        bail!("this pack isn't for Minecraft");
    }
    let mc = index.dependencies.get("minecraft").context("pack doesn't say which Minecraft version")?.clone();
    let (loader, lv) = [("fabric-loader", "fabric"), ("quilt-loader", "quilt"), ("neoforge", "neoforge"), ("forge", "forge")]
        .iter()
        .find_map(|(k, l)| index.dependencies.get(*k).map(|v| (l.to_string(), Some(v.clone()))))
        .unwrap_or(("vanilla".into(), None));

    if base.name.trim().is_empty() {
        base.name = index.name.clone();
    }
    base.mc_version = mc;
    base.loader = loader;
    base.loader_version = lv;
    base.icon = "package".into();
    let inst = instances::create(paths, base)?;
    let gd = instances::game_dir(paths, &inst.id);

    let mut items = vec![];
    for f in &index.files {
        if f.env.as_ref().and_then(|e| e.get("client")).map(|c| c == "unsupported").unwrap_or(false) {
            continue;
        }
        let urls: Vec<String> = f.downloads.iter().filter(|u| mrpack_host_ok(u)).cloned().collect();
        if urls.is_empty() {
            continue;
        }
        let dest = safe_join(&gd, &f.path)?;
        items.push(DlItem { urls, path: dest, sha1: f.hashes.get("sha1").cloned(), sha512: f.hashes.get("sha512").cloned(), size: Some(f.file_size), cache: true });
    }
    dl.run("modpack", &format!("Downloading {}", index.name), items).await?;

    // overrides/ then client-overrides/ (client wins)
    let gd2 = gd.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&pack)?)?;
        for prefix in ["overrides/", "client-overrides/"] {
            for i in 0..zip.len() {
                let mut e = zip.by_index(i)?;
                let name = e.name().to_string();
                let Some(rel) = name.strip_prefix(prefix) else { continue };
                if rel.is_empty() || e.is_dir() {
                    continue;
                }
                let out = safe_join(&gd2, rel)?;
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut f = std::fs::File::create(out)?;
                std::io::copy(&mut e, &mut f)?;
            }
        }
        Ok(())
    })
    .await??;

    // Track pack files as content so the Installed tab and updater know them.
    let lookup: HashMap<String, MrFile> = index.files.iter().filter_map(|f| f.hashes.get("sha1").map(|h| (h.clone(), f.clone()))).collect();
    if let Ok(found) = modrinth_by_hashes(dl.http, lookup.keys().cloned().collect()).await {
        let mut idx = content_index(paths, &inst.id);
        for (hash, v) in found {
            if let Some(f) = lookup.get(&hash) {
                idx.insert(
                    f.path.replace('\\', "/"),
                    ContentMeta {
                        source: "modrinth".into(),
                        project_id: v["project_id"].as_str().map(String::from),
                        version_id: v["id"].as_str().map(String::from),
                        title: v["name"].as_str().map(String::from),
                        version_number: v["version_number"].as_str().map(String::from),
                        ..Default::default()
                    },
                );
            }
        }
        save_content_index(paths, &inst.id, &idx)?;
    }
    Ok(inst)
}

async fn modrinth_by_hashes(http: &reqwest::Client, hashes: Vec<String>) -> anyhow::Result<HashMap<String, Value>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(http.post(format!("{MODRINTH}/version_files")).json(&json!({ "hashes": hashes, "algorithm": "sha1" })).send().await?.error_for_status()?.json().await?)
}

/// Exports an instance as .mrpack. Files Modrinth knows are referenced by URL;
/// everything else (and config/) goes into overrides/.
pub async fn export_mrpack(http: &reqwest::Client, paths: &Paths, inst: &Instance, dest: &Path) -> anyhow::Result<()> {
    let gd = instances::game_dir(paths, &inst.id);
    let mut candidates = vec![];
    for dir in ["mods", "resourcepacks", "shaderpacks"] {
        if let Ok(rd) = std::fs::read_dir(gd.join(dir)) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if e.path().is_file() && (n.ends_with(".jar") || n.ends_with(".zip")) {
                    candidates.push((format!("{dir}/{n}"), e.path()));
                }
            }
        }
    }
    let mut hashed = vec![];
    for (rel, p) in &candidates {
        hashed.push((rel.clone(), p.clone(), sha1_file(p).await?, sha512_file(p).await?, std::fs::metadata(p)?.len()));
    }
    let known = modrinth_by_hashes(http, hashed.iter().map(|h| h.2.clone()).collect()).await.unwrap_or_default();

    let mut files = vec![];
    let mut overrides: Vec<(String, PathBuf)> = vec![];
    for (rel, p, sha1, sha512, size) in hashed {
        let url = known.get(&sha1).and_then(|v| v["files"].as_array()?.iter().find(|f| f["hashes"]["sha1"] == sha1.as_str())?["url"].as_str().map(String::from));
        match url {
            Some(url) => files.push(MrFile { path: rel, hashes: HashMap::from([("sha1".into(), sha1), ("sha512".into(), sha512)]), env: None, downloads: vec![url], file_size: size }),
            None => overrides.push((rel, p)),
        }
    }
    for extra in ["config", "options.txt", "servers.dat"] {
        let p = gd.join(extra);
        if p.is_dir() {
            for e in walkdir::WalkDir::new(&p).into_iter().flatten().filter(|e| e.file_type().is_file()) {
                let rel = e.path().strip_prefix(&gd)?.to_string_lossy().replace('\\', "/");
                overrides.push((rel, e.path().to_path_buf()));
            }
        } else if p.is_file() {
            overrides.push((extra.to_string(), p));
        }
    }

    let mut deps = HashMap::from([("minecraft".to_string(), inst.mc_version.clone())]);
    if let Some(lv) = &inst.loader_version {
        let key = match inst.loader.as_str() {
            "fabric" => Some("fabric-loader"),
            "quilt" => Some("quilt-loader"),
            "forge" => Some("forge"),
            "neoforge" => Some("neoforge"),
            _ => None,
        };
        if let Some(k) = key {
            deps.insert(k.into(), lv.clone());
        }
    }
    let index = MrIndex { format_version: 1, game: "minecraft".into(), version_id: "1.0.0".into(), name: inst.name.clone(), summary: None, files, dependencies: deps };
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&dest)?);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("modrinth.index.json", opts)?;
        zip.write_all(&serde_json::to_vec_pretty(&index)?)?;
        for (rel, p) in overrides {
            zip.start_file(format!("overrides/{rel}"), opts)?;
            zip.write_all(&std::fs::read(p)?)?;
        }
        zip.finish()?;
        Ok(())
    })
    .await??;
    Ok(())
}

// ---------------------------------------------------------------------------
// Importing from other launchers
// ---------------------------------------------------------------------------

/// Prism Launcher / MultiMC instance folder.
pub fn import_prism(paths: &Paths, folder: &Path) -> anyhow::Result<Instance> {
    let cfg = std::fs::read_to_string(folder.join("instance.cfg")).context("not a Prism/MultiMC instance (no instance.cfg)")?;
    let name = cfg.lines().find_map(|l| l.strip_prefix("name=")).unwrap_or("Imported").trim().to_string();
    let pack: Value = crate::core::read_json(&folder.join("mmc-pack.json")).context("no mmc-pack.json")?;
    let mut inst = Instance { name, icon: "package".into(), ..Default::default() };
    for c in pack["components"].as_array().cloned().unwrap_or_default() {
        let v = c["version"].as_str().map(String::from);
        match c["uid"].as_str().unwrap_or("") {
            "net.minecraft" => inst.mc_version = v.unwrap_or_default(),
            "net.fabricmc.fabric-loader" => (inst.loader, inst.loader_version) = ("fabric".into(), v),
            "org.quiltmc.quilt-loader" => (inst.loader, inst.loader_version) = ("quilt".into(), v),
            "net.minecraftforge" => (inst.loader, inst.loader_version) = ("forge".into(), v),
            "net.neoforged" => (inst.loader, inst.loader_version) = ("neoforge".into(), v),
            _ => {}
        }
    }
    if inst.mc_version.is_empty() {
        bail!("couldn't find the Minecraft version in mmc-pack.json");
    }
    let inst = instances::create(paths, inst)?;
    let src = [".minecraft", "minecraft"].iter().map(|d| folder.join(d)).find(|p| p.is_dir()).context("instance has no .minecraft folder")?;
    crate::core::copy_dir(&src, &instances::game_dir(paths, &inst.id))?;
    Ok(inst)
}

/// Copies worlds, packs and settings from an existing .minecraft folder.
pub fn import_vanilla(paths: &Paths, folder: &Path, mc_version: &str, name: &str) -> anyhow::Result<Instance> {
    if !folder.join("options.txt").exists() && !folder.join("saves").exists() {
        bail!("that doesn't look like a .minecraft folder");
    }
    let inst = instances::create(paths, Instance { name: name.into(), mc_version: mc_version.into(), icon: "box".into(), ..Default::default() })?;
    let gd = instances::game_dir(paths, &inst.id);
    for item in ["saves", "resourcepacks", "shaderpacks", "screenshots", "options.txt", "servers.dat"] {
        let p = folder.join(item);
        if p.is_dir() {
            crate::core::copy_dir(&p, &gd.join(item))?;
        } else if p.is_file() {
            std::fs::copy(&p, gd.join(item))?;
        }
    }
    Ok(inst)
}

pub fn default_minecraft_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        dirs::data_dir().map(|d| d.join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        dirs::data_dir().map(|d| d.join("minecraft"))
    } else {
        dirs::home_dir().map(|d| d.join(".minecraft"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mrpack_hosts() {
        assert!(mrpack_host_ok("https://cdn.modrinth.com/data/x/versions/y/a.jar"));
        assert!(!mrpack_host_ok("http://cdn.modrinth.com/a.jar"));
        assert!(!mrpack_host_ok("https://evil.example/a.jar"));
    }

    #[test]
    fn mrpack_roundtrip_index() {
        let raw = r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"P","files":[{"path":"mods/a.jar","hashes":{"sha1":"x","sha512":"y"},"env":{"client":"required","server":"optional"},"downloads":["https://cdn.modrinth.com/a.jar"],"fileSize":3}],"dependencies":{"minecraft":"1.21.1","fabric-loader":"0.16.10"}}"#;
        let i: MrIndex = serde_json::from_str(raw).unwrap();
        assert_eq!(i.files[0].file_size, 3);
        assert_eq!(i.dependencies["fabric-loader"], "0.16.10");
    }
}
