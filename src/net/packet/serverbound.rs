use crate::net::protocol::{PacketBuffer, PROTOCOL_VERSION};
use crate::net::slot::{self, Slot};

// --- Serverbound packets ---

pub fn write_keep_alive(keep_alive_id: i32) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_varint(keep_alive_id);
    buf.into_inner()
}

pub fn write_player_position(x: f64, y: f64, z: f64, on_ground: bool) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_double(x);
    buf.write_double(y);
    buf.write_double(z);
    buf.write_bool(on_ground);
    buf.into_inner()
}

pub fn write_player_look(yaw: f32, pitch: f32, on_ground: bool) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_float(yaw);
    buf.write_float(pitch);
    buf.write_bool(on_ground);
    buf.into_inner()
}

pub fn write_player_on_ground(on_ground: bool) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_bool(on_ground);
    buf.into_inner()
}

pub fn write_player_position_and_look(
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_double(x);
    buf.write_double(y);
    buf.write_double(z);
    buf.write_float(yaw);
    buf.write_float(pitch);
    buf.write_bool(on_ground);
    buf.into_inner()
}

pub fn write_chat_message(message: &str) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_string(message);
    buf.into_inner()
}

pub fn write_held_item_change(slot: i16) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_short(slot);
    buf.into_inner()
}

pub fn write_click_window(
    window_id: u8,
    slot: i16,
    button: u8,
    action_number: i16,
    mode: u8,
    clicked_item: &Slot,
) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_unsigned_byte(window_id);
    buf.write_short(slot);
    buf.write_unsigned_byte(button);
    buf.write_short(action_number);
    buf.write_unsigned_byte(mode);
    slot::write_slot(&mut buf, clicked_item);
    buf.into_inner()
}

pub fn write_confirm_transaction(window_id: u8, action_number: i16, accepted: bool) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_unsigned_byte(window_id);
    buf.write_short(action_number);
    buf.write_bool(accepted);
    buf.into_inner()
}

pub fn write_enchant_item(window_id: u8, enchantment: u8) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_unsigned_byte(window_id);
    buf.write_unsigned_byte(enchantment.min(2));
    buf.into_inner()
}

pub fn write_use_entity(entity_id: i32, action: i32) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_varint(entity_id);
    buf.write_varint(action);
    buf.into_inner()
}

/// C02 `INTERACT_AT` includes the hit point relative to the target entity.
pub fn write_use_entity_interact_at(entity_id: i32, target: [f32; 3]) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_varint(entity_id);
    buf.write_varint(2);
    buf.write_float(target[0]);
    buf.write_float(target[1]);
    buf.write_float(target[2]);
    buf.into_inner()
}

pub fn write_entity_action(entity_id: i32, action_id: i32, jump_boost: i32) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_varint(entity_id);
    buf.write_varint(action_id);
    buf.write_varint(jump_boost);
    buf.into_inner()
}

/// C0C Packet Input, sent by EntityPlayerSP every tick while riding.
pub fn write_player_input(strafe: f32, forward: f32, jumping: bool, sneaking: bool) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_float(strafe);
    buf.write_float(forward);
    buf.write_unsigned_byte((jumping as u8) | ((sneaking as u8) << 1));
    buf.into_inner()
}

pub fn write_client_status(action_id: i32) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_varint(action_id);
    buf.into_inner()
}

pub fn write_player_abilities(flags: u8, flying_speed: f32, walking_speed: f32) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_unsigned_byte(flags);
    buf.write_float(flying_speed);
    buf.write_float(walking_speed);
    buf.into_inner()
}

pub fn write_creative_inventory_action(slot: i16, clicked_item: &Slot) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_short(slot);
    slot::write_slot(&mut buf, clicked_item);
    buf.into_inner()
}

pub fn write_close_window(window_id: u8) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_byte(window_id as i8);
    buf.into_inner()
}

