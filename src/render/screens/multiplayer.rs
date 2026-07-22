use super::scroll_list::GuiScrollList;
use super::{draw_button, draw_button_enabled, draw_title};
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::{Renderer, ServerListRow};
use crate::ui::button_ids as btn;

impl Renderer {
    pub(super) fn draw_alt_manager_menu(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = metrics.sw;
        let sh = metrics.sh;
        let gs = metrics.gs;
        let text = self.state.settings.ui_text();
        draw_title(
            &mut self.font,
            font_gui,
            sw / 2.0,
            36.0 * gs,
            text.get("rustcraft.altmanager.title"),
            16.0 * gs,
            gs,
        );
        let account = self.state.account.account_name().clone();
        let status = self.state.account.account_status().clone();
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            82.0 * gs,
            &account,
            metrics.font_sz,
            [1.0, 1.0, 1.0, 1.0],
        );
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            100.0 * gs,
            &status,
            metrics.font_sz * 0.75,
            [0.7, 0.7, 0.7, 1.0],
        );
        let y = sh / 2.0 - 8.0 * gs;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::ALT_LOGIN,
            [metrics.btn_x, y, metrics.btn_w, metrics.btn_h],
            text.get("rustcraft.altmanager.addAccount"),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::ALT_LOGOUT,
            [metrics.btn_x, y + 24.0 * gs, metrics.btn_w, metrics.btn_h],
            text.get("rustcraft.altmanager.remove"),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::ALT_BACK,
            [metrics.btn_x, y + 48.0 * gs, metrics.btn_w, metrics.btn_h],
            text.get("gui.back"),
        );
    }
    pub(super) fn draw_multiplayer_menu(
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
        let font_size = metrics.font_sz;

        self.draw_standard_screen(metrics, font_gui, "selectServer.title", 20.0 * gs, 12.0 * gs);
        let text = self.state.settings.ui_text();

        let list = GuiScrollList::new(
            0.0,
            32.0 * gs,
            sw,
            (sh - 96.0 * gs).max(36.0 * gs),
            36.0 * gs,
            self.state.server_list.server_list().len(),
            self.state.server_list.server_list_scroll(),
        );
        self.state.server_list.set_server_list_scroll(list.first_row);
        list.draw_background(font_gui);

        if self.state.server_list.server_list().is_empty() {
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                list.y + 16.0 * gs,
                text.get("selectServer.empty"),
                font_size,
                [0.72, 0.72, 0.72, 1.0],
            );
        } else {
            let content_width = (306.0 * gs).min(sw - 24.0 * gs);
            let content_x = (sw - content_width) / 2.0;
            for index in list.visible_range() {
                let server = self.state.server_list.server_list_mut()[index].clone();
                let row_y = list.row_y(index);
                let selected = index == self.state.server_list.selected_server();
                let hovered = metrics.mouse_pos[0] >= content_x
                    && metrics.mouse_pos[0] <= content_x + content_width
                    && metrics.mouse_pos[1] >= row_y
                    && metrics.mouse_pos[1] <= row_y + 36.0 * gs;

                if selected {
                    font_gui.fill_rect(
                        content_x - gs,
                        row_y,
                        content_width + 2.0 * gs,
                        36.0 * gs,
                        [0.58, 0.58, 0.58, 1.0],
                    );
                    font_gui.fill_rect(
                        content_x,
                        row_y + gs,
                        content_width,
                        34.0 * gs,
                        [0.08, 0.08, 0.08, 0.96],
                    );
                } else if hovered {
                    font_gui.fill_rect(
                        content_x,
                        row_y,
                        content_width,
                        36.0 * gs,
                        [0.16, 0.16, 0.16, 0.92],
                    );
                }

                widget_gui.register_button(
                    btn::SERVER_ROW_BASE + index as u32,
                    content_x,
                    row_y,
                    content_width,
                    36.0 * gs,
                );

                let icon_size = 32.0 * gs;
                let icon_x = content_x;
                let icon_y = row_y + 2.0 * gs;

                // Server icon: draw favicon if available, otherwise gray placeholder
                if let Some(ref pixels) = server.favicon_pixels {
                    let img_w = 64u32;
                    let img_h = 64u32;
                    let pw = icon_size / img_w as f32;
                    let ph = icon_size / img_h as f32;
                    let expected = (img_w * img_h * 4) as usize;
                    if pixels.len() >= expected {
                        for py in 0..img_h {
                            for px in 0..img_w {
                                let idx = ((py * img_w + px) * 4) as usize;
                                let r = pixels[idx] as f32 / 255.0;
                                let g = pixels[idx + 1] as f32 / 255.0;
                                let b = pixels[idx + 2] as f32 / 255.0;
                                let a = pixels[idx + 3] as f32 / 255.0;
                                if a > 0.01 {
                                    font_gui.fill_rect(
                                        icon_x + px as f32 * pw,
                                        icon_y + py as f32 * ph,
                                        pw,
                                        ph,
                                        [r, g, b, a],
                                    );
                                }
                            }
                        }
                    }
                } else {
                    font_gui.fill_rect(
                        icon_x,
                        icon_y,
                        icon_size,
                        icon_size,
                        [0.11, 0.11, 0.11, 1.0],
                    );
                    font_gui.fill_rect(
                        icon_x + gs,
                        icon_y + 3.0 * gs,
                        icon_size - 2.0 * gs,
                        icon_size - 2.0 * gs,
                        [0.24, 0.24, 0.24, 1.0],
                    );
                }
                let initials: String = server
                    .name
                    .split_whitespace()
                    .filter_map(|part| part.chars().next())
                    .take(2)
                    .collect();
                font_gui.draw_text_centered(
                    &mut self.font,
                    content_x + icon_size / 2.0,
                    row_y + 11.0 * gs,
                    &initials,
                    font_size * 0.72,
                    [0.82, 0.82, 0.82, 1.0],
                );

                let text_x = content_x + 35.0 * gs;
                font_gui.draw_text(
                    &mut self.font,
                    text_x,
                    row_y + 2.0 * gs,
                    &server.name,
                    font_size,
                    [1.0, 1.0, 1.0, 1.0],
                );

                let detail = server
                    .description
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&server.address);
                font_gui.draw_text(
                    &mut self.font,
                    text_x,
                    row_y + 14.0 * gs,
                    detail,
                    font_size * 0.72,
                    [0.63, 0.63, 0.63, 1.0],
                );
                if detail != server.address {
                    font_gui.draw_text(
                        &mut self.font,
                        text_x,
                        row_y + 24.0 * gs,
                        &server.address,
                        font_size * 0.62,
                        [0.48, 0.48, 0.48, 1.0],
                    );
                }

                let status = server_status_label(&text, &server);
                let status_width = self.font.text_width(&status, font_size * 0.72);
                font_gui.draw_text(
                    &mut self.font,
                    content_x + content_width - status_width - 14.0 * gs,
                    row_y + 2.0 * gs,
                    &status,
                    font_size * 0.72,
                    if server.online {
                        [0.72, 0.72, 0.72, 1.0]
                    } else {
                        [0.55, 0.28, 0.28, 1.0]
                    },
                );
                draw_ping_bars(
                    font_gui,
                    content_x + content_width - 10.0 * gs,
                    row_y + 2.0 * gs,
                    gs,
                    server.ping_ms,
                    server.online,
                );
            }
        }

        list.draw_scrollbar(font_gui, gs);
        list.draw_edge_fades(font_gui, gs);

        let has_selection = !self.state.server_list.server_list().is_empty()
            && self.state.server_list.selected_server() < self.state.server_list.server_list().len();
        let top_button_y = sh - 52.0 * gs;
        let lower_button_y = sh - 28.0 * gs;
        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::CONNECT,
            [
                sw / 2.0 - 154.0 * gs,
                top_button_y,
                100.0 * gs,
                metrics.btn_h,
            ],
            text.get("selectServer.select"),
            has_selection,
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DIRECT_CONNECT,
            [
                sw / 2.0 - 50.0 * gs,
                top_button_y,
                100.0 * gs,
                metrics.btn_h,
            ],
            text.get("selectServer.direct"),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::ADD_SERVER,
            [
                sw / 2.0 + 54.0 * gs,
                top_button_y,
                100.0 * gs,
                metrics.btn_h,
            ],
            text.get("selectServer.add"),
        );

        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::EDIT_SERVER,
            [
                sw / 2.0 - 154.0 * gs,
                lower_button_y,
                70.0 * gs,
                metrics.btn_h,
            ],
            text.get("selectServer.edit"),
            has_selection,
        );
        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DELETE_SERVER,
            [
                sw / 2.0 - 74.0 * gs,
                lower_button_y,
                70.0 * gs,
                metrics.btn_h,
            ],
            text.get("selectServer.delete"),
            has_selection,
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::REFRESH_SERVER_LIST,
            [
                sw / 2.0 + 4.0 * gs,
                lower_button_y,
                70.0 * gs,
                metrics.btn_h,
            ],
            text.get("selectServer.refresh"),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::MULTIPLAYER_CANCEL,
            [
                sw / 2.0 + 80.0 * gs,
                lower_button_y,
                75.0 * gs,
                metrics.btn_h,
            ],
            text.get("gui.cancel"),
        );
    }

    pub(super) fn draw_direct_connect_menu(
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

        self.draw_standard_screen(metrics, font_gui, "selectServer.direct", 20.0 * gs, 12.0 * gs);
        let text = self.state.settings.ui_text();
        let field_x = sw / 2.0 - 100.0 * gs;
        let field_y = 116.0 * gs;
        font_gui.draw_text(
            &mut self.font,
            field_x,
            100.0 * gs,
            text.get("addServer.enterIp"),
            metrics.font_sz,
            [0.65, 0.65, 0.65, 1.0],
        );
        draw_text_field(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DIRECT_ADDRESS_FIELD,
            field_x,
            field_y,
            200.0 * gs,
            &self.state.server_list.server_address().clone(),
            true,
        );

        let button_y = sh / 4.0 + 108.0 * gs;
        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::CONNECT,
            [metrics.btn_x, button_y, metrics.btn_w, metrics.btn_h],
            text.get("selectServer.select"),
            !self.state.server_list.server_address().trim().is_empty(),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::MULTIPLAYER_CANCEL,
            [
                metrics.btn_x,
                button_y + 24.0 * gs,
                metrics.btn_w,
                metrics.btn_h,
            ],
            text.get("gui.cancel"),
        );
    }

    pub(super) fn draw_server_editor_menu(
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

        self.draw_standard_screen(metrics, font_gui, "addServer.title", 17.0 * gs, 12.0 * gs);
        let text = self.state.settings.ui_text();

        let field_x = sw / 2.0 - 100.0 * gs;
        font_gui.draw_text(
            &mut self.font,
            field_x,
            53.0 * gs,
            text.get("addServer.enterName"),
            metrics.font_sz,
            [0.65, 0.65, 0.65, 1.0],
        );
        draw_text_field(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::SERVER_NAME_FIELD,
            field_x,
            66.0 * gs,
            200.0 * gs,
            &self.state.server_list.server_editor_name().clone(),
            !self.state.server_list.server_editor_address_focused(),
        );

        font_gui.draw_text(
            &mut self.font,
            field_x,
            94.0 * gs,
            text.get("addServer.enterIp"),
            metrics.font_sz,
            [0.65, 0.65, 0.65, 1.0],
        );
        draw_text_field(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::SERVER_ADDRESS_FIELD,
            field_x,
            106.0 * gs,
            200.0 * gs,
            &self.state.server_list.server_editor_address().clone(),
            self.state.server_list.server_editor_address_focused(),
        );

        let button_y = sh / 4.0 + 114.0 * gs;
        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::SERVER_EDITOR_SAVE,
            [metrics.btn_x, button_y, metrics.btn_w, metrics.btn_h],
            text.get("addServer.add"),
            !self.state.server_list.server_editor_name().trim().is_empty()
                && !self.state.server_list.server_editor_address().trim().is_empty(),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::SERVER_EDITOR_CANCEL,
            [
                metrics.btn_x,
                button_y + 24.0 * gs,
                metrics.btn_w,
                metrics.btn_h,
            ],
            text.get("gui.cancel"),
        );
    }
}

