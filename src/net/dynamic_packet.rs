//! Structured packet representation used by scripting and protocol translation.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::io;

use crate::net::packet::ClientboundPacket;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketDirection {
    Inbound,
    Outbound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicProtocolState {
    Handshake,
    Status,
    Login,
    Configuration,
    Play,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion(pub i32);

impl ProtocolVersion {
    pub const V1_8_9: Self = Self(47);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicPacket {
    pub direction: PacketDirection,
    pub state: DynamicProtocolState,
    pub version: ProtocolVersion,
    pub packet_id: i32,
    pub packet_name: Option<String>,
    pub fields: Value,
    #[serde(skip_serializing, skip_deserializing)]
    pub raw_payload: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedPacket {
    pub packet_id: i32,
    pub payload: Vec<u8>,
}

impl DynamicPacket {
    /// Decode the small, explicitly supported play-packet surface used by protocol tests and
    /// scripting. Unknown packets stay on the typed Rust path instead of exposing raw bytes.
    pub fn decode_supported_play(
        version: ProtocolVersion,
        direction: PacketDirection,
        packet_id: i32,
        payload: &[u8],
    ) -> io::Result<Self> {
        let (packet_name, fields) = match (version.0, direction, packet_id) {
            (47, PacketDirection::Inbound, 0x02) | (107, PacketDirection::Inbound, 0x0F) => {
                let mut buffer = crate::net::protocol::PacketBuffer::new(payload.to_vec());
                let message = buffer.read_string()?;
                if message.len() > 262_144 {
                    return Err(invalid("chat component exceeds 262144 bytes"));
                }
                let position = buffer.read_byte()?;
                if buffer.remaining() != 0 {
                    return Err(invalid("chat packet contains trailing bytes"));
                }
                (
                    "clientbound_chat_message",
                    serde_json::json!({"message": message, "position": position}),
                )
            }
            (47, PacketDirection::Outbound, 0x01) | (107, PacketDirection::Outbound, 0x02) => {
                let mut buffer = crate::net::protocol::PacketBuffer::new(payload.to_vec());
                let message = buffer.read_string()?;
                if message.len() > 100 {
                    return Err(invalid("chat message exceeds 100 bytes"));
                }
                if buffer.remaining() != 0 {
                    return Err(invalid("chat packet contains trailing bytes"));
                }
                (
                    "serverbound_chat_message",
                    serde_json::json!({"message": message}),
                )
            }
            _ => {
                return Err(invalid(format!(
                    "unsupported protocol {} play packet 0x{packet_id:02X}",
                    version.0
                )))
            }
        };
        Ok(Self {
            direction,
            state: DynamicProtocolState::Play,
            version,
            packet_id,
            packet_name: Some(packet_name.into()),
            fields,
            raw_payload: None,
        })
    }

    pub fn encode_supported_play(&self) -> io::Result<EncodedPacket> {
        if self.state != DynamicProtocolState::Play {
            return Err(invalid("packet is not in the play state"));
        }
        let name = self
            .packet_name
            .as_deref()
            .ok_or_else(|| invalid("packet name is required"))?;
        let fields = object(self.fields.clone())?;
        let packet_id = match (self.version.0, self.direction, name) {
            (47, PacketDirection::Inbound, "clientbound_chat_message") => 0x02,
            (107, PacketDirection::Inbound, "clientbound_chat_message") => 0x0F,
            (47, PacketDirection::Outbound, "serverbound_chat_message") => 0x01,
            (107, PacketDirection::Outbound, "serverbound_chat_message") => 0x02,
            _ => return Err(invalid("unsupported packet for the selected version")),
        };
        let mut buffer = crate::net::protocol::PacketBuffer::empty();
        match self.direction {
            PacketDirection::Inbound => {
                exact_fields(&fields, &["message", "position"])?;
                let message = string_field(&fields, "message", 262_144)?;
                let position = i8_field(&fields, "position")?;
                buffer.write_string(&message);
                buffer.write_byte(position);
            }
            PacketDirection::Outbound => {
                exact_fields(&fields, &["message"])?;
                let message = string_field(&fields, "message", 100)?;
                buffer.write_string(&message);
            }
        }
        Ok(EncodedPacket {
            packet_id,
            payload: buffer.into_inner(),
        })
    }

    pub fn from_v47_clientbound(packet: &ClientboundPacket) -> Option<Self> {
        let (packet_id, packet_name, fields) = match packet {
            ClientboundPacket::ChatMessage { json, position } => (
                0x02,
                "clientbound_chat_message",
                serde_json::json!({ "message": json, "position": position }),
            ),
            ClientboundPacket::PluginMessage { channel, data } => (
                0x3F,
                "clientbound_custom_payload",
                serde_json::json!({ "channel": channel, "data": data }),
            ),
            ClientboundPacket::Disconnect { reason } => (
                0x40,
                "clientbound_disconnect",
                serde_json::json!({ "reason": reason }),
            ),
            _ => return None,
        };
        Some(Self {
            direction: PacketDirection::Inbound,
            state: DynamicProtocolState::Play,
            version: ProtocolVersion::V1_8_9,
            packet_id,
            packet_name: Some(packet_name.into()),
            fields,
            raw_payload: None,
        })
    }

    pub fn into_v47_clientbound(self) -> io::Result<ClientboundPacket> {
        if self.direction != PacketDirection::Inbound
            || self.state != DynamicProtocolState::Play
            || self.version != ProtocolVersion::V1_8_9
        {
            return Err(invalid("packet is not a v47 inbound play packet"));
        }
        let name = self
            .packet_name
            .as_deref()
            .ok_or_else(|| invalid("packet name is required"))?;
        let fields = object(self.fields)?;
        match name {
            "clientbound_chat_message" => {
                exact_fields(&fields, &["message", "position"])?;
                let message = string_field(&fields, "message", 262_144)?;
                let position = i8_field(&fields, "position")?;
                Ok(ClientboundPacket::ChatMessage {
                    json: message,
                    position,
                })
            }
            "clientbound_custom_payload" => {
                exact_fields(&fields, &["channel", "data"])?;
                let channel = string_field(&fields, "channel", 20)?;
                let data = byte_array_field(&fields, "data", 1024 * 1024)?;
                Ok(ClientboundPacket::PluginMessage { channel, data })
            }
            "clientbound_disconnect" => {
                exact_fields(&fields, &["reason"])?;
                Ok(ClientboundPacket::Disconnect {
                    reason: string_field(&fields, "reason", 262_144)?,
                })
            }
            _ => Err(invalid(format!(
                "unsupported v47 clientbound packet '{name}'"
            ))),
        }
    }

    pub fn v47_chat_message(message: impl Into<String>) -> Self {
        Self {
            direction: PacketDirection::Outbound,
            state: DynamicProtocolState::Play,
            version: ProtocolVersion::V1_8_9,
            packet_id: 0x01,
            packet_name: Some("serverbound_chat_message".into()),
            fields: serde_json::json!({ "message": message.into() }),
            raw_payload: None,
        }
    }

    pub fn v47_serverbound_named(name: impl Into<String>, fields: Value) -> io::Result<Self> {
        let name = name.into();
        let packet_id = packet_id_for(
            PacketDirection::Outbound,
            DynamicProtocolState::Play,
            ProtocolVersion::V1_8_9,
            &name,
        )
        .ok_or_else(|| invalid(format!("unsupported v47 serverbound packet '{name}'")))?;
        let packet = Self {
            direction: PacketDirection::Outbound,
            state: DynamicProtocolState::Play,
            version: ProtocolVersion::V1_8_9,
            packet_id,
            packet_name: Some(name),
            fields,
            raw_payload: None,
        };
        packet.encode_v47_serverbound()?;
        Ok(packet)
    }

    pub fn encode_v47_serverbound(&self) -> io::Result<EncodedPacket> {
        if self.direction != PacketDirection::Outbound
            || self.state != DynamicProtocolState::Play
            || self.version != ProtocolVersion::V1_8_9
        {
            return Err(invalid("packet is not a v47 outbound play packet"));
        }
        let name = self
            .packet_name
            .as_deref()
            .ok_or_else(|| invalid("packet name is required"))?;
        let fields = object(self.fields.clone())?;
        let (packet_id, payload) = match name {
            "serverbound_chat_message" => {
                exact_fields(&fields, &["message"])?;
                let message = string_field(&fields, "message", 100)?;
                (0x01, crate::net::packet::write_chat_message(&message))
            }
            "serverbound_client_information" => {
                exact_fields(
                    &fields,
                    &[
                        "locale",
                        "view_distance",
                        "chat_mode",
                        "chat_colors",
                        "skin_parts",
                    ],
                )?;
                let locale = string_field(&fields, "locale", 16)?;
                let view_distance = i8_field(&fields, "view_distance")?.clamp(2, 16);
                let chat_mode = u8_field(&fields, "chat_mode")?;
                let chat_colors = bool_field(&fields, "chat_colors")?;
                let skin_parts = u8_field(&fields, "skin_parts")?;
                (
                    0x15,
                    crate::net::packet::write_client_settings(
                        &locale,
                        view_distance,
                        chat_mode,
                        chat_colors,
                        skin_parts,
                    ),
                )
            }
            "serverbound_custom_payload" => {
                exact_fields(&fields, &["channel", "data"])?;
                let channel = string_field(&fields, "channel", 20)?;
                let data = byte_array_field(&fields, "data", 32_767)?;
                (
                    0x17,
                    crate::net::packet::write_plugin_message(&channel, &data),
                )
            }
            _ => {
                return Err(invalid(format!(
                    "unsupported v47 serverbound packet '{name}'"
                )))
            }
        };
        Ok(EncodedPacket { packet_id, payload })
    }

    pub fn replacement(&self, name: String, fields: Value) -> io::Result<Self> {
        let packet_id = packet_id_for(self.direction, self.state, self.version, &name)
            .ok_or_else(|| invalid(format!("unsupported packet replacement '{name}'")))?;
        Ok(Self {
            direction: self.direction,
            state: self.state,
            version: self.version,
            packet_id,
            packet_name: Some(name),
            fields,
            raw_payload: None,
        })
    }
}

pub fn packet_id_for(
    direction: PacketDirection,
    state: DynamicProtocolState,
    version: ProtocolVersion,
    name: &str,
) -> Option<i32> {
    if state != DynamicProtocolState::Play || version != ProtocolVersion::V1_8_9 {
        return None;
    }
    match (direction, name) {
        (PacketDirection::Inbound, "clientbound_chat_message") => Some(0x02),
        (PacketDirection::Inbound, "clientbound_custom_payload") => Some(0x3F),
        (PacketDirection::Inbound, "clientbound_disconnect") => Some(0x40),
        (PacketDirection::Outbound, "serverbound_chat_message") => Some(0x01),
        (PacketDirection::Outbound, "serverbound_client_information") => Some(0x15),
        (PacketDirection::Outbound, "serverbound_custom_payload") => Some(0x17),
        _ => None,
    }
}

fn object(value: Value) -> io::Result<Map<String, Value>> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("packet fields must be an object"))
}

fn exact_fields(fields: &Map<String, Value>, expected: &[&str]) -> io::Result<()> {
    let expected: HashSet<_> = expected.iter().copied().collect();
    if let Some(unknown) = fields.keys().find(|key| !expected.contains(key.as_str())) {
        return Err(invalid(format!("unknown packet field '{unknown}'")));
    }
    if let Some(missing) = expected.iter().find(|key| !fields.contains_key(**key)) {
        return Err(invalid(format!("missing packet field '{missing}'")));
    }
    Ok(())
}

fn string_field(fields: &Map<String, Value>, name: &str, max: usize) -> io::Result<String> {
    let value = fields
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("field '{name}' must be a string")))?;
    if value.len() > max {
        return Err(invalid(format!("field '{name}' exceeds {max} bytes")));
    }
    Ok(value.to_owned())
}

