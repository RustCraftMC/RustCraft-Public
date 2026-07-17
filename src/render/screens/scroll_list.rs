use crate::render::gui::GuiVertexBuilder;
use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub(super) struct GuiScrollList {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub row_height: f32,
    pub item_count: usize,
    pub first_row: usize,
    visible_rows: usize,
}

impl GuiScrollList {
    pub fn new(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        row_height: f32,
        item_count: usize,
        requested_first_row: usize,
    ) -> Self {
        let visible_rows = (height / row_height).floor().max(1.0) as usize;
        let max_first = item_count.saturating_sub(visible_rows);
        Self {
            x,
            y,
            width,
            height,
            row_height,
            item_count,
            first_row: requested_first_row.min(max_first),
            visible_rows,
        }
    }

    pub fn visible_range(&self) -> Range<usize> {
        self.first_row..(self.first_row + self.visible_rows).min(self.item_count)
    }

    pub fn row_y(&self, index: usize) -> f32 {
        self.y + (index.saturating_sub(self.first_row)) as f32 * self.row_height
    }

    pub fn max_first_row(&self) -> usize {
        self.item_count.saturating_sub(self.visible_rows)
    }

    pub fn draw_background(&self, gui: &mut GuiVertexBuilder) {
        gui.fill_rect(
            self.x,
            self.y,
            self.width,
            self.height,
            [0.0, 0.0, 0.0, 0.58],
        );
        gui.fill_rect(self.x, self.y, self.width, 1.0, [0.5, 0.5, 0.5, 0.5]);
        gui.fill_rect(
            self.x,
            self.y + self.height - 1.0,
            self.width,
            1.0,
            [0.5, 0.5, 0.5, 0.5],
        );
    }

    pub fn draw_scrollbar(&self, gui: &mut GuiVertexBuilder, scale: f32) {
        let max_first = self.max_first_row();
        if max_first == 0 {
            return;
        }

        let track_width = 6.0 * scale;
        let track_x = self.x + self.width - track_width - 2.0 * scale;
        let track_y = self.y + 2.0 * scale;
        let track_height = self.height - 4.0 * scale;
        gui.fill_rect(
            track_x,
            track_y,
            track_width,
            track_height,
            [0.0, 0.0, 0.0, 0.65],
        );

        let visible_ratio = self.visible_rows as f32 / self.item_count.max(1) as f32;
        let thumb_height = (track_height * visible_ratio).max(16.0 * scale);
        let progress = self.first_row as f32 / max_first as f32;
        let thumb_y = track_y + (track_height - thumb_height) * progress;
        gui.fill_rect(
            track_x,
            thumb_y,
            track_width,
            thumb_height,
            [0.55, 0.55, 0.55, 1.0],
        );
        gui.fill_rect(
            track_x,
            thumb_y,
            1.0 * scale,
            thumb_height,
            [0.82, 0.82, 0.82, 1.0],
        );
    }

    pub fn draw_edge_fades(&self, gui: &mut GuiVertexBuilder, scale: f32) {
        let band = 3.0 * scale;
        gui.fill_rect(self.x, self.y, self.width, band, [0.0, 0.0, 0.0, 0.35]);
        gui.fill_rect(
            self.x,
            self.y + self.height - band,
            self.width,
            band,
            [0.0, 0.0, 0.0, 0.35],
        );
    }
}
