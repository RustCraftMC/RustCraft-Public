//! Vanilla 1.8.9 `Material` classification for the subset of behaviour the
//! client needs: tool requirements (`Material.setRequiresTool`) and tool
//! effectiveness (`ItemPickaxe`/`ItemAxe`/`ItemSword`/`ItemShears`
//! `getStrVsBlock` test materials, not the full material list).

use super::Block;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VanillaMaterial {
    Rock,
    Iron,
    Anvil,
    Web,
    Snow,
    CraftedSnow,
    Barrier,
    Wood,
    Plants,
    Vine,
    Leaves,
    Gourd,
    Cloth,
    Other,
}

impl VanillaMaterial {
    /// `Material.isToolNotRequired()` is false for these (1.8.9 Material.java:
    /// rock, iron, anvil, web, snow, craftedSnow, barrier call
    /// `setRequiresTool()`); harvesting them without the right tool uses the
    /// /100 dig-speed path and drops nothing.
    pub fn requires_tool(self) -> bool {
        matches!(
            self,
            VanillaMaterial::Rock
                | VanillaMaterial::Iron
                | VanillaMaterial::Anvil
                | VanillaMaterial::Web
                | VanillaMaterial::Snow
                | VanillaMaterial::CraftedSnow
                | VanillaMaterial::Barrier
        )
    }
}

impl Block {
    /// Vanilla material of this block as registered in 1.8.9 `Blocks.java`.
    pub fn material(self) -> VanillaMaterial {
        use VanillaMaterial::*;
        match self {
            // Material.rock
            Block::Stone
            | Block::StoneGranite
            | Block::StoneDiorite
            | Block::StoneAndesite
            | Block::Cobblestone
            | Block::Bedrock
            | Block::GoldOre
            | Block::IronOre
            | Block::CoalOre
            | Block::LapisOre
            | Block::Dispenser
            | Block::Dropper
            | Block::Sandstone
            | Block::RedSandstone
            | Block::DoubleStoneSlab
            | Block::StoneSlab
            | Block::DoubleStoneSlab2
            | Block::StoneSlab2
            | Block::Bricks
            | Block::MossyCobblestone
            | Block::Obsidian
            | Block::MobSpawner
            | Block::DiamondOre
            | Block::Furnace
            | Block::LitFurnace
            | Block::CobblestoneStairs
            | Block::RedstoneOre
            | Block::LitRedstoneOre
            | Block::Netherrack
            | Block::StoneBricks
            | Block::BrickStairs
            | Block::StoneBrickStairs
            | Block::NetherBrick
            | Block::NetherBrickFence
            | Block::NetherBrickStairs
            | Block::EnchantingTable
            | Block::EndPortalFrame
            | Block::EndStone
            | Block::SandstoneStairs
            | Block::EmeraldOre
            | Block::EnderChest
            | Block::CobblestoneWall
            | Block::QuartzOre
            | Block::QuartzBlock
            | Block::QuartzStairs
            | Block::StainedClay
            | Block::HardenedClay
            | Block::CoalBlock
            | Block::StonePressurePlate
            | Block::Prismarine
            | Block::RedSandstoneStairs => Rock,

            // Material.iron
            Block::IronBlock
            | Block::GoldBlock
            | Block::DiamondBlock
            | Block::EmeraldBlock
            | Block::RedstoneBlock
            | Block::LapisBlock
            | Block::IronDoor
            | Block::IronTrapdoor
            | Block::IronBars
            | Block::Hopper
            | Block::Cauldron
            | Block::BrewingStand
            | Block::LightWeightedPressurePlate
            | Block::HeavyWeightedPressurePlate
            | Block::CommandBlock => Iron,

            Block::Anvil => Anvil,
            Block::Cobweb => Web,
            Block::SnowLayer => Snow,
            Block::SnowBlock => CraftedSnow,
            Block::Barrier => Barrier,

            // Material.wood
            Block::Planks
            | Block::Log
            | Block::Log2
            | Block::Log3
            | Block::Bookshelf
            | Block::DoubleWoodSlab
            | Block::WoodSlab
            | Block::OakStairs
            | Block::SpruceStairs
            | Block::BirchStairs
            | Block::JungleStairs
            | Block::AcaciaStairs
            | Block::DarkOakStairs
            | Block::CraftingTable
            | Block::Chest
            | Block::TrappedChest
            | Block::OakDoor
            | Block::SpruceDoor
            | Block::BirchDoor
            | Block::JungleDoor
            | Block::AcaciaDoor
            | Block::DarkOakDoor
            | Block::Trapdoor
            | Block::OakFence
            | Block::SpruceFence
            | Block::BirchFence
            | Block::JungleFence
            | Block::DarkOakFence
            | Block::AcaciaFence
            | Block::OakFenceGate
            | Block::SpruceFenceGate
            | Block::BirchFenceGate
            | Block::JungleFenceGate
            | Block::DarkOakFenceGate
            | Block::AcaciaFenceGate
            | Block::WoodenPressurePlate
            | Block::NoteBlock
            | Block::Jukebox
            | Block::StandingSign
            | Block::WallSign
            | Block::DaylightDetector
            | Block::DaylightDetectorInverted
            | Block::BrownMushroomBlock
            | Block::RedMushroomBlock
            | Block::StandingBanner
            | Block::WallBanner => Wood,

            // Material.plants
            Block::Sapling
            | Block::Dandelion
            | Block::Flower
            | Block::BrownMushroom
            | Block::RedMushroom
            | Block::Wheat
            | Block::Carrots
            | Block::Potatoes
            | Block::SugarCane
            | Block::NetherWart
            | Block::Cocoa
            | Block::LilyPad
            | Block::PumpkinStem
            | Block::MelonStem => Plants,

            // Material.vine
            Block::TallGrass | Block::DeadBush | Block::Vine | Block::LargeFlower => Vine,

            Block::Leaves | Block::Leaves2 | Block::Leaves3 => Leaves,
            Block::Pumpkin | Block::JackOLantern | Block::MelonBlock => Gourd,
            Block::Wool | Block::Carpet => Cloth,

            _ => Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_required_materials_match_vanilla_set_requires_tool() {
        assert!(Block::Stone.material().requires_tool());
        assert!(Block::IronBlock.material().requires_tool());
        assert!(Block::Anvil.material().requires_tool());
        assert!(Block::Cobweb.material().requires_tool());
        assert!(Block::SnowLayer.material().requires_tool());
        assert!(Block::SnowBlock.material().requires_tool());
        assert!(!Block::Dirt.material().requires_tool());
        assert!(!Block::Planks.material().requires_tool());
        assert!(!Block::StoneButton.material().requires_tool());
    }
}
