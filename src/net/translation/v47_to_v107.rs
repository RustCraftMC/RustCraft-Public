use crate::net::dynamic_packet::{DynamicPacket, PacketDirection, ProtocolVersion};

use super::{ProtocolError, ProtocolTranslator, TranslationContext};

pub struct V47ToV107Translator;

impl ProtocolTranslator for V47ToV107Translator {
    fn source_version(&self) -> ProtocolVersion {
        ProtocolVersion(47)
    }

    fn target_version(&self) -> ProtocolVersion {
        ProtocolVersion(107)
    }

    fn translate_inbound(
        &mut self,
        mut packet: DynamicPacket,
        context: &mut TranslationContext,
    ) -> Result<Vec<DynamicPacket>, ProtocolError> {
        if packet.version != self.source_version() || packet.direction != PacketDirection::Inbound {
            return Err(ProtocolError(
                "v47→v107 inbound packet has wrong envelope".into(),
            ));
        }
        packet.version = self.target_version();
        if packet.packet_name.as_deref() == Some("clientbound_chat_message") {
            require_fields(&packet, &["message", "position"])?;
            packet.packet_id = 0x0F;
        }
        context.state = packet.state;
        Ok(vec![packet])
    }

    fn translate_outbound(
        &mut self,
        mut packet: DynamicPacket,
        context: &mut TranslationContext,
    ) -> Result<Vec<DynamicPacket>, ProtocolError> {
        if packet.version != self.target_version() || packet.direction != PacketDirection::Outbound
        {
            return Err(ProtocolError(
                "v107→v47 outbound packet has wrong envelope".into(),
            ));
        }
        packet.version = self.source_version();
        if packet.packet_name.as_deref() == Some("serverbound_chat_message") {
            require_fields(&packet, &["message"])?;
            packet.packet_id = 0x01;
        }
        context.state = packet.state;
        Ok(vec![packet])
    }
}

fn require_fields(packet: &DynamicPacket, fields: &[&str]) -> Result<(), ProtocolError> {
    let object = packet
        .fields
        .as_object()
        .ok_or_else(|| ProtocolError("packet fields must be an object".into()))?;
    for field in fields {
        if !object.contains_key(*field) {
            return Err(ProtocolError(format!("missing packet field '{field}'")));
        }
    }
    Ok(())
}
