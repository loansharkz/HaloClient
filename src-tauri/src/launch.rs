//! Launching: account refresh → loader/version install → files → Java →
//! arguments → spawn → log streaming, boot timer, recent servers, crash analysis.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::core::{now_ms, Paths};
use crate::download::Downloader;
use crate::instances::{self, BootSample, Instance};
use crate::versions::{self, ArgValue, Features, Prepared, StringOrVec};
use crate::AppState;

/// Log lines that mean the title screen is about to appear.
const BOOT_MARKERS: &[&str] = &["Sound engine started", "OpenAL initialized"];

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LaunchState {
    pub instance_id: String,
    pub phase: String, // preparing | running | stopped | error
    pub message: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameLog {
    pub instance_id: String,
    pub lines: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrashFix {
    pub kind: String, // install_dependency | disable_mod | increase_ram | auto_java | update_drivers | remove_duplicate | reset_jvm_args
    pub arg: Option<String>,
    pub label: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub title: String,
    pub detail: String,
    pub fix: Option<CrashFix>,
    pub crash_file: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameExit {
    pub instance_id: String,
    pub code: Option<i32>,
    pub played_secs: u64,
    pub boot_ms: Option<u64>,
    pub crash: Option<CrashReport>,
    pub killed: bool,
}

pub struct RunningGame {
    pub pid: Option<u32>,
    pub started: Instant,
    pub kill: Option<tokio::sync::oneshot::Sender<()>>,
    /// Set for Instant Play: the game loads hidden and waits for Play.
    pub prewarm: Option<Arc<std::sync::Mutex<Prewarm>>>,
}

/// State of a game pre-loaded by Instant Play.
pub struct Prewarm {
    pub pid: u32,
    pub created: Instant,
    pub ready: bool,
    pub revealed_at: Option<Instant>,
    pub hidden: Vec<isize>,
    pub ended: bool,
    pub last_trim: Option<Instant>,
    /// Play was pressed before the title screen: show the window as soon as it's ready.
    pub reveal_requested: bool,
}

impl RunningGame {
    /// True while this is an Instant Play game that hasn't been shown yet.
    pub fn waiting(&self) -> bool {
        self.prewarm.as_ref().map(|p| p.lock().unwrap().revealed_at.is_none()).unwrap_or(false)
    }
    pub fn phase(&self) -> &'static str {
        match &self.prewarm {
            Some(p) => {
                let p = p.lock().unwrap();
                if p.revealed_at.is_some() {
                    "running"
                } else if p.reveal_requested {
                    "opening"
                } else if p.ready {
                    "ready"
                } else {
                    "prewarming"
                }
            }
            None => "running",
        }
    }
}

/// What the caller should do after a game exits.
pub struct ExitOutcome {
    /// Start a new Instant Play pre-load (after a played game or an invalidation).
    pub reprewarm: bool,
    /// An unshown pre-load ended on its own: stop auto-retrying until something changes.
    pub prewarm_failed: bool,
}

fn emit_state<R: tauri::Runtime>(app: &AppHandle<R>, id: &str, phase: &str, message: impl Into<String>) {
    let _ = app.emit("launch-state", LaunchState { instance_id: id.into(), phase: phase.into(), message: message.into() });
}

/// Parses "host[:port]" into (host, port).
pub fn split_address(addr: &str) -> (String, u16) {
    let addr = addr.trim();
    // IPv6 in brackets: [::1]:25565
    if let Some(rest) = addr.strip_prefix('[') {
        if let Some((h, p)) = rest.split_once(']') {
            return (h.to_string(), p.trim_start_matches(':').parse().unwrap_or(25565));
        }
    }
    match addr.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => (h.to_string(), p.parse().unwrap_or(25565)),
        _ => (addr.to_string(), 25565),
    }
}

pub async fn launch(app: AppHandle, state: Arc<AppState>, instance_id: String, server: Option<String>, prewarm: bool) -> anyhow::Result<()> {
    {
        let running = state.running.lock().await;
        if running.contains_key(&instance_id) {
            bail!("This instance is already running.");
        }
    }
    {
        let mut launching = state.launching.lock().await;
        if !launching.insert(instance_id.clone()) {
            bail!("This instance is already launching.");
        }
    }
    let result = launch_inner(&app, &state, &instance_id, server, prewarm).await;
    state.launching.lock().await.remove(&instance_id);
    if let Err(e) = &result {
        // A failed background pre-load shouldn't look like a failed Play.
        emit_state(&app, &instance_id, if prewarm { "stopped" } else { "error" }, format!("{e:#}"));
    }
    result
}

async fn launch_inner(app: &AppHandle, state: &Arc<AppState>, instance_id: &str, server: Option<String>, prewarm: bool) -> anyhow::Result<()> {
    let prep = if prewarm { "prewarming" } else { "preparing" };
    let clicked = Instant::now();
    let paths = &state.paths;
    let http = &state.http;
    let settings = state.settings.read().await.clone();
    let mut inst = instances::load(paths, instance_id)?;
    let game_dir = instances::game_dir(paths, instance_id);
    std::fs::create_dir_all(&game_dir)?;

    // --- account
    emit_state(app, instance_id, prep, "Checking account");
    let account_id = inst.account_id.clone().or(settings.active_account_id.clone());
    let mut account = {
        let store = state.accounts.lock().await;
        account_id
            .and_then(|id| store.accounts.iter().find(|a| a.id == id).cloned())
            .or_else(|| store.accounts.first().cloned())
            .context("Add an account first (Accounts → Add Microsoft account).")?
    };
    let before = account.expires_at;
    let auth = crate::auth::ensure_fresh(http, paths, settings.msa_client_id.as_deref(), &mut account).await?;
    if account.expires_at != before {
        let mut store = state.accounts.lock().await;
        store.upsert(account.clone());
        store.save(paths)?;
    }

    let dl = Downloader { http, paths, app, threads: settings.download_threads.clamp(1, 64) as usize, verify_existing: false, silent: prewarm };

    // --- loader + version
    emit_state(app, instance_id, prep, "Preparing game version");
    let version_ready = inst.version_id.as_ref().map(|v| versions::version_json_path(paths, v).exists()).unwrap_or(false);
    if !version_ready {
        let vanilla = versions::ensure_vanilla_json(http, paths, &inst.mc_version).await?;
        let java_for_installer = crate::java::ensure(http, paths, app, vanilla.java_version.map(|j| j.major_version).unwrap_or(8)).await?;
        let id = crate::loaders::install(&dl, &java_for_installer, &inst.loader, &inst.mc_version, inst.loader_version.as_deref()).await?;
        inst.version_id = Some(id);
        instances::save(paths, &inst)?;
    }
    let version_id = inst.version_id.clone().unwrap();

    // Quick play is supported when the version JSON has the feature rule.
    let resolved = versions::resolve(http, paths, &version_id).await?;
    let supports_quick_play = resolved
        .arguments
        .as_ref()
        .map(|a| a.game.iter().any(|g| matches!(g, ArgValue::Ruled { value, .. } if value_contains(value, "quickPlayMultiplayer"))))
        .unwrap_or(false);
    let features = Features { custom_resolution: inst.width.is_some() && inst.height.is_some(), quick_play_multiplayer: server.is_some() && supports_quick_play };

    emit_state(app, instance_id, prep, "Checking game files");
    let prepared = versions::prepare(&dl, &version_id, &game_dir, &features).await?;

    // --- Halo HUD keeps itself in sync with the bundled build
    crate::content::sync_halo_hud(app, paths, &inst).ok();

    // --- java
    emit_state(app, instance_id, prep, "Checking Java");
    let (java, java_major) = match inst.java_path.as_deref().filter(|p| !p.is_empty()) {
        Some(custom) => {
            let p = PathBuf::from(custom);
            let major = crate::java::probe_major(&p).await.unwrap_or(prepared.java_major);
            (p, major)
        }
        None if settings.auto_java => (crate::java::ensure(http, paths, app, prepared.java_major).await?, crate::java::effective_major(prepared.java_major)),
        None => (PathBuf::from(if cfg!(windows) { "javaw" } else { "java" }), prepared.java_major),
    };

    // --- args
    let ram = inst.ram_mb.unwrap_or(settings.default_ram_mb).clamp(512, 65536);
    // AppCDS is opt-in on Windows (Settings → Class-data sharing) until benchmarked there.
    let cds_allowed = !cfg!(windows) || settings.class_data_sharing || std::env::var("HALO_FORCE_CDS").is_ok();
    let args = build_args(paths, &inst, &prepared, &auth, &game_dir, ram, java_major, server.as_deref(), &features, cds_allowed);

    // --- pre-launch hook
    if let Some(cmd) = inst.pre_launch.as_deref().filter(|c| !c.trim().is_empty()) {
        emit_state(app, instance_id, prep, "Running pre-launch command");
        run_hook(cmd, &game_dir, &inst).await?;
    }

    // --- spawn
    emit_state(app, instance_id, prep, "Starting Minecraft");
    // Console java.exe (window hidden) gives the most reliable output capture on Windows.
    let java = console_java(java);
    // Always leave a launch log, even if the game never starts.
    let launch_log = game_dir.join("logs").join("halo-launch.log");
    write_launch_header(&launch_log, &java, java_major, &version_id, &args, &auth.access_token);
    log::info!("launching {instance_id} with {} (Java {java_major}), {} args", java.display(), args.len());
    // Java 9+ reads arguments from a file, which avoids Windows' 32k command-line limit
    // with large modded classpaths. The file holds the session token, so it's deleted
    // right after Java has read it.
    let argfile = paths.instance(instance_id).join(".halo").join("launch.args");
    let spawn_args = if java_major >= 9 {
        write_argfile(&argfile, &args)?;
        vec![format!("@{}", argfile.display())]
    } else {
        args.clone()
    };
    // Java writes straight into the launch log. A file never fills up the way a
    // Windows pipe (~4 KB) can, which would freeze the game at its first log line.
    let out = std::fs::OpenOptions::new().append(true).create(true).open(&launch_log).context("couldn't open the launch log")?;
    let err = out.try_clone()?;
    let log_offset = out.metadata().map(|m| m.len()).unwrap_or(0);
    let mut cmd = tokio::process::Command::new(&java);
    cmd.args(&spawn_args).current_dir(&game_dir).stdout(std::process::Stdio::from(out)).stderr(std::process::Stdio::from(err)).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let spawned = cmd.spawn();
    if java_major >= 9 {
        let f = argfile.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            let _ = std::fs::remove_file(f);
        });
    }
    let child = spawned.with_context(|| format!("couldn't start Java at {}", java.display()))?;
    let pid = child.id();
    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
    let pw = match (prewarm, pid) {
        (true, Some(pid)) => Some(Arc::new(std::sync::Mutex::new(Prewarm { pid, created: Instant::now(), ready: false, revealed_at: None, hidden: vec![], ended: false, last_trim: None, reveal_requested: false }))),
        _ => None,
    };
    state.running.lock().await.insert(instance_id.to_string(), RunningGame { pid, started: Instant::now(), kill: Some(kill_tx), prewarm: pw.clone() });

    match (&pw, pid) {
        (Some(pw), Some(pid)) => {
            emit_state(app, instance_id, "prewarming", "Getting ready in the background");
            spawn_hider(pw.clone(), pid);
        }
        _ => {
            emit_state(app, instance_id, "running", "Game running");
            on_shown(app, state, &mut inst, server.as_deref(), &settings).await?;
        }
    }

    // --- monitor
    let app2 = app.clone();
    let state2 = state.clone();
    let id = instance_id.to_string();
    let token = auth.access_token.clone();
    let cds = inst.cds && java_major >= 19 && cds_allowed;
    tokio::spawn(async move {
        match monitor(&app2, &state2, &id, child, kill_rx, clicked, &token, cds, &launch_log, log_offset, pw).await {
            Ok(outcome) => {
                if outcome.prewarm_failed {
                    state2.prewarm_paused.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if outcome.reprewarm {
                    crate::schedule_prewarm(app2.clone(), state2.clone(), std::time::Duration::from_secs(1));
                }
            }
            Err(e) => log::error!("monitor failed: {e:#}"),
        }
    });
    Ok(())
}

