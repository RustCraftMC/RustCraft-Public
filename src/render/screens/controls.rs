use super::scroll_list::GuiScrollList;
use super::{draw_button, draw_button_enabled, draw_slider};
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::{ControlBindingRow, Renderer};
use crate::ui::button_ids as btn;

enum ControlListItem {
    Category(String),
    Binding(ControlBindingRow),
}

impl Renderer {
    pub(super) fn draw_controls_menu(
        &mut self,
        metrics: &MenuMetrics,
        background_gui: &mut GuiVertexBuilder,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let _ = background_gui;
        let sw = metrics.sw;
        let sh = metrics.sh;
        let gs = metrics.gs;

        // Build the control list and sync scroll state before borrowing
        // `ui_text` so the mutable `set_controls_list_scroll` call does not
        // conflict with the immutable text borrow held for the rest of the
        // frame.
        let mut items = Vec::new();
        let mut category = String::new();
        for row in self.state.settings.control_bindings().iter().cloned() {
            if row.category != category {
                category = row.category.clone();
                items.push(ControlListItem::Category(category.clone()));
            }
            items.push(ControlListItem::Binding(row));
        }

        let list = GuiScrollList::new(
            0.0,
            70.0 * gs,
            sw,
            (sh - 80.0 * gs).max(20.0 * gs),
            20.0 * gs,
            items.len(),
            self.state.settings.controls_list_scroll(),
        );
        self.state.settings.set_controls_list_scroll(list.first_row);

        self.draw_standard_screen(metrics, font_gui, "controls.title", 8.0 * gs, 12.0 * gs);
        let text = self.state.settings.ui_text();
        let device_label = if self.state.settings.controls_gamepad() {
            text.get("rustcraft.controls.controller")
        } else {
            text.get("rustcraft.controls.keyboardMouse")
        };
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::CONTROL_DEVICE_TOGGLE,
            [sw / 2.0 - 75.0 * gs, 23.0 * gs, 150.0 * gs, 18.0 * gs],
            device_label,
        );

        // Vanilla exposes mouse sensitivity in the Options screen.  Keep the
        // setting alongside bindings here, and show controller-specific
        // equivalents when the controller binding page is active.
        if self.state.settings.controls_gamepad() {
            draw_slider(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                btn::GAMEPAD_LOOK_SENSITIVITY,
                [sw / 2.0 - 155.0 * gs, 45.0 * gs, 150.0 * gs, 18.0 * gs],
                &format!(
                    "{}: {}%",
                    text.get("rustcraft.controls.lookSensitivity"),
                    (self.state.settings.gamepad_look_sensitivity() * 200.0).round()
                ),
                self.state.settings.gamepad_look_sensitivity(),
            );
            draw_slider(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                btn::GAMEPAD_CURSOR_SPEED,
                [sw / 2.0 + 5.0 * gs, 45.0 * gs, 150.0 * gs, 18.0 * gs],
                &format!(
                    "{}: {}%",
                    text.get("rustcraft.controls.cursorSpeed"),
                    (self.state.settings.gamepad_cursor_speed() * 200.0).round()
                ),
                self.state.settings.gamepad_cursor_speed(),
            );
        } else {
            draw_slider(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                btn::MOUSE_SENSITIVITY,
                [sw / 2.0 - 155.0 * gs, 45.0 * gs, 150.0 * gs, 18.0 * gs],
                &format!(
                    "{}: {}%",
                    text.get("options.sensitivity"),
                    (self.state.settings.mouse_sensitivity() * 200.0).round()
                ),
                self.state.settings.mouse_sensitivity(),
            );
            draw_button(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                btn::INVERT_MOUSE_TOGGLE,
                [sw / 2.0 + 5.0 * gs, 45.0 * gs, 150.0 * gs, 18.0 * gs],
                if self.state.settings.invert_mouse() {
                    text.get("options.invertMouse.on")
                } else {
                    text.get("options.invertMouse.off")
                },
            );
        }

        list.draw_background(font_gui);

        let binding_x = sw / 2.0 - 25.0 * gs;
        let reset_x = binding_x + 80.0 * gs;
        for index in list.visible_range() {
            let y = list.row_y(index);
            match &items[index] {
                ControlListItem::Category(label) => {
                    font_gui.draw_text_centered(
                        &mut self.font,
                        sw / 2.0,
                        y + 7.0 * gs,
                        label,
                        metrics.font_sz * 0.8,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
                ControlListItem::Binding(row) => {
                    let label = fit_control_text(
                        &self.font,
                        &row.label,
                        metrics.font_sz,
                        (binding_x - 12.0 * gs).max(40.0 * gs),
                    );
                    let label_width = self.font.text_width(&label, metrics.font_sz);
                    font_gui.draw_text(
                        &mut self.font,
                        binding_x - 5.0 * gs - label_width,
                        y + 6.0 * gs,
                        &label,
                        metrics.font_sz,
                        [1.0, 1.0, 1.0, 1.0],
                    );

                    let Some(action_index) = row.action.bindable_index() else {
                        continue;
                    };
                    let binding_label = if row.listening {
                        format!("> {} <", row.binding)
                    } else {
                        row.binding.clone()
                    };
                    draw_button(
                        &mut self.font,
                        metrics,
                        widget_gui,
                        font_gui,
                        btn::CONTROL_BIND_BASE + action_index as u32,
                        [binding_x, y, 75.0 * gs, 20.0 * gs],
                        &binding_label,
                    );
                    if row.conflict {
                        font_gui.fill_rect(
                            binding_x,
                            y + 18.0 * gs,
                            75.0 * gs,
                            2.0 * gs,
                            [0.9, 0.18, 0.18, 1.0],
                        );
                    }
                    draw_button_enabled(
                        &mut self.font,
                        metrics,
                        widget_gui,
                        font_gui,
                        btn::CONTROL_RESET_BASE + action_index as u32,
                        [reset_x, y, 50.0 * gs, 20.0 * gs],
                        text.get("controls.reset"),
                        !row.is_default,
                    );
                }
            }
        }

        list.draw_scrollbar(font_gui, gs);
        list.draw_edge_fades(font_gui, gs);

        let bottom_y = sh - 29.0 * gs;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [sw / 2.0 - 155.0 * gs, bottom_y, 150.0 * gs, metrics.btn_h],
            text.get("gui.done"),
        );
        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::RESET_CONTROLS,
            [sw / 2.0 + 5.0 * gs, bottom_y, 150.0 * gs, metrics.btn_h],
            text.get("controls.resetAll"),
            self.state
                .settings
                .control_bindings()
                .iter()
                .any(|row| !row.is_default),
        );
    }
}

fn fit_control_text(
    font: &crate::ui::font::FontRenderer,
    text: &str,
    size: f32,
    max: f32,
) -> String {
    if font.text_width(text, size) <= max {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let mut candidate = out.clone();
        candidate.push(ch);
        candidate.push_str("...");
        if font.text_width(&candidate, size) > max {
            break;
        }
        out.push(ch);
    }
    out.push_str("...");
    out
}
