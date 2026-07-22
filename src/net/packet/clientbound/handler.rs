//! Handler registry for clientbound packet dispatch.
//!
//! Each packet type can register a handler function. The registry provides a
//! type-safe dispatch mechanism that replaces the large match blocks in the
//! network processing pipeline. Handlers are grouped by protocol state
//! (Login/Play) and identified by packet ID.

use super::super::{ClientboundPacket, ProtocolState};
use std::collections::HashMap;

/// Opaque handler ID — the protocol packet ID within its state.
pub type PacketId = i32;

/// Result of a handler invocation.
#[derive(Debug)]
pub enum HandleResult {
    /// Packet was consumed; no further processing needed.
    Consumed,
    /// Packet was not handled; pass it to the next handler in the pipeline.
    PassThrough,
}

/// Type-erased packet handler function.
///
/// The handler receives the full parsed packet and returns whether it was
/// consumed or should be passed through to the next handler in the pipeline.
/// Because each handler needs different context (connection, entities, world,
/// etc.), the context is passed as a raw pointer to a handler-specific struct.
/// The caller is responsible for ensuring the pointer is valid and of the
/// correct type for each handler.
pub type PacketHandlerFn = fn(PacketId, &ClientboundPacket, *mut u8) -> HandleResult;

/// A registered handler with its priority (lower = runs first).
pub struct RegisteredHandler {
    pub handler: PacketHandlerFn,
    pub priority: u32,
    /// Human-readable name for debugging.
    pub name: &'static str,
}

/// Registry of packet handlers, organized by protocol state.
pub struct PacketHandlerRegistry {
    login: HashMap<PacketId, Vec<RegisteredHandler>>,
    play: HashMap<PacketId, Vec<RegisteredHandler>>,
}

impl PacketHandlerRegistry {
    pub fn new() -> Self {
        PacketHandlerRegistry {
            login: HashMap::new(),
            play: HashMap::new(),
        }
    }

    /// Register a handler for a login-state packet ID.
    pub fn register_login(
        &mut self,
        packet_id: PacketId,
        priority: u32,
        name: &'static str,
        handler: PacketHandlerFn,
    ) {
        self.login
            .entry(packet_id)
            .or_default()
            .push(RegisteredHandler {
                handler,
                priority,
                name,
            });
    }

    /// Register a handler for a play-state packet ID.
    pub fn register_play(
        &mut self,
        packet_id: PacketId,
        priority: u32,
        name: &'static str,
        handler: PacketHandlerFn,
    ) {
        self.play
            .entry(packet_id)
            .or_default()
            .push(RegisteredHandler {
                handler,
                priority,
                name,
            });
    }

    /// Sort handlers by priority for all registered packet IDs.
    pub fn finalize(&mut self) {
        for handlers in self.login.values_mut() {
            handlers.sort_by_key(|h| h.priority);
        }
        for handlers in self.play.values_mut() {
            handlers.sort_by_key(|h| h.priority);
        }
    }

    /// Dispatch a packet to all registered handlers for its ID.
    ///
    /// Handlers are called in priority order. If any handler returns
    /// `Consumed`, subsequent handlers for the same packet ID are skipped.
    /// Returns true if the packet was consumed.
    pub fn dispatch(
        &self,
        state: ProtocolState,
        packet_id: PacketId,
        packet: &ClientboundPacket,
        ctx: *mut u8,
    ) -> bool {
        let handlers = match state {
            ProtocolState::Login => self.login.get(&packet_id),
            ProtocolState::Play => self.play.get(&packet_id),
            _ => None,
        };

        if let Some(handlers) = handlers {
            for registered in handlers {
                if let HandleResult::Consumed = (registered.handler)(packet_id, packet, ctx) {
                    return true;
                }
            }
        }
        false
    }

    /// List all registered handler names for a given state and packet ID.
    pub fn handler_names(&self, state: ProtocolState, packet_id: PacketId) -> Vec<&'static str> {
        let handlers = match state {
            ProtocolState::Login => self.login.get(&packet_id),
            ProtocolState::Play => self.play.get(&packet_id),
            _ => None,
        };
        handlers
            .map(|h| h.iter().map(|r| r.name).collect())
            .unwrap_or_default()
    }
}

