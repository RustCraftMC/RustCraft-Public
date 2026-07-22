use super::scroll_list::GuiScrollList;
use super::{draw_button, draw_title};
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;
use crate::ui::button_ids as btn;

impl Renderer {
    pub(super) fn draw_shader_packs_screen(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = metrics.sw;
        let sh = metrics.sh;
        let gs = metrics.gs;
        draw_title(
            &mut self.font,
            font_gui,
            sw / 2.0,
            16.0 * gs,
            "Shader Packs",
            metrics.font_sz,
            gs,
        );

        let packs = self.state.server_list.shader_packs().clone();
        let list = GuiScrollList::new(
            sw / 2.0 - 155.0 * gs,
            38.0 * gs,
            310.0 * gs,
            (sh - 112.0 * gs).max(40.0 * gs),
            34.0 * gs,
            packs.len(),
            self.state.server_list.shader_pack_scroll(),
        );
        self.state.server_list.set_shader_pack_scroll(list.first_row);
        font_gui.fill_rect(
            list.x,
            list.y,
            list.width,
            list.height,
            [0.0, 0.0, 0.0, 0.58],
        );

        for index in list.visible_range() {
            let pack = &packs[index];
            let y = list.row_y(index);
            let selected = self.state.server_list.selected_shader_pack().as_deref() == Some(&pack.source_name);
            let color = if selected {
                [0.18, 0.38, 0.18, 0.9]
            } else if !pack.compatible {
                [0.38, 0.08, 0.08, 0.9]
            } else {
                [0.08, 0.08, 0.08, 0.85]
            };
            font_gui.fill_rect(list.x + gs, y, list.width - 7.0 * gs, 31.0 * gs, color);
            if pack.compatible {
                widget_gui.register_button(
                    btn::SHADER_PACK_BASE + index as u32,
                    list.x + gs,
                    y,
                    list.width - 7.0 * gs,
                    31.0 * gs,
                );
            }
            font_gui.draw_text(
                &mut self.font,
                list.x + 6.0 * gs,
                y + 3.0 * gs,
                &pack.name,
                metrics.font_sz,
                [1.0, 1.0, 1.0, 1.0],
            );
            let detail = pack.error.as_deref().unwrap_or(&pack.description);
            font_gui.draw_text(
                &mut self.font,
                list.x + 6.0 * gs,
                y + 17.0 * gs,
                detail,
                metrics.font_sz * 0.65,
                if pack.compatible {
                    [0.65, 0.65, 0.65, 1.0]
                } else {
                    [1.0, 0.45, 0.45, 1.0]
                },
            );
        }
        list.draw_scrollbar(font_gui, gs);
        list.draw_edge_fades(font_gui, gs);

        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            sh - 69.0 * gs,
            &format!(
                "{} | Vulkan RT: {} | FSR 3: {}",
                self.state.server_list.shader_pack_status(),
                if self.state.server_list.ray_tracing_available() {
                    "Yes"
                } else {
                    "No"
                },
                if self.state.server_list.fsr3_available() {
                    "Yes"
                } else {
                    "SDK not linked"
                }
            ),
            metrics.font_sz * 0.65,
            [0.7, 0.7, 0.7, 1.0],
        );

        let y = sh - 46.0 * gs;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::SHADER_PACK_OPEN_FOLDER,
            [sw / 2.0 - 155.0 * gs, y, 100.0 * gs, metrics.btn_h],
            "Open Folder",
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::SHADER_PACK_OFF,
            [sw / 2.0 - 50.0 * gs, y, 100.0 * gs, metrics.btn_h],
            "Off",
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [sw / 2.0 + 55.0 * gs, y, 100.0 * gs, metrics.btn_h],
            "Done",
        );
    }
}
