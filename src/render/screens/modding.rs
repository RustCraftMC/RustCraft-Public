//! Lua mod inventory, capability inspection, and runtime controls.

use super::scroll_list::GuiScrollList;
use super::{draw_button, draw_button_enabled, draw_title};
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::{ModManagerRow, Renderer};
use crate::ui::button_ids as btn;

impl Renderer {
    pub(super) fn draw_modding_screen(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = metrics.sw;
        let sh = metrics.sh;
        let gs = metrics.gs;
        let text = self.state.ui_text.clone();
        let list_width = (420.0 * gs).min((sw - 24.0 * gs).max(200.0 * gs));
        let list_x = (sw - list_width) / 2.0;
        let list_y = 36.0 * gs;
        let detail_y = sh - 121.0 * gs;
        let list_height = (detail_y - list_y - 4.0 * gs).max(38.0 * gs);
        let rows = self.state.modding_rows.clone();
        let list = GuiScrollList::new(
            list_x,
            list_y,
            list_width,
            list_height,
            38.0 * gs,
            rows.len(),
            self.state.modding_scroll,
        );
        self.state.modding_scroll = list.first_row;

        draw_title(
            self,
            font_gui,
            sw / 2.0,
            16.0 * gs,
            text.get("rustcraft.modding.title"),
            metrics.font_sz,
            gs,
        );
        list.draw_background(font_gui);
        if rows.is_empty() {
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                list_y + 16.0 * gs,
                text.get("rustcraft.modding.empty"),
                metrics.font_sz,
                [0.72, 0.72, 0.72, 1.0],
            );
        } else {
            for index in list.visible_range() {
                draw_mod_row(
                    self,
                    metrics,
                    widget_gui,
                    font_gui,
                    list_x + 3.0 * gs,
                    list.row_y(index) + 2.0 * gs,
                    list_width - 12.0 * gs,
                    &rows[index],
                    index,
                    index == self.state.modding_selected,
                );
            }
        }
        list.draw_scrollbar(font_gui, gs);
        list.draw_edge_fades(font_gui, gs);

        let selected = rows.get(self.state.modding_selected);
        let connection_locked = selected
            .is_some_and(|row| self.state.modding_connection_active && row.protocol_translator);
        draw_mod_details(
            self,
            metrics,
            font_gui,
            list_x,
            detail_y,
            list_width,
            selected,
            connection_locked,
        );

        let status = if connection_locked {
            text.get("rustcraft.modding.disconnectTranslator")
                .to_string()
        } else {
            self.state.modding_status.clone()
        };
        let status = fit_text(&self.font, &status, metrics.font_sz * 0.72, list_width);
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            sh - 66.0 * gs,
            &status,
            metrics.font_sz * 0.72,
            if connection_locked {
                [1.0, 0.7, 0.3, 1.0]
            } else {
                [0.75, 0.75, 0.75, 1.0]
            },
        );
        let action_y = sh - 52.0 * gs;
        let column_gap = 4.0 * gs;
        let column_width = (list_width - column_gap * 3.0) / 4.0;
        let has_selection = selected.is_some();
        let is_enabled = selected.is_some_and(|row| row.enabled);
        draw_button_enabled(
            self,
            metrics,
            widget_gui,
            font_gui,
            btn::MODDING_TOGGLE,
            [list_x, action_y, column_width, metrics.btn_h],
            if is_enabled {
                text.get("rustcraft.modding.disable")
            } else {
                text.get("rustcraft.modding.enable")
            },
            has_selection && !connection_locked,
        );
        draw_button_enabled(
            self,
            metrics,
            widget_gui,
            font_gui,
            btn::MODDING_RELOAD,
            [
                list_x + column_width + column_gap,
                action_y,
                column_width,
                metrics.btn_h,
            ],
            text.get("rustcraft.modding.reload"),
            is_enabled && !connection_locked,
        );
        draw_button_enabled(
            self,
            metrics,
            widget_gui,
            font_gui,
            btn::MODDING_CONFIGURE,
            [
                list_x + (column_width + column_gap) * 2.0,
                action_y,
                column_width,
                metrics.btn_h,
            ],
            text.get("rustcraft.modding.configure"),
            selected.is_some_and(|row| row.config_entries > 0),
        );
        draw_button(
            self,
            metrics,
            widget_gui,
            font_gui,
            btn::MODDING_RELOAD_ALL,
            [
                list_x + (column_width + column_gap) * 3.0,
                action_y,
                column_width,
                metrics.btn_h,
            ],
            text.get("rustcraft.modding.reloadAll"),
        );
        draw_button(
            self,
            metrics,
            widget_gui,
            font_gui,
            btn::MODDING_BACK,
            [list_x, sh - 28.0 * gs, list_width, metrics.btn_h],
            text.get("gui.back"),
        );
    }
}

