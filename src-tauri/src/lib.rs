//! Halo Client backend: app state and the IPC commands the UI calls.

mod auth;
mod content;
mod core;
mod download;
mod extras;
mod instances;
mod java;
mod launch;
mod loaders;
mod servers;
mod versions;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tokio::sync::{Mutex, RwLock};

use crate::auth::{Account, AccountStore, PendingFlows};
use crate::core::{read_json, write_json, CmdResult, Paths, Settings};
use crate::download::Downloader;
use crate::instances::Instance;

pub struct AppState {
    pub paths: Paths,
    pub http: reqwest::Client,
    pub settings: RwLock<Settings>,
    pub accounts: Mutex<AccountStore>,
    pub flows: PendingFlows,
    pub running: Mutex<HashMap<String, launch::RunningGame>>,
    pub launching: Mutex<HashSet<String>>,
    pub logs: std::sync::Mutex<HashMap<String, VecDeque<String>>>,
    pub discord: extras::Discord,
    pub update: Mutex<Option<extras::UpdateReady>>,
}

type S<'a> = State<'a, Arc<AppState>>;

impl AppState {
    async fn downloader<'a>(&'a self, app: &'a AppHandle, verify: bool) -> Downloader<'a> {
        let threads = self.settings.read().await.download_threads.clamp(1, 64) as usize;
        Downloader { http: &self.http, paths: &self.paths, app, threads, verify_existing: verify }
    }
}

// ---------------------------------------------------------------------------
// App / settings
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    data_dir: String,
    platform: String,
    msa_configured: bool,
    curseforge: bool,
    discord: bool,
    sentry: bool,
    hud: Option<content::HudInfo>,
}

#[tauri::command]
async fn app_info(app: AppHandle, state: S<'_>) -> CmdResult<AppInfo> {
    let s = state.settings.read().await;
    Ok(AppInfo {
        version: core::APP_VERSION.into(),
        data_dir: state.paths.root.to_string_lossy().into(),
        platform: core::os_name().into(),
        msa_configured: auth::client_id(s.msa_client_id.as_deref()).is_ok(),
        curseforge: content::cf_available(),
        discord: extras::Discord::available(),
        sentry: option_env!("HALO_SENTRY_DSN").map(|d| !d.is_empty()).unwrap_or(false),
        hud: content::hud_info(&app).map(|(i, _)| i),
    })
}

