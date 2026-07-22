pub mod material;
pub mod properties;
pub mod registry;
pub mod sound;
pub mod states;

pub use registry::{BlockInfo, BlockRegistry};

// Block type definitions — full MC 1.8.9 block registry.
//
// Block IDs match MC 1.8.9 protocol (data value = block_id << 4 | metadata).
// Texture atlas indices map to 16×16 tiles in a 16×16 grid (256 tiles max).

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Block {
    Air,
    Stone,
    Grass,
    Dirt,
    Cobblestone,
    Planks,
    Sapling,
    Bedrock,
    FlowingWater,
    StillWater,
    FlowingLava,
    StillLava,
    Sand,
    Gravel,
    GoldOre,
    IronOre,
    CoalOre,
    Log,
    Log2,
    Leaves,
    Leaves2,
    Sponge,
    Glass,
    LapisOre,
    LapisBlock,
    Dispenser,
    Sandstone,
    NoteBlock,
    Bed,
    PoweredRail,
    DetectorRail,
    StickyPiston,
    Cobweb,
    TallGrass,
    DeadBush,
    Piston,
    PistonHead,
    Wool,
    PistonExtension,
    Dandelion,
    Flower,
    BrownMushroom,
    RedMushroom,
    GoldBlock,
    IronBlock,
    DoubleStoneSlab,
    StoneSlab,
    Bricks,
    Tnt,
    Bookshelf,
    MossyCobblestone,
    Obsidian,
    Torch,
    Fire,
    MobSpawner,
    OakStairs,
    Chest,
    RedstoneWire,
    DiamondOre,
    DiamondBlock,
    CraftingTable,
    Wheat,
    Farmland,
    Furnace,
    LitFurnace,
    StandingSign,
    OakDoor,
    Ladder,
    Rail,
    CobblestoneStairs,
    WallSign,
    Lever,
    StonePressurePlate,
    IronDoor,
    WoodenPressurePlate,
    RedstoneOre,
    LitRedstoneOre,
    UnlitRedstoneTorch,
    RedstoneTorch,
    StoneButton,
    SnowLayer,
    Ice,
    SnowBlock,
    Cactus,
    Clay,
    SugarCane,
    Jukebox,
    OakFence,
    Pumpkin,
    Netherrack,
    SoulSand,
    Glowstone,
    NetherPortal,
    JackOLantern,
    Cake,
    UnpoweredRepeater,
    PoweredRepeater,
    Trapdoor,
    MonsterEgg,
    StoneBricks,
    BrownMushroomBlock,
    RedMushroomBlock,
    IronBars,
    GlassPane,
    MelonBlock,
    PumpkinStem,
    MelonStem,
    Vine,
    OakFenceGate,
    BrickStairs,
    StoneBrickStairs,
    Mycelium,
    LilyPad,
    NetherBrick,
    NetherBrickFence,
    NetherBrickStairs,
    NetherWart,
    EnchantingTable,
    BrewingStand,
    Cauldron,
    EndPortal,
    EndPortalFrame,
    EndStone,
    DragonEgg,
    RedstoneLamp,
    LitRedstoneLamp,
    DoubleWoodSlab,
    WoodSlab,
    Cocoa,
    SandstoneStairs,
    EmeraldOre,
    EnderChest,
    TripwireHook,
    Tripwire,
    EmeraldBlock,
    SpruceStairs,
    BirchStairs,
    JungleStairs,
    CommandBlock,
    Beacon,
    CobblestoneWall,
    FlowerPot,
    Carrots,
    Potatoes,
    WoodenButton,
    Skull,
    Anvil,
    TrappedChest,
    LightWeightedPressurePlate,
    HeavyWeightedPressurePlate,
    UnpoweredComparator,
    PoweredComparator,
    DaylightDetector,
    RedstoneBlock,
    QuartzOre,
    Hopper,
    QuartzBlock,
    QuartzStairs,
    ActivatorRail,
    Dropper,
    StainedClay,
    StainedGlassPane,
    Leaves3,
    Log3,
    HayBlock,
    Carpet,
    HardenedClay,
    CoalBlock,
    PackedIce,
    LargeFlower,
    // Metadata variants (stored as separate entries for simplicity)
    StoneGranite,
    StoneDiorite,
    StoneAndesite,
    GrassSnowy, // grass with snow on top
    StainedGlass,
    AcaciaStairs,
    DarkOakStairs,
    SlimeBlock,
    Barrier,
    IronTrapdoor,
    Prismarine,
    SeaLantern,
    StandingBanner,
    WallBanner,
    DaylightDetectorInverted,
    RedSandstone,
    RedSandstoneStairs,
    DoubleStoneSlab2,
    StoneSlab2,
    SpruceFenceGate,
    BirchFenceGate,
    JungleFenceGate,
    DarkOakFenceGate,
    AcaciaFenceGate,
    SpruceFence,
    BirchFence,
    JungleFence,
    DarkOakFence,
    AcaciaFence,
    SpruceDoor,
    BirchDoor,
    JungleDoor,
    AcaciaDoor,
    DarkOakDoor,
}

