//! Servers: native Server List Ping (favicon, MOTD, players, latency),
//! per-instance servers.dat, and the "recently played" history used by
//! Quick join.

use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::core::{now_ms, read_json, write_json, Paths};

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub online: bool,
    pub motd: Vec<String>, // lines with § formatting codes
    pub players_online: u64,
    pub players_max: u64,
    pub favicon: Option<String>, // data:image/png;base64,...
    pub version: Option<String>,
    pub ping_ms: Option<u64>,
}

// --- protocol helpers -------------------------------------------------------

fn write_varint(buf: &mut Vec<u8>, mut v: i32) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v = ((v as u32) >> 7) as i32;
        if v != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if v == 0 {
            break;
        }
    }
}

async fn read_varint(s: &mut TcpStream) -> anyhow::Result<i32> {
    let mut result = 0i32;
    for i in 0..5 {
        let b = s.read_u8().await?;
        result |= ((b & 0x7f) as i32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(result);
        }
    }
    bail!("VarInt too long")
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = vec![];
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = vec![];
    write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

/// Resolves _minecraft._tcp SRV records like the game does.
async fn resolve_srv(host: &str) -> Option<(String, u16)> {
    use hickory_resolver::TokioAsyncResolver;
    let resolver = TokioAsyncResolver::tokio_from_system_conf()
        .unwrap_or_else(|_| TokioAsyncResolver::tokio(Default::default(), Default::default()));
    let lookup = tokio::time::timeout(Duration::from_secs(3), resolver.srv_lookup(format!("_minecraft._tcp.{host}"))).await.ok()?.ok()?;
    let r = lookup.iter().next()?;
    Some((r.target().to_utf8().trim_end_matches('.').to_string(), r.port()))
}

pub async fn ping(address: &str) -> anyhow::Result<ServerStatus> {
    let (host, port) = crate::launch::split_address(address);
    let explicit_port = address.trim().rsplit_once(':').map(|(h, _)| !h.contains(':') || address.contains(']')).unwrap_or(false);
    let (connect_host, connect_port) = if !explicit_port && host.parse::<std::net::IpAddr>().is_err() {
        resolve_srv(&host).await.unwrap_or((host.clone(), port))
    } else {
        (host.clone(), port)
    };

    let mut s = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect((connect_host.as_str(), connect_port)))
        .await
        .context("server didn't answer")??;
    s.set_nodelay(true).ok();

    // Handshake (next state = status)
    let mut hs = vec![];
    write_varint(&mut hs, -1); // protocol version: "any"
    write_varint(&mut hs, host.len() as i32);
    hs.extend_from_slice(host.as_bytes());
    hs.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut hs, 1);
    s.write_all(&packet(0x00, &hs)).await?;
    s.write_all(&packet(0x00, &[])).await?;

    let json_text = tokio::time::timeout(Duration::from_secs(5), async {
        let _len = read_varint(&mut s).await?;
        let id = read_varint(&mut s).await?;
        if id != 0x00 {
            bail!("unexpected packet {id}");
        }
        let n = read_varint(&mut s).await? as usize;
        if n > 2 * 1024 * 1024 {
            bail!("status too large");
        }
        let mut buf = vec![0u8; n];
        s.read_exact(&mut buf).await?;
        Ok(String::from_utf8_lossy(&buf).to_string())
    })
    .await
    .context("server status timed out")??;

    // Ping/pong for latency
    let t0 = Instant::now();
    let ping_ms = async {
        s.write_all(&packet(0x01, &42i64.to_be_bytes())).await.ok()?;
        tokio::time::timeout(Duration::from_secs(3), async {
            let _ = read_varint(&mut s).await.ok()?;
            let _ = read_varint(&mut s).await.ok()?;
            let _ = s.read_i64().await.ok()?;
            Some(())
        })
        .await
        .ok()??;
        Some(t0.elapsed().as_millis() as u64)
    }
    .await;

    let v: Value = serde_json::from_str(&json_text)?;
    Ok(ServerStatus {
        online: true,
        motd: chat_to_legacy(&v["description"]).lines().map(|l| l.to_string()).take(2).collect(),
        players_online: v["players"]["online"].as_u64().unwrap_or(0),
        players_max: v["players"]["max"].as_u64().unwrap_or(0),
        favicon: v["favicon"].as_str().filter(|f| f.starts_with("data:image/png;base64,")).map(|f| f.replace('\n', "")),
        version: v["version"]["name"].as_str().map(String::from),
        ping_ms,
    })
}

