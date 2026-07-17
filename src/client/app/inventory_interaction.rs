use super::App;
use crate::audio::AudioBackend;
use crate::client;
use crate::client::keybind::Action;
use winit::event::MouseButton;

impl App {
    pub(super) fn toggle_inventory(&mut self) {
        if self.inventory_open {
            self.close_inventory_screen(true);
        } else {
            self.inventory_open = true;
            self.mouse_captured = false;
            self.set_cursor_captured(false);
        }
    }

    pub(super) fn close_inventory_screen(&mut self, recapture: bool) {
        let open_chest_position = self.inventory.open_chest_position.take();
        self.inventory_open = false;
        self.creative_scroll_dragging = false;
        self.gamepad_inventory_drag = None;
        self.inventory.had_server_window = false;
        self.inventory.cursor = crate::client::inventory::ItemStack::EMPTY;
        client::network::send_close_window(&self.connection, self.inventory.open_window_id);
        if let Some(position) = open_chest_position {
            self.world.close_chest_for_local_viewer(position);
        }
        self.inventory.close_window(self.inventory.open_window_id);
        if recapture && matches!(self.state, crate::client::state::GameState::Playing) {
            self.mouse_captured = true;
            self.set_cursor_captured(true);
        }
    }

    pub(super) fn begin_gamepad_inventory_drag(&mut self, button: MouseButton) {
        if self.inventory.cursor.is_empty() {
            self.handle_inventory_click(button);
            return;
        }
        let mut slots = Vec::new();
        self.push_hovered_gamepad_drag_slot(&mut slots);
        self.gamepad_inventory_drag = Some((button, slots));
    }

    pub(super) fn extend_gamepad_inventory_drag(&mut self) {
        let Some((button, mut slots)) = self.gamepad_inventory_drag.take() else {
            return;
        };
        self.push_hovered_gamepad_drag_slot(&mut slots);
        self.gamepad_inventory_drag = Some((button, slots));
    }

    pub(super) fn finish_gamepad_inventory_drag(&mut self, released_button: MouseButton) {
        let Some((button, slots)) = self.gamepad_inventory_drag.take() else {
            return;
        };
        if button != released_button {
            self.gamepad_inventory_drag = Some((button, slots));
            return;
        }
        if slots.len() < 2 || self.connection.is_none() {
            self.handle_inventory_click(button);
            return;
        }
        let drag_mode = if button == MouseButton::Right { 1 } else { 0 };
        let base_button = drag_mode << 2;
        self.send_inventory_click(
            -999,
            base_button,
            5,
            crate::client::inventory::ItemStackView::default(),
        );
        for slot in slots {
            self.send_inventory_click(
                slot,
                base_button | 1,
                5,
                crate::client::inventory::ItemStackView::default(),
            );
        }
        self.send_inventory_click(
            -999,
            base_button | 2,
            5,
            crate::client::inventory::ItemStackView::default(),
        );
    }

