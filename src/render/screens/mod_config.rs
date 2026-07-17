//! Declarative Lua mod configuration rendered with native Minecraft-style controls.

use super::scroll_list::GuiScrollList;
use super::{draw_button, draw_button_enabled, draw_title};
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::{ModConfigRow, Renderer};
use crate::ui::button_ids as btn;

impl Renderer {
    pub(super) fn draw_mod_config_screen(
        &mut self,
        metrics: &MenuMetrics,
        overlay_gui: &mut GuiVertexBuilder,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = metrics.sw;
        let sh = metrics.sh;
        let gs = metrics.gs;
        let width = (440.0 * gs).min((sw - 24.0 * gs).max(220.0 * gs));
        let x = (sw - width) / 2.0;
        let list_y = 39.0 * gs;
        let list_height = (sh - 91.0 * gs).max(48.0 * gs);
        let rows = self.state.mod_config_rows.clone();
        let list = GuiScrollList::new(
            x,
            list_y,
            width,
            list_height,
            48.0 * gs,
            rows.len(),
            self.state.mod_config_scroll,
        );
        self.state.mod_config_scroll = list.first_row;

        let title = fit_text(
            &self.font,
            self.state
                .mod_config_title
                .as_deref()
                .unwrap_or("Mod Configuration"),
            metrics.font_sz,
            width,
        );
        draw_title(
            self,
            font_gui,
            sw / 2.0,
            14.0 * gs,
            &title,
            metrics.font_sz,
            gs,
        );
        let subtitle = fit_text(
            &self.font,
            "Settings are validated and saved in this mod's private data directory",
            metrics.font_sz * 0.62,
            width,
        );
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            27.0 * gs,
            &subtitle,
            metrics.font_sz * 0.62,
            [0.52, 0.7, 0.72, 1.0],
        );

