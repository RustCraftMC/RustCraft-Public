use crate::render::gui::widgets::{draw_button, MenuMetrics};
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;

impl Renderer {
    pub(super) fn draw_book_editor(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        if !self.state.hud.book_editor_open() {
            return;
        }
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz * 0.68;
        let book_w = 192.0 * gs;
        let book_h = 192.0 * gs;
        let x = (sw - book_w) * 0.5;
        let y = (sh - book_h) * 0.5;
        let page_x = x + 14.0 * gs;
        let page_y = y + 29.0 * gs;
        let page_w = book_w - 28.0 * gs;
        let page_h = 112.0 * gs;

        font_gui.fill_rect(0.0, 0.0, sw, sh, [0.0, 0.0, 0.0, 0.62]);
        font_gui.fill_rect(x, y, book_w, book_h, [0.37, 0.23, 0.12, 1.0]);
        font_gui.fill_rect(
            x + 3.0 * gs,
            y + 3.0 * gs,
            book_w - 6.0 * gs,
            book_h - 6.0 * gs,
            [0.92, 0.83, 0.62, 1.0],
        );
        font_gui.fill_rect(
            x + book_w * 0.5 - gs,
            y + 4.0 * gs,
            2.0 * gs,
            book_h - 8.0 * gs,
            [0.48, 0.31, 0.16, 0.42],
        );

        if self.state.hud.book_signing() {
            self.draw_book_signing(metrics, widget_gui, font_gui, x, y, book_w, book_h, font_sz);
        } else {
            self.draw_book_page(
                metrics, widget_gui, font_gui, x, y, book_w, book_h, page_x, page_y, page_w,
                page_h, font_sz,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_book_page(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        x: f32,
        y: f32,
        book_w: f32,
        book_h: f32,
        page_x: f32,
        page_y: f32,
        page_w: f32,
        page_h: f32,
        font_sz: f32,
    ) {
        let gs = metrics.gs;
        let page = self.state.hud.book_pages().get(self.state.hud.book_page())
            .map(String::as_str)
            .unwrap_or("");
        let hovered = metrics.mouse_pos[0] >= page_x
            && metrics.mouse_pos[0] <= page_x + page_w
            && metrics.mouse_pos[1] >= page_y
            && metrics.mouse_pos[1] <= page_y + page_h;
        widget_gui.register_button(
            crate::ui::button_ids::BOOK_PAGE_FIELD,
            page_x,
            page_y,
            page_w,
            page_h,
        );
        if hovered {
            font_gui.fill_rect(page_x, page_y, page_w, page_h, [1.0, 1.0, 1.0, 0.14]);
        }
        font_gui.draw_text_centered(
            &mut self.font,
            x + book_w * 0.5,
            y + 10.0 * gs,
            "Book & Quill",
            font_sz,
            [0.20, 0.12, 0.06, 1.0],
        );
        for (line, text) in book_lines(page, 24).into_iter().take(10).enumerate() {
            font_gui.draw_text(
                &mut self.font,
                page_x + 3.0 * gs,
                page_y + line as f32 * 11.0 * gs,
                &text,
                font_sz,
                [0.15, 0.09, 0.04, 1.0],
            );
        }
        let cursor_y = page_y + book_lines(page, 24).len().min(9) as f32 * 11.0 * gs;
        font_gui.draw_text(
            &mut self.font,
            page_x + 3.0 * gs,
            cursor_y,
            "_",
            font_sz,
            [0.15, 0.09, 0.04, 1.0],
        );
        font_gui.draw_text_centered(
            &mut self.font,
            x + book_w * 0.5,
            y + 145.0 * gs,
            &format!(
                "Page {} of {}",
                self.state.hud.book_page() + 1,
                self.state.hud.book_pages().len()
            ),
            font_sz * 0.9,
            [0.25, 0.15, 0.08, 1.0],
        );

        let button_y = y + book_h - 25.0 * gs;
        if self.state.hud.book_page() > 0 {
            draw_button(
                metrics,
                widget_gui,
                font_gui,
                crate::ui::button_ids::BOOK_PREVIOUS_PAGE,
                [x + 8.0 * gs, button_y, 34.0 * gs, 18.0 * gs],
                "<",
                &mut self.font,
            );
        }
        draw_button(
            metrics,
            widget_gui,
            font_gui,
            crate::ui::button_ids::BOOK_NEXT_PAGE,
            [x + book_w - 42.0 * gs, button_y, 34.0 * gs, 18.0 * gs],
            ">",
            &mut self.font,
        );
        draw_button(
            metrics,
            widget_gui,
            font_gui,
            crate::ui::button_ids::BOOK_DONE,
            [x + 52.0 * gs, button_y, 42.0 * gs, 18.0 * gs],
            "Done",
            &mut self.font,
        );
        draw_button(
            metrics,
            widget_gui,
            font_gui,
            crate::ui::button_ids::BOOK_SIGN,
            [x + book_w - 94.0 * gs, button_y, 42.0 * gs, 18.0 * gs],
            "Sign",
            &mut self.font,
        );
    }

    fn draw_book_signing(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        x: f32,
        y: f32,
        book_w: f32,
        book_h: f32,
        font_sz: f32,
    ) {
        let gs = metrics.gs;
        let title_x = x + 28.0 * gs;
        let title_y = y + 66.0 * gs;
        let title_w = book_w - 56.0 * gs;
        let title_hovered = metrics.mouse_pos[0] >= title_x
            && metrics.mouse_pos[0] <= title_x + title_w
            && metrics.mouse_pos[1] >= title_y
            && metrics.mouse_pos[1] <= title_y + 18.0 * gs;
        widget_gui.register_button(
            crate::ui::button_ids::BOOK_TITLE_FIELD,
            title_x,
            title_y,
            title_w,
            18.0 * gs,
        );
        font_gui.draw_text_centered(
            &mut self.font,
            x + book_w * 0.5,
            y + 36.0 * gs,
            "Enter book title",
            font_sz * 1.2,
            [0.18, 0.10, 0.05, 1.0],
        );
        font_gui.fill_rect(
            title_x,
            title_y,
            title_w,
            18.0 * gs,
            [0.20, 0.12, 0.06, 0.25],
        );
        if title_hovered {
            font_gui.fill_rect(title_x, title_y, title_w, 2.0 * gs, [0.50, 0.30, 0.12, 0.9]);
        }
        font_gui.draw_text_centered(
            &mut self.font,
            x + book_w * 0.5,
            title_y + 4.0 * gs,
            &format!("{}_", self.state.hud.book_title()),
            font_sz,
            [0.14, 0.08, 0.03, 1.0],
        );
        let button_y = y + book_h - 30.0 * gs;
        draw_button(
            metrics,
            widget_gui,
            font_gui,
            crate::ui::button_ids::BOOK_CANCEL_SIGN,
            [x + 32.0 * gs, button_y, 56.0 * gs, 20.0 * gs],
            "Cancel",
            &mut self.font,
        );
        if !self.state.hud.book_title().trim().is_empty() {
            draw_button(
                metrics,
                widget_gui,
                font_gui,
                crate::ui::button_ids::BOOK_FINALIZE,
                [x + book_w - 88.0 * gs, button_y, 56.0 * gs, 20.0 * gs],
                "Finalize",
                &mut self.font,
            );
        }
    }
}

fn book_lines(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(String::new());
        } else if let Some(line) = lines.last_mut() {
            line.push(ch);
            if line.chars().count() >= width {
                lines.push(String::new());
            }
        }
    }
    lines
}
