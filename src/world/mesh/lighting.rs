use crate::world::block::Block;
use crate::world::light::LightLevel;

pub fn smooth_vertex_light(
    p: [f32; 3],
    n: [f32; 3],
    base_x: i32,
    base_z: i32,
    block_pos: [i32; 3],
    world_get: impl Fn(i32, i32, i32) -> Block + Copy,
    light_get: impl Fn(i32, i32, i32) -> LightLevel + Copy,
) -> (f32, f32, f32) {
    let world_pos = [base_x as f32 + p[0], p[1], base_z as f32 + p[2]];
    let normal = [
        n[0].round() as i32,
        n[1].round() as i32,
        n[2].round() as i32,
    ];
    let face_pos = light_sample_pos(world_pos, normal, block_pos);
    let tangent_axes = if normal[0] != 0 {
        [1, 2]
    } else if normal[1] != 0 {
        [0, 2]
    } else {
        [0, 1]
    };
    let mut side_a_offset = [0; 3];
    let mut side_b_offset = [0; 3];
    side_a_offset[tangent_axes[0]] =
        vertex_side(world_pos[tangent_axes[0]], block_pos[tangent_axes[0]]);
    side_b_offset[tangent_axes[1]] =
        vertex_side(world_pos[tangent_axes[1]], block_pos[tangent_axes[1]]);

    let side_a = add(face_pos, side_a_offset);
    let side_b = add(face_pos, side_b_offset);
    let diagonal_pos = add(side_a, side_b_offset);
    let side_a_outer = add(side_a, normal);
    let side_b_outer = add(side_b, normal);
    let diagonal = if face_pos != block_pos
        && is_opaque(world_get, side_a_outer)
        && is_opaque(world_get, side_b_outer)
    {
        side_a
    } else {
        diagonal_pos
    };

    let center_light = light_at(light_get, face_pos);
    let samples = [
        center_light,
        light_at(light_get, side_a),
        light_at(light_get, side_b),
        light_at(light_get, diagonal),
    ];
    let sky_total = samples.iter().map(|light| light.sky as f32).sum::<f32>();
    let block_total = samples.iter().map(|light| light.block as f32).sum::<f32>();
    let ao_total = [face_pos, side_a, side_b, diagonal]
        .iter()
        .map(|&pos| if is_opaque(world_get, pos) { 0.2 } else { 1.0 })
        .sum::<f32>();

    (
        (sky_total * 0.25).clamp(0.0, 15.0),
        (block_total * 0.25).clamp(0.0, 15.0),
        (ao_total * 0.25).clamp(0.2, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_ao_samples_do_not_darken_light_average() {
        let (sky, block, ao) = smooth_vertex_light(
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
            0,
            0,
            [0, 0, 0],
            |x, y, z| {
                if x == 0 && y == 0 && z == 0 {
                    Block::Stone
                } else {
                    Block::Air
                }
            },
            |x, y, z| {
                if x == 0 && y == 0 && z == 0 {
                    LightLevel { sky: 0, block: 0 }
                } else {
                    LightLevel { sky: 15, block: 6 }
                }
            },
        );
        assert_eq!(sky, 15.0);
        assert_eq!(block, 6.0);
        assert_eq!(ao, 1.0);
    }

    #[test]
    fn open_pit_wall_uses_the_air_cells_skylight() {
        let (sky, _, ao) = smooth_vertex_light(
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 0.0],
            0,
            0,
            [0, 0, 0],
            |x, y, z| {
                if x <= 0 || y < 0 || z < 0 {
                    Block::Dirt
                } else {
                    Block::Air
                }
            },
            |x, y, z| {
                if x >= 1 && y >= 0 && z >= 0 {
                    LightLevel { sky: 15, block: 0 }
                } else {
                    LightLevel { sky: 0, block: 0 }
                }
            },
        );
        assert_eq!(sky, 15.0);
        assert!(ao >= 0.6);
    }

    #[test]
    fn partial_block_faces_use_their_own_voxel_light() {
        let (sky, block, _) = smooth_vertex_light(
            [0.5, 0.5, 0.5],
            [0.0, 1.0, 0.0],
            0,
            0,
            [0, 0, 0],
            |_, _, _| Block::Air,
            |_, y, _| {
                if y == 0 {
                    LightLevel { sky: 9, block: 4 }
                } else {
                    LightLevel { sky: 0, block: 0 }
                }
            },
        );
        assert_eq!((sky, block), (9.0, 4.0));
    }

    #[test]
    fn full_block_faces_still_use_neighbor_light() {
        let (sky, block, _) = smooth_vertex_light(
            [0.5, 1.0, 0.5],
            [0.0, 1.0, 0.0],
            0,
            0,
            [0, 0, 0],
            |_, _, _| Block::Air,
            |_, y, _| {
                if y == 1 {
                    LightLevel { sky: 14, block: 2 }
                } else {
                    LightLevel { sky: 0, block: 0 }
                }
            },
        );
        assert_eq!((sky, block), (14.0, 2.0));
    }

    #[test]
    fn dark_neighbor_samples_are_not_replaced_with_center_light() {
        let (sky, block, _) = smooth_vertex_light(
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
            0,
            0,
            [0, 0, 0],
            |_, _, _| Block::Air,
            |x, _, z| {
                if x == 0 && z == 0 {
                    LightLevel { sky: 12, block: 8 }
                } else {
                    LightLevel { sky: 0, block: 0 }
                }
            },
        );
        assert_eq!((sky, block), (3.0, 2.0));
    }
}

pub fn face_light(
    p: [f32; 3],
    n: [f32; 3],
    base_x: i32,
    base_z: i32,
    block_pos: [i32; 3],
    light_get: impl Fn(i32, i32, i32) -> LightLevel + Copy,
) -> (f32, f32, f32) {
    let normal = [
        n[0].round() as i32,
        n[1].round() as i32,
        n[2].round() as i32,
    ];
    let world_pos = [base_x as f32 + p[0], p[1], base_z as f32 + p[2]];
    let ll = light_at(light_get, light_sample_pos(world_pos, normal, block_pos));
    (ll.sky as f32, ll.block as f32, 1.0)
}

/// Full-cube faces take their light from the neighbouring voxel. A face inside
/// its voxel (a slab top, stair riser, or other partial-model surface) instead
/// takes the block's own light, matching vanilla's block-model lighting path.
fn light_sample_pos(world_pos: [f32; 3], normal: [i32; 3], block_pos: [i32; 3]) -> [i32; 3] {
    let Some(axis) = normal.iter().position(|component| *component != 0) else {
        return block_pos;
    };
    let boundary = block_pos[axis] as f32 + if normal[axis] > 0 { 1.0 } else { 0.0 };
    if (world_pos[axis] - boundary).abs() <= 0.001 {
        add(block_pos, normal)
    } else {
        block_pos
    }
}

fn vertex_side(coord: f32, block_coord: i32) -> i32 {
    if coord < block_coord as f32 + 0.5 {
        -1
    } else {
        1
    }
}

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn light_at(light_get: impl Fn(i32, i32, i32) -> LightLevel + Copy, pos: [i32; 3]) -> LightLevel {
    light_get(pos[0], pos[1], pos[2])
}

fn is_opaque(world_get: impl Fn(i32, i32, i32) -> Block + Copy, pos: [i32; 3]) -> bool {
    world_get(pos[0], pos[1], pos[2]).properties().is_opaque
}