    fn push_hovered_gamepad_drag_slot(&self, slots: &mut Vec<i16>) {
        let slot = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32))
            .and_then(|id| {
                id.checked_sub(crate::ui::button_ids::INVENTORY_SLOT_BASE)
                    .filter(|slot| *slot < crate::ui::button_ids::INVENTORY_SLOT_MAX as u32)
                    .map(|slot| slot as i16)
            });
        if let Some(slot) = slot.filter(|slot| !slots.contains(slot)) {
            slots.push(slot);
        }
    }

    pub(super) fn handle_inventory_click(&mut self, button: MouseButton) {
        let hit = self
            .renderer
            .as_ref()
            .and_then(|r| r.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32));
        let right_click = button == MouseButton::Right;
        let shift_click = self.input.is_held(Action::Sneak);
        let Some(id) = hit else {
            // Click outside any slot
            let is_inventory_tab = self
                .renderer
                .as_ref()
                .map_or(false, |r| r.state.creative_tab == 11);
            if self.connection.is_some()
                && self.session.gamemode == 1
                && self.inventory.open_window_id == 0
                && is_inventory_tab
            {
                let stack = self.inventory.cursor;
                if stack.is_empty() {
                    return;
                }
                if right_click {
                    let single = crate::client::inventory::ItemStack { count: 1, ..stack };
                    self.inventory.click_outside(true);
                    client::network::send_creative_inventory_action(&self.connection, -1, &single);
                } else {
                    self.inventory.click_outside(false);
                    client::network::send_creative_inventory_action(&self.connection, -1, &stack);
                }
            } else if self.connection.is_some() {
                self.inventory.click_outside(right_click);
                self.send_inventory_click(
                    -999,
                    if right_click { 1 } else { 0 },
                    0,
                    crate::client::inventory::ItemStackView::default(),
                );
            } else {
                self.inventory.click_outside(right_click);
            }
            return;
        };

        if let Some(slot) = creative_slot_id(id) {
            if self.session.gamemode == 1 && self.inventory.open_window_id == 0 {
                self.handle_creative_grid_click(slot, right_click, shift_click);
            }
            return;
        }

        if id == crate::ui::button_ids::CREATIVE_TRASH {
            self.inventory.cursor = crate::client::inventory::ItemStack::EMPTY;
            self.inventory.cursor_meta = crate::client::inventory::ItemStackMeta::default();
            return;
        }

        // Creative tab click
        if id >= crate::ui::button_ids::CREATIVE_TAB_BASE
            && id
                < crate::ui::button_ids::CREATIVE_TAB_BASE
                    + crate::ui::button_ids::CREATIVE_TAB_MAX as u32
        {
            let tab = (id - crate::ui::button_ids::CREATIVE_TAB_BASE) as usize;
            if let Some(renderer) = &mut self.renderer {
                renderer.state.creative_tab = tab;
                renderer.state.creative_scroll = 0.0;
                if tab == crate::render::hud::inventory::CREATIVE_TAB_SEARCH {
                    renderer.state.creative_search.clear();
                }
            }
            self.audio.play(crate::audio::SoundEvent {
                name: "random.click".to_string(),
                category: crate::audio::SoundCategory::Ui,
                volume: 1.0,
                pitch: 1.0,
                position: None,
            });
            return;
        }

        if id >= crate::ui::button_ids::ENCHANT_OPTION_BASE
            && id
                < crate::ui::button_ids::ENCHANT_OPTION_BASE
                    + crate::ui::button_ids::ENCHANT_OPTION_MAX as u32
        {
            if self.connection.is_some() && self.inventory.open_window_id != 0 {
                client::network::send_enchant_item(
                    &self.connection,
                    self.inventory.open_window_id,
                    (id - crate::ui::button_ids::ENCHANT_OPTION_BASE) as u8,
                );
            }
            return;
        }

        let slot_id = id.saturating_sub(crate::ui::button_ids::INVENTORY_SLOT_BASE);
        if id < crate::ui::button_ids::INVENTORY_SLOT_BASE
            || slot_id >= crate::ui::button_ids::INVENTORY_SLOT_MAX as u32
        {
            return;
        }

        let protocol_slot = slot_id as i16;
        if self.connection.is_some()
            && self.session.gamemode == 1
            && self.inventory.open_window_id == 0
        {
            self.handle_creative_player_slot_click(protocol_slot, right_click);
            return;
        }
        if self.connection.is_some() {
            let before = self.inventory.item_view_for_protocol_slot(protocol_slot);
            let mut handled = true;
            if shift_click {
                self.inventory.shift_click_protocol_slot(protocol_slot);
            } else if self.inventory.open_window_id == 0 {
                self.inventory
                    .click_player_window_slot(protocol_slot, right_click);
            } else if protocol_slot >= 0
                && (protocol_slot as usize) < self.inventory.open_window_slot_count
            {
                self.inventory
                    .click_open_window_slot(protocol_slot, right_click);
            } else if let Some(local_slot) = self
                .inventory
                .local_index_for_open_window_slot(protocol_slot)
            {
                self.inventory.click_local_slot(local_slot, right_click);
            } else {
                handled = false;
            }
            if handled {
                let mode = if shift_click { 1 } else { 0 };
                let after = self.inventory.item_view_for_protocol_slot(protocol_slot);
                self.send_inventory_click(
                    protocol_slot,
                    if right_click { 1 } else { 0 },
                    mode,
                    click_window_result_stack(mode, before, after),
                );
            }
        } else if shift_click {
            self.inventory.shift_click_protocol_slot(protocol_slot);
        } else if self.inventory.open_window_id == 0 {
            self.inventory
                .click_player_window_slot(protocol_slot, right_click);
        } else {
            // Inventory window is open and we're running client-side (no connection).
            // If the clicked protocol slot is within the open window's own slots,
            // operate on those slots directly. Otherwise map to a local player slot.
            if protocol_slot >= 0
                && (protocol_slot as usize) < self.inventory.open_window_slot_count
            {
                self.inventory
                    .click_open_window_slot(protocol_slot, right_click);
            } else if let Some(local_slot) = self
                .inventory
                .local_index_for_open_window_slot(protocol_slot)
            {
                self.inventory.click_local_slot(local_slot, right_click);
            }
        }
    }

    pub(super) fn drop_hovered_inventory_slot(&mut self, drop_stack: bool) {
        let hit = self
            .renderer
            .as_ref()
            .and_then(|r| r.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32));
        let creative_slot = hit.and_then(creative_slot_id);
        let protocol_slot = hit.and_then(|id| {
            if id < crate::ui::button_ids::INVENTORY_SLOT_BASE {
                return None;
            }
            let slot_id = id - crate::ui::button_ids::INVENTORY_SLOT_BASE;
            (slot_id < crate::ui::button_ids::INVENTORY_SLOT_MAX as u32).then_some(slot_id as i16)
        });

        let protocol_slot = protocol_slot.unwrap_or(-999);
        if self.connection.is_some()
            && self.session.gamemode == 1
            && self.inventory.open_window_id == 0
        {
            let is_inventory_tab = self
                .renderer
                .as_ref()
                .map_or(false, |r| r.state.creative_tab == 11);
            if let Some(slot) = creative_slot {
                if !is_inventory_tab {
                    if let Some(stack) = self.creative_grid_stack(slot, drop_stack) {
                        client::network::send_creative_inventory_action(
                            &self.connection,
                            -1,
                            &stack,
                        );
                    }
                }
                return;
            }
            if is_inventory_tab {
                if protocol_slot == -999 {
                    let stack = self.inventory.cursor;
                    self.inventory.click_outside(!drop_stack);
                    let send_stack = if drop_stack {
                        stack
                    } else {
                        crate::client::inventory::ItemStack {
                            count: 1.min(stack.count),
                            ..stack
                        }
                    };
                    client::network::send_creative_inventory_action(
                        &self.connection,
                        -1,
                        &send_stack,
                    );
                } else {
                    self.inventory.drop_protocol_slot(protocol_slot, drop_stack);
                    let stack = self.inventory.item_for_protocol_slot(protocol_slot);
                    if protocol_slot >= 1 {
                        client::network::send_creative_inventory_action(
                            &self.connection,
                            protocol_slot,
                            &stack,
                        );
                    }
                }
            } else {
                let stack = if protocol_slot == -999 {
                    self.inventory.cursor
                } else {
                    self.inventory.item_for_protocol_slot(protocol_slot)
                };
                let single = crate::client::inventory::ItemStack {
                    count: 1.min(stack.count),
                    ..stack
                };
                client::network::send_creative_inventory_action(
                    &self.connection,
                    if protocol_slot == -999 {
                        -1
                    } else {
                        protocol_slot
                    },
                    if drop_stack { &stack } else { &single },
                );
            }
        } else if self.connection.is_some() {
            self.inventory.drop_protocol_slot(protocol_slot, drop_stack);
            self.send_inventory_click(
                protocol_slot,
                if drop_stack { 1 } else { 0 },
                4,
                crate::client::inventory::ItemStackView::default(),
            );
        } else {
            self.inventory.drop_protocol_slot(protocol_slot, drop_stack);
        }
    }

    fn handle_creative_grid_click(&mut self, slot: usize, right_click: bool, full_stack: bool) {
        let Some(stack) = self.creative_grid_stack(slot, full_stack) else {
            return;
        };

        if right_click
            && !self.inventory.cursor.is_empty()
            && self.inventory.cursor.item_id == stack.item_id
            && self.inventory.cursor.damage == stack.damage
        {
            self.inventory.cursor = crate::client::inventory::ItemStack::EMPTY;
        } else {
            self.inventory.cursor = stack;
        }
        self.inventory.cursor_meta = crate::client::inventory::ItemStackMeta::default();
    }

    fn handle_creative_player_slot_click(&mut self, protocol_slot: i16, right_click: bool) {
        let before = self.inventory.item_for_protocol_slot(protocol_slot);
        self.inventory
            .click_player_window_slot(protocol_slot, right_click);
        let after = self.inventory.item_for_protocol_slot(protocol_slot);
        if before != after && matches!(protocol_slot, 5..=44) {
            client::network::send_creative_inventory_action(
                &self.connection,
                protocol_slot,
                &after,
            );
        }
    }

    fn creative_grid_stack(
        &self,
        slot: usize,
        full_stack: bool,
    ) -> Option<crate::client::inventory::ItemStack> {
        let renderer = self.renderer.as_ref()?;
        if renderer.state.creative_tab == crate::render::hud::inventory::CREATIVE_TAB_INVENTORY {
            return None;
        }
        let items = renderer.creative_visible_entries();
        let cols = 9usize;
        let max_scroll_rows = crate::render::hud::inventory::creative_max_scroll_rows(items.len());
        let scroll_start =
            (renderer.state.creative_scroll * max_scroll_rows as f32).round() as usize * cols;
        let item = *items.get(scroll_start + slot)?;
        let mut stack = crate::client::inventory::ItemStack::new(item.item_id, 1);
        stack.damage = item.damage;
        if full_stack {
            stack.count = stack.max_stack();
        }
        Some(stack)
    }

    fn send_inventory_click(
        &mut self,
        protocol_slot: i16,
        button: u8,
        mode: u8,
        clicked: crate::client::inventory::ItemStackView,
    ) {
        if self.connection.is_none() {
            return;
        }

        let action = self.inventory_action_number;
        self.inventory_action_number = self.inventory_action_number.wrapping_add(1).max(1);
        self.pending_click_windows
            .push(crate::net::packet::write_click_window(
                self.inventory.open_window_id,
                protocol_slot,
                button,
                action,
                mode,
                &clicked.to_protocol_slot(),
            ));
    }
}