impl Block {
    #[inline]
    pub fn is_liquid(self) -> bool {
        matches!(
            self,
            Block::FlowingWater | Block::StillWater | Block::FlowingLava | Block::StillLava
        )
    }

    #[inline]
    pub fn is_solid(self) -> bool {
        !matches!(
            self,
            Block::Air
                | Block::FlowingWater
                | Block::StillWater
                | Block::FlowingLava
                | Block::StillLava
                | Block::Fire
                | Block::Cobweb
                | Block::NetherPortal
                | Block::EndPortal
                | Block::Torch
                | Block::RedstoneTorch
                | Block::UnlitRedstoneTorch
                | Block::RedstoneWire
                | Block::Sapling
                | Block::Dandelion
                | Block::Flower
                | Block::LargeFlower
                | Block::BrownMushroom
                | Block::RedMushroom
                | Block::DeadBush
                | Block::TallGrass
                | Block::Ladder
                | Block::Vine
                | Block::Rail
                | Block::PoweredRail
                | Block::DetectorRail
                | Block::ActivatorRail
                | Block::SnowLayer
                | Block::Carpet
                | Block::SugarCane
                | Block::Cactus
                | Block::LilyPad
                | Block::Carrots
                | Block::Potatoes
                | Block::Wheat
                | Block::NetherWart
                | Block::PumpkinStem
                | Block::MelonStem
                | Block::Tripwire
                | Block::TripwireHook
                | Block::StandingSign
                | Block::WallSign
                | Block::FlowerPot
                | Block::Lever
                | Block::StoneButton
                | Block::WoodenButton
                | Block::StonePressurePlate
                | Block::WoodenPressurePlate
                | Block::LightWeightedPressurePlate
                | Block::HeavyWeightedPressurePlate
                | Block::PistonHead
                | Block::PistonExtension
                | Block::StoneSlab
                | Block::WoodSlab
                | Block::OakStairs
                | Block::SpruceStairs
                | Block::BirchStairs
                | Block::JungleStairs
                | Block::AcaciaStairs
                | Block::DarkOakStairs
                | Block::BrickStairs
                | Block::StoneBrickStairs
                | Block::SandstoneStairs
                | Block::NetherBrickStairs
                | Block::QuartzStairs
                | Block::RedSandstoneStairs
                | Block::CobblestoneStairs
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
                | Block::NetherBrickFence
                | Block::CobblestoneWall
                | Block::GlassPane
                | Block::StainedGlassPane
                | Block::IronBars
                | Block::OakDoor
                | Block::SpruceDoor
                | Block::BirchDoor
                | Block::JungleDoor
                | Block::AcaciaDoor
                | Block::DarkOakDoor
                | Block::IronDoor
                | Block::Trapdoor
                | Block::IronTrapdoor
                | Block::MobSpawner
                | Block::EnchantingTable
                | Block::BrewingStand
                | Block::Cauldron
                | Block::EndPortalFrame
                | Block::Skull
                | Block::Anvil
                | Block::Farmland
                | Block::Cake
                | Block::Bed
                | Block::Hopper
                | Block::UnpoweredRepeater
                | Block::PoweredRepeater
                | Block::UnpoweredComparator
                | Block::PoweredComparator
                | Block::DaylightDetector
                | Block::DaylightDetectorInverted
                | Block::Cocoa
                | Block::Chest
                | Block::TrappedChest
                | Block::StandingBanner
                | Block::WallBanner
        )
    }