#[tauri::command]
async fn get_settings(state: S<'_>) -> CmdResult<Settings> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
async fn save_settings(state: S<'_>, settings: Settings) -> CmdResult<()> {
    let mut s = state.settings.write().await;
    // These are owned by the backend; keep the backend's values.
    let keep_active = s.active_account_id.clone();
    let keep_last = s.last_instance_id.clone();
    *s = Settings { active_account_id: keep_active, last_instance_id: keep_last, ..settings };
    write_json(&state.paths.settings_file(), &*s)?;
    Ok(())
}

#[tauri::command]
async fn set_data_dir(path: Option<String>) -> CmdResult<()> {
    core::set_custom_root(path.as_deref().map(std::path::Path::new))?;
    Ok(())
}

#[tauri::command]
async fn open_path(app: AppHandle, path: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(path, None::<&str>)?;
    Ok(())
}

#[tauri::command]
async fn open_url(app: AppHandle, url: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    if !url.starts_with("https://") {
        return Err(core::CmdError("only https links can be opened".into()));
    }
    app.opener().open_url(url, None::<&str>)?;
    Ok(())
}

#[tauri::command]
async fn check_update(state: S<'_>) -> CmdResult<extras::UpdateCheck> {
    Ok(extras::check_update(&state.http).await?)
}

/// Downloads and verifies a newer version if there is one.
#[tauri::command]
async fn prepare_update(app: AppHandle, state: S<'_>) -> CmdResult<Option<extras::UpdateReady>> {
    let r = extras::download_update(&state.http, &state.paths).await?;
    if let Some(u) = &r {
        *state.update.lock().await = Some(u.clone());
        use tauri::Emitter;
        let _ = app.emit("update-ready", u);
    }
    Ok(r)
}

/// Installs the downloaded update and closes Halo; the installer reopens it.
#[tauri::command]
async fn apply_update(app: AppHandle, state: S<'_>) -> CmdResult<()> {
    if !state.running.lock().await.is_empty() {
        return Err(core::CmdError("Close Minecraft before updating.".into()));
    }
    let u = state.update.lock().await.clone().ok_or_else(|| core::CmdError("No update has been downloaded yet.".into()))?;
    extras::install_update(std::path::Path::new(&u.path))?;
    app.exit(0);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Preflight {
    account_ok: bool,
    account_message: Option<String>,
    instance_id: Option<String>,
    instance_ready: bool,
}

/// Runs on launcher open: refreshes the active token and checks the last
/// instance's files in the background so Play can go straight to spawning.
#[tauri::command]
async fn preflight(app: AppHandle, state: S<'_>) -> CmdResult<Preflight> {
    let settings = state.settings.read().await.clone();
    let mut out = Preflight { account_ok: false, account_message: None, instance_id: settings.last_instance_id.clone(), instance_ready: false };
    let account = {
        let store = state.accounts.lock().await;
        settings.active_account_id.as_ref().and_then(|id| store.accounts.iter().find(|a| &a.id == id).cloned()).or_else(|| store.accounts.first().cloned())
    };
    match account {
        Some(mut a) => match auth::ensure_fresh(&state.http, &state.paths, settings.msa_client_id.as_deref(), &mut a).await {
            Ok(_) => {
                out.account_ok = true;
                let mut store = state.accounts.lock().await;
                store.upsert(a);
                let _ = store.save(&state.paths);
            }
            Err(e) => out.account_message = Some(format!("{e:#}")),
        },
        None => out.account_message = Some("Add an account to play".into()),
    }
    if let Some(id) = &settings.last_instance_id {
        if let Ok(inst) = instances::load(&state.paths, id) {
            if let Some(v) = inst.version_id.as_deref() {
                if versions::version_json_path(&state.paths, v).exists() {
                    let dl = state.downloader(&app, false).await;
                    let f = versions::Features::default();
                    out.instance_ready = versions::prepare(&dl, v, &instances::game_dir(&state.paths, id), &f).await.is_ok();
                }
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Versions & Java
// ---------------------------------------------------------------------------

#[tauri::command]
async fn game_versions(state: S<'_>) -> CmdResult<Vec<versions::GameVersion>> {
    Ok(versions::game_versions(&state.http, &state.paths).await?)
}

#[tauri::command]
async fn loader_versions(state: S<'_>, loader: String, mc: String) -> CmdResult<Vec<loaders::LoaderVersion>> {
    Ok(loaders::versions(&state.http, &state.paths, &loader, &mc).await?)
}

#[tauri::command]
async fn java_runtimes(state: S<'_>) -> CmdResult<Vec<java::JavaRuntime>> {
    Ok(java::installed(&state.paths))
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountList {
    accounts: Vec<Account>,
    active_id: Option<String>,
}

#[tauri::command]
async fn list_accounts(state: S<'_>) -> CmdResult<AccountList> {
    let accounts = state.accounts.lock().await.accounts.clone();
    let active = state.settings.read().await.active_account_id.clone().filter(|id| accounts.iter().any(|a| &a.id == id)).or_else(|| accounts.first().map(|a| a.id.clone()));
    Ok(AccountList { accounts, active_id: active })
}

async fn set_active(state: &AppState, id: Option<String>) -> anyhow::Result<()> {
    let mut s = state.settings.write().await;
    s.active_account_id = id;
    write_json(&state.paths.settings_file(), &*s)
}

#[tauri::command]
async fn set_active_account(state: S<'_>, id: String) -> CmdResult<()> {
    set_active(&state, Some(id)).await?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthStart {
    flow_id: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
}

#[tauri::command]
async fn auth_start(state: S<'_>) -> CmdResult<AuthStart> {
    let cid = auth::client_id(state.settings.read().await.msa_client_id.as_deref())?;
    let dc = auth::start_device_flow(&state.http, &cid).await?;
    let flow_id = uuid::Uuid::new_v4().to_string();
    let out = AuthStart { flow_id: flow_id.clone(), user_code: dc.user_code.clone(), verification_uri: dc.verification_uri.clone(), expires_in: dc.expires_in };
    state.flows.lock().await.insert(flow_id, dc);
    Ok(out)
}

#[tauri::command]
async fn auth_poll(state: S<'_>, flow_id: String) -> CmdResult<Account> {
    let cid = auth::client_id(state.settings.read().await.msa_client_id.as_deref())?;
    let (access, refresh) = auth::poll_device_flow(&state.http, &cid, &flow_id, &state.flows).await?;
    let account = auth::finish_login(&state.http, &state.paths, &access, &refresh).await?;
    {
        let mut store = state.accounts.lock().await;
        store.upsert(account.clone());
        store.save(&state.paths)?;
    }
    set_active(&state, Some(account.id.clone())).await?;
    Ok(account)
}

#[tauri::command]
async fn auth_cancel(state: S<'_>, flow_id: String) -> CmdResult<()> {
    state.flows.lock().await.remove(&flow_id);
    Ok(())
}

#[tauri::command]
async fn add_offline(state: S<'_>, name: String) -> CmdResult<Account> {
    let mut store = state.accounts.lock().await;
    // Offline profiles can't join online-mode servers, so they're allowed without a
    // Microsoft account (needed while Halo's Azure app awaits Mojang approval).
    let acc = auth::offline_account(name.trim())?;
    store.upsert(acc.clone());
    store.save(&state.paths)?;
    Ok(acc)
}

#[tauri::command]
async fn remove_account(state: S<'_>, id: String) -> CmdResult<()> {
    let mut store = state.accounts.lock().await;
    store.accounts.retain(|a| a.id != id);
    store.save(&state.paths)?;
    auth::remove(&state.paths, &id);
    let next = store.accounts.first().map(|a| a.id.clone());
    drop(store);
    if state.settings.read().await.active_account_id.as_deref() == Some(id.as_str()) {
        set_active(&state, next).await?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Textures {
    skin: Option<String>,
    cape: Option<String>,
    slim: bool,
}

#[tauri::command]
async fn account_textures(state: S<'_>, id: String) -> CmdResult<Textures> {
    let acc = state.accounts.lock().await.accounts.iter().find(|a| a.id == id).cloned().ok_or_else(|| core::CmdError("account not found".into()))?;
    let skin = match &acc.skin_url {
        Some(u) => auth::texture_data_uri(&state.http, &state.paths, u).await.ok(),
        None => None,
    };
    let cape = match acc.capes.iter().find(|c| c.active) {
        Some(c) => auth::texture_data_uri(&state.http, &state.paths, &c.url).await.ok(),
        None => None,
    };
    Ok(Textures { skin, cape, slim: acc.skin_variant.as_deref() == Some("slim") })
}

async fn with_account<F, Fut>(state: &AppState, id: &str, f: F) -> CmdResult<Account>
where
    F: FnOnce(Account) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Account>>,
{
    let acc = state.accounts.lock().await.accounts.iter().find(|a| a.id == id).cloned().ok_or_else(|| core::CmdError("account not found".into()))?;
    let acc = f(acc).await?;
    let mut store = state.accounts.lock().await;
    store.upsert(acc.clone());
    store.save(&state.paths)?;
    Ok(acc)
}

#[tauri::command]
async fn upload_skin(state: S<'_>, id: String, path: String, slim: bool) -> CmdResult<Account> {
    let png = std::fs::read(&path)?;
    let cid = state.settings.read().await.msa_client_id.clone();
    let st = state.inner().clone();
    with_account(&state, &id, |mut a| async move {
        auth::upload_skin(&st.http, &st.paths, cid.as_deref(), &mut a, png, slim).await?;
        Ok(a)
    })
    .await
}

#[tauri::command]
async fn set_cape(state: S<'_>, id: String, cape_id: Option<String>) -> CmdResult<Account> {
    let cid = state.settings.read().await.msa_client_id.clone();
    let st = state.inner().clone();
    with_account(&state, &id, |mut a| async move {
        auth::set_cape(&st.http, &st.paths, cid.as_deref(), &mut a, cape_id.as_deref()).await?;
        Ok(a)
    })
    .await
}

// ---------------------------------------------------------------------------
// Instances
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceView {
    #[serde(flatten)]
    inst: Instance,
    running: bool,
    vanilla_boot_ms: Option<u64>,
    hud_supported: bool,
}

async fn view(app: &AppHandle, state: &AppState, inst: Instance) -> InstanceView {
    let running = state.running.lock().await.contains_key(&inst.id);
    // Baseline for the "× faster than vanilla" chip: median boot of any
    // vanilla instance on the same Minecraft version.
    let vanilla_boot_ms = instances::list(&state.paths)
        .into_iter()
        .filter(|i| i.loader == "vanilla" && i.mc_version == inst.mc_version && !i.cds && !i.tuned_flags)
        .find_map(|i| i.median_boot(false));
    let hud_supported = content::hud_supported(app, &inst);
    InstanceView { inst, running, vanilla_boot_ms, hud_supported }
}

#[tauri::command]
async fn list_instances(app: AppHandle, state: S<'_>) -> CmdResult<Vec<InstanceView>> {
    let mut out = vec![];
    for i in instances::list(&state.paths) {
        out.push(view(&app, &state, i).await);
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSpec {
    name: String,
    mc_version: String,
    loader: String,
    loader_version: Option<String>,
    #[serde(default)]
    perf_mods: Vec<String>,
    #[serde(default)]
    halo_hud: bool,
    icon: Option<String>,
    hue: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateResult {
    instance: InstanceView,
    warnings: Vec<String>,
}

#[tauri::command]
async fn create_instance(app: AppHandle, state: S<'_>, spec: CreateSpec) -> CmdResult<CreateResult> {
    let ram = state.settings.read().await.default_ram_mb;
    let inst = instances::create(
        &state.paths,
        Instance {
            name: spec.name,
            mc_version: spec.mc_version,
            loader: spec.loader,
            loader_version: spec.loader_version.filter(|v| !v.is_empty()),
            icon: spec.icon.unwrap_or_else(|| "box".into()),
            hue: spec.hue.unwrap_or(275),
            ram_mb: Some(ram),
            perf_profile: !spec.perf_mods.is_empty(),
            ..Default::default()
        },
    )?;
    let mut warnings = vec![];
    if !spec.perf_mods.is_empty() && matches!(inst.loader.as_str(), "fabric" | "quilt") {
        let dl = state.downloader(&app, false).await;
        let ctx = content::InstallCtx { dl: &dl, inst: &inst, world: None };
        match content::install_perf_profile(&ctx, Some(spec.perf_mods)).await {
            Ok(r) => warnings.extend(r.warnings),
            Err(e) => warnings.push(format!("{e:#}")),
        }
    }
    if spec.halo_hud {
        if let Err(e) = content::install_halo_hud(&app, &state.paths, &inst) {
            warnings.push(format!("Halo HUD: {e:#}"));
        }
    }
    Ok(CreateResult { instance: view(&app, &state, inst).await, warnings })
}

#[tauri::command]
async fn update_instance(app: AppHandle, state: S<'_>, instance: Instance) -> CmdResult<InstanceView> {
    let mut cur = instances::load(&state.paths, &instance.id)?;
    // Only user-editable fields; history and ids stay backend-owned.
    let loader_changed = cur.loader != instance.loader || cur.loader_version != instance.loader_version || cur.mc_version != instance.mc_version;
    cur.name = instance.name;
    cur.icon = instance.icon;
    cur.hue = instance.hue;
    cur.group = instance.group;
    cur.ram_mb = instance.ram_mb;
    cur.java_path = instance.java_path;
    cur.jvm_args = instance.jvm_args;
    cur.width = instance.width;
    cur.height = instance.height;
    cur.pre_launch = instance.pre_launch;
    cur.post_exit = instance.post_exit;
    cur.account_id = instance.account_id;
    cur.cds = instance.cds;
    cur.tuned_flags = instance.tuned_flags;
    cur.mc_version = instance.mc_version;
    cur.loader = instance.loader;
    cur.loader_version = instance.loader_version;
    if loader_changed {
        cur.version_id = None;
    }
    instances::save(&state.paths, &cur)?;
    Ok(view(&app, &state, cur).await)
}

#[tauri::command]
async fn delete_instance(state: S<'_>, id: String) -> CmdResult<()> {
    if state.running.lock().await.contains_key(&id) {
        return Err(core::CmdError("Close the game before deleting this instance.".into()));
    }
    instances::delete(&state.paths, &id)?;
    Ok(())
}

#[tauri::command]
async fn duplicate_instance(app: AppHandle, state: S<'_>, id: String) -> CmdResult<InstanceView> {
    let inst = instances::duplicate(&state.paths, &id)?;
    Ok(view(&app, &state, inst).await)
}

#[tauri::command]
async fn set_instance_icon(state: S<'_>, id: String, path: String) -> CmdResult<()> {
    let bytes = std::fs::read(&path)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(core::CmdError("Pick an image under 2 MB.".into()));
    }
    let ext = PathBuf::from(&path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "png".into());
    let dest = state.paths.instance(&id).join(format!("icon.{ext}"));
    std::fs::write(&dest, bytes)?;
    let mut inst = instances::load(&state.paths, &id)?;
    inst.icon_path = Some(dest.to_string_lossy().into());
    instances::save(&state.paths, &inst)?;
    Ok(())
}

#[tauri::command]
async fn list_content(state: S<'_>, id: String) -> CmdResult<Vec<instances::ContentItem>> {
    Ok(instances::list_content(&state.paths, &id))
}

#[tauri::command]
async fn toggle_content(state: S<'_>, id: String, key: String, enabled: bool) -> CmdResult<()> {
    instances::set_enabled(&state.paths, &id, &key, enabled)?;
    Ok(())
}

#[tauri::command]
async fn remove_content(state: S<'_>, id: String, key: String) -> CmdResult<()> {
    instances::remove_content(&state.paths, &id, &key)?;
    Ok(())
}

#[tauri::command]
async fn check_content_updates(state: S<'_>, id: String) -> CmdResult<Vec<content::UpdateInfo>> {
    let inst = instances::load(&state.paths, &id)?;
    Ok(content::check_updates(&state.http, &state.paths, &inst).await?)
}

#[tauri::command]
async fn apply_content_updates(app: AppHandle, state: S<'_>, id: String, updates: Vec<(String, String)>) -> CmdResult<content::InstallResult> {
    // updates: (projectId, versionId)
    let inst = instances::load(&state.paths, &id)?;
    let dl = state.downloader(&app, false).await;
    let ctx = content::InstallCtx { dl: &dl, inst: &inst, world: None };
    let mut out = content::InstallResult::default();
    for (pid, vid) in updates {
        match content::install_modrinth(&ctx, &pid, Some(&vid), None).await {
            Ok(r) => {
                out.installed.extend(r.installed);
                out.warnings.extend(r.warnings);
            }
            Err(e) => out.warnings.push(format!("{e:#}")),
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallReq {
    instance_id: String,
    source: String,
    project_id: String,
    version_id: Option<String>,
    kind: String,
    world: Option<String>,
}

#[tauri::command]
async fn install_project(app: AppHandle, state: S<'_>, req: InstallReq) -> CmdResult<content::InstallResult> {
    let inst = instances::load(&state.paths, &req.instance_id)?;
    let dl = state.downloader(&app, false).await;
    let ctx = content::InstallCtx { dl: &dl, inst: &inst, world: req.world };
    if req.kind == "modpack" {
        return Err(core::CmdError("Use 'Install as new instance' for modpacks.".into()));
    }
    let r = if req.source == "curseforge" {
        content::install_curseforge(&ctx, &req.project_id, &req.kind).await?
    } else {
        content::install_modrinth(&ctx, &req.project_id, req.version_id.as_deref(), Some(&req.kind)).await?
    };
    Ok(r)
}

/// Installs a Modrinth modpack as a new instance (downloads the .mrpack, then imports it).
#[tauri::command]
async fn install_modpack(app: AppHandle, state: S<'_>, project_id: String, version_id: Option<String>) -> CmdResult<InstanceView> {
    let version: serde_json::Value = match version_id {
        Some(v) => core::get_json(&state.http, &format!("https://api.modrinth.com/v2/version/{v}")).await?,
        None => {
            let list = content::project_versions(&state.http, &project_id, &[], &[]).await?;
            list.into_iter().next().ok_or_else(|| core::CmdError("this modpack has no versions".into()))?
        }
    };
    let file = version["files"].as_array().and_then(|f| f.iter().find(|x| x["primary"] == true).or(f.first())).cloned().ok_or_else(|| core::CmdError("no pack file".into()))?;
    let dest = state.paths.cache().join("packs").join(format!("{}.mrpack", version["id"].as_str().unwrap_or("pack")));
    let dl = state.downloader(&app, false).await;
    dl.run(
        "modpack",
        "Downloading modpack",
        vec![download::DlItem::new(file["url"].as_str().unwrap_or_default(), dest.clone()).sha512(file["hashes"]["sha512"].as_str().map(String::from)).size(file["size"].as_u64())],
    )
    .await?;
    let inst = content::import_mrpack(&dl, &dest, Instance { icon: "package".into(), ..Default::default() }).await?;
    Ok(view(&app, &state, inst).await)
}

#[tauri::command]
async fn install_perf(app: AppHandle, state: S<'_>, id: String, mods: Option<Vec<String>>) -> CmdResult<content::InstallResult> {
    let mut inst = instances::load(&state.paths, &id)?;
    let dl = state.downloader(&app, false).await;
    let ctx = content::InstallCtx { dl: &dl, inst: &inst, world: None };
    let r = content::install_perf_profile(&ctx, mods).await?;
    inst.perf_profile = true;
    instances::save(&state.paths, &inst)?;
    Ok(r)
}

#[tauri::command]
async fn install_hud(app: AppHandle, state: S<'_>, id: String) -> CmdResult<()> {
    let inst = instances::load(&state.paths, &id)?;
    content::install_halo_hud(&app, &state.paths, &inst)?;
    Ok(())
}

#[tauri::command]
async fn screenshots(state: S<'_>, id: String) -> CmdResult<Vec<instances::Screenshot>> {
    Ok(instances::screenshots(&state.paths, &id))
}

#[tauri::command]
async fn worlds(state: S<'_>, id: String) -> CmdResult<Vec<String>> {
    Ok(instances::worlds(&state.paths, &id))
}

#[tauri::command]
async fn instance_dir(state: S<'_>, id: String) -> CmdResult<String> {
    Ok(instances::game_dir(&state.paths, &id).to_string_lossy().into())
}

#[tauri::command]
async fn export_mrpack(state: S<'_>, id: String, path: String) -> CmdResult<()> {
    let inst = instances::load(&state.paths, &id)?;
    content::export_mrpack(&state.http, &state.paths, &inst, std::path::Path::new(&path)).await?;
    Ok(())
}

#[tauri::command]
async fn import_mrpack(app: AppHandle, state: S<'_>, path: String) -> CmdResult<InstanceView> {
    let dl = state.downloader(&app, false).await;
    let inst = content::import_mrpack(&dl, std::path::Path::new(&path), Instance::default()).await?;
    Ok(view(&app, &state, inst).await)
}

#[tauri::command]
async fn import_prism(app: AppHandle, state: S<'_>, path: String) -> CmdResult<InstanceView> {
    let inst = content::import_prism(&state.paths, std::path::Path::new(&path))?;
    Ok(view(&app, &state, inst).await)
}

#[tauri::command]
async fn import_vanilla(app: AppHandle, state: S<'_>, path: String, mc_version: String, name: String) -> CmdResult<InstanceView> {
    let inst = content::import_vanilla(&state.paths, std::path::Path::new(&path), &mc_version, &name)?;
    Ok(view(&app, &state, inst).await)
}

#[tauri::command]
async fn default_minecraft_dir() -> CmdResult<Option<String>> {
    Ok(content::default_minecraft_dir().filter(|p| p.exists()).map(|p| p.to_string_lossy().into()))
}

/// Re-checks every game file by hash and re-downloads anything damaged.
#[tauri::command]
async fn repair_instance(app: AppHandle, state: S<'_>, id: String) -> CmdResult<()> {
    let inst = instances::load(&state.paths, &id)?;
    let v = inst.version_id.clone().ok_or_else(|| core::CmdError("Play this instance once first.".into()))?;
    let dl = state.downloader(&app, true).await;
    versions::prepare(&dl, &v, &instances::game_dir(&state.paths, &id), &versions::Features::default()).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Launching
// ---------------------------------------------------------------------------

#[tauri::command]
async fn launch_instance(app: AppHandle, state: S<'_>, id: String, server: Option<String>) -> CmdResult<()> {
    launch::launch(app, state.inner().clone(), id, server).await?;
    Ok(())
}

#[tauri::command]
async fn stop_instance(state: S<'_>, id: String) -> CmdResult<()> {
    if let Some(r) = state.running.lock().await.get_mut(&id) {
        if let Some(k) = r.kill.take() {
            let _ = k.send(());
        }
    }
    Ok(())
}

#[tauri::command]
async fn running_instances(state: S<'_>) -> CmdResult<Vec<String>> {
    Ok(state.running.lock().await.keys().cloned().collect())
}

#[tauri::command]
async fn game_log(state: S<'_>, id: String) -> CmdResult<Vec<String>> {
    if let Some(buf) = state.logs.lock().unwrap().get(&id).filter(|b| !b.is_empty()) {
        return Ok(buf.iter().cloned().collect());
    }
    // Nothing streamed (yet): show the game's own latest.log.
    let p = instances::game_dir(&state.paths, &id).join("logs").join("latest.log");
    Ok(std::fs::read_to_string(p).map(|s| s.lines().rev().take(5000).collect::<Vec<_>>().into_iter().rev().map(String::from).collect()).unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Servers
// ---------------------------------------------------------------------------

#[tauri::command]
async fn recent_servers(state: S<'_>) -> CmdResult<Vec<servers::RecentServer>> {
    Ok(servers::recent(&state.paths))
}

#[tauri::command]
async fn forget_recent_server(state: S<'_>, address: String) -> CmdResult<()> {
    servers::forget_recent(&state.paths, &address)?;
    Ok(())
}

#[tauri::command]
async fn ping_server(address: String) -> CmdResult<servers::ServerStatus> {
    Ok(servers::ping(&address).await.unwrap_or_default())
}

#[tauri::command]
async fn server_list(state: S<'_>, id: String) -> CmdResult<Vec<servers::ServerEntry>> {
    Ok(servers::read_servers_dat(&state.paths, &id))
}

#[tauri::command]
async fn save_server_list(state: S<'_>, id: String, servers_list: Vec<servers::ServerEntry>) -> CmdResult<()> {
    servers::write_servers_dat(&state.paths, &id, servers_list)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Content browser
// ---------------------------------------------------------------------------

#[tauri::command]
async fn search_content(state: S<'_>, query: content::SearchQuery) -> CmdResult<content::SearchResult> {
    Ok(content::search(&state.http, &query).await?)
}

#[tauri::command]
async fn get_project(state: S<'_>, source: String, id: String) -> CmdResult<serde_json::Value> {
    Ok(content::project(&state.http, &source, &id).await?)
}

#[tauri::command]
async fn get_project_versions(state: S<'_>, id: String, loaders: Vec<String>, game_versions: Vec<String>) -> CmdResult<Vec<serde_json::Value>> {
    Ok(content::project_versions(&state.http, &id, &loaders, &game_versions).await?)
}

// ---------------------------------------------------------------------------
// Logging + entry point
// ---------------------------------------------------------------------------

struct FileLogger(std::sync::Mutex<std::fs::File>);

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            use std::io::Write;
            let _ = writeln!(self.0.lock().unwrap(), "{} [{}] {}", chrono::Local::now().format("%H:%M:%S"), r.level(), r.args());
        }
    }
    fn flush(&self) {}
}

fn init_logging(paths: &Paths) {
    if let Ok(f) = std::fs::File::create(paths.logs().join("launcher.log")) {
        let _ = log::set_boxed_logger(Box::new(FileLogger(std::sync::Mutex::new(f)))).map(|_| log::set_max_level(log::LevelFilter::Info));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = Paths::resolve();
    init_logging(&paths);
    let settings: Settings = read_json(&paths.settings_file()).unwrap_or_default();
    let _sentry = extras::init_sentry(settings.crash_reports);
    let state = Arc::new(AppState {
        accounts: Mutex::new(AccountStore::load(&paths)),
        http: core::http_client(),
        settings: RwLock::new(settings),
        flows: Default::default(),
        running: Default::default(),
        launching: Default::default(),
        logs: Default::default(),
        discord: Default::default(),
        update: Default::default(),
        paths,
    });
    let root = state.paths.root.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |app| {
            // Screenshots and instance icons are shown straight from the data folder.
            app.asset_protocol_scope().allow_directory(&root, true)?;
            // Background auto-update check shortly after start (never delays opening).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                let state = handle.state::<Arc<AppState>>().inner().clone();
                if !state.settings.read().await.auto_update {
                    return;
                }
                match extras::download_update(&state.http, &state.paths).await {
                    Ok(Some(u)) => {
                        *state.update.lock().await = Some(u.clone());
                        use tauri::Emitter;
                        let _ = handle.emit("update-ready", u);
                    }
                    Ok(None) => {}
                    Err(e) => log::warn!("update check failed: {e:#}"),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_settings,
            save_settings,
            set_data_dir,
            open_path,
            open_url,
            check_update,
            prepare_update,
            apply_update,
            preflight,
            game_versions,
            loader_versions,
            java_runtimes,
            list_accounts,
            set_active_account,
            auth_start,
            auth_poll,
            auth_cancel,
            add_offline,
            remove_account,
            account_textures,
            upload_skin,
            set_cape,
            list_instances,
            create_instance,
            update_instance,
            delete_instance,
            duplicate_instance,
            set_instance_icon,
            list_content,
            toggle_content,
            remove_content,
            check_content_updates,
            apply_content_updates,
            install_project,
            install_modpack,
            install_perf,
            install_hud,
            screenshots,
            worlds,
            instance_dir,
            export_mrpack,
            import_mrpack,
            import_prism,
            import_vanilla,
            default_minecraft_dir,
            repair_instance,
            launch_instance,
            stop_instance,
            running_instances,
            game_log,
            recent_servers,
            forget_recent_server,
            ping_server,
            server_list,
            save_server_list,
            search_content,
            get_project,
            get_project_versions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Halo Client");
}
