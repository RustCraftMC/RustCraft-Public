use crate::audio::{self, AudioBackend};
use crate::client::network::{MeshList, SignUpdateList};
use crate::client::particles::ParticleSystem;
use crate::client::player::Player;
use crate::net;
use crate::world;
use std::borrow::Cow;

/// Sound event names reused on hot packet paths. Keeping them as `const`
/// avoids re-allocating the same `&'static str` lookup each time the matching
/// packet is handled and makes the intent self-documenting.
const SOUND_EXPLODE: &str = "random.explode";

pub(super) fn handle_packet(
    world: &mut world::World,
    particles: &mut ParticleSystem,
    audio: &mut impl AudioBackend,
    player: &mut Player,
    dimension: i8,
    packet: Option<net::packet::ClientboundPacket>,
    meshes: &mut MeshList,
    sign_updates: &mut SignUpdateList,
) -> Option<net::packet::ClientboundPacket> {
    let packet = packet?;
    match packet {
        net::packet::ClientboundPacket::Particle {
            particle_id,
            x,
            y,
            z,
            offset_x,
            offset_y,
            offset_z,
            speed,
            count,
            data,
            ..
        } => {
            particles.spawn_from_server(
                particle_id,
                nalgebra::Point3::new(x, y, z),
                nalgebra::Vector3::new(offset_x, offset_y, offset_z),
                speed,
                count,
                &data,
            );
        }
        net::packet::ClientboundPacket::ChunkData {
            chunk_x,
            chunk_z,
            full_chunk,
            primary_bit_mask,
            data,
        } => {
            if full_chunk && primary_bit_mask == 0 {
                meshes.extend(world.unload_chunk(chunk_x, chunk_z));
            } else {
                meshes.extend(world.apply_chunk_data(
                    chunk_x,
                    chunk_z,
                    full_chunk,
                    primary_bit_mask,
                    &data,
                    super::chunk_options_for_dimension(dimension),
                ));
            }
        }
        net::packet::ClientboundPacket::MapChunkBulk { sky_light, chunks } => {
            for chunk in chunks {
                meshes.extend(world.apply_chunk_data(
                    chunk.chunk_x,
                    chunk.chunk_z,
                    true,
                    chunk.primary_bit_mask,
                    &chunk.data,
                    world::network::ChunkDataOptions::sky_light(sky_light),
                ));
            }
        }
        net::packet::ClientboundPacket::BlockChange {
            x,
            y,
            z,
            block_state,
        } => {
            meshes.extend(world.apply_block_change(x, y, z, block_state));
        }
        net::packet::ClientboundPacket::MultiBlockChange {
            chunk_x,
            chunk_z,
            records,
        } => {
            meshes.extend(world.apply_multi_block_change(chunk_x, chunk_z, &records));
        }
        net::packet::ClientboundPacket::BlockAction {
            x,
            y,
            z,
            byte1,
            byte2,
            block_type,
        } => {
            if byte1 == 1 && matches!(block_type, 54 | 130 | 146) {
                world.apply_chest_event(x, y, z, byte2);
            }
            // BlockPistonBase sends event 0 when extending. Vanilla installs a
            // TileEntityPiston here, which moves overlapping entities before
            // subsequent block-change packets update the static world.
            if byte1 == 0 && matches!(block_type, 29 | 33) {
                player.push_by_extending_piston(world, x, y, z, byte2);
            }
        }
        net::packet::ClientboundPacket::BlockBreakAnimation { .. } => {
            // S25 only updates another player's crack overlay. Vanilla does
            // not create EntityDiggingFX particles for this packet.
        }
        net::packet::ClientboundPacket::UpdateBlockEntity {
            x,
            y,
            z,
            action,
            nbt,
        } => {
            world.apply_block_entity_update(x, y, z, action, &nbt);
            // Parse sign tile entity NBT for text display
            if action == 9 {
                if let Ok(root) = crate::net::nbt::parse_root(&nbt) {
                    if let Some(compound) = root.as_compound() {
                        let text1 = nbt_str(compound, "Text1").unwrap_or_default();
                        let text2 = nbt_str(compound, "Text2").unwrap_or_default();
                        let text3 = nbt_str(compound, "Text3").unwrap_or_default();
                        let text4 = nbt_str(compound, "Text4").unwrap_or_default();
                        // Parse JSON text components to plain text
                        let lines = [
                            json_to_plain(&text1),
                            json_to_plain(&text2),
                            json_to_plain(&text3),
                            json_to_plain(&text4),
                        ];
                        sign_updates.push((x, y, z, lines));
                    }
                }
            }
        }
        net::packet::ClientboundPacket::Effect {
            effect_id,
            x,
            y,
            z,
            data,
            ..
        } => {
            super::effects::handle_world_effect(particles, audio, effect_id, [x, y, z], data);
        }
        net::packet::ClientboundPacket::Explosion {
            x,
            y,
            z,
            radius,
            records,
            player_motion,
        } => {
            // NetHandlerPlayClient.handleExplosion adds the packet's local
            // player impulse; it does not replace existing motion.
            log::debug!(
                target: "rustcraft::movement",
                "apply S27 explosion velocity: impulse=({:.6},{:.6},{:.6})",
                player_motion[0],
                player_motion[1],
                player_motion[2]
            );
            player.velocity.x += player_motion[0] as f64;
            player.velocity.y += player_motion[1] as f64;
            player.velocity.z += player_motion[2] as f64;
            particles.spawn_explosion(nalgebra::Point3::new(x, y, z), radius, records.len());
            let explode_name: Cow<'static, str> = Cow::Borrowed(SOUND_EXPLODE);
            audio.play(audio::SoundEvent {
                name: explode_name.into_owned(),
                category: audio::SoundCategory::Blocks,
                volume: 1.0,
                pitch: 1.0,
                position: Some([x, y, z]),
            });
        }
        net::packet::ClientboundPacket::NamedSoundEffect {
            name,
            x,
            y,
            z,
            volume,
            pitch,
        } => {
            audio.play(audio::SoundEvent {
                name,
                category: audio::SoundCategory::Blocks,
                volume,
                pitch,
                position: Some([x, y, z]),
            });
        }
        other => return Some(other),
    }
    None
}

