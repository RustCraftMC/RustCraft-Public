//! Block icon rendering for inventory and hotbar.
//!
//! Uses the JSON block model system to render proper 3D-block previews
//! in GUI, using the block texture atlas.

use crate::assets::texture::tile_uv_rect;
use crate::render::gui::GuiVertexBuilder;
use crate::world::block::Block;
use crate::world::block_models::BlockModelCache;

/// Draw an isometric 3D block model at `(cx, cy)` using the JSON model system.
///
/// Returns false when no 3D model is available (caller should fall back to 2D icon).
pub fn draw_block_icon(
    gui: &mut GuiVertexBuilder,
    cx: f32,
    cy: f32,
    size: f32,
    block_id: u16,
    meta: u16,
) -> bool {
    if block_id == 0 {
        return false;
    }
    draw_block_model_3d(gui, cx, cy, size, block_id, meta as u8, -30.0, 45.0)
}

/// Draw a block model item in the handheld position (first person view).
pub fn draw_held_block_3d(
    gui: &mut GuiVertexBuilder,
    cx: f32,
    cy: f32,
    size: f32,
    block_id: u16,
) -> bool {
    draw_block_model_3d(gui, cx, cy, size, block_id, 0, -10.0, 0.0)
}

fn draw_block_model_3d(
    gui: &mut GuiVertexBuilder,
    cx: f32,
    cy: f32,
    size: f32,
    block_id: u16,
    meta: u8,
    rot_x: f32,
    rot_y: f32,
) -> bool {
    if !BlockModelCache::is_available() {
        draw_flat_block_icon(gui, cx, cy, size, block_id);
        return true;
    }
    let cache = BlockModelCache::global();
    let Some(model) = cache.get_model(block_id, meta) else {
        draw_flat_block_icon(gui, cx, cy, size, block_id);
        return true;
    };
    if model.faces.is_empty() {
        return false;
    }

    let scale = size / 16.0;
    let (sx, cx_r) = rot_x.to_radians().sin_cos();
    let (sy, cy_r) = rot_y.to_radians().sin_cos();

    let block = Block::from_state((block_id << 4) | meta as u16);
    let mut face_draws: Vec<(f32, [[f32; 2]; 4], [[f32; 2]; 4], [f32; 3])> =
        Vec::with_capacity(model.faces.len());

    for face in &model.faces {
        if face.vertices.is_empty() {
            continue;
        }
        let tile_idx = cache.texture_index(&face.texture);
        let rect = tile_uv_rect(tile_idx);
        let [u_min, v_min, u_max, v_max] = rect;

        let mut screen_pts = [[0.0f32; 2]; 4];
        let mut depth = 0.0f32;
        let normal = rotate(face.normal, sx, cx_r, sy, cy_r);
        if !front_facing(normal) {
            continue;
        }

        for i in 0..4 {
            let v = face.vertices[i];
            let px = v[0] - 8.0;
            let py = v[1] - 8.0;
            let pz = v[2] - 8.0;

            // Y rotation
            let rx = px * cy_r + pz * sy;
            let rz1 = -px * sy + pz * cy_r;
            // X rotation
            let ry = py * cx_r - rz1 * sx;
            let rz = py * sx + rz1 * cx_r;

            screen_pts[i] = [cx + rx * scale, cy - ry * scale];
            depth += rz;
        }
        depth /= 4.0;

        let fuvs = face.uvs;
        let auvs: [[f32; 2]; 4] = [
            [
                u_min + fuvs[0][0] * (u_max - u_min),
                v_min + fuvs[0][1] * (v_max - v_min),
            ],
            [
                u_min + fuvs[1][0] * (u_max - u_min),
                v_min + fuvs[1][1] * (v_max - v_min),
            ],
            [
                u_min + fuvs[2][0] * (u_max - u_min),
                v_min + fuvs[2][1] * (v_max - v_min),
            ],
            [
                u_min + fuvs[3][0] * (u_max - u_min),
                v_min + fuvs[3][1] * (v_max - v_min),
            ],
        ];

        let material = crate::world::material::model_material(block, face.tintindex);
        let tint = crate::world::material::material_color(material);
        let shade = standard_item_light(normal);
        face_draws.push((
            depth,
            screen_pts,
            auvs,
            [tint[0] * shade, tint[1] * shade, tint[2] * shade],
        ));
    }

    face_draws.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    for (_depth, pts, uvs, color) in &face_draws {
        gui.textured_quad(
            [pts[0], pts[1], pts[2], pts[3]],
            *uvs,
            [color[0], color[1], color[2], 1.0],
        );
    }
    true
}

