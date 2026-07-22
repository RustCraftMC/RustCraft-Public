//! Block registry — ID-based lookup for block data with mod extension support.
//!
//! The `BlockRegistry` provides an alternative to the `Block` enum's method-based
//! property lookup. It supports both built-in MC 1.8.9 blocks (registered at
//! construction) and custom blocks added by mods. The `Block` enum remains the
//! primary way to refer to known block types; the registry enables extension
//! without modifying the enum.

use super::Block;
use super::properties::BlockProperties;
use std::collections::HashMap;

/// Comprehensive block info stored in the registry.
#[derive(Clone, Debug)]
pub struct BlockInfo {
    /// Numeric block ID (MC 1.8.9 protocol).
    pub id: u16,
    /// Block name (e.g., "stone", "grass").
    pub name: String,
    /// Block properties (hardness, resistance, etc.).
    pub properties: BlockProperties,
    /// Whether this block is a liquid.
    pub is_liquid: bool,
    /// Whether this block is solid (blocks movement).
    pub is_solid: bool,
    /// Whether this block is transparent (allows light through).
    pub is_transparent: bool,
}

/// Registry for block data. Supports both built-in MC 1.8.9 blocks
/// (pre-registered at construction) and custom blocks added by mods.
pub struct BlockRegistry {
    by_id: HashMap<u16, BlockInfo>,
    by_name: HashMap<String, u16>,
}

