//! Accounts: Microsoft device-code login → Xbox Live → XSTS → Minecraft,
//! silent refresh, ownership check, offline profiles, skins and capes.
//!
//! Tokens are stored in the OS keychain (Windows Credential Manager) and are
//! never logged or sent to the frontend. Only display metadata lives in
//! accounts.json.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;

use crate::core::{now_ms, read_json, write_json, Paths};

const KEYRING_SERVICE: &str = "HaloClient";
const SCOPE: &str = "XboxLive.signin offline_access";
const MSA_DEVICE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MSA_TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const MC_API: &str = "https://api.minecraftservices.com";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Cape {
    pub id: String,
    pub alias: String,
    pub url: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub kind: String, // "microsoft" | "offline"
    pub username: String,
    pub uuid: String,
    #[serde(default)]
    pub skin_url: Option<String>,
    #[serde(default)]
    pub skin_variant: Option<String>,
    #[serde(default)]
    pub capes: Vec<Cape>,
    /// Minecraft token expiry (ms since epoch). Tokens themselves are in the keychain.
    #[serde(default)]
    pub expires_at: i64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
}

impl AccountStore {
    pub fn load(paths: &Paths) -> Self {
        read_json(&paths.accounts_file()).unwrap_or_default()
    }
    pub fn save(&self, paths: &Paths) -> anyhow::Result<()> {
        write_json(&paths.accounts_file(), self)
    }
    pub fn upsert(&mut self, a: Account) {
        match self.accounts.iter_mut().find(|x| x.id == a.id) {
            Some(x) => *x = a,
            None => self.accounts.push(a),
        }
    }
}

// ---------------------------------------------------------------------------
// Secret storage
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
struct Secrets {
    refresh_token: String,
    mc_token: String,
}

/// Each secret gets its own keychain entry: Windows Credential Manager caps a
/// single blob at 2560 bytes, and MS refresh tokens can be long.
fn secret_entry(account_id: &str, which: &str) -> anyhow::Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, &format!("{account_id}:{which}"))?)
}

fn fallback_secret_file(paths: &Paths, account_id: &str) -> std::path::PathBuf {
    paths.root.join("secrets").join(format!("{account_id}.json"))
}

fn store_secrets(paths: &Paths, account_id: &str, s: &Secrets) -> anyhow::Result<()> {
    let ok = (|| -> anyhow::Result<()> {
        secret_entry(account_id, "refresh")?.set_secret(s.refresh_token.as_bytes())?;
        secret_entry(account_id, "mc")?.set_secret(s.mc_token.as_bytes())?;
        Ok(())
    })();
    if let Err(e) = ok {
        // Some Linux setups have no secret service. Windows/macOS always do.
        if cfg!(target_os = "windows") {
            return Err(e.context("couldn't save login to Windows Credential Manager"));
        }
        log::warn!("keychain unavailable, using a private file instead: {e:#}");
        let f = fallback_secret_file(paths, account_id);
        write_json(&f, s)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600));
        }
    }
    Ok(())
}

fn load_secrets(paths: &Paths, account_id: &str) -> anyhow::Result<Secrets> {
    let from_keyring = (|| -> anyhow::Result<Secrets> {
        let refresh = String::from_utf8(secret_entry(account_id, "refresh")?.get_secret()?)?;
        let mc = String::from_utf8(secret_entry(account_id, "mc")?.get_secret()?)?;
        Ok(Secrets { refresh_token: refresh, mc_token: mc })
    })();
    from_keyring.or_else(|_| read_json(&fallback_secret_file(paths, account_id))).context("saved login not found; sign in again")
}

fn delete_secrets(paths: &Paths, account_id: &str) {
    for which in ["refresh", "mc"] {
        if let Ok(e) = secret_entry(account_id, which) {
            let _ = e.delete_credential();
        }
    }
    let _ = std::fs::remove_file(fallback_secret_file(paths, account_id));
}

// ---------------------------------------------------------------------------
// Microsoft device-code flow
// ---------------------------------------------------------------------------

