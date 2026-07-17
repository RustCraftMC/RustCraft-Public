use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;

impl Renderer {
    pub(super) fn draw_title_overlay(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
    ) {
        if self.state.title_alpha <= 0.01
            || (self.state.title_text.is_none() && self.state.subtitle_text.is_none())
        {
            return;
        }
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let alpha = self.state.title_alpha.clamp(0.0, 1.0);

        if let Some(title) = self.state.title_text.clone() {
            font_gui.draw_text_shadowed(
                &mut self.font,
                sw / 2.0,
                sh * 0.36,
                &title,
                22.0 * gs,
                [1.0, 1.0, 1.0, alpha],
                gs * 1.6,
            );
        }
        if let Some(subtitle) = self.state.subtitle_text.clone() {
            font_gui.draw_text_shadowed(
                &mut self.font,
                sw / 2.0,
                sh * 0.36 + 30.0 * gs,
                &subtitle,
                12.0 * gs,
                [1.0, 1.0, 1.0, alpha],
                gs,
            );
        }
    }

    /// Render the scoreboard sidebar — matches MCP 1.8.9 GuiIngame.renderScoreboard layout.
    pub(super) fn draw_scoreboard_sidebar(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
    ) {
        if self.state.sidebar_lines.is_empty() || self.state.chat_open || self.state.inventory_open
        {
            return;
        }
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let font_h = 10.0 * metrics.gs;
        let font_sz = metrics.font_sz * 0.72;
        let title = self.state.sidebar_title.clone().unwrap_or_default();

        // MCP: max width = max(titleWidth, max("name: score" widths))
        let mut max_w = self.font.text_width(&title, font_sz);
        for line in &self.state.sidebar_lines {
            let label = format!("{}: {}", line.display, line.score);
            max_w = max_w.max(self.font.text_width(&label, font_sz));
        }

        // MCP: l1 = screenWidth - max_w - 3 (left text edge), k1 = 3
        // Keep at least 4 px from the right screen edge so CJK titles never clip.
        let margin = 4.0 * metrics.gs;
        let l1 = (sw - max_w - margin).max(margin);
        let right_bg_edge = (l1 + max_w + 2.0 * metrics.gs).min(sw - margin);

        // MCP: vertical baseline = screenHeight/2 + totalScoreHeight/3
        let total_height = self.state.sidebar_lines.len() as f32 * font_h;
        let base_y = sh * 0.5 + total_height / 3.0;

        // sidebar_lines is sorted from highest to lowest score. Pixel y grows
        // downward, so the first (highest) entry needs the largest row index
        // to appear at the top, like vanilla GuiIngame.renderScoreboard.
        for (j, line) in self.state.sidebar_lines.iter().enumerate() {
            let row_y = base_y - (self.state.sidebar_lines.len().saturating_sub(j) as f32) * font_h;

            font_gui.fill_rect(
                l1 - 2.0 * metrics.gs,
                row_y,
                right_bg_edge - (l1 - 2.0 * metrics.gs),
                font_h,
                [0.0, 0.0, 0.0, 0.50],
            );

            let score_str = line.score.to_string();
            let score_w = self.font.text_width(&score_str, font_sz);
            let label_width = (max_w - score_w - 4.0 * metrics.gs).max(8.0 * metrics.gs);
            let label = fit_scoreboard_text(&self.font, &line.display, font_sz, label_width);
            font_gui.draw_text(
                &mut self.font,
                l1,
                row_y + 1.0 * metrics.gs,
                &label,
                font_sz,
                [0.84, 0.84, 0.84, 1.0],
            );

            font_gui.draw_text(
                &mut self.font,
                right_bg_edge - score_w,
                row_y + 1.0 * metrics.gs,
                &score_str,
                font_sz,
                [1.0, 0.32, 0.32, 1.0],
            );

            // Title drawn above the TOP row
            if j == 0 {
                let title_y = row_y - font_h;
                let title_w = self.font.text_width(&title, font_sz);
                font_gui.fill_rect(
                    l1 - 2.0 * metrics.gs,
                    title_y - 1.0 * metrics.gs,
                    right_bg_edge - (l1 - 2.0 * metrics.gs),
                    font_h + 1.0 * metrics.gs,
                    [0.0, 0.0, 0.0, 0.60],
                );
                font_gui.fill_rect(
                    l1 - 2.0 * metrics.gs,
                    title_y + font_h,
                    right_bg_edge - (l1 - 2.0 * metrics.gs),
                    1.0 * metrics.gs,
                    [0.0, 0.0, 0.0, 0.50],
                );
                let title_x = l1 + max_w * 0.5 - title_w * 0.5;
                font_gui.draw_text(
                    &mut self.font,
                    title_x,
                    title_y + 1.0 * metrics.gs,
                    &title,
                    font_sz,
                    [0.84, 0.84, 0.84, 1.0],
                );
            }
        }
    }
}

fn fit_scoreboard_text(
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