fn draw_mod_row(
    renderer: &mut Renderer,
    metrics: &MenuMetrics,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    x: f32,
    y: f32,
    width: f32,
    row: &ModManagerRow,
    index: usize,
    selected: bool,
) {
    let gs = metrics.gs;
    let status = if row.enabled {
        renderer
            .state
            .ui_text
            .get("rustcraft.modding.enabled")
            .to_string()
    } else {
        renderer
            .state
            .ui_text
            .get("rustcraft.modding.disabled")
            .to_string()
    };
    let status_color = if row.enabled {
        [0.4, 1.0, 0.4, 1.0]
    } else {
        [1.0, 0.45, 0.45, 1.0]
    };
    let hovered = metrics.mouse_pos[0] >= x
        && metrics.mouse_pos[0] <= x + width
        && metrics.mouse_pos[1] >= y
        && metrics.mouse_pos[1] <= y + 34.0 * gs;
    let background = if selected {
        [0.08, 0.2, 0.24, 0.96]
    } else if hovered {
        [0.14, 0.14, 0.14, 0.94]
    } else {
        [0.07, 0.07, 0.07, 0.88]
    };
    font_gui.fill_rect(x, y, width, 34.0 * gs, background);
    if selected {
        font_gui.fill_rect(x, y, 2.0 * gs, 34.0 * gs, [0.25, 0.82, 0.85, 1.0]);
    }
    widget_gui.register_button(btn::MODDING_ROW_BASE + index as u32, x, y, width, 34.0 * gs);
    let title = format!("{}  v{}", row.name, row.version);
    let title = fit_text(&renderer.font, &title, metrics.font_sz, width - 82.0 * gs);
    font_gui.draw_text(
        &mut renderer.font,
        x + 4.0 * gs,
        y + 3.0 * gs,
        &title,
        metrics.font_sz,
        [1.0, 1.0, 1.0, 1.0],
    );
    let id = fit_text(
        &renderer.font,
        &row.id,
        metrics.font_sz * 0.72,
        width - 8.0 * gs,
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + width - 58.0 * gs,
        y + 3.0 * gs,
        &status,
        metrics.font_sz * 0.72,
        status_color,
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + 4.0 * gs,
        y + 16.0 * gs,
        &id,
        metrics.font_sz * 0.72,
        [0.72, 0.72, 0.72, 1.0],
    );
    let text = &renderer.state.ui_text;
    let permissions = format!(
        "{}: {}{}",
        text.get("rustcraft.modding.permissions"),
        row.granted_permissions.len(),
        if row.denied_permissions.is_empty() {
            String::new()
        } else {
            format!(
                ", {} {}",
                row.denied_permissions.len(),
                text.get("rustcraft.modding.denied")
            )
        }
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + 4.0 * gs,
        y + 26.0 * gs,
        &permissions,
        metrics.font_sz * 0.62,
        if row.denied_permissions.is_empty() {
            [0.55, 0.75, 0.55, 1.0]
        } else {
            [1.0, 0.72, 0.32, 1.0]
        },
    );
}

fn draw_mod_details(
    renderer: &mut Renderer,
    metrics: &MenuMetrics,
    font_gui: &mut GuiVertexBuilder,
    x: f32,
    y: f32,
    width: f32,
    selected: Option<&ModManagerRow>,
    connection_locked: bool,
) {
    let gs = metrics.gs;
    font_gui.fill_rect(x, y, width, 47.0 * gs, [0.035, 0.035, 0.035, 0.92]);
    font_gui.fill_rect(x, y, width, 1.0 * gs, [0.25, 0.82, 0.85, 0.75]);
    let Some(row) = selected else {
        font_gui.draw_text_centered(
            &mut renderer.font,
            x + width / 2.0,
            y + 19.0 * gs,
            renderer
                .state
                .ui_text
                .get("rustcraft.modding.inspectPrompt"),
            metrics.font_sz * 0.78,
            [0.65, 0.65, 0.65, 1.0],
        );
        return;
    };

    let header = format!(
        "{}  [{}]{}  {} {}",
        row.name,
        if row.enabled {
            renderer.state.ui_text.get("rustcraft.modding.running")
        } else {
            renderer.state.ui_text.get("rustcraft.modding.stopped")
        },
        if row.protocol_translator {
            renderer
                .state
                .ui_text
                .get("rustcraft.modding.protocolTranslator")
        } else {
            ""
        },
        row.config_entries,
        renderer.state.ui_text.get("rustcraft.modding.settings"),
    );
    let header = fit_text(
        &renderer.font,
        &header,
        metrics.font_sz * 0.82,
        width - 8.0 * gs,
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + 4.0 * gs,
        y + 5.0 * gs,
        &header,
        metrics.font_sz * 0.82,
        if connection_locked {
            [1.0, 0.72, 0.3, 1.0]
        } else {
            [0.82, 0.96, 0.98, 1.0]
        },
    );

    let granted = permission_line(
        renderer.state.ui_text.get("rustcraft.modding.granted"),
        renderer.state.ui_text.get("gui.none"),
        &row.granted_permissions,
    );
    let granted = fit_text(
        &renderer.font,
        &granted,
        metrics.font_sz * 0.66,
        width - 8.0 * gs,
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + 4.0 * gs,
        y + 18.0 * gs,
        &granted,
        metrics.font_sz * 0.66,
        [0.5, 0.9, 0.56, 1.0],
    );

    let denied = permission_line(
        renderer.state.ui_text.get("rustcraft.modding.denied"),
        renderer.state.ui_text.get("gui.none"),
        &row.denied_permissions,
    );
    let denied = fit_text(
        &renderer.font,
        &denied,
        metrics.font_sz * 0.66,
        width - 8.0 * gs,
    );
    font_gui.draw_text(
        &mut renderer.font,
        x + 4.0 * gs,
        y + 29.0 * gs,
        &denied,
        metrics.font_sz * 0.66,
        if row.denied_permissions.is_empty() {
            [0.52, 0.52, 0.52, 1.0]
        } else {
            [1.0, 0.62, 0.35, 1.0]
        },
    );

    font_gui.draw_text(
        &mut renderer.font,
        x + 4.0 * gs,
        y + 39.0 * gs,
        renderer.state.ui_text.get("rustcraft.modding.shortcuts"),
        metrics.font_sz * 0.56,
        [0.48, 0.65, 0.68, 1.0],
    );
}

fn permission_line(label: &str, none_label: &str, permissions: &[String]) -> String {
    if permissions.is_empty() {
        format!("{label}: {none_label}")
    } else {
        format!("{label}: {}", permissions.join(", "))
    }
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
