//! Discord Rich Presence, launcher update check, and crash reporting setup.

use std::sync::Mutex;

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use serde::Serialize;

use crate::instances::Instance;

/// Discord application id (create one at discord.com/developers and upload a
/// "halo" art asset). Without it, Rich Presence is off.
/// Halo Client's Discord application (public id, not a secret). Override with HALO_DISCORD_APP_ID.
const DEFAULT_DISCORD_APP_ID: &str = "1566610639223529514";

fn discord_app_id() -> Option<String> {
    std::env::var("HALO_DISCORD_APP_ID")
        .ok()
        .or_else(|| option_env!("HALO_DISCORD_APP_ID").map(String::from))
        .filter(|s| !s.is_empty())
        .or_else(|| Some(DEFAULT_DISCORD_APP_ID.to_string()))
}

/// Where the "Get Halo Client" button on Discord points.
fn download_url() -> &'static str {
    option_env!("HALO_DOWNLOAD_URL").filter(|s| s.starts_with("https://")).unwrap_or("https://loansharkz.github.io/HaloClient/")
}

#[derive(Default)]
pub struct Discord {
    client: Mutex<Option<DiscordIpcClient>>,
}

impl Discord {
    pub fn available() -> bool {
        discord_app_id().is_some()
    }

    pub fn playing(&self, inst: &Instance, server: Option<&str>) {
        let Some(app_id) = discord_app_id() else { return };
        let mut guard = self.client.lock().unwrap();
        if guard.is_none() {
            let Ok(mut c) = DiscordIpcClient::new(&app_id) else { return };
            if c.connect().is_err() {
                return; // Discord isn't running
            }
            *guard = Some(c);
        }
        let details = format!("Playing {}", inst.name);
        let loader = match inst.loader.as_str() {
            "fabric" => "Fabric",
            "quilt" => "Quilt",
            "forge" => "Forge",
            "neoforge" => "NeoForge",
            _ => "Vanilla",
        };
        let state = match server {
            Some(s) => format!("On {s}"),
            None => format!("{} · {loader}", inst.mc_version),
        };
        let start = chrono::Utc::now().timestamp();
        let act = activity::Activity::new()
            .details(&details)
            .state(&state)
            .timestamps(activity::Timestamps::new().start(start))
            .assets(activity::Assets::new().large_image("halo").large_text("Halo Client"))
            .buttons(vec![activity::Button::new("Get Halo Client", download_url())]);
        if let Some(c) = guard.as_mut() {
            if c.set_activity(act).is_err() {
                *guard = None;
            }
        }
    }

