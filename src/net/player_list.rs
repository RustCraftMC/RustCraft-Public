use super::protocol::PacketBuffer;
use std::io;

#[derive(Debug, Clone)]
pub enum PlayerListAction {
    AddPlayer,
    UpdateGamemode,
    UpdateLatency,
    UpdateDisplayName,
    RemovePlayer,
}

#[derive(Debug, Clone)]
pub struct PlayerListEntry {
    pub uuid: String,
    pub name: Option<String>,
    pub properties: Vec<PlayerProperty>,
    pub gamemode: Option<i32>,
    pub ping: Option<i32>,
    pub display_name_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlayerProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

pub fn read_player_list_item(
    buf: &mut PacketBuffer,
) -> io::Result<(PlayerListAction, Vec<PlayerListEntry>)> {
    let raw_action = buf.read_varint()?;
    let action = match raw_action {
        0 => PlayerListAction::AddPlayer,
        1 => PlayerListAction::UpdateGamemode,
        2 => PlayerListAction::UpdateLatency,
        3 => PlayerListAction::UpdateDisplayName,
        4 => PlayerListAction::RemovePlayer,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid player list action {}", raw_action),
            ))
        }
    };
    let count = buf.read_varint()?.max(0) as usize;
    let mut players = Vec::with_capacity(count);
    for _ in 0..count {
        let uuid = buf.read_uuid()?;
        let mut entry = PlayerListEntry {
            uuid,
            name: None,
            properties: Vec::new(),
            gamemode: None,
            ping: None,
            display_name_json: None,
        };
        match action {
            PlayerListAction::AddPlayer => {
                entry.name = Some(buf.read_string()?);
                let prop_count = buf.read_varint()?.max(0) as usize;
                for _ in 0..prop_count {
                    let name = buf.read_string()?;
                    let value = buf.read_string()?;
                    let signature = if buf.read_bool()? {
                        Some(buf.read_string()?)
                    } else {
                        None
                    };
                    entry.properties.push(PlayerProperty {
                        name,
                        value,
                        signature,
                    });
                }
                entry.gamemode = Some(buf.read_varint()?);
                entry.ping = Some(buf.read_varint()?);
                entry.display_name_json = if buf.read_bool()? {
                    Some(buf.read_string()?)
                } else {
                    None
                };
            }
            PlayerListAction::UpdateGamemode => {
                entry.gamemode = Some(buf.read_varint()?);
            }
            PlayerListAction::UpdateLatency => {
                entry.ping = Some(buf.read_varint()?);
            }
            PlayerListAction::UpdateDisplayName => {
                entry.display_name_json = if buf.read_bool()? {
                    Some(buf.read_string()?)
                } else {
                    None
                };
            }
            PlayerListAction::RemovePlayer => {}
        }
        players.push(entry);
    }
    Ok((action, players))
}
