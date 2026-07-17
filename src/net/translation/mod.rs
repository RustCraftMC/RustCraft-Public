//! Data-driven-friendly protocol translation pipeline layered above compression/encryption.

pub mod item;
pub mod metadata;
mod v47_to_v107;

use crate::net::dynamic_packet::{DynamicPacket, DynamicProtocolState, ProtocolVersion};
use std::collections::{HashMap, VecDeque};
use std::fmt;

pub use v47_to_v107::V47ToV107Translator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError(pub String);

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Clone, Debug, Default)]
pub struct EntityTracker {
    pub entity_types: HashMap<i32, String>,
    pub metadata: HashMap<i32, serde_json::Value>,
}

#[derive(Clone, Debug, Default)]
pub struct RegistryTracker {
    pub dimensions: HashMap<i32, String>,
    pub block_states: HashMap<i32, String>,
}

#[derive(Clone, Debug, Default)]
pub struct InventoryTracker {
    pub window_id: u8,
    pub slots: HashMap<i16, serde_json::Value>,
}

#[derive(Clone, Debug, Default)]
pub struct WorldTracker {
    pub dimension: Option<String>,
    pub pending_teleports: VecDeque<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct ChatSessionState {
    pub last_seen_message: u64,
}

#[derive(Clone, Debug)]
pub struct TranslationContext {
    pub source_version: ProtocolVersion,
    pub target_version: ProtocolVersion,
    pub state: DynamicProtocolState,
    pub entity_tracker: EntityTracker,
    pub registry_tracker: RegistryTracker,
    pub inventory_tracker: InventoryTracker,
    pub world_tracker: WorldTracker,
    pub chat_session: ChatSessionState,
}

impl TranslationContext {
    pub fn new(source_version: ProtocolVersion, target_version: ProtocolVersion) -> Self {
        Self {
            source_version,
            target_version,
            state: DynamicProtocolState::Handshake,
            entity_tracker: EntityTracker::default(),
            registry_tracker: RegistryTracker::default(),
            inventory_tracker: InventoryTracker::default(),
            world_tracker: WorldTracker::default(),
            chat_session: ChatSessionState::default(),
        }
    }

    pub fn clear_connection_state(&mut self) {
        self.entity_tracker = EntityTracker::default();
        self.registry_tracker = RegistryTracker::default();
        self.inventory_tracker = InventoryTracker::default();
        self.world_tracker = WorldTracker::default();
        self.chat_session = ChatSessionState::default();
        self.state = DynamicProtocolState::Handshake;
    }
}

pub trait ProtocolTranslator {
    fn source_version(&self) -> ProtocolVersion;
    fn target_version(&self) -> ProtocolVersion;

    fn translate_inbound(
        &mut self,
        packet: DynamicPacket,
        context: &mut TranslationContext,
    ) -> Result<Vec<DynamicPacket>, ProtocolError>;

    fn translate_outbound(
        &mut self,
        packet: DynamicPacket,
        context: &mut TranslationContext,
    ) -> Result<Vec<DynamicPacket>, ProtocolError>;
}

pub struct TranslationPipeline {
    source_version: ProtocolVersion,
    target_version: ProtocolVersion,
    translators: Vec<Box<dyn ProtocolTranslator>>,
    context: TranslationContext,
}

impl TranslationPipeline {
    fn new(
        source_version: ProtocolVersion,
        target_version: ProtocolVersion,
        translators: Vec<Box<dyn ProtocolTranslator>>,
    ) -> Self {
        Self {
            source_version,
            target_version,
            translators,
            context: TranslationContext::new(source_version, target_version),
        }
    }

    pub fn len(&self) -> usize {
        self.translators.len()
    }

    pub fn is_empty(&self) -> bool {
        self.translators.is_empty()
    }

    pub fn translate_inbound(
        &mut self,
        packet: DynamicPacket,
    ) -> Result<Vec<DynamicPacket>, ProtocolError> {
        let mut packets = vec![packet];
        for translator in &mut self.translators {
            let mut translated = Vec::new();
            for packet in packets {
                translated.extend(translator.translate_inbound(packet, &mut self.context)?);
            }
            packets = translated;
        }
        Ok(packets)
    }

    pub fn translate_outbound(
        &mut self,
        packet: DynamicPacket,
    ) -> Result<Vec<DynamicPacket>, ProtocolError> {
        let mut packets = vec![packet];
        for translator in self.translators.iter_mut().rev() {
            let mut translated = Vec::new();
            for packet in packets {
                translated.extend(translator.translate_outbound(packet, &mut self.context)?);
            }
            packets = translated;
        }
        Ok(packets)
    }

    pub fn context(&self) -> &TranslationContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut TranslationContext {
        &mut self.context
    }

    pub fn source_version(&self) -> ProtocolVersion {
        self.source_version
    }

    pub fn target_version(&self) -> ProtocolVersion {
        self.target_version
    }

    pub fn set_state(&mut self, state: DynamicProtocolState) {
        self.context.state = state;
    }

