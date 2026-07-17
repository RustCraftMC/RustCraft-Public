use nalgebra::{Matrix3, Vector3};

use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::hooks::ScriptDrawCommand;
use crate::render::Renderer;

impl Renderer {
    pub(super) fn draw_script_hud(
        &mut self,
        commands: &[ScriptDrawCommand],
        metrics: &MenuMetrics,
        font_batch: &mut GuiVertexBuilder,
        widget_batch: &mut GuiVertexBuilder,
        icons_batch: &mut GuiVertexBuilder,
        inventory_batch: &mut GuiVertexBuilder,
    ) {
        let mut transform = Matrix3::<f32>::identity();
        let mut stack = Vec::new();
        let mut scissor: Option<[f32; 4]> = None;

        for command in commands {
            match command {
                ScriptDrawCommand::PushTransform => stack.push(transform),
                ScriptDrawCommand::PopTransform => {
                    if let Some(previous) = stack.pop() {
                        transform = previous;
                    }
                }
                ScriptDrawCommand::Translate { x, y } => {
                    transform = Matrix3::new(1.0, 0.0, *x, 0.0, 1.0, *y, 0.0, 0.0, 1.0) * transform;
                }
                ScriptDrawCommand::Rotate { degrees } => {
                    let (sin, cos) = degrees.to_radians().sin_cos();
                    transform =
                        Matrix3::new(cos, -sin, 0.0, sin, cos, 0.0, 0.0, 0.0, 1.0) * transform;
                }
                ScriptDrawCommand::Scale { x, y } => {
                    transform = Matrix3::new(*x, 0.0, 0.0, 0.0, *y, 0.0, 0.0, 0.0, 1.0) * transform;
                }
                ScriptDrawCommand::SetScissor(rect) => scissor = *rect,
                ScriptDrawCommand::Text {
                    text,
                    x,
                    y,
                    scale,
                    color,
                } => {
                    let point = transform_point(&transform, *x, *y);
                    if point_visible(point, scissor) {
                        let scale_factor = transform_scale(&transform);
                        font_batch.draw_text(
                            &mut self.font,
                            point[0],
                            point[1],
                            text,
                            9.0 * metrics.gs * scale * scale_factor,
                            color.array(),
                        );
                    }
                }
                ScriptDrawCommand::Rect {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => {
                    let points = transformed_quad(&transform, *x, *y, *width, *height);
                    if points.iter().any(|point| point_visible(*point, scissor)) {
                        font_batch.fill_quad(
                            points[0][0],
                            points[0][1],
                            points[1][0],
                            points[1][1],
                            points[2][0],
                            points[2][1],
                            points[3][0],
                            points[3][1],
                            color.array(),
                        );
                    }
                }
                ScriptDrawCommand::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    color,
                } => {
                    let from = transform_point(&transform, *x1, *y1);
                    let to = transform_point(&transform, *x2, *y2);
                    if point_visible(from, scissor) || point_visible(to, scissor) {
                        font_batch.draw_line(
                            from,
                            to,
                            width * transform_scale(&transform),
                            color.array(),
                        );
                    }
                }
                ScriptDrawCommand::Crosshair {
                    x_offset,
                    y_offset,
                    size,
                    gap,
                    thickness,
                    color,
                } => {
                    let cx = self.swapchain_extent.width as f32 * 0.5 + *x_offset;
                    let cy = self.swapchain_extent.height as f32 * 0.5 + *y_offset;
                    let half_thickness = *thickness * 0.5;
                    let color = color.array();
                    font_batch.fill_rect(
                        cx - gap - size,
                        cy - half_thickness,
                        *size,
                        *thickness,
                        color,
                    );
                    font_batch.fill_rect(cx + gap, cy - half_thickness, *size, *thickness, color);
                    font_batch.fill_rect(
                        cx - half_thickness,
                        cy - gap - size,
                        *thickness,
                        *size,
                        color,
                    );
                    font_batch.fill_rect(cx - half_thickness, cy + gap, *thickness, *size, color);
                }
                ScriptDrawCommand::Image {
                    resource,
                    x,
                    y,
                    width,
                    height,
                    color,
                } => {
                    let points = transformed_quad(&transform, *x, *y, *width, *height);
                    if !points.iter().any(|point| point_visible(*point, scissor)) {
                        continue;
                    }
                    let target = match resource.as_str() {
                        "minecraft:textures/gui/widgets.png" => Some(&mut *widget_batch),
                        "minecraft:textures/gui/icons.png" => Some(&mut *icons_batch),
                        "minecraft:textures/gui/container/inventory.png" => {
                            Some(&mut *inventory_batch)
                        }
                        _ => None,
                    };
                    if let Some(target) = target {
                        target.textured_quad(
                            points,
                            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                            color.array(),
                        );
                    }
                }
            }
        }
    }
}

fn transform_point(transform: &Matrix3<f32>, x: f32, y: f32) -> [f32; 2] {
    let point = transform * Vector3::new(x, y, 1.0);
    [point.x, point.y]
}

fn transformed_quad(
    transform: &Matrix3<f32>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> [[f32; 2]; 4] {
    [
        transform_point(transform, x, y),
        transform_point(transform, x + width, y),
        transform_point(transform, x + width, y + height),
        transform_point(transform, x, y + height),
    ]
}

fn transform_scale(transform: &Matrix3<f32>) -> f32 {
    let x = (transform[(0, 0)].powi(2) + transform[(1, 0)].powi(2)).sqrt();
    let y = (transform[(0, 1)].powi(2) + transform[(1, 1)].powi(2)).sqrt();
    ((x + y) * 0.5).clamp(0.01, 100.0)
}

fn point_visible(point: [f32; 2], scissor: Option<[f32; 4]>) -> bool {
    let Some([x, y, width, height]) = scissor else {
        return true;
    };
    point[0] >= x && point[0] <= x + width && point[1] >= y && point[1] <= y + height
}
