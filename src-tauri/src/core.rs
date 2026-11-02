//! Shared infrastructure: errors, paths, settings, HTTP client, events, hashing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha1::Digest;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncReadExt;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const USER_AGENT: &str = concat!("loansharkz/HaloClient/", env!("CARGO_PKG_VERSION"), " (github.com/loansharkz/haloclient)");

// ---------------------------------------------------------------------------
// Errors: commands return `CmdResult<T>`, which serializes the error as a
// plain message for the UI. Internals use anyhow.
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct CmdError(pub String);

impl Serialize for CmdError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<E: Into<anyhow::Error>> From<E> for CmdError {
    fn from(e: E) -> Self {
        let e: anyhow::Error = e.into();
        // `{:#}` includes the context chain on one line.
        CmdError(format!("{e:#}"))
    }
}

pub type CmdResult<T> = Result<T, CmdError>;

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Launcher data layout. `root` mirrors a vanilla `.minecraft` layout
/// (versions/, libraries/, assets/) so official Forge/NeoForge installers can
/// target it directly.
#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let root = custom_root().unwrap_or_else(default_root);
        let p = Paths { root };
        for d in [p.instances(), p.libraries(), p.assets(), p.versions(), p.java(), p.cache(), p.meta(), p.logs()] {
            let _ = std::fs::create_dir_all(d);
        }
        p
    }
    pub fn instances(&self) -> PathBuf { self.root.join("instances") }
    pub fn instance(&self, id: &str) -> PathBuf { self.instances().join(id) }
    pub fn libraries(&self) -> PathBuf { self.root.join("libraries") }
    pub fn assets(&self) -> PathBuf { self.root.join("assets") }
    pub fn versions(&self) -> PathBuf { self.root.join("versions") }
    pub fn java(&self) -> PathBuf { self.root.join("java") }
    pub fn cache(&self) -> PathBuf { self.root.join("cache") }
    pub fn meta(&self) -> PathBuf { self.root.join("meta") }
    pub fn logs(&self) -> PathBuf { self.root.join("logs") }
    pub fn natives(&self, version_id: &str) -> PathBuf { self.root.join("natives").join(version_id) }
    pub fn settings_file(&self) -> PathBuf { self.root.join("settings.json") }
    pub fn accounts_file(&self) -> PathBuf { self.root.join("accounts.json") }
    pub fn recent_servers_file(&self) -> PathBuf { self.root.join("recent_servers.json") }
}

fn default_root() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("HaloClient")
}

/// Pointer file that lets the user move the data folder (Settings → Data folder).
fn root_pointer() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("HaloClient").join("data-location.txt"))
}

fn custom_root() -> Option<PathBuf> {
    let s = std::fs::read_to_string(root_pointer()?).ok()?;
    let p = PathBuf::from(s.trim());
    p.is_absolute().then_some(p)
}

pub fn set_custom_root(path: Option<&Path>) -> anyhow::Result<()> {
    let ptr = root_pointer().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    match path {
        Some(p) => {
            std::fs::create_dir_all(ptr.parent().unwrap())?;
            std::fs::write(ptr, p.to_string_lossy().as_bytes())?;
        }
        None => {
            let _ = std::fs::remove_file(ptr);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings (mirrors the frontend `Settings` type)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,
    pub accent_hue: u32,
    pub animated_background: bool,
    pub default_ram_mb: u32,
    pub download_threads: u32,
    pub close_on_launch: bool,
    pub discord_rpc: bool,
    pub crash_reports: bool,
    pub crash_reports_prompted: bool,
    pub auto_update: bool,
    pub auto_java: bool,
    pub preflight: bool,
    pub show_reserved_slots: bool,
    pub browse_view: String,
    pub active_account_id: Option<String>,
    pub last_instance_id: Option<String>,
    /// Advanced: lets a dev point the launcher at their own Azure app at runtime.
    pub msa_client_id: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "dark".into(),
            accent_hue: 275,
            animated_background: true,
            default_ram_mb: 4096,
            download_threads: 12,
            close_on_launch: false,
            discord_rpc: true,
            crash_reports: true,
            crash_reports_prompted: false,
            auto_update: true,
            auto_java: true,
            preflight: true,
            show_reserved_slots: false,
            browse_view: "list".into(),
            active_account_id: None,
            last_instance_id: None,
            msa_client_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// JSON file helpers (atomic writes so a crash never leaves half a file)
// ---------------------------------------------------------------------------

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(300))
        .pool_max_idle_per_host(16)
        .build()
        .expect("http client")
}

/// GET JSON with a small retry loop for flaky networks.
pub async fn get_json<T: serde::de::DeserializeOwned>(http: &reqwest::Client, url: &str) -> anyhow::Result<T> {
    let mut last = None;
    for attempt in 0..3 {
        match http.get(url).send().await.and_then(|r| r.error_for_status()) {
            Ok(r) => return Ok(r.json::<T>().await?),
            Err(e) => {
                // Don't retry client errors (404 etc.)
                if e.status().map(|s| s.is_client_error()).unwrap_or(false) {
                    return Err(anyhow::anyhow!("{url}: {e}"));
                }
                last = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(400 * (attempt + 1))).await;
            }
        }
    }
    Err(anyhow::anyhow!("{url}: {}", last.unwrap()))
}

