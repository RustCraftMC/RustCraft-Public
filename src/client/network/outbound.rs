use crate::client::inventory::ItemStack;
use crate::client::physics::BlockFace;
use crate::client::player::Player;
use crate::net;

pub struct ClientNetworkState {
    last_position: nalgebra::Point3<f64>,
    last_yaw: f32,
    last_pitch: f32,
    last_sneaking: bool,
    last_sprinting: bool,
    last_held_item: usize,
    ticks_since_full: u32,
    initialized: bool,
    force_sync: bool,
}

impl ClientNetworkState {
    pub fn new() -> Self {
        Self {
            last_position: nalgebra::Point3::new(0.0, 0.0, 0.0),
            last_yaw: 0.0,
            last_pitch: 0.0,
            last_sneaking: false,
            last_sprinting: false,
            // PlayerControllerMP.currentPlayerItem defaults to hotbar slot zero.
            last_held_item: 0,
            ticks_since_full: 20,
            initialized: false,
            force_sync: false,
        }
    }

    /// A server S08 position correction becomes the new vanilla
    /// `lastReported*` baseline after its mandatory C06 acknowledgement.
    /// Without this, the next tick replays the pre-correction displacement and
    /// anti-cheat sees an immediate invalid move after a teleport/rubberband.
    pub fn synchronize_after_server_correction(&mut self, player: &Player) {
        self.last_position = player.position;
        self.last_yaw = player.camera.mc_yaw_degrees();
        self.last_pitch = player.camera.mc_pitch_degrees();
        self.last_sneaking = player.sneaking;
        self.last_sprinting = player.movement_sprinting();
        self.ticks_since_full = 0;
        self.initialized = true;
        self.force_sync = true;
    }
}

pub fn send_chat_message(connection: &Option<net::connection::Connection>, message: &str) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(0x01, &net::packet::write_chat_message(message));
        log_outbound_chat(message);
    }
}

pub fn send_book_update(
    connection: &Option<net::connection::Connection>,
    signed: bool,
    stack: &net::slot::Slot,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }
    let mut book = net::protocol::PacketBuffer::empty();
    net::slot::write_slot(&mut book, stack);
    let book = book.into_inner();
    if book.len() > 32_767 {
        log::warn!("book payload exceeds the 1.8.9 custom-payload limit");
        return;
    }
    let channel = if signed { "MC|BSign" } else { "MC|BEdit" };
    connection.send_play_packet(0x17, &net::packet::write_plugin_message(channel, &book));
}

pub fn send_dynamic_packet(
    connection: &Option<net::connection::Connection>,
    packet: &crate::net::dynamic_packet::DynamicPacket,
) -> std::io::Result<()> {
    let encoded = packet.encode_v47_serverbound()?;
    let Some(connection) = connection else {
        return Ok(());
    };
    if connection.state != net::packet::ProtocolState::Play {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "connection is not in play state",
        ));
    }
    connection.send_play_packet(encoded.packet_id, &encoded.payload);
    if packet.packet_name.as_deref() == Some("serverbound_chat_message") {
        if let Some(message) = packet
            .fields
            .get("message")
            .and_then(|value| value.as_str())
        {
            log_outbound_chat(message);
        }
    }
    Ok(())
}

fn log_outbound_chat(message: &str) {
    let kind = if message.starts_with('/') {
        "command"
    } else {
        "chat"
    };
    log::info!(
        target: "rustcraft::chat",
        "outgoing {kind}: {}",
        crate::logging::outbound_chat_text(message)
    );
}

pub fn send_held_item_change(connection: &Option<net::connection::Connection>, selected: usize) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(
            0x09,
            &net::packet::write_held_item_change(selected.min(8) as i16),
        );
    }
}

pub fn send_close_window(connection: &Option<net::connection::Connection>, window_id: u8) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(0x0D, &net::packet::write_close_window(window_id));
    }
}

pub fn send_confirm_transaction(
    connection: &Option<net::connection::Connection>,
    window_id: u8,
    action_number: i16,
    accepted: bool,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(
            0x0F,
            &net::packet::write_confirm_transaction(window_id, action_number, accepted),
        );
    }
}

pub fn send_creative_inventory_action(
    connection: &Option<net::connection::Connection>,
    slot: i16,
    stack: &ItemStack,
) {
    send_creative_inventory_action_slot(connection, slot, &stack.to_protocol_slot());
}

pub fn send_creative_inventory_action_slot(
    connection: &Option<net::connection::Connection>,
    slot: i16,
    clicked_item: &net::slot::Slot,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(
            0x10,
            &net::packet::write_creative_inventory_action(slot, clicked_item),
        );
    }
}

