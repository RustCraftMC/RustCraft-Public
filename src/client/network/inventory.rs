use crate::client::inventory::Inventory;
use crate::client::session::SessionState;
use crate::net;
use std::borrow::Cow;

pub(super) fn handle_packet(
    connection: &mut net::connection::Connection,
    inventory: &mut Inventory,
    session: &mut SessionState,
    packet: Option<net::packet::ClientboundPacket>,
    i18n: Option<&crate::ui::i18n::I18n>,
) -> Option<net::packet::ClientboundPacket> {
    let packet = packet?;
    match packet {
        net::packet::ClientboundPacket::HeldItemChange { slot } => {
            if (0..=8).contains(&slot) {
                inventory.set_selected(slot as usize);
            }
        }
        net::packet::ClientboundPacket::OpenWindow {
            window_id,
            window_type,
            title_json,
            slot_count,
            entity_id: _,
        } => {
            let title = plain_json_text(&title_json, i18n);
            inventory.set_open_window(window_id, window_type, title, slot_count as usize);
            // Format the integers once into owned buffers, then run the two
            // template substitutions. Using `Cow::Owned` keeps the hot path
            // (no window open) allocation-free while still producing a `&str`
            // for `str::replace`.
            let id_str: Cow<'_, str> = Cow::Owned(window_id.to_string());
            let slot_str: Cow<'_, str> = Cow::Owned(slot_count.to_string());
            session.push_system_line(
                session
                    .text
                    .opened_window
                    .replace("%1$s", &id_str)
                    .replace("%2$s", &slot_str),
            );
        }
        net::packet::ClientboundPacket::CloseWindow { window_id } => {
            inventory.close_window(window_id);
        }
        net::packet::ClientboundPacket::SetSlot {
            window_id,
            slot,
            item,
        } => {
            inventory.apply_window_slot(window_id, slot, &item);
        }
        net::packet::ClientboundPacket::WindowItems { window_id, slots } => {
            inventory.apply_window_items(window_id, &slots);
        }
        net::packet::ClientboundPacket::WindowProperty {
            window_id,
            property,
            value,
        } => {
            inventory.apply_window_property(window_id, property, value);
        }
        net::packet::ClientboundPacket::ConfirmTransaction {
            window_id,
            action_number,
            accepted,
        } => {
            // Vanilla acknowledges only rejected transactions, and it sends
            // accepted=true. The server locks this container after a mismatch
            // until this exact C0F arrives.
            if !accepted {
                connection.send_play_packet(
                    0x0F,
                    &net::packet::write_confirm_transaction(window_id, action_number, true),
                );
            }
        }
        other => return Some(other),
    }
    None
}

fn plain_json_text(json: &str, i18n: Option<&crate::ui::i18n::I18n>) -> String {
    if let Some(i18n) = i18n {
        crate::client::session::localized_chat_text(json, i18n).unwrap_or_else(|| json.to_string())
    } else {
        crate::client::session::plain_text(json).unwrap_or_else(|| json.to_string())
    }
}
