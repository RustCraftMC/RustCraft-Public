use crate::assets::texture::tex_idx;
use crate::assets::texture::tile_uv as atlas_tile_uv;
use crate::world::block::Block;
use crate::world::chunk::{Chunk, CHUNK_HEIGHT, CHUNK_SIZE};
use crate::world::light::LightLevel;

use super::fluid::corner_fluid_height;
use super::lighting::{face_light, smooth_vertex_light};
use super::types::{ChunkMesh, MeshOptions, Vertex};

fn tile_uv(tile: usize) -> ([f32; 2], [f32; 2]) {
    atlas_tile_uv(tile)
}

fn auto_element_uv(
    from: [f32; 3],
    to: [f32; 3],
    face_idx: usize,
    tile_uv0: [f32; 2],
    tile_uv1: [f32; 2],
) -> [[f32; 2]; 4] {
    // V coordinate: element bottom (from[1]) → texture bottom (1.0),
    //               element top (to[1]) → texture top (0.0)
    let v_bottom = (16.0 - from[1]) / 16.0;
    let v_top = (16.0 - to[1]) / 16.0;
    let f: [[f32; 2]; 4] = match face_idx {
        0 => [
            [from[0] / 16.0, to[2] / 16.0],
            [to[0] / 16.0, to[2] / 16.0],
            [to[0] / 16.0, from[2] / 16.0],
            [from[0] / 16.0, from[2] / 16.0],
        ],
        1 => [
            [from[0] / 16.0, from[2] / 16.0],
            [to[0] / 16.0, from[2] / 16.0],
            [to[0] / 16.0, to[2] / 16.0],
            [from[0] / 16.0, to[2] / 16.0],
        ],
        2 => [
            [to[0] / 16.0, v_bottom],
            [from[0] / 16.0, v_bottom],
            [from[0] / 16.0, v_top],
            [to[0] / 16.0, v_top],
        ],
        3 => [
            [from[0] / 16.0, v_bottom],
            [to[0] / 16.0, v_bottom],
            [to[0] / 16.0, v_top],
            [from[0] / 16.0, v_top],
        ],
        4 => [
            [from[2] / 16.0, v_bottom],
            [to[2] / 16.0, v_bottom],
            [to[2] / 16.0, v_top],
            [from[2] / 16.0, v_top],
        ],
        _ => [
            [to[2] / 16.0, v_bottom],
            [from[2] / 16.0, v_bottom],
            [from[2] / 16.0, v_top],
            [to[2] / 16.0, v_top],
        ],
    };
    f.map(|[u, v]| {
        [
            tile_uv0[0] + u * (tile_uv1[0] - tile_uv0[0]),
            tile_uv0[1] + v * (tile_uv1[1] - tile_uv0[1]),
        ]
    })
}

fn element_face_uvs(
    elem: &crate::world::shape::BlockElement,
    face_idx: usize,
    tile_idx: usize,
) -> [[f32; 2]; 4] {
    let (uv0, uv1) = tile_uv(tile_idx);
    if let Some(custom) = elem.uvs[face_idx] {
        custom.map(|[u, v]| {
            [
                uv0[0] + u * (uv1[0] - uv0[0]),
                uv0[1] + v * (uv1[1] - uv0[1]),
            ]
        })
    } else {
        auto_element_uv(elem.from, elem.to, face_idx, uv0, uv1)
    }
}

fn liquid_side_uvs(tile_idx: usize, h0: f32, h1: f32) -> [[f32; 2]; 4] {
    let (uv0, uv1) = tile_uv(tile_idx);
    let du = uv1[0] - uv0[0];
    let dv = uv1[1] - uv0[1];
    let u_mid = uv0[0] + du * 0.5;
    let v_mid = uv0[1] + dv * 0.5;
    let v_top0 = uv0[1] + (1.0 - h0) * dv * 0.5;
    let v_top1 = uv0[1] + (1.0 - h1) * dv * 0.5;
    [
        [uv0[0], v_mid],
        [u_mid, v_mid],
        [u_mid, v_top1],
        [uv0[0], v_top0],
    ]
}

/// Build mesh for one chunk. Vertices are in LOCAL chunk space.

/// Inset face UVs by 1 texel on edges where the in-plane neighbour is the
/// same glass-like block.  This removes the texture-border grid that vanilla
/// leaves between connected transparent blocks.
fn connected_glass_uvs(
    fuvs: &[[f32; 2]; 4],
    vertices: &[[f32; 3]; 4],
    face_dir: crate::assets::model::FaceDir,
    wx: i32,
    wy: i32,
    wz: i32,
    block: Block,
    world_get: impl Fn(i32, i32, i32) -> Block,
) -> [[f32; 2]; 4] {
    use crate::assets::model::FaceDir;
    // Model UVs are normalized to a 16x16 vanilla texture tile, so one source
    // texel is 1/16 rather than a full UV unit.
    let inset = 1.0 / 16.0;
    let mut result = *fuvs;

    let neighbor = |dx, dy, dz| {
        let nb = world_get(wx + dx, wy + dy, wz + dz);
        if block == Block::Glass {
            nb == Block::Glass
        } else if block == Block::StainedGlass {
            nb == Block::StainedGlass
        } else {
            nb == block
        }
    };
    // JSON model faces can rotate their UVs independently, so identify the
    // shared geometric edge first and then inset its constant UV coordinate.
    let mut inset_edge = |axis: usize, minimum: bool| {
        let boundary = vertices
            .iter()
            .map(|vertex| vertex[axis])
            .reduce(if minimum { f32::min } else { f32::max })
            .expect("glass face has four vertices");
        let indices: Vec<_> = vertices
            .iter()
            .enumerate()
            .filter_map(|(index, vertex)| {
                ((vertex[axis] - boundary).abs() < f32::EPSILON).then_some(index)
            })
            .collect();
        if indices.len() != 2 {
            return;
        }

        for uv_axis in 0..2 {
            let coordinate = fuvs[indices[0]][uv_axis];
            if (fuvs[indices[1]][uv_axis] - coordinate).abs() >= f32::EPSILON {
                continue;
            }
            let min_uv = fuvs
                .iter()
                .map(|uv| uv[uv_axis])
                .reduce(f32::min)
                .expect("glass face has four UVs");
            let max_uv = fuvs
                .iter()
                .map(|uv| uv[uv_axis])
                .reduce(f32::max)
                .expect("glass face has four UVs");
            let adjustment = if (coordinate - min_uv).abs() < f32::EPSILON {
                inset
            } else if (coordinate - max_uv).abs() < f32::EPSILON {
                -inset
            } else {
                continue;
            };
            for index in indices {
                result[index][uv_axis] += adjustment;
            }
            break;
        }
    };

    match face_dir {
        FaceDir::Up | FaceDir::Down => {
            if neighbor(-1, 0, 0) {
                inset_edge(0, true);
            }
            if neighbor(1, 0, 0) {
                inset_edge(0, false);
            }
            if neighbor(0, 0, -1) {
                inset_edge(2, true);
            }
            if neighbor(0, 0, 1) {
                inset_edge(2, false);
            }
        }
        FaceDir::North | FaceDir::South => {
            if neighbor(-1, 0, 0) {
                inset_edge(0, true);
            }
            if neighbor(1, 0, 0) {
                inset_edge(0, false);
            }
            if neighbor(0, -1, 0) {
                inset_edge(1, true);
            }
            if neighbor(0, 1, 0) {
                inset_edge(1, false);
            }
        }
        FaceDir::West | FaceDir::East => {
            if neighbor(0, 0, -1) {
                inset_edge(2, true);
            }
            if neighbor(0, 0, 1) {
                inset_edge(2, false);
            }
            if neighbor(0, -1, 0) {
                inset_edge(1, true);
            }
            if neighbor(0, 1, 0) {
                inset_edge(1, false);
            }
        }
    }
    result
}