pub fn write_player_digging(status: u8, x: i32, y: i32, z: i32, face: i8) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_unsigned_byte(status);
    buf.write_long(pack_position(x, y, z));
    buf.write_byte(face);
    buf.into_inner()
}

pub fn write_player_block_placement(
    x: i32,
    y: i32,
    z: i32,
    face: i8,
    held_item: &Slot,
    cursor_x: u8,
    cursor_y: u8,
    cursor_z: u8,
) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_long(pack_position(x, y, z));
    buf.write_byte(face);
    slot::write_slot(&mut buf, held_item);
    buf.write_unsigned_byte(cursor_x.min(15));
    buf.write_unsigned_byte(cursor_y.min(15));
    buf.write_unsigned_byte(cursor_z.min(15));
    buf.into_inner()
}

pub fn write_animation() -> Vec<u8> {
    Vec::new()
}

pub fn write_client_settings(
    locale: &str,
    view_distance: i8,
    chat_flags: u8,
    colors: bool,
    skin_parts: u8,
) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_string(locale);
    buf.write_byte(view_distance);
    buf.write_unsigned_byte(chat_flags);
    buf.write_bool(colors);
    buf.write_unsigned_byte(skin_parts);
    buf.into_inner()
}

pub fn write_tab_complete(text: &str, block: Option<(i32, i32, i32)>) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_string(text);
    buf.write_bool(block.is_some());
    if let Some((x, y, z)) = block {
        buf.write_long(pack_position(x, y, z));
    }
    buf.into_inner()
}

pub fn write_update_sign(x: i32, y: i32, z: i32, lines: [&str; 4]) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_long(pack_position(x, y, z));
    for line in lines {
        buf.write_string(line);
    }
    buf.into_inner()
}

pub fn write_plugin_message(channel: &str, data: &[u8]) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_string(channel);
    buf.write_bytes(data);
    buf.into_inner()
}

pub fn write_brand(brand: &str) -> Vec<u8> {
    let mut brand_buf = PacketBuffer::empty();
    brand_buf.write_string(brand);
    write_plugin_message("MC|Brand", &brand_buf.into_inner())
}

pub fn write_resource_pack_status(hash: &str, result: i32) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_string(hash);
    buf.write_varint(result);
    buf.into_inner()
}

fn pack_position(x: i32, y: i32, z: i32) -> i64 {
    (((x as i64) & 0x3FFFFFF) << 38) | (((y as i64) & 0xFFF) << 26) | ((z as i64) & 0x3FFFFFF)
}

pub fn write_handshake(server_addr: &str, server_port: u16, next_state: i32) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_varint(PROTOCOL_VERSION);
    buf.write_string(server_addr);
    buf.write_short(server_port as i16);
    buf.write_varint(next_state);
    buf.into_inner()
}

pub fn write_login_start(username: &str) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_string(username);
    buf.into_inner()
}

pub fn write_encryption_response(shared_secret: &[u8], verify_token: &[u8]) -> Vec<u8> {
    let mut buf = PacketBuffer::empty();
    buf.write_varint(shared_secret.len() as i32);
    buf.write_bytes(shared_secret);
    buf.write_varint(verify_token.len() as i32);
    buf.write_bytes(verify_token);
    buf.into_inner()
}

#[cfg(test)]
mod tests {
    use super::{write_confirm_transaction, write_player_input};

    #[test]
    fn rejected_inventory_ack_uses_the_server_action_and_accepts_resync() {
        assert_eq!(
            write_confirm_transaction(2, 0x1234, true),
            [0x02, 0x12, 0x34, 0x01]
        );
    }

    #[test]
    fn player_input_uses_protocol_47_float_and_flag_layout() {
        let bytes = write_player_input(1.0, -0.5, true, true);
        assert_eq!(
            bytes,
            [0x3F, 0x80, 0x00, 0x00, 0xBF, 0x00, 0x00, 0x00, 0x03]
        );
    }
}
