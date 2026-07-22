//! Architecture abstraction layer — dependency-inversion interfaces for App / Renderer / Scripting.
//!
//! Goals:
//! - High-level modules depend on traits, not concrete `World` / `InputState` /
//!   `AssetResolver` / `Connection` types, so implementations can be mocked or swapped.
//! - This module only defines traits and minimal supporting types; existing concrete
//!   types can implement them later without changing call sites first.
//! - Depends only on `std` so it stays isolated from other crate modules.
//!
//! Trait surface is the smallest useful subset of the existing APIs.

use std::fmt::Debug;

// ===========================================================================
// World abstractions
// ===========================================================================

/// Read-only block state view, hiding storage (enum / state id / nibble) details.
///
/// Existing `crate::world::block::Block` and raw `u16` state ids can implement this via adapters.
pub trait BlockState: Debug {
    /// Block id (upper 12 bits).
    fn block_id(&self) -> u16;
    /// Block metadata / data value (lower 4 bits).
    fn metadata(&self) -> u8;
    /// Combined state id: `block_id << 4 | metadata` (MC 1.8.9 protocol).
    fn state(&self) -> u16 {
        (self.block_id() << 4) | (self.metadata() & 0x0F) as u16
    }
    /// Whether this is air (id == 0).
    fn is_air(&self) -> bool {
        self.block_id() == 0
    }
}

/// Read-only chunk view, matching `crate::world::chunk::Chunk`.
///
/// Coordinates are local to the chunk (0..16, 0..256, 0..16).
pub trait ChunkData: Debug {
    fn cx(&self) -> i32;
    fn cz(&self) -> i32;
    /// Block state at a local position, or `None` if out of range.
    fn block_at(&self, x: usize, y: usize, z: usize) -> Option<&dyn BlockState>;
}

/// World query service for App / Renderer / Scripting instead of owning `World` directly.
pub trait WorldProvider {
    /// Block state at world coordinates.
    fn get_block(&self, x: i32, y: i32, z: i32) -> Option<&dyn BlockState>;
    /// Chunk data at chunk coordinates.
    fn get_chunk(&self, cx: i32, cz: i32) -> Option<&dyn ChunkData>;
    /// Combined state id at world coordinates (same as `get_block(...).state()`).
    fn get_block_state(&self, x: i32, y: i32, z: i32) -> u16;
}

// ===========================================================================
// Input abstractions
// ===========================================================================

/// Input snapshot from one poll. Independent of winit / gilrs / keybind mapping.
/// Minimal public subset of `crate::client::keybind::InputState` semantics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputSnapshot {
    // Held actions
    pub attack_held: bool,
    pub use_held: bool,
    pub forward_held: bool,
    pub backward_held: bool,
    pub strafe_left_held: bool,
    pub strafe_right_held: bool,
    pub jump_held: bool,
    pub sneak_held: bool,
    pub sprint_held: bool,
    // Edge events for this frame (implementations clear after poll)
    pub inventory_pressed: bool,
    pub chat_pressed: bool,
}

/// Input source service instead of owning `InputState` / `GamepadInput` directly.
pub trait InputSource {
    /// Collect the current frame of input as a snapshot.
    fn poll_input(&mut self) -> InputSnapshot;
}

// ===========================================================================
// Asset abstractions
// ===========================================================================

/// Decoded texture data (RGBA8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Asset source service instead of owning `AssetResolver` / `ResourcePack` directly.
pub trait AssetSource {
    /// Load a decoded texture by path, or `None` if missing / decode fails.
    fn load_texture(&self, path: &str) -> Option<TextureData>;
    /// Whether the asset exists without fully decoding it.
    fn has_asset(&self, path: &str) -> bool;
}

// ===========================================================================
// Network abstractions
// ===========================================================================

