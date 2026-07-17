use super::GuiVertexBuilder;
use crate::ui::font::FontRenderer;

pub struct MenuMetrics {
    pub sw: f32,
    pub sh: f32,
    pub gs: f32,
    pub font_sz: f32,
    pub btn_w: f32,
    pub btn_h: f32,
    pub btn_gap: f32,
    pub btn_x: f32,
    pub mouse_pos: [f32; 2],
}

impl MenuMetrics {
    pub fn new(sw: f32, sh: f32, gui_scale: u32, mouse_pos: [f32; 2]) -> Self {
        let gs = gui_scale as f32;
        let btn_w = 200.0 * gs;
        Self {
            sw,
            sh,
            gs,
            font_sz: 9.0 * gs,
            btn_w,
            btn_h: 20.0 * gs,
            btn_gap: 4.0 * gs,
            btn_x: (sw - btn_w) / 2.0,
            mouse_pos,
        }
    }
}

pub fn draw_button(
    metrics: &MenuMetrics,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    id: u32,
    rect: [f32; 4],
    label: &str,
    font: &mut FontRenderer,
) {
    let [x, y, w, h] = rect;
    let hovered = metrics.mouse_pos[0] >= x
        && metrics.mouse_pos[0] <= x + w
        && metrics.mouse_pos[1] >= y
        && metrics.mouse_pos[1] <= y + h;
    let state = if hovered { 1 } else { 0 };
    widget_gui.draw_button_rect_state(x, y, w, h, state);
    widget_gui.register_button(id, x, y, w, h);
    font_gui.draw_text_shadowed(
        font,
        x + w / 2.0,
        y + (h - metrics.font_sz) / 2.0,
        label,
        metrics.font_sz,
        [0.9, 0.9, 0.9, 1.0],
        metrics.gs,
    );
}