impl BlockRegistry {
    /// Create a new registry with all built-in MC 1.8.9 blocks pre-registered.
    ///
    /// This iterates over all known `Block` enum variants and registers each
    /// one by its protocol ID and properties.
    pub fn new() -> Self {
        let mut registry = BlockRegistry {
            by_id: HashMap::new(),
            by_name: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    /// Register a custom block. Returns false if the ID is already taken.
    pub fn register_custom(&mut self, info: BlockInfo) -> bool {
        if self.by_id.contains_key(&info.id) {
            return false;
        }
        self.by_name.insert(info.name.clone(), info.id);
        self.by_id.insert(info.id, info);
        true
    }

    /// Look up block info by protocol ID.
    pub fn get_by_id(&self, id: u16) -> Option<&BlockInfo> {
        self.by_id.get(&id)
    }

    /// Look up block info by name.
    pub fn get_by_name(&self, name: &str) -> Option<&BlockInfo> {
        self.by_name.get(name).and_then(|id| self.by_id.get(id))
    }

    /// Number of registered block types.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    fn register_builtins(&mut self) {
        // Register all built-in blocks from the enum.
        // We iterate over every variant and collect its data.
        let builtins: &[(Block, &str)] = &[
            (Block::Air, "air"),
            (Block::Stone, "stone"),
            (Block::Grass, "grass"),
            (Block::Dirt, "dirt"),
            (Block::Cobblestone, "cobblestone"),
            (Block::Planks, "planks"),
            (Block::Sapling, "sapling"),
            (Block::Bedrock, "bedrock"),
            (Block::FlowingWater, "flowing_water"),
            (Block::StillWater, "water"),
            (Block::FlowingLava, "flowing_lava"),
            (Block::StillLava, "lava"),
            (Block::Sand, "sand"),
            (Block::Gravel, "gravel"),
            (Block::GoldOre, "gold_ore"),
            (Block::IronOre, "iron_ore"),
            (Block::CoalOre, "coal_ore"),
            (Block::Log, "log"),
            (Block::Log2, "log2"),
            (Block::Leaves, "leaves"),
            (Block::Leaves2, "leaves2"),
            (Block::Sponge, "sponge"),
            (Block::Glass, "glass"),
            (Block::LapisOre, "lapis_ore"),
            (Block::LapisBlock, "lapis_block"),
            (Block::Dispenser, "dispenser"),
            (Block::Sandstone, "sandstone"),
            (Block::NoteBlock, "noteblock"),
            (Block::Bed, "bed"),
            (Block::PoweredRail, "powered_rail"),
            (Block::DetectorRail, "detector_rail"),
            (Block::StickyPiston, "sticky_piston"),
            (Block::Cobweb, "web"),
            (Block::TallGrass, "tallgrass"),
            (Block::DeadBush, "deadbush"),
            (Block::Piston, "piston"),
            (Block::PistonHead, "piston_head"),
            (Block::Wool, "wool"),
            (Block::PistonExtension, "piston_extension"),
            (Block::Dandelion, "flower_dandelion"),
            (Block::Flower, "flower_rose"),
            (Block::BrownMushroom, "mushroom_brown"),
            (Block::RedMushroom, "mushroom_red"),
            (Block::GoldBlock, "gold_block"),
            (Block::IronBlock, "iron_block"),
            (Block::DoubleStoneSlab, "double_stone_slab"),
            (Block::StoneSlab, "stone_slab"),
            (Block::Bricks, "brick"),
            (Block::Tnt, "tnt"),
            (Block::Bookshelf, "bookshelf"),
            (Block::MossyCobblestone, "cobblestone_mossy"),
            (Block::Obsidian, "obsidian"),
            (Block::Torch, "torch"),
            (Block::Fire, "fire"),
            (Block::MobSpawner, "mob_spawner"),
            (Block::OakStairs, "oak_stairs"),
            (Block::Chest, "chest"),
            (Block::RedstoneWire, "redstone_wire"),
            (Block::DiamondOre, "diamond_ore"),
            (Block::DiamondBlock, "diamond_block"),
            (Block::CraftingTable, "crafting_table"),
            (Block::Wheat, "wheat"),
            (Block::Farmland, "farmland"),
            (Block::Furnace, "furnace"),
            (Block::LitFurnace, "lit_furnace"),
            (Block::StandingSign, "standing_sign"),
            (Block::OakDoor, "oak_door"),
            (Block::Ladder, "ladder"),
            (Block::Rail, "rail"),
            (Block::CobblestoneStairs, "cobblestone_stairs"),
            (Block::WallSign, "wall_sign"),
            (Block::Lever, "lever"),
            (Block::StonePressurePlate, "stone_pressure_plate"),
            (Block::IronDoor, "iron_door"),
            (Block::WoodenPressurePlate, "wooden_pressure_plate"),
            (Block::RedstoneOre, "redstone_ore"),
            (Block::LitRedstoneOre, "lit_redstone_ore"),
            (Block::UnlitRedstoneTorch, "unlit_redstone_torch"),
            (Block::RedstoneTorch, "redstone_torch"),
            (Block::StoneButton, "stone_button"),
            (Block::SnowLayer, "snow_layer"),
            (Block::Ice, "ice"),
            (Block::SnowBlock, "snow"),
            (Block::Cactus, "cactus"),
            (Block::Clay, "clay"),
            (Block::SugarCane, "reeds"),
            (Block::Jukebox, "jukebox"),
            (Block::OakFence, "oak_fence"),
            (Block::Pumpkin, "pumpkin"),
            (Block::Netherrack, "netherrack"),
            (Block::SoulSand, "soul_sand"),
            (Block::Glowstone, "glowstone"),
            (Block::NetherPortal, "portal"),
            (Block::JackOLantern, "lit_pumpkin"),
            (Block::Cake, "cake"),
            (Block::UnpoweredRepeater, "unpowered_repeater"),
            (Block::PoweredRepeater, "powered_repeater"),
            (Block::Trapdoor, "trapdoor"),
            (Block::MonsterEgg, "monster_egg"),
            (Block::StoneBricks, "stonebrick"),
            (Block::BrownMushroomBlock, "brown_mushroom_block"),
            (Block::RedMushroomBlock, "red_mushroom_block"),
            (Block::IronBars, "iron_bars"),
            (Block::GlassPane, "glass_pane"),
            (Block::MelonBlock, "melon_block"),
            (Block::PumpkinStem, "pumpkin_stem"),
            (Block::MelonStem, "melon_stem"),
            (Block::Vine, "vine"),
            (Block::OakFenceGate, "oak_fence_gate"),
            (Block::BrickStairs, "brick_stairs"),
            (Block::StoneBrickStairs, "stone_brick_stairs"),
            (Block::Mycelium, "mycelium"),
            (Block::LilyPad, "waterlily"),
            (Block::NetherBrick, "nether_brick"),
            (Block::NetherBrickFence, "nether_brick_fence"),
            (Block::NetherBrickStairs, "nether_brick_stairs"),
            (Block::NetherWart, "nether_wart"),
            (Block::EnchantingTable, "enchanting_table"),
            (Block::BrewingStand, "brewing_stand"),
            (Block::Cauldron, "cauldron"),
            (Block::EndPortal, "end_portal"),
            (Block::EndPortalFrame, "end_portal_frame"),
            (Block::EndStone, "end_stone"),
            (Block::DragonEgg, "dragon_egg"),
            (Block::RedstoneLamp, "redstone_lamp"),
            (Block::LitRedstoneLamp, "lit_redstone_lamp"),
            (Block::DoubleWoodSlab, "double_wood_slab"),
            (Block::WoodSlab, "wood_slab"),
            (Block::Cocoa, "cocoa"),
            (Block::SandstoneStairs, "sandstone_stairs"),
            (Block::EmeraldOre, "emerald_ore"),
            (Block::EnderChest, "ender_chest"),
            (Block::TripwireHook, "tripwire_hook"),
            (Block::Tripwire, "tripwire"),
            (Block::EmeraldBlock, "emerald_block"),
            (Block::SpruceStairs, "spruce_stairs"),
            (Block::BirchStairs, "birch_stairs"),
            (Block::JungleStairs, "jungle_stairs"),
            (Block::CommandBlock, "command_block"),
            (Block::Beacon, "beacon"),
            (Block::CobblestoneWall, "cobblestone_wall"),
            (Block::FlowerPot, "flower_pot"),
            (Block::Carrots, "carrots"),
            (Block::Potatoes, "potatoes"),
            (Block::WoodenButton, "wooden_button"),
            (Block::Skull, "skull"),
            (Block::Anvil, "anvil"),
            (Block::TrappedChest, "trapped_chest"),
            (Block::LightWeightedPressurePlate, "light_weighted_pressure_plate"),
            (Block::HeavyWeightedPressurePlate, "heavy_weighted_pressure_plate"),
            (Block::UnpoweredComparator, "unpowered_comparator"),
            (Block::PoweredComparator, "powered_comparator"),
            (Block::DaylightDetector, "daylight_detector"),
            (Block::RedstoneBlock, "redstone_block"),
            (Block::QuartzOre, "quartz_ore"),
            (Block::Hopper, "hopper"),
            (Block::QuartzBlock, "quartz_block"),
            (Block::QuartzStairs, "quartz_stairs"),
            (Block::ActivatorRail, "activator_rail"),
            (Block::Dropper, "dropper"),
            (Block::StainedClay, "stained_clay"),
            (Block::StainedGlassPane, "stained_glass_pane"),
            (Block::Leaves3, "leaves3"),
            (Block::Log3, "log3"),
            (Block::HayBlock, "hay_block"),
            (Block::Carpet, "carpet"),
            (Block::HardenedClay, "hardened_clay"),
            (Block::CoalBlock, "coal_block"),
            (Block::PackedIce, "ice_packed"),
            (Block::LargeFlower, "large_flower"),
            (Block::StoneGranite, "stone_granite"),
            (Block::StoneDiorite, "stone_diorite"),
            (Block::StoneAndesite, "stone_andesite"),
            (Block::GrassSnowy, "grass_snowy"),
            (Block::StainedGlass, "stained_glass"),
            (Block::AcaciaStairs, "acacia_stairs"),
            (Block::DarkOakStairs, "dark_oak_stairs"),
            (Block::SlimeBlock, "slime_block"),
            (Block::Barrier, "barrier"),
            (Block::IronTrapdoor, "iron_trapdoor"),
            (Block::Prismarine, "prismarine"),
            (Block::SeaLantern, "sea_lantern"),
            (Block::StandingBanner, "standing_banner"),
            (Block::WallBanner, "wall_banner"),
            (Block::DaylightDetectorInverted, "daylight_detector_inverted"),
            (Block::RedSandstone, "red_sandstone"),
            (Block::RedSandstoneStairs, "red_sandstone_stairs"),
            (Block::DoubleStoneSlab2, "double_stone_slab2"),
            (Block::StoneSlab2, "stone_slab2"),
            (Block::SpruceFenceGate, "spruce_fence_gate"),
            (Block::BirchFenceGate, "birch_fence_gate"),
            (Block::JungleFenceGate, "jungle_fence_gate"),
            (Block::DarkOakFenceGate, "dark_oak_fence_gate"),
            (Block::AcaciaFenceGate, "acacia_fence_gate"),
            (Block::SpruceFence, "spruce_fence"),
            (Block::BirchFence, "birch_fence"),
            (Block::JungleFence, "jungle_fence"),
            (Block::DarkOakFence, "dark_oak_fence"),
            (Block::AcaciaFence, "acacia_fence"),
            (Block::SpruceDoor, "spruce_door"),
            (Block::BirchDoor, "birch_door"),
            (Block::JungleDoor, "jungle_door"),
            (Block::AcaciaDoor, "acacia_door"),
            (Block::DarkOakDoor, "dark_oak_door"),
        ];

        for &(block, name) in builtins {
            let props = block.properties();
            let info = BlockInfo {
                id: props.id,
                name: name.to_string(),
                properties: props,
                is_liquid: block.is_liquid(),
                is_solid: block.is_solid(),
                is_transparent: block.is_transparent(),
            };
            self.by_name.insert(name.to_string(), info.id);
            self.by_id.insert(info.id, info);
        }
    }
}

impl Default for BlockRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_key_builtin_blocks() {
        let registry = BlockRegistry::new();
        assert!(registry.get_by_id(0).is_some()); // Air
        assert!(registry.get_by_id(1).is_some()); // Stone
        assert!(registry.get_by_id(2).is_some()); // Grass
        assert!(registry.get_by_id(8).is_some()); // FlowingWater
    }