impl Default for PacketHandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Packet ID constants for Login state (protocol v47).
pub mod login_ids {
    pub const DISCONNECT: i32 = 0x00;
    pub const ENCRYPTION_REQUEST: i32 = 0x01;
    pub const LOGIN_SUCCESS: i32 = 0x02;
    pub const SET_COMPRESSION: i32 = 0x03;
}

/// Packet ID constants for Play state (protocol v47).
pub mod play_ids {
    pub const KEEP_ALIVE: i32 = 0x00;
    pub const JOIN_GAME: i32 = 0x01;
    pub const CHAT_MESSAGE: i32 = 0x02;
    pub const TIME_UPDATE: i32 = 0x03;
    pub const ENTITY_EQUIPMENT: i32 = 0x04;
    pub const SPAWN_POSITION: i32 = 0x05;
    pub const UPDATE_HEALTH: i32 = 0x06;
    pub const RESPAWN: i32 = 0x07;
    pub const PLAYER_POSITION_AND_LOOK: i32 = 0x08;
    pub const HELD_ITEM_CHANGE: i32 = 0x09;
    pub const USE_BED: i32 = 0x0A;
    pub const ANIMATION: i32 = 0x0B;
    pub const SPAWN_PLAYER: i32 = 0x0C;
    pub const COLLECT_ITEM: i32 = 0x0D;
    pub const SPAWN_OBJECT: i32 = 0x0E;
    pub const SPAWN_MOB: i32 = 0x0F;
    pub const ENTITY_VELOCITY: i32 = 0x12;
    pub const DESTROY_ENTITIES: i32 = 0x13;
    pub const ENTITY: i32 = 0x14;
    pub const ENTITY_MOVE: i32 = 0x15;
    pub const ENTITY_LOOK: i32 = 0x16;
    pub const ENTITY_MOVE_LOOK: i32 = 0x17;
    pub const ENTITY_TELEPORT: i32 = 0x18;
    pub const ENTITY_HEAD_LOOK: i32 = 0x19;
    pub const ENTITY_STATUS: i32 = 0x1A;
    pub const ATTACH_ENTITY: i32 = 0x1B;
    pub const ENTITY_METADATA: i32 = 0x1C;
    pub const ENTITY_EFFECT: i32 = 0x1D;
    pub const REMOVE_ENTITY_EFFECT: i32 = 0x1E;
    pub const SET_EXPERIENCE: i32 = 0x1F;
    pub const ENTITY_PROPERTIES: i32 = 0x20;
    pub const CHUNK_DATA: i32 = 0x21;
    pub const BLOCK_CHANGE: i32 = 0x23;
    pub const MULTI_BLOCK_CHANGE: i32 = 0x22;
    pub const BLOCK_ACTION: i32 = 0x24;
    pub const BLOCK_BREAK_ANIMATION: i32 = 0x25;
    pub const MAP_CHUNK_BULK: i32 = 0x26;
    pub const EXPLOSION: i32 = 0x27;
    pub const EFFECT: i32 = 0x28;
    pub const NAMED_SOUND_EFFECT: i32 = 0x29;
    pub const PARTICLE: i32 = 0x2A;
    pub const CHANGE_GAME_STATE: i32 = 0x2B;
    pub const SPAWN_GLOBAL_ENTITY: i32 = 0x10;
    pub const EXPERIENCE_ORB_SPAWN: i32 = 0x11;
    pub const OPEN_WINDOW: i32 = 0x2D;
    pub const CLOSE_WINDOW: i32 = 0x2E;
    pub const SET_SLOT: i32 = 0x2F;
    pub const WINDOW_ITEMS: i32 = 0x30;
    pub const WINDOW_PROPERTY: i32 = 0x31;
    pub const CONFIRM_TRANSACTION: i32 = 0x32;
    pub const UPDATE_SIGN: i32 = 0x33;
    pub const MAP_DATA: i32 = 0x34;
    pub const UPDATE_BLOCK_ENTITY: i32 = 0x35;
    pub const SIGN_EDITOR_OPEN: i32 = 0x36;
    pub const STATISTICS: i32 = 0x37;
    pub const PLAYER_LIST_ITEM: i32 = 0x38;
    pub const PLAYER_ABILITIES: i32 = 0x39;
    pub const TAB_COMPLETE: i32 = 0x3A;
    pub const SCOREBOARD_OBJECTIVE: i32 = 0x3B;
    pub const UPDATE_SCORE: i32 = 0x3C;
    pub const DISPLAY_SCOREBOARD: i32 = 0x3D;
    pub const TEAMS: i32 = 0x3E;
    pub const PLUGIN_MESSAGE: i32 = 0x3F;
    pub const DISCONNECT: i32 = 0x40;
    pub const SERVER_DIFFICULTY: i32 = 0x41;
    pub const COMBAT_EVENT: i32 = 0x42;
    pub const CAMERA: i32 = 0x43;
    pub const WORLD_BORDER: i32 = 0x44;
    pub const TITLE: i32 = 0x45;
    pub const SET_COMPRESSION: i32 = 0x46;
    pub const PLAYER_LIST_HEADER_FOOTER: i32 = 0x47;
    pub const RESOURCE_PACK_SEND: i32 = 0x48;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_dispatches_to_registered_handler() {
        fn test_handler(
            _id: PacketId,
            _packet: &ClientboundPacket,
            _ctx: *mut u8,
        ) -> HandleResult {
            HandleResult::Consumed
        }

        let mut registry = PacketHandlerRegistry::new();
        registry.register_play(play_ids::KEEP_ALIVE, 0, "test", test_handler);
        registry.finalize();

        let packet = ClientboundPacket::KeepAlive { id: 42 };
        assert!(registry.dispatch(ProtocolState::Play, play_ids::KEEP_ALIVE, &packet, std::ptr::null_mut()));
    }