fn click_window_result_stack(
    mode: u8,
    before: crate::client::inventory::ItemStackView,
    after: crate::client::inventory::ItemStackView,
) -> crate::client::inventory::ItemStackView {
    match mode {
        // Container#slotClick captures the original target stack before a
        // normal pickup/place/swap operation.
        0 => before,
        // transferStackInSlot returns the original stack only when something
        // was actually transferred.
        1 if before != after => before,
        _ => crate::client::inventory::ItemStackView::default(),
    }
}

fn creative_slot_id(id: u32) -> Option<usize> {
    if id < crate::ui::button_ids::CREATIVE_SLOT_BASE {
        return None;
    }
    let slot = id - crate::ui::button_ids::CREATIVE_SLOT_BASE;
    (slot < crate::ui::button_ids::CREATIVE_SLOT_MAX as u32).then_some(slot as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item_id: u16, count: u8) -> crate::client::inventory::ItemStackView {
        crate::client::inventory::ItemStackView {
            item_id,
            count,
            damage: 0,
            nbt: None,
        }
    }

    #[test]
    fn click_packet_uses_container_slot_click_return_stack() {
        let original = stack(1, 32);
        let placed = stack(1, 32);
        let empty = crate::client::inventory::ItemStackView::default();

        assert_eq!(
            click_window_result_stack(0, original.clone(), empty.clone()),
            original
        );
        assert_eq!(click_window_result_stack(0, empty.clone(), placed), empty);
    }

    #[test]
    fn shift_click_returns_original_stack_only_when_it_moves() {
        let original = stack(276, 1);
        assert_eq!(
            click_window_result_stack(
                1,
                original.clone(),
                crate::client::inventory::ItemStackView::default(),
            ),
            original
        );
        assert!(click_window_result_stack(1, original.clone(), original).is_empty());
    }
}