/// Bookkeeping for when a game becomes visible (normal launch, or Play on a pre-load).
async fn on_shown<R: tauri::Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, inst: &mut Instance, server: Option<&str>, settings: &crate::core::Settings) -> anyhow::Result<()> {
    inst.last_played = Some(now_ms());
    instances::save(&state.paths, inst)?;
    {
        let mut s = state.settings.write().await;
        s.last_instance_id = Some(inst.id.clone());
        let _ = crate::core::write_json(&state.paths.settings_file(), &*s);
    }
    if settings.discord_rpc {
        state.discord.playing(inst, server);
    }
    if settings.close_on_launch {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    }
    Ok(())
}

/// Keeps a pre-loading game's windows hidden until Play; re-trims memory every
/// 10 minutes while it waits.
fn spawn_hider(pw: Arc<std::sync::Mutex<Prewarm>>, pid: u32) {
    std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            {
                let mut p = pw.lock().unwrap();
                if p.revealed_at.is_some() || p.ended {
                    return;
                }
                // Hide under the lock so Play can never be undone by a late hide.
                let newly = crate::winctl::hide_visible(pid);
                p.hidden.extend(newly);
                p.hidden.dedup();
                if p.ready && p.last_trim.map(|t| t.elapsed().as_secs() >= 600).unwrap_or(false) {
                    let _ = crate::winctl::set_background(pid, true);
                    p.last_trim = Some(Instant::now());
                }
            }
            // Poll fast while the window is being created, then relax.
            let ms = if start.elapsed().as_secs() < 120 { 10 } else { 250 };
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    });
}