/// Converts a JSON chat component to a legacy "§"-formatted string.
pub fn chat_to_legacy(v: &Value) -> String {
    let mut out = String::new();
    walk(v, &mut out, &Style::default());
    out
}

#[derive(Default, Clone)]
struct Style {
    color: Option<char>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strike: bool,
    obf: bool,
}

fn color_code(name: &str) -> Option<char> {
    Some(match name {
        "black" => '0',
        "dark_blue" => '1',
        "dark_green" => '2',
        "dark_aqua" => '3',
        "dark_red" => '4',
        "dark_purple" => '5',
        "gold" => '6',
        "gray" => '7',
        "dark_gray" => '8',
        "blue" => '9',
        "green" => 'a',
        "aqua" => 'b',
        "red" => 'c',
        "light_purple" => 'd',
        "yellow" => 'e',
        "white" => 'f',
        hex if hex.starts_with('#') && hex.len() == 7 => nearest_color(hex)?,
        _ => return None,
    })
}

/// Maps a #rrggbb color to the nearest legacy color code.
fn nearest_color(hex: &str) -> Option<char> {
    let n = u32::from_str_radix(&hex[1..], 16).ok()?;
    let (r, g, b) = ((n >> 16) as i32, ((n >> 8) & 0xff) as i32, (n & 0xff) as i32);
    const PALETTE: [(char, i32, i32, i32); 16] = [
        ('0', 0, 0, 0), ('1', 0, 0, 170), ('2', 0, 170, 0), ('3', 0, 170, 170), ('4', 170, 0, 0), ('5', 170, 0, 170), ('6', 255, 170, 0), ('7', 170, 170, 170),
        ('8', 85, 85, 85), ('9', 85, 85, 255), ('a', 85, 255, 85), ('b', 85, 255, 255), ('c', 255, 85, 85), ('d', 255, 85, 255), ('e', 255, 255, 85), ('f', 255, 255, 255),
    ];
    PALETTE.iter().min_by_key(|(_, pr, pg, pb)| (pr - r).pow(2) + (pg - g).pow(2) + (pb - b).pow(2)).map(|p| p.0)
}

fn walk(v: &Value, out: &mut String, parent: &Style) {
    match v {
        Value::String(s) => out.push_str(s),
        Value::Array(a) => a.iter().for_each(|x| walk(x, out, parent)),
        Value::Object(o) => {
            let mut st = parent.clone();
            if let Some(c) = o.get("color").and_then(|c| c.as_str()).and_then(color_code) {
                st.color = Some(c);
            }
            let flag = |k: &str, cur: bool| o.get(k).and_then(|b| b.as_bool()).unwrap_or(cur);
            st.bold = flag("bold", st.bold);
            st.italic = flag("italic", st.italic);
            st.underlined = flag("underlined", st.underlined);
            st.strike = flag("strikethrough", st.strike);
            st.obf = flag("obfuscated", st.obf);
            let text = o.get("text").and_then(|t| t.as_str()).unwrap_or("");
            if !text.is_empty() {
                out.push_str("§r");
                if let Some(c) = st.color {
                    out.push('§');
                    out.push(c);
                }
                for (on, code) in [(st.obf, 'k'), (st.bold, 'l'), (st.strike, 'm'), (st.underlined, 'n'), (st.italic, 'o')] {
                    if on {
                        out.push('§');
                        out.push(code);
                    }
                }
                out.push_str(text);
            }
            if let Some(extra) = o.get("extra") {
                walk(extra, out, &st);
            }
        }
        _ => {}
    }
}

// --- servers.dat ------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerEntry {
    #[serde(default)]
    pub name: String,
    pub ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(rename = "acceptTextures", default, skip_serializing_if = "Option::is_none")]
    pub accept_textures: Option<bool>,
}

#[derive(Serialize, Deserialize, Default)]
struct ServersDat {
    #[serde(default)]
    servers: Vec<ServerEntry>,
}

pub fn read_servers_dat(paths: &Paths, instance_id: &str) -> Vec<ServerEntry> {
    let p = crate::instances::game_dir(paths, instance_id).join("servers.dat");
    std::fs::read(p).ok().and_then(|b| fastnbt::from_bytes::<ServersDat>(&b).ok()).map(|d| d.servers).unwrap_or_default()
}

pub fn write_servers_dat(paths: &Paths, instance_id: &str, servers: Vec<ServerEntry>) -> anyhow::Result<()> {
    let p = crate::instances::game_dir(paths, instance_id).join("servers.dat");
    let bytes = fastnbt::to_bytes(&ServersDat { servers })?;
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, bytes)?;
    Ok(())
}