/// Fallback: draw a simple 3D cube icon for blocks without a JSON model.
/// Uses the block's tile textures (top, bottom, side) just like world rendering.
fn draw_flat_block_icon(gui: &mut GuiVertexBuilder, cx: f32, cy: f32, size: f32, block_id: u16) {
    let block = Block::from_id(block_id);
    let (top_t, bot_t, side_t) = block.tiles();

    let scale = size / 16.0;
    let rot_y = 45.0f32.to_radians();
    let rot_x = -30.0f32.to_radians();
    let (sx, cx_r) = rot_x.sin_cos();
    let (sy, cy_r) = rot_y.sin_cos();

    struct CubeFace {
        tile: usize,
        normal: [f32; 3],
        corners: [[f32; 3]; 4],
    }

    let faces = [
        CubeFace {
            tile: top_t,
            normal: [0.0, 1.0, 0.0],
            corners: [
                [-8.0, 8.0, -8.0],
                [8.0, 8.0, -8.0],
                [8.0, 8.0, 8.0],
                [-8.0, 8.0, 8.0],
            ],
        },
        CubeFace {
            tile: bot_t,
            normal: [0.0, -1.0, 0.0],
            corners: [
                [-8.0, -8.0, 8.0],
                [8.0, -8.0, 8.0],
                [8.0, -8.0, -8.0],
                [-8.0, -8.0, -8.0],
            ],
        },
        CubeFace {
            tile: side_t,
            normal: [0.0, 0.0, 1.0],
            corners: [
                [-8.0, -8.0, 8.0],
                [8.0, -8.0, 8.0],
                [8.0, 8.0, 8.0],
                [-8.0, 8.0, 8.0],
            ],
        },
        CubeFace {
            tile: side_t,
            normal: [0.0, 0.0, -1.0],
            corners: [
                [8.0, -8.0, -8.0],
                [-8.0, -8.0, -8.0],
                [-8.0, 8.0, -8.0],
                [8.0, 8.0, -8.0],
            ],
        },
        CubeFace {
            tile: side_t,
            normal: [1.0, 0.0, 0.0],
            corners: [
                [8.0, -8.0, -8.0],
                [8.0, -8.0, 8.0],
                [8.0, 8.0, 8.0],
                [8.0, 8.0, -8.0],
            ],
        },
        CubeFace {
            tile: side_t,
            normal: [-1.0, 0.0, 0.0],
            corners: [
                [-8.0, -8.0, 8.0],
                [-8.0, -8.0, -8.0],
                [-8.0, 8.0, -8.0],
                [-8.0, 8.0, 8.0],
            ],
        },
    ];

    let mut face_draws: Vec<(f32, [[f32; 2]; 4], [[f32; 2]; 4], [f32; 3])> = Vec::new();

    for face in &faces {
        let normal = rotate(face.normal, sx, cx_r, sy, cy_r);
        if !front_facing(normal) {
            continue;
        }
        let rect = tile_uv_rect(face.tile);
        let [u_min, v_min, u_max, v_max] = rect;

        let mut pts = [[0.0f32; 2]; 4];
        let mut depth = 0.0f32;
        for (i, &v) in face.corners.iter().enumerate() {
            let [px, py, pz] = v;
            let rx = px as f32 * cy_r + pz as f32 * sy;
            let rz1 = -px as f32 * sy + pz as f32 * cy_r;
            let ry = py as f32 * cx_r - rz1 * sx;
            let rz = py as f32 * sx + rz1 * cx_r;
            pts[i] = [cx + rx * scale, cy - ry * scale];
            depth += rz;
        }
        depth /= 4.0;

        let uvs = [
            [u_min, v_min],
            [u_max, v_min],
            [u_max, v_max],
            [u_min, v_max],
        ];
        let material = crate::world::material::fallback_face_material(block, face.normal);
        let tint = crate::world::material::material_color(material);
        let shade = standard_item_light(normal);
        face_draws.push((
            depth,
            pts,
            uvs,
            [tint[0] * shade, tint[1] * shade, tint[2] * shade],
        ));
    }

    face_draws.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    for (_depth, pts, uvs, color) in &face_draws {
        gui.textured_quad(
            [pts[0], pts[1], pts[2], pts[3]],
            *uvs,
            [color[0], color[1], color[2], 1.0],
        );
    }
}

fn rotate(v: [f32; 3], sx: f32, cx: f32, sy: f32, cy: f32) -> [f32; 3] {
    let rx = v[0] * cy + v[2] * sy;
    let rz1 = -v[0] * sy + v[2] * cy;
    [rx, v[1] * cx - rz1 * sx, v[1] * sx + rz1 * cx]
}

fn front_facing(normal: [f32; 3]) -> bool {
    normal[2] < -0.001
}

/// Vanilla's standard item lighting: ambient 0.4 plus two 0.6 diffuse lights.
fn standard_item_light(normal: [f32; 3]) -> f32 {
    const LIGHT_0: [f32; 3] = [0.16169041, 0.80845207, -0.56591645];
    const LIGHT_1: [f32; 3] = [-0.16169041, 0.80845207, 0.56591645];
    let dot = |light: [f32; 3]| {
        (normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2]).max(0.0)
    };
    (0.4 + 0.6 * dot(LIGHT_0) + 0.6 * dot(LIGHT_1)).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_rotation_keeps_only_three_cube_faces() {
        let (sx, cx) = (-30.0f32).to_radians().sin_cos();
        let (sy, cy) = 45.0f32.to_radians().sin_cos();
        let normals = [
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
        ];
        assert_eq!(
            normals
                .into_iter()
                .filter(|normal| front_facing(rotate(*normal, sx, cx, sy, cy)))
                .count(),
            3
        );
    }
}
