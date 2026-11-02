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
}

fn emit_state(app: &AppHandle, id: &str, phase: &str, message: impl Into<String>) {
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

pub async fn launch(app: AppHandle, state: Arc<AppState>, instance_id: String, server: Option<String>) -> anyhow::Result<()> {
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
    let result = launch_inner(&app, &state, &instance_id, server).await;
    state.launching.lock().await.remove(&instance_id);
    if let Err(e) = &result {
        emit_state(&app, &instance_id, "error", format!("{e:#}"));
    }
    result
}

async fn launch_inner(app: &AppHandle, state: &Arc<AppState>, instance_id: &str, server: Option<String>) -> anyhow::Result<()> {
    let clicked = Instant::now();
    let paths = &state.paths;
    let http = &state.http;
    let settings = state.settings.read().await.clone();
    let mut inst = instances::load(paths, instance_id)?;
    let game_dir = instances::game_dir(paths, instance_id);
    std::fs::create_dir_all(&game_dir)?;

    // --- account
    emit_state(app, instance_id, "preparing", "Checking account");
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

    let dl = Downloader { http, paths, app, threads: settings.download_threads.clamp(1, 64) as usize, verify_existing: false };

    // --- loader + version
    emit_state(app, instance_id, "preparing", "Preparing game version");
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

    emit_state(app, instance_id, "preparing", "Checking game files");
    let prepared = versions::prepare(&dl, &version_id, &game_dir, &features).await?;

    // --- Halo HUD keeps itself in sync with the bundled build
    crate::content::sync_halo_hud(app, paths, &inst).ok();

    // --- java
    emit_state(app, instance_id, "preparing", "Checking Java");
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
    let args = build_args(paths, &inst, &prepared, &auth, &game_dir, ram, java_major, server.as_deref(), &features);

    // --- pre-launch hook
    if let Some(cmd) = inst.pre_launch.as_deref().filter(|c| !c.trim().is_empty()) {
        emit_state(app, instance_id, "preparing", "Running pre-launch command");
        run_hook(cmd, &game_dir, &inst).await?;
    }

    // --- spawn
    emit_state(app, instance_id, "preparing", "Starting Minecraft");
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
    let mut cmd = tokio::process::Command::new(&java);
    cmd.args(&spawn_args).current_dir(&game_dir).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null());
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
    let mut child = spawned.with_context(|| format!("couldn't start Java at {}", java.display()))?;
    let pid = child.id();
    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
    state.running.lock().await.insert(instance_id.to_string(), RunningGame { pid, started: Instant::now(), kill: Some(kill_tx) });
    emit_state(app, instance_id, "running", "Game running");

    inst.last_played = Some(now_ms());
    instances::save(paths, &inst)?;
    {
        let mut s = state.settings.write().await;
        s.last_instance_id = Some(instance_id.to_string());
        let _ = crate::core::write_json(&paths.settings_file(), &*s);
    }
    if settings.discord_rpc {
        state.discord.playing(&inst, server.as_deref());
    }
    if settings.close_on_launch {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    }

    // --- monitor
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let app2 = app.clone();
    let state2 = state.clone();
    let id = instance_id.to_string();
    let token = auth.access_token.clone();
    let cds = inst.cds && java_major >= 19;
    tokio::spawn(async move {
        let res = monitor(&app2, &state2, &id, child, stdout, stderr, kill_rx, clicked, &token, cds, &launch_log).await;
        if let Err(e) = res {
            log::error!("monitor failed: {e:#}");
        }
    });
    Ok(())
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
    if inst.cds && java_major >= 19 && all_jars {
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
async fn monitor(
    app: &AppHandle,
    state: &Arc<AppState>,
    id: &str,
    mut child: tokio::process::Child,
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    kill_rx: tokio::sync::oneshot::Receiver<()>,
    clicked: Instant,
    token: &str,
    cds: bool,
    launch_log: &Path,
) -> anyhow::Result<()> {
    let paths = &state.paths;
    let mut log_file = std::fs::OpenOptions::new().append(true).create(true).open(launch_log).ok();
    let mut got_output = false;
    let mut warned_silent = false;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    crate::core::spawn_line_reader(stdout, tx.clone());
    crate::core::spawn_line_reader(stderr, tx.clone());
    drop(tx);

    let connect_re = regex::Regex::new(r"Connecting to ([^\s,]+), (\d+)").unwrap();
    let mut boot_ms: Option<u64> = None;
    let mut batch: Vec<String> = vec![];
    let mut last_flush = Instant::now();
    let mut killed = false;
    let mut kill_rx = kill_rx;
    let started = Instant::now();
    state.logs.lock().unwrap().insert(id.to_string(), std::collections::VecDeque::new());

    let status = loop {
        tokio::select! {
            line = rx.recv() => {
                match line {
                    Some(mut line) => {
                        if !token.is_empty() && token != "0" && line.contains(token) {
                            line = line.replace(token, "<access token hidden>");
                        }
                        got_output = true;
                        if let Some(f) = log_file.as_mut() {
                            use std::io::Write;
                            let _ = writeln!(f, "{line}");
                        }
                        if boot_ms.is_none() && BOOT_MARKERS.iter().any(|m| line.contains(m)) {
                            let ms = clicked.elapsed().as_millis() as u64;
                            boot_ms = Some(ms);
                            let _ = app.emit("game-boot", serde_json::json!({ "instanceId": id, "bootMs": ms }));
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
                        // Streams closed: the process is exiting.
                        if !batch.is_empty() {
                            let _ = app.emit("game-log", GameLog { instance_id: id.into(), lines: std::mem::take(&mut batch) });
                        }
                        break child.wait().await?;
                    }
                }
            }
            _ = &mut kill_rx, if !killed => {
                killed = true;
                let _ = child.start_kill();
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
                    let _ = app.emit("game-boot", serde_json::json!({ "instanceId": id, "bootMs": ms }));
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)), if !batch.is_empty() => {
                let _ = app.emit("game-log", GameLog { instance_id: id.into(), lines: std::mem::take(&mut batch) });
                last_flush = Instant::now();
            }
        }
    };

    if let Some(f) = log_file.as_mut() {
        use std::io::Write;
        let _ = writeln!(f, "--- process exited: {:?} after {}s", status.code(), started.elapsed().as_secs());
    }
    let played = started.elapsed().as_secs();
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

    let crash = if !killed && code.map(|c| c != 0).unwrap_or(true) {
        let log_text = state.logs.lock().unwrap().get(id).map(|b| b.iter().cloned().collect::<Vec<_>>().join("\n")).unwrap_or_default();
        let crash_file = newest_crash_report(&instances::game_dir(paths, id), started);
        let crash_text = crash_file.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        Some(analyze(&format!("{log_text}\n{crash_text}"), code).with_crash_file(crash_file))
    } else {
        None
    };

    if let Some(cmd) = inst.post_exit.as_deref().filter(|c| !c.trim().is_empty()) {
        let _ = run_hook(cmd, &instances::game_dir(paths, id), &inst).await;
    }
    state.discord.clear();
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
    }
    let _ = app.emit("game-exit", GameExit { instance_id: id.into(), code, played_secs: played, boot_ms, crash, killed });
    emit_state(app, id, "stopped", "Game closed");
    Ok(())
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
        let args = build_args(&paths, &inst, &prepared, &auth, &game_dir, 1024, 21, Some("mc.hypixel.net"), &f);

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
        let a2 = build_args(&paths, &inst, &p2, &auth, &game_dir, 1024, 21, None, &Features::default());
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
        let args = build_args(&paths, &inst, &prepared, &auth, &tmp, 2048, 8, Some("play.example.com:25570"), &Features::default());
        assert!(args.contains(&"-cp".to_string()));
        assert!(!args.iter().any(|a| a.contains("AutoCreateSharedArchive")), "no CDS on Java 8");
        let i = args.iter().position(|a| a == "--server").unwrap();
        assert_eq!(args[i + 1], "play.example.com");
        assert_eq!(args[i + 3], "25570");
        assert!(args.iter().any(|a| a == "1.8"));
    }
}