        list.draw_background(overlay_gui);
        if rows.is_empty() {
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                list_y + 18.0 * gs,
                "This mod does not expose configurable settings",
                metrics.font_sz * 0.82,
                [0.68, 0.68, 0.68, 1.0],
            );
        } else {
            let selected = self.state.mod_config_selected;
            let locked = self.state.mod_config_locked;
            for index in list.visible_range() {
                draw_config_row(
                    self,
                    metrics,
                    overlay_gui,
                    widget_gui,
                    font_gui,
                    x + 3.0 * gs,
                    list.row_y(index) + 2.0 * gs,
                    width - 12.0 * gs,
                    &rows[index],
                    index,
                    index == selected,
                    locked,
                );
            }
        }
        list.draw_scrollbar(font_gui, gs);
        list.draw_edge_fades(overlay_gui, gs);

        let status = if self.state.mod_config_locked {
            "Disconnect before editing a protocol translator's configuration".to_string()
        } else if self.state.mod_config_status.is_empty() {
            "Left/Right: change   R: reset   Esc: back".to_string()
        } else {
            self.state.mod_config_status.clone()
        };
        let status = fit_text(&self.font, &status, metrics.font_sz * 0.68, width);
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            sh - 42.0 * gs,
            &status,
            metrics.font_sz * 0.68,
            if self.state.mod_config_locked {
                [1.0, 0.68, 0.3, 1.0]
            } else {
                [0.7, 0.7, 0.7, 1.0]
            },
        );

        let button_y = sh - 28.0 * gs;
        let half = (width - 4.0 * gs) / 2.0;
        draw_button_enabled(
            self,
            metrics,
            widget_gui,
            font_gui,
            btn::MOD_CONFIG_RESET_ALL,
            [x, button_y, half, metrics.btn_h],
            "Reset all",
            !self.state.mod_config_locked && rows.iter().any(|row| !row.is_default),
        );
        draw_button(
            self,
            metrics,
            widget_gui,
            font_gui,
            btn::MOD_CONFIG_BACK,
            [x + half + 4.0 * gs, button_y, half, metrics.btn_h],
            "Back",
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_config_row(
    renderer: &mut Renderer,
    metrics: &MenuMetrics,
    overlay_gui: &mut GuiVertexBuilder,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    x: f32,
    y: f32,
    width: f32,
    row: &ModConfigRow,
    index: usize,
    selected: bool,
    locked: bool,
) {
    let gs = metrics.gs;
    let hovered = metrics.mouse_pos[0] >= x
        && metrics.mouse_pos[0] <= x + width
        && metrics.mouse_pos[1] >= y
        && metrics.mouse_pos[1] <= y + 44.0 * gs;
    overlay_gui.fill_rect(
        x,
        y,
        width,
        44.0 * gs,
        if selected {
            [0.08, 0.2, 0.24, 0.96]
        } else if hovered {
            [0.14, 0.14, 0.14, 0.94]
        } else {
            [0.07, 0.07, 0.07, 0.88]
        },
    );
    if selected {
        overlay_gui.fill_rect(x, y, 2.0 * gs, 44.0 * gs, [0.25, 0.82, 0.85, 1.0]);
    }
    widget_gui.register_button(
        btn::MOD_CONFIG_ROW_BASE + index as u32,
        x,
        y,
        width,
        44.0 * gs,
    );

    let controls_width = (172.0 * gs).min((width - 82.0 * gs).max(126.0 * gs));
    let text_width = (width - controls_width - 12.0 * gs).max(70.0 * gs);
    let label = fit_text(
        &renderer.font,
        &row.label,
        metrics.font_sz * 0.9,
        text_width,
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + 6.0 * gs,
        y + 5.0 * gs,
        &label,
        metrics.font_sz * 0.9,
        [0.95, 0.95, 0.95, 1.0],
    );
    let description = if row.description.is_empty() {
        row.key.as_str()
    } else {
        row.description.as_str()
    };
    let description = fit_text(
        &renderer.font,
        description,
        metrics.font_sz * 0.62,
        text_width,
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + 6.0 * gs,
        y + 19.0 * gs,
        &description,
        metrics.font_sz * 0.62,
        [0.62, 0.62, 0.62, 1.0],
    );
    let key = fit_text(
        &renderer.font,
        &format!("Key: {}", row.key),
        metrics.font_sz * 0.54,
        text_width,
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + 6.0 * gs,
        y + 32.0 * gs,
        &key,
        metrics.font_sz * 0.54,
        [0.42, 0.58, 0.6, 1.0],
    );

    let control_x = x + width - controls_width - 4.0 * gs;
    let control_y = y + 6.0 * gs;
    let arrow_width = 20.0 * gs;
    let reset_width = 38.0 * gs;
    let value_width = controls_width - arrow_width * 2.0 - reset_width - 8.0 * gs;
    draw_button_enabled(
        renderer,
        metrics,
        widget_gui,
        font_gui,
        btn::MOD_CONFIG_PREVIOUS_BASE + index as u32,
        [control_x, control_y, arrow_width, metrics.btn_h],
        "<",
        !locked && row.can_previous,
    );
    let value_x = control_x + arrow_width + 2.0 * gs;
    widget_gui.draw_button_rect_state(value_x, control_y, value_width, metrics.btn_h, 2);
    let value = fit_text(
        &renderer.font,
        &row.value,
        metrics.font_sz * 0.75,
        value_width - 6.0 * gs,
    );
    font_gui.draw_text_centered(
        &mut renderer.font,
        value_x + value_width / 2.0,
        control_y + (metrics.btn_h - metrics.font_sz * 0.75) / 2.0,
        &value,
        metrics.font_sz * 0.75,
        if row.is_default {
            [0.78, 0.92, 0.94, 1.0]
        } else {
            [1.0, 0.86, 0.36, 1.0]
        },
    );
    let next_x = value_x + value_width + 2.0 * gs;
    draw_button_enabled(
        renderer,
        metrics,
        widget_gui,
        font_gui,
        btn::MOD_CONFIG_NEXT_BASE + index as u32,
        [next_x, control_y, arrow_width, metrics.btn_h],
        ">",
        !locked && row.can_next,
    );
    draw_button_enabled(
        renderer,
        metrics,
        widget_gui,
        font_gui,
        btn::MOD_CONFIG_RESET_BASE + index as u32,
        [
            next_x + arrow_width + 4.0 * gs,
            control_y,
            reset_width,
            metrics.btn_h,
        ],
        "Reset",
        !locked && !row.is_default,
    );
}

fn fit_text(font: &crate::ui::font::FontRenderer, text: &str, size: f32, max_width: f32) -> String {
    if font.text_width(text, size) <= max_width {
        return text.to_string();
    }
    let suffix = "...";
    let suffix_width = font.text_width(suffix, size);
    let mut output = String::new();
    for ch in text.chars() {
        let mut candidate = output.clone();
        candidate.push(ch);
        if font.text_width(&candidate, size) + suffix_width > max_width {
            break;
        }
        output.push(ch);
    }
    output.push_str(suffix);
    output
}