pub fn send_enchant_item(
    connection: &Option<net::connection::Connection>,
    window_id: u8,
    enchantment: u8,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(
            0x11,
            &net::packet::write_enchant_item(window_id, enchantment),
        );
    }
}

pub fn send_update_sign(
    connection: &Option<net::connection::Connection>,
    pos: (i32, i32, i32),
    lines: [&str; 4],
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(
            0x12,
            &net::packet::write_update_sign(pos.0, pos.1, pos.2, lines),
        );
    }
}

pub fn send_tab_complete(
    connection: &Option<net::connection::Connection>,
    text: &str,
    block: Option<(i32, i32, i32)>,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(0x14, &net::packet::write_tab_complete(text, block));
    }
}

pub fn send_client_settings(
    connection: &Option<net::connection::Connection>,
    locale: &str,
    view_distance: u8,
    skin_parts: u8,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(
            0x15,
            &net::packet::write_client_settings(
                locale,
                view_distance.min(16) as i8,
                0,
                true,
                skin_parts,
            ),
        );
    }
}

pub fn send_resource_pack_status(
    connection: &Option<net::connection::Connection>,
    hash: &str,
    result: i32,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(0x19, &net::packet::write_resource_pack_status(hash, result));
        let status = match result {
            0 => "successfully_loaded",
            1 => "declined",
            2 => "failed_download",
            3 => "accepted",
            _ => "unknown",
        };
        log::info!(
            target: "rustcraft::gameplay",
            "resource pack response: status={status}, result={result}, hash={}",
            crate::logging::event_text(hash)
        );
    }
}

pub fn send_drop_selected_item(connection: &Option<net::connection::Connection>, drop_stack: bool) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }

    let status = if drop_stack { 3 } else { 4 };
    connection.send_play_packet(0x07, &net::packet::write_player_digging(status, 0, 0, 0, 0));
}

pub fn send_use_entity_attack(connection: &Option<net::connection::Connection>, entity_id: i32) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }

    log::trace!(
        target: "rustcraft::interaction",
        "send C0A animation before C02 attack: entity_id={entity_id}"
    );
    // EntityPlayerSP.swingItem always sends C0A on the client — the
    // isSwingInProgress gate in EntityLivingBase.swingItem only affects
    // the server-side S0B broadcast and the local animation state.
    // The C02 attack packet is sent alongside unconditionally.
    connection.send_play_packet(0x0A, &net::packet::write_animation());
    connection.send_play_packet(0x02, &net::packet::write_use_entity(entity_id, 1));
}

pub fn send_use_entity_interact(connection: &Option<net::connection::Connection>, entity_id: i32) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }
    log::trace!(
        target: "rustcraft::interaction",
        "send C02 interact: entity_id={entity_id}"
    );
    connection.send_play_packet(0x02, &net::packet::write_use_entity(entity_id, 0));
}

pub fn send_use_entity_interact_at(
    connection: &Option<net::connection::Connection>,
    entity_id: i32,
    target: [f32; 3],
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(
            0x02,
            &net::packet::write_use_entity_interact_at(entity_id, target),
        );
    }
}

pub fn send_client_status(connection: &Option<net::connection::Connection>, action_id: i32) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(0x16, &net::packet::write_client_status(action_id));
    }
}

pub fn send_respawn_request(connection: &Option<net::connection::Connection>) {
    send_client_status(connection, 0);
}

pub fn send_player_abilities(
    connection: &Option<net::connection::Connection>,
    flags: u8,
    flying_speed: f32,
    walking_speed: f32,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        connection.send_play_packet(
            0x13,
            &net::packet::write_player_abilities(flags, flying_speed, walking_speed),
        );
    }
}

pub fn send_digging_start(
    connection: &Option<net::connection::Connection>,
    pos: (i32, i32, i32),
    face: BlockFace,
) {
    send_digging_status(connection, 0, pos, face);
}

pub fn send_digging_cancel(
    connection: &Option<net::connection::Connection>,
    pos: (i32, i32, i32),
    face: BlockFace,
) {
    send_digging_status(connection, 1, pos, face);
}

pub fn send_digging_finish(
    connection: &Option<net::connection::Connection>,
    pos: (i32, i32, i32),
    face: BlockFace,
) {
    send_digging_status(connection, 2, pos, face);
}

