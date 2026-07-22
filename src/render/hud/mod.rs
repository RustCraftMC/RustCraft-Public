mod book;
mod chat;
mod debug;
pub mod entities;
pub mod hand;
pub(crate) mod inventory;
mod scoreboard;
mod status;
pub(crate) mod tooltip;

use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;

pub struct HudBatches<'a> {
    /// Untextured full-screen overlay batch, rendered behind every other GUI
    /// batch. Full-screen dims/masks must go here so they never cover widgets.
    pub overlay: &'a mut GuiVertexBuilder,
    pub widget: &'a mut GuiVertexBuilder,
    pub inventory: &'a mut GuiVertexBuilder,
    pub generic54: &'a mut GuiVertexBuilder,
    pub font: &'a mut GuiVertexBuilder,
    pub block: &'a mut GuiVertexBuilder,
    pub item: &'a mut GuiVertexBuilder,
    pub icons: &'a mut GuiVertexBuilder,
    pub creative: &'a mut GuiVertexBuilder,
}

impl Renderer {
    pub(super) fn draw_ingame_hud(&mut self, metrics: &MenuMetrics, batches: HudBatches<'_>) {
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;

        if self.state.settings.hud_visible() {
            let script_before = self.state.hud.script_hud_before_commands().clone();
            self.draw_script_hud(
                &script_before,
                metrics,
                batches.font,
                batches.widget,
                batches.icons,
                batches.inventory,
            );
            if self.state.settings.debug_overlay() {
                self.draw_debug_panel(metrics, batches.font);
            }
            self.draw_entity_overlays(metrics, batches.font, batches.block);
            self.draw_block_selection(metrics, batches.font);
            if self.state.settings.crosshair_visible() {
                self.draw_crosshair(gs, sw, sh, batches.font);
            }
            self.draw_potion_effects_hud(metrics, batches.inventory);
            self.draw_hotbar_and_status(
                metrics,
                batches.widget,
                batches.font,
                batches.block,
                batches.item,
                batches.icons,
            );
            self.draw_chat_overlay(metrics, batches.widget, batches.font);
            self.draw_title_overlay(metrics, batches.font);
            self.draw_scoreboard_sidebar(metrics, batches.font);
            self.draw_player_list_overlay(metrics, batches.font);
        }

        // Interactive overlays remain available even when a mod hides the HUD. Otherwise a mod
        // could strand the player in a sign editor, death screen, or resource-pack prompt.
        self.draw_sign_editor(metrics, batches.font);
        self.draw_book_editor(metrics, batches.widget, batches.font);
        self.draw_resource_pack_notice(metrics, batches.overlay, batches.widget, batches.font);
        self.draw_death_screen(metrics, batches.overlay, batches.widget, batches.font);

        if self.state.settings.hud_visible() {
            let script_after = self.state.hud.script_hud_commands().clone();
            self.draw_script_hud(
                &script_after,
                metrics,
                batches.font,
                batches.widget,
                batches.icons,
                batches.inventory,
            );
        }

        if self.state.inventory.inventory_open() {
            self.draw_inventory_overlay(
                metrics,
                batches.overlay,
                batches.widget,
                batches.inventory,
                batches.generic54,
                batches.font,
                batches.block,
                batches.item,
                batches.creative,
            );
        }
    }
}