// --- recently played ----------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecentServer {
    pub address: String,
    pub name: String,
    pub instance_id: String,
    pub last_played: i64,
    pub joins: u32,
}

pub fn recent(paths: &Paths) -> Vec<RecentServer> {
    let mut v: Vec<RecentServer> = read_json(&paths.recent_servers_file()).unwrap_or_default();
    // Drop entries whose instance was deleted.
    v.retain(|r| paths.instance(&r.instance_id).exists());
    v.sort_by_key(|r| std::cmp::Reverse(r.last_played));
    v
}

pub fn record_recent(paths: &Paths, address: &str, instance_id: &str) -> anyhow::Result<()> {
    let mut v: Vec<RecentServer> = read_json(&paths.recent_servers_file()).unwrap_or_default();
    let key = address.to_lowercase();
    // Prefer the name the player gave it in their server list.
    let name = read_servers_dat(paths, instance_id)
        .into_iter()
        .find(|s| s.ip.to_lowercase() == key || s.ip.to_lowercase() == format!("{key}:25565"))
        .map(|s| s.name)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| address.to_string());
    match v.iter_mut().find(|r| r.address.to_lowercase() == key) {
        Some(r) => {
            r.last_played = now_ms();
            r.instance_id = instance_id.into();
            r.joins += 1;
            r.name = name;
        }
        None => v.push(RecentServer { address: address.into(), name, instance_id: instance_id.into(), last_played: now_ms(), joins: 1 }),
    }
    v.sort_by_key(|r| std::cmp::Reverse(r.last_played));
    v.truncate(20);
    write_json(&paths.recent_servers_file(), &v)
}

pub fn forget_recent(paths: &Paths, address: &str) -> anyhow::Result<()> {
    let mut v: Vec<RecentServer> = read_json(&paths.recent_servers_file()).unwrap_or_default();
    v.retain(|r| !r.address.eq_ignore_ascii_case(address));
    write_json(&paths.recent_servers_file(), &v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn varints() {
        let mut b = vec![];
        write_varint(&mut b, 300);
        assert_eq!(b, vec![0xac, 0x02]);
        let mut b = vec![];
        write_varint(&mut b, -1);
        assert_eq!(b, vec![0xff, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn chat_components() {
        let v = json!({"text":"","extra":[{"text":"Hypixel ","color":"green"},{"text":"Network","color":"red","bold":true}]});
        assert_eq!(chat_to_legacy(&v), "§r§aHypixel §r§c§lNetwork");
        assert_eq!(chat_to_legacy(&json!("§aplain")), "§aplain");
        assert_eq!(color_code("#ff5555"), Some('c'));
    }

    #[test]
    fn servers_dat_roundtrip() {
        let d = ServersDat { servers: vec![ServerEntry { name: "Hypixel".into(), ip: "mc.hypixel.net".into(), icon: None, accept_textures: None }] };
        let bytes = fastnbt::to_bytes(&d).unwrap();
        let back: ServersDat = fastnbt::from_bytes(&bytes).unwrap();
        assert_eq!(back.servers[0].ip, "mc.hypixel.net");
    }

    /// Full protocol round-trip against an in-process fake server.
    #[tokio::test]
    async fn ping_fake_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            // handshake + status request
            for _ in 0..2 {
                let len = read_varint(&mut s).await.unwrap();
                let mut buf = vec![0u8; len as usize];
                s.read_exact(&mut buf).await.unwrap();
            }
            let body = r#"{"description":{"text":"Hi","color":"gold"},"players":{"online":5,"max":20},"version":{"name":"1.21.1"}}"#;
            let mut payload = vec![];
            write_varint(&mut payload, body.len() as i32);
            payload.extend_from_slice(body.as_bytes());
            s.write_all(&packet(0, &payload)).await.unwrap();
            // ping → pong
            let len = read_varint(&mut s).await.unwrap();
            let mut buf = vec![0u8; len as usize];
            s.read_exact(&mut buf).await.unwrap();
            s.write_all(&packet(1, &buf[1..])).await.unwrap();
        });
        let st = ping(&format!("127.0.0.1:{port}")).await.unwrap();
        assert!(st.online);
        assert_eq!(st.players_online, 5);
        assert_eq!(st.motd, vec!["§r§6Hi".to_string()]);
        assert!(st.ping_ms.is_some());
    }
}