pub fn client_id(settings_override: Option<&str>) -> anyhow::Result<String> {
    settings_override
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .or_else(|| std::env::var("HALO_MSA_CLIENT_ID").ok().filter(|s| !s.is_empty()))
        .or_else(|| option_env!("HALO_MSA_CLIENT_ID").map(String::from).filter(|s| !s.is_empty()))
        .ok_or_else(|| anyhow!("Microsoft login isn't set up in this build. Set HALO_MSA_CLIENT_ID (see README → Azure app registration)."))
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    #[serde(alias = "user_code")]
    pub user_code: String,
    #[serde(alias = "verification_uri")]
    pub verification_uri: String,
    #[serde(alias = "expires_in")]
    pub expires_in: u64,
    #[serde(default = "five", alias = "interval")]
    pub interval: u64,
    #[serde(skip_serializing, alias = "device_code")]
    pub device_code: String,
    #[serde(alias = "message", default)]
    pub message: String,
}
fn five() -> u64 {
    5
}

/// In-flight device-code logins, keyed by flow id. Cancelling drops the entry.
pub type PendingFlows = Arc<Mutex<HashMap<String, DeviceCode>>>;

pub async fn start_device_flow(http: &reqwest::Client, client_id: &str) -> anyhow::Result<DeviceCode> {
    let r = http.post(MSA_DEVICE).form(&[("client_id", client_id), ("scope", SCOPE)]).send().await?;
    if !r.status().is_success() {
        let body = r.text().await.unwrap_or_default();
        bail!("Microsoft refused the login request ({}). Check the Azure app has 'Allow public client flows' on.", short(&body));
    }
    Ok(r.json().await?)
}

#[derive(Deserialize)]
struct MsaToken {
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct MsaError {
    error: String,
}

/// Polls until the user finishes signing in, the code expires, or the flow is cancelled.
pub async fn poll_device_flow(http: &reqwest::Client, client_id: &str, flow_id: &str, flows: &PendingFlows) -> anyhow::Result<(String, String)> {
    let dc = flows.lock().await.get(flow_id).cloned().context("login was cancelled")?;
    let mut interval = dc.interval.max(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(dc.expires_in);
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        if !flows.lock().await.contains_key(flow_id) {
            bail!("login was cancelled");
        }
        if std::time::Instant::now() > deadline {
            bail!("the sign-in code expired; try again");
        }
        let r = http
            .post(MSA_TOKEN)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", dc.device_code.as_str()),
            ])
            .send()
            .await?;
        if r.status().is_success() {
            let t: MsaToken = r.json().await?;
            flows.lock().await.remove(flow_id);
            return Ok((t.access_token, t.refresh_token));
        }
        let err: MsaError = r.json().await.unwrap_or(MsaError { error: "unknown".into() });
        match err.error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => interval += 5,
            "authorization_declined" => bail!("sign-in was declined"),
            "expired_token" => bail!("the sign-in code expired; try again"),
            other => bail!("Microsoft sign-in failed: {other}"),
        }
    }
}

async fn refresh_msa(http: &reqwest::Client, client_id: &str, refresh: &str) -> anyhow::Result<(String, String)> {
    let r = http
        .post(MSA_TOKEN)
        .form(&[("grant_type", "refresh_token"), ("client_id", client_id), ("refresh_token", refresh), ("scope", SCOPE)])
        .send()
        .await?;
    if !r.status().is_success() {
        bail!("your Microsoft login expired; sign in again");
    }
    let t: MsaToken = r.json().await?;
    Ok((t.access_token, t.refresh_token))
}

// ---------------------------------------------------------------------------
// Xbox → Minecraft
// ---------------------------------------------------------------------------