/// Once a pre-load reaches the title screen: mute it, then drop it to the
/// lowest priority with efficiency mode and trimmed memory.
fn settle_prewarm(pw: Arc<std::sync::Mutex<Prewarm>>, pid: u32) {
    std::thread::spawn(move || {
        // The audio session appears when the sound engine starts; keep trying briefly.
        for _ in 0..30 {
            {
                let p = pw.lock().unwrap();
                if p.revealed_at.is_some() || p.reveal_requested {
                    return;
                }
            }
            if crate::winctl::set_mute(pid, true).unwrap_or(false) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        // Let the title screen finish loading before trimming memory.
        std::thread::sleep(std::time::Duration::from_secs(4));
        let mut p = pw.lock().unwrap();
        if p.revealed_at.is_none() && !p.ended {
            let _ = crate::winctl::set_background(pid, true);
            p.last_trim = Some(Instant::now());
        }
    });
}

/// If Play was pressed during loading and the title-screen line never appeared,
/// mark it ready so it can be shown anyway. Returns true if it was stuck.
pub async fn force_ready_if_stuck(state: &Arc<AppState>, id: &str) -> bool {
    let running = state.running.lock().await;
    let Some(pw) = running.get(id).and_then(|g| g.prewarm.clone()) else { return false };
    let mut p = pw.lock().unwrap();
    let stuck = p.reveal_requested && p.revealed_at.is_none() && !p.ended && !p.ready;
    if stuck {
        p.ready = true;
    }
    stuck
}

/// Shows a pre-loaded game: restores sound and priority, shows its window and
/// starts playtime/Discord. Returns false if there was nothing to reveal.
pub async fn reveal<R: tauri::Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, id: &str) -> anyhow::Result<bool> {
    let (pid, pw) = {
        let running = state.running.lock().await;
        match running.get(id) {
            Some(g) if g.waiting() => (g.pid, g.prewarm.clone().unwrap()),
            _ => return Ok(false),
        }
    };
    let hidden = {
        let mut p = pw.lock().unwrap();
        if !p.ready {
            // Still loading: keep it hidden and show it the moment the title screen is up,
            // so the player never sees Minecraft's loading screen.
            p.reveal_requested = true;
            drop(p);
            emit_state(app, id, "opening", "Opening");
            return Ok(true);
        }
        p.revealed_at = Some(Instant::now());
        p.hidden.clone()
    };
    if let Some(pid) = pid {
        let _ = crate::winctl::set_background(pid, false);
        crate::winctl::show(pid, &hidden);
        tokio::task::spawn_blocking(move || {
            let _ = crate::winctl::set_mute(pid, false);
        });
    }
    emit_state(app, id, "running", "Game running");
    let settings = state.settings.read().await.clone();
    let mut inst = instances::load(&state.paths, id)?;
    on_shown(app, state, &mut inst, None, &settings).await?;
    log::info!("{id}: Instant Play reveal");
    Ok(true)
}

fn value_contains(v: &StringOrVec, needle: &str) -> bool {
    match v {
        StringOrVec::One(s) => s.contains(needle),
        StringOrVec::Many(v) => v.iter().any(|s| s.contains(needle)),
    }
}

/// G1 settings tuned for a client: short pauses, fast young-gen growth, no
/// pre-touching (that would slow boot). Safe on Java 8+.
pub const TUNED_G1: &[&str] = &[
    "-XX:+UseG1GC",
    "-XX:+ParallelRefProcEnabled",
    "-XX:MaxGCPauseMillis=40",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+DisableExplicitGC",
    "-XX:G1NewSizePercent=30",
    "-XX:G1MaxNewSizePercent=40",
    "-XX:G1HeapRegionSize=8M",
    "-XX:G1ReservePercent=20",
    "-XX:G1MixedGCCountTarget=4",
    "-XX:InitiatingHeapOccupancyPercent=15",
    "-XX:G1MixedGCLiveThresholdPercent=90",
    "-XX:SurvivorRatio=32",
    "-XX:+PerfDisableSharedMem",
];

