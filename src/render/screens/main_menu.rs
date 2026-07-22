use super::{draw_button, draw_button_enabled};
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;
use crate::ui::button_ids as btn;

impl Renderer {
    pub(super) fn draw_main_menu(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let text = self.state.settings.ui_text();
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;

        let title_y = sh / 4.0 - 30.0 * gs;
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            title_y,
            text.get("rustcraft.brand"),
            24.0 * gs,
            [1.0, 1.0, 1.0, 1.0],
            gs,
        );
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            title_y + 28.0 * gs,
            text.get("rustcraft.version"),
            12.0 * gs,
            [0.75, 0.75, 0.75, 1.0],
            gs,
        );

        let button_y = sh / 4.0 + 48.0 * gs;
        // Vanilla keeps the unavailable single-player entry visible. RustCraft
        // currently implements multiplayer only, so expose it as disabled rather
        // than silently omitting the standard menu row.
        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::SINGLEPLAYER,
            [metrics.btn_x, button_y, metrics.btn_w, metrics.btn_h],
            text.get("menu.singleplayer"),
            false,
        );
        let multiplayer_y = button_y + metrics.btn_h + metrics.btn_gap;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::MULTIPLAYER,
            [metrics.btn_x, multiplayer_y, metrics.btn_w, metrics.btn_h],
            text.get("menu.multiplayer"),
        );
        let alt_y = multiplayer_y + metrics.btn_h + metrics.btn_gap;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::ALT_MANAGER,
            [metrics.btn_x, alt_y, metrics.btn_w, metrics.btn_h],
            text.get("rustcraft.mainmenu.altManager"),
        );

        let half_width = (metrics.btn_w - metrics.btn_gap) / 2.0;
        let modding_y = alt_y + metrics.btn_h + metrics.btn_gap;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::MODDING,
            [metrics.btn_x, modding_y, metrics.btn_w, metrics.btn_h],
            text.get("rustcraft.mainmenu.modding"),
        );
        let bottom_y = modding_y + metrics.btn_h + metrics.btn_gap;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::OPTIONS,
            [metrics.btn_x, bottom_y, half_width, metrics.btn_h],
            text.get("menu.options"),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::QUIT,
            [
                metrics.btn_x + half_width + metrics.btn_gap,
                bottom_y,
                half_width,
                metrics.btn_h,
            ],
            text.get("menu.quit"),
        );

        font_gui.draw_text(
            &mut self.font,
            2.0 * gs,
            sh - 11.0 * gs,
            text.get("rustcraft.version"),
            8.0 * gs,
            [1.0, 1.0, 1.0, 0.8],
        );
    }
}