async fn xbox_to_minecraft(http: &reqwest::Client, msa_access: &str) -> anyhow::Result<(String, i64)> {
    let xbl: serde_json::Value = http
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={msa_access}")},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()
        .context("Xbox Live sign-in failed")?
        .json()
        .await?;
    let xbl_token = xbl["Token"].as_str().context("no Xbox token")?;
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().context("no Xbox user hash")?;

    let xsts = http
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;
    if xsts.status().as_u16() == 401 {
        let v: serde_json::Value = xsts.json().await.unwrap_or_default();
        let code = v["XErr"].as_u64().unwrap_or(0);
        bail!(match code {
            2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at minecraft.net, then try again.",
            2148916235 => "Xbox Live isn't available in this account's country.",
            2148916236 | 2148916237 => "This account needs adult verification on xbox.com first.",
            2148916238 => "This is a child account. An adult needs to add it to a Microsoft Family group first.",
            _ => "Xbox Live refused the sign-in (XSTS).",
        });
    }
    let xsts: serde_json::Value = xsts.error_for_status()?.json().await?;
    let xsts_token = xsts["Token"].as_str().context("no XSTS token")?;

    let mc = http
        .post(format!("{MC_API}/authentication/login_with_xbox"))
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }))
        .send()
        .await?;
    if mc.status().as_u16() == 403 {
        bail!("Minecraft refused the login. If this is a new Azure app, Mojang still has to approve it (README → step 6).");
    }
    let mc: serde_json::Value = mc.error_for_status().context("Minecraft sign-in failed")?.json().await?;
    let token = mc["access_token"].as_str().context("no Minecraft token")?.to_string();
    let expires_in = mc["expires_in"].as_i64().unwrap_or(86400);
    Ok((token, now_ms() + expires_in * 1000))
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<ProfileSkin>,
    #[serde(default)]
    capes: Vec<ProfileCape>,
}
#[derive(Deserialize)]
struct ProfileSkin {
    url: String,
    state: String,
    variant: Option<String>,
}
#[derive(Deserialize)]
struct ProfileCape {
    id: String,
    url: String,
    state: String,
    alias: String,
}

async fn fetch_profile(http: &reqwest::Client, mc_token: &str) -> anyhow::Result<Profile> {
    let r = http.get(format!("{MC_API}/minecraft/profile")).bearer_auth(mc_token).send().await?;
    if r.status().as_u16() == 404 {
        bail!("This account doesn't own Minecraft: Java Edition (or hasn't picked a username yet at minecraft.net).");
    }
    Ok(r.error_for_status()?.json().await?)
}

fn account_from_profile(p: Profile, expires_at: i64) -> Account {
    let skin = p.skins.iter().find(|s| s.state == "ACTIVE");
    Account {
        id: p.id.clone(),
        kind: "microsoft".into(),
        username: p.name,
        uuid: p.id,
        skin_url: skin.map(|s| s.url.clone()),
        skin_variant: skin.and_then(|s| s.variant.clone()).map(|v| v.to_lowercase()),
        capes: p.capes.into_iter().map(|c| Cape { id: c.id, alias: c.alias, url: c.url, active: c.state == "ACTIVE" }).collect(),
        expires_at,
    }
}

/// Completes a login from MSA tokens: Xbox chain, ownership check, save.
pub async fn finish_login(http: &reqwest::Client, paths: &Paths, msa_access: &str, msa_refresh: &str) -> anyhow::Result<Account> {
    let (mc_token, expires_at) = xbox_to_minecraft(http, msa_access).await?;
    let profile = fetch_profile(http, &mc_token).await?;
    let account = account_from_profile(profile, expires_at);
    store_secrets(paths, &account.id, &Secrets { refresh_token: msa_refresh.into(), mc_token })?;
    Ok(account)
}

/// Credentials passed to the game.
pub struct LaunchAuth {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: &'static str,
}

/// Returns a valid token, refreshing silently when it expires within 10 minutes.
pub async fn ensure_fresh(http: &reqwest::Client, paths: &Paths, client_id: Option<&str>, account: &mut Account) -> anyhow::Result<LaunchAuth> {
    if account.kind == "offline" {
        return Ok(LaunchAuth { username: account.username.clone(), uuid: account.uuid.clone(), access_token: "0".into(), user_type: "legacy" });
    }
    let mut secrets = load_secrets(paths, &account.id)?;
    if account.expires_at - now_ms() < 10 * 60 * 1000 {
        let cid = client_id_or(client_id)?;
        let (access, refresh) = refresh_msa(http, &cid, &secrets.refresh_token).await?;
        let (mc_token, expires_at) = xbox_to_minecraft(http, &access).await?;
        let profile = fetch_profile(http, &mc_token).await?;
        *account = account_from_profile(profile, expires_at);
        secrets = Secrets { refresh_token: refresh, mc_token };
        store_secrets(paths, &account.id, &secrets)?;
    }
    Ok(LaunchAuth { username: account.username.clone(), uuid: account.uuid.clone(), access_token: secrets.mc_token, user_type: "msa" })
}

fn client_id_or(o: Option<&str>) -> anyhow::Result<String> {
    client_id(o)
}