pub fn build_chunk_mesh(
    chunk: &Chunk,
    world_get: impl Fn(i32, i32, i32) -> Block + Copy + Sync,
    light_get: impl Fn(i32, i32, i32) -> LightLevel + Copy + Sync,
    state_get: impl Fn(i32, i32, i32) -> u16 + Copy + Sync,
    options: MeshOptions,
) -> ChunkMesh {
    let mut vertices = Vec::new();
    let mut opaque_indices = Vec::new();
    let mut transparent_indices = Vec::new();

    let base_x = chunk.cx * CHUNK_SIZE as i32;
    let base_z = chunk.cz * CHUNK_SIZE as i32;
    let model_cache = if crate::world::block_models::BlockModelCache::is_available() {
        Some(crate::world::block_models::BlockModelCache::global())
    } else {
        None
    };

    for lx in 0..CHUNK_SIZE {
        for lz in 0..CHUNK_SIZE {
            for y in 0..CHUNK_HEIGHT {
                let block_state = chunk.state(lx, y, lz);
                let block = Block::from_state(block_state);
                let block_id = block_state >> 4;
                let block_meta = (block_state & 0x0f) as u8;
                // Allow non-solid blocks with custom shapes (flowers, torches, etc.)
                // OR blocks that have a JSON block model (tall grass, ferns, etc.).
                if !block.is_solid()
                    && !block.is_liquid()
                    && !crate::world::shape::has_custom_shape(block)
                    && !crate::world::block_models::has_json_model(block_id)
                {
                    continue;
                }

                // BlockSkull has no baked block model in vanilla. Its complete
                // geometry and texture are supplied by TileEntitySkullRenderer;
                // emitting the fallback shape here creates a stone placeholder.
                // BlockPistonMoving (id 36) is the same class of placeholder:
                // vanilla draws the stored block via TileEntityPistonRenderer.
                if matches!(block, Block::Skull | Block::PistonExtension) {
                    continue;
                }

                // Render layer is a block property, not a shader material.
                // Clear glass writes depth; stained glass and liquids blend.
                let indices = if crate::world::material::uses_translucent_layer(block) {
                    &mut transparent_indices
                } else {
                    &mut opaque_indices
                };

                let wx = base_x + lx as i32;
                let wz = base_z + lz as i32;

                let fx = lx as f32;
                let fy = y as f32;
                let fz = lz as f32;
                let vertex_light = |p: [f32; 3], n: [f32; 3]| -> (f32, f32, f32) {
                    if options.smooth_lighting {
                        smooth_vertex_light(
                            p,
                            n,
                            base_x,
                            base_z,
                            [wx, y as i32, wz],
                            world_get,
                            light_get,
                        )
                    } else {
                        face_light(p, n, base_x, base_z, [wx, y as i32, wz], light_get)
                    }
                };

                macro_rules! quad {
                    ($tile:expr, $n:expr, $p0:expr, $p1:expr, $p2:expr, $p3:expr) => {{
                        quad!($tile, $n, $p0, $p1, $p2, $p3, 0.0);
                    }};
                    ($tile:expr, $n:expr, $p0:expr, $p1:expr, $p2:expr, $p3:expr, $bt:expr) => {{
                        let (uv0, uv1) = tile_uv($tile);
                        let base = vertices.len() as u32;
                        let n = $n;
                        let bt = $bt;
                        let (sl0, bl0, ao0) = vertex_light($p0, n);
                        let (sl1, bl1, ao1) = vertex_light($p1, n);
                        let (sl2, bl2, ao2) = vertex_light($p2, n);
                        let (sl3, bl3, ao3) = vertex_light($p3, n);
                        vertices.push(Vertex {
                            pos: $p0,
                            normal: n,
                            uv: [uv0[0], uv1[1]],
                            block_type: bt,
                            sky_light: sl0,
                            block_light: bl0,
                            ambient_occlusion: ao0,
                        });
                        vertices.push(Vertex {
                            pos: $p1,
                            normal: n,
                            uv: [uv1[0], uv1[1]],
                            block_type: bt,
                            sky_light: sl1,
                            block_light: bl1,
                            ambient_occlusion: ao1,
                        });
                        vertices.push(Vertex {
                            pos: $p2,
                            normal: n,
                            uv: [uv1[0], uv0[1]],
                            block_type: bt,
                            sky_light: sl2,
                            block_light: bl2,
                            ambient_occlusion: ao2,
                        });
                        vertices.push(Vertex {
                            pos: $p3,
                            normal: n,
                            uv: [uv0[0], uv0[1]],
                            block_type: bt,
                            sky_light: sl3,
                            block_light: bl3,
                            ambient_occlusion: ao3,
                        });
                        indices.extend_from_slice(&[
                            base,
                            base + 1,
                            base + 2,
                            base,
                            base + 2,
                            base + 3,
                        ]);
                    }};
                    ($n:expr, $p0:expr, $p1:expr, $p2:expr, $p3:expr, $bt:expr, $uv0:expr, $uv1:expr, $uv2:expr, $uv3:expr) => {{
                        let base = vertices.len() as u32;
                        let n = $n;
                        let bt = $bt;
                        let (sl0, bl0, ao0) = vertex_light($p0, n);
                        let (sl1, bl1, ao1) = vertex_light($p1, n);
                        let (sl2, bl2, ao2) = vertex_light($p2, n);
                        let (sl3, bl3, ao3) = vertex_light($p3, n);
                        vertices.push(Vertex {
                            pos: $p0,
                            normal: n,
                            uv: $uv0,
                            block_type: bt,
                            sky_light: sl0,
                            block_light: bl0,
                            ambient_occlusion: ao0,
                        });
                        vertices.push(Vertex {
                            pos: $p1,
                            normal: n,
                            uv: $uv1,
                            block_type: bt,
                            sky_light: sl1,
                            block_light: bl1,
                            ambient_occlusion: ao1,
                        });
                        vertices.push(Vertex {
                            pos: $p2,
                            normal: n,
                            uv: $uv2,
                            block_type: bt,
                            sky_light: sl2,
                            block_light: bl2,
                            ambient_occlusion: ao2,
                        });
                        vertices.push(Vertex {
                            pos: $p3,
                            normal: n,
                            uv: $uv3,
                            block_type: bt,
                            sky_light: sl3,
                            block_light: bl3,
                            ambient_occlusion: ao3,
                        });
                        indices.extend_from_slice(&[
                            base,
                            base + 1,
                            base + 2,
                            base,
                            base + 2,
                            base + 3,
                        ]);
                    }};
                }

                // Render using JSON block model system
                if model_cache.as_ref().is_some()
                    && !block.is_liquid()
                    && block != Block::Fire
                    && block != Block::Chest
                    && block != Block::TrappedChest
                    && block != Block::EnderChest
                    // Vine attachments are derived from metadata and the block above.
                    // Its JSON model cannot express the dynamic UP property.
                    && block != Block::Vine
                {
                    let cache = model_cache.as_ref().expect("model cache availability checked above");
                    if block_id != 0
                        && !crate::world::block_models::is_connection_state_block(block_id)
                    {
                        if let Some(model) = cache
                            .get_model(block_id, block_meta)
                            .filter(|model| !model.faces.is_empty())
                        {
                            for face in &model.faces {
                                // Match Block.shouldSideBeRendered, including the state-sensitive
                                // rules used by glass and stained glass in BlockBreakable.
                                if let Some(cull_dir) = face.cullface {
                                    let off = match cull_dir {
                                        crate::assets::model::FaceDir::Down => (0, -1, 0),
                                        crate::assets::model::FaceDir::Up => (0, 1, 0),
                                        crate::assets::model::FaceDir::North => (0, 0, -1),
                                        crate::assets::model::FaceDir::South => (0, 0, 1),
                                        crate::assets::model::FaceDir::West => (-1, 0, 0),
                                        crate::assets::model::FaceDir::East => (1, 0, 0),
                                    };
                                    if !face_visible_with_state(
                                        wx + off.0,
                                        y as i32 + off.1,
                                        wz + off.2,
                                        block,
                                        block_state,
                                        false,
                                        &state_get,
                                    ) {
                                        continue;
                                    }
                                }

                                use crate::assets::model::FaceDir;

                                let tile_idx = if options.better_grass
                                    && (block == Block::Grass || block == Block::GrassSnowy)
                                    && matches!(
                                        face.cullface,
                                        Some(FaceDir::North)
                                            | Some(FaceDir::South)
                                            | Some(FaceDir::East)
                                            | Some(FaceDir::West)
                                    ) {
                                    // Replace side texture with grass_top for green sides
                                    tex_idx("grass_top")
                                } else {
                                    cache.texture_index(&face.texture)
                                };
                                let (uv0, uv1) = tile_uv(tile_idx);
                                let mut fuvs = face.uvs;

                                // Connected Textures: remove the 1-pixel glass border
                                // on edges where the in-plane neighbour is the same block.
                                let is_glassy = matches!(block, Block::Glass | Block::StainedGlass);
                                if options.connected_textures
                                    && is_glassy
                                    && !matches!(block, Block::GlassPane | Block::StainedGlassPane)
                                {
                                    if let Some(dir) = face.cullface {
                                        fuvs = connected_glass_uvs(
                                            &fuvs,
                                            &face.vertices,
                                            dir,
                                            wx,
                                            y as i32,
                                            wz,
                                            block,
                                            &world_get,
                                        );
                                    }
                                }

                                let auvs = [
                                    [
                                        uv0[0] + fuvs[0][0] * (uv1[0] - uv0[0]),
                                        uv0[1] + fuvs[0][1] * (uv1[1] - uv0[1]),
                                    ],
                                    [
                                        uv0[0] + fuvs[1][0] * (uv1[0] - uv0[0]),
                                        uv0[1] + fuvs[1][1] * (uv1[1] - uv0[1]),
                                    ],
                                    [
                                        uv0[0] + fuvs[2][0] * (uv1[0] - uv0[0]),
                                        uv0[1] + fuvs[2][1] * (uv1[1] - uv0[1]),
                                    ],
                                    [
                                        uv0[0] + fuvs[3][0] * (uv1[0] - uv0[0]),
                                        uv0[1] + fuvs[3][1] * (uv1[1] - uv0[1]),
                                    ],
                                ];

                                let bt = if options.better_grass
                                    && (block == Block::Grass || block == Block::GrassSnowy)
                                    && matches!(
                                        face.cullface,
                                        Some(FaceDir::North)
                                            | Some(FaceDir::South)
                                            | Some(FaceDir::East)
                                            | Some(FaceDir::West)
                                    ) {
                                    crate::world::material::MATERIAL_GRASS
                                } else {
                                    crate::world::material::model_material(block, face.tintindex)
                                };

                                let p = face.vertices;
                                let verts = [
                                    [
                                        fx + p[0][0] / 16.0,
                                        fy + p[0][1] / 16.0,
                                        fz + p[0][2] / 16.0,
                                    ],
                                    [
                                        fx + p[1][0] / 16.0,
                                        fy + p[1][1] / 16.0,
                                        fz + p[1][2] / 16.0,
                                    ],
                                    [
                                        fx + p[2][0] / 16.0,
                                        fy + p[2][1] / 16.0,
                                        fz + p[2][2] / 16.0,
                                    ],
                                    [
                                        fx + p[3][0] / 16.0,
                                        fy + p[3][1] / 16.0,
                                        fz + p[3][2] / 16.0,
                                    ],
                                ];

                                let n = face.normal;
                                let (sl0, bl0, ao0) = vertex_light(verts[0], n);
                                let (sl1, bl1, ao1) = vertex_light(verts[1], n);
                                let (sl2, bl2, ao2) = vertex_light(verts[2], n);
                                let (sl3, bl3, ao3) = vertex_light(verts[3], n);

                                let base = vertices.len() as u32;
                                vertices.push(Vertex {
                                    pos: verts[0],
                                    normal: n,
                                    uv: auvs[0],
                                    block_type: bt,
                                    sky_light: sl0,
                                    block_light: bl0,
                                    ambient_occlusion: ao0,
                                });
                                vertices.push(Vertex {
                                    pos: verts[1],
                                    normal: n,
                                    uv: auvs[1],
                                    block_type: bt,
                                    sky_light: sl1,
                                    block_light: bl1,
                                    ambient_occlusion: ao1,
                                });
                                vertices.push(Vertex {
                                    pos: verts[2],
                                    normal: n,
                                    uv: auvs[2],
                                    block_type: bt,
                                    sky_light: sl2,
                                    block_light: bl2,
                                    ambient_occlusion: ao2,
                                });
                                vertices.push(Vertex {
                                    pos: verts[3],
                                    normal: n,
                                    uv: auvs[3],
                                    block_type: bt,
                                    sky_light: sl3,
                                    block_light: bl3,
                                    ambient_occlusion: ao3,
                                });
                                indices.extend_from_slice(&[
                                    base,
                                    base + 1,
                                    base + 2,
                                    base,
                                    base + 2,
                                    base + 3,
                                ]);
                            }
                            continue;
                        }
                    }
                }

                // Old shape system for full blocks and blocks without JSON models
                if let Some(elements) = crate::world::shape::block_elements(
                    block,
                    block_state,
                    lx,
                    y,
                    lz,
                    |dx, dy, dz| {
                        world_get(
                            base_x + dx,
                            dy.clamp(0, CHUNK_HEIGHT as i32 - 1),
                            base_z + dz,
                        )
                    },
                    |dx, dy, dz| {
                        state_get(
                            base_x + dx,
                            dy.clamp(0, CHUNK_HEIGHT as i32 - 1),
                            base_z + dz,
                        )
                    },
                ) {
                    // Redstone wire is tinted from its 0..15 metadata power
                    // level, using a dedicated shader encoding (8.0..9.0).
                    let element_bt = if block == Block::RedstoneWire {
                        8.0 + block_meta as f32 / 15.0
                    } else if matches!(block, Block::Vine | Block::TallGrass) {
                        crate::world::material::model_material(block, Some(0))
                    } else {
                        0.0
                    };
                    for elem in elements {
                        let (sin_x, cos_x) = elem.rotation_x.to_radians().sin_cos();
                        let (sin_y, cos_y) = elem.rotation_y.to_radians().sin_cos();
                        let (sin_z, cos_z) = elem.rotation_z.to_radians().sin_cos();
                        let rotate_position = |p: [f32; 3]| {
                            let (mut x, mut y, mut z) =
                                (p[0] - fx - 0.5, p[1] - fy - 0.5, p[2] - fz - 0.5);
                            (y, z) = (y * cos_x - z * sin_x, y * sin_x + z * cos_x);
                            (x, z) = (x * cos_y - z * sin_y, x * sin_y + z * cos_y);
                            (x, y) = (x * cos_z - y * sin_z, x * sin_z + y * cos_z);
                            [fx + 0.5 + x, fy + 0.5 + y, fz + 0.5 + z]
                        };
                        let rotate_normal = |n: [f32; 3]| {
                            let (mut x, mut y, mut z) = (n[0], n[1], n[2]);
                            (y, z) = (y * cos_x - z * sin_x, y * sin_x + z * cos_x);
                            (x, z) = (x * cos_y - z * sin_y, x * sin_y + z * cos_y);
                            (x, y) = (x * cos_z - y * sin_z, x * sin_z + y * cos_z);
                            [x, y, z]
                        };
                        macro_rules! element_quad {
                            ($n:expr, $p0:expr, $p1:expr, $p2:expr, $p3:expr, $bt:expr, $uv0:expr, $uv1:expr, $uv2:expr, $uv3:expr) => {
                                quad!(
                                    rotate_normal($n),
                                    rotate_position($p0),
                                    rotate_position($p1),
                                    rotate_position($p2),
                                    rotate_position($p3),
                                    $bt,
                                    $uv0,
                                    $uv1,
                                    $uv2,
                                    $uv3
                                )
                            };
                        }
                        let e_tile = (elem.tile_top, elem.tile_bottom, elem.tile_side);
                        let from = [
                            fx + elem.from[0] / 16.0,
                            fy + elem.from[1] / 16.0,
                            fz + elem.from[2] / 16.0,
                        ];
                        let to = [
                            fx + elem.to[0] / 16.0,
                            fy + elem.to[1] / 16.0,
                            fz + elem.to[2] / 16.0,
                        ];

                        // +Y (check boundary)
                        if elem.visible_faces[0]
                            && (elem.to[1] != 16.0
                                || face_visible_with_state(
                                    wx,
                                    y as i32 + 1,
                                    wz,
                                    block,
                                    block_state,
                                    false,
                                    &state_get,
                                ))
                        {
                            let uvs = element_face_uvs(&elem, 0, elem.face_tile(0, e_tile.0));
                            element_quad!(
                                [0.0, 1.0, 0.0],
                                [from[0], to[1], to[2]],
                                [to[0], to[1], to[2]],
                                [to[0], to[1], from[2]],
                                [from[0], to[1], from[2]],
                                element_bt,
                                uvs[0],
                                uvs[1],
                                uvs[2],
                                uvs[3]
                            );
                        }
                        // -Y (check boundary)
                        if elem.visible_faces[1]
                            && (elem.from[1] != 0.0
                                || face_visible_with_state(
                                    wx,
                                    y as i32 - 1,
                                    wz,
                                    block,
                                    block_state,
                                    false,
                                    &state_get,
                                ))
                        {
                            let uvs = element_face_uvs(&elem, 1, elem.face_tile(1, e_tile.1));
                            element_quad!(
                                [0.0, -1.0, 0.0],
                                [from[0], from[1], from[2]],
                                [to[0], from[1], from[2]],
                                [to[0], from[1], to[2]],
                                [from[0], from[1], to[2]],
                                element_bt,
                                uvs[0],
                                uvs[1],
                                uvs[2],
                                uvs[3]
                            );
                        }
                        // +Z (check boundary)
                        if elem.visible_faces[3]
                            && (elem.to[2] != 16.0
                                || face_visible_with_state(
                                    wx,
                                    y as i32,
                                    wz + 1,
                                    block,
                                    block_state,
                                    false,
                                    &state_get,
                                ))
                        {
                            let uvs = element_face_uvs(&elem, 3, elem.face_tile(3, e_tile.2));
                            element_quad!(
                                [0.0, 0.0, 1.0],
                                [from[0], from[1], to[2]],
                                [to[0], from[1], to[2]],
                                [to[0], to[1], to[2]],
                                [from[0], to[1], to[2]],
                                element_bt,
                                uvs[0],
                                uvs[1],
                                uvs[2],
                                uvs[3]
                            );
                        }
                        // -Z (check boundary)
                        if elem.visible_faces[2]
                            && (elem.from[2] != 0.0
                                || face_visible_with_state(
                                    wx,
                                    y as i32,
                                    wz - 1,
                                    block,
                                    block_state,
                                    false,
                                    &state_get,
                                ))
                        {
                            let uvs = element_face_uvs(&elem, 2, elem.face_tile(2, e_tile.2));
                            element_quad!(
                                [0.0, 0.0, -1.0],
                                [to[0], from[1], from[2]],
                                [from[0], from[1], from[2]],
                                [from[0], to[1], from[2]],
                                [to[0], to[1], from[2]],
                                element_bt,
                                uvs[0],
                                uvs[1],
                                uvs[2],
                                uvs[3]
                            );
                        }
                        // +X (check boundary)
                        if elem.visible_faces[5]
                            && (elem.to[0] != 16.0
                                || face_visible_with_state(
                                    wx + 1,
                                    y as i32,
                                    wz,
                                    block,
                                    block_state,
                                    false,
                                    &state_get,
                                ))
                        {
                            let uvs = element_face_uvs(&elem, 5, elem.face_tile(5, e_tile.2));
                            element_quad!(
                                [1.0, 0.0, 0.0],
                                [to[0], from[1], to[2]],
                                [to[0], from[1], from[2]],
                                [to[0], to[1], from[2]],
                                [to[0], to[1], to[2]],
                                element_bt,
                                uvs[0],
                                uvs[1],
                                uvs[2],
                                uvs[3]
                            );
                        }
                        // -X (check boundary)
                        if elem.visible_faces[4]
                            && (elem.from[0] != 0.0
                                || face_visible_with_state(
                                    wx - 1,
                                    y as i32,
                                    wz,
                                    block,
                                    block_state,
                                    false,
                                    &state_get,
                                ))
                        {
                            let uvs = element_face_uvs(&elem, 4, elem.face_tile(4, e_tile.2));
                            element_quad!(
                                [-1.0, 0.0, 0.0],
                                [from[0], from[1], from[2]],
                                [from[0], from[1], to[2]],
                                [from[0], to[1], to[2]],
                                [from[0], to[1], from[2]],
                                element_bt,
                                uvs[0],
                                uvs[1],
                                uvs[2],
                                uvs[3]
                            );
                        }
                    }
                } else {
                    // Texture-table lookups take a global lock, so only do
                    // them after both model systems have missed.
                    let (top_t, bot_t, mut side_t) = block.tiles();
                    // Better Grass: grass block sides use the top texture with
                    // biome tint, matching OptiFine's Better Grass option.
                    let is_grass = block == Block::Grass || block == Block::GrassSnowy;
                    if options.better_grass && is_grass {
                        side_t = tex_idx("grass_top");
                    }
                    // Grass top + leaves get biome tint, water gets blue tint (MC 1.8.9)
                    let is_leaves =
                        matches!(block, Block::Leaves | Block::Leaves2 | Block::Leaves3);
                    let is_water = block.is_liquid()
                        && matches!(block, Block::FlowingWater | Block::StillWater);
                    let is_glass = matches!(
                        block,
                        Block::Glass | Block::GlassPane | Block::StainedGlassPane
                    );
                    // block_type encoding: 0=normal, 1=grass_top, 2=grass_side, 3=leaves, 4=water, 5=glass
                    let grass_top_bt = if is_grass {
                        1.0
                    } else if is_leaves {
                        3.0
                    } else if is_water {
                        4.0
                    } else if is_glass {
                        5.0
                    } else {
                        0.0
                    };
                    let leaf_bt = if is_leaves {
                        3.0
                    } else if is_water {
                        4.0
                    } else if is_glass {
                        5.0
                    } else {
                        0.0
                    };
                    // Better Grass: side faces of grass blocks get grass tint
                    let side_bt = if options.better_grass && is_grass {
                        grass_top_bt
                    } else {
                        leaf_bt
                    };

                    // Standard full-block face generation
                    let is_liquid = block.is_liquid();

                    if is_liquid {
                        // --- Liquid rendering with variable surface height ---
                        // Get smooth corner heights for the top surface
                        // Corner order: (0,0), (0,1), (1,1), (1,0) matching quad winding
                        let h00 =
                            corner_fluid_height(block, wx, y as i32, wz, &world_get, &state_get);
                        let h01 = corner_fluid_height(
                            block,
                            wx,
                            y as i32,
                            wz + 1,
                            &world_get,
                            &state_get,
                        );
                        let h11 = corner_fluid_height(
                            block,
                            wx + 1,
                            y as i32,
                            wz + 1,
                            &world_get,
                            &state_get,
                        );
                        let h10 = corner_fluid_height(
                            block,
                            wx + 1,
                            y as i32,
                            wz,
                            &world_get,
                            &state_get,
                        );

                        // +Y (top). Lower the visible surface once, then reuse
                        // the adjusted heights for both the top and side faces.
                        let eps = 0.001;
                        let top_visible =
                            face_visible(wx, y as i32 + 1, wz, block, true, &world_get);
                        let (h00, h01, h11, h10) = if top_visible {
                            (h00 - eps, h01 - eps, h11 - eps, h10 - eps)
                        } else {
                            (h00, h01, h11, h10)
                        };
                        if top_visible {
                            quad!(
                                top_t,
                                [0.0, 1.0, 0.0],
                                [fx, fy + h01, fz + 1.0],
                                [fx + 1.0, fy + h11, fz + 1.0],
                                [fx + 1.0, fy + h10, fz],
                                [fx, fy + h00, fz],
                                grass_top_bt
                            );
                        }

                        // -Y (bottom) stays exactly on the block boundary.
                        if liquid_side_visible(block, world_get(wx, y as i32 - 1, wz)) {
                            quad!(
                                bot_t,
                                [0.0, -1.0, 0.0],
                                [fx, fy, fz],
                                [fx + 1.0, fy, fz],
                                [fx + 1.0, fy, fz + 1.0],
                                [fx, fy, fz + 1.0],
                                leaf_bt
                            );
                        }

                        // Side faces are inset horizontally like BlockFluidRenderer.
                        let sby = fy;
                        // +Z side
                        let neighbor_z = world_get(wx, y as i32, wz + 1);
                        if liquid_side_visible(block, neighbor_z) {
                            let side_h0 = h01;
                            let side_h1 = h11;
                            let side_z = fz + 1.0 - eps;
                            if side_h0 > 0.001 || side_h1 > 0.001 {
                                let uvs = liquid_side_uvs(side_t, side_h0, side_h1);
                                quad!(
                                    [0.0, 0.0, 1.0],
                                    [fx, sby, side_z],
                                    [fx + 1.0, sby, side_z],
                                    [fx + 1.0, fy + side_h1, side_z],
                                    [fx, fy + side_h0, side_z],
                                    leaf_bt,
                                    uvs[0],
                                    uvs[1],
                                    uvs[2],
                                    uvs[3]
                                );
                            }
                        }

                        // -Z side
                        let neighbor_nz = world_get(wx, y as i32, wz - 1);
                        if liquid_side_visible(block, neighbor_nz) {
                            let side_h0 = h10;
                            let side_h1 = h00;
                            let side_z = fz + eps;
                            if side_h0 > 0.001 || side_h1 > 0.001 {
                                let uvs = liquid_side_uvs(side_t, side_h0, side_h1);
                                quad!(
                                    [0.0, 0.0, -1.0],
                                    [fx + 1.0, sby, side_z],
                                    [fx, sby, side_z],
                                    [fx, fy + side_h1, side_z],
                                    [fx + 1.0, fy + side_h0, side_z],
                                    leaf_bt,
                                    uvs[0],
                                    uvs[1],
                                    uvs[2],
                                    uvs[3]
                                );
                            }
                        }

                        // +X side
                        let neighbor_x = world_get(wx + 1, y as i32, wz);
                        if liquid_side_visible(block, neighbor_x) {
                            let side_h0 = h11;
                            let side_h1 = h10;
                            let side_x = fx + 1.0 - eps;
                            if side_h0 > 0.001 || side_h1 > 0.001 {
                                let uvs = liquid_side_uvs(side_t, side_h0, side_h1);
                                quad!(
                                    [1.0, 0.0, 0.0],
                                    [side_x, sby, fz + 1.0],
                                    [side_x, sby, fz],
                                    [side_x, fy + side_h1, fz],
                                    [side_x, fy + side_h0, fz + 1.0],
                                    leaf_bt,
                                    uvs[0],
                                    uvs[1],
                                    uvs[2],
                                    uvs[3]
                                );
                            }
                        }

                        // -X side
                        let neighbor_nx = world_get(wx - 1, y as i32, wz);
                        if liquid_side_visible(block, neighbor_nx) {
                            let side_h0 = h00;
                            let side_h1 = h01;
                            let side_x = fx + eps;
                            if side_h0 > 0.001 || side_h1 > 0.001 {
                                let uvs = liquid_side_uvs(side_t, side_h0, side_h1);
                                quad!(
                                    [-1.0, 0.0, 0.0],
                                    [side_x, sby, fz],
                                    [side_x, sby, fz + 1.0],
                                    [side_x, fy + side_h1, fz + 1.0],
                                    [side_x, fy + side_h0, fz],
                                    leaf_bt,
                                    uvs[0],
                                    uvs[1],
                                    uvs[2],
                                    uvs[3]
                                );
                            }
                        }
                    } else {
                        // --- Solid block face generation (unchanged) ---
                        // +Y (top)
                        if face_visible_with_state(
                            wx,
                            y as i32 + 1,
                            wz,
                            block,
                            block_state,
                            false,
                            &state_get,
                        ) {
                            quad!(
                                top_t,
                                [0.0, 1.0, 0.0],
                                [fx, fy + 1.0, fz + 1.0],
                                [fx + 1.0, fy + 1.0, fz + 1.0],
                                [fx + 1.0, fy + 1.0, fz],
                                [fx, fy + 1.0, fz],
                                grass_top_bt
                            );
                        }
                        // -Y (bottom)
                        if face_visible_with_state(
                            wx,
                            y as i32 - 1,
                            wz,
                            block,
                            block_state,
                            false,
                            &state_get,
                        ) {
                            quad!(
                                bot_t,
                                [0.0, -1.0, 0.0],
                                [fx, fy, fz],
                                [fx + 1.0, fy, fz],
                                [fx + 1.0, fy, fz + 1.0],
                                [fx, fy, fz + 1.0],
                                leaf_bt
                            );
                        }
                        // +Z
                        if face_visible_with_state(
                            wx,
                            y as i32,
                            wz + 1,
                            block,
                            block_state,
                            false,
                            &state_get,
                        ) {
                            quad!(
                                side_t,
                                [0.0, 0.0, 1.0],
                                [fx, fy, fz + 1.0],
                                [fx + 1.0, fy, fz + 1.0],
                                [fx + 1.0, fy + 1.0, fz + 1.0],
                                [fx, fy + 1.0, fz + 1.0],
                                side_bt
                            );
                        }
                        // -Z
                        if face_visible_with_state(
                            wx,
                            y as i32,
                            wz - 1,
                            block,
                            block_state,
                            false,
                            &state_get,
                        ) {
                            quad!(
                                side_t,
                                [0.0, 0.0, -1.0],
                                [fx + 1.0, fy, fz],
                                [fx, fy, fz],
                                [fx, fy + 1.0, fz],
                                [fx + 1.0, fy + 1.0, fz],
                                side_bt
                            );
                        }
                        // +X
                        if face_visible_with_state(
                            wx + 1,
                            y as i32,
                            wz,
                            block,
                            block_state,
                            false,
                            &state_get,
                        ) {
                            quad!(
                                side_t,
                                [1.0, 0.0, 0.0],
                                [fx + 1.0, fy, fz + 1.0],
                                [fx + 1.0, fy, fz],
                                [fx + 1.0, fy + 1.0, fz],
                                [fx + 1.0, fy + 1.0, fz + 1.0],
                                side_bt
                            );
                        }
                        // -X
                        if face_visible_with_state(
                            wx - 1,
                            y as i32,
                            wz,
                            block,
                            block_state,
                            false,
                            &state_get,
                        ) {
                            quad!(
                                side_t,
                                [-1.0, 0.0, 0.0],
                                [fx, fy, fz],
                                [fx, fy, fz + 1.0],
                                [fx, fy + 1.0, fz + 1.0],
                                [fx, fy + 1.0, fz],
                                side_bt
                            );
                        }
                    }
                }
            }
        }
    }

    let opaque_count = opaque_indices.len() as u32;
    opaque_indices.append(&mut transparent_indices);

    // Compute world-space AABB in the background thread so the render thread
    // can skip a full vertex scan during upload.  Vertex positions are still
    // chunk-local at this point; add the chunk origin to get world coords.
    let ox = (chunk.cx * CHUNK_SIZE as i32) as f32;
    let oz = (chunk.cz * CHUNK_SIZE as i32) as f32;
    let mut aabb_min = [f32::MAX; 3];
    let mut aabb_max = [f32::MIN; 3];
    for v in &vertices {
        aabb_min[0] = aabb_min[0].min(v.pos[0] + ox);
        aabb_max[0] = aabb_max[0].max(v.pos[0] + ox);
        aabb_min[1] = aabb_min[1].min(v.pos[1]);
        aabb_max[1] = aabb_max[1].max(v.pos[1]);
        aabb_min[2] = aabb_min[2].min(v.pos[2] + oz);
        aabb_max[2] = aabb_max[2].max(v.pos[2] + oz);
    }

    ChunkMesh {
        vertices,
        indices: opaque_indices,
        cx: chunk.cx,
        cz: chunk.cz,
        transparent_start: opaque_count,
        aabb_min,
        aabb_max,
    }
}

