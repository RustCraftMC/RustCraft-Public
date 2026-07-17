//! Vanilla 1.8.9 `Block.SoundType` mapping.
//!
//! Keep this next to the block registry so breaking, placing, walking and
//! server world-event playback all use one source of truth.

use super::Block;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockSound {
    Stone,
    Metal,
    Wood,
    Gravel,
    Grass,
    Cloth,
    Sand,
    Snow,
    Ladder,
    Anvil,
    Slime,
    Glass,
}

impl BlockSound {
    pub const fn step_event(self) -> &'static str {
        match self {
            Self::Stone | Self::Metal | Self::Glass => "step.stone",
            Self::Wood => "step.wood",
            Self::Gravel => "step.gravel",
            Self::Grass => "step.grass",
            Self::Cloth => "step.cloth",
            Self::Sand => "step.sand",
            Self::Snow => "step.snow",
            Self::Ladder => "step.ladder",
            Self::Anvil => "step.anvil",
            Self::Slime => "step.slime",
        }
    }

    pub const fn dig_event(self) -> &'static str {
        match self {
            Self::Glass => "dig.glass",
            // `soundTypeLadder` overrides only the break sound in vanilla.
            Self::Ladder => "dig.wood",
            // `soundTypeAnvil` overrides only the break sound in vanilla.
            Self::Anvil => "dig.stone",
            Self::Stone | Self::Metal => "dig.stone",
            Self::Wood => "dig.wood",
            Self::Gravel => "dig.gravel",
            Self::Grass => "dig.grass",
            Self::Cloth => "dig.cloth",
            Self::Sand => "dig.sand",
            Self::Snow => "dig.snow",
            Self::Slime => "dig.slime",
        }
    }

    pub const fn volume(self) -> f32 {
        if matches!(self, Self::Anvil) {
            0.3
        } else {
            1.0
        }
    }

    pub const fn pitch(self) -> f32 {
        if matches!(self, Self::Metal) {
            1.5
        } else {
            1.0
        }
    }
}

impl Block {
    /// The exact 1.8.9 `Block.stepSound` family for this registry entry.
    pub const fn sound_type(self) -> BlockSound {
        match self {
            // Block.java: soundTypeGlass overrides breaking to dig.glass.
            Self::Glass
            | Self::Ice
            | Self::PackedIce
            | Self::StainedGlass
            | Self::GlassPane
            | Self::StainedGlassPane
            | Self::Glowstone
            | Self::NetherPortal
            | Self::EndPortal
            | Self::EndPortalFrame
            | Self::RedstoneLamp
            | Self::LitRedstoneLamp
            | Self::SeaLantern => BlockSound::Glass,

            Self::Ladder => BlockSound::Ladder,
            Self::Anvil => BlockSound::Anvil,
            Self::SlimeBlock => BlockSound::Slime,
            Self::SnowLayer | Self::SnowBlock => BlockSound::Snow,

            // soundTypeMetal still uses the stone samples, at pitch 1.5.
            Self::GoldBlock
            | Self::IronBlock
            | Self::DiamondBlock
            | Self::LapisBlock
            | Self::EmeraldBlock
            | Self::RedstoneBlock
            | Self::MobSpawner
            | Self::IronBars
            | Self::IronDoor
            | Self::IronTrapdoor
            | Self::PoweredRail
            | Self::DetectorRail
            | Self::ActivatorRail
            | Self::Rail
            | Self::Hopper => BlockSound::Metal,

            Self::Dirt | Self::Farmland | Self::Gravel | Self::Clay => BlockSound::Gravel,
            Self::Sand | Self::SoulSand | Self::RedSandstone => BlockSound::Sand,
            Self::Wool | Self::Cobweb | Self::Fire | Self::Cactus | Self::Cake | Self::Carpet => {
                BlockSound::Cloth
            }

            // Plants and leaves use vanilla's grass sound type, not stone.
            Self::Grass
            | Self::GrassSnowy
            | Self::Sapling
            | Self::Leaves
            | Self::Leaves2
            | Self::Leaves3
            | Self::Sponge
            | Self::TallGrass
            | Self::DeadBush
            | Self::Dandelion
            | Self::Flower
            | Self::LargeFlower
            | Self::BrownMushroom
            | Self::RedMushroom
            | Self::Tnt
            | Self::SugarCane
            | Self::Mycelium
            | Self::LilyPad
            | Self::Vine
            | Self::HayBlock => BlockSound::Grass,

            Self::Planks
            | Self::Log
            | Self::Log2
            | Self::Log3
            | Self::Bookshelf
            | Self::Bed
            | Self::Chest
            | Self::TrappedChest
            | Self::CraftingTable
            | Self::StandingSign
            | Self::WallSign
            | Self::StandingBanner
            | Self::WallBanner
            | Self::OakDoor
            | Self::SpruceDoor
            | Self::BirchDoor
            | Self::JungleDoor
            | Self::AcaciaDoor
            | Self::DarkOakDoor
            | Self::Trapdoor
            | Self::OakFence
            | Self::SpruceFence
            | Self::BirchFence
            | Self::JungleFence
            | Self::DarkOakFence
            | Self::AcaciaFence
            | Self::OakFenceGate
            | Self::SpruceFenceGate
            | Self::BirchFenceGate
            | Self::JungleFenceGate
            | Self::DarkOakFenceGate
            | Self::AcaciaFenceGate
            | Self::OakStairs
            | Self::SpruceStairs
            | Self::BirchStairs
            | Self::JungleStairs
            | Self::AcaciaStairs
            | Self::DarkOakStairs
            | Self::DoubleWoodSlab
            | Self::WoodSlab
            | Self::WoodenPressurePlate
            | Self::WoodenButton
            | Self::NoteBlock
            | Self::Jukebox
            | Self::Torch
            | Self::UnlitRedstoneTorch
            | Self::RedstoneTorch
            | Self::Lever
            | Self::UnpoweredRepeater
            | Self::PoweredRepeater
            | Self::UnpoweredComparator
            | Self::PoweredComparator
            | Self::Pumpkin
            | Self::JackOLantern
            | Self::MelonBlock
            | Self::PumpkinStem
            | Self::MelonStem
            | Self::NetherWart
            | Self::Cocoa
            | Self::BrownMushroomBlock
            | Self::RedMushroomBlock => BlockSound::Wood,

            _ => BlockSound::Stone,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foliage_and_flowers_use_grass_sounds() {
        assert_eq!(Block::Leaves.sound_type(), BlockSound::Grass);
        assert_eq!(Block::Flower.sound_type().dig_event(), "dig.grass");
    }

    #[test]
    fn ladder_and_glass_keep_vanilla_overrides() {
        assert_eq!(Block::Ladder.sound_type().step_event(), "step.ladder");
        assert_eq!(Block::Ladder.sound_type().dig_event(), "dig.wood");
        assert_eq!(Block::Glass.sound_type().dig_event(), "dig.glass");
    }
}
