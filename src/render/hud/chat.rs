use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;

impl Renderer {
    pub(super) fn draw_chat_overlay(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sh = self.swapchain.swapchain_extent.height as f32;
        let sw = self.swapchain.swapchain_extent.width as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        let chat_x = 4.0 * gs;
        let row_h = 11.0 * gs;
        // Open → full configured height.  Closed → only the last few messages.
        let max_visible = if self.state.hud.chat_open() {
            self.state.hud.chat_height().clamp(1, 30) as usize
        } else {
            6usize
        };

        if !self.state.hud.chat_overlay() && !self.state.hud.chat_open() {
            return;
        }

        // Calculate chat alpha:
        // - Open: always visible
        // - Closed: visible for 15s after last message, then fade over 1s
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        if self.state.hud.chat_open() {
            self.state.hud.set_chat_alpha(1.0);
        } else {
            let since_last = (now - self.state.hud.chat_last_message_time()) as f32;
            if since_last < 15.0 {
                self.state.hud.set_chat_alpha(1.0);
            } else {
                let fade = since_last - 15.0; // fade starts after 15s
                self.state.hud.set_chat_alpha((1.0 - fade).clamp(0.0, 1.0));
            }
        }

        if self.state.hud.chat_alpha() <= 0.01 && !self.state.hud.chat_open() {
            return;
        }

        let alpha = self.state.hud.chat_alpha();

        let chat_w = (sw * self.state.hud.chat_width().clamp(0.1, 1.0)).min(sw - 8.0 * gs) + 4.0 * gs;
        let avatar_size = (row_h - 2.0 * gs).max(0.0);
        let avatar_w = self.state.hud.chat_player_avatars() as u8 as f32 * (avatar_size + 2.0 * gs);
        let text_x = chat_x + 1.0 * gs + avatar_w;
        let text_w = (chat_w - avatar_w - 4.0 * gs).max(1.0);
        let text_size = font_sz * 0.72;
        let mut wrapped = Vec::new();
        for (idx, line) in self.state.hud.chat_lines().iter().enumerate() {
            let face = self.state.hud.chat_faces().get(idx).cloned().flatten();
            for text in wrap_chat_line(&self.font, line, text_size, text_w) {
                wrapped.push((text, face));
            }
        }

        // Bottom-to-top: visible rows are the last N wrapped message rows.
        let total = wrapped.len();
        let max_scroll = total.saturating_sub(max_visible);
        if self.state.hud.chat_scroll() > max_scroll {
            self.state.hud.set_chat_scroll(max_scroll);
        }

        // Scroll offset: 0 = show most recent, N = show N lines earlier
        let scroll = self.state.hud.chat_scroll();
        let visible = if scroll == 0 {
            // Show most recent messages
            total.saturating_sub(max_visible)..total
        } else {
            // Scrolled back: show earlier messages
            let start = total.saturating_sub(max_visible + scroll);
            let end = (start + max_visible).min(total);
            start..end
        };

        let visible_count = visible.len();
        // Position: bottom of chat area grows upward, sitting directly above
        // the input bar (input_y = sh - 28*gs, height = 18*gs → bottom = sh-28*gs).
        let chat_bottom_y = sh - 30.0 * gs;
        if self.state.hud.chat_background() && visible_count > 0 {
            font_gui.fill_rect(
                chat_x,
                chat_bottom_y - visible_count as f32 * row_h - 2.0 * gs,
                chat_w,
                visible_count as f32 * row_h + 2.0 * gs,
                [0.0, 0.0, 0.0, 0.78 * alpha],
            );
        }

        for (i, idx) in visible.enumerate() {
            // i=0 = oldest visible (top), i=visible_count-1 = newest (bottom)
            // Place text at the TOP of its row so it sits entirely inside the background.
            let (line, face) = &wrapped[idx];
            let y = chat_bottom_y - (visible_count - i) as f32 * row_h;
            if self.state.hud.chat_player_avatars() {
                if let Some(face) = face {
                    font_gui.draw_pixel_face(chat_x + 2.0 * gs, y, avatar_size / 8.0, face);
                }
            }
            font_gui.draw_text(
                &mut self.font,
                text_x,
                y,
                line,
                text_size,
                [0.9, 0.9, 0.9, alpha],
            );
        }

        if self.state.hud.chat_open() {
            let input_y = sh - 28.0 * gs;
            let input_w = (sw * 0.5).min(450.0 * gs);
            let input_hovered = metrics.mouse_pos[0] >= chat_x
                && metrics.mouse_pos[0] <= chat_x + input_w
                && metrics.mouse_pos[1] >= input_y
                && metrics.mouse_pos[1] <= input_y + 18.0 * gs;
            widget_gui.register_button(
                crate::ui::button_ids::CHAT_INPUT,
                chat_x,
                input_y,
                input_w,
                18.0 * gs,
            );
            font_gui.fill_rect(chat_x, input_y, input_w, 18.0 * gs, [0.0, 0.0, 0.0, 0.78]);
            font_gui.fill_rect(
                chat_x,
                input_y,
                input_w,
                1.0 * gs,
                if input_hovered {
                    [0.82, 0.82, 0.82, 1.0]
                } else {
                    [0.55, 0.55, 0.55, 0.9]
                },
            );
            font_gui.draw_text(
                &mut self.font,
                chat_x + 4.0 * gs,
                input_y + 5.0 * gs,
                &format!(">{}", self.state.hud.chat_input()),
                font_sz,
                [1.0, 1.0, 1.0, 1.0],
            );
            let button_x = chat_x + input_w + 3.0 * gs;
            let button_w = 16.0 * gs;
            let button_h = 8.0 * gs;
            let up_hovered = metrics.mouse_pos[0] >= button_x
                && metrics.mouse_pos[0] <= button_x + button_w
                && metrics.mouse_pos[1] >= input_y
                && metrics.mouse_pos[1] <= input_y + button_h;
            let down_y = input_y + 10.0 * gs;
            let down_hovered = metrics.mouse_pos[0] >= button_x
                && metrics.mouse_pos[0] <= button_x + button_w
                && metrics.mouse_pos[1] >= down_y
                && metrics.mouse_pos[1] <= down_y + button_h;
            widget_gui.register_button(
                crate::ui::button_ids::CHAT_SCROLL_UP,
                button_x,
                input_y,
                button_w,
                button_h,
            );
            widget_gui.register_button(
                crate::ui::button_ids::CHAT_SCROLL_DOWN,
                button_x,
                down_y,
                button_w,
                button_h,
            );
            font_gui.fill_rect(
                button_x,
                input_y,
                button_w,
                button_h,
                if up_hovered {
                    [0.32, 0.32, 0.32, 0.92]
                } else {
                    [0.0, 0.0, 0.0, 0.78]
                },
            );
            font_gui.fill_rect(
                button_x,
                down_y,
                button_w,
                button_h,
                if down_hovered {
                    [0.32, 0.32, 0.32, 0.92]
                } else {
                    [0.0, 0.0, 0.0, 0.78]
                },
            );
            font_gui.draw_text_centered(
                &mut self.font,
                button_x + button_w * 0.5,
                input_y - 2.0 * gs,
                "^",
                font_sz * 0.65,
                [0.9, 0.9, 0.9, 1.0],
            );
            font_gui.draw_text_centered(
                &mut self.font,
                button_x + button_w * 0.5,
                input_y + 8.0 * gs,
                "v",
                font_sz * 0.65,
                [0.9, 0.9, 0.9, 1.0],
            );
        }
    }