/// GET JSON, caching the body under meta/ so the launcher works offline and
/// opens instantly. `max_age_secs` = how long the cache is considered fresh.
pub async fn get_json_cached<T: serde::de::DeserializeOwned + Serialize>(
    http: &reqwest::Client,
    paths: &Paths,
    url: &str,
    cache_name: &str,
    max_age_secs: u64,
) -> anyhow::Result<T> {
    let file = paths.meta().join(cache_name);
    let fresh = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().map(|e| e.as_secs() < max_age_secs).unwrap_or(false))
        .unwrap_or(false);
    if fresh {
        if let Ok(v) = read_json::<T>(&file) {
            return Ok(v);
        }
    }
    match get_json::<T>(http, url).await {
        Ok(v) => {
            let _ = write_json(&file, &v);
            Ok(v)
        }
        Err(e) => read_json::<T>(&file).map_err(|_| e),
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub task: String,
    pub label: String,
    pub done: u64,
    pub total: u64,
}

pub fn emit_progress(app: &AppHandle, task: &str, label: &str, done: u64, total: u64) {
    let _ = app.emit("task-progress", TaskProgress { task: task.into(), label: label.into(), done, total });
}

// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------

pub async fn sha1_file(path: &Path) -> anyhow::Result<String> {
    hash_file::<sha1::Sha1>(path).await
}

pub async fn sha512_file(path: &Path) -> anyhow::Result<String> {
    hash_file::<sha2::Sha512>(path).await
}

pub async fn sha256_file(path: &Path) -> anyhow::Result<String> {
    hash_file::<sha2::Sha256>(path).await
}

async fn hash_file<D: Digest>(path: &Path) -> anyhow::Result<String> {
    let mut f = tokio::fs::File::open(path).await?;
    let mut h = D::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Joins `rel` under `base`, refusing absolute paths and `..` so archive
/// contents (modpacks, zips) can never write outside the target folder.
pub fn safe_join(base: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let rel = rel.replace('\\', "/");
    let mut out = base.to_path_buf();
    for part in rel.split('/') {
        match part {
            "" | "." => continue,
            ".." => anyhow::bail!("unsafe path in archive: {rel}"),
            p if p.contains(':') => anyhow::bail!("unsafe path in archive: {rel}"),
            p => out.push(p),
        }
    }
    if out == base {
        anyhow::bail!("empty path in archive");
    }
    Ok(out)
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub fn copy_dir(from: &Path, to: &Path) -> anyhow::Result<()> {
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(from)?;
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() {
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Forwards a child process's output line by line. Bytes are decoded
/// leniently: Java on Windows writes in the system code page, and a strict
/// UTF-8 reader would stop at the first non-UTF-8 character.
pub fn spawn_line_reader<R>(reader: R, tx: tokio::sync::mpsc::UnboundedSender<String>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    use tokio::io::AsyncBufReadExt;
    tokio::spawn(async move {
        let mut r = tokio::io::BufReader::new(reader);
        let mut buf = Vec::with_capacity(512);
        loop {
            buf.clear();
            match r.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// Streams new lines appended to `path` (starting at `offset`) until `done` is
/// set and everything written so far has been read. Decodes leniently.
pub fn spawn_file_tail(path: PathBuf, offset: u64, done: std::sync::Arc<std::sync::atomic::AtomicBool>, tx: tokio::sync::mpsc::UnboundedSender<String>) {
    use std::io::{Read, Seek, SeekFrom};
    tokio::task::spawn_blocking(move || {
        let Ok(mut f) = std::fs::File::open(&path) else { return };
        let _ = f.seek(SeekFrom::Start(offset));
        let mut pending: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            let finished = done.load(std::sync::atomic::Ordering::SeqCst);
            let n = f.read(&mut chunk).unwrap_or(0);
            if n > 0 {
                pending.extend_from_slice(&chunk[..n]);
                while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = pending.drain(..=pos).collect();
                    let text = String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_string();
                    if tx.send(text).is_err() {
                        return;
                    }
                }
                continue;
            }
            if finished {
                if !pending.is_empty() {
                    let _ = tx.send(String::from_utf8_lossy(&pending).trim_end().to_string());
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
    });
}

#[cfg(test)]
mod reader_tests {
    #[tokio::test]
    async fn tails_a_growing_file_until_done() {
        use std::io::Write;
        use std::sync::atomic::{AtomicBool, Ordering};
        let path = std::env::temp_dir().join(format!("halo-tail-{}.log", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"header\n").unwrap();
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        super::spawn_file_tail(path.clone(), 7, done.clone(), tx);
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"one\r\ntw").unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        f.write_all(b"o\nthr\xe9e").unwrap();
        done.store(true, Ordering::SeqCst);
        let mut got = vec![];
        while let Some(l) = rx.recv().await {
            got.push(l);
        }
        assert_eq!(got[0], "one");
        assert_eq!(got[1], "two");
        assert!(got[2].starts_with("thr"));
        assert_eq!(got.len(), 3);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn survives_non_utf8_output() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // "caf\xe9" is Windows-1252 for "café": invalid UTF-8.
        let data: &[u8] = b"first\r\ncaf\xe9 line\nlast";
        super::spawn_line_reader(data, tx);
        let mut got = vec![];
        while let Some(l) = rx.recv().await {
            got.push(l);
        }
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], "first");
        assert!(got[1].starts_with("caf"));
        assert_eq!(got[2], "last");
    }
}