fn face_visible_with_state(
    bx: i32,
    by: i32,
    bz: i32,
    block: Block,
    block_state: u16,
    is_liquid: bool,
    state_get: impl Fn(i32, i32, i32) -> u16 + Copy,
) -> bool {
    let neighbor_state = state_get(bx, by, bz);
    face_visible_between(
        block,
        block_state,
        Block::from_state(neighbor_state),
        neighbor_state,
        is_liquid,
    )
}

/// Mirrors the relevant 1.8.9 `shouldSideBeRendered` implementations.
/// `BlockBreakable` compares complete states for full glass blocks, while
/// `BlockPane` compares only the block type (including stained pane colors).
fn face_visible_between(
    block: Block,
    block_state: u16,
    neighbor: Block,
    neighbor_state: u16,
    is_liquid: bool,
) -> bool {
    if is_liquid {
        return !same_fluid(block, neighbor);
    }

    // Vanilla: transparent blocks cull faces between same-type neighbours.
    if block == Block::Glass {
        return neighbor != Block::Glass && !neighbor.properties().is_opaque;
    }

    if block == Block::StainedGlass {
        if neighbor == Block::StainedGlass {
            return block_state != neighbor_state;
        }
        return !neighbor.properties().is_opaque;
    }

    if matches!(
        block,
        Block::GlassPane | Block::StainedGlassPane | Block::IronBars
    ) && block == neighbor
    {
        return false;
    }

    if matches!(block, Block::Ice | Block::SlimeBlock) && block == neighbor {
        return false;
    }

    !neighbor.properties().is_opaque
}