    pub fn clear(&self) {
        if let Some(c) = self.client.lock().unwrap().as_mut() {
            let _ = c.clear_activity();
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current: String,
    pub latest: Option<String>,
    pub available: bool,
    pub url: Option<String>,
}

/// Checks GitHub Releases for a newer launcher build.
pub async fn check_update(http: &reqwest::Client) -> anyhow::Result<UpdateCheck> {
    let current = crate::core::APP_VERSION.to_string();
    let r = http.get("https://api.github.com/repos/loansharkz/haloclient/releases/latest").header("Accept", "application/vnd.github+json").send().await?;
    if !r.status().is_success() {
        return Ok(UpdateCheck { current, latest: None, available: false, url: None });
    }
    let v: serde_json::Value = r.json().await?;
    let latest = v["tag_name"].as_str().map(|t| t.trim_start_matches('v').to_string());
    let available = latest.as_deref().map(|l| newer(l, &current)).unwrap_or(false);
    Ok(UpdateCheck { current, latest, available, url: v["html_url"].as_str().map(String::from) })
}

/// True when `a` is a higher dotted version than `b` (pre-release tags ignored).
pub fn newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| s.split(['-', '+']).next().unwrap_or("").split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (a, b) = (parse(a), parse(b));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Auto-update: download the newest installer in the background, verify its
// SHA-256 against the digest GitHub publishes, then install silently on restart.
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReady {
    pub version: String,
    pub path: String,
    pub notes_url: Option<String>,
}

const LATEST_RELEASE: &str = "https://api.github.com/repos/loansharkz/haloclient/releases/latest";

/// Returns a verified installer for a newer version, downloading it if needed.
pub async fn download_update(http: &reqwest::Client, paths: &crate::core::Paths) -> anyhow::Result<Option<UpdateReady>> {
    use anyhow::Context;
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;
    let r = http.get(LATEST_RELEASE).header("Accept", "application/vnd.github+json").send().await?;
    if !r.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = r.json().await?;
    let latest = v["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
    if latest.is_empty() || !newer(&latest, crate::core::APP_VERSION) {
        return Ok(None);
    }
    let asset = v["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str().map(|n| n.ends_with("-setup.exe")).unwrap_or(false)))
        .context("the latest release has no Windows installer")?;
    let name = asset["name"].as_str().unwrap_or("halo-setup.exe");
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        anyhow::bail!("unexpected installer name");
    }
    // Refuse to install anything GitHub didn't publish a checksum for.
    let expected = asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).context("release has no checksum")?.to_lowercase();
    let url = asset["browser_download_url"].as_str().context("no download url")?;
    if !url.starts_with("https://github.com/loansharkz/") {
        anyhow::bail!("unexpected download host");
    }
    let dir = paths.cache().join("updates");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(name);
    let ready = UpdateReady { version: latest, path: dest.to_string_lossy().into(), notes_url: v["html_url"].as_str().map(String::from) };
    if dest.exists() && crate::core::sha256_file(&dest).await.map(|h| h == expected).unwrap_or(false) {
        return Ok(Some(ready));
    }
    let part = dir.join(format!("{name}.part"));
    let resp = http.get(url).send().await?.error_for_status()?;
    let mut f = tokio::fs::File::create(&part).await?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        f.write_all(&chunk?).await?;
    }
    f.flush().await?;
    drop(f);
    if crate::core::sha256_file(&part).await? != expected {
        let _ = std::fs::remove_file(&part);
        anyhow::bail!("the downloaded update failed its checksum");
    }
    std::fs::rename(&part, &dest)?;
    Ok(Some(ready))
}

/// Runs the verified installer silently after Halo exits, then reopens Halo.
pub fn install_update(installer: &std::path::Path) -> anyhow::Result<()> {
    if !cfg!(windows) {
        anyhow::bail!("automatic install is only available on Windows");
    }
    let exe = std::env::current_exe()?;
    let script = installer.with_extension("cmd");
    // ping is a console-free way to wait ~2s for Halo to close.
    let body = format!(
        "@echo off\r\nping -n 3 127.0.0.1 >nul\r\n\"{}\" /S\r\nstart \"\" \"{}\"\r\n",
        installer.display(),
        exe.display()
    );
    std::fs::write(&script, body)?;
    let mut cmd = std::process::Command::new("cmd");
    cmd.arg("/C").arg(&script);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn()?;
    Ok(())
}

/// Starts Sentry when a DSN was compiled in and the user hasn't opted out.
/// Events never include user info, tokens or file paths outside the app.
pub fn init_sentry(enabled: bool) -> Option<sentry::ClientInitGuard> {
    let dsn = option_env!("HALO_SENTRY_DSN").filter(|s| !s.is_empty())?;
    if !enabled {
        return None;
    }
    Some(sentry::init((
        dsn,
        sentry::ClientOptions {
            release: Some(crate::core::APP_VERSION.into()),
            send_default_pii: false,
            before_send: Some(std::sync::Arc::new(|mut e| {
                e.user = None;
                e.server_name = None;
                let home = dirs::home_dir().map(|h| h.to_string_lossy().to_string());
                if let (Some(home), Some(msg)) = (home, e.message.as_mut()) {
                    *msg = msg.replace(&home, "~");
                }
                Some(e)
            })),
            ..Default::default()
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_compare() {
        assert!(newer("0.2.0", "0.1.9"));
        assert!(newer("1.0", "0.9.9"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0-beta", "0.1.0"));
    }
}