fn i32_field(fields: &Map<String, Value>, name: &str) -> io::Result<i32> {
    fields
        .get(name)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| invalid(format!("field '{name}' must be an i32")))
}

fn i8_field(fields: &Map<String, Value>, name: &str) -> io::Result<i8> {
    i32_field(fields, name).and_then(|value| {
        i8::try_from(value).map_err(|_| invalid(format!("field '{name}' must be an i8")))
    })
}

fn u8_field(fields: &Map<String, Value>, name: &str) -> io::Result<u8> {
    i32_field(fields, name).and_then(|value| {
        u8::try_from(value).map_err(|_| invalid(format!("field '{name}' must be a u8")))
    })
}

fn bool_field(fields: &Map<String, Value>, name: &str) -> io::Result<bool> {
    fields
        .get(name)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("field '{name}' must be a bool")))
}

fn byte_array_field(fields: &Map<String, Value>, name: &str, max: usize) -> io::Result<Vec<u8>> {
    let values = fields
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("field '{name}' must be a byte array")))?;
    if values.len() > max {
        return Err(invalid(format!("field '{name}' exceeds {max} bytes")));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| invalid(format!("field '{name}' contains a non-byte value")))
        })
        .collect()
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v47_chat_decode_modify_encode_uses_fixed_vector() {
        let original = ClientboundPacket::ChatMessage {
            json: r#"{"text":"hello"}"#.into(),
            position: 0,
        };
        let mut dynamic = DynamicPacket::from_v47_clientbound(&original).unwrap();
        dynamic.fields["message"] = Value::String(r#"{"text":"changed"}"#.into());
        let converted = dynamic.into_v47_clientbound().unwrap();
        assert!(matches!(
            converted,
            ClientboundPacket::ChatMessage { json, position: 0 } if json == r#"{"text":"changed"}"#
        ));

        let encoded = DynamicPacket::v47_chat_message("hello")
            .encode_v47_serverbound()
            .unwrap();
        assert_eq!(encoded.packet_id, 0x01);
        assert_eq!(encoded.payload, vec![5, b'h', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn v47_codec_rejects_unknown_fields_and_oversized_payloads() {
        let mut packet = DynamicPacket::v47_chat_message("hello");
        packet.fields["unknown"] = Value::Bool(true);
        assert!(packet.encode_v47_serverbound().is_err());

        packet.fields = serde_json::json!({"message": "x".repeat(101)});
        assert!(packet.encode_v47_serverbound().is_err());
    }
}