fn send_digging_status(
    connection: &Option<net::connection::Connection>,
    status: u8,
    pos: (i32, i32, i32),
    face: BlockFace,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }

    let face = protocol_face(face);
    let action = match status {
        0 => "start",
        1 => "abort",
        2 => "stop",
        _ => "unknown",
    };
    log::trace!(
        target: "rustcraft::interaction",
        "send C07 digging: action={action}, status={status}, pos=({},{},{}), face={face}",
        pos.0,
        pos.1,
        pos.2
    );
    connection.send_play_packet(
        0x07,
        &net::packet::write_player_digging(status, pos.0, pos.1, pos.2, face),
    );
}

/// EntityPlayerSP.swingItem sends C0A separately from the interaction packet.
/// Keeping this explicit preserves vanilla's packet ordering for each action.
/// Note: EntityPlayerSP.swingItem unconditionally sends C0A regardless of
/// isSwingInProgress — the gate only gates EntityLivingBase.swingItem which
/// controls the server-side S0B broadcast and local visual animation state.
pub fn send_animation(connection: &Option<net::connection::Connection>) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        log::trace!(target: "rustcraft::interaction", "send C0A animation");
        connection.send_play_packet(0x0A, &net::packet::write_animation());
    }
}

pub fn send_block_placement(
    connection: &Option<net::connection::Connection>,
    selected_item: &ItemStack,
    pos: (i32, i32, i32),
    face: BlockFace,
    cursor: (f32, f32, f32),
) {
    let held = selected_item.to_protocol_slot();
    send_block_placement_slot(connection, &held, pos, face, cursor);
}

/// Send a "use item" packet (right-click without targeting a block).
/// Used for eating, drinking potions, drawing bow, throwing projectiles, etc.
/// In MC 1.8.9 this is the same block-placement packet (0x08) with position
/// (-1, -1, -1) and facing 255.
pub fn send_use_item(connection: &Option<net::connection::Connection>, held: &net::slot::Slot) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }
    log::trace!(
        target: "rustcraft::interaction",
        "send C08 use item: item_id={}, damage={}, count={}",
        held.item_id,
        held.damage,
        held.count
    );
    connection.send_play_packet(
        0x08,
        &net::packet::write_player_block_placement(-1, -1, -1, -1i8, held, 0, 0, 0),
    );
}

/// Release a charged/continuous item use (bow draw, sword block, eating).
/// MC 1.8.9 uses C07PacketPlayerDigging.Action.RELEASE_USE_ITEM (status 5).
pub fn send_release_use_item(connection: &Option<net::connection::Connection>) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state == net::packet::ProtocolState::Play {
        log::trace!(
            target: "rustcraft::interaction",
            "send C07 release use item"
        );
        connection.send_play_packet(0x07, &net::packet::write_player_digging(5, 0, 0, 0, 0));
    }
}

pub fn send_block_placement_slot(
    connection: &Option<net::connection::Connection>,
    held: &net::slot::Slot,
    pos: (i32, i32, i32),
    face: BlockFace,
    cursor: (f32, f32, f32),
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }

    // C08PacketPlayerBlockPlacement encodes the in-block hit position as
    // `(int)(hit * 16)`. The server replays vanilla onBlockPlaced from these
    // bytes (e.g. upside-down stairs and slabs from hitY > 0.5).
    let face = protocol_face(face);
    log::trace!(
        target: "rustcraft::interaction",
        "send C08 block placement: pos=({},{},{}), face={face}, cursor=({:.3},{:.3},{:.3}), item_id={}, damage={}, count={}",
        pos.0,
        pos.1,
        pos.2,
        cursor.0,
        cursor.1,
        cursor.2,
        held.item_id,
        held.damage,
        held.count
    );
    connection.send_play_packet(
        0x08,
        &net::packet::write_player_block_placement(
            pos.0,
            pos.1,
            pos.2,
            face,
            held,
            (cursor.0 * 16.0) as u8,
            (cursor.1 * 16.0) as u8,
            (cursor.2 * 16.0) as u8,
        ),
    );
    connection.send_play_packet(0x0A, &net::packet::write_animation());
}

/// Vanilla PlayerControllerMP.updateController synchronizes the selected
/// hotbar slot before runTick dispatches any interaction packets.
pub fn sync_held_item(
    connection: &Option<net::connection::Connection>,
    state: &mut ClientNetworkState,
    held_item: usize,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }

    let held_item = held_item.min(8);
    if held_item != state.last_held_item {
        connection.send_play_packet(0x09, &net::packet::write_held_item_change(held_item as i16));
        state.last_held_item = held_item;
    }
}

