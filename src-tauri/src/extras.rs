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
    option_env!("HALO_DOWNLOAD_URL").filter(|s| s.starts_with("https://")).unwrap_or("https://loansharkz.github.io/haloclient/")
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