fn nbt_str<'a>(
    compound: &'a std::collections::HashMap<String, crate::net::nbt::NbtTag>,
    key: &str,
) -> Option<Cow<'a, str>> {
    compound.get(key)?.as_str().map(Cow::Borrowed)
}

pub(crate) fn json_to_plain(raw: &str) -> String {
    json_to_plain_cow(raw).into_owned()
}

fn json_to_plain_cow(raw: &str) -> Cow<'_, str> {
    if raw.is_empty() {
        return Cow::Borrowed(raw);
    }
    fn append_component(value: &serde_json::Value, out: &mut String) {
        match value {
            serde_json::Value::String(text) => out.push_str(text),
            serde_json::Value::Array(items) => {
                for item in items {
                    append_component(item, out);
                }
            }
            serde_json::Value::Object(component) => {
                if let Some(text) = component.get("text").and_then(|text| text.as_str()) {
                    out.push_str(text);
                }
                // 1.8 JSON components place formatted fragments recursively in
                // `extra`; preserve their visible text even though colour/style
                // support is not yet represented by the sign texture.
                if let Some(extra) = component.get("extra") {
                    append_component(extra, out);
                }
            }
            _ => {}
        }
    }

    serde_json::from_str::<serde_json::Value>(raw)
        .map(|value| {
            let mut out = String::new();
            append_component(&value, &mut out);
            Cow::Owned(out)
        })
        .unwrap_or_else(|_| Cow::Borrowed(raw))
}

#[cfg(test)]
mod tests {
    use super::json_to_plain;

    #[test]
    fn json_text_primitives_do_not_render_their_quotes() {
        assert_eq!(json_to_plain("\"\""), "");
        assert_eq!(json_to_plain("\"Welcome\""), "Welcome");
    }

    #[test]
    fn json_text_components_preserve_visible_fragments() {
        assert_eq!(
            json_to_plain(r#"[{"text":"Hello"},{"text":" world"}]"#),
            "Hello world"
        );
    }
}