    /// Vanilla `Block.isCollidable` — every non-air, non-liquid block can be
    /// targeted by the player's crosshair raycast.
    #[inline]
    pub fn can_collide_check(self) -> bool {
        self != Block::Air
    }

    #[inline]
    pub fn is_transparent(self) -> bool {
        !self.is_solid()
            || matches!(
                self,
                Block::Glass
                    | Block::GlassPane
                    | Block::StainedGlassPane
                    | Block::Ice
                    | Block::Leaves
                    | Block::Leaves2
                    | Block::Leaves3
                    | Block::StainedGlass
                    | Block::SlimeBlock
                    | Block::Barrier
                    | Block::IronTrapdoor
                    | Block::StandingBanner
                    | Block::WallBanner
            )
    }

    /// Returns (top_tile, bottom_tile, side_tile) indices into the texture atlas.
    /// Uses the dynamic atlas's name→index lookup so indices match whatever
    /// textures were loaded at startup.
    pub fn tiles(self) -> (usize, usize, usize) {
        use crate::assets::texture::tex_idx as t;
        match self {
            Block::Air => (0, 0, 0),
            Block::Stone => (t("stone"), t("stone"), t("stone")),
            Block::Grass => (t("grass_top"), t("dirt"), t("grass_side")),
            Block::GrassSnowy => (t("grass_side_snowed"), t("dirt"), t("grass_side_snowed")),
            Block::Dirt => (t("dirt"), t("dirt"), t("dirt")),
            Block::Cobblestone => (t("cobblestone"), t("cobblestone"), t("cobblestone")),
            Block::Planks => (t("planks_oak"), t("planks_oak"), t("planks_oak")),
            Block::Sapling => (t("sapling_oak"), t("sapling_oak"), t("sapling_oak")),
            Block::Bedrock => (t("bedrock"), t("bedrock"), t("bedrock")),
            Block::FlowingWater | Block::StillWater => {
                (t("water_still"), t("water_still"), t("water_flow"))
            }
            Block::FlowingLava | Block::StillLava => {
                (t("lava_still"), t("lava_still"), t("lava_flow"))
            }
            Block::Sand => (t("sand"), t("sand"), t("sand")),
            Block::Gravel => (t("gravel"), t("gravel"), t("gravel")),
            Block::GoldOre => (t("gold_ore"), t("gold_ore"), t("gold_ore")),
            Block::IronOre => (t("iron_ore"), t("iron_ore"), t("iron_ore")),
            Block::CoalOre => (t("coal_ore"), t("coal_ore"), t("coal_ore")),
            Block::Log => (t("log_oak_top"), t("log_oak_top"), t("log_oak")),
            Block::Log2 => (t("log_oak_top"), t("log_oak_top"), t("log_oak")),
            Block::Leaves => (t("leaves_oak"), t("leaves_oak"), t("leaves_oak")),
            Block::Leaves2 => (t("leaves_oak"), t("leaves_oak"), t("leaves_oak")),
            Block::Sponge => (t("sponge"), t("sponge"), t("sponge")),
            Block::Glass => (t("glass"), t("glass"), t("glass")),
            Block::LapisOre => (t("lapis_ore"), t("lapis_ore"), t("lapis_ore")),
            Block::LapisBlock => (t("lapis_block"), t("lapis_block"), t("lapis_block")),
            Block::Sandstone => (
                t("sandstone_top"),
                t("sandstone_bottom"),
                t("sandstone_normal"),
            ),
            Block::Wool => (
                t("wool_colored_white"),
                t("wool_colored_white"),
                t("wool_colored_white"),
            ),
            Block::GoldBlock => (t("gold_block"), t("gold_block"), t("gold_block")),
            Block::IronBlock => (t("iron_block"), t("iron_block"), t("iron_block")),
            Block::Bricks => (t("brick"), t("brick"), t("brick")),
            Block::Tnt => (t("tnt_top"), t("tnt_bottom"), t("tnt_side")),
            Block::Bookshelf => (t("planks_oak"), t("planks_oak"), t("bookshelf")),
            Block::MossyCobblestone => (
                t("cobblestone_mossy"),
                t("cobblestone_mossy"),
                t("cobblestone_mossy"),
            ),
            Block::Obsidian => (t("obsidian"), t("obsidian"), t("obsidian")),
            Block::DiamondOre => (t("diamond_ore"), t("diamond_ore"), t("diamond_ore")),
            Block::DiamondBlock => (t("diamond_block"), t("diamond_block"), t("diamond_block")),
            Block::CraftingTable => (
                t("crafting_table_top"),
                t("planks_oak"),
                t("crafting_table_side"),
            ),
            Block::Furnace | Block::LitFurnace => {
                (t("furnace_side"), t("furnace_top"), t("furnace_front_off"))
            }
            Block::Chest => (t("chest_top"), t("chest_top"), t("chest_side")),
            Block::SnowBlock => (t("snow"), t("snow"), t("snow")),
            Block::Ice => (t("ice"), t("ice"), t("ice")),
            Block::Cactus => (t("cactus_top"), t("cactus_bottom"), t("cactus_side")),
            Block::Clay => (t("clay"), t("clay"), t("clay")),
            Block::Jukebox => (t("jukebox_top"), t("planks_oak"), t("jukebox_side")),
            Block::Pumpkin => (t("pumpkin_top"), t("pumpkin_top"), t("pumpkin_side")),
            Block::Netherrack => (t("netherrack"), t("netherrack"), t("netherrack")),
            Block::SoulSand => (t("soul_sand"), t("soul_sand"), t("soul_sand")),
            Block::Glowstone => (t("glowstone"), t("glowstone"), t("glowstone")),
            Block::JackOLantern => (t("pumpkin_top"), t("pumpkin_top"), t("pumpkin_face_on")),
            Block::StoneBricks => (t("stonebrick"), t("stonebrick"), t("stonebrick")),
            Block::MelonBlock => (t("melon_top"), t("melon_top"), t("melon_side")),
            Block::NetherBrick => (t("nether_brick"), t("nether_brick"), t("nether_brick")),
            Block::EndStone => (t("end_stone"), t("end_stone"), t("end_stone")),
            Block::EmeraldOre => (t("emerald_ore"), t("emerald_ore"), t("emerald_ore")),
            Block::EmeraldBlock => (t("emerald_block"), t("emerald_block"), t("emerald_block")),
            Block::RedstoneBlock => (
                t("redstone_block"),
                t("redstone_block"),
                t("redstone_block"),
            ),
            Block::QuartzOre => (t("quartz_ore"), t("quartz_ore"), t("quartz_ore")),
            Block::QuartzBlock => (
                t("quartz_block_top"),
                t("quartz_block_bottom"),
                t("quartz_block_side"),
            ),
            Block::StainedClay => (t("hardened_clay"), t("hardened_clay"), t("hardened_clay")),
            Block::HardenedClay => (t("hardened_clay"), t("hardened_clay"), t("hardened_clay")),
            Block::CoalBlock => (t("coal_block"), t("coal_block"), t("coal_block")),
            Block::HayBlock => (t("hay_block_top"), t("hay_block_top"), t("hay_block_side")),
            Block::PackedIce => (t("ice_packed"), t("ice_packed"), t("ice_packed")),
            Block::Anvil => (t("anvil_top_damaged_0"), t("anvil_base"), t("anvil_base")),
            Block::TrappedChest => (t("chest_top"), t("chest_top"), t("chest_side")),
            Block::Hopper => (
                t("hopper_outside"),
                t("hopper_outside"),
                t("hopper_outside"),
            ),
            Block::Dropper => (t("furnace_side"), t("furnace_top"), t("furnace_front_off")),
            Block::CommandBlock => (t("command_block"), t("command_block"), t("command_block")),
            Block::Beacon => (t("beacon"), t("beacon"), t("beacon")),
            Block::DaylightDetector => (
                t("daylight_detector_top"),
                t("daylight_detector_top"),
                t("daylight_detector_side"),
            ),
            Block::StoneGranite => (t("stone_granite"), t("stone_granite"), t("stone_granite")),
            Block::StoneDiorite => (t("stone_diorite"), t("stone_diorite"), t("stone_diorite")),
            Block::StoneAndesite => (
                t("stone_andesite"),
                t("stone_andesite"),
                t("stone_andesite"),
            ),
            Block::OakFence | Block::OakFenceGate => {
                (t("planks_oak"), t("planks_oak"), t("planks_oak"))
            }
            Block::IronBars => (t("iron_bars"), t("iron_bars"), t("iron_bars")),
            Block::GlassPane => (t("glass"), t("glass"), t("glass")),
            Block::StainedGlassPane => (t("glass"), t("glass"), t("glass")),
            Block::CobblestoneWall => (t("cobblestone"), t("cobblestone"), t("cobblestone")),
            Block::OakStairs | Block::SpruceStairs | Block::BirchStairs | Block::JungleStairs => {
                (t("planks_oak"), t("planks_oak"), t("planks_oak"))
            }
            Block::BrickStairs => (t("brick"), t("brick"), t("brick")),
            Block::StoneBrickStairs => (t("stonebrick"), t("stonebrick"), t("stonebrick")),
            Block::SandstoneStairs => (
                t("sandstone_top"),
                t("sandstone_bottom"),
                t("sandstone_normal"),
            ),
            Block::NetherBrickStairs => (t("nether_brick"), t("nether_brick"), t("nether_brick")),
            Block::QuartzStairs => (
                t("quartz_block_top"),
                t("quartz_block_bottom"),
                t("quartz_block_side"),
            ),
            Block::Carpet => (
                t("wool_colored_white"),
                t("wool_colored_white"),
                t("wool_colored_white"),
            ),
            Block::Mycelium => (t("mycelium_top"), t("dirt"), t("mycelium_side")),
            Block::LilyPad => (t("waterlily"), t("waterlily"), t("waterlily")),
            Block::Vine => (t("vine"), t("vine"), t("vine")),
            Block::NetherWart => (
                t("nether_wart_stage_2"),
                t("nether_wart_stage_2"),
                t("nether_wart_stage_2"),
            ),
            Block::EnchantingTable => (
                t("enchanting_table_top"),
                t("enchanting_table_bottom"),
                t("enchanting_table_side"),
            ),
            Block::BrewingStand => (
                t("brewing_stand"),
                t("brewing_stand_base"),
                t("brewing_stand"),
            ),
            Block::Cauldron => (
                t("cauldron_inner"),
                t("cauldron_bottom"),
                t("cauldron_side"),
            ),
            Block::EndPortalFrame => (t("endframe_top"), t("endframe_top"), t("endframe_side")),
            Block::DragonEgg => (t("dragon_egg"), t("dragon_egg"), t("dragon_egg")),
            Block::RedstoneLamp | Block::LitRedstoneLamp => (
                t("redstone_lamp_off"),
                t("redstone_lamp_off"),
                t("redstone_lamp_off"),
            ),
            Block::Cocoa => (t("cocoa_stage_2"), t("cocoa_stage_2"), t("cocoa_stage_2")),
            Block::TripwireHook => (
                t("trip_wire_source"),
                t("trip_wire_source"),
                t("trip_wire_source"),
            ),
            Block::Skull => (t("stone"), t("stone"), t("stone")),
            Block::FlowerPot => (t("flower_pot"), t("flower_pot"), t("flower_pot")),
            Block::LargeFlower => (
                t("double_plant_sunflower_top"),
                t("double_plant_sunflower_bottom"),
                t("double_plant_sunflower_front"),
            ),
            Block::DoubleStoneSlab | Block::StoneSlab => (
                t("stone_slab_top"),
                t("stone_slab_top"),
                t("stone_slab_side"),
            ),
            Block::DoubleWoodSlab | Block::WoodSlab => {
                (t("planks_oak"), t("planks_oak"), t("planks_oak"))
            }
            Block::Piston => (t("piston_top_normal"), t("piston_bottom"), t("piston_side")),
            Block::StickyPiston => (t("piston_top_sticky"), t("piston_bottom"), t("piston_side")),
            Block::PistonHead => (
                t("piston_top_normal"),
                t("piston_top_normal"),
                t("piston_side"),
            ),
            // Cross-shape plants
            Block::Dandelion => (
                t("flower_dandelion"),
                t("flower_dandelion"),
                t("flower_dandelion"),
            ),
            Block::Flower => (t("flower_rose"), t("flower_rose"), t("flower_rose")),
            Block::BrownMushroom => (
                t("mushroom_brown"),
                t("mushroom_brown"),
                t("mushroom_brown"),
            ),
            Block::RedMushroom => (t("mushroom_red"), t("mushroom_red"), t("mushroom_red")),
            Block::TallGrass => (t("tallgrass"), t("tallgrass"), t("tallgrass")),
            Block::DeadBush => (t("deadbush"), t("deadbush"), t("deadbush")),
            // Crops
            Block::Wheat => (t("wheat_stage_7"), t("wheat_stage_7"), t("wheat_stage_7")),
            Block::Carrots => (
                t("carrots_stage_3"),
                t("carrots_stage_3"),
                t("carrots_stage_3"),
            ),
            Block::Potatoes => (
                t("potatoes_stage_3"),
                t("potatoes_stage_3"),
                t("potatoes_stage_3"),
            ),
            Block::PumpkinStem => (
                t("pumpkin_stem_disconnected"),
                t("pumpkin_stem_disconnected"),
                t("pumpkin_stem_disconnected"),
            ),
            Block::MelonStem => (
                t("melon_stem_disconnected"),
                t("melon_stem_disconnected"),
                t("melon_stem_disconnected"),
            ),
            // Rails
            Block::Rail => (t("rail_normal"), t("rail_normal"), t("rail_normal")),
            Block::PoweredRail => (t("rail_golden"), t("rail_golden"), t("rail_golden")),
            Block::DetectorRail => (t("rail_detector"), t("rail_detector"), t("rail_detector")),
            Block::ActivatorRail => (
                t("rail_activator"),
                t("rail_activator"),
                t("rail_activator"),
            ),
            // Signs
            Block::StandingSign | Block::WallSign => {
                (t("planks_oak"), t("planks_oak"), t("planks_oak"))
            }
            // Redstone
            Block::RedstoneWire => (
                t("redstone_dust_cross"),
                t("redstone_dust_cross"),
                t("redstone_dust_cross"),
            ),
            Block::Lever => (t("lever"), t("lever"), t("lever")),
            Block::StoneButton => (t("stone"), t("stone"), t("stone")),
            Block::WoodenButton => (t("planks_oak"), t("planks_oak"), t("planks_oak")),
            Block::StonePressurePlate | Block::WoodenPressurePlate => {
                (t("stone"), t("stone"), t("stone"))
            }
            Block::LightWeightedPressurePlate => {
                (t("gold_block"), t("gold_block"), t("gold_block"))
            }
            Block::HeavyWeightedPressurePlate => {
                (t("iron_block"), t("iron_block"), t("iron_block"))
            }
            Block::UnpoweredRepeater | Block::PoweredRepeater => {
                (t("repeater_off"), t("repeater_off"), t("repeater_off"))
            }
            Block::UnpoweredComparator | Block::PoweredComparator => (
                t("comparator_off"),
                t("comparator_off"),
                t("comparator_off"),
            ),
            // Misc
            Block::Farmland => (t("farmland_wet"), t("dirt"), t("dirt")),
            Block::Cake => (t("cake_top"), t("cake_bottom"), t("cake_side")),
            Block::Bed => (t("bed_feet_top"), t("bed_feet_top"), t("bed_feet_side")),
            Block::Fire => (t("fire_layer_0"), t("fire_layer_0"), t("fire_layer_0")),
            Block::NetherPortal => (t("portal"), t("portal"), t("portal")),
            Block::EndPortal => (t("portal"), t("portal"), t("portal")),
            Block::Ladder => (t("ladder"), t("ladder"), t("ladder")),
            Block::Cobweb => (t("web"), t("web"), t("web")),
            Block::Dispenser => (
                t("furnace_side"),
                t("furnace_top"),
                t("dispenser_front_horizontal"),
            ),
            Block::NoteBlock => (t("noteblock"), t("noteblock"), t("noteblock")),
            Block::MobSpawner => (t("mob_spawner"), t("mob_spawner"), t("mob_spawner")),
            Block::RedstoneOre | Block::LitRedstoneOre => {
                (t("redstone_ore"), t("redstone_ore"), t("redstone_ore"))
            }
            Block::UnlitRedstoneTorch | Block::RedstoneTorch => (
                t("redstone_torch_off"),
                t("redstone_torch_off"),
                t("redstone_torch_off"),
            ),
            Block::SnowLayer => (t("snow"), t("snow"), t("snow")),
            Block::SugarCane => (t("reeds"), t("reeds"), t("reeds")),
            Block::Torch => (t("torch_on"), t("torch_on"), t("torch_on")),
            Block::MonsterEgg => (t("stone"), t("stone"), t("stone")),
            Block::BrownMushroomBlock => (
                t("mushroom_block_skin_brown"),
                t("mushroom_block_skin_brown"),
                t("mushroom_block_skin_brown"),
            ),
            Block::RedMushroomBlock => (
                t("mushroom_block_skin_red"),
                t("mushroom_block_skin_red"),
                t("mushroom_block_skin_red"),
            ),
            Block::IronDoor => (
                t("door_iron_upper"),
                t("door_iron_lower"),
                t("door_iron_upper"),
            ),
            Block::OakDoor => (
                t("door_wood_upper"),
                t("door_wood_lower"),
                t("door_wood_upper"),
            ),
            Block::Trapdoor => (t("trapdoor"), t("trapdoor"), t("trapdoor")),
            Block::NetherBrickFence => (t("nether_brick"), t("nether_brick"), t("nether_brick")),
            Block::Tripwire => (t("trip_wire"), t("trip_wire"), t("trip_wire")),
            Block::Leaves3 => (t("leaves_acacia"), t("leaves_acacia"), t("leaves_acacia")),
            Block::Log3 => (t("log_acacia_top"), t("log_acacia_top"), t("log_acacia")),
            Block::EnderChest => (t("obsidian"), t("obsidian"), t("obsidian")),
            Block::PistonExtension => (
                t("piston_top_sticky"),
                t("piston_top_sticky"),
                t("piston_top_sticky"),
            ),
            Block::StainedGlass => (t("glass"), t("glass"), t("glass")),
            Block::AcaciaStairs => (t("planks_acacia"), t("planks_acacia"), t("planks_acacia")),
            Block::DarkOakStairs => (
                t("planks_big_oak"),
                t("planks_big_oak"),
                t("planks_big_oak"),
            ),
            Block::SlimeBlock => (t("slime"), t("slime"), t("slime")),
            Block::Barrier => (t("stone"), t("stone"), t("stone")),
            Block::IronTrapdoor => (t("trapdoor"), t("trapdoor"), t("trapdoor")),
            Block::Prismarine => (
                t("prismarine_rough"),
                t("prismarine_rough"),
                t("prismarine_rough"),
            ),
            Block::SeaLantern => (t("sea_lantern"), t("sea_lantern"), t("sea_lantern")),
            Block::StandingBanner | Block::WallBanner => {
                (t("planks_oak"), t("planks_oak"), t("planks_oak"))
            }
            Block::DaylightDetectorInverted => (
                t("daylight_detector_inverted_top"),
                t("daylight_detector_inverted_top"),
                t("daylight_detector_side"),
            ),
            Block::RedSandstone => (
                t("red_sandstone_top"),
                t("red_sandstone_bottom"),
                t("red_sandstone_normal"),
            ),
            Block::RedSandstoneStairs => (
                t("red_sandstone_top"),
                t("red_sandstone_bottom"),
                t("red_sandstone_normal"),
            ),
            Block::DoubleStoneSlab2 | Block::StoneSlab2 => (
                t("red_sandstone_top"),
                t("red_sandstone_top"),
                t("red_sandstone_normal"),
            ),
            Block::SpruceFenceGate => (t("planks_spruce"), t("planks_spruce"), t("planks_spruce")),
            Block::BirchFenceGate => (t("planks_birch"), t("planks_birch"), t("planks_birch")),
            Block::JungleFenceGate => (t("planks_jungle"), t("planks_jungle"), t("planks_jungle")),
            Block::DarkOakFenceGate => (
                t("planks_big_oak"),
                t("planks_big_oak"),
                t("planks_big_oak"),
            ),
            Block::AcaciaFenceGate => (t("planks_acacia"), t("planks_acacia"), t("planks_acacia")),
            Block::SpruceFence => (t("planks_spruce"), t("planks_spruce"), t("planks_spruce")),
            Block::BirchFence => (t("planks_birch"), t("planks_birch"), t("planks_birch")),
            Block::JungleFence => (t("planks_jungle"), t("planks_jungle"), t("planks_jungle")),
            Block::DarkOakFence => (
                t("planks_big_oak"),
                t("planks_big_oak"),
                t("planks_big_oak"),
            ),
            Block::AcaciaFence => (t("planks_acacia"), t("planks_acacia"), t("planks_acacia")),
            Block::SpruceDoor => (
                t("door_spruce_upper"),
                t("door_spruce_lower"),
                t("door_spruce_upper"),
            ),
            Block::BirchDoor => (
                t("door_birch_upper"),
                t("door_birch_lower"),
                t("door_birch_upper"),
            ),
            Block::JungleDoor => (
                t("door_jungle_upper"),
                t("door_jungle_lower"),
                t("door_jungle_upper"),
            ),
            Block::AcaciaDoor => (
                t("door_acacia_upper"),
                t("door_acacia_lower"),
                t("door_acacia_upper"),
            ),
            Block::DarkOakDoor => (
                t("door_dark_oak_upper"),
                t("door_dark_oak_lower"),
                t("door_dark_oak_upper"),
            ),
            // Fallback
            _ => (t("stone"), t("stone"), t("stone")),
        }
    }
}