    #[test]
    fn registry_lookup_by_name() {
        let registry = BlockRegistry::new();
        let info = registry.get_by_name("stone").unwrap();
        assert_eq!(info.id, 1);
        assert_eq!(info.properties.hardness, 1.5);
    }

    #[test]
    fn registry_rejects_duplicate_id() {
        let mut registry = BlockRegistry::new();
        let custom = BlockInfo {
            id: 1, // Stone already registered
            name: "custom_stone".to_string(),
            properties: BlockProperties {
                id: 1,
                name: "custom_stone",
                hardness: 5.0,
                resistance: 50.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            is_liquid: false,
            is_solid: true,
            is_transparent: false,
        };
        assert!(!registry.register_custom(custom));
    }

    #[test]
    fn registry_accepts_new_id() {
        let mut registry = BlockRegistry::new();
        let custom = BlockInfo {
            id: 255, // Outside MC 1.8.9 block range
            name: "custom_block".to_string(),
            properties: BlockProperties {
                id: 255,
                name: "custom_block",
                hardness: 2.0,
                resistance: 10.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            is_liquid: false,
            is_solid: true,
            is_transparent: false,
        };
        assert!(registry.register_custom(custom));
        assert_eq!(registry.get_by_id(255).unwrap().properties.hardness, 2.0);
    }
}