pub fn send_player_tick(
    connection: &Option<net::connection::Connection>,
    player: &mut Player,
    state: &mut ClientNetworkState,
    entity_id: Option<i32>,
) {
    let Some(connection) = connection else {
        return;
    };
    if connection.state != net::packet::ProtocolState::Play {
        return;
    }

    if let (Some(entity_id), Some(jump_power)) = (entity_id, player.take_pending_horse_jump()) {
        connection.send_play_packet(
            0x0B,
            &net::packet::write_entity_action(entity_id, 5, jump_power),
        );
    }

    // EntityPlayerSP.onUpdate has a distinct mounted branch. It sends a look
    // packet followed by C0C input every tick and deliberately does not call
    // onUpdateWalkingPlayer (so no sprint/sneak action or position packets).
    if player.vehicle_id.is_some() {
        connection.send_play_packet(
            0x05,
            &net::packet::write_player_look(
                player.camera.mc_yaw_degrees(),
                player.camera.mc_pitch_degrees(),
                player.on_ground,
            ),
        );
        connection.send_play_packet(
            0x0C,
            &net::packet::write_player_input(
                player.move_strafe,
                player.move_forward,
                player.movement_jump,
                player.sneaking,
            ),
        );
        return;
    }

    if let Some(entity_id) = entity_id {
        // EntityPlayerSP selects sprint before movement, then
        // onUpdateWalkingPlayer reports that same current state. Item use
        // immediately slows input and must immediately stop sprinting too.
        let sprinting = player.movement_sprinting();
        if sprinting != state.last_sprinting {
            let action_id = if sprinting { 3 } else { 4 };
            connection.send_play_packet(
                0x0B,
                &net::packet::write_entity_action(entity_id, action_id, 0),
            );
        }
        if player.sneaking != state.last_sneaking {
            let action_id = if player.sneaking { 0 } else { 1 };
            connection.send_play_packet(
                0x0B,
                &net::packet::write_entity_action(entity_id, action_id, 0),
            );
        }
        // MUST update here, not only in synchronize_after_server_correction.
        // Without this, START/STOP_SPRINTING is re-sent every tick because
        // last_sprinting never matches the current state, flooding the server
        // and causing GrimAC Simulation violations.
        state.last_sprinting = sprinting;
        state.last_sneaking = player.sneaking;
    }

    let pos = player.position;
    let yaw = player.camera.mc_yaw_degrees();
    let pitch = player.camera.mc_pitch_degrees();
    let moved = state.force_sync
        || !state.initialized
        || (pos - state.last_position).magnitude_squared() > 9.0e-4
        || state.ticks_since_full >= 20;
    let looked = state.force_sync
        || !state.initialized
        || yaw - state.last_yaw != 0.0
        || pitch - state.last_pitch != 0.0;

    let had_force = state.force_sync;
    state.force_sync = false;

    {
        use std::sync::atomic::{AtomicU32, Ordering};
        static DBG_TICK: AtomicU32 = AtomicU32::new(0);
        let tick = DBG_TICK.fetch_add(1, Ordering::Relaxed);
        if tick < 10 {
            log::trace!(
                "movement packet decision: tick={}, pos=({:.6},{:.6},{:.6}), on_ground={}, moved={}, looked={}, force={}",
                tick, pos.x, pos.y, pos.z, player.on_ground, moved, looked, had_force
            );
        }
    }

    if moved && looked {
        let payload = net::packet::write_player_position_and_look(
            pos.x as f64,
            pos.y as f64,
            pos.z as f64,
            yaw,
            pitch,
            player.on_ground,
        );
        connection.send_play_packet(0x06, &payload);
    } else if moved {
        let payload = net::packet::write_player_position(
            pos.x as f64,
            pos.y as f64,
            pos.z as f64,
            player.on_ground,
        );
        connection.send_play_packet(0x04, &payload);
    } else if looked {
        let payload = net::packet::write_player_look(yaw, pitch, player.on_ground);
        connection.send_play_packet(0x05, &payload);
    } else {
        connection.send_play_packet(0x03, &net::packet::write_player_on_ground(player.on_ground));
    }

    state.ticks_since_full = state.ticks_since_full.saturating_add(1);
    if moved {
        state.last_position = pos;
        state.ticks_since_full = 0;
    }
    if looked {
        state.last_yaw = yaw;
        state.last_pitch = pitch;
    }
    state.last_sneaking = player.sneaking;
    state.last_sprinting = player.movement_sprinting();
    state.initialized = true;
}

fn protocol_face(face: BlockFace) -> i8 {
    match face {
        BlockFace::Bottom => 0,
        BlockFace::Top => 1,
        BlockFace::North => 2,
        BlockFace::South => 3,
        BlockFace::West => 4,
        BlockFace::East => 5,
    }
}