/// Packet view that hides `DynamicPacket` / encoded bytes / protocol version details.
pub trait Packet: Debug {
    /// Packet name (e.g. `clientbound_chat_message`), or empty if unknown.
    fn name(&self) -> &str;
    /// Protocol packet id.
    fn id(&self) -> i32;
    /// Whether this is an outbound (client-to-server) packet.
    fn is_outbound(&self) -> bool;
    /// Raw payload without the packet header.
    fn payload(&self) -> &[u8];
}

/// Network source service instead of owning `net::connection::Connection` directly.
pub trait NetworkSource {
    /// Send an outbound packet.
    fn send_packet(&mut self, packet: &dyn Packet);
    /// Whether the source is connected and can send.
    fn is_connected(&self) -> bool;
}

// ===========================================================================
// Test mocks
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ----- BlockState / ChunkData mock -----

    #[derive(Clone, Debug)]
    struct MockBlock {
        id: u16,
        meta: u8,
    }

    impl BlockState for MockBlock {
        fn block_id(&self) -> u16 {
            self.id
        }
        fn metadata(&self) -> u8 {
            self.meta
        }
    }

    #[derive(Debug, Default)]
    struct MockChunk {
        cx: i32,
        cz: i32,
        blocks: HashMap<(usize, usize, usize), MockBlock>,
    }

    impl ChunkData for MockChunk {
        fn cx(&self) -> i32 {
            self.cx
        }
        fn cz(&self) -> i32 {
            self.cz
        }
        fn block_at(&self, x: usize, y: usize, z: usize) -> Option<&dyn BlockState> {
            self.blocks
                .get(&(x, y, z))
                .map(|block| block as &dyn BlockState)
        }
    }

    // ----- WorldProvider mock -----

    #[derive(Default)]
    struct MockWorld {
        blocks: HashMap<(i32, i32, i32), MockBlock>,
        chunks: HashMap<(i32, i32), MockChunk>,
    }

    impl WorldProvider for MockWorld {
        fn get_block(&self, x: i32, y: i32, z: i32) -> Option<&dyn BlockState> {
            self.blocks
                .get(&(x, y, z))
                .map(|block| block as &dyn BlockState)
        }
        fn get_chunk(&self, cx: i32, cz: i32) -> Option<&dyn ChunkData> {
            self.chunks
                .get(&(cx, cz))
                .map(|chunk| chunk as &dyn ChunkData)
        }
        fn get_block_state(&self, x: i32, y: i32, z: i32) -> u16 {
            self.get_block(x, y, z).map_or(0, |block| block.state())
        }
    }

    // ----- InputSource mock -----

    struct MockInput {
        snapshot: InputSnapshot,
    }

    impl InputSource for MockInput {
        fn poll_input(&mut self) -> InputSnapshot {
            self.snapshot.clone()
        }
    }

    // ----- AssetSource mock -----

    struct MockAssets {
        textures: HashMap<String, TextureData>,
    }

    impl AssetSource for MockAssets {
        fn load_texture(&self, path: &str) -> Option<TextureData> {
            self.textures.get(path).cloned()
        }
        fn has_asset(&self, path: &str) -> bool {
            self.textures.contains_key(path)
        }
    }

    // ----- Packet / NetworkSource mock -----

    #[derive(Clone, Debug)]
    struct MockPacket {
        name: String,
        id: i32,
        outbound: bool,
        payload: Vec<u8>,
    }

    impl Packet for MockPacket {
        fn name(&self) -> &str {
            &self.name
        }
        fn id(&self) -> i32 {
            self.id
        }
        fn is_outbound(&self) -> bool {
            self.outbound
        }
        fn payload(&self) -> &[u8] {
            &self.payload
        }
    }

    #[derive(Default)]
    struct MockNetwork {
        sent: Vec<MockPacket>,
        connected: bool,
    }

    impl NetworkSource for MockNetwork {
        fn send_packet(&mut self, packet: &dyn Packet) {
            self.sent.push(MockPacket {
                name: packet.name().to_string(),
                id: packet.id(),
                outbound: packet.is_outbound(),
                payload: packet.payload().to_vec(),
            });
        }
        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    // ----- tests -----

    #[test]
    fn block_state_default_state_encoding_matches_protocol() {
        let block = MockBlock { id: 1, meta: 2 };
        assert_eq!(block.state(), (1 << 4) | 2);
        assert!(!block.is_air());

        let air = MockBlock { id: 0, meta: 0 };
        assert!(air.is_air());
        assert_eq!(air.state(), 0);
    }

    #[test]
    fn mock_world_provider_returns_blocks_and_state() {
        let mut world = MockWorld::default();
        world.blocks.insert((1, 2, 3), MockBlock { id: 1, meta: 0 });

        let block = world.get_block(1, 2, 3).expect("block should exist");
        assert_eq!(block.block_id(), 1);
        assert_eq!(world.get_block_state(1, 2, 3), 16);
        assert!(world.get_block(0, 0, 0).is_none());
        assert_eq!(world.get_block_state(0, 0, 0), 0);
    }

    #[test]
    fn mock_world_provider_returns_chunks() {
        let mut world = MockWorld::default();
        let mut chunk = MockChunk::default();
        chunk.cx = 5;
        chunk.cz = -3;
        chunk.blocks.insert((0, 0, 0), MockBlock { id: 7, meta: 1 });
        world.chunks.insert((5, -3), chunk);

        let view = world.get_chunk(5, -3).expect("chunk should exist");
        assert_eq!(view.cx(), 5);
        assert_eq!(view.cz(), -3);
        let block = view.block_at(0, 0, 0).expect("block should exist");
        assert_eq!(block.block_id(), 7);
        assert_eq!(block.metadata(), 1);
        assert!(view.block_at(16, 0, 0).is_none());
    }

    #[test]
    fn mock_input_source_polls_snapshot() {
        let mut input = MockInput {
            snapshot: InputSnapshot {
                forward_held: true,
                jump_held: true,
                inventory_pressed: true,
                ..Default::default()
            },
        };
        let snapshot = input.poll_input();
        assert!(snapshot.forward_held);
        assert!(snapshot.jump_held);
        assert!(snapshot.inventory_pressed);
        assert!(!snapshot.attack_held);
    }

    #[test]
    fn mock_asset_source_loads_texture_and_reports_existence() {
        let mut assets = MockAssets {
            textures: HashMap::new(),
        };
        assets.textures.insert(
            "blocks/stone".to_string(),
            TextureData {
                width: 16,
                height: 16,
                rgba: vec![0u8; 16 * 16 * 4],
            },
        );

        assert!(assets.has_asset("blocks/stone"));
        assert!(!assets.has_asset("blocks/missing"));

        let texture = assets.load_texture("blocks/stone").expect("texture");
        assert_eq!(texture.width, 16);
        assert_eq!(texture.height, 16);
        assert_eq!(texture.rgba.len(), 16 * 16 * 4);
        assert!(assets.load_texture("blocks/missing").is_none());
    }

    #[test]
    fn mock_network_source_records_sent_packets() {
        let mut network = MockNetwork {
            sent: Vec::new(),
            connected: true,
        };
        assert!(network.is_connected());

        network.send_packet(&MockPacket {
            name: "serverbound_chat_message".to_string(),
            id: 0x01,
            outbound: true,
            payload: vec![0x00],
        });

        assert_eq!(network.sent.len(), 1);
        assert_eq!(network.sent[0].name, "serverbound_chat_message");
        assert_eq!(network.sent[0].id, 0x01);
        assert!(network.sent[0].outbound);
        assert_eq!(network.sent[0].payload, vec![0x00]);
    }

    #[test]
    fn mock_network_source_handles_disconnected_state() {
        let mut network = MockNetwork::default();
        assert!(!network.is_connected());
        // Trait still allows send_packet while disconnected; impl may drop it.
        network.send_packet(&MockPacket {
            name: "ping".to_string(),
            id: 0,
            outbound: true,
            payload: Vec::new(),
        });
        assert_eq!(network.sent.len(), 1);
    }
}