    #[test]
    fn unregistered_packet_is_not_consumed() {
        let registry = PacketHandlerRegistry::new();
        let packet = ClientboundPacket::KeepAlive { id: 42 };
        assert!(!registry.dispatch(ProtocolState::Play, play_ids::KEEP_ALIVE, &packet, std::ptr::null_mut()));
    }

    #[test]
    fn pass_through_allows_next_handler_to_run() {
        fn passthrough_handler(
            _id: PacketId,
            _packet: &ClientboundPacket,
            _ctx: *mut u8,
        ) -> HandleResult {
            HandleResult::PassThrough
        }

        fn consume_handler(
            _id: PacketId,
            _packet: &ClientboundPacket,
            _ctx: *mut u8,
        ) -> HandleResult {
            HandleResult::Consumed
        }

        let mut registry = PacketHandlerRegistry::new();
        registry.register_play(play_ids::KEEP_ALIVE, 0, "passthrough", passthrough_handler);
        registry.register_play(play_ids::KEEP_ALIVE, 1, "consume", consume_handler);
        registry.finalize();

        let packet = ClientboundPacket::KeepAlive { id: 42 };
        assert!(registry.dispatch(ProtocolState::Play, play_ids::KEEP_ALIVE, &packet, std::ptr::null_mut()));
    }

    #[test]
    fn handler_names_lists_registered_handlers() {
        fn test_handler(
            _id: PacketId,
            _packet: &ClientboundPacket,
            _ctx: *mut u8,
        ) -> HandleResult {
            HandleResult::Consumed
        }

        let mut registry = PacketHandlerRegistry::new();
        registry.register_play(play_ids::KEEP_ALIVE, 0, "keep_alive", test_handler);
        registry.register_play(play_ids::KEEP_ALIVE, 1, "keep_alive_log", test_handler);
        registry.finalize();

        let names = registry.handler_names(ProtocolState::Play, play_ids::KEEP_ALIVE);
        assert_eq!(names, vec!["keep_alive", "keep_alive_log"]);
    }
}