    pub fn disconnect(&mut self) {
        self.context.clear_connection_state();
    }
}

type TranslatorFactory = fn() -> Box<dyn ProtocolTranslator>;

#[derive(Default)]
pub struct ProtocolTranslationRegistry {
    factories: HashMap<(ProtocolVersion, ProtocolVersion), TranslatorFactory>,
}

impl ProtocolTranslationRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::default();
        registry.register(ProtocolVersion(47), ProtocolVersion(107), || {
            Box::new(V47ToV107Translator)
        });
        registry
    }

    pub fn register(
        &mut self,
        source: ProtocolVersion,
        target: ProtocolVersion,
        factory: TranslatorFactory,
    ) {
        self.factories.insert((source, target), factory);
    }

    pub fn build_shortest(
        &self,
        source: ProtocolVersion,
        target: ProtocolVersion,
    ) -> Result<TranslationPipeline, ProtocolError> {
        if source == target {
            return Ok(TranslationPipeline::new(source, target, Vec::new()));
        }
        let mut queue = VecDeque::from([source]);
        let mut previous = HashMap::<ProtocolVersion, ProtocolVersion>::new();
        previous.insert(source, source);
        while let Some(version) = queue.pop_front() {
            for &(edge_source, edge_target) in self.factories.keys() {
                if edge_source != version || previous.contains_key(&edge_target) {
                    continue;
                }
                previous.insert(edge_target, version);
                if edge_target == target {
                    queue.clear();
                    break;
                }
                queue.push_back(edge_target);
            }
        }
        if !previous.contains_key(&target) {
            return Err(ProtocolError(format!(
                "no protocol translation path from {} to {}",
                source.0, target.0
            )));
        }
        let mut versions = vec![target];
        while *versions.last().unwrap() != source {
            versions.push(previous[versions.last().unwrap()]);
        }
        versions.reverse();
        let translators = versions
            .windows(2)
            .map(|pair| self.factories[&(pair[0], pair[1])]())
            .collect();
        Ok(TranslationPipeline::new(source, target, translators))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::dynamic_packet::{PacketDirection, ProtocolVersion};

    struct TestTranslator {
        source: ProtocolVersion,
        target: ProtocolVersion,
        fan_out: bool,
    }

    impl TestTranslator {
        fn translate(
            &self,
            mut packet: DynamicPacket,
            version: ProtocolVersion,
            marker: &str,
        ) -> Vec<DynamicPacket> {
            packet.version = version;
            let trace = packet
                .fields
                .as_object_mut()
                .expect("test packet fields must be an object")
                .entry("trace")
                .or_insert_with(|| serde_json::json!([]));
            trace
                .as_array_mut()
                .expect("test trace must be an array")
                .push(serde_json::Value::String(marker.into()));
            if self.fan_out {
                let mut duplicate = packet.clone();
                duplicate.packet_id += 1;
                vec![packet, duplicate]
            } else {
                vec![packet]
            }
        }
    }

    impl ProtocolTranslator for TestTranslator {
        fn source_version(&self) -> ProtocolVersion {
            self.source
        }

        fn target_version(&self) -> ProtocolVersion {
            self.target
        }

        fn translate_inbound(
            &mut self,
            packet: DynamicPacket,
            _context: &mut TranslationContext,
        ) -> Result<Vec<DynamicPacket>, ProtocolError> {
            Ok(self.translate(
                packet,
                self.target,
                &format!("{}>{}", self.source.0, self.target.0),
            ))
        }

        fn translate_outbound(
            &mut self,
            packet: DynamicPacket,
            _context: &mut TranslationContext,
        ) -> Result<Vec<DynamicPacket>, ProtocolError> {
            Ok(self.translate(
                packet,
                self.source,
                &format!("{}<{}", self.source.0, self.target.0),
            ))
        }
    }

    fn translator_1_to_2() -> Box<dyn ProtocolTranslator> {
        Box::new(TestTranslator {
            source: ProtocolVersion(1),
            target: ProtocolVersion(2),
            fan_out: true,
        })
    }

    fn translator_2_to_3() -> Box<dyn ProtocolTranslator> {
        Box::new(TestTranslator {
            source: ProtocolVersion(2),
            target: ProtocolVersion(3),
            fan_out: false,
        })
    }

    #[test]
    fn selects_chain_and_translates_real_chat_packet_both_directions() {
        let registry = ProtocolTranslationRegistry::with_builtins();
        let mut pipeline = registry
            .build_shortest(ProtocolVersion(47), ProtocolVersion(107))
            .unwrap();
        assert_eq!(pipeline.len(), 1);
        let inbound = DynamicPacket {
            direction: PacketDirection::Inbound,
            state: DynamicProtocolState::Play,
            version: ProtocolVersion(47),
            packet_id: 0x02,
            packet_name: Some("clientbound_chat_message".into()),
            fields: serde_json::json!({"message":"{\"text\":\"hello\"}","position":0}),
            raw_payload: None,
        };
        let translated = pipeline.translate_inbound(inbound).unwrap();
        assert_eq!(translated[0].version, ProtocolVersion(107));
        assert_eq!(translated[0].packet_id, 0x0F);

        let outbound = DynamicPacket {
            direction: PacketDirection::Outbound,
            state: DynamicProtocolState::Play,
            version: ProtocolVersion(107),
            packet_id: 0x02,
            packet_name: Some("serverbound_chat_message".into()),
            fields: serde_json::json!({"message":"hello"}),
            raw_payload: None,
        };
        let translated = pipeline.translate_outbound(outbound).unwrap();
        assert_eq!(translated[0].version, ProtocolVersion(47));
        assert_eq!(translated[0].packet_id, 0x01);
    }

    #[test]
    fn fixed_vector_decodes_translates_and_encodes_v47_chat_as_v107() {
        let v47_payload = [
            0x10, b'{', b'"', b't', b'e', b'x', b't', b'"', b':', b'"', b'h', b'e', b'l', b'l',
            b'o', b'"', b'}', 0x00,
        ];
        let decoded = DynamicPacket::decode_supported_play(
            ProtocolVersion(47),
            PacketDirection::Inbound,
            0x02,
            &v47_payload,
        )
        .unwrap();
        let registry = ProtocolTranslationRegistry::with_builtins();
        let mut pipeline = registry
            .build_shortest(ProtocolVersion(47), ProtocolVersion(107))
            .unwrap();
        let translated = pipeline.translate_inbound(decoded).unwrap();
        assert_eq!(translated.len(), 1);
        let encoded = translated[0].encode_supported_play().unwrap();
        assert_eq!(encoded.packet_id, 0x0F);
        assert_eq!(encoded.payload, v47_payload);
    }

    #[test]
    fn disconnect_clears_all_connection_trackers() {
        let registry = ProtocolTranslationRegistry::with_builtins();
        let mut pipeline = registry
            .build_shortest(ProtocolVersion(47), ProtocolVersion(107))
            .unwrap();
        pipeline
            .context_mut()
            .entity_tracker
            .entity_types
            .insert(1, "minecraft:player".into());
        pipeline.disconnect();
        assert!(pipeline.context().entity_tracker.entity_types.is_empty());
        assert_eq!(pipeline.context().state, DynamicProtocolState::Handshake);
    }

    #[test]
    fn pipeline_tracks_protocol_state_transitions() {
        let registry = ProtocolTranslationRegistry::with_builtins();
        let mut pipeline = registry
            .build_shortest(ProtocolVersion(47), ProtocolVersion(107))
            .unwrap();
        assert_eq!(pipeline.source_version(), ProtocolVersion(47));
        assert_eq!(pipeline.target_version(), ProtocolVersion(107));
        pipeline.set_state(DynamicProtocolState::Login);
        assert_eq!(pipeline.context().state, DynamicProtocolState::Login);
        pipeline.set_state(DynamicProtocolState::Play);
        assert_eq!(pipeline.context().state, DynamicProtocolState::Play);
    }

    #[test]
    fn shortest_pipeline_supports_multi_hop_and_one_to_many_packets() {
        let mut registry = ProtocolTranslationRegistry::default();
        registry.register(ProtocolVersion(1), ProtocolVersion(2), translator_1_to_2);
        registry.register(ProtocolVersion(2), ProtocolVersion(3), translator_2_to_3);
        let mut pipeline = registry
            .build_shortest(ProtocolVersion(1), ProtocolVersion(3))
            .unwrap();
        assert_eq!(pipeline.len(), 2);

        let packet = DynamicPacket {
            direction: PacketDirection::Inbound,
            state: DynamicProtocolState::Play,
            version: ProtocolVersion(1),
            packet_id: 7,
            packet_name: Some("test".into()),
            fields: serde_json::json!({}),
            raw_payload: None,
        };
        let translated = pipeline.translate_inbound(packet).unwrap();
        assert_eq!(translated.len(), 2);
        assert_eq!(translated[0].version, ProtocolVersion(3));
        assert_eq!(translated[1].version, ProtocolVersion(3));
        assert_eq!(translated[0].packet_id, 7);
        assert_eq!(translated[1].packet_id, 8);
        assert_eq!(
            translated[0].fields["trace"],
            serde_json::json!(["1>2", "2>3"])
        );
    }

    #[test]
    fn outbound_translation_walks_the_multi_hop_pipeline_in_reverse() {
        let mut registry = ProtocolTranslationRegistry::default();
        registry.register(ProtocolVersion(1), ProtocolVersion(2), translator_1_to_2);
        registry.register(ProtocolVersion(2), ProtocolVersion(3), translator_2_to_3);
        let mut pipeline = registry
            .build_shortest(ProtocolVersion(1), ProtocolVersion(3))
            .unwrap();
        let packet = DynamicPacket {
            direction: PacketDirection::Outbound,
            state: DynamicProtocolState::Play,
            version: ProtocolVersion(3),
            packet_id: 9,
            packet_name: Some("test".into()),
            fields: serde_json::json!({}),
            raw_payload: None,
        };
        let translated = pipeline.translate_outbound(packet).unwrap();
        assert_eq!(translated.len(), 2);
        assert!(translated
            .iter()
            .all(|packet| packet.version == ProtocolVersion(1)));
        assert_eq!(
            translated[0].fields["trace"],
            serde_json::json!(["2<3", "1<2"])
        );
    }
}