#[allow(clippy::too_many_arguments)]
pub fn build_args(
    paths: &Paths,
    inst: &Instance,
    p: &Prepared,
    auth: &crate::auth::LaunchAuth,
    game_dir: &Path,
    ram_mb: u32,
    java_major: u32,
    server: Option<&str>,
    f: &Features,
    cds_allowed: bool,
) -> Vec<String> {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut seen = std::collections::HashSet::new();
    let classpath: Vec<String> = p.classpath.iter().filter(|c| seen.insert((*c).clone())).map(|c| c.to_string_lossy().to_string()).collect();
    let (host, port) = server.map(split_address).unwrap_or_default();

    let vars: HashMap<&str, String> = HashMap::from([
        ("natives_directory", p.natives_dir.to_string_lossy().to_string()),
        ("launcher_name", "halo-client".into()),
        ("launcher_version", crate::core::APP_VERSION.into()),
        ("classpath", classpath.join(sep)),
        ("classpath_separator", sep.into()),
        ("library_directory", paths.libraries().to_string_lossy().to_string()),
        ("version_name", p.version.id.clone()),
        ("auth_player_name", auth.username.clone()),
        ("game_directory", game_dir.to_string_lossy().to_string()),
        ("assets_root", paths.assets().to_string_lossy().to_string()),
        ("game_assets", p.game_assets_dir.to_string_lossy().to_string()),
        ("assets_index_name", p.assets_index.clone()),
        ("auth_uuid", auth.uuid.clone()),
        ("auth_access_token", auth.access_token.clone()),
        ("auth_session", auth.access_token.clone()),
        ("clientid", String::new()),
        ("auth_xuid", String::new()),
        ("user_type", auth.user_type.into()),
        ("version_type", "Halo Client".into()),
        ("user_properties", "{}".into()),
        ("resolution_width", inst.width.unwrap_or(854).to_string()),
        ("resolution_height", inst.height.unwrap_or(480).to_string()),
        ("quickPlayMultiplayer", format!("{host}:{port}")),
    ]);
    let sub = |s: &str| -> String {
        let mut out = s.to_string();
        // Repeat for values that reference other values (rare, but cheap).
        for _ in 0..2 {
            for (k, v) in &vars {
                out = out.replace(&format!("${{{k}}}"), v);
            }
        }
        out
    };
    let expand = |list: &[ArgValue]| -> Vec<String> {
        let mut v = vec![];
        for a in list {
            match a {
                ArgValue::Plain(s) => v.push(sub(s)),
                ArgValue::Ruled { rules, value } if versions::rules_allow(Some(rules), f) => match value {
                    StringOrVec::One(s) => v.push(sub(s)),
                    StringOrVec::Many(m) => v.extend(m.iter().map(|s| sub(s))),
                },
                _ => {}
            }
        }
        v
    };

    let mut args = vec![format!("-Xmx{ram_mb}M"), format!("-Xms{}M", (ram_mb / 4).clamp(512, 2048).min(ram_mb))];
    if inst.tuned_flags {
        args.extend(TUNED_G1.iter().map(|s| s.to_string()));
    }
    // AppCDS: Java 19+ creates and reuses a per-instance class archive automatically.
    // The JVM refuses to archive when the classpath has directories, so only jars qualify.
    let all_jars = p.classpath.iter().all(|c| c.extension().map(|e| e == "jar").unwrap_or(false));
    if inst.cds && java_major >= 19 && all_jars && cds_allowed {
        let dir = paths.instance(&inst.id).join(".halo");
        let _ = std::fs::create_dir_all(&dir);
        let name: String = p.version.id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
        args.push("-XX:+AutoCreateSharedArchive".into());
        args.push(format!("-XX:SharedArchiveFile={}", dir.join(format!("{name}.jsa")).display()));
    }
    args.push("-Dlog4j2.formatMsgNoLookups=true".into());
    // Make the game's console output UTF-8 so logs read cleanly (Java 18+; ignored before).
    args.push("-Dstdout.encoding=UTF-8".into());
    args.push("-Dstderr.encoding=UTF-8".into());
    args.extend(split_args(&inst.jvm_args));

    match p.version.arguments.as_ref().filter(|a| !a.jvm.is_empty()) {
        Some(a) => args.extend(expand(&a.jvm)),
        None => {
            if cfg!(windows) {
                args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into());
            }
            args.push(format!("-Djava.library.path={}", vars["natives_directory"]));
            args.push("-cp".into());
            args.push(vars["classpath"].clone());
        }
    }
    args.push(p.version.main_class.clone().unwrap_or_else(|| "net.minecraft.client.main.Main".into()));

    let modern_game = p.version.arguments.as_ref().map(|a| !a.game.is_empty()).unwrap_or(false);
    if modern_game {
        args.extend(expand(&p.version.arguments.as_ref().unwrap().game));
    }
    if let Some(legacy) = &p.version.minecraft_arguments {
        args.extend(legacy.split_whitespace().map(sub));
    }
    // Drop options whose value is empty (e.g. --clientId "" / --xuid ""): some
    // argument parsers misread a blank value as the next option.
    let mut cleaned = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") && args.get(i + 1).map(|v| v.is_empty()).unwrap_or(false) {
            i += 2;
            continue;
        }
        cleaned.push(std::mem::take(&mut args[i]));
        i += 1;
    }
    args = cleaned;
    if server.is_some() && !f.quick_play_multiplayer {
        args.extend(["--server".into(), host, "--port".into(), port.to_string()]);
    }
    if !modern_game && f.custom_resolution {
        args.extend(["--width".into(), vars["resolution_width"].clone(), "--height".into(), vars["resolution_height"].clone()]);
    }
    args
}

/// On Windows, prefer java.exe over javaw.exe (same runtime, reliable output pipes).
fn console_java(java: PathBuf) -> PathBuf {
    if cfg!(windows) && java.file_name().map(|n| n.eq_ignore_ascii_case("javaw.exe")).unwrap_or(false) {
        let j = java.with_file_name("java.exe");
        if j.exists() {
            return j;
        }
    }
    java
}

/// Writes a Java @argfile: one quoted argument per line, with \ and " escaped.
pub fn write_argfile(path: &Path, args: &[String]) -> anyhow::Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let body: String = args.iter().map(|a| format!("\"{}\"\n", a.replace('\\', "\\\\").replace('"', "\\\""))).collect();
    std::fs::write(path, body)?;
    Ok(())
}

fn write_launch_header(path: &Path, java: &Path, java_major: u32, version_id: &str, args: &[String], token: &str) {
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(path));
    let shown: Vec<String> = args.iter().map(|a| if !token.is_empty() && token != "0" && a.contains(token) { a.replace(token, "<access token hidden>") } else { a.clone() }).collect();
    let text = format!(
        "Halo Client {} launch log\ntime: {}\njava: {} (Java {java_major})\nversion: {version_id}\nargs:\n  {}\n--- game output ---\n",
        crate::core::APP_VERSION,
        chrono::Local::now().to_rfc3339(),
        java.display(),
        shown.join("\n  ")
    );
    let _ = std::fs::write(path, text);
}

/// Splits user JVM args on whitespace, keeping "double quoted" parts together.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