pub fn remove(paths: &Paths, account_id: &str) {
    delete_secrets(paths, account_id);
}

// ---------------------------------------------------------------------------
// Offline profiles
// ---------------------------------------------------------------------------

/// Same UUID vanilla servers derive for offline players: v3 MD5 of "OfflinePlayer:<name>".
pub fn offline_uuid(name: &str) -> String {
    use md5::Digest;
    let mut h = md5::Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    hex::encode(h)
}

pub fn valid_username(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn offline_account(name: &str) -> anyhow::Result<Account> {
    if !valid_username(name) {
        bail!("Usernames are 3–16 characters: letters, numbers and _.");
    }
    Ok(Account {
        id: format!("offline-{}", name.to_lowercase()),
        kind: "offline".into(),
        username: name.into(),
        uuid: offline_uuid(name),
        skin_url: None,
        skin_variant: None,
        capes: vec![],
        expires_at: i64::MAX,
    })
}

// ---------------------------------------------------------------------------
// Skins & capes (official Minecraft Services API)
// ---------------------------------------------------------------------------

pub async fn upload_skin(http: &reqwest::Client, paths: &Paths, client_id: Option<&str>, account: &mut Account, png: Vec<u8>, slim: bool) -> anyhow::Result<()> {
    if png.len() > 24 * 1024 || !png.starts_with(b"\x89PNG") {
        bail!("Skins must be a PNG file (64×64 or 64×32).");
    }
    let auth = ensure_fresh(http, paths, client_id, account).await?;
    let form = reqwest::multipart::Form::new()
        .text("variant", if slim { "slim" } else { "classic" })
        .part("file", reqwest::multipart::Part::bytes(png).file_name("skin.png").mime_str("image/png")?);
    http.post(format!("{MC_API}/minecraft/profile/skins"))
        .bearer_auth(&auth.access_token)
        .multipart(form)
        .send()
        .await?
        .error_for_status()
        .context("Mojang rejected the skin")?;
    refresh_profile(http, account, &auth.access_token).await
}

pub async fn set_cape(http: &reqwest::Client, paths: &Paths, client_id: Option<&str>, account: &mut Account, cape_id: Option<&str>) -> anyhow::Result<()> {
    let auth = ensure_fresh(http, paths, client_id, account).await?;
    let url = format!("{MC_API}/minecraft/profile/capes/active");
    let req = match cape_id {
        Some(id) => http.put(&url).json(&json!({ "capeId": id })),
        None => http.delete(&url),
    };
    req.bearer_auth(&auth.access_token).send().await?.error_for_status().context("couldn't change cape")?;
    refresh_profile(http, account, &auth.access_token).await
}

async fn refresh_profile(http: &reqwest::Client, account: &mut Account, token: &str) -> anyhow::Result<()> {
    let p = fetch_profile(http, token).await?;
    let exp = account.expires_at;
    *account = account_from_profile(p, exp);
    Ok(())
}

/// Downloads a texture (skin/cape) once and returns it as a data URI so the UI
/// never needs network access to textures.minecraft.net.
pub async fn texture_data_uri(http: &reqwest::Client, paths: &Paths, url: &str) -> anyhow::Result<String> {
    use base64::Engine;
    if !url.starts_with("https://textures.minecraft.net/") && !url.starts_with("http://textures.minecraft.net/") {
        bail!("unexpected texture host");
    }
    let name = url.rsplit('/').next().unwrap_or("tex");
    let file = paths.cache().join("textures").join(format!("{name}.png"));
    let bytes = match std::fs::read(&file) {
        Ok(b) => b,
        Err(_) => {
            let b = http.get(url.replacen("http://", "https://", 1)).send().await?.error_for_status()?.bytes().await?.to_vec();
            std::fs::create_dir_all(file.parent().unwrap())?;
            std::fs::write(&file, &b)?;
            b
        }
    };
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn short(s: &str) -> String {
    s.chars().take(160).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_uuid_matches_vanilla() {
        // UUID.nameUUIDFromBytes("OfflinePlayer:Notch") = b50ad385-829d-3141-a216-7e7d7539ba7f
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }
    #[test]
    fn usernames() {
        assert!(valid_username("Leo_123"));
        assert!(!valid_username("ab"));
        assert!(!valid_username("bad name"));
        assert!(!valid_username("waytoolongusername1"));
    }
}