/// Legacy wrapper used by the fluid path, where metadata does not affect
/// whether two neighboring blocks belong to the same fluid.
fn face_visible(
    bx: i32,
    by: i32,
    bz: i32,
    block: Block,
    is_liquid: bool,
    world_get: impl Fn(i32, i32, i32) -> Block + Copy,
) -> bool {
    let neighbor = world_get(bx, by, bz);
    face_visible_between(
        block,
        block.to_id() << 4,
        neighbor,
        neighbor.to_id() << 4,
        is_liquid,
    )
}

fn same_fluid(a: Block, b: Block) -> bool {
    matches!(a, Block::FlowingWater | Block::StillWater)
        && matches!(b, Block::FlowingWater | Block::StillWater)
        || matches!(a, Block::FlowingLava | Block::StillLava)
            && matches!(b, Block::FlowingLava | Block::StillLava)
}

/// BlockLiquid delegates non-top faces to Block#shouldSideBeRendered: faces
/// shared with the same fluid or an opaque full cube are never visible.
fn liquid_side_visible(block: Block, neighbor: Block) -> bool {
    !same_fluid(block, neighbor) && !neighbor.properties().is_opaque
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(block: Block, metadata: u16) -> u16 {
        (block.to_id() << 4) | (metadata & 0x0f)
    }

    #[test]
    fn full_glass_culls_identical_states_and_coplanar_opaque_boundaries() {
        let glass = state(Block::Glass, 0);
        assert!(!face_visible_between(
            Block::Glass,
            glass,
            Block::Glass,
            glass,
            false,
        ));
        assert!(!face_visible_between(
            Block::Glass,
            state(Block::Glass, 7),
            Block::Glass,
            state(Block::Glass, 3),
            false,
        ));
        assert!(face_visible_between(
            Block::Glass,
            glass,
            Block::StainedGlass,
            state(Block::StainedGlass, 0),
            false,
        ));
        assert!(!face_visible_between(
            Block::Glass,
            glass,
            Block::Stone,
            state(Block::Stone, 0),
            false,
        ));
    }

    #[test]
    fn connected_glass_insets_only_the_shared_texture_edge() {
        use crate::assets::model::FaceDir;

        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ];
        let connected = connected_glass_uvs(
            &uvs,
            &vertices,
            FaceDir::Up,
            0,
            0,
            0,
            Block::Glass,
            |x, _, _| if x == -1 { Block::Glass } else { Block::Air },
        );

        assert_eq!(connected[0][0], 1.0 / 16.0);
        assert_eq!(connected[3][0], 1.0 / 16.0);
        assert_eq!(connected[1], uvs[1]);
        assert_eq!(connected[2], uvs[2]);

        let rotated_uvs = [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let rotated = connected_glass_uvs(
            &rotated_uvs,
            &vertices,
            FaceDir::Up,
            0,
            0,
            0,
            Block::Glass,
            |x, _, _| if x == -1 { Block::Glass } else { Block::Air },
        );
        assert_eq!(rotated[0][1], 1.0 - 1.0 / 16.0);
        assert_eq!(rotated[3][1], 1.0 - 1.0 / 16.0);
        assert_eq!(rotated[1], rotated_uvs[1]);
        assert_eq!(rotated[2], rotated_uvs[2]);
    }

    #[test]
    fn connected_glass_handles_every_face_orientation() {
        use crate::assets::model::FaceDir;

        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for (face, vertices) in [
            (
                FaceDir::Up,
                [
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 0.0, 1.0],
                    [0.0, 0.0, 1.0],
                ],
            ),
            (
                FaceDir::Down,
                [
                    [0.0, 1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
            ),
            (
                FaceDir::North,
                [
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                ],
            ),
            (
                FaceDir::South,
                [
                    [1.0, 0.0, 1.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 1.0, 1.0],
                    [1.0, 1.0, 1.0],
                ],
            ),
            (
                FaceDir::West,
                [
                    [0.0, 0.0, 1.0],
                    [0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 1.0],
                ],
            ),
            (
                FaceDir::East,
                [
                    [1.0, 0.0, 0.0],
                    [1.0, 0.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 1.0, 0.0],
                ],
            ),
        ] {
            let connected =
                connected_glass_uvs(&uvs, &vertices, face, 0, 0, 0, Block::Glass, |_, _, _| {
                    Block::Glass
                });
            assert_eq!(
                connected,
                [
                    [1.0 / 16.0, 1.0 / 16.0],
                    [15.0 / 16.0, 1.0 / 16.0],
                    [15.0 / 16.0, 15.0 / 16.0],
                    [1.0 / 16.0, 15.0 / 16.0],
                ],
                "{face:?}"
            );
        }
    }

    #[test]
    fn stained_glass_preserves_boundaries_between_colors() {
        let red = state(Block::StainedGlass, 14);
        let blue = state(Block::StainedGlass, 11);
        assert!(!face_visible_between(
            Block::StainedGlass,
            red,
            Block::StainedGlass,
            red,
            false,
        ));
        assert!(face_visible_between(
            Block::StainedGlass,
            red,
            Block::StainedGlass,
            blue,
            false,
        ));
        assert!(!face_visible_between(
            Block::StainedGlass,
            red,
            Block::Stone,
            state(Block::Stone, 0),
            false,
        ));
    }

    #[test]
    fn glass_panes_cull_same_block_even_when_stained_colors_differ() {
        assert!(!face_visible_between(
            Block::StainedGlassPane,
            state(Block::StainedGlassPane, 14),
            Block::StainedGlassPane,
            state(Block::StainedGlassPane, 11),
            false,
        ));
        assert!(face_visible_between(
            Block::StainedGlassPane,
            state(Block::StainedGlassPane, 14),
            Block::GlassPane,
            state(Block::GlassPane, 0),
            false,
        ));
        assert!(!face_visible_between(
            Block::IronBars,
            state(Block::IronBars, 0),
            Block::IronBars,
            state(Block::IronBars, 0),
            false,
        ));
    }

    #[test]
    fn liquid_sides_cull_same_fluid_and_opaque_blocks() {
        assert!(!liquid_side_visible(Block::StillWater, Block::FlowingWater));
        assert!(!liquid_side_visible(Block::StillWater, Block::Stone));
        assert!(liquid_side_visible(Block::StillWater, Block::Air));
        assert!(liquid_side_visible(Block::StillWater, Block::Glass));
        assert!(liquid_side_visible(Block::StillWater, Block::OakStairs));
        for block in [
            Block::Beacon,
            Block::SlimeBlock,
            Block::Barrier,
            Block::Chest,
            Block::TrappedChest,
            Block::EnderChest,
            Block::Piston,
            Block::StickyPiston,
        ] {
            assert!(liquid_side_visible(Block::StillWater, block), "{block:?}");
        }
        for block in [
            Block::DoubleStoneSlab,
            Block::DoubleStoneSlab2,
            Block::DoubleWoodSlab,
        ] {
            assert!(!liquid_side_visible(Block::StillWater, block), "{block:?}");
        }
    }

    #[test]
    fn slime_blocks_cull_only_their_shared_face() {
        let slime = state(Block::SlimeBlock, 0);
        assert!(!face_visible_between(
            Block::SlimeBlock,
            slime,
            Block::SlimeBlock,
            slime,
            false,
        ));
        assert!(face_visible_between(
            Block::SlimeBlock,
            slime,
            Block::Glass,
            state(Block::Glass, 0),
            false,
        ));
    }

    #[test]
    fn fancy_leaves_keep_internal_faces() {
        let leaves = state(Block::Leaves, 0);
        assert!(face_visible_between(
            Block::Leaves,
            leaves,
            Block::Leaves,
            leaves,
            false,
        ));
    }

    #[test]
    fn clear_glass_writes_depth_before_adjacent_water() {
        let mut chunk = Chunk::new(0, 0);
        chunk.set(1, 1, 1, Block::Glass);
        chunk.set(2, 1, 1, Block::StillWater);

        let state_at = |x: i32, y: i32, z: i32| match (x, y, z) {
            (1, 1, 1) => state(Block::Glass, 0),
            (2, 1, 1) => state(Block::StillWater, 0),
            _ => 0,
        };
        let mesh = build_chunk_mesh(
            &chunk,
            |x, y, z| Block::from_state(state_at(x, y, z)),
            |_, _, _| LightLevel { sky: 15, block: 0 },
            state_at,
            MeshOptions::default(),
        );

        let transparent_start = mesh.transparent_start as usize;
        assert!(transparent_start > 0);
        assert!(transparent_start < mesh.indices.len());
        assert!(mesh.indices[..transparent_start]
            .iter()
            .all(|&index| mesh.vertices[index as usize].block_type == 5.0));
        assert!(mesh.indices[transparent_start..]
            .iter()
            .all(|&index| mesh.vertices[index as usize].block_type == 4.0));
    }

    #[test]
    fn skull_is_rendered_only_by_its_block_entity() {
        let mut chunk = Chunk::new(0, 0);
        chunk.set(1, 1, 1, Block::Skull);

        let mesh = build_chunk_mesh(
            &chunk,
            |x, y, z| {
                if (x, y, z) == (1, 1, 1) {
                    Block::Skull
                } else {
                    Block::Air
                }
            },
            |_, _, _| LightLevel { sky: 15, block: 0 },
            |x, y, z| {
                if (x, y, z) == (1, 1, 1) {
                    state(Block::Skull, 0)
                } else {
                    0
                }
            },
            MeshOptions::default(),
        );

        assert!(mesh.vertices.is_empty());
        assert!(mesh.indices.is_empty());
    }

    #[test]
    fn moving_piston_placeholder_emits_no_static_mesh() {
        assert_eq!(Block::from_id(36), Block::PistonExtension);
        assert_eq!(Block::PistonExtension.to_id(), 36);

        let mut chunk = Chunk::new(0, 0);
        chunk.set(1, 1, 1, Block::PistonExtension);

        let mesh = build_chunk_mesh(
            &chunk,
            |x, y, z| {
                if (x, y, z) == (1, 1, 1) {
                    Block::PistonExtension
                } else {
                    Block::Air
                }
            },
            |_, _, _| LightLevel { sky: 15, block: 0 },
            |x, y, z| {
                if (x, y, z) == (1, 1, 1) {
                    state(Block::PistonExtension, 1)
                } else {
                    0
                }
            },
            MeshOptions::default(),
        );

        assert!(mesh.vertices.is_empty());
        assert!(mesh.indices.is_empty());
    }
}
