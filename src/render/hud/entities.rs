use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;

/// Text and placement state for one sign tile entity.
///
/// The sign block's metadata is needed because vanilla rotates the text with
/// the board; it cannot be reconstructed from the tile-entity NBT alone.
#[derive(Clone, Debug)]
pub struct SignEntry {
    pub position: [i32; 3],
    pub lines: Vec<String>,
    pub wall_mounted: bool,
    pub metadata: u8,
}

impl Renderer {
    pub(super) fn draw_block_selection(
        &self,
        _metrics: &MenuMetrics,
        _font_gui: &mut GuiVertexBuilder,
    ) {
        // Block selection wireframe and dig crack overlay are now rendered as 3D geometry
        // in the entity pass (upload_entity_meshes in rendering.rs) with proper depth testing.
        // This function is now a no-op — all visuals are in the 3D pass.
    }

    pub(super) fn draw_entity_overlays(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
        _block_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        let Some(camera) = self.current_camera.as_ref() else {
            return;
        };

        for entity in self.state.entity_billboards.iter().take(256) {
            let top = [
                entity.position[0],
                entity.position[1] + entity.height + 0.35,
                entity.position[2],
            ];
            let Some(([sx, sy], depth)) =
                crate::render::particles::project_world_to_screen(camera, top, sw, sh)
            else {
                continue;
            };
            if depth > self.state.render_distance as f32 * 16.0 {
                continue;
            }

            let scale = (70.0 / depth).clamp(0.35, 1.35) * gs;
            let body_h = (entity.height * 15.0 * scale).clamp(8.0 * gs, 34.0 * gs);
            let body_w = (entity.width * 14.0 * scale).clamp(4.0 * gs, 18.0 * gs);
            let y = sy + entity.death_alpha * 5.0 * gs;

            // Mob geometry is rendered by the 3D entity pass. Do not draw the
            // old screen-space preview here as well: it was offset from the
            // entity and appeared as an unexplained image above animals such
            // as sheep and wolves.

            if entity.critical_alpha > 0.0 {
                draw_crit_sparkles(font_gui, sx, y, body_w, body_h, gs, entity.critical_alpha);
            }

            // Nametags are now rendered as 3D billboard meshes
            // with depth testing disabled (visible through walls).
        }

        // Sign text is rendered as 3D textured quads in upload_entity_meshes
    }
}

fn draw_skin_preview_pixels(
    font_gui: &mut GuiVertexBuilder,
    preview: &crate::assets::skin::SkinPreviewPixels,
    x: f32,
    y: f32,
    px: f32,
    slim: bool,
    swing: f32,
    shade: f32,
) {
    let arm_w = if slim { 3 } else { 4 };
    let arm_gap = if slim { px } else { 0.0 };
    draw_skin_region(font_gui, x + 4.0 * px, y, 8, 8, 8, px, &preview.head, shade);
    draw_skin_region(
        font_gui,
        x + 4.0 * px,
        y + 8.0 * px,
        8,
        8,
        12,
        px,
        &preview.body,
        shade,
    );
    draw_skin_region(
        font_gui,
        x + arm_gap,
        y + (8.0 + swing * 2.0) * px,
        4,
        arm_w,
        12,
        px,
        &preview.right_arm,
        shade * 0.88,
    );
    draw_skin_region(
        font_gui,
        x + 12.0 * px,
        y + (8.0 - swing * 2.0) * px,
        4,
        arm_w,
        12,
        px,
        &preview.left_arm,
        shade * 0.88,
    );
    draw_skin_region(
        font_gui,
        x + 4.0 * px,
        y + 20.0 * px,
        4,
        4,
        12,
        px,
        &preview.right_leg,
        shade * 0.92,
    );
    draw_skin_region(
        font_gui,
        x + 8.0 * px,
        y + 20.0 * px,
        4,
        4,
        12,
        px,
        &preview.left_leg,
        shade * 0.92,
    );
}

fn draw_skin_region<const N: usize>(
    font_gui: &mut GuiVertexBuilder,
    x: f32,
    y: f32,
    source_w: usize,
    draw_w: usize,
    h: usize,
    px: f32,
    pixels: &[[u8; 4]; N],
    shade: f32,
) {
    for py in 0..h {
        for px_idx in 0..draw_w {
            let p = pixels[py * source_w + px_idx];
            if p[3] == 0 {
                continue;
            }
            font_gui.fill_rect(
                x + px_idx as f32 * px,
                y + py as f32 * px,
                px,
                px,
                [
                    p[0] as f32 / 255.0 * shade,
                    p[1] as f32 / 255.0 * shade,
                    p[2] as f32 / 255.0 * shade,
                    p[3] as f32 / 255.0,
                ],
            );
        }
    }
}

fn draw_crit_sparkles(
    font_gui: &mut GuiVertexBuilder,
    sx: f32,
    y: f32,
    body_w: f32,
    body_h: f32,
    gs: f32,
    alpha: f32,
) {
    let color = [0.72, 0.86, 1.0, 0.75 * alpha];
    for (ox, oy) in [(-0.6, 0.2), (0.4, 0.1), (-0.2, 0.55), (0.65, 0.65)] {
        let x = sx + ox * body_w;
        let cy = y + oy * body_h;
        font_gui.draw_line([x - 2.0 * gs, cy], [x + 2.0 * gs, cy], 1.0 * gs, color);
        font_gui.draw_line([x, cy - 2.0 * gs], [x, cy + 2.0 * gs], 1.0 * gs, color);
    }
}
