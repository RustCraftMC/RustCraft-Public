use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;
use crate::ui::format::format_text;

impl Renderer {
    pub(super) fn draw_crosshair(
        &self,
        gs: f32,
        sw: f32,
        sh: f32,
        font_gui: &mut GuiVertexBuilder,
    ) {
        if self.state.chat_open || self.state.inventory_open {
            return;
        }
        let cx = sw / 2.0;
        let cy = sh / 2.0;
        let cl = 6.0 * gs;
        let cw = gs;
        font_gui.fill_rect(cx - cl, cy - cw / 2.0, cl * 2.0, cw, [1.0, 1.0, 1.0, 0.8]);
        font_gui.fill_rect(cx - cw / 2.0, cy - cl, cw, cl * 2.0, [1.0, 1.0, 1.0, 0.8]);
    }

    pub(super) fn draw_hotbar_and_status(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
        icons_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        let slot_size = 16.0 * gs;
        let hotbar_w = 182.0 * gs;
        let hotbar_x = (sw - hotbar_w) / 2.0;
        let hotbar_y = sh - 22.0 * gs - 4.0 * gs;

        widget_gui.draw_hotbar_bg(hotbar_x, hotbar_y, hotbar_w, 22.0 * gs);
        let sel_x = hotbar_x + self.state.hotbar_selected as f32 * 20.0 * gs;
        widget_gui.draw_hotbar_select(sel_x - 1.0 * gs, hotbar_y - 1.0 * gs, 24.0 * gs, 24.0 * gs);

        for i in 0..9 {
            let sx = hotbar_x + 3.0 * gs + i as f32 * 20.0 * gs + 2.0 * gs;
            let sy = hotbar_y + 3.0 * gs;
            let (item_id, count, damage) = self.state.hotbar_slots[i];
            if count == 0 {
                continue;
            }
            self.draw_item_icon_2d_or_block(
                block_gui, item_gui, font_gui, sx, sy, slot_size, item_id, damage, gs, false,
            );
            super::inventory::draw_durability_bar(font_gui, sx, sy, slot_size, item_id, damage);
            if count > 1 {
                self.draw_item_count(font_gui, sx, sy, slot_size, count, font_sz);
            }
        }

        let status_y = hotbar_y - 20.0 * gs;
        let heart_size = 9.0 * gs;
        let is_creative = self.state.gamemode == 1;
        let is_spectator = self.state.gamemode == 3;
        let show_status = !is_creative && !is_spectator;

        if show_status {
            // MC 1.8.9 layout (bottom to top):
            //   Health hearts (left) + Food (right)     — row 0
            //   Absorption golden hearts (left)           — row 1 (if any)
            //   Armor bar (left)                          — row 2 (if any)
            // Icons.png UV (256x256):
            // Row 0 (y=0): Hearts — empty(16,0) full(52,0) half(61,0)
            //   absorption full(160,0) half(169,0)
            // Row 1 (y=9): Armor — empty(16,9) half(25,9) full(34,9)
            // Row 3 (y=27): Food — empty(16,27) full(52,27) half(61,27)

            // Vanilla hearts use 9px texture cells at 8px spacing.
            let heart_spacing = 8.0 * gs;

            let abs_amount = self.state.absorption;
            let has_abs = abs_amount > 0.0;
            let has_armor = self.state.armor_points > 0;

            // Row 0: health (left) + food (right)
            let health_y = status_y;

            // Row 1: absorption above health (if present)
            let abs_y = if has_abs {
                status_y - heart_size - 1.0 * gs
            } else {
                status_y
            };

            // Row 2: armor above absorption (or above health if no absorption)
            let armor_y = if has_armor {
                if has_abs {
                    abs_y - heart_size - 1.0 * gs
                } else {
                    status_y - heart_size - 1.0 * gs
                }
            } else {
                status_y
            };

            // -- Absorption hearts (golden, no empty background) --
            if has_abs {
                let full_abs = (abs_amount / 2.0).floor().clamp(0.0, 10.0) as i32;
                let has_half_abs = abs_amount % 2.0 >= 0.5 && full_abs < 10;
                for i in 0..full_abs {
                    let x = hotbar_x + i as f32 * heart_spacing;
                    icons_gui.add_quad(
                        x,
                        abs_y,
                        heart_size,
                        heart_size,
                        160.0 / 256.0,
                        0.0 / 256.0,
                        9.0 / 256.0,
                        9.0 / 256.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
                if has_half_abs {
                    let x = hotbar_x + full_abs as f32 * heart_spacing;
                    icons_gui.add_quad(
                        x,
                        abs_y,
                        heart_size,
                        heart_size,
                        169.0 / 256.0,
                        0.0 / 256.0,
                        9.0 / 256.0,
                        9.0 / 256.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
            }

            // -- Health hearts --
            let flash_health =
                self.state.health_timer > 0 && (self.state.health_timer / 3) % 2 == 1;
            let display_health = if flash_health && self.state.prev_health > 0.0 {
                self.state.prev_health
            } else {
                self.state.health
            };
            let full_hearts = (display_health / 2.0).floor().clamp(0.0, 10.0) as i32;
            let has_half = display_health % 2.0 >= 0.5 && full_hearts < 10;

            for i in 0..10 {
                let x = hotbar_x + i as f32 * heart_spacing;
                icons_gui.add_quad(
                    x,
                    health_y,
                    heart_size,
                    heart_size,
                    16.0 / 256.0,
                    0.0 / 256.0,
                    9.0 / 256.0,
                    9.0 / 256.0,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
            for i in 0..full_hearts {
                let x = hotbar_x + i as f32 * heart_spacing;
                icons_gui.add_quad(
                    x,
                    health_y,
                    heart_size,
                    heart_size,
                    52.0 / 256.0,
                    0.0 / 256.0,
                    9.0 / 256.0,
                    9.0 / 256.0,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
            if has_half {
                let x = hotbar_x + full_hearts as f32 * heart_spacing;
                icons_gui.add_quad(
                    x,
                    health_y,
                    heart_size,
                    heart_size,
                    61.0 / 256.0,
                    0.0 / 256.0,
                    9.0 / 256.0,
                    9.0 / 256.0,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }

            // -- Armor bar --
            if has_armor {
                let armor_points = self.state.armor_points.min(20);
                let full_armor = (armor_points / 2) as i32;
                let has_half_armor = armor_points % 2 == 1 && full_armor < 10;

                for i in 0..10 {
                    let x = hotbar_x + i as f32 * heart_spacing;
                    icons_gui.add_quad(
                        x,
                        armor_y,
                        heart_size,
                        heart_size,
                        16.0 / 256.0,
                        9.0 / 256.0,
                        9.0 / 256.0,
                        9.0 / 256.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
                for i in 0..full_armor {
                    let x = hotbar_x + i as f32 * heart_spacing;
                    icons_gui.add_quad(
                        x,
                        armor_y,
                        heart_size,
                        heart_size,
                        34.0 / 256.0,
                        9.0 / 256.0,
                        9.0 / 256.0,
                        9.0 / 256.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
                if has_half_armor {
                    let x = hotbar_x + full_armor as f32 * heart_spacing;
                    icons_gui.add_quad(
                        x,
                        armor_y,
                        heart_size,
                        heart_size,
                        25.0 / 256.0,
                        9.0 / 256.0,
                        9.0 / 256.0,
                        9.0 / 256.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
            }

            // Hunger shanks using icons.png
            // MC 1.8.9: empty(16,27)  full(52,27)  half(61,27)
            let food_count = self.state.food.clamp(0, 20);
            let flash_food = self.state.food_timer > 0 && (self.state.food_timer / 3) % 2 == 1;
            let display_food = if flash_food && self.state.prev_food > 0 {
                self.state.prev_food
            } else {
                food_count
            };
            let full_food = (display_food / 2) as i32;
            let has_half_food = food_count % 2 == 1 && full_food < 10;

            for i in 0..10 {
                let x = hotbar_x + hotbar_w - (i as f32 + 1.0) * heart_spacing;
                icons_gui.add_quad(
                    x,
                    health_y,
                    heart_size,
                    heart_size,
                    16.0 / 256.0,
                    27.0 / 256.0,
                    9.0 / 256.0,
                    9.0 / 256.0,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
            for i in 0..full_food {
                let x = hotbar_x + hotbar_w - (i as f32 + 1.0) * heart_spacing;
                icons_gui.add_quad(
                    x,
                    health_y,
                    heart_size,
                    heart_size,
                    52.0 / 256.0,
                    27.0 / 256.0,
                    9.0 / 256.0,
                    9.0 / 256.0,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
            if has_half_food {
                let x = hotbar_x + hotbar_w - (full_food as f32 + 1.0) * heart_spacing;
                icons_gui.add_quad(
                    x,
                    health_y,
                    heart_size,
                    heart_size,
                    61.0 / 256.0,
                    27.0 / 256.0,
                    9.0 / 256.0,
                    9.0 / 256.0,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
        }

        // Experience bar — hidden in creative/spectator mode (MC 1.8.9)
        if show_status {
            let xp_y = hotbar_y - 8.0 * gs;
            let xp_bar_h = 5.0 * gs;
            icons_gui.add_quad(
                hotbar_x,
                xp_y,
                hotbar_w,
                xp_bar_h,
                0.0 / 256.0,
                64.0 / 256.0,
                182.0 / 256.0,
                5.0 / 256.0,
                [1.0, 1.0, 1.0, 1.0],
            );
            let xp_progress = self.state.experience_bar.clamp(0.0, 1.0);
            if xp_progress > 0.0 {
                icons_gui.add_quad(
                    hotbar_x,
                    xp_y,
                    hotbar_w * xp_progress,
                    xp_bar_h,
                    0.0 / 256.0,
                    69.0 / 256.0,
                    182.0 / 256.0 * xp_progress,
                    5.0 / 256.0,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
            // XP level number (white text with shadow, vanilla style)
            if self.state.experience_level > 0 {
                font_gui.draw_text_shadowed(
                    &mut self.font,
                    sw / 2.0,
                    hotbar_y - 21.0 * gs,
                    &self.state.experience_level.to_string(),
                    font_sz * 0.85,
                    [1.0, 1.0, 1.0, 1.0],
                    gs,
                );
            }
        } // end if show_status

        // Action bar — vanilla MC 1.8.9 GuiIngame.renderGameOverlay
        if let Some(ref text) = self.state.action_bar {
            let action_font_sz = font_sz * 0.85;
            let bar_y = hotbar_y - 37.0 * gs;
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                bar_y,
                text,
                action_font_sz,
                [1.0, 1.0, 1.0, 0.95],
            );
        }

        if self.state.raining {
            let label = if self.state.thunder_level > 0.05 {
                self.state.ui_text.get("rustcraft.weather.thunderstorm")
            } else {
                self.state.ui_text.get("rustcraft.weather.rain")
            };
            font_gui.fill_rect(
                8.0 * gs,
                sh - 42.0 * gs,
                74.0 * gs,
                12.0 * gs,
                [0.0, 0.0, 0.0, 0.38],
            );
            font_gui.draw_text(
                &mut self.font,
                12.0 * gs,
                sh - 39.0 * gs,
                label,
                font_sz * 0.62,
                [0.62, 0.76, 0.95, 0.90],
            );
        }
    }

    pub(super) fn draw_potion_effects_hud(
        &mut self,
        metrics: &MenuMetrics,
        inventory_gui: &mut GuiVertexBuilder,
    ) {
        let effects = &self.state.active_potion_effects;
        if effects.is_empty() {
            return;
        }
        let gs = metrics.gs;
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let icon_size = 18.0 * gs;
        let icon_step = 20.0 * gs;
        let icon_x = sw - icon_size - 2.0 * gs;
        let max_visible = ((sh - 4.0 * gs) / icon_step).max(0.0) as usize;

        for (i, icon_idx) in effects
            .iter()
            .filter_map(|effect| crate::entity::potion_icon_index(effect.effect_id))
            .take(max_visible)
            .enumerate()
        {
            let icon_col = icon_idx as f32 % 8.0;
            let icon_row = (icon_idx as f32 / 8.0).floor();

            inventory_gui.add_quad(
                icon_x,
                2.0 * gs + i as f32 * icon_step,
                icon_size,
                icon_size,
                icon_col * 18.0 / 256.0,
                (198.0 + icon_row * 18.0) / 256.0,
                18.0 / 256.0,
                18.0 / 256.0,
                [1.0, 1.0, 1.0, 1.0],
            );
        }
    }

    pub(super) fn draw_resource_pack_notice(
        &mut self,
        metrics: &MenuMetrics,
        overlay_gui: &mut GuiVertexBuilder,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let Some(status) = self.state.resource_pack_status.clone() else {
            return;
        };
        if self.state.chat_open || self.state.inventory_open || self.state.health <= 0.0 {
            return;
        }
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        if status.starts_with("available") {
            let w = 260.0 * gs;
            let h = 92.0 * gs;
            let x = (sw - w) * 0.5;
            let y = (sh - h) * 0.35;
            overlay_gui.fill_rect(0.0, 0.0, sw, sh, [0.0, 0.0, 0.0, 0.35]);
            overlay_gui.fill_rect(x, y, w, h, [0.0, 0.0, 0.0, 0.82]);
            overlay_gui.fill_rect(x, y, w, 1.0 * gs, [0.65, 0.65, 0.65, 0.9]);
            font_gui.draw_text_shadowed(
                &mut self.font,
                sw / 2.0,
                y + 10.0 * gs,
                self.state.ui_text.get("resourcePack.title"),
                font_sz,
                [1.0, 1.0, 1.0, 1.0],
                gs,
            );
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                y + 32.0 * gs,
                self.state.ui_text.get("rustcraft.resourcePack.prompt"),
                font_sz * 0.72,
                [0.82, 0.82, 0.82, 1.0],
            );
            let bw = 96.0 * gs;
            let by = y + 58.0 * gs;
            crate::render::gui::widgets::draw_button(
                metrics,
                widget_gui,
                font_gui,
                crate::ui::button_ids::RESOURCE_PACK_ACCEPT,
                [sw / 2.0 - bw - 4.0 * gs, by, bw, metrics.btn_h],
                self.state.ui_text.get("rustcraft.resourcePack.accept"),
                &mut self.font,
            );
            crate::render::gui::widgets::draw_button(
                metrics,
                widget_gui,
                font_gui,
                crate::ui::button_ids::RESOURCE_PACK_DECLINE,
                [sw / 2.0 + 4.0 * gs, by, bw, metrics.btn_h],
                self.state.ui_text.get("rustcraft.resourcePack.decline"),
                &mut self.font,
            );
            return;
        }

        let w = 210.0 * gs;
        let x = sw - w - 8.0 * gs;
        let y = 8.0 * gs;
        font_gui.fill_rect(x, y, w, 30.0 * gs, [0.0, 0.0, 0.0, 0.55]);
        font_gui.fill_rect(x, y, w, 1.0 * gs, [0.55, 0.55, 0.55, 0.75]);
        font_gui.draw_text(
            &mut self.font,
            x + 6.0 * gs,
            y + 5.0 * gs,
            self.state.ui_text.get("rustcraft.resourcePack.status"),
            font_sz * 0.72,
            [0.95, 0.95, 0.95, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            x + 6.0 * gs,
            y + 17.0 * gs,
            &status,
            font_sz * 0.58,
            [0.70, 0.82, 0.95, 1.0],
        );
    }

    pub(super) fn draw_death_screen(
        &mut self,
        metrics: &MenuMetrics,
        overlay_gui: &mut GuiVertexBuilder,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        if self.state.health > 0.0 {
            return;
        }
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let btn_w = metrics.btn_w;
        let btn_h = metrics.btn_h;
        let btn_x = metrics.btn_x;
        // Vanilla GuiGameOver.drawScreen: drawGradientRect(0, 0, w, h,
        // 0x60500000, 0xA0803030), rendered behind the buttons.
        overlay_gui.fill_rect_gradient(
            0.0,
            0.0,
            sw,
            sh,
            [80.0 / 255.0, 0.0, 0.0, 96.0 / 255.0],
            [128.0 / 255.0, 48.0 / 255.0, 48.0 / 255.0, 160.0 / 255.0],
        );
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            sh / 4.0,
            self.state.ui_text.get("deathScreen.title"),
            22.0 * gs,
            [1.0, 1.0, 1.0, 1.0],
            gs,
        );
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            sh / 4.0 + 24.0 * gs,
            &format_text(
                self.state.ui_text.get("deathScreen.score"),
                &[&self.state.experience_total.max(0).to_string()],
            ),
            metrics.font_sz,
            [1.0, 1.0, 1.0, 1.0],
            gs,
        );
        crate::render::gui::widgets::draw_button(
            metrics,
            widget_gui,
            font_gui,
            crate::ui::button_ids::RESPAWN,
            [btn_x, sh / 4.0 + 72.0 * gs, btn_w, btn_h],
            self.state.ui_text.get("deathScreen.respawn"),
            &mut self.font,
        );
        crate::render::gui::widgets::draw_button(
            metrics,
            widget_gui,
            font_gui,
            crate::ui::button_ids::DEATH_TITLE_SCREEN,
            [btn_x, sh / 4.0 + 100.0 * gs, btn_w, btn_h],
            self.state.ui_text.get("deathScreen.titleScreen"),
            &mut self.font,
        );
    }
}
