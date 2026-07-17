use crate::client::inventory::ItemStackView;
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::{entity::player_model, Renderer};
use crate::ui::format::format_text;

// Creative inventory tab indices (MC 1.8.9)
pub const CREATIVE_TAB_BLOCK: usize = 0;
pub const CREATIVE_TAB_DECORATION: usize = 1;
pub const CREATIVE_TAB_REDSTONE: usize = 2;
pub const CREATIVE_TAB_TRANSPORT: usize = 3;
pub const CREATIVE_TAB_MISC: usize = 4;
pub const CREATIVE_TAB_SEARCH: usize = 5;
pub const CREATIVE_TAB_FOOD: usize = 6;
pub const CREATIVE_TAB_TOOLS: usize = 7;
pub const CREATIVE_TAB_COMBAT: usize = 8;
pub const CREATIVE_TAB_BREWING: usize = 9;
pub const CREATIVE_TAB_MATERIALS: usize = 10;
pub const CREATIVE_TAB_INVENTORY: usize = 11;

const CREATIVE_PANEL_WIDTH: f32 = 195.0;
const CREATIVE_PANEL_HEIGHT: f32 = 136.0;
const CREATIVE_SCROLLBAR_X: f32 = 175.0;
const CREATIVE_SCROLLBAR_Y: f32 = 18.0;
const CREATIVE_SCROLLBAR_WIDTH: f32 = 14.0;
const CREATIVE_SCROLLBAR_HEIGHT: f32 = 112.0;
const CREATIVE_SCROLLBAR_THUMB_HEIGHT: f32 = 15.0;