    pub(super) fn draw_player_list_overlay(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
    ) {
        if !self.state.hud.player_list_open() || self.state.hud.player_list().is_empty() || self.state.hud.chat_open()
        {
            return;
        }
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;

        // Measure the longest player name using actual font metrics (MCP style)
        let ping_w = self.font.text_width("XXXXms", font_sz * 0.62);
        let max_name_w = self.state.hud.player_list().iter()
            .map(|(name, _, _)| self.font.text_width(name, font_sz * 0.72))
            .fold(40.0 * gs, f32::max);
        let col_w = (max_name_w + ping_w + 24.0 * gs).max(80.0 * gs);

        let max_total_w = sw * 0.85; // max 85% of screen width
        let columns = ((self.state.hud.player_list().len() + 19) / 20).clamp(1, 4);
        let rows = ((self.state.hud.player_list().len() + columns - 1) / columns).min(20);
        let header_lines = self.state.hud.tab_header().as_ref()
            .map(|text| split_overlay_lines(text, 3))
            .unwrap_or_default();
        let footer_lines = self.state.hud.tab_footer().as_ref()
            .map(|text| split_overlay_lines(text, 3))
            .unwrap_or_default();
        let header_h = header_lines.len() as f32 * 11.0 * gs;
        let footer_h = footer_lines.len() as f32 * 11.0 * gs;

        // If panel would overflow, reduce columns
        let mut panel_w = columns as f32 * col_w + 12.0 * gs;
        if panel_w > max_total_w {
            let reduced_cols = ((max_total_w - 12.0 * gs) / col_w).floor() as usize;
            let cols = reduced_cols.max(1).min(columns);
            panel_w = cols as f32 * col_w + 12.0 * gs;
        }

        let row_h = 11.0 * gs;
        let panel_h = rows as f32 * row_h + 28.0 * gs + header_h + footer_h;
        let x = (sw - panel_w) / 2.0;
        let y = 18.0 * gs;

        font_gui.fill_rect(x, y, panel_w, panel_h, [0.0, 0.0, 0.0, 0.72]);
        font_gui.fill_rect(x, y, panel_w, 1.0 * gs, [0.55, 0.55, 0.55, 0.9]);
        let mut cursor_y = y + 5.0 * gs;
        for line in &header_lines {
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                cursor_y,
                line,
                font_sz * 0.72,
                [0.92, 0.92, 0.92, 1.0],
            );
            cursor_y += 11.0 * gs;
        }
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            cursor_y,
            &format!("Players ({})", self.state.hud.player_list().len()),
            font_sz,
            [1.0, 1.0, 1.0, 1.0],
            gs,
        );
        let list_y = cursor_y + 15.0 * gs;

        for (idx, (name, ping, gamemode)) in self.state.hud.player_list().iter().enumerate() {
            let col = idx / rows;
            let row = idx % rows;
            let tx = x + 6.0 * gs + col as f32 * col_w;
            let ty = list_y + row as f32 * row_h;
            let ping_color = if *ping < 150 {
                [0.45, 1.0, 0.35, 1.0]
            } else if *ping < 300 {
                [1.0, 0.85, 0.25, 1.0]
            } else {
                [1.0, 0.35, 0.25, 1.0]
            };
            let mode = match *gamemode {
                1 => "C",
                2 => "A",
                3 => "S",
                _ => "",
            };
            let name_width = (col_w - ping_w - 10.0 * gs).max(12.0 * gs);
            let label = fit_overlay_text(
                &self.font,
                &format!("{} {}", mode, name),
                font_sz * 0.72,
                name_width,
            );
            font_gui.draw_text(
                &mut self.font,
                tx,
                ty,
                &label,
                font_sz * 0.72,
                [0.92, 0.92, 0.92, 1.0],
            );
            // Align ping to right edge of column with padding
            let ping_x = tx + col_w - ping_w - 4.0 * gs;
            font_gui.draw_text(
                &mut self.font,
                ping_x,
                ty,
                &format!("{}ms", ping),
                font_sz * 0.62,
                ping_color,
            );
        }

        let footer_y = list_y + rows as f32 * row_h + 4.0 * gs;
        for (i, line) in footer_lines.iter().enumerate() {
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                footer_y + i as f32 * 11.0 * gs,
                line,
                font_sz * 0.70,
                [0.78, 0.78, 0.78, 1.0],
            );
        }
    }

    pub(super) fn draw_sign_editor(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
    ) {
        if !self.state.hud.sign_editor_open() {
            return;
        }
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        font_gui.fill_rect(0.0, 0.0, sw, sh, [0.0, 0.0, 0.0, 0.52]);

        let sign_w = 160.0 * gs;
        let sign_h = 96.0 * gs;
        let x = (sw - sign_w) * 0.5;
        let y = (sh - sign_h) * 0.5 - 24.0 * gs;
        font_gui.fill_rect(x, y, sign_w, sign_h, [0.57, 0.35, 0.16, 1.0]);
        font_gui.fill_rect(
            x + 4.0 * gs,
            y + 4.0 * gs,
            sign_w - 8.0 * gs,
            sign_h - 8.0 * gs,
            [0.70, 0.46, 0.22, 1.0],
        );
        font_gui.fill_rect(
            x + sign_w * 0.5 - 5.0 * gs,
            y + sign_h,
            10.0 * gs,
            42.0 * gs,
            [0.33, 0.20, 0.10, 1.0],
        );

        for i in 0..4 {
            let line_y = y + 18.0 * gs + i as f32 * 17.0 * gs;
            if i == self.state.hud.sign_editor_active_line() {
                font_gui.fill_rect(
                    x + 18.0 * gs,
                    line_y - 3.0 * gs,
                    sign_w - 36.0 * gs,
                    13.0 * gs,
                    [0.0, 0.0, 0.0, 0.20],
                );
            }
            let mut text = self.state.hud.sign_editor_lines_mut()[i].clone();
            if i == self.state.hud.sign_editor_active_line() {
                text.push('_');
            }
            font_gui.draw_text_centered(
                &mut self.font,
                x + sign_w * 0.5,
                line_y,
                &text,
                font_sz * 0.90,
                [0.08, 0.06, 0.04, 1.0],
            );
        }
    }
}

fn fit_overlay_text(
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

fn wrap_chat_line(
    font: &crate::ui::font::FontRenderer,
    text: &str,
    size: f32,
    max_width: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut active_color = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{00a7}' {
            if let Some(code) = chars.next() {
                line.push(ch);
                line.push(code);
                if code.eq_ignore_ascii_case(&'r') {
                    active_color.clear();
                } else if code.is_ascii_hexdigit() {
                    active_color.clear();
                    active_color.push(ch);
                    active_color.push(code);
                }
            }
            continue;
        }
        let mut candidate = line.clone();
        candidate.push(ch);
        if !line.is_empty() && font.text_width(&candidate, size) > max_width {
            lines.push(line);
            line = active_color.clone();
        }
        line.push(ch);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn split_overlay_lines(text: &str, max_lines: usize) -> Vec<String> {
    text.lines()
        .flat_map(|line| {
            let mut out = Vec::new();
            let mut current = String::new();
            for ch in line.chars() {
                current.push(ch);
                if current.chars().count() >= 42 {
                    out.push(std::mem::take(&mut current));
                }
            }
            if !current.is_empty() {
                out.push(current);
            }
            out
        })
        .filter(|line| !line.trim().is_empty())
        .take(max_lines)
        .collect()
}
