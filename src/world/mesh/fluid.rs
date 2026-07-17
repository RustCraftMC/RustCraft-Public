use crate::world::block::Block;

/// Get the rendered height for a liquid block based on its level metadata.
/// MC 1.8.9: level 0 = source (1/9 height), level 1-7 = flowing, level 8+ = falling.
pub fn liquid_render_height(level: u8) -> f32 {
    let effective = if level >= 8 { 0 } else { level };
    (effective as f32 + 1.0) / 9.0
}

/// Get smooth liquid height at a corner by sampling the 2x2 neighborhood.
/// Returns a height in [0.0, 1.0] for the surface at this corner.
pub fn corner_fluid_height(
    fluid: Block,
    wx: i32,
    y: i32,
    wz: i32,
    world_get: impl Fn(i32, i32, i32) -> Block + Copy,
    state_get: impl Fn(i32, i32, i32) -> u16 + Copy,
) -> f32 {
    let mut total_weight = 0.0f32;
    let mut weighted_sum = 0.0f32;

    // Sample 4 blocks that touch this corner: offsets (0,0), (-1,0), (0,-1), (-1,-1)
    for j in 0..4 {
        let dx = -(j & 1) as i32;
        let dz = -((j >> 1) & 1) as i32;
        let sx = wx + dx;
        let sz = wz + dz;

        // BlockFluidRenderer#getFluidHeight only treats the same material
        // above this sample as a full-height continuation.
        if same_fluid_material(fluid, world_get(sx, y + 1, sz)) {
            return 1.0;
        }

        let block = world_get(sx, y, sz);
        if same_fluid_material(fluid, block) {
            let state = state_get(sx, y, sz);
            let level = (state & 0x0f) as u8;
            let height = liquid_render_height(level);
            // Vanilla gives source and falling liquid ten extra samples so a
            // single flowing neighbor cannot invert or sharply dent a pool.
            if level == 0 || level >= 8 {
                weighted_sum += height * 10.0;
                total_weight += 10.0;
            }
            weighted_sum += height;
            total_weight += 1.0;
        } else if !has_solid_material(block) {
            // Air, plants and other non-solid materials contribute an empty
            // sample (depth 1), which lowers the surface smoothly at shores.
            weighted_sum += 1.0;
            total_weight += 1.0;
        }
    }

    if total_weight == 0.0 {
        return 0.0;
    }
    (1.0 - weighted_sum / total_weight).max(0.0)
}

fn same_fluid_material(a: Block, b: Block) -> bool {
    matches!(a, Block::FlowingWater | Block::StillWater)
        && matches!(b, Block::FlowingWater | Block::StillWater)
        || matches!(a, Block::FlowingLava | Block::StillLava)
            && matches!(b, Block::FlowingLava | Block::StillLava)
}

/// `Material#isSolid`, not the block's collision/full-cube property. Partial
/// blocks such as stairs have a solid material and must not count as air.
fn has_solid_material(block: Block) -> bool {
    !matches!(
        block,
        Block::Air
            | Block::Sapling
            | Block::FlowingWater
            | Block::StillWater
            | Block::FlowingLava
            | Block::StillLava
            | Block::TallGrass
            | Block::DeadBush
            | Block::Dandelion
            | Block::Flower
            | Block::BrownMushroom
            | Block::RedMushroom
            | Block::Torch
            | Block::Fire
            | Block::RedstoneWire
            | Block::Wheat
            | Block::Ladder
            | Block::Rail
            | Block::PoweredRail
            | Block::DetectorRail
            | Block::ActivatorRail
            | Block::Lever
            | Block::UnlitRedstoneTorch
            | Block::RedstoneTorch
            | Block::StoneButton
            | Block::WoodenButton
            | Block::SnowLayer
            | Block::SugarCane
            | Block::NetherPortal
            | Block::UnpoweredRepeater
            | Block::PoweredRepeater
            | Block::PumpkinStem
            | Block::MelonStem
            | Block::Vine
            | Block::LilyPad
            | Block::NetherWart
            | Block::EndPortal
            | Block::Cocoa
            | Block::TripwireHook
            | Block::Tripwire
            | Block::FlowerPot
            | Block::Carrots
            | Block::Potatoes
            | Block::Skull
            | Block::UnpoweredComparator
            | Block::PoweredComparator
            | Block::Carpet
            | Block::LargeFlower
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_fluid_dominates_flowing_neighbor_like_vanilla() {
        let block_at = |x, y, z| {
            if y != 0 {
                Block::Air
            } else if (x, z) == (1, 1) || (x, z) == (0, 1) {
                Block::StillWater
            } else {
                Block::Stone
            }
        };
        let state_at = |x, _y, z| if (x, z) == (0, 1) { 7 } else { 0 };

        let height = corner_fluid_height(Block::StillWater, 1, 0, 1, block_at, state_at);
        let expected = 89.0 / 108.0;
        assert!((height - expected).abs() < 1.0e-6);
    }

    #[test]
    fn non_solid_neighbors_lower_a_shore_corner() {
        let block_at = |x, y, z| {
            if y == 0 && (x, z) == (1, 1) {
                Block::StillWater
            } else {
                Block::Air
            }
        };

        let height = corner_fluid_height(Block::StillWater, 1, 0, 1, block_at, |_, _, _| 0);
        assert!((height - 44.0 / 63.0).abs() < 1.0e-6);
    }

    #[test]
    fn opposite_liquid_above_does_not_force_full_height() {
        let block_at = |x, y, z| match (x, y, z) {
            (1, 0, 1) => Block::StillWater,
            (1, 1, 1) => Block::StillLava,
            _ => Block::Stone,
        };

        let height = corner_fluid_height(Block::StillWater, 1, 0, 1, block_at, |_, _, _| 0);
        assert!((height - 8.0 / 9.0).abs() < 1.0e-6);
    }

    #[test]
    fn partial_block_with_solid_material_is_not_an_air_sample() {
        let block_at = |x, y, z| {
            if y == 0 && (x, z) == (1, 1) {
                Block::StillWater
            } else {
                Block::OakStairs
            }
        };

        let height = corner_fluid_height(Block::StillWater, 1, 0, 1, block_at, |_, _, _| 0);
        assert!((height - 8.0 / 9.0).abs() < 1.0e-6);
    }
}
