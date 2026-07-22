use crate::client::player::Player;
use crate::client::session::{ResourcePackOffer, SessionState};
use crate::entity::EntityManager;
use crate::net;
use crate::world;

pub(super) fn handle_packet(
    connection: &mut net::connection::Connection,
    player: &mut Player,
    network_state: &mut super::ClientNetworkState,
    world: &mut world::World,
    session: &mut SessionState,
    entities: &mut EntityManager,
    packet: Option<net::packet::ClientboundPacket>,
) -> Option<net::packet::ClientboundPacket> {
    let packet = packet?;
    match packet {
        net::packet::ClientboundPacket::KeepAlive { id } => {
            connection.send_play_packet(0x00, &net::packet::write_keep_alive(id));
        }
        net::packet::ClientboundPacket::JoinGame {
            entity_id,
            gamemode,
            dimension,
            difficulty,
            max_players,
            level_type,
            reduced_debug,
        } => {
            log::info!(
                target: "rustcraft::gameplay",
                "joined game: entity_id={entity_id}, gamemode={gamemode}, dimension={dimension}, difficulty={difficulty}, max_players={max_players}, level_type={}, reduced_debug={reduced_debug}",
                crate::logging::event_text(&level_type)
            );
            world.clear_server_world();
            entities.despawn_all();
            session.entity_id = Some(entity_id);
            session.set_gamemode(gamemode);
            session.dimension = dimension;
            session.difficulty = difficulty;
            session.max_players = max_players;
            session.level_type = level_type;
            session.reduced_debug = reduced_debug;
            connection.send_play_packet(
                0x15,
                &net::packet::write_client_settings(
                    session.locale.as_str(),
                    session.view_distance as i8,
                    0,
                    true,
                    session.skin_parts,
                ),
            );
            connection.send_play_packet(0x17, &net::packet::write_brand("RustCraft"));
        }
        net::packet::ClientboundPacket::Disconnect { reason } => {
            let reason_text =
                crate::client::session::plain_text(&reason).unwrap_or_else(|| reason.clone());
            log::warn!(
                target: "rustcraft::gameplay",
                "disconnected by server: {}",
                crate::logging::event_text(&reason_text)
            );
            session.last_disconnect_reason = Some(reason.clone());
            session.push_chat_json(&reason, 0);
            connection
                .connected
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
        net::packet::ClientboundPacket::Respawn {
            dimension,
            difficulty,
            gamemode,
            level_type,
        } => {
            log::info!(
                target: "rustcraft::gameplay",
                "respawn: dimension={dimension}, difficulty={difficulty}, gamemode={gamemode}, level_type={}",
                crate::logging::event_text(&level_type)
            );
            world.clear_server_world();
            entities.despawn_all();
            session.dimension = dimension as i8;
            session.difficulty = difficulty;
            session.set_gamemode(gamemode);
            session.level_type = level_type;
        }
        net::packet::ClientboundPacket::ChatMessage { json, position } => {
            session.push_chat_json(&json, position);
        }
        net::packet::ClientboundPacket::TimeUpdate {
            world_time,
            day_time,
        } => {
            session.world_time = world_time;
            session.day_time = day_time;
        }
        net::packet::ClientboundPacket::SpawnPosition { x, y, z } => {
            session.spawn_position = Some([x, y, z]);
        }
        net::packet::ClientboundPacket::UpdateHealth {
            health,
            food,
            saturation,
        } => {
            let previous_health = session.health;
            log::debug!(
                target: "rustcraft::gameplay",
                "health update: health={health}, previous_health={previous_health}, food={food}, saturation={saturation}"
            );
            if previous_health > 0.0 && health <= 0.0 {
                log::info!(target: "rustcraft::gameplay", "local player died");
            }
            session.health = health;
            session.food = food;
            session.saturation = saturation;
            player.food_level = food;
            if health <= 0.0 {
                player.flying = false;
            }
        }
        net::packet::ClientboundPacket::SetExperience { bar, level, total } => {
            session.experience_bar = bar;
            session.experience_level = level;
            session.experience_total = total;
        }
        net::packet::ClientboundPacket::PlayerPositionAndLook {
            x,
            y,
            z,
            yaw,
            pitch,
            flags,
        } => {
            // NetHandlerPlayClient.handlePlayerPosLook clears velocity on each
            // absolute axis before acknowledging the correction. Retaining an
            // old velocity here causes the local player to immediately move
            // away from the accepted server position and repeatedly rubberband.
            if flags & 0x01 == 0 {
                player.velocity.x = 0.0;
            }
            if flags & 0x02 == 0 {
                player.velocity.y = 0.0;
            }
            if flags & 0x04 == 0 {
                player.velocity.z = 0.0;
            }
            let px = if flags & 0x01 != 0 {
                player.position.x as f64 + x
            } else {
                x
            };
            let py = if flags & 0x02 != 0 {
                player.position.y as f64 + y
            } else {
                y
            };
            let pz = if flags & 0x04 != 0 {
                player.position.z as f64 + z
            } else {
                z
            };
            let pyaw = if flags & 0x08 != 0 {
                player.camera.mc_yaw_degrees() + yaw
            } else {
                yaw
            };
            let ppitch = if flags & 0x10 != 0 {
                player.camera.mc_pitch_degrees() + pitch
            } else {
                pitch
            };

            player.position = nalgebra::Point3::new(px, py, pz);
            player.camera.position = nalgebra::Point3::new(
                px as f32,
                (py + crate::client::physics::PLAYER_EYE_HEIGHT) as f32,
                pz as f32,
            );
            player.camera.set_from_mc_yaw(pyaw);
            player.camera.set_from_mc_pitch(ppitch);
            log::debug!(
                "initial player position: pos=({:.6},{:.6},{:.6}), on_ground={}, velocity=({:.6},{:.6},{:.6})",
                px,
                py,
                pz,
                player.on_ground,
                player.velocity.x,
                player.velocity.y,
                player.velocity.z
            );
            let payload =
                net::packet::write_player_position_and_look(px, py, pz, pyaw, ppitch, false);
            connection.send_play_packet(0x06, &payload);
            network_state.synchronize_after_server_correction(player);
            session.received_initial_position = true;
        }
        net::packet::ClientboundPacket::HeldItemChange { slot } => {
            return Some(net::packet::ClientboundPacket::HeldItemChange { slot });
        }
        net::packet::ClientboundPacket::ServerDifficulty { difficulty } => {
            session.difficulty = difficulty;
        }
        net::packet::ClientboundPacket::ChangeGameState { reason, value } => {
            session.apply_game_state(reason, value);
        }
        net::packet::ClientboundPacket::PluginMessage { channel, data } => {
            session.apply_plugin_message(channel, data);
        }
        net::packet::ClientboundPacket::TabComplete { matches } => {
            session.apply_tab_complete(matches);
        }
        net::packet::ClientboundPacket::SignEditorOpen { x, y, z } => {
            session.open_sign_editor(x, y, z);
        }
        net::packet::ClientboundPacket::CombatEvent { event } => {
            session.apply_combat_event(event);
        }
        net::packet::ClientboundPacket::ResourcePackSend { url, hash } => {
            log::info!(
                target: "rustcraft::gameplay",
                "server offered resource pack: url_length={}, hash={}",
                url.len(),
                crate::logging::event_text(&hash)
            );
            session.resource_pack = Some(ResourcePackOffer {
                url,
                hash,
                status: "available".to_string(),
            });
            session.push_system_line(session.text.resource_pack_offered.clone());
        }
        net::packet::ClientboundPacket::Camera { camera_id } => {
            session.camera_entity_id = Some(camera_id);
        }
        net::packet::ClientboundPacket::UseBed { entity_id, x, y, z } => {
            session.last_bed_use = Some((entity_id, [x, y, z]));
        }
        net::packet::ClientboundPacket::PlayerAbilities {
            flags,
            flying_speed,
            walking_speed,
        } => {
            session.ability_flags = flags;
            session.flying_speed = flying_speed;
            session.walking_speed = walking_speed;
            player.fly_speed = flying_speed;
            player.flying = flags & 0x02 != 0;
            player.allow_flying = flags & 0x04 != 0;
        }
        net::packet::ClientboundPacket::PlayerListItem { action, players } => {
            retain_spawned_player_profiles(entities, &action, &players);
            session.apply_player_list_item(action, players);
        }
        net::packet::ClientboundPacket::Statistics { entries } => {
            session.apply_statistics(entries);
        }
        net::packet::ClientboundPacket::PlayerListHeaderFooter {
            header_json,
            footer_json,
        } => {
            session.set_tab_header_footer(header_json, footer_json);
        }
        net::packet::ClientboundPacket::ScoreboardObjective {
            name,
            mode,
            value,
            render_type,
        } => {
            session.apply_scoreboard_objective(name, mode, value, render_type);
        }
        net::packet::ClientboundPacket::UpdateScore {
            item_name,
            action,
            score_name,
            value,
        } => {
            session.apply_update_score(item_name, action, score_name, value);
        }
        net::packet::ClientboundPacket::DisplayScoreboard {
            position,
            score_name,
        } => {
            session.apply_display_scoreboard(position, score_name);
        }
        net::packet::ClientboundPacket::Teams {
            name,
            mode,
            display_name,
            prefix,
            suffix,
            friendly_flags,
            name_tag_visibility,
            color,
            players,
        } => {
            session.apply_team(
                name,
                mode,
                display_name,
                prefix,
                suffix,
                friendly_flags,
                name_tag_visibility,
                color,
                players,
            );
        }
        net::packet::ClientboundPacket::Title {
            action,
            text_json,
            fade_in,
            stay,
            fade_out,
        } => {
            session.apply_title(action, text_json, fade_in, stay, fade_out);
        }
        net::packet::ClientboundPacket::UpdateSign { x, y, z, lines } => {
            // UpdateSign is the normal 1.8.9 sign update path.  Its four
            // strings are JSON chat components just like Text1..Text4 in a
            // block-entity update, so decode them consistently.
            session.sign_data.insert(
                (x, y, z),
                lines.map(|line| super::world_packets::json_to_plain(&line)),
            );
        }
        net::packet::ClientboundPacket::WorldBorder { update, .. } => {
            session.apply_world_border(update);
        }
        other => return Some(other),
    }
    None
}

/// Vanilla constructs EntityOtherPlayerMP from NetworkPlayerInfo's GameProfile.
/// Keep the same profile data on an already spawned entity so a later tab-list
/// removal cannot discard the skin, and so unusual add-after-spawn ordering works.
fn retain_spawned_player_profiles(
    entities: &mut EntityManager,
    action: &net::packet::PlayerListAction,
    players: &[net::packet::PlayerListEntry],
) {
    if !matches!(action, net::packet::PlayerListAction::AddPlayer) {
        return;
    }

    for player in players {
        let skin_property = player
            .properties
            .iter()
            .find(|property| property.name == "textures")
            .map(|property| property.value.clone());
        for (_, mut entity) in entities
            .iter_mut()
            .into_iter()
            .filter(|(_, entity)| entity.uuid.as_deref() == Some(player.uuid.as_str()))
        {
            let crate::entity::EntityData::Player {
                name,
                skin_property: entity_skin_property,
                ..
            } = &mut entity.data
            else {
                continue;
            };
            if let Some(player_name) = &player.name {
                name.clone_from(player_name);
            }
            entity_skin_property.clone_from(&skin_property);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Entity, EntityData, EntityType};
    use crate::net::player_list::PlayerProperty;

    #[test]
    fn add_player_profile_is_retained_by_an_existing_entity() {
        let uuid = "12345678-1234-5678-9abc-def012345678";
        let mut entities = EntityManager::new();
        let mut entity = Entity::new(7, EntityType::Player, nalgebra::Point3::origin());
        entity.uuid = Some(uuid.to_string());
        entity.data = EntityData::Player {
            name: "Player".to_string(),
            gamemode: 0,
            skin_property: None,
        };
        entities.spawn(entity);
        let entry = net::packet::PlayerListEntry {
            uuid: uuid.to_string(),
            name: Some("RemotePlayer".to_string()),
            properties: vec![PlayerProperty {
                name: "textures".to_string(),
                value: "encoded-skin".to_string(),
                signature: Some("signature".to_string()),
            }],
            gamemode: Some(0),
            ping: Some(20),
            display_name_json: None,
        };

        retain_spawned_player_profiles(
            &mut entities,
            &net::packet::PlayerListAction::AddPlayer,
            &[entry.clone()],
        );
        retain_spawned_player_profiles(
            &mut entities,
            &net::packet::PlayerListAction::RemovePlayer,
            &[entry],
        );

        let EntityData::Player {
            name,
            skin_property,
            ..
        } = &entities.get(7).unwrap().data
        else {
            panic!("expected player entity");
        };
        assert_eq!(name, "RemotePlayer");
        assert_eq!(skin_property.as_deref(), Some("encoded-skin"));
    }
}
