use crate::audio::AudioBackend;
use crate::client::session::SessionState;
use crate::entity::{Entity, EntityEffectState, EntityManager, EntityType};
use crate::net;

pub(super) fn handle_packet(
    entities: &mut EntityManager,
    session: &mut SessionState,
    player: &mut crate::client::player::Player,
    audio: &mut impl AudioBackend,
    packet: Option<net::packet::ClientboundPacket>,
) -> Option<net::packet::ClientboundPacket> {
    let packet = packet?;
    match packet {
        net::packet::ClientboundPacket::Animation {
            entity_id,
            animation,
        } => {
            if let Some(entity) = entities.get_mut(entity_id) {
                entity.apply_animation(animation);
            }
        }
        net::packet::ClientboundPacket::EntityEquipment {
            entity_id,
            slot,
            item,
        } => {
            if let Some(entity) = entities.get_mut(entity_id) {
                entity.set_equipment(slot, item);
            }
        }
        net::packet::ClientboundPacket::EntitySpawn {
            entity_id,
            spawn_kind,
            entity_type,
            uuid,
            current_item,
            object_data,
            x,
            y,
            z,
            yaw,
            pitch,
            head_yaw,
            velocity,
            metadata,
        } => {
            let entity_kind = match spawn_kind {
                net::packet::EntitySpawnKind::Player => EntityType::Player,
                net::packet::EntitySpawnKind::Object => {
                    EntityType::from_object_id(entity_type, object_data)
                }
                net::packet::EntitySpawnKind::Mob => EntityType::from_id(entity_type),
            };
            let mut entity = Entity::new(entity_id, entity_kind, nalgebra::Point3::new(x, y, z));
            entity.uuid = uuid;
            entity.current_item = (current_item >= 0).then_some(current_item);
            if entity_kind == EntityType::Player {
                let profile = entity
                    .uuid
                    .as_deref()
                    .and_then(|uuid| session.player_list.get(uuid));
                let name = profile
                    .map(|player| player.name.clone())
                    .unwrap_or_else(|| "Player".to_string());
                let skin_property = profile.and_then(|player| player.skin_property.clone());
                entity.data = crate::entity::EntityData::Player {
                    name,
                    gamemode: 0,
                    skin_property,
                };
            }
            entity.yaw = yaw;
            entity.body_yaw = yaw;
            entity.pitch = pitch;
            entity.head_yaw = head_yaw;
            entity.velocity = nalgebra::Vector3::new(velocity[0], velocity[1], velocity[2]);
            entity.apply_metadata(metadata);
            entities.spawn(entity);
        }
        net::packet::ClientboundPacket::ExperienceOrbSpawn {
            entity_id,
            x,
            y,
            z,
            count,
        } => {
            let mut entity =
                Entity::new(entity_id, EntityType::XPOrb, nalgebra::Point3::new(x, y, z));
            entity.data = crate::entity::EntityData::XPOrb {
                value: count as i32,
            };
            entities.spawn(entity);
        }
        net::packet::ClientboundPacket::EntityMove {
            entity_id,
            dx,
            dy,
            dz,
            on_ground,
        } => {
            if let Some(entity) = entities.get_mut(entity_id) {
                entity.move_relative(dx, dy, dz, 3);
                entity.on_ground = on_ground;
            }
        }
        net::packet::ClientboundPacket::EntityLook {
            entity_id,
            yaw,
            pitch,
            on_ground,
        } => {
            if let Some(entity) = entities.get_mut(entity_id) {
                entity.set_remote_rotation(yaw, pitch, 3);
                entity.on_ground = on_ground;
            }
        }
        net::packet::ClientboundPacket::EntityMoveLook {
            entity_id,
            dx,
            dy,
            dz,
            yaw,
            pitch,
            on_ground,
        } => {
            if let Some(entity) = entities.get_mut(entity_id) {
                if entity.move_relative(dx, dy, dz, 3) {
                    entity.set_remote_rotation(yaw, pitch, 3);
                }
                entity.on_ground = on_ground;
            }
        }
        net::packet::ClientboundPacket::EntityTeleport {
            entity_id,
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
        } => {
            if let Some(entity) = entities.get_mut(entity_id) {
                if entity.teleport(nalgebra::Point3::new(x, y, z), 3) {
                    entity.set_remote_rotation(yaw, pitch, 3);
                }
                entity.on_ground = on_ground;
            }
        }
        net::packet::ClientboundPacket::EntityHeadLook {
            entity_id,
            head_yaw,
        } => {
            if let Some(entity) = entities.get_mut(entity_id) {
                entity.head_yaw = head_yaw;
            }
        }
        net::packet::ClientboundPacket::EntityVelocity {
            entity_id,
            vx,
            vy,
            vz,
        } => {
            // The local player is deliberately not stored in EntityManager.
            // Vanilla NetHandlerPlayClient still resolves it from the world and
            // calls Entity.setVelocity, which *replaces* all three motion axes.
            if session.entity_id == Some(entity_id) {
                log::debug!(
                    target: "rustcraft::movement",
                    "apply S12 local velocity: entity_id={entity_id}, velocity=({vx:.6},{vy:.6},{vz:.6}), previous=({:.6},{:.6},{:.6}), on_ground={}, sprinting={}, movement_sprinting={}",
                    player.velocity.x,
                    player.velocity.y,
                    player.velocity.z,
                    player.on_ground,
                    player.sprinting,
                    player.movement_sprinting(),
                );
                player.velocity = nalgebra::Vector3::new(vx, vy, vz);
            } else if let Some(entity) = entities.get_mut(entity_id) {
                entity.velocity = nalgebra::Vector3::new(vx as f32, vy as f32, vz as f32);
            }
        }
        net::packet::ClientboundPacket::CollectItem {
            collected_entity_id,
            collector_entity_id: _,
        } => {
            entities.despawn(collected_entity_id);
        }
        net::packet::ClientboundPacket::EntityStatus { entity_id, status } => {
            // Status 2 = entity hurt (MC 1.8.9)
            // Play the entity's hurt sound with random pitch, matching vanilla behavior:
            // pitch = (rand - rand) * 0.2 + 1.0
            if status == 2 {
                if session.entity_id == Some(entity_id) {
                    player.trigger_hurt();
                    let pitch = (rand_f32() - rand_f32()) * 0.2 + 1.0;
                    audio.play(crate::audio::SoundEvent {
                        name: "game.player.hurt".to_string(),
                        category: crate::audio::SoundCategory::Players,
                        volume: 1.0,
                        pitch,
                        position: Some([
                            player.position.x as f32,
                            player.position.y as f32,
                            player.position.z as f32,
                        ]),
                    });
                }
                if let Some(entity) = entities.get(entity_id) {
                    let hurt_sound = entity_hurt_sound(entity.entity_type);
                    let pitch = (rand_f32() - rand_f32()) * 0.2 + 1.0;
                    audio.play(crate::audio::SoundEvent {
                        name: hurt_sound.to_string(),
                        category: crate::audio::SoundCategory::Hostile,
                        volume: 1.0,
                        pitch,
                        position: Some([entity.position.x, entity.position.y, entity.position.z]),
                    });
                }
            }
            // Vanilla EntityPlayer.handleStatusUpdate(9) → onItemUseFinish():
            // the server sends S19 status 9 when food/potion consumption is
            // complete.  The client must clear its local item-use state so
            // movement returns to full speed on the next tick.
            if status == 9 && session.entity_id == Some(entity_id) {
                player.on_item_use_finished();
            }
            if let Some(entity) = entities.get_mut(entity_id) {
                entity.apply_status(status);
            }
        }
        net::packet::ClientboundPacket::AttachEntity {
            entity_id,
            vehicle_id,
            leash,
        } => {
            if !leash && session.entity_id == Some(entity_id) {
                if let Some(previous_vehicle) = player.vehicle_id {
                    if let Some(boat) = entities.get_mut(previous_vehicle) {
                        boat.set_boat_empty(true);
                    }
                }
                player.vehicle_id = (vehicle_id >= 0).then_some(vehicle_id);
                if let Some(boat) = entities.get_mut(vehicle_id) {
                    boat.set_boat_empty(false);
                }
            } else if let Some(entity) = entities.get_mut(entity_id) {
                entity.set_attachment(vehicle_id, leash);
            }
        }
        net::packet::ClientboundPacket::EntityMetadata {
            entity_id,
            metadata,
        } => {
            if let Some(entity) = entities.get_mut(entity_id) {
                entity.apply_metadata(metadata);
            }
        }
        net::packet::ClientboundPacket::EntityEffect {
            entity_id,
            effect_id,
            amplifier,
            duration,
            hide_particles,
        } => {
            if session.entity_id == Some(entity_id) {
                player.add_effect(EntityEffectState {
                    effect_id,
                    amplifier,
                    duration,
                    hide_particles,
                });
            } else if let Some(entity) = entities.get_mut(entity_id) {
                entity.add_effect(EntityEffectState {
                    effect_id,
                    amplifier,
                    duration,
                    hide_particles,
                });
            }
        }
        net::packet::ClientboundPacket::RemoveEntityEffect {
            entity_id,
            effect_id,
        } => {
            if session.entity_id == Some(entity_id) {
                player.remove_effect(effect_id);
            } else if let Some(entity) = entities.get_mut(entity_id) {
                entity.remove_effect(effect_id);
            }
        }
        net::packet::ClientboundPacket::EntityProperties {
            entity_id,
            properties,
        } => {
            if session.entity_id == Some(entity_id) {
                player.apply_entity_properties(properties);
            } else if let Some(entity) = entities.get_mut(entity_id) {
                entity.apply_properties(properties);
            }
        }
        net::packet::ClientboundPacket::DestroyEntities { ids } => {
            entities.despawn_batch(&ids);
        }
        other => return Some(other),
    }
    None
}

