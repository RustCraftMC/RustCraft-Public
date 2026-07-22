pub mod handler;
pub mod login;
pub mod play;

pub use handler::{HandleResult, PacketHandlerFn, PacketHandlerRegistry, PacketId, RegisteredHandler};

use super::{
    ChunkBulkData, ChunkMeta, CombatEvent, EntityProperty, EntityPropertyModifier, MapIcon,
    WorldBorderUpdate,
};
use crate::net::packet::{ClientboundPacket, ProtocolState};
use std::io;

impl ClientboundPacket {
    pub fn parse(state: ProtocolState, id: i32, data: &[u8]) -> io::Result<Self> {
        match state {
            ProtocolState::Login => Self::parse_login(id, data),
            ProtocolState::Play => Self::parse_play(id, data),
            _ => Ok(ClientboundPacket::Unknown { id }),
        }
    }
}