/// Convert a vanilla sRGB color byte (0-255) to the linear value the GUI
/// pipeline expects. The swapchain is `*_SRGB`, so fragment colors are
/// re-encoded on write; feeding byte/255 directly would brighten the color
/// (e.g. the durability track's dark green 64 would display as ~137).
fn srgb_byte_to_linear(byte: f32) -> f32 {
    let c = (byte / 255.0).clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Vanilla `RenderItem.renderItemOverlayIntoGUI` durability bar: a 13×2 black
/// backdrop at (x+2, y+13) in 16px slot space, a dark 12×1 track, and a
/// green→red foreground whose width and hue scale with remaining durability.
pub(super) fn draw_durability_bar(
    font_gui: &mut GuiVertexBuilder,
    sx: f32,
    sy: f32,
    slot_size: f32,
    item_id: u16,
    damage: u16,
) {
    let max = crate::client::inventory::max_damage(item_id);
    // ItemStack.isItemDamaged: damageable and damage > 0.
    if max == 0 || damage == 0 {
        return;
    }
    let px = slot_size / 16.0;
    let damage = f64::from(damage.min(max));
    let max = f64::from(max);
    let width = (13.0 - damage * 13.0 / max).round() as f32;
    let green = (255.0 - damage * 255.0 / max).round() as f32;
    let red = 255.0 - green;
    let bar_x = sx + 2.0 * px;
    let bar_y = sy + 13.0 * px;
    font_gui.fill_rect(bar_x, bar_y, 13.0 * px, 2.0 * px, [0.0, 0.0, 0.0, 1.0]);
    font_gui.fill_rect(
        bar_x,
        bar_y,
        12.0 * px,
        1.0 * px,
        [
            srgb_byte_to_linear(((red as i32) / 4) as f32),
            srgb_byte_to_linear(64.0),
            0.0,
            1.0,
        ],
    );
    font_gui.fill_rect(
        bar_x,
        bar_y,
        width * px,
        1.0 * px,
        [
            srgb_byte_to_linear(red),
            srgb_byte_to_linear(green),
            0.0,
            1.0,
        ],
    );
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CreativeScrollbarGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub thumb_height: f32,
    pub scale: f32,
}

impl CreativeScrollbarGeometry {
    pub fn contains(self, mouse_x: f32, mouse_y: f32) -> bool {
        mouse_x >= self.x
            && mouse_x < self.x + self.width
            && mouse_y >= self.y
            && mouse_y < self.y + self.height
    }

    pub fn scroll_for_mouse_y(self, mouse_y: f32) -> f32 {
        ((mouse_y - self.y - 7.5 * self.scale) / (self.height - self.thumb_height)).clamp(0.0, 1.0)
    }

    fn thumb_y(self, scroll: f32) -> f32 {
        // GuiContainerCreative uses 17 here even though the texture is 15 px high.
        self.y + scroll.clamp(0.0, 1.0) * (self.height - 17.0 * self.scale)
    }
}

pub(crate) fn creative_scrollbar_geometry(
    viewport_width: f32,
    viewport_height: f32,
    gui_scale: f32,
) -> CreativeScrollbarGeometry {
    let scale = gui_scale.max(1.0);
    let panel_x = (viewport_width - CREATIVE_PANEL_WIDTH * scale) * 0.5;
    let panel_y = (viewport_height - CREATIVE_PANEL_HEIGHT * scale) * 0.5;
    CreativeScrollbarGeometry {
        x: panel_x + CREATIVE_SCROLLBAR_X * scale,
        y: panel_y + CREATIVE_SCROLLBAR_Y * scale,
        width: CREATIVE_SCROLLBAR_WIDTH * scale,
        height: CREATIVE_SCROLLBAR_HEIGHT * scale,
        thumb_height: CREATIVE_SCROLLBAR_THUMB_HEIGHT * scale,
        scale,
    }
}

impl Renderer {
    pub(crate) fn creative_visible_entries(&self) -> Vec<CreativeItemEntry> {
        let mut items = creative_tab_entries(self.state.creative_tab);
        if self.state.creative_tab == CREATIVE_TAB_SEARCH {
            let query = self.state.creative_search.trim().to_lowercase();
            if !query.is_empty() {
                items.retain(|item| {
                    self.item_display_name(item.item_id, item.damage)
                        .to_lowercase()
                        .contains(&query)
                });
            }
        }
        items
    }

    pub(super) fn draw_inventory_overlay(
        &mut self,
        metrics: &MenuMetrics,
        overlay_gui: &mut GuiVertexBuilder,
        widget_gui: &mut GuiVertexBuilder,
        inventory_gui: &mut GuiVertexBuilder,
        generic54_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
        creative_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;

        // Vanilla GuiContainer draws drawDefaultBackground() behind the panel.
        overlay_gui.fill_world_background(sw, sh);

        if self.state.inventory_window_id != 0 {
            self.draw_container_window_overlay(
                metrics,
                widget_gui,
                generic54_gui,
                font_gui,
                block_gui,
                item_gui,
            );
            return;
        }

        // Creative mode inventory
        if self.state.gamemode == 1 {
            self.draw_creative_inventory(
                metrics,
                widget_gui,
                font_gui,
                block_gui,
                item_gui,
                creative_gui,
            );
            return;
        }

        font_gui.fill_rect(0.0, 0.0, sw, sh, [0.0, 0.0, 0.0, 0.68]);
        let inv_w = 176.0 * gs;
        let inv_h = 166.0 * gs;
        let panel_x = (sw - inv_w) * 0.5;
        let panel_y = (sh - inv_h) * 0.5;

        draw_potion_effects_inventory(self, metrics, inventory_gui, font_gui);
        inventory_gui.add_quad(
            panel_x,
            panel_y,
            inv_w,
            inv_h,
            0.0,
            0.0,
            176.0 / 256.0,
            166.0 / 256.0,
            [1.0, 1.0, 1.0, 1.0],
        );
        let slot_size = 16.0 * gs;
        let slot_step = 18.0 * gs;
        let grid_x = panel_x + 8.0 * gs;
        let grid_y = panel_y + 84.0 * gs;

        let portrait_x = panel_x + 51.0 * gs;
        let portrait_bottom = panel_y + 75.0 * gs;
        let portrait_px = 30.0 / 16.0 * gs;
        let (portrait_yaw, portrait_pitch) =
            vanilla_preview_rotation(portrait_x, portrait_bottom, metrics.mouse_pos, gs);
        player_model::draw_player_preview(
            font_gui,
            &mut self.player_preview_cache,
            &self.state.local_skin,
            portrait_x,
            portrait_bottom - 32.0 * portrait_px,
            portrait_px,
            portrait_yaw,
            portrait_pitch,
            self.state.local_skin_slim,
            self.state.skin_parts,
            1.0,
        );

        for i in 0..4 {
            let sx = panel_x + 8.0 * gs;
            let sy = panel_y + (8.0 + i as f32 * 18.0) * gs;
            self.draw_inventory_slot(
                widget_gui,
                font_gui,
                block_gui,
                item_gui,
                sx,
                sy,
                slot_size,
                self.state.inventory_armor_slots[i].clone(),
                font_sz,
                gs,
                Some(5 + i as i16),
                metrics.mouse_pos,
            );
        }

        let craft_x = panel_x + 98.0 * gs;
        let craft_y = panel_y + 18.0 * gs;
        font_gui.draw_text(
            &mut self.font,
            panel_x + 86.0 * gs,
            panel_y + 16.0 * gs,
            self.state.ui_text.get("container.crafting"),
            font_sz * 0.64,
            [0.25, 0.25, 0.25, 1.0],
        );
        for row in 0..2 {
            for col in 0..2 {
                let craft_idx = 1 + row * 2 + col;
                let sx = craft_x + col as f32 * slot_step;
                let sy = craft_y + row as f32 * slot_step;
                self.draw_inventory_slot(
                    widget_gui,
                    font_gui,
                    block_gui,
                    item_gui,
                    sx,
                    sy,
                    slot_size,
                    self.state.inventory_crafting_slots[craft_idx].clone(),
                    font_sz,
                    gs,
                    Some(craft_idx as i16),
                    metrics.mouse_pos,
                );
            }
        }
        let result_x = panel_x + 154.0 * gs;
        let result_y = panel_y + 28.0 * gs;
        self.draw_inventory_slot(
            widget_gui,
            font_gui,
            block_gui,
            item_gui,
            result_x,
            result_y,
            slot_size,
            self.state.inventory_crafting_slots[0].clone(),
            font_sz,
            gs,
            Some(0),
            metrics.mouse_pos,
        );

        for row in 0..3 {
            for col in 0..9 {
                let idx = 9 + row * 9 + col;
                let sx = grid_x + col as f32 * slot_step;
                let sy = grid_y + row as f32 * slot_step;
                self.draw_inventory_slot(
                    widget_gui,
                    font_gui,
                    block_gui,
                    item_gui,
                    sx,
                    sy,
                    slot_size,
                    self.state.inventory_slots[idx].clone(),
                    font_sz,
                    gs,
                    Some(idx as i16),
                    metrics.mouse_pos,
                );
            }
        }

        let hotbar_y = panel_y + 142.0 * gs;
        for col in 0..9 {
            let sx = grid_x + col as f32 * slot_step;
            self.draw_inventory_slot(
                widget_gui,
                font_gui,
                block_gui,
                item_gui,
                sx,
                hotbar_y,
                slot_size,
                self.state.inventory_slots[col].clone(),
                font_sz,
                gs,
                Some(36 + col as i16),
                metrics.mouse_pos,
            );
        }

        if self
            .state
            .inventory_slots
            .iter()
            .all(|slot| slot.count == 0)
        {
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                hotbar_y + slot_size + 10.0 * gs,
                self.state.ui_text.get("rustcraft.inventory.empty"),
                font_sz * 0.72,
                [0.7, 0.7, 0.7, 1.0],
            );
        }

        let tooltip = std::mem::take(&mut self.state.hovered_tooltip);
        self.draw_cursor_stack(
            font_gui,
            block_gui,
            item_gui,
            metrics.mouse_pos,
            slot_size,
            font_sz,
            gs,
        );
        if self.state.inventory_cursor_slot.is_empty() {
            self.draw_item_tooltip(font_gui, metrics.mouse_pos, &tooltip, font_sz, gs);
        }
    }

    pub(super) fn draw_creative_inventory(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
        creative_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        let slot_size = 16.0 * gs;
        let slot_step = 18.0 * gs;
        let cols = 9usize;
        let rows = 5usize;
        let total_slots = cols * rows;

        // Dark overlay
        font_gui.fill_rect(0.0, 0.0, sw, sh, [0.0, 0.0, 0.0, 0.68]);

        // Vanilla creative inventory is 195x136 for every tab, including the inventory tab.
        let inv_w = CREATIVE_PANEL_WIDTH * gs;
        let is_inventory_tab = self.state.creative_tab == CREATIVE_TAB_INVENTORY;
        let inv_h = CREATIVE_PANEL_HEIGHT * gs;
        let panel_x = (sw - inv_w) * 0.5;
        let panel_y = (sh - inv_h) * 0.5;

        // Panel background from creative atlas
        // Most tabs (0-10 except search): tab_items.png (cell 1: uv 0.5, 0)
        // Tab 5 (search): tab_item_search.png (cell 3: uv 0.5, 0.5)
        // Tab 11 (inventory): tab_inventory.png (cell 2: uv 0.0, 0.5)
        let (panel_u, panel_v, panel_tex_h) = match self.state.creative_tab {
            CREATIVE_TAB_SEARCH => (0.5, 0.5, 136.0),
            CREATIVE_TAB_INVENTORY => (0.0, 0.5, 136.0),
            _ => (0.5, 0.0, 136.0),
        };
        creative_gui.add_quad(
            panel_x,
            panel_y,
            inv_w,
            inv_h,
            panel_u,
            panel_v,
            195.0 / 512.0,
            panel_tex_h / 512.0,
            [1.0, 1.0, 1.0, 1.0],
        );

        if is_inventory_tab {
            let portrait_x = panel_x + 43.0 * gs;
            let portrait_bottom = panel_y + 45.0 * gs;
            let portrait_px = 20.0 / 16.0 * gs;
            let (portrait_yaw, portrait_pitch) =
                vanilla_preview_rotation(portrait_x, portrait_bottom, metrics.mouse_pos, gs);
            player_model::draw_player_preview(
                font_gui,
                &mut self.player_preview_cache,
                &self.state.local_skin,
                portrait_x,
                portrait_bottom - 32.0 * portrait_px,
                portrait_px,
                portrait_yaw,
                portrait_pitch,
                self.state.local_skin_slim,
                self.state.skin_parts,
                1.0,
            );
        }

        if self.state.creative_tab == CREATIVE_TAB_SEARCH {
            font_gui.draw_text(
                &mut self.font,
                panel_x + 82.0 * gs,
                panel_y + 6.0 * gs,
                &self.state.creative_search,
                font_sz * 0.64,
                [1.0, 1.0, 1.0, 1.0],
            );
        }

        // Tab icons: (item_id, damage)
        let tab_icons: [(u16, u16); 12] = [
            (45, 0),  // Building Blocks: brick_block
            (175, 2), // Decorations: double_plant (paeonia)
            (331, 0), // Redstone: redstone
            (27, 0),  // Transportation: golden_rail
            (327, 0), // Misc: lava_bucket
            (345, 0), // Search: compass
            (260, 0), // Food: apple
            (258, 0), // Tools: iron_axe
            (283, 0), // Combat: golden_sword
            (373, 0), // Brewing: potionitem
            (280, 0), // Materials: stick
            (54, 0),  // Inventory: chest
        ];

        // --- Tabs: 12 tabs, 6 per row. Tab backgrounds from tabs.png (cell 0) ---
        let tab_w_tex = 28.0f32;
        let tab_h_tex = 32.0f32;
        let tab_w = tab_w_tex * gs;
        let tab_h = tab_h_tex * gs;

        for i in 0..12 {
            let col = i % 6;
            let is_top = i < 6;
            let is_selected = i == self.state.creative_tab;

            // Screen position (MC 1.8.9 layout)
            let tab_x = if col == 5 {
                panel_x + inv_w - tab_w
            } else {
                panel_x + col as f32 * tab_w
            };
            let tab_y = if is_top {
                panel_y - 28.0 * gs
            } else {
                panel_y + inv_h - 4.0 * gs
            };

            // Register tab as clickable button
            widget_gui.register_button(
                crate::ui::button_ids::CREATIVE_TAB_BASE + i as u32,
                tab_x,
                tab_y,
                tab_w,
                tab_h,
            );

            // Texture UV in tabs.png (cell 0 of creative atlas)
            let tex_u = col as f32 * tab_w_tex;
            let tex_v = match (is_top, is_selected) {
                (true, false) => 0.0,   // top unselected
                (true, true) => 32.0,   // top selected
                (false, false) => 64.0, // bottom unselected
                (false, true) => 96.0,  // bottom selected
            };

            // Draw tab background from creative atlas
            creative_gui.add_quad(
                tab_x,
                tab_y,
                tab_w,
                tab_h,
                tex_u / 512.0,
                tex_v / 512.0,
                tab_w_tex / 512.0,
                tab_h_tex / 512.0,
                [1.0, 1.0, 1.0, 1.0],
            );

            // Tab icon
            let (icon_id, icon_damage) = tab_icons[i];
            self.draw_item_icon_2d_or_block(
                block_gui,
                item_gui,
                font_gui,
                tab_x + (tab_w - slot_size) * 0.5,
                tab_y + 6.0 * gs,
                slot_size,
                icon_id,
                icon_damage,
                gs,
                false,
            );
        }

        // --- Item grid (9 columns x 5 rows = 45 slots) ---
        let grid_x = panel_x + 9.0 * gs;
        let grid_y = panel_y + 18.0 * gs;

        let items = if is_inventory_tab {
            vec![]
        } else {
            self.creative_visible_entries()
        };
        let max_scroll_rows = creative_max_scroll_rows(items.len());
        let scroll_start =
            (self.state.creative_scroll * max_scroll_rows as f32).round() as usize * cols;
        let visible_count = total_slots.min(items.len().saturating_sub(scroll_start));

        // Draw slot backgrounds and items (skip for inventory tab — it draws its own layout)
        if !is_inventory_tab {
            for i in 0..total_slots {
                let col = i % cols;
                let row = i / cols;
                let sx = grid_x + col as f32 * slot_step;
                let sy = grid_y + row as f32 * slot_step;

                // Draw slot background for all positions
                widget_gui.add_quad(
                    sx,
                    sy,
                    slot_size,
                    slot_size,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    [0.0, 0.0, 0.0, 0.5],
                );

                if i < visible_count {
                    widget_gui.register_button(
                        crate::ui::button_ids::CREATIVE_SLOT_BASE + i as u32,
                        sx,
                        sy,
                        slot_size,
                        slot_size,
                    );
                    let hovered = metrics.mouse_pos[0] >= sx
                        && metrics.mouse_pos[0] <= sx + slot_size
                        && metrics.mouse_pos[1] >= sy
                        && metrics.mouse_pos[1] <= sy + slot_size;
                    if hovered {
                        font_gui.fill_rect(sx, sy, slot_size, slot_size, [1.0, 1.0, 1.0, 0.34]);
                        if self.state.hovered_tooltip.is_empty() {
                            let stack = self.creative_stack_for_slot(i);
                            self.state.hovered_tooltip = self.tooltip_lines_for_stack(&stack);
                        }
                    }
                    let item = items[scroll_start + i];
                    let stack = crate::client::inventory::ItemStackView {
                        item_id: item.item_id,
                        count: 1,
                        damage: item.damage,
                        nbt: None,
                    };
                    self.draw_item_icon(
                        block_gui, item_gui, font_gui, sx, sy, slot_size, &stack, gs,
                    );
                }
            }

            // Vanilla always renders the fixed-size thumb; U=244 is its disabled state.
            let scrollbar = creative_scrollbar_geometry(sw, sh, gs);
            let can_scroll = max_scroll_rows > 0;
            creative_gui.add_quad(
                scrollbar.x,
                scrollbar.thumb_y(self.state.creative_scroll),
                12.0 * gs,
                scrollbar.thumb_height,
                if can_scroll {
                    232.0 / 512.0
                } else {
                    244.0 / 512.0
                },
                0.0,
                12.0 / 512.0,
                15.0 / 512.0,
                [1.0, 1.0, 1.0, 1.0],
            );
        }

        // --- Player inventory section ---
        if is_inventory_tab {
            // Vanilla creative inventory tab layout:
            // armor slots are 2x2 on the left, inventory on the right, hotbar at the bottom.
            for j in 5..9 {
                let rel = j - 5;
                let col = rel / 2;
                let row = rel % 2;
                let sx = panel_x + (9.0 + col as f32 * 54.0) * gs;
                let sy = panel_y + (6.0 + row as f32 * 27.0) * gs;
                self.draw_inventory_slot(
                    widget_gui,
                    font_gui,
                    block_gui,
                    item_gui,
                    sx,
                    sy,
                    slot_size,
                    self.state.inventory_armor_slots[(j - 5) as usize].clone(),
                    font_sz,
                    gs,
                    Some(j as i16),
                    metrics.mouse_pos,
                );
            }

            let inv_grid_y = panel_y + 54.0 * gs;
            for row in 0..3 {
                for col in 0..9 {
                    let idx = 9 + row * 9 + col;
                    let sx = panel_x + (9.0 + col as f32 * 18.0) * gs;
                    let sy = inv_grid_y + row as f32 * slot_step;
                    self.draw_inventory_slot(
                        widget_gui,
                        font_gui,
                        block_gui,
                        item_gui,
                        sx,
                        sy,
                        slot_size,
                        self.state.inventory_slots[idx].clone(),
                        font_sz,
                        gs,
                        Some(idx as i16),
                        metrics.mouse_pos,
                    );
                }
            }

            let hotbar_y = panel_y + 112.0 * gs;
            for i in 0..9 {
                let sx = panel_x + (9.0 + i as f32 * 18.0) * gs;
                self.draw_inventory_slot(
                    widget_gui,
                    font_gui,
                    block_gui,
                    item_gui,
                    sx,
                    hotbar_y,
                    slot_size,
                    self.state.inventory_slots[i].clone(),
                    font_sz,
                    gs,
                    Some((36 + i) as i16),
                    metrics.mouse_pos,
                );
            }

            let trash_x = panel_x + 173.0 * gs;
            let trash_y = panel_y + 112.0 * gs;
            widget_gui.register_button(
                crate::ui::button_ids::CREATIVE_TRASH,
                trash_x,
                trash_y,
                slot_size,
                slot_size,
            );
            let trash_hovered = metrics.mouse_pos[0] >= trash_x
                && metrics.mouse_pos[0] <= trash_x + slot_size
                && metrics.mouse_pos[1] >= trash_y
                && metrics.mouse_pos[1] <= trash_y + slot_size;
            if trash_hovered {
                font_gui.fill_rect(
                    trash_x,
                    trash_y,
                    slot_size,
                    slot_size,
                    [1.0, 1.0, 1.0, 0.34],
                );
                if self.state.hovered_tooltip.is_empty() {
                    self.state.hovered_tooltip = vec![super::tooltip::TooltipLine {
                        text: self.t_dynamic("inventory.binSlot"),
                        color: [1.0, 1.0, 1.0, 1.0],
                    }];
                }
            }
        } else {
            // Non-inventory tabs: only show hotbar
            let hotbar_y = panel_y + 112.0 * gs;
            for i in 0..9 {
                let sx = grid_x + i as f32 * slot_step;
                let slot = self.state.inventory_slots[i].clone();
                self.draw_inventory_slot(
                    widget_gui,
                    font_gui,
                    block_gui,
                    item_gui,
                    sx,
                    hotbar_y,
                    slot_size,
                    slot,
                    font_sz,
                    gs,
                    Some((36 + i) as i16),
                    metrics.mouse_pos,
                );
            }
        }

        // Cursor stack
        self.draw_cursor_stack(
            font_gui,
            block_gui,
            item_gui,
            metrics.mouse_pos,
            slot_size,
            font_sz,
            gs,
        );

        // Tooltip for hovered slot
        let tooltip = std::mem::take(&mut self.state.hovered_tooltip);
        if self.state.inventory_cursor_slot.is_empty() && !tooltip.is_empty() {
            self.draw_item_tooltip(font_gui, metrics.mouse_pos, &tooltip, font_sz, gs);
        }
    }

    fn draw_container_window_overlay(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        generic54_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        let slot_size = 16.0 * gs;
        let slot_gap = 2.0 * gs;
        let layout = ContainerLayout::for_window(
            &self.state.inventory_window_type,
            self.state.inventory_window_slot_count,
        );
        let use_generic54 = layout.kind == ContainerKind::Generic;
        let generic_rows = layout.rows.clamp(1, 6);
        let panel_w = if use_generic54 {
            176.0 * gs
        } else {
            layout.atlas_region().w * gs
        };
        let panel_h = if use_generic54 {
            (114.0 + generic_rows as f32 * 18.0) * gs
        } else {
            layout.atlas_region().h * gs
        };
        let panel_x = (sw - panel_w) * 0.5;
        let panel_y = (sh - panel_h) * 0.5;
        let player_grid_x = panel_x + layout.player_inventory_x() * gs;
        let container_x = if use_generic54 {
            player_grid_x
        } else {
            panel_x
        };
        let container_y = if use_generic54 {
            panel_y + 18.0 * gs
        } else {
            panel_y
        };
        let main_y = if use_generic54 {
            panel_y + panel_h - 82.0 * gs
        } else {
            panel_y + layout.player_inventory_y() * gs
        };

        if use_generic54 {
            let top_h = generic_rows as f32 * 18.0 + 17.0;
            generic54_gui.add_quad(
                panel_x,
                panel_y,
                panel_w,
                top_h * gs,
                0.0,
                0.0,
                176.0 / 1024.0,
                top_h / 768.0,
                [1.0, 1.0, 1.0, 1.0],
            );
            generic54_gui.add_quad(
                panel_x,
                panel_y + top_h * gs,
                panel_w,
                96.0 * gs,
                0.0,
                126.0 / 768.0,
                176.0 / 1024.0,
                96.0 / 768.0,
                [1.0, 1.0, 1.0, 1.0],
            );
        } else {
            let atlas = layout.atlas_region();
            generic54_gui.add_quad(
                panel_x,
                panel_y,
                atlas.w * gs,
                atlas.h * gs,
                atlas.u(),
                atlas.v(),
                atlas.w / 1024.0,
                atlas.h / 768.0,
                [1.0, 1.0, 1.0, 1.0],
            );
            if layout.kind == ContainerKind::Horse && layout.slot_count > 2 {
                draw_atlas_sprite(
                    generic54_gui,
                    panel_x + 79.0 * gs,
                    panel_y + 17.0 * gs,
                    90.0 * gs,
                    54.0 * gs,
                    atlas.cell,
                    0.0,
                    166.0,
                    90.0,
                    54.0,
                );
            }
        }
        font_gui.draw_text(
            &mut self.font,
            player_grid_x,
            panel_y + 6.0 * gs,
            &self.state.inventory_window_title.clone(),
            font_sz * 0.64,
            [0.25, 0.25, 0.25, 1.0],
        );

        if layout.kind == ContainerKind::Furnace {
            self.draw_furnace_indicators(generic54_gui, panel_x, panel_y, gs);
        } else if layout.kind == ContainerKind::BrewingStand {
            self.draw_brewing_indicators(generic54_gui, panel_x, panel_y, gs);
        } else if layout.kind == ContainerKind::EnchantingTable {
            self.draw_enchanting_indicators(
                widget_gui,
                generic54_gui,
                font_gui,
                panel_x,
                panel_y,
                font_sz,
                gs,
            );
        } else if layout.kind == ContainerKind::Anvil {
            self.draw_anvil_indicators(generic54_gui, font_gui, panel_x, panel_y, font_sz, gs);
        }

        for slot in 0..layout
            .slot_count
            .min(self.state.inventory_window_slot_count)
        {
            let Some((sx, sy)) =
                layout.slot_pos(slot, container_x, container_y, slot_size, slot_gap, gs)
            else {
                continue;
            };
            let item = self
                .state
                .inventory_window_slots
                .get(slot)
                .cloned()
                .unwrap_or_default();
            self.draw_inventory_slot(
                widget_gui,
                font_gui,
                block_gui,
                item_gui,
                sx,
                sy,
                slot_size,
                item,
                font_sz,
                gs,
                Some(slot as i16),
                metrics.mouse_pos,
            );
        }

        font_gui.draw_text(
            &mut self.font,
            player_grid_x,
            main_y - 12.0 * gs,
            self.state.ui_text.get("container.inventory"),
            font_sz * 0.64,
            [0.25, 0.25, 0.25, 1.0],
        );
        for row in 0..3 {
            for col in 0..9 {
                let local_idx = 9 + row * 9 + col;
                let protocol_slot =
                    self.state.inventory_window_slot_count as i16 + row as i16 * 9 + col as i16;
                let sx = player_grid_x + col as f32 * (slot_size + slot_gap);
                let sy = main_y + row as f32 * (slot_size + slot_gap);
                self.draw_inventory_slot(
                    widget_gui,
                    font_gui,
                    block_gui,
                    item_gui,
                    sx,
                    sy,
                    slot_size,
                    self.state.inventory_slots[local_idx].clone(),
                    font_sz,
                    gs,
                    Some(protocol_slot),
                    metrics.mouse_pos,
                );
            }
        }

        let hotbar_y = main_y + 58.0 * gs;
        for col in 0..9 {
            let sx = player_grid_x + col as f32 * (slot_size + slot_gap);
            let protocol_slot = self.state.inventory_window_slot_count as i16 + 27 + col as i16;

            self.draw_inventory_slot(
                widget_gui,
                font_gui,
                block_gui,
                item_gui,
                sx,
                hotbar_y,
                slot_size,
                self.state.inventory_slots[col].clone(),
                font_sz,
                gs,
                Some(protocol_slot),
                metrics.mouse_pos,
            );
        }

        let tooltip = std::mem::take(&mut self.state.hovered_tooltip);
        self.draw_cursor_stack(
            font_gui,
            block_gui,
            item_gui,
            metrics.mouse_pos,
            slot_size,
            font_sz,
            gs,
        );
        if self.state.inventory_cursor_slot.is_empty() {
            self.draw_item_tooltip(font_gui, metrics.mouse_pos, &tooltip, font_sz, gs);
        }
    }

    fn draw_furnace_indicators(
        &mut self,
        generic54_gui: &mut GuiVertexBuilder,
        x: f32,
        y: f32,
        gs: f32,
    ) {
        let burn = window_property(&self.state.inventory_window_properties, 0).max(0) as f32;
        let mut burn_total = window_property(&self.state.inventory_window_properties, 1) as f32;
        if burn_total <= 0.0 {
            burn_total = 200.0;
        }
        let cook = window_property(&self.state.inventory_window_properties, 2).max(0) as f32;
        let cook_total = window_property(&self.state.inventory_window_properties, 3).max(1) as f32;
        let burn_h = (13.0 * (burn / burn_total).clamp(0.0, 1.0)) as i32;
        if burn_h > 0 {
            draw_atlas_sprite(
                generic54_gui,
                x + 56.0 * gs,
                y + (36.0 + 12.0 - burn_h as f32) * gs,
                14.0 * gs,
                (burn_h + 1) as f32 * gs,
                2,
                176.0,
                12.0 - burn_h as f32,
                14.0,
                (burn_h + 1) as f32,
            );
        }
        let cook_w = (24.0 * (cook / cook_total).clamp(0.0, 1.0)) as i32;
        if cook_w > 0 {
            draw_atlas_sprite(
                generic54_gui,
                x + 79.0 * gs,
                y + 34.0 * gs,
                (cook_w + 1) as f32 * gs,
                16.0 * gs,
                2,
                176.0,
                14.0,
                (cook_w + 1) as f32,
                16.0,
            );
        }
    }

    fn draw_brewing_indicators(
        &mut self,
        generic54_gui: &mut GuiVertexBuilder,
        x: f32,
        y: f32,
        gs: f32,
    ) {
        let brew = window_property(&self.state.inventory_window_properties, 0).max(0) as f32;
        if brew <= 0.0 {
            return;
        }
        let progress_h = (28.0 * (1.0 - brew / 400.0)).max(0.0) as i32;
        if progress_h > 0 {
            draw_atlas_sprite(
                generic54_gui,
                x + 97.0 * gs,
                y + 16.0 * gs,
                9.0 * gs,
                progress_h as f32 * gs,
                5,
                176.0,
                0.0,
                9.0,
                progress_h as f32,
            );
        }
        let bubble_h = match (brew as i32 / 2) % 7 {
            0 => 29,
            1 => 24,
            2 => 20,
            3 => 16,
            4 => 11,
            5 => 6,
            _ => 0,
        };
        if bubble_h > 0 {
            draw_atlas_sprite(
                generic54_gui,
                x + 65.0 * gs,
                y + (14 + 29 - bubble_h) as f32 * gs,
                12.0 * gs,
                bubble_h as f32 * gs,
                5,
                185.0,
                (29 - bubble_h) as f32,
                12.0,
                bubble_h as f32,
            );
        }
    }

    fn draw_enchanting_indicators(
        &mut self,
        widget_gui: &mut GuiVertexBuilder,
        generic54_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        grid_x: f32,
        y: f32,
        font_sz: f32,
        gs: f32,
    ) {
        for row in 0..3 {
            let level = window_property(&self.state.inventory_window_properties, row as i16).max(0);
            draw_atlas_sprite(
                generic54_gui,
                grid_x + 60.0 * gs,
                y + (14.0 + row as f32 * 19.0) * gs,
                108.0 * gs,
                19.0 * gs,
                6,
                0.0,
                if level > 0 { 166.0 } else { 185.0 },
                108.0,
                19.0,
            );
            widget_gui.register_button(
                crate::ui::button_ids::ENCHANT_OPTION_BASE + row as u32,
                grid_x + 60.0 * gs,
                y + (14.0 + row as f32 * 19.0) * gs,
                108.0 * gs,
                19.0 * gs,
            );
            font_gui.draw_text(
                &mut self.font,
                grid_x + 142.0 * gs,
                y + (21.0 + row as f32 * 19.0) * gs,
                &if level > 0 {
                    format_text(
                        self.state.ui_text.get("rustcraft.enchant.level"),
                        &[&level.to_string()],
                    )
                } else {
                    "-".to_string()
                },
                font_sz * 0.55,
                [0.50, 0.55, 0.32, 1.0],
            );
        }
    }

    fn draw_anvil_indicators(
        &mut self,
        generic54_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        x: f32,
        y: f32,
        font_sz: f32,
        gs: f32,
    ) {
        let has_input = self
            .state
            .inventory_window_slots
            .first()
            .is_some_and(|slot| !slot.is_empty());
        let has_second = self
            .state
            .inventory_window_slots
            .get(1)
            .is_some_and(|slot| !slot.is_empty());
        let has_output = self
            .state
            .inventory_window_slots
            .get(2)
            .is_some_and(|slot| !slot.is_empty());
        draw_atlas_sprite(
            generic54_gui,
            x + 59.0 * gs,
            y + 20.0 * gs,
            110.0 * gs,
            16.0 * gs,
            7,
            0.0,
            if has_input { 166.0 } else { 182.0 },
            110.0,
            16.0,
        );
        if (has_input || has_second) && !has_output {
            draw_atlas_sprite(
                generic54_gui,
                x + 99.0 * gs,
                y + 45.0 * gs,
                28.0 * gs,
                21.0 * gs,
                7,
                176.0,
                0.0,
                28.0,
                21.0,
            );
        }
        let cost = window_property(&self.state.inventory_window_properties, 0);
        if cost > 0 && has_output {
            let cost_string = cost.to_string();
            let cost_label = format_text(
                &self.state.ui_text.dynamic("container.repair.cost"),
                &[&cost_string],
            );
            font_gui.draw_text(
                &mut self.font,
                x + 100.0 * gs,
                y + 67.0 * gs,
                &cost_label,
                font_sz * 0.55,
                if cost >= 40 {
                    [1.0, 0.25, 0.25, 1.0]
                } else {
                    [0.50, 0.80, 0.20, 1.0]
                },
            );
        }
    }

    pub(super) fn draw_item_count(
        &mut self,
        font_gui: &mut GuiVertexBuilder,
        sx: f32,
        sy: f32,
        slot_size: f32,
        count: u8,
        font_sz: f32,
    ) {
        let text = count.to_string();
        let text_size = font_sz * (8.0 / 9.0);
        let text_width = text
            .chars()
            .map(|ch| self.font.get_or_render(ch, text_size).advance)
            .sum::<f32>();
        let px = slot_size / 16.0;
        // RenderItem.renderItemOverlayIntoGUI: x + 17 - text_width, y + 9.
        let x = sx + 17.0 * px - text_width;
        let y = sy + 9.0 * px;
        font_gui.draw_text(&mut self.font, x, y, &text, text_size, [1.0, 1.0, 1.0, 1.0]);
    }

    fn draw_inventory_slot(
        &mut self,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
        sx: f32,
        sy: f32,
        slot_size: f32,
        slot: ItemStackView,
        font_sz: f32,
        gs: f32,
        protocol_slot: Option<i16>,
        mouse_pos: [f32; 2],
    ) {
        let hovered = mouse_pos[0] >= sx
            && mouse_pos[0] <= sx + slot_size
            && mouse_pos[1] >= sy
            && mouse_pos[1] <= sy + slot_size;
        if let Some(slot_index) = protocol_slot {
            widget_gui.register_button(
                crate::ui::button_ids::INVENTORY_SLOT_BASE + slot_index as u32,
                sx,
                sy,
                slot_size,
                slot_size,
            );
        }
        let count = slot.count;
        if count == 0 {
            return;
        }
        // Track hovered slot for tooltip (avoids one-frame delay from gui_hit_test)
        if hovered && self.state.hovered_tooltip.is_empty() {
            let stack = self.inventory_stack_for_protocol_slot(protocol_slot.unwrap_or(0));
            self.state.hovered_tooltip = self.tooltip_lines_for_stack(&stack);
        }
        self.draw_item_icon(block_gui, item_gui, font_gui, sx, sy, slot_size, &slot, gs);
        draw_durability_bar(font_gui, sx, sy, slot_size, slot.item_id, slot.damage);
        if count > 1 {
            self.draw_item_count(font_gui, sx, sy, slot_size, count, font_sz);
        }
    }

    fn draw_item_icon(
        &mut self,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        sx: f32,
        sy: f32,
        slot_size: f32,
        slot: &ItemStackView,
        gs: f32,
    ) {
        let glint = crate::client::inventory::has_glint(slot);
        if crate::render::item_icons::item_icon_path(slot.item_id, slot.damage).is_some()
            && matches!(slot.item_id, 50 | 75 | 76 | 166)
        {
            self.draw_item_icon_2d_or_block(
                block_gui,
                item_gui,
                font_gui,
                sx,
                sy,
                slot_size,
                slot.item_id,
                slot.damage,
                gs,
                glint,
            );
            return;
        }
        if slot.item_id <= 255 && slot.item_id != 0 {
            let block =
                crate::world::block::Block::from_state((slot.item_id << 4) | (slot.damage & 0x0f));
            if block != crate::world::block::Block::Air
                && block.to_id() == slot.item_id
                && is_full_block_inventory(block)
            {
                if crate::client::block_icon::draw_block_icon(
                    block_gui,
                    sx + slot_size / 2.0,
                    sy + slot_size * 0.5,
                    slot_size * 0.55,
                    slot.item_id,
                    slot.damage,
                ) {
                    return;
                }
            }
        }
        self.draw_item_icon_2d_or_block(
            block_gui,
            item_gui,
            font_gui,
            sx,
            sy,
            slot_size,
            slot.item_id,
            slot.damage,
            gs,
            glint,
        );
    }

    pub(super) fn draw_item_icon_2d_or_block(
        &mut self,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        sx: f32,
        sy: f32,
        slot_size: f32,
        item_id: u16,
        damage: u16,
        gs: f32,
        glint: bool,
    ) {
        // Bow pulling frames are keyed by damage 1-3 for the first-person
        // hand pass; GUI slots always show the standby icon regardless of
        // durability wear.
        let damage = if item_id == 261 { 0 } else { damage };
        let force_item_icon = matches!(item_id, 50 | 75 | 76 | 166);
        if item_id <= 255 && item_id != 0 && !force_item_icon {
            let block = crate::world::block::Block::from_state((item_id << 4) | (damage & 0x0f));
            if block != crate::world::block::Block::Air
                && block.to_id() == item_id
                && is_full_block_inventory(block)
            {
                if crate::client::block_icon::draw_block_icon(
                    block_gui,
                    sx + slot_size / 2.0,
                    sy + slot_size * 0.5,
                    slot_size * 0.55,
                    item_id,
                    damage,
                ) {
                    return;
                }
            }
        }

        let layers = crate::render::item_icons::item_icon_layers(item_id, damage);
        if !layers.is_empty() {
            let margin = 0.0;
            for layer in &layers {
                let Some(index) = crate::render::item_icons::item_icon_entry_index(layer.path)
                else {
                    continue;
                };
                let rect = crate::render::item_icons::item_icon_uv_rect(index);
                item_gui.add_quad(
                    sx + margin,
                    sy + margin,
                    slot_size - 2.0 * margin,
                    slot_size - 2.0 * margin,
                    rect[0],
                    rect[1],
                    rect[2] - rect[0],
                    rect[3] - rect[1],
                    layer.color,
                );
            }
            if glint {
                let tint = crate::client::inventory::glint_tint(self.state.time);
                let rect = crate::render::item_icons::item_icon_uv_rect(
                    crate::render::item_icons::item_icon_entry_index(layers[0].path).unwrap_or(0),
                );
                item_gui.add_quad(
                    sx + margin,
                    sy + margin,
                    slot_size - 2.0 * margin,
                    slot_size - 2.0 * margin,
                    rect[0],
                    rect[1],
                    rect[2] - rect[0],
                    rect[3] - rect[1],
                    tint,
                );
            }
            return;
        }

        font_gui.fill_rect(
            sx + 2.0 * gs,
            sy + 2.0 * gs,
            slot_size - 4.0 * gs,
            slot_size - 4.0 * gs,
            [0.18, 0.18, 0.22, 0.95],
        );
        font_gui.fill_rect(
            sx + 4.0 * gs,
            sy + 4.0 * gs,
            slot_size - 8.0 * gs,
            3.0 * gs,
            [0.62, 0.62, 0.72, 0.9],
        );
        font_gui.draw_text_centered(
            &mut self.font,
            sx + slot_size * 0.5,
            sy + slot_size * 0.46,
            "?",
            slot_size * 0.42,
            [0.95, 0.95, 1.0, 1.0],
        );
    }

    fn draw_cursor_stack(
        &mut self,
        font_gui: &mut GuiVertexBuilder,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
        mouse_pos: [f32; 2],
        slot_size: f32,
        font_sz: f32,
        gs: f32,
    ) {
        let slot = self.state.inventory_cursor_slot.clone();
        if slot.count == 0 {
            return;
        }
        let x = mouse_pos[0] - slot_size * 0.45;
        let y = mouse_pos[1] - slot_size * 0.45;
        font_gui.fill_rect(
            x - 2.0 * gs,
            y - 2.0 * gs,
            slot_size + 4.0 * gs,
            slot_size + 4.0 * gs,
            [0.0, 0.0, 0.0, 0.35],
        );
        self.draw_item_icon(block_gui, item_gui, font_gui, x, y, slot_size, &slot, gs);
        draw_durability_bar(font_gui, x, y, slot_size, slot.item_id, slot.damage);
        if slot.count > 1 {
            self.draw_item_count(font_gui, x, y, slot_size, slot.count, font_sz);
        }
    }

    fn hovered_inventory_tooltip(&self, mouse_pos: [f32; 2]) -> Vec<super::tooltip::TooltipLine> {
        let Some(id) = self.gui_hit_test(mouse_pos[0], mouse_pos[1]) else {
            return Vec::new();
        };
        if let Some(slot) = creative_slot_id(id) {
            let stack = self.creative_stack_for_slot(slot);
            return self.tooltip_lines_for_stack(&stack);
        }
        if id < crate::ui::button_ids::INVENTORY_SLOT_BASE {
            return Vec::new();
        }
        let slot = id - crate::ui::button_ids::INVENTORY_SLOT_BASE;
        if slot >= crate::ui::button_ids::INVENTORY_SLOT_MAX as u32 {
            return Vec::new();
        }
        let stack = self.inventory_stack_for_protocol_slot(slot as i16);
        self.tooltip_lines_for_stack(&stack)
    }

    fn creative_stack_for_slot(&self, slot: usize) -> ItemStackView {
        if self.state.creative_tab == CREATIVE_TAB_INVENTORY {
            return ItemStackView::default();
        }
        let items = creative_tab_entries(self.state.creative_tab);
        let cols = 9usize;
        let rows = 5usize;
        let max_scroll_rows = (items.len() / cols).saturating_sub(rows);
        let scroll_start =
            (self.state.creative_scroll * max_scroll_rows as f32).round() as usize * cols;
        let Some(item) = items.get(scroll_start + slot).copied() else {
            return ItemStackView::default();
        };
        ItemStackView {
            item_id: item.item_id,
            count: 1,
            damage: item.damage,
            nbt: None,
        }
    }

    fn inventory_stack_for_protocol_slot(&self, protocol_slot: i16) -> ItemStackView {
        if self.state.inventory_window_id != 0
            && protocol_slot >= 0
            && (protocol_slot as usize) < self.state.inventory_window_slot_count
        {
            return self
                .state
                .inventory_window_slots
                .get(protocol_slot as usize)
                .cloned()
                .unwrap_or_default();
        }

        if self.state.inventory_window_id != 0
            && protocol_slot >= self.state.inventory_window_slot_count as i16
        {
            let player_slot = protocol_slot - self.state.inventory_window_slot_count as i16;
            return match player_slot {
                0..=26 => self.state.inventory_slots[(player_slot + 9) as usize].clone(),
                27..=35 => self.state.inventory_slots[(player_slot - 27) as usize].clone(),
                _ => ItemStackView::default(),
            };
        }

        match protocol_slot {
            0..=4 => self.state.inventory_crafting_slots[protocol_slot as usize].clone(),
            5..=8 => self.state.inventory_armor_slots[(protocol_slot - 5) as usize].clone(),
            9..=35 => self.state.inventory_slots[protocol_slot as usize].clone(),
            36..=44 => self.state.inventory_slots[(protocol_slot - 36) as usize].clone(),
            _ => ItemStackView::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContainerKind {
    Generic,
    CraftingTable,
    Dispenser,
    Furnace,
    BrewingStand,
    EnchantingTable,
    Anvil,
    Beacon,
    Hopper,
    Villager,
    Horse,
}

#[derive(Clone, Copy, Debug)]
struct ContainerLayout {
    kind: ContainerKind,
    slot_count: usize,
    columns: usize,
    rows: usize,
}

#[derive(Clone, Copy, Debug)]
struct AtlasRegion {
    cell: usize,
    w: f32,
    h: f32,
}

impl AtlasRegion {
    fn u(self) -> f32 {
        ((self.cell % 4) as f32 * 256.0) / 1024.0
    }

    fn v(self) -> f32 {
        ((self.cell / 4) as f32 * 256.0) / 768.0
    }
}

impl ContainerLayout {
    fn for_window(window_type: &str, slot_count: usize) -> Self {
        let normalized = window_type
            .strip_prefix("minecraft:")
            .unwrap_or(window_type);
        let kind = match normalized {
            "crafting_table" | "workbench" => ContainerKind::CraftingTable,
            "dispenser" | "dropper" => ContainerKind::Dispenser,
            "furnace" => ContainerKind::Furnace,
            "brewing_stand" | "brewing" => ContainerKind::BrewingStand,
            "enchanting_table" | "enchanting" => ContainerKind::EnchantingTable,
            "anvil" => ContainerKind::Anvil,
            "beacon" => ContainerKind::Beacon,
            "hopper" => ContainerKind::Hopper,
            "villager" => ContainerKind::Villager,
            "EntityHorse" => ContainerKind::Horse,
            _ => ContainerKind::Generic,
        };
        let slot_count =
            crate::client::inventory::effective_container_slot_count(window_type, slot_count);
        let (columns, rows, slot_count) = match kind {
            ContainerKind::CraftingTable => (3, 3, slot_count.max(10).min(10)),
            ContainerKind::Dispenser => (3, 3, slot_count.max(9).min(9)),
            ContainerKind::Furnace => (3, 2, slot_count.max(3).min(3)),
            ContainerKind::BrewingStand => (4, 2, slot_count.max(4).min(4)),
            ContainerKind::EnchantingTable => (2, 1, slot_count.max(2).min(2)),
            ContainerKind::Anvil => (3, 1, slot_count.max(3).min(3)),
            ContainerKind::Beacon => (1, 1, slot_count.max(1).min(1)),
            ContainerKind::Hopper => (5, 1, slot_count.max(5).min(5)),
            ContainerKind::Villager => (3, 1, slot_count.max(3).min(3)),
            ContainerKind::Horse => (5, 3, slot_count.clamp(2, 17)),
            ContainerKind::Generic => {
                let count = slot_count.min(90);
                (9, ((count + 8) / 9).max(1), count)
            }
        };
        Self {
            kind,
            slot_count,
            columns,
            rows,
        }
    }

    fn atlas_region(&self) -> AtlasRegion {
        match self.kind {
            ContainerKind::CraftingTable => AtlasRegion {
                cell: 1,
                w: 176.0,
                h: 166.0,
            },
            ContainerKind::Furnace => AtlasRegion {
                cell: 2,
                w: 176.0,
                h: 166.0,
            },
            ContainerKind::Dispenser => AtlasRegion {
                cell: 3,
                w: 176.0,
                h: 166.0,
            },
            ContainerKind::Hopper => AtlasRegion {
                cell: 4,
                w: 176.0,
                h: 133.0,
            },
            ContainerKind::BrewingStand => AtlasRegion {
                cell: 5,
                w: 176.0,
                h: 166.0,
            },
            ContainerKind::EnchantingTable => AtlasRegion {
                cell: 6,
                w: 176.0,
                h: 166.0,
            },
            ContainerKind::Anvil => AtlasRegion {
                cell: 7,
                w: 176.0,
                h: 166.0,
            },
            ContainerKind::Beacon => AtlasRegion {
                cell: 8,
                w: 230.0,
                h: 219.0,
            },
            ContainerKind::Villager => AtlasRegion {
                cell: 9,
                w: 176.0,
                h: 166.0,
            },
            ContainerKind::Horse => AtlasRegion {
                cell: 10,
                w: 176.0,
                h: 166.0,
            },
            ContainerKind::Generic => AtlasRegion {
                cell: 0,
                w: 176.0,
                h: 114.0 + self.rows.clamp(1, 6) as f32 * 18.0,
            },
        }
    }

    fn player_inventory_x(&self) -> f32 {
        match self.kind {
            ContainerKind::Beacon => 36.0,
            _ => 8.0,
        }
    }

    fn player_inventory_y(&self) -> f32 {
        match self.kind {
            ContainerKind::Hopper => 51.0,
            ContainerKind::Beacon => 137.0,
            _ => 84.0,
        }
    }

    fn slot_pos(
        &self,
        slot: usize,
        grid_x: f32,
        y: f32,
        slot_size: f32,
        slot_gap: f32,
        gs: f32,
    ) -> Option<(f32, f32)> {
        match self.kind {
            ContainerKind::CraftingTable => match slot {
                0 => Some((grid_x + 124.0 * gs, y + 35.0 * gs)),
                1..=9 => {
                    let idx = slot - 1;
                    let col = idx % 3;
                    let row = idx / 3;
                    Some((
                        grid_x + 30.0 * gs + col as f32 * (slot_size + slot_gap),
                        y + 17.0 * gs + row as f32 * (slot_size + slot_gap),
                    ))
                }
                _ => None,
            },
            ContainerKind::Dispenser => {
                let col = slot % 3;
                let row = slot / 3;
                Some((
                    grid_x + 62.0 * gs + col as f32 * (slot_size + slot_gap),
                    y + 17.0 * gs + row as f32 * (slot_size + slot_gap),
                ))
            }
            ContainerKind::Furnace => match slot {
                0 => Some((grid_x + 56.0 * gs, y + 17.0 * gs)),
                1 => Some((grid_x + 56.0 * gs, y + 53.0 * gs)),
                2 => Some((grid_x + 116.0 * gs, y + 35.0 * gs)),
                _ => None,
            },
            ContainerKind::BrewingStand => match slot {
                0 => Some((grid_x + 56.0 * gs, y + 46.0 * gs)),
                1 => Some((grid_x + 79.0 * gs, y + 53.0 * gs)),
                2 => Some((grid_x + 102.0 * gs, y + 46.0 * gs)),
                3 => Some((grid_x + 79.0 * gs, y + 17.0 * gs)),
                _ => None,
            },
            ContainerKind::EnchantingTable => match slot {
                0 => Some((grid_x + 15.0 * gs, y + 47.0 * gs)),
                1 => Some((grid_x + 35.0 * gs, y + 47.0 * gs)),
                _ => None,
            },
            ContainerKind::Anvil => match slot {
                0 => Some((grid_x + 27.0 * gs, y + 47.0 * gs)),
                1 => Some((grid_x + 76.0 * gs, y + 47.0 * gs)),
                2 => Some((grid_x + 134.0 * gs, y + 47.0 * gs)),
                _ => None,
            },
            ContainerKind::Beacon => match slot {
                0 => Some((grid_x + 136.0 * gs, y + 110.0 * gs)),
                _ => None,
            },
            ContainerKind::Hopper => {
                let col = slot % 5;
                Some((
                    grid_x + 44.0 * gs + col as f32 * (slot_size + slot_gap),
                    y + 20.0 * gs,
                ))
            }
            ContainerKind::Villager => match slot {
                0 => Some((grid_x + 36.0 * gs, y + 53.0 * gs)),
                1 => Some((grid_x + 62.0 * gs, y + 53.0 * gs)),
                2 => Some((grid_x + 120.0 * gs, y + 53.0 * gs)),
                _ => None,
            },
            ContainerKind::Horse => match slot {
                0 => Some((grid_x + 8.0 * gs, y + 18.0 * gs)),
                1 => Some((grid_x + 8.0 * gs, y + 36.0 * gs)),
                2..=16 => {
                    let idx = slot - 2;
                    Some((
                        grid_x + 80.0 * gs + (idx % 5) as f32 * (slot_size + slot_gap),
                        y + 18.0 * gs + (idx / 5) as f32 * (slot_size + slot_gap),
                    ))
                }
                _ => None,
            },
            ContainerKind::Generic => {
                let col = slot % self.columns;
                let row = slot / self.columns;
                Some((
                    grid_x + col as f32 * (slot_size + slot_gap),
                    y + row as f32 * (slot_size + slot_gap),
                ))
            }
        }
    }
}

#[cfg(test)]
mod container_layout_tests {
    use super::{ContainerKind, ContainerLayout};

    #[test]
    fn every_vanilla_special_window_uses_its_native_container() {
        let cases = [
            (
                "minecraft:crafting_table",
                0,
                ContainerKind::CraftingTable,
                10,
            ),
            ("minecraft:dispenser", 9, ContainerKind::Dispenser, 9),
            ("minecraft:dropper", 9, ContainerKind::Dispenser, 9),
            ("minecraft:furnace", 3, ContainerKind::Furnace, 3),
            ("minecraft:brewing_stand", 4, ContainerKind::BrewingStand, 4),
            (
                "minecraft:enchanting_table",
                0,
                ContainerKind::EnchantingTable,
                2,
            ),
            ("minecraft:anvil", 0, ContainerKind::Anvil, 3),
            ("minecraft:beacon", 1, ContainerKind::Beacon, 1),
            ("minecraft:hopper", 5, ContainerKind::Hopper, 5),
            ("minecraft:villager", 3, ContainerKind::Villager, 3),
            ("EntityHorse", 17, ContainerKind::Horse, 17),
        ];

        for (window_type, advertised, expected_kind, expected_slots) in cases {
            let layout = ContainerLayout::for_window(window_type, advertised);
            assert_eq!(layout.kind, expected_kind, "{window_type}");
            assert_eq!(layout.slot_count, expected_slots, "{window_type}");
        }
    }

    #[test]
    fn special_layouts_match_vanilla_dimensions_and_slot_origins() {
        let beacon = ContainerLayout::for_window("minecraft:beacon", 1);
        assert_eq!(
            (beacon.atlas_region().w, beacon.atlas_region().h),
            (230.0, 219.0)
        );
        assert_eq!(
            (beacon.player_inventory_x(), beacon.player_inventory_y()),
            (36.0, 137.0)
        );

        let brewing = ContainerLayout::for_window("minecraft:brewing_stand", 4);
        assert_eq!(
            brewing.slot_pos(0, 0.0, 0.0, 16.0, 2.0, 1.0),
            Some((56.0, 46.0))
        );
        assert_eq!(
            brewing.slot_pos(3, 0.0, 0.0, 16.0, 2.0, 1.0),
            Some((79.0, 17.0))
        );

        let horse = ContainerLayout::for_window("EntityHorse", 17);
        assert_eq!(
            horse.slot_pos(16, 0.0, 0.0, 16.0, 2.0, 1.0),
            Some((152.0, 54.0))
        );
    }
}

/// Get items for a creative tab — MC 1.8.9 ordering.
pub fn creative_tab_items(tab: usize) -> Vec<u16> {
    match tab {
        // Item registry order, filtered by the item's vanilla creative tab.
        CREATIVE_TAB_BLOCK => vec![
            1, 2, 3, 4, 5, 7, 12, 13, 14, 15, 16, 17, 19, 20, 21, 22, 24, 35, 41, 42, 44, 45, 47,
            48, 49, 53, 56, 57, 67, 73, 79, 80, 82, 86, 87, 88, 89, 91, 95, 98, 103, 108, 109, 110,
            112, 114, 121, 126, 128, 129, 133, 134, 135, 136, 139, 153, 155, 156, 159, 162, 163,
            164, 168, 169, 170, 172, 173, 174, 179, 180, 182,
        ],
        CREATIVE_TAB_DECORATION => vec![
            6, 18, 30, 31, 32, 37, 38, 39, 40, 50, 54, 58, 61, 65, 78, 81, 84, 85, 97, 101, 102,
            106, 111, 113, 116, 120, 130, 145, 146, 160, 161, 165, 171, 175, 188, 189, 190, 191,
            192, 321, 323, 355, 389, 390, 397, 416, 425,
        ],
        CREATIVE_TAB_REDSTONE => vec![
            23, 25, 29, 33, 46, 69, 70, 72, 76, 77, 96, 107, 123, 131, 143, 147, 148, 151, 152,
            154, 158, 167, 183, 184, 185, 186, 187, 324, 330, 331, 356, 404, 427, 428, 429, 430,
            431,
        ],
        CREATIVE_TAB_TRANSPORT => vec![27, 28, 66, 157, 328, 329, 333, 342, 343, 398, 407, 408],
        CREATIVE_TAB_MISC => vec![
            138, 325, 326, 327, 332, 335, 339, 340, 341, 352, 368, 381, 383, 384, 385, 386, 395,
            402, 417, 418, 419, 2256, 2257, 2258, 2259, 2260, 2261, 2262, 2263, 2264, 2265, 2266,
            2267,
        ],
        CREATIVE_TAB_SEARCH => vec![],
        CREATIVE_TAB_FOOD => vec![
            260, 282, 297, 319, 320, 322, 349, 350, 354, 357, 360, 363, 364, 365, 366, 367, 375,
            391, 392, 393, 394, 400, 411, 412, 413, 423, 424,
        ],
        CREATIVE_TAB_TOOLS => vec![
            256, 257, 258, 259, 269, 270, 271, 273, 274, 275, 277, 278, 279, 284, 285, 286, 290,
            291, 292, 293, 294, 345, 346, 347, 359, 420, 421,
        ],
        CREATIVE_TAB_COMBAT => vec![
            261, 262, 267, 268, 272, 276, 283, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307,
            308, 309, 310, 311, 312, 313, 314, 315, 316, 317,
        ],
        CREATIVE_TAB_BREWING => vec![370, 373, 374, 376, 377, 378, 379, 380, 382, 396, 414],
        CREATIVE_TAB_MATERIALS => vec![
            263, 264, 265, 266, 280, 281, 287, 288, 289, 295, 296, 318, 334, 336, 337, 338, 344,
            348, 351, 353, 361, 362, 369, 371, 372, 388, 399, 405, 406, 409, 410, 415,
        ],
        CREATIVE_TAB_INVENTORY => vec![],
        _ => vec![],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CreativeItemEntry {
    pub item_id: u16,
    pub damage: u16,
}

pub fn creative_tab_entries(tab: usize) -> Vec<CreativeItemEntry> {
    if tab == CREATIVE_TAB_SEARCH {
        let mut seen = std::collections::HashSet::new();
        let mut all = Vec::new();
        for source_tab in [
            CREATIVE_TAB_BLOCK,
            CREATIVE_TAB_DECORATION,
            CREATIVE_TAB_REDSTONE,
            CREATIVE_TAB_TRANSPORT,
            CREATIVE_TAB_MISC,
            CREATIVE_TAB_FOOD,
            CREATIVE_TAB_TOOLS,
            CREATIVE_TAB_COMBAT,
            CREATIVE_TAB_BREWING,
            CREATIVE_TAB_MATERIALS,
        ] {
            for entry in creative_tab_entries(source_tab) {
                if seen.insert(entry) {
                    all.push(entry);
                }
            }
        }
        return all;
    }

    let mut entries: Vec<_> = creative_tab_items(tab)
        .into_iter()
        .map(|item_id| CreativeItemEntry { item_id, damage: 0 })
        .collect();
    let mut add_variants = |item_id: u16, damages: &[u16]| {
        for &damage in damages {
            let entry = CreativeItemEntry { item_id, damage };
            if !entries.contains(&entry) {
                entries.push(entry);
            }
        }
    };

    match tab {
        CREATIVE_TAB_BLOCK => {
            add_variants(1, &[1, 2, 3, 4, 5, 6]);
            add_variants(3, &[1, 2]);
            add_variants(5, &[1, 2, 3, 4, 5]);
            add_variants(12, &[1]);
            add_variants(17, &[1, 2, 3]);
            add_variants(24, &[1, 2]);
            add_variants(35, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
            add_variants(44, &[1, 2, 3, 4, 5, 6, 7]);
            add_variants(95, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
            add_variants(98, &[1, 2, 3]);
            add_variants(126, &[1, 2, 3, 4, 5]);
            add_variants(139, &[1]);
            add_variants(155, &[1, 2]);
            add_variants(159, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
            add_variants(162, &[1]);
            add_variants(168, &[1, 2]);
            add_variants(179, &[1, 2]);
        }
        CREATIVE_TAB_DECORATION => {
            add_variants(6, &[1, 2, 3, 4, 5]);
            add_variants(18, &[1, 2, 3]);
            add_variants(31, &[1, 2]);
            add_variants(38, &[1, 2, 3, 4, 5, 6, 7, 8]);
            add_variants(97, &[1, 2, 3, 4, 5]);
            add_variants(145, &[1, 2]);
            add_variants(160, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
            add_variants(161, &[1]);
            add_variants(171, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
            add_variants(175, &[1, 2, 3, 4, 5]);
            add_variants(397, &[1, 2, 3, 4]);
            add_variants(425, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        }
        CREATIVE_TAB_MISC => {
            add_variants(
                383,
                &[
                    50, 51, 52, 54, 55, 56, 57, 58, 59, 60, 61, 62, 65, 66, 67, 68, 90, 91, 92, 93,
                    94, 95, 96, 98, 100, 101, 120,
                ],
            );
        }
        CREATIVE_TAB_FOOD => {
            add_variants(322, &[1]);
            add_variants(349, &[1, 2, 3]);
            add_variants(350, &[1]);
        }
        CREATIVE_TAB_BREWING => {
            add_variants(
                373,
                &[
                    16, 32, 64, 8193, 8194, 8195, 8196, 8197, 8198, 8200, 8201, 8202, 8204, 8205,
                    8206, 8225, 8226, 8228, 8229, 8233, 8234, 16385, 16386, 16387, 16388, 16389,
                    16390, 16392, 16393, 16394, 16396, 16397, 16398,
                ],
            );
        }
        CREATIVE_TAB_MATERIALS => {
            add_variants(263, &[1]);
            add_variants(351, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        }
        _ => {}
    }
    entries
}

pub fn creative_max_scroll_rows(item_count: usize) -> usize {
    item_count.div_ceil(9).saturating_sub(5)
}

fn vanilla_preview_rotation(
    anchor_x: f32,
    anchor_bottom: f32,
    mouse_pos: [f32; 2],
    gui_scale: f32,
) -> (f32, f32) {
    let scale = gui_scale.max(1.0);
    let mouse_x = (anchor_x - mouse_pos[0]) / scale;
    let mouse_y = (anchor_bottom - 30.0 * scale - mouse_pos[1]) / scale;
    (
        (mouse_x / 40.0).atan() * 20.0,
        (mouse_y / 40.0).atan() * 20.0,
    )
}

fn draw_atlas_sprite(
    builder: &mut GuiVertexBuilder,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    cell: usize,
    src_x: f32,
    src_y: f32,
    src_w: f32,
    src_h: f32,
) {
    let cell_x = (cell % 4) as f32 * 256.0;
    let cell_y = (cell / 4) as f32 * 256.0;
    builder.add_quad(
        x,
        y,
        w,
        h,
        (cell_x + src_x) / 1024.0,
        (cell_y + src_y) / 768.0,
        src_w / 1024.0,
        src_h / 768.0,
        [1.0, 1.0, 1.0, 1.0],
    );
}

/// Draw the active potion effects panel to the right of the survival inventory.
/// This is a free function (not a method) to avoid confusion with the impl block.
pub fn draw_potion_effects_inventory(
    renderer: &mut Renderer,
    metrics: &MenuMetrics,
    inventory_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
) {
    let effects = &renderer.state.active_potion_effects;
    if effects.is_empty() {
        return;
    }
    let gs = metrics.gs;
    let font_sz = metrics.font_sz;
    let sw = renderer.swapchain_extent.width as f32;
    let inv_w = 176.0 * gs;
    let panel_x = (sw - inv_w) * 0.5;
    let panel_right = panel_x + inv_w;
    let start_y = (renderer.swapchain_extent.height as f32 - 166.0 * gs) * 0.5;

    let effect_w = 140.0 * gs;
    let effect_h = 32.0 * gs;
    let effect_x = panel_right + 4.0 * gs;
    let row_step = if effects.len() > 5 {
        132.0 / (effects.len() - 1) as f32
    } else {
        33.0
    } * gs;

    for (i, effect) in effects.iter().enumerate() {
        let ey = start_y + i as f32 * row_step;

        inventory_gui.add_quad(
            effect_x,
            ey,
            effect_w,
            effect_h,
            0.0,
            166.0 / 256.0,
            140.0 / 256.0,
            32.0 / 256.0,
            [1.0, 1.0, 1.0, 1.0],
        );

        if let Some(icon_idx) = crate::entity::potion_icon_index(effect.effect_id) {
            let icon_col = icon_idx as f32 % 8.0;
            let icon_row = (icon_idx as f32 / 8.0).floor();
            let icon_size = 18.0 * gs;
            inventory_gui.add_quad(
                effect_x + 6.0 * gs,
                ey + 7.0 * gs,
                icon_size,
                icon_size,
                icon_col * 18.0 / 256.0,
                (198.0 + icon_row * 18.0) / 256.0,
                18.0 / 256.0,
                18.0 / 256.0,
                [1.0, 1.0, 1.0, 1.0],
            );
        }

        let name_key = crate::entity::potion_effect_name(effect.effect_id);
        let name = renderer.state.ui_text.dynamic(name_key);
        let amp_str = crate::entity::potion_amplifier_string(effect.amplifier);
        let display = if amp_str.is_empty() {
            name
        } else {
            format!("{} {}", name, amp_str)
        };
        font_gui.draw_text(
            &mut renderer.font,
            effect_x + 28.0 * gs,
            ey + 6.0 * gs,
            &display,
            font_sz * 0.9,
            [1.0, 1.0, 1.0, 1.0],
        );

        let dur = crate::entity::potion_duration_string(effect.duration);
        font_gui.draw_text(
            &mut renderer.font,
            effect_x + 28.0 * gs,
            ey + 6.0 * gs + font_sz * 1.0,
            &dur,
            font_sz * 0.7,
            [0.7, 0.7, 0.7, 1.0],
        );
    }
}

fn is_full_block_inventory(block: crate::world::block::Block) -> bool {
    use crate::world::block::Block;
    // Blocks that look wrong as 3D cubes in inventory — render as flat item instead
    !matches!(
        block,
        Block::GlassPane
            | Block::StainedGlassPane
            | Block::IronBars
            | Block::OakFence
            | Block::SpruceFence
            | Block::BirchFence
            | Block::JungleFence
            | Block::DarkOakFence
            | Block::AcaciaFence
            | Block::NetherBrickFence
            | Block::CobblestoneWall
            | Block::Cobweb
            | Block::Ladder
            | Block::Vine
            | Block::SugarCane
            | Block::Cactus
            | Block::Cake
    )
}

fn window_property(properties: &[(i16, i16)], property: i16) -> i16 {
    properties
        .iter()
        .find(|(key, _)| *key == property)
        .map(|(_, value)| *value)
        .unwrap_or(0)
}

fn creative_slot_id(id: u32) -> Option<usize> {
    if id < crate::ui::button_ids::CREATIVE_SLOT_BASE {
        return None;
    }
    let slot = id - crate::ui::button_ids::CREATIVE_SLOT_BASE;
    (slot < crate::ui::button_ids::CREATIVE_SLOT_MAX as u32).then_some(slot as usize)
}

#[cfg(test)]
mod creative_tests {
    use super::*;

    #[test]
    fn search_tab_contains_metadata_variants() {
        let entries = creative_tab_entries(CREATIVE_TAB_SEARCH);
        assert!(entries.contains(&CreativeItemEntry {
            item_id: 35,
            damage: 15,
        }));
        assert!(entries.contains(&CreativeItemEntry {
            item_id: 383,
            damage: 120,
        }));
        assert!(entries.contains(&CreativeItemEntry {
            item_id: 373,
            damage: 16385,
        }));
    }

    #[test]
    fn creative_items_have_a_visual_mapping() {
        for entry in creative_tab_entries(CREATIVE_TAB_SEARCH) {
            if entry.item_id > 255 {
                assert!(
                    crate::render::item_icons::item_icon_path(entry.item_id, entry.damage)
                        .is_some(),
                    "creative item {}:{} has no icon mapping",
                    entry.item_id,
                    entry.damage
                );
            }
        }
    }

    #[test]
    fn creative_scroll_includes_a_partial_last_row() {
        assert_eq!(creative_max_scroll_rows(45), 0);
        assert_eq!(creative_max_scroll_rows(46), 1);
        assert_eq!(creative_max_scroll_rows(54), 1);
        assert_eq!(creative_max_scroll_rows(55), 2);
    }

    #[test]
    fn vanilla_representative_items_are_in_only_their_own_tab() {
        let cases = [
            (258, CREATIVE_TAB_TOOLS),
            (307, CREATIVE_TAB_COMBAT),
            (373, CREATIVE_TAB_BREWING),
            (260, CREATIVE_TAB_FOOD),
            (265, CREATIVE_TAB_MATERIALS),
            (328, CREATIVE_TAB_TRANSPORT),
            (331, CREATIVE_TAB_REDSTONE),
        ];
        let tabs = [
            CREATIVE_TAB_BLOCK,
            CREATIVE_TAB_DECORATION,
            CREATIVE_TAB_REDSTONE,
            CREATIVE_TAB_TRANSPORT,
            CREATIVE_TAB_MISC,
            CREATIVE_TAB_FOOD,
            CREATIVE_TAB_TOOLS,
            CREATIVE_TAB_COMBAT,
            CREATIVE_TAB_BREWING,
            CREATIVE_TAB_MATERIALS,
        ];

        for (item_id, expected_tab) in cases {
            for tab in tabs {
                assert_eq!(
                    creative_tab_items(tab).contains(&item_id),
                    tab == expected_tab,
                    "item {item_id} has the wrong creative-tab membership"
                );
            }
        }
    }

    #[test]
    fn base_items_do_not_leak_between_creative_tabs() {
        let mut owner = std::collections::HashMap::new();
        for tab in 0..=CREATIVE_TAB_MATERIALS {
            if tab == CREATIVE_TAB_SEARCH {
                continue;
            }
            for item_id in creative_tab_items(tab) {
                assert_eq!(
                    owner.insert(item_id, tab),
                    None,
                    "item {item_id} appears in more than one creative tab"
                );
            }
        }
    }

    #[test]
    fn creative_scrollbar_uses_vanilla_track_and_drag_formula() {
        let geometry = creative_scrollbar_geometry(800.0, 600.0, 2.0);
        assert_eq!(geometry.x, 555.0);
        assert_eq!(geometry.y, 200.0);
        assert_eq!(geometry.width, 28.0);
        assert_eq!(geometry.height, 224.0);
        assert!(geometry.contains(555.0, 200.0));
        assert!(!geometry.contains(583.0, 200.0));
        assert_eq!(geometry.scroll_for_mouse_y(215.0), 0.0);
        assert_eq!(geometry.scroll_for_mouse_y(409.0), 1.0);
    }

    #[test]
    fn player_preview_faces_the_cursor_at_rest() {
        let (yaw, pitch) = vanilla_preview_rotation(100.0, 100.0, [100.0, 70.0], 1.0);
        assert_eq!(yaw, 0.0);
        assert_eq!(pitch, 0.0);
    }

    #[test]
    fn player_preview_vertical_rotation_follows_the_cursor() {
        let (_, looking_up) = vanilla_preview_rotation(100.0, 100.0, [100.0, 50.0], 1.0);
        let (_, looking_down) = vanilla_preview_rotation(100.0, 100.0, [100.0, 90.0], 1.0);
        assert!(looking_up > 0.0);
        assert!(looking_down < 0.0);
    }
}
