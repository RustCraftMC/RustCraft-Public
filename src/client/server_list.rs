use crate::net;
use serde::{Deserialize, Serialize};
use std::io;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerEntry {
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub last_seen: Option<String>,
    #[serde(skip, default)]
    pub status: ServerStatus,
}

#[derive(Clone, Debug, Default)]
pub struct ServerStatus {
    pub online: bool,
    pub ping_ms: Option<u32>,
    pub version_name: Option<String>,
    pub protocol: Option<i32>,
    pub players_online: Option<u32>,
    pub players_max: Option<u32>,
    pub description: Option<String>,
    pub error: Option<String>,
    /// Raw base64 favicon data (server icon PNG, always in-memory, never persisted).
    pub favicon: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerList {
    pub servers: Vec<ServerEntry>,
}

impl ServerList {
    pub fn load_default() -> Self {
        Self::load(default_path()).unwrap_or_else(|| {
            let list = Self {
                servers: vec![ServerEntry {
                    name: "Local test server".to_string(),
                    address: "127.0.0.1:25565".to_string(),
                    last_seen: None,
                    status: ServerStatus::default(),
                }],
            };
            list.save_default();
            list
        })
    }

    pub fn load(path: impl AsRef<Path>) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save_default(&self) {
        let path = default_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn selected_address(&self, selected: usize) -> Option<&str> {
        self.servers
            .get(selected)
            .map(|server| server.address.as_str())
    }

    pub fn upsert(&mut self, name: impl Into<String>, address: impl Into<String>) -> usize {
        let name = name.into();
        let address = address.into();
        if let Some((idx, server)) = self
            .servers
            .iter_mut()
            .enumerate()
            .find(|(_, server)| server.address.eq_ignore_ascii_case(&address))
        {
            server.name = name;
            server.address = address;
            self.save_default();
            return idx;
        }

        self.servers.push(ServerEntry {
            name,
            address,
            last_seen: None,
            status: ServerStatus::default(),
        });
        self.save_default();
        self.servers.len() - 1
    }

    pub fn add(&mut self, name: impl Into<String>, address: impl Into<String>) -> usize {
        self.servers.push(ServerEntry {
            name: name.into(),
            address: address.into(),
            last_seen: None,
            status: ServerStatus::default(),
        });
        self.save_default();
        self.servers.len() - 1
    }

    pub fn update(
        &mut self,
        index: usize,
        name: impl Into<String>,
        address: impl Into<String>,
    ) -> bool {
        let Some(server) = self.servers.get_mut(index) else {
            return false;
        };
        server.name = name.into();
        server.address = address.into();
        self.save_default();
        true
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.servers.len() {
            return false;
        }
        self.servers.remove(index);
        self.save_default();
        true
    }

    pub fn rename(&mut self, index: usize, name: impl Into<String>) -> bool {
        let Some(server) = self.servers.get_mut(index) else {
            return false;
        };
        server.name = name.into();
        self.save_default();
        true
    }

    pub fn refresh_statuses(&mut self) {
        for server in &mut self.servers {
            server.status = query_status(&server.address).unwrap_or_else(|err| ServerStatus {
                online: false,
                error: Some(err.to_string()),
                ..ServerStatus::default()
            });
            if server.status.online {
                server.last_seen = Some("online".to_string());
            }
        }
        // Status is runtime data — never persisted to config.
    }
}

fn default_path() -> PathBuf {
    PathBuf::from("servers.json")
}

fn query_status(address: &str) -> io::Result<ServerStatus> {
    let (host, port) = parse_addr(address);
    let mut stream = TcpStream::connect((host, port))?;
    stream.set_read_timeout(Some(Duration::from_millis(800)))?;
    stream.set_write_timeout(Some(Duration::from_millis(800)))?;

    let handshake = net::packet::write_handshake(host, port, 1);
    net::protocol::write_packet(&mut stream, 0x00, &handshake)?;
    net::protocol::write_packet(&mut stream, 0x00, &[])?;

    let start = Instant::now();
    let (packet_id, payload) = net::protocol::read_packet(&mut stream)?;
    if packet_id != 0x00 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected status packet {}", packet_id),
        ));
    }
    let mut buf = net::protocol::PacketBuffer::new(payload);
    let json = buf.read_string()?;

    let mut ping = net::protocol::PacketBuffer::empty();
    ping.write_long(0);
    net::protocol::write_packet(&mut stream, 0x01, &ping.into_inner())?;
    let _ = net::protocol::read_packet(&mut stream);
    let elapsed = start.elapsed().as_millis().min(u32::MAX as u128) as u32;

    Ok(parse_status_json(&json, elapsed))
}

fn parse_status_json(json: &str, ping_ms: u32) -> ServerStatus {
    let value: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let version = value.get("version").and_then(|v| v.as_object());
    let players = value.get("players").and_then(|v| v.as_object());
    ServerStatus {
        online: true,
        ping_ms: Some(ping_ms),
        version_name: version
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        protocol: version
            .and_then(|v| v.get("protocol"))
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        players_online: players
            .and_then(|v| v.get("online"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        players_max: players
            .and_then(|v| v.get("max"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        description: value.get("description").map(status_description_text),
        error: None,
        favicon: value
            .get("favicon")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

fn status_description_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => {
            let mut out = String::new();
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                out.push_str(text);
            }
            if let Some(extra) = map.get("extra").and_then(|v| v.as_array()) {
                for item in extra {
                    out.push_str(&status_description_text(item));
                }
            }
            out
        }
        serde_json::Value::Array(items) => items
            .iter()
            .map(status_description_text)
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn parse_addr(addr: &str) -> (&str, u16) {
    if let Some(idx) = addr.rfind(':') {
        let host = &addr[..idx];
        let port = addr[idx + 1..].parse().unwrap_or(25565);
        (host, port)
    } else {
        (addr, 25565)
    }
}

/// Decode a Minecraft server favicon base64 data URL to raw RGBA pixels.
/// Format: `data:image/png;base64,<base64>`
pub fn decode_favicon(favicon: &str) -> Option<Vec<u8>> {
    let b64 = favicon.strip_prefix("data:image/png;base64,")?;
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    Some(img.to_rgba8().into_raw())
}