async fn run_hook(cmd: &str, dir: &Path, inst: &Instance) -> anyhow::Result<()> {
    let mut c = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(cmd);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    };
    c.current_dir(dir).env("INST_NAME", &inst.name).env("INST_ID", &inst.id).env("INST_MC_DIR", dir);
    #[cfg(windows)]
    c.creation_flags(0x0800_0000);
    let status = c.status().await.context("couldn't run the instance command")?;
    if !status.success() {
        bail!("Pre-launch command exited with {status}");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn monitor<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    id: &str,
    mut child: tokio::process::Child,
    kill_rx: tokio::sync::oneshot::Receiver<()>,
    clicked: Instant,
    token: &str,
    cds: bool,
    launch_log: &Path,
    log_offset: u64,
    prewarm: Option<Arc<std::sync::Mutex<Prewarm>>>,
) -> anyhow::Result<ExitOutcome> {
    let paths = &state.paths;
    let mut got_output = false;
    let mut warned_silent = false;

    // Waiter: owns the process; reports (exit status, killed by user).
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (exit_tx, exit_rx) = tokio::sync::oneshot::channel::<(std::io::Result<std::process::ExitStatus>, bool)>();
    {
        let done = done.clone();
        tokio::spawn(async move {
            let r = tokio::select! {
                s = child.wait() => (s, false),
                _ = kill_rx => {
                    let _ = child.start_kill();
                    (child.wait().await, true)
                }
            };
            done.store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = exit_tx.send(r);
        });
    }
    // Tail: streams new lines from the launch log until the process has exited.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    crate::core::spawn_file_tail(launch_log.to_path_buf(), log_offset, done.clone(), tx);

    let connect_re = regex::Regex::new(r"Connecting to ([^\s,]+), (\d+)").unwrap();
    let mut boot_ms: Option<u64> = None;
    let mut batch: Vec<String> = vec![];
    let mut last_flush = Instant::now();
    let started = Instant::now();
    state.logs.lock().unwrap().insert(id.to_string(), std::collections::VecDeque::new());

    loop {
        tokio::select! {
            line = rx.recv() => {
                match line {
                    Some(mut line) => {
                        if !token.is_empty() && token != "0" && line.contains(token) {
                            line = line.replace(token, "<access token hidden>");
                        }
                        got_output = true;
                        if boot_ms.is_none() && BOOT_MARKERS.iter().any(|m| line.contains(m)) {
                            let ms = clicked.elapsed().as_millis() as u64;
                            boot_ms = Some(ms);
                            on_boot(app, state, id, ms, &prewarm);
                        }
                        if let Some(c) = connect_re.captures(&line) {
                            let port: u16 = c[2].parse().unwrap_or(25565);
                            let addr = if port == 25565 { c[1].to_string() } else { format!("{}:{}", &c[1], port) };
                            crate::servers::record_recent(paths, &addr, id).ok();
                            let _ = app.emit("recent-servers-changed", ());
                        }
                        {
                            let mut logs = state.logs.lock().unwrap();
                            let buf = logs.entry(id.to_string()).or_default();
                            buf.push_back(line.clone());
                            while buf.len() > 5000 { buf.pop_front(); }
                        }
                        batch.push(line);
                        if last_flush.elapsed().as_millis() > 150 || batch.len() > 200 {
                            let _ = app.emit("game-log", GameLog { instance_id: id.into(), lines: std::mem::take(&mut batch) });
                            last_flush = Instant::now();
                        }
                    }
                    None => {
                        // The process exited and the log is fully read.
                        if !batch.is_empty() {
                            let _ = app.emit("game-log", GameLog { instance_id: id.into(), lines: std::mem::take(&mut batch) });
                        }
                        break;
                    }
                }
            }
            // Watchdog: tell the player if the game has been silent for 20 seconds.
            _ = tokio::time::sleep(std::time::Duration::from_millis(1000)), if !got_output && !warned_silent && started.elapsed().as_secs() >= 20 => {
                warned_silent = true;
                log::warn!("{id}: no output from the game after 20s");
                let _ = app.emit("launch-warning", serde_json::json!({
                    "instanceId": id,
                    "message": "Minecraft hasn't printed anything for 20 seconds. It may be stuck.",
                    "logPath": launch_log.to_string_lossy(),
                }));
            }
            // Fallback timer: if the game prints nothing to us, read its own latest.log for the marker.
            _ = tokio::time::sleep(std::time::Duration::from_millis(1000)), if boot_ms.is_none() && started.elapsed().as_secs() >= 3 => {
                let log = instances::game_dir(paths, id).join("logs").join("latest.log");
                let fresh = std::fs::metadata(&log).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|e| e <= started.elapsed()).unwrap_or(false);
                if fresh && std::fs::read_to_string(&log).map(|t| BOOT_MARKERS.iter().any(|m| t.contains(m))).unwrap_or(false) {
                    let ms = clicked.elapsed().as_millis() as u64;
                    boot_ms = Some(ms);
                    on_boot(app, state, id, ms, &prewarm);
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)), if !batch.is_empty() => {
                let _ = app.emit("game-log", GameLog { instance_id: id.into(), lines: std::mem::take(&mut batch) });
                last_flush = Instant::now();
            }
        }
    }

    let (status, killed) = exit_rx.await.map_err(|_| anyhow::anyhow!("lost track of the game process"))?;
    let status = status?;
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(launch_log) {
        use std::io::Write;
        let _ = writeln!(f, "--- process exited: {:?} after {}s", status.code(), started.elapsed().as_secs());
    }
    // For Instant Play, playtime starts when the window was shown.
    let (was_prewarm, revealed_at) = match &prewarm {
        Some(p) => {
            let mut p = p.lock().unwrap();
            p.ended = true;
            (true, p.revealed_at)
        }
        None => (false, None),
    };
    let shown = !was_prewarm || revealed_at.is_some();
    let played = if was_prewarm { revealed_at.map(|t| t.elapsed().as_secs()).unwrap_or(0) } else { started.elapsed().as_secs() };
    state.running.lock().await.remove(id);
    let code = status.code();
    let mut inst = instances::load(paths, id)?;
    inst.playtime_secs += played;
    if let Some(ms) = boot_ms {
        inst.last_boot_ms = Some(ms);
        inst.boot_history.push(BootSample { ms, at: now_ms(), cds, perf: inst.perf_profile });
        if inst.boot_history.len() > 30 {
            inst.boot_history.remove(0);
        }
    }
    instances::save(paths, &inst)?;

    let crash = if shown && !killed && code.map(|c| c != 0).unwrap_or(true) {
        let log_text = state.logs.lock().unwrap().get(id).map(|b| b.iter().cloned().collect::<Vec<_>>().join("\n")).unwrap_or_default();
        let crash_file = newest_crash_report(&instances::game_dir(paths, id), started);
        let crash_text = crash_file.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        Some(analyze(&format!("{log_text}\n{crash_text}"), code).with_crash_file(crash_file))
    } else {
        None
    };

    if shown {
        if let Some(cmd) = inst.post_exit.as_deref().filter(|c| !c.trim().is_empty()) {
            let _ = run_hook(cmd, &instances::game_dir(paths, id), &inst).await;
        }
        state.discord.clear();
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
        }
    }
    let _ = app.emit("game-exit", GameExit { instance_id: id.into(), code, played_secs: played, boot_ms, crash, killed });
    emit_state(app, id, "stopped", "Game closed");
    Ok(ExitOutcome {
        // Pre-load again after a real play session, or after we stopped a pre-load ourselves.
        reprewarm: shown || killed,
        prewarm_failed: was_prewarm && !shown && !killed,
    })
}

fn on_boot<R: tauri::Runtime>(app: &AppHandle<R>, state: &Arc<AppState>, id: &str, ms: u64, prewarm: &Option<Arc<std::sync::Mutex<Prewarm>>>) {
    let _ = app.emit("game-boot", serde_json::json!({ "instanceId": id, "bootMs": ms, "background": prewarm.is_some() }));
    if let Some(pw) = prewarm {
        let (pid, show_now) = {
            let mut p = pw.lock().unwrap();
            if p.revealed_at.is_some() {
                return;
            }
            p.ready = true;
            (p.pid, p.reveal_requested)
        };
        if show_now {
            // Play was pressed while it loaded: show it now that the title screen is up.
            let (app, state, id) = (app.clone(), state.clone(), id.to_string());
            tauri::async_runtime::spawn(async move {
                if let Err(e) = reveal(&app, &state, &id).await {
                    log::warn!("reveal after load failed: {e:#}");
                }
            });
            return;
        }
        settle_prewarm(pw.clone(), pid);
        emit_state(app, id, "ready", "Instant Play ready");
    }
}