fn draw_text_field(
    font: &mut crate::ui::font::FontRenderer,
    metrics: &MenuMetrics,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    id: u32,
    x: f32,
    y: f32,
    width: f32,
    value: &str,
    focused: bool,
) {
    let gs = metrics.gs;
    font_gui.fill_rect(
        x - gs,
        y - gs,
        width + 2.0 * gs,
        22.0 * gs,
        if focused {
            [0.78, 0.78, 0.78, 1.0]
        } else {
            [0.42, 0.42, 0.42, 1.0]
        },
    );
    font_gui.fill_rect(x, y, width, 20.0 * gs, [0.0, 0.0, 0.0, 1.0]);
    widget_gui.register_button(id, x, y, width, 20.0 * gs);

    let blink = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 500)
        % 2
        == 0;
    let display = if focused && blink {
        format!("{}_", value)
    } else {
        value.to_string()
    };
    font_gui.draw_text(
        font,
        x + 4.0 * gs,
        y + 5.0 * gs,
        &display,
        metrics.font_sz,
        [0.9, 0.9, 0.9, 1.0],
    );
}

fn draw_ping_bars(
    gui: &mut GuiVertexBuilder,
    x: f32,
    y: f32,
    scale: f32,
    ping_ms: Option<u32>,
    online: bool,
) {
    let strength = if !online {
        0
    } else {
        match ping_ms.unwrap_or(u32::MAX) {
            0..=149 => 5,
            150..=299 => 4,
            300..=599 => 3,
            600..=999 => 2,
            _ => 1,
        }
    };
    for bar in 0..5 {
        let height = (bar + 1) as f32 * 1.4 * scale;
        gui.fill_rect(
            x + bar as f32 * 1.8 * scale,
            y + 8.0 * scale - height,
            1.2 * scale,
            height,
            if bar < strength {
                [0.65, 0.85, 0.42, 1.0]
            } else {
                [0.25, 0.25, 0.25, 1.0]
            },
        );
    }
}

fn server_status_label(text: &crate::ui::text::UiText, server: &ServerListRow) -> String {
    if server.online {
        match (server.players_online, server.players_max) {
            (Some(online), Some(max)) => format!("{}/{}", online, max),
            _ => text.get("rustcraft.multiplayer.online").to_string(),
        }
    } else if server.error.is_some() {
        text.get("rustcraft.multiplayer.offline").to_string()
    } else {
        text.get("rustcraft.multiplayer.unknown").to_string()
    }
}