/// Map entity type to its MC 1.8 hurt sound event name.
fn entity_hurt_sound(entity_type: crate::entity::EntityType) -> &'static str {
    use crate::entity::EntityType;
    match entity_type {
        EntityType::Zombie | EntityType::PigZombie => "game.neutral.hurt",
        EntityType::Skeleton
        | EntityType::Creeper
        | EntityType::Spider
        | EntityType::CaveSpider
        | EntityType::Enderman
        | EntityType::Witch
        | EntityType::Silverfish
        | EntityType::Blaze
        | EntityType::LavaSlime
        | EntityType::Ghast
        | EntityType::Slime
        | EntityType::Guardian
        | EntityType::WitherBoss
        | EntityType::Bat
        | EntityType::IronGolem
        | EntityType::SnowMan
        | EntityType::EnderDragon
        | EntityType::Endermite
        | EntityType::Giant => "game.neutral.hurt",

        EntityType::Villager
        | EntityType::Cow
        | EntityType::Pig
        | EntityType::Sheep
        | EntityType::Chicken
        | EntityType::Horse
        | EntityType::Wolf
        | EntityType::Ocelot
        | EntityType::Rabbit
        | EntityType::Squid
        | EntityType::Mooshroom => "game.neutral.hurt",

        EntityType::Player => "game.player.hurt",

        _ => "game.neutral.hurt",
    }
}

/// Simple pseudo-random f32 for pitch variation.
static mut RAND_STATE: u32 = 67890;
fn rand_f32() -> f32 {
    unsafe {
        RAND_STATE ^= RAND_STATE << 13;
        RAND_STATE ^= RAND_STATE >> 17;
        RAND_STATE ^= RAND_STATE << 5;
        (RAND_STATE as f32) / (u32::MAX as f32)
    }
}