fn newest_crash_report(game_dir: &Path, since: Instant) -> Option<PathBuf> {
    let since_sys = std::time::SystemTime::now().checked_sub(since.elapsed())?;
    std::fs::read_dir(game_dir.join("crash-reports"))
        .ok()?
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(t, _)| *t >= since_sys)
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

impl CrashReport {
    fn with_crash_file(mut self, p: Option<PathBuf>) -> Self {
        self.crash_file = p.map(|p| p.to_string_lossy().to_string());
        self
    }
}

/// Recognizes the most common reasons a game fails to start or crashes.
pub fn analyze(text: &str, code: Option<i32>) -> CrashReport {
    let r = |t: &str, d: String, fix: Option<CrashFix>| CrashReport { title: t.into(), detail: d, fix, crash_file: None };
    let fix = |kind: &str, arg: Option<String>, label: &str| Some(CrashFix { kind: kind.into(), arg, label: label.into() });
    let cap = |pat: &str| regex::Regex::new(pat).ok().and_then(|re| re.captures(text)).map(|c| c[1].to_string());

    if text.contains("UnsupportedClassVersionError") || text.contains("compiled by a more recent version of the Java Runtime") {
        return r("Wrong Java version", "A mod or the game needs a newer Java than the one used.".into(), fix("auto_java", None, "Use Halo's managed Java"));
    }
    // Fabric: "requires any version of fabric-api, which is missing!" / "requires version X of mod 'y'"
    if let Some(dep) = cap(r"requires (?:any version|version [^ ]+(?: or later)?) of (?:mod )?'?([A-Za-z0-9_\-]+)'?(?:,|\s)+which is missing")
        .or_else(|| cap(r"(?i)requires .*? of (?:mod )?'?([A-Za-z0-9_\-]+)'?, which is missing"))
    {
        return r("Missing dependency", format!("A mod needs '{dep}', which isn't installed."), fix("install_dependency", Some(dep), "Install it"));
    }
    // NeoForge/Forge: "Mod ID: 'x', Requested by: 'y'"
    if let Some(dep) = cap(r"Mod ID: '([^']+)', Requested by") {
        return r("Missing dependency", format!("A mod needs '{dep}', which isn't installed."), fix("install_dependency", Some(dep), "Install it"));
    }
    if text.contains("DuplicateModsFoundException") || text.contains("Found duplicate mods") || text.contains("Duplicate mods found") {
        let m = cap(r"(?i)duplicate mods?.*?'?([A-Za-z0-9_\-]+)'?").unwrap_or_default();
        return r("Duplicate mod", format!("The same mod is installed twice{}.", if m.is_empty() { String::new() } else { format!(" ({m})") }), fix("remove_duplicate", Some(m), "Show mods"));
    }
    if text.contains("OutOfMemoryError") {
        return r("Out of memory", "The game ran out of memory.".into(), fix("increase_ram", None, "Give it more RAM"));
    }
    if let Some(m) = cap(r"Mixin apply (?:for mod|failed) ([A-Za-z0-9_\-]+)").or_else(|| cap(r"from mod ([A-Za-z0-9_\-]+)\]")) {
        return r("Mod conflict", format!("'{m}' failed to patch the game. It may be incompatible with this version or another mod."), fix("disable_mod", Some(m), "Disable it"));
    }
    if text.contains("Incompatible mods found") || text.contains("Incompatible mod set") {
        return r("Incompatible mods", "Some mods don't work with this Minecraft version or each other. See the log for the list.".into(), None);
    }
    if text.contains("Could not create the Java Virtual Machine") || text.contains("Invalid maximum heap size") || text.contains("Unrecognized VM option") {
        return r("Invalid Java arguments", "Java didn't accept the JVM arguments.".into(), fix("reset_jvm_args", None, "Reset JVM arguments"));
    }
    if text.contains("Pixel format not accelerated") || text.contains("GLFW error 65542") || text.contains("WGL: The driver does not appear to support OpenGL") {
        return r("Graphics driver problem", "Your GPU driver doesn't support OpenGL properly. Update your graphics driver.".into(), fix("update_drivers", None, "How to update"));
    }
    r(
        "The game crashed",
        format!("Minecraft exited with code {}. Open the log for details.", code.map(|c| c.to_string()).unwrap_or_else(|| "unknown".into())),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(split_address("mc.hypixel.net"), ("mc.hypixel.net".into(), 25565));
        assert_eq!(split_address("play.example.com:25570"), ("play.example.com".into(), 25570));
        assert_eq!(split_address("[::1]:25566"), ("::1".into(), 25566));
    }

    #[test]
    fn jvm_arg_split() {
        assert_eq!(split_args(r#"-Xss2M "-Dfoo=a b"  -Dx=1"#), vec!["-Xss2M", "-Dfoo=a b", "-Dx=1"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn crash_rules() {
        assert_eq!(analyze("java.lang.UnsupportedClassVersionError: x", Some(1)).fix.unwrap().kind, "auto_java");
        let fabric = "net.fabricmc.loader.impl.FormattedException: Mod 'Iris' (iris) 1.9.6 requires any version of sodium, which is missing!";
        let a = analyze(fabric, Some(1));
        assert_eq!(a.title, "Missing dependency");
        assert_eq!(a.fix.unwrap().arg.unwrap(), "sodium");
        let neo = "Missing or unsupported mandatory dependencies:\n\tMod ID: 'architectury', Requested by: 'rei', Expected range: '[13,)'";
        assert_eq!(analyze(neo, Some(1)).fix.unwrap().arg.unwrap(), "architectury");
        assert_eq!(analyze("java.lang.OutOfMemoryError: Java heap space", Some(1)).fix.unwrap().kind, "increase_ram");
        assert_eq!(analyze("nothing", Some(-1)).title, "The game crashed");
    }
}

/// Runs the real argument builder and launches an actual JVM with the result,
/// using a tiny stand-in "game" class. Proves the tuned G1 + AppCDS flags are
/// accepted by Java and that every placeholder is substituted.
#[cfg(test)]
mod jvm_tests {
    use super::*;
    use crate::auth::LaunchAuth;
    use crate::versions::{Arguments, VersionJson};

    fn have_javac() -> bool {
        std::process::Command::new("javac").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    #[test]
    fn launches_a_real_jvm_with_built_args() {
        if !have_javac() {
            eprintln!("skipping: no javac");
            return;
        }
        let tmp = std::env::temp_dir().join(format!("halo-jvm-test-{}", uuid::Uuid::new_v4()));
        let paths = Paths { root: tmp.clone() };
        std::fs::create_dir_all(paths.libraries()).unwrap();
        let classes = tmp.join("classes");
        std::fs::create_dir_all(&classes).unwrap();
        std::fs::write(
            classes.join("FakeGame.java"),
            r#"public class FakeGame { public static void main(String[] a) {
                System.out.println("ARGS " + String.join("|", a));
                System.out.println("[Render thread/INFO]: Sound engine started");
            } }"#,
        )
        .unwrap();
        let ok = std::process::Command::new("javac").arg("-d").arg(&classes).arg(classes.join("FakeGame.java")).status().unwrap();
        assert!(ok.success());
        let jar = tmp.join("game.jar");
        let ok = std::process::Command::new("jar").arg("cf").arg(&jar).arg("-C").arg(&classes).arg("FakeGame.class").status().unwrap();
        assert!(ok.success());

        let version = VersionJson {
            id: "fake-1.0".into(),
            main_class: Some("FakeGame".into()),
            arguments: Some(Arguments {
                game: vec![
                    ArgValue::Plain("--username".into()),
                    ArgValue::Plain("${auth_player_name}".into()),
                    ArgValue::Plain("--gameDir".into()),
                    ArgValue::Plain("${game_directory}".into()),
                    ArgValue::Plain("--uuid".into()),
                    ArgValue::Plain("${auth_uuid}".into()),
                    ArgValue::Ruled {
                        rules: vec![crate::versions::Rule { action: "allow".into(), os: None, features: Some(HashMap::from([("is_quick_play_multiplayer".into(), true)])) }],
                        value: StringOrVec::Many(vec!["--quickPlayMultiplayer".into(), "${quickPlayMultiplayer}".into()]),
                    },
                ],
                jvm: vec![ArgValue::Plain("-Djava.library.path=${natives_directory}".into()), ArgValue::Plain("-cp".into()), ArgValue::Plain("${classpath}".into())],
            }),
            ..Default::default()
        };
        let prepared = Prepared {
            version,
            classpath: vec![jar.clone()],
            natives_dir: tmp.join("natives"),
            client_jar: jar.clone(),
            assets_index: "17".into(),
            game_assets_dir: paths.assets(),
            java_major: 21,
        };
        let inst = Instance { id: "test".into(), name: "Test".into(), ..Default::default() };
        let auth = LaunchAuth { username: "LeoPlays".into(), uuid: "abc123".into(), access_token: "secret-token".into(), user_type: "msa" };
        let game_dir = tmp.join("game");
        std::fs::create_dir_all(&game_dir).unwrap();
        let f = Features { custom_resolution: false, quick_play_multiplayer: true };
        let args = build_args(&paths, &inst, &prepared, &auth, &game_dir, 1024, 21, Some("mc.hypixel.net"), &f, true);

        assert!(args.iter().all(|a| !a.contains("${")), "unsubstituted placeholder in {args:?}");
        assert!(args.contains(&"-Xmx1024M".to_string()));
        assert!(args.iter().any(|a| a == "-XX:+AutoCreateSharedArchive"));
        assert!(args.iter().any(|a| a == "mc.hypixel.net:25565"));
        assert!(!args.iter().any(|a| a == "--server"), "quick play replaces --server");

        // Run via an @argfile (as on Windows) with a path containing spaces, quotes and backslashes.
        let mut tricky = args.clone();
        tricky.extend(["C:\\Users\\Some One\\x".to_string(), "say \"hi\"".to_string()]);
        let argfile = tmp.join("dir with space").join("launch.args");
        write_argfile(&argfile, &tricky).unwrap();
        let out = std::process::Command::new("java").arg(format!("@{}", argfile.display())).current_dir(&game_dir).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "argfile run failed: {}", String::from_utf8_lossy(&out.stderr));
        assert!(stdout.contains("|C:\\Users\\Some One\\x|say \"hi\""), "{stdout}");

        // First run creates the CDS archive; second run must reuse it without errors.
        for run in 0..2 {
            let out = std::process::Command::new("java").args(&args).current_dir(&game_dir).output().unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(out.status.success(), "run {run} failed: {stderr}");
            assert!(stdout.contains("ARGS --username|LeoPlays|--gameDir|"), "{stdout}");
            assert!(stdout.contains("--quickPlayMultiplayer|mc.hypixel.net:25565"), "{stdout}");
            assert!(stdout.contains("Sound engine started"));
            assert!(!stderr.contains("Unrecognized VM option"), "{stderr}");
        }
        let mut dir_cp = prepared.classpath.clone();
        dir_cp.push(classes.clone());
        let p2 = Prepared { classpath: dir_cp, ..prepared };
        let a2 = build_args(&paths, &inst, &p2, &auth, &game_dir, 1024, 21, None, &Features::default(), true);
        assert!(!a2.iter().any(|a| a.contains("AutoCreateSharedArchive")), "CDS must be off when the classpath has directories");
        let jsa = paths.instance("test").join(".halo").join("fake-1.0.jsa");
        assert!(jsa.exists(), "CDS archive should have been created at {}", jsa.display());
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn legacy_versions_get_server_flag_and_cp() {
        let tmp = std::env::temp_dir().join("halo-legacy-test");
        let paths = Paths { root: tmp.clone() };
        let version = VersionJson {
            id: "1.8.9".into(),
            main_class: Some("net.minecraft.client.main.Main".into()),
            minecraft_arguments: Some("--username ${auth_player_name} --session ${auth_session} --assetIndex ${assets_index_name}".into()),
            ..Default::default()
        };
        let prepared = Prepared { version, classpath: vec![tmp.join("a.jar")], natives_dir: tmp.join("n"), client_jar: tmp.join("a.jar"), assets_index: "1.8".into(), game_assets_dir: tmp.clone(), java_major: 8 };
        let inst = Instance { id: "x".into(), cds: true, ..Default::default() };
        let auth = LaunchAuth { username: "Steve".into(), uuid: "u".into(), access_token: "tok".into(), user_type: "msa" };
        let args = build_args(&paths, &inst, &prepared, &auth, &tmp, 2048, 8, Some("play.example.com:25570"), &Features::default(), true);
        assert!(args.contains(&"-cp".to_string()));
        assert!(!args.iter().any(|a| a.contains("AutoCreateSharedArchive")), "no CDS on Java 8");
        let i = args.iter().position(|a| a == "--server").unwrap();
        assert_eq!(args[i + 1], "play.example.com");
        assert_eq!(args[i + 3], "25570");
        assert!(args.iter().any(|a| a == "1.8"));
    }

    #[test]
    fn empty_option_values_are_dropped() {
        let tmp = std::env::temp_dir().join("halo-empty-test");
        let paths = Paths { root: tmp.clone() };
        let version = VersionJson {
            id: "x".into(),
            main_class: Some("M".into()),
            arguments: Some(Arguments {
                game: ["--clientId", "${clientid}", "--xuid", "${auth_xuid}", "--versionType", "${version_type}"].iter().map(|s| ArgValue::Plain(s.to_string())).collect(),
                jvm: vec![ArgValue::Plain("-cp".into()), ArgValue::Plain("${classpath}".into())],
            }),
            ..Default::default()
        };
        let prepared = Prepared { version, classpath: vec![tmp.join("a.jar")], natives_dir: tmp.clone(), client_jar: tmp.join("a.jar"), assets_index: "1".into(), game_assets_dir: tmp.clone(), java_major: 21 };
        let auth = LaunchAuth { username: "a".into(), uuid: "u".into(), access_token: "0".into(), user_type: "legacy" };
        let args = build_args(&paths, &Instance::default(), &prepared, &auth, &tmp, 1024, 21, None, &Features::default(), true);
        assert!(!args.iter().any(|a| a == "--clientId" || a == "--xuid"), "{args:?}");
        let i = args.iter().position(|a| a == "--versionType").unwrap();
        assert_eq!(args[i + 1], "Halo Client");
    }
}

/// End-to-end: real JVM → launch log file → tail → monitor (boot timer,
/// recent servers, exit). The fake game prints far more than a Windows pipe
/// buffer holds before the boot marker, so a pipe-based design would stall.
#[cfg(test)]
mod monitor_tests {
    use super::*;

    #[tokio::test]
    async fn monitor_streams_file_output_and_sees_exit() {
        if std::process::Command::new("javac").arg("-version").output().map(|o| !o.status.success()).unwrap_or(true) {
            eprintln!("skipping: no javac");
            return;
        }
        let tmp = std::env::temp_dir().join(format!("halo-monitor-{}", uuid::Uuid::new_v4()));
        let paths = Paths { root: tmp.clone() };
        let inst = instances::create(&paths, Instance { name: "Mon".into(), mc_version: "x".into(), ..Default::default() }).unwrap();
        let classes = tmp.join("c");
        std::fs::create_dir_all(&classes).unwrap();
        std::fs::write(
            classes.join("Noisy.java"),
            r#"public class Noisy { public static void main(String[] a) throws Exception {
                StringBuilder sb = new StringBuilder();
                for (int i = 0; i < 400; i++) System.out.println("[main/INFO]: filler line " + i + " ................................");
                System.err.println("[Render thread/WARN]: on stderr");
                System.out.println("[Render thread/INFO]: Sound engine started");
                System.out.println("[Render thread/INFO]: Connecting to play.example.com, 25570");
                Thread.sleep(300);
            } }"#,
        )
        .unwrap();
        assert!(std::process::Command::new("javac").arg("-d").arg(&classes).arg(classes.join("Noisy.java")).status().unwrap().success());

        let launch_log = instances::game_dir(&paths, &inst.id).join("logs").join("halo-launch.log");
        std::fs::create_dir_all(launch_log.parent().unwrap()).unwrap();
        std::fs::write(&launch_log, b"header\n").unwrap();
        let out = std::fs::OpenOptions::new().append(true).open(&launch_log).unwrap();
        let err = out.try_clone().unwrap();
        let offset = out.metadata().unwrap().len();
        let child = tokio::process::Command::new("java")
            .arg("-cp")
            .arg(&classes)
            .arg("Noisy")
            .stdout(std::process::Stdio::from(out))
            .stderr(std::process::Stdio::from(err))
            .spawn()
            .unwrap();

        let app = tauri::test::mock_app();
        let state = Arc::new(AppState {
            accounts: tokio::sync::Mutex::new(Default::default()),
            http: crate::core::http_client(),
            settings: tokio::sync::RwLock::new(Default::default()),
            flows: Default::default(),
            running: Default::default(),
            launching: Default::default(),
            logs: Default::default(),
            discord: Default::default(),
            update: Default::default(),
            prewarm_paused: Default::default(),
            paths: paths.clone(),
        });
        let (_kill_tx, kill_rx) = tokio::sync::oneshot::channel();
        let r = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            monitor(app.handle(), &state, &inst.id, child, kill_rx, Instant::now(), "0", false, &launch_log, offset, None),
        )
        .await;
        assert!(r.is_ok(), "monitor hung");
        r.unwrap().unwrap();

        let lines: Vec<String> = state.logs.lock().unwrap().get(&inst.id).unwrap().iter().cloned().collect();
        assert!(lines.len() >= 403, "got {} lines", lines.len());
        assert!(lines.iter().any(|l| l.contains("on stderr")));
        let saved = instances::load(&paths, &inst.id).unwrap();
        assert!(saved.last_boot_ms.is_some(), "boot marker not detected");
        let recent = crate::servers::recent(&paths);
        assert_eq!(recent[0].address, "play.example.com:25570");
        let log = std::fs::read_to_string(&launch_log).unwrap();
        assert!(log.contains("--- process exited: Some(0)"), "exit not recorded");
        let _ = std::fs::remove_dir_all(tmp);
    }
}

#[cfg(test)]
mod prewarm_tests {
    use super::*;

    #[test]
    fn phases_follow_prewarm_lifecycle() {
        let normal = RunningGame { pid: Some(1), started: Instant::now(), kill: None, prewarm: None };
        assert_eq!(normal.phase(), "running");
        assert!(!normal.waiting());

        let pw = Arc::new(std::sync::Mutex::new(Prewarm { pid: 1, created: Instant::now(), ready: false, revealed_at: None, hidden: vec![], ended: false, last_trim: None, reveal_requested: false }));
        let g = RunningGame { pid: Some(1), started: Instant::now(), kill: None, prewarm: Some(pw.clone()) };
        assert_eq!(g.phase(), "prewarming");
        assert!(g.waiting());
        pw.lock().unwrap().reveal_requested = true;
        assert_eq!(g.phase(), "opening");
        pw.lock().unwrap().reveal_requested = false;
        pw.lock().unwrap().ready = true;
        assert_eq!(g.phase(), "ready");
        pw.lock().unwrap().revealed_at = Some(Instant::now());
        assert_eq!(g.phase(), "running");
        assert!(!g.waiting());
    }
}
