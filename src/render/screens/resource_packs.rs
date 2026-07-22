use super::scroll_list::GuiScrollList;
use super::{draw_button, draw_pack_icon};
use crate::client::app::ResourcePackInfo;
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;
use crate::ui::button_ids as btn;

impl Renderer {
    pub(super) fn draw_resource_packs_screen(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = metrics.sw;
        let sh = metrics.sh;
        let gs = metrics.gs;
        let list_width = 200.0 * gs;
        let list_top = 32.0 * gs;
        let list_height = (sh - 83.0 * gs).max(36.0 * gs);
        let left_x = sw / 2.0 - 204.0 * gs;
        let right_x = sw / 2.0 + 4.0 * gs;

        self.draw_standard_screen(metrics, font_gui, "resourcePack.title", 16.0 * gs, metrics.font_sz);
        let text = self.state.settings.ui_text();

        let available_packs = self.state.server_list.available_resource_packs().clone();
        let available = GuiScrollList::new(
            left_x,
            list_top + 16.0 * gs,
            list_width,
            list_height - 16.0 * gs,
            36.0 * gs,
            available_packs.len(),
            self.state.server_list.available_resource_pack_scroll(),
        );
        self.state.server_list.set_available_resource_pack_scroll(available.first_row);
        draw_list_background(font_gui, left_x, list_top, list_width, list_height);
        draw_list_header(
            &mut self.font,
            metrics,
            font_gui,
            left_x,
            list_width,
            list_top,
            text.get("resourcePack.available.title"),
        );
        for index in available.visible_range() {
            let y = available.row_y(index);
            draw_pack_entry(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                btn::RESOURCE_PACK_BASE + index as u32,
                left_x + 2.0 * gs,
                y,
                list_width - 7.0 * gs,
                &available_packs[index],
                true,
                false,
                false,
            );
        }
        available.draw_scrollbar(font_gui, gs);
        available.draw_edge_fades(font_gui, gs);

        let selected_packs = self.state.server_list.selected_resource_packs().clone();
        let selected = GuiScrollList::new(
            right_x,
            list_top + 16.0 * gs,
            list_width,
            list_height - 16.0 * gs,
            36.0 * gs,
            selected_packs.len(),
            self.state.server_list.selected_resource_pack_scroll(),
        );
        self.state.server_list.set_selected_resource_pack_scroll(selected.first_row);
        draw_list_background(font_gui, right_x, list_top, list_width, list_height);
        draw_list_header(
            &mut self.font,
            metrics,
            font_gui,
            right_x,
            list_width,
            list_top,
            text.get("resourcePack.selected.title"),
        );
        for index in selected.visible_range() {
            let y = selected.row_y(index);
            draw_pack_entry(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                btn::RESOURCE_PACK_SELECTED_BASE + index as u32,
                right_x + 2.0 * gs,
                y,
                list_width - 7.0 * gs,
                &selected_packs[index],
                false,
                !selected_packs[index].is_default && index > 0,
                !selected_packs[index].is_default
                    && index + 1 < selected_packs.len()
                    && !selected_packs[index + 1].is_default,
            );
        }
        selected.draw_scrollbar(font_gui, gs);
        selected.draw_edge_fades(font_gui, gs);

        let button_y = sh - 48.0 * gs;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::RESOURCE_PACK_OPEN_FOLDER,
            [sw / 2.0 - 154.0 * gs, button_y, 150.0 * gs, metrics.btn_h],
            text.get("resourcePack.openFolder"),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [sw / 2.0 + 4.0 * gs, button_y, 150.0 * gs, metrics.btn_h],
            text.get("gui.done"),
        );
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0 - 77.0 * gs,
            sh - 26.0 * gs,
            text.get("resourcePack.folderInfo"),
            metrics.font_sz * 0.72,
            [0.5, 0.5, 0.5, 1.0],
        );
    }
}

fn draw_list_background(gui: &mut GuiVertexBuilder, x: f32, y: f32, width: f32, height: f32) {
    gui.fill_rect(x, y, width, height, [0.0, 0.0, 0.0, 0.58]);
    gui.fill_rect(x, y, width, 1.0, [0.5, 0.5, 0.5, 0.5]);
    gui.fill_rect(x, y + height - 1.0, width, 1.0, [0.5, 0.5, 0.5, 0.5]);
}

