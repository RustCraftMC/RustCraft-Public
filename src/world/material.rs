//! Shared material semantics for world meshes and GUI block previews.

use crate::world::block::Block;

pub const MATERIAL_NORMAL: f32 = 0.0;
pub const MATERIAL_GRASS: f32 = 1.0;
pub const MATERIAL_FOLIAGE: f32 = 3.0;
pub const MATERIAL_WATER: f32 = 4.0;
pub const MATERIAL_CUTOUT: f32 = 5.0;

pub const GRASS_COLOR: [f32; 3] = [
    0x91 as f32 / 255.0,
    0xbd as f32 / 255.0,
    0x59 as f32 / 255.0,
];
pub const FOLIAGE_COLOR: [f32; 3] = [
    0x77 as f32 / 255.0,
    0xab as f32 / 255.0,
    0x2f as f32 / 255.0,
];
pub const WATER_COLOR: [f32; 3] = [
    0x3f as f32 / 255.0,
    0x76 as f32 / 255.0,
    0xe4 as f32 / 255.0,
];

/// Resolve the JSON model tint index using block semantics. A tint index only
/// selects a model layer; the block decides which color map that layer uses.
pub fn model_material(block: Block, tint_index: Option<i32>) -> f32 {
    if is_water(block) {
        return MATERIAL_WATER;
    }
    if is_cutout_material(block) {
        return MATERIAL_CUTOUT;
    }
    if tint_index.is_none() {
        return MATERIAL_NORMAL;
    }
    if is_foliage(block) {
        MATERIAL_FOLIAGE
    } else if is_grass_tinted(block) {
        MATERIAL_GRASS
    } else {
        MATERIAL_NORMAL
    }
}

pub fn material_color(material: f32) -> [f32; 3] {
    if (material - MATERIAL_GRASS).abs() < 0.25 {
        GRASS_COLOR
    } else if (material - MATERIAL_FOLIAGE).abs() < 0.25 {
        FOLIAGE_COLOR
    } else if (material - MATERIAL_WATER).abs() < 0.25 {
        WATER_COLOR
    } else {
        [1.0; 3]
    }
}

pub fn fallback_face_material(block: Block, normal: [f32; 3]) -> f32 {
    if is_foliage(block) {
        MATERIAL_FOLIAGE
    } else if matches!(block, Block::Grass | Block::GrassSnowy) && normal[1] > 0.5 {
        MATERIAL_GRASS
    } else {
        model_material(block, None)
    }
}

/// Whether world geometry belongs to vanilla's `TRANSLUCENT` render layer.
/// Shader material/tint values are separate from this decision: clear glass
/// is alpha-tested with depth writes, while stained glass is blended.
pub fn uses_translucent_layer(block: Block) -> bool {
    matches!(
        block,
        Block::FlowingWater
            | Block::StillWater
            | Block::FlowingLava
            | Block::StillLava
            | Block::StainedGlass
            | Block::StainedGlassPane
            | Block::Ice
            | Block::SlimeBlock
            | Block::NetherPortal
            | Block::Tripwire
    )
}

fn is_grass_tinted(block: Block) -> bool {
    matches!(
        block,
        Block::Grass
            | Block::GrassSnowy
            | Block::TallGrass
            | Block::Vine
            | Block::LilyPad
            | Block::SugarCane
    )
}

fn is_foliage(block: Block) -> bool {
    matches!(block, Block::Leaves | Block::Leaves2 | Block::Leaves3)
}

fn is_water(block: Block) -> bool {
    matches!(block, Block::FlowingWater | Block::StillWater)
}

fn is_cutout_material(block: Block) -> bool {
    matches!(
        block,
        Block::Glass
            | Block::StainedGlass
            | Block::GlassPane
            | Block::StainedGlassPane
            | Block::Ice
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untinted_grass_model_faces_stay_untinted() {
        assert_eq!(model_material(Block::Grass, None), MATERIAL_NORMAL);
        assert_eq!(model_material(Block::Grass, Some(0)), MATERIAL_GRASS);
    }

    #[test]
    fn tint_index_uses_block_semantics() {
        assert_eq!(model_material(Block::Leaves, Some(0)), MATERIAL_FOLIAGE);
        assert_eq!(model_material(Block::Stone, Some(0)), MATERIAL_NORMAL);
    }

    #[test]
    fn stained_glass_uses_the_blended_material() {
        assert_eq!(model_material(Block::StainedGlass, None), MATERIAL_CUTOUT);
    }

    #[test]
    fn vanilla_opaque_full_blocks_stay_in_the_opaque_pass() {
        for block in [Block::PackedIce, Block::Prismarine, Block::SeaLantern] {
            assert!(!block.is_transparent());
            assert!(block.properties().is_opaque);
            assert_eq!(model_material(block, None), MATERIAL_NORMAL);
        }
    }

    #[test]
    fn vanilla_glass_layers_keep_clear_glass_depth_writing() {
        for block in [Block::Glass, Block::GlassPane, Block::IronBars] {
            assert!(!uses_translucent_layer(block), "{block:?}");
        }
        for block in [
            Block::StainedGlass,
            Block::StainedGlassPane,
            Block::StillWater,
            Block::StillLava,
            Block::Ice,
            Block::SlimeBlock,
            Block::NetherPortal,
            Block::Tripwire,
        ] {
            assert!(uses_translucent_layer(block), "{block:?}");
        }
    }
}