fn draw_list_header(
    font: &mut crate::ui::font::FontRenderer,
    metrics: &MenuMetrics,
    font_gui: &mut GuiVertexBuilder,
    x: f32,
    width: f32,
    y: f32,
    label: &str,
) {
    font_gui.draw_text_centered(
        font,
        x + width / 2.0,
        y + 3.0 * metrics.gs,
        label,
        metrics.font_sz,
        [1.0, 1.0, 1.0, 1.0],
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_pack_entry(
    font: &mut crate::ui::font::FontRenderer,
    metrics: &MenuMetrics,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    id: u32,
    x: f32,
    y: f32,
    width: f32,
    pack: &ResourcePackInfo,
    available: bool,
    can_move_up: bool,
    can_move_down: bool,
) {
    let gs = metrics.gs;
    let row_height = 36.0 * gs;
    let hovered = metrics.mouse_pos[0] >= x
        && metrics.mouse_pos[0] <= x + 32.0 * gs
        && metrics.mouse_pos[1] >= y
        && metrics.mouse_pos[1] <= y + row_height;
    let compatible = pack.pack_format == 0 || pack.pack_format == 1;
    if !compatible {
        font_gui.fill_rect(
            x - gs,
            y - gs,
            width + 2.0 * gs,
            34.0 * gs,
            [0.47, 0.0, 0.0, 1.0],
        );
    }
    if !pack.is_default {
        if available {
            widget_gui.register_button(id, x, y, 32.0 * gs, 32.0 * gs);
        } else {
            widget_gui.register_button(id, x, y, 16.0 * gs, 32.0 * gs);
            let index = id - btn::RESOURCE_PACK_SELECTED_BASE;
            if can_move_up {
                widget_gui.register_button(
                    btn::RESOURCE_PACK_SELECTED_UP_BASE + index,
                    x + 16.0 * gs,
                    y,
                    16.0 * gs,
                    16.0 * gs,
                );
            }
            if can_move_down {
                widget_gui.register_button(
                    btn::RESOURCE_PACK_SELECTED_DOWN_BASE + index,
                    x + 16.0 * gs,
                    y + 16.0 * gs,
                    16.0 * gs,
                    16.0 * gs,
                );
            }
        }
    }

    font_gui.fill_rect(x, y, 32.0 * gs, 32.0 * gs, [0.08, 0.08, 0.08, 1.0]);
    if let Some(icon) = &pack.icon {
        draw_pack_icon(font_gui, x, y, 32.0 * gs, icon);
    }
    if hovered {
        font_gui.fill_rect(x, y, 32.0 * gs, 32.0 * gs, [0.0, 0.0, 0.0, 0.55]);
        if available {
            draw_pack_control(font_gui, font, x + 16.0 * gs, y + 10.0 * gs, ">", gs);
        } else if !pack.is_default {
            draw_pack_control(font_gui, font, x + 8.0 * gs, y + 10.0 * gs, "<", gs);
            if can_move_up {
                draw_pack_control(font_gui, font, x + 24.0 * gs, y + 2.0 * gs, "^", gs);
            }
            if can_move_down {
                draw_pack_control(font_gui, font, x + 24.0 * gs, y + 18.0 * gs, "v", gs);
            }
        }
    }

    let text_x = x + 34.0 * gs;
    let text_width = 157.0 * gs;
    let name = fit_text(font, &pack.name, metrics.font_sz, text_width);
    font_gui.draw_text(
        font,
        text_x,
        y + gs,
        &name,
        metrics.font_sz,
        [1.0, 1.0, 1.0, 1.0],
    );
    let description = if compatible {
        pack.description.as_str()
    } else {
        "Incompatible with Minecraft 1.8.9"
    };
    for (line, description) in wrap_text(
        font,
        description,
        metrics.font_sz * 0.72,
        text_width,
    )
    .into_iter()
    .take(2)
    .enumerate()
    {
        font_gui.draw_text(
            font,
            text_x,
            y + (13 + 10 * line) as f32 * gs,
            &description,
            metrics.font_sz * 0.72,
            [0.5, 0.5, 0.5, 1.0],
        );
    }
}

fn draw_pack_control(
    font_gui: &mut GuiVertexBuilder,
    font: &mut crate::ui::font::FontRenderer,
    x: f32,
    y: f32,
    symbol: &str,
    gs: f32,
) {
    font_gui.draw_text_centered(
        font,
        x,
        y,
        symbol,
        12.0 * gs,
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn wrap_text(
    font: &mut crate::ui::font::FontRenderer,
    text: &str,
    size: f32,
    max_width: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines().filter(|line| !line.is_empty()) {
        let mut line = String::new();
        for ch in paragraph.chars() {
            let mut candidate = line.clone();
            candidate.push(ch);
            if !line.is_empty() && font.text_width(&candidate, size) > max_width {
                lines.push(line);
                line = String::new();
            }
            line.push(ch);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

fn fit_text(
    font: &mut crate::ui::font::FontRenderer,
    text: &str,
    size: f32,
    max_width: f32,
) -> String {
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
