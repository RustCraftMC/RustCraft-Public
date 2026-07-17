//! Block model lookup — maps (block_id, metadata) to baked JSON models.
//!
//! Uses the vanilla blockstate JSON files (assets/minecraft/blockstates/*.json)
//! to select the correct model variant for each block state, then resolves it
//! through the ModelRegistry (parent inheritance, texture variables, per-face
//! UVs, cullface) to produce a BakedModel ready for the mesh builder.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::assets::model::{BlockModel, ModelRegistry};

/// Global model cache — initialised at startup, replaced on resource pack reload.
static MODEL_CACHE: Mutex<Option<Box<BlockModelCache>>> = Mutex::new(None);

/// Cached baked models keyed by (block_id, metadata).
pub struct BlockModelCache {
    /// (block_id << 4 | meta) → baked model
    models: HashMap<u16, BlockModel>,
    /// Texture name → atlas tile index (populated from the dynamic atlas)
    texture_map: HashMap<String, usize>,
}

impl BlockModelCache {
    /// Build the cache by loading all blockstates and baking all variants.
    pub fn build(registry: &mut ModelRegistry, texture_map: HashMap<String, usize>) -> Self {
        let mut cache = BlockModelCache {
            models: HashMap::new(),
            texture_map,
        };

        let mut block_types = 0usize;
        for block_id in 1u16..=255 {
            let before = cache.models.len();
            for meta in 0u8..16 {
                let Some(blockstate_name) = blockstate_name_for_state(block_id, meta) else {
                    continue;
                };
                let variants = registry.load_blockstate_variants(&blockstate_name);
                let Some((_, model_name, x_rot, y_rot)) =
                    select_state_variant(block_id, meta, &variants)
                else {
                    continue;
                };
                if let Some(model) = registry.bake_model_with_rotation(&model_name, x_rot, y_rot) {
                    cache.models.insert((block_id << 4) | meta as u16, model);
                }
            }
            if cache.models.len() > before {
                block_types += 1;
            }
        }

        log::info!(
            "block model cache ready: states={}, block_types={}",
            cache.models.len(),
            block_types,
        );

        cache
    }

    /// Get the baked model for a block state (block_id, metadata).
    /// Returns None if no model is cached (falls back to old system).
    pub fn get_model(&self, block_id: u16, meta: u8) -> Option<&BlockModel> {
        let key = (block_id << 4) | (meta as u16);
        if let Some(m) = self.models.get(&key) {
            return Some(m);
        }
        let key0 = block_id << 4;
        if let Some(m) = self.models.get(&key0) {
            return Some(m);
        }
        None
    }

    /// Look up atlas tile index for a texture name.
    pub fn texture_index(&self, name: &str) -> usize {
        // Try exact name
        if let Some(&idx) = self.texture_map.get(name) {
            return idx;
        }
        // Try stripping "blocks/" prefix
        let stripped = name.strip_prefix("blocks/").unwrap_or(name);
        if let Some(&idx) = self.texture_map.get(stripped) {
            return idx;
        }
        self.texture_map
            .get("__missing")
            .copied()
            .unwrap_or(crate::assets::texture::MISSING_TILE_INDEX)
    }

    /// Initialise or replace the global cache. Previous cache is dropped.
    pub fn init(cache: BlockModelCache) {
        if let Ok(mut lock) = MODEL_CACHE.lock() {
            *lock = Some(Box::new(cache));
        }
    }

    /// Access the global cache (panics if not initialised).
    /// The returned reference must not be held across calls to [`init`].
    pub fn global() -> &'static BlockModelCache {
        let guard = MODEL_CACHE.lock().unwrap();
        let bx = guard.as_ref().expect("BlockModelCache not initialised");
        // SAFETY: Callers use the reference immediately and never hold it
        // across an `init()` call (which only occurs during resource reload).
        unsafe { &*(bx.as_ref() as *const BlockModelCache) }
    }

    /// Check if the global cache is available.
    pub fn is_available() -> bool {
        MODEL_CACHE
            .lock()
            .ok()
            .map(|l| l.is_some())
            .unwrap_or(false)
    }
}

fn select_state_variant(
    block_id: u16,
    meta: u8,
    variants: &[(String, String, f32, f32)],
) -> Option<(String, String, f32, f32)> {
    if variants.is_empty() {
        return None;
    }
    let desired = canonical_variant_key(&meta_to_variant_key(block_id, meta));
    let exact = variants
        .iter()
        .find(|(key, _, _, _)| canonical_variant_key(key) == desired);
    let stage = format!("stage={}", (meta >> 3) & 1);
    let half = if meta & 0x08 != 0 {
        "half=upper"
    } else {
        "half=bottom"
    };
    let fallback = exact
        .or_else(|| variants.iter().find(|(key, _, _, _)| key == "normal"))
        .or_else(|| variants.iter().find(|(key, _, _, _)| key == &stage))
        .or_else(|| variants.iter().find(|(key, _, _, _)| key == half))
        .or_else(|| {
            variants.iter().find(|(key, _, _, _)| {
                key.contains("east=false")
                    && key.contains("north=false")
                    && key.contains("south=false")
                    && key.contains("west=false")
            })
        })
        .or_else(|| variants.first())?;
    Some(fallback.clone())
}

fn canonical_variant_key(key: &str) -> String {
    let mut parts: Vec<_> = key.split(',').map(str::trim).collect();
    parts.sort_unstable();
    parts.join(",")
}

fn blockstate_name_for_state(block_id: u16, meta: u8) -> Option<String> {
    const WOODS: [&str; 6] = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"];
    const STONE: [&str; 7] = [
        "stone",
        "granite",
        "smooth_granite",
        "diorite",
        "smooth_diorite",
        "andesite",
        "smooth_andesite",
    ];
    const STONE_SLABS: [&str; 8] = [
        "stone",
        "sandstone",
        "wood_old",
        "cobblestone",
        "brick",
        "stone_brick",
        "nether_brick",
        "quartz",
    ];
    const STONE_BRICKS: [&str; 4] = [
        "stonebrick",
        "mossy_stonebrick",
        "cracked_stonebrick",
        "chiseled_stonebrick",
    ];
    const MONSTER_EGGS: [&str; 6] = [
        "stone_monster_egg",
        "cobblestone_monster_egg",
        "stone_brick_monster_egg",
        "mossy_brick_monster_egg",
        "cracked_brick_monster_egg",
        "chiseled_brick_monster_egg",
    ];

    let name = match block_id {
        1 => STONE[(meta as usize).min(6)].to_string(),
        3 => ["dirt", "coarse_dirt", "podzol"][(meta as usize).min(2)].to_string(),
        5 => format!("{}_planks", WOODS[(meta as usize).min(5)]),
        6 => format!("{}_sapling", WOODS[((meta & 7) as usize).min(5)]),
        12 => if meta & 1 == 0 { "sand" } else { "red_sand" }.to_string(),
        17 => format!("{}_log", WOODS[(meta & 3) as usize]),
        18 => format!("{}_leaves", WOODS[(meta & 3) as usize]),
        24 => ["sandstone", "chiseled_sandstone", "smooth_sandstone"][(meta as usize).min(2)]
            .to_string(),
        31 => if meta & 3 == 2 { "fern" } else { "tall_grass" }.to_string(),
        35 => format!("{}_wool", DYE_COLORS[meta as usize]),
        38 => [
            "poppy",
            "blue_orchid",
            "allium",
            "houstonia",
            "red_tulip",
            "orange_tulip",
            "white_tulip",
            "pink_tulip",
            "oxeye_daisy",
        ][(meta as usize).min(8)]
        .to_string(),
        43 | 44 => format!(
            "{}_{}slab",
            STONE_SLABS[(meta as usize).min(7)],
            if block_id == 43 { "double_" } else { "" }
        ),
        95 => format!("{}_stained_glass", DYE_COLORS[meta as usize]),
        97 => MONSTER_EGGS[(meta as usize).min(5)].to_string(),
        98 => STONE_BRICKS[(meta as usize).min(3)].to_string(),
        125 | 126 => format!(
            "{}_{}slab",
            WOODS[((meta & 7) as usize).min(5)],
            if block_id == 125 { "double_" } else { "" }
        ),
        155 => ["quartz_block", "chiseled_quartz_block", "quartz_column"][(meta as usize).min(2)]
            .to_string(),
        159 => format!("{}_stained_hardened_clay", DYE_COLORS[meta as usize]),
        160 => format!("{}_stained_glass_pane", DYE_COLORS[meta as usize]),
        161 => format!("{}_leaves", WOODS[4 + (meta & 1) as usize]),
        162 => format!("{}_log", WOODS[4 + (meta & 1) as usize]),
        168 => ["prismarine", "prismarine_bricks", "dark_prismarine"][(meta as usize).min(2)]
            .to_string(),
        171 => format!("{}_carpet", DYE_COLORS[meta as usize]),
        175 => [
            "sunflower",
            "syringa",
            "double_grass",
            "double_fern",
            "double_rose",
            "paeonia",
        ][((meta & 0x07) as usize).min(5)]
        .to_string(),
        179 => [
            "red_sandstone",
            "chiseled_red_sandstone",
            "smooth_red_sandstone",
        ][(meta as usize).min(2)]
        .to_string(),
        181 | 182 => format!(
            "red_sandstone_{}slab",
            if block_id == 181 { "double_" } else { "" }
        ),
        _ => block_id_to_name(block_id)?.to_string(),
    };
    Some(name)
}

// =========================================================================
// Block ID → blockstate file name mapping (vanilla MC 1.8.9)
// =========================================================================

/// For blocks with split blockstate files (one JSON per variant), returns all file names.
/// Returns None for blocks that use a single blockstate file (handled by block_id_to_name).
fn split_blockstate_names(id: u16) -> Option<Vec<&'static str>> {
    match id {
        // RedFlower — each flower type has its own blockstate file
        38 => Some(vec![
            "poppy",
            "blue_orchid",
            "allium",
            "houstonia",
            "red_tulip",
            "orange_tulip",
            "white_tulip",
            "pink_tulip",
            "oxeye_daisy",
        ]),
        // DoublePlant — each plant type has its own blockstate file
        175 => Some(vec![
            "sunflower",
            "syringa",
            "double_grass",
            "double_fern",
            "double_rose",
            "paeonia",
        ]),
        _ => None,
    }
}

/// Returns the metadata offset for a split blockstate file name.
fn split_file_meta_offset(block_id: u16, file_name: &str) -> u8 {
    match block_id {
        38 => match file_name {
            "poppy" => 0,
            "blue_orchid" => 1,
            "allium" => 2,
            "houstonia" => 3,
            "red_tulip" => 4,
            "orange_tulip" => 5,
            "white_tulip" => 6,
            "pink_tulip" => 7,
            "oxeye_daisy" => 8,
            _ => 0,
        },
        175 => match file_name {
            "sunflower" => 0,
            "syringa" => 1,
            "double_grass" => 2,
            "double_fern" => 3,
            "double_rose" => 4,
            "paeonia" => 5,
            _ => 0,
        },
        _ => 0,
    }
}

/// Maps a MC 1.8.9 block ID to its blockstate JSON file name (without .json).
/// Returns None for air (0) and unknown IDs.
pub fn block_id_to_name(id: u16) -> Option<&'static str> {
    let name = match id {
        0 => return None, // air
        1 => "stone",
        2 => "grass",
        3 => "dirt",
        4 => "cobblestone",
        5 => "planks",
        6 => "sapling",
        7 => "bedrock",
        8 => "flowing_water",
        9 => "water",
        10 => "flowing_lava",
        11 => "lava",
        12 => "sand",
        13 => "gravel",
        14 => "gold_ore",
        15 => "iron_ore",
        16 => "coal_ore",
        17 => "log",
        18 => "leaves",
        19 => "sponge",
        20 => "glass",
        21 => "lapis_ore",
        22 => "lapis_block",
        23 => "dispenser",
        24 => "sandstone",
        25 => "note_block",
        26 => "bed",
        27 => "golden_rail",
        28 => "detector_rail",
        29 => "sticky_piston",
        30 => "web",
        31 => "tall_grass",
        32 => "dead_bush",
        33 => "piston",
        34 => "piston_head",
        35 => "wool",
        36 => "piston_extension",
        37 => "dandelion",
        38 => "red_flower", // handled specially in build() — split files per variant
        39 => "brown_mushroom",
        40 => "red_mushroom",
        41 => "gold_block",
        42 => "iron_block",
        43 => "double_stone_slab",
        44 => "stone_slab",
        45 => "brick_block",
        46 => "tnt",
        47 => "bookshelf",
        48 => "mossy_cobblestone",
        49 => "obsidian",
        50 => "torch",
        51 => "fire",
        52 => "mob_spawner",
        53 => "oak_stairs",
        54 => "chest",
        55 => "redstone_wire",
        56 => "diamond_ore",
        57 => "diamond_block",
        58 => "crafting_table",
        59 => "wheat",
        60 => "farmland",
        61 => "furnace",
        62 => "lit_furnace",
        63 => "standing_sign",
        64 => "wooden_door",
        65 => "ladder",
        66 => "rail",
        67 => "stone_stairs",
        68 => "wall_sign",
        69 => "lever",
        70 => "stone_pressure_plate",
        71 => "iron_door",
        72 => "wooden_pressure_plate",
        73 => "redstone_ore",
        74 => "lit_redstone_ore",
        75 => "unlit_redstone_torch",
        76 => "redstone_torch",
        77 => "stone_button",
        78 => "snow_layer",
        79 => "ice",
        80 => "snow",
        81 => "cactus",
        82 => "clay",
        83 => "reeds",
        84 => "jukebox",
        85 => "fence",
        86 => "pumpkin",
        87 => "netherrack",
        88 => "soul_sand",
        89 => "glowstone",
        90 => "portal",
        91 => "lit_pumpkin",
        92 => "cake",
        93 => "unpowered_repeater",
        94 => "powered_repeater",
        96 => "trapdoor",
        97 => "monster_egg",
        98 => "stonebrick",
        99 => "brown_mushroom_block",
        100 => "red_mushroom_block",
        101 => "iron_bars",
        102 => "glass_pane",
        103 => "melon_block",
        104 => "pumpkin_stem",
        105 => "melon_stem",
        106 => "vine",
        107 => "fence_gate",
        108 => "brick_stairs",
        109 => "stone_brick_stairs",
        110 => "mycelium",
        111 => "waterlily",
        112 => "nether_brick",
        113 => "nether_brick_fence",
        114 => "nether_brick_stairs",
        115 => "nether_wart",
        116 => "enchanting_table",
        117 => "brewing_stand",
        118 => "cauldron",
        119 => "end_portal",
        120 => "end_portal_frame",
        121 => "end_stone",
        122 => "dragon_egg",
        123 => "redstone_lamp",
        124 => "lit_redstone_lamp",
        125 => "double_wooden_slab",
        126 => "wooden_slab",
        127 => "cocoa",
        128 => "sandstone_stairs",
        129 => "emerald_ore",
        130 => "ender_chest",
        131 => "tripwire_hook",
        132 => "tripwire_hook", // tripwire is part of tripwire_hook blockstate
        133 => "emerald_block",
        134 => "spruce_stairs",
        135 => "birch_stairs",
        136 => "jungle_stairs",
        137 => "command_block",
        138 => "beacon",
        139 => "cobblestone_wall",
        140 => "flower_pot",
        141 => "carrots",
        142 => "potatoes",
        143 => "wooden_button",
        144 => "skull",
        145 => "anvil",
        146 => "trapped_chest",
        147 => "light_weighted_pressure_plate",
        148 => "heavy_weighted_pressure_plate",
        149 => "unpowered_comparator",
        150 => "powered_comparator",
        151 => "daylight_detector",
        152 => "redstone_block",
        153 => "quartz_ore",
        154 => "hopper",
        155 => "quartz_block",
        156 => "quartz_stairs",
        157 => "activator_rail",
        158 => "dropper",
        159 => "stained_hardened_clay",
        160 => "stained_glass_pane",
        161 => "leaves2",
        162 => "log2",
        163 => "acacia_stairs",
        164 => "dark_oak_stairs",
        165 => "slime",
        166 => "barrier",
        167 => "iron_trapdoor",
        168 => "prismarine",
        169 => "sea_lantern",
        170 => "hay_block",
        171 => "carpet",
        172 => "hardened_clay",
        173 => "coal_block",
        174 => "packed_ice",
        175 => "double_plant",
        176 => "standing_banner",
        177 => "wall_banner",
        178 => "daylight_detector_inverted",
        179 => "red_sandstone",
        180 => "red_sandstone_stairs",
        181 => "red_sandstone_double_slab",
        182 => "red_sandstone_slab",
        183 => "spruce_fence_gate",
        184 => "birch_fence_gate",
        185 => "jungle_fence_gate",
        186 => "dark_oak_fence_gate",
        187 => "acacia_fence_gate",
        188 => "spruce_fence",
        189 => "birch_fence",
        190 => "jungle_fence",
        191 => "dark_oak_fence",
        192 => "acacia_fence",
        193 => "spruce_door",
        194 => "birch_door",
        195 => "jungle_door",
        196 => "acacia_door",
        197 => "dark_oak_door",
        _ => return None,
    };
    Some(name)
}

/// Lit blocks that share a blockstate file with their unlit variant.
const LIT_BLOCK_MAP: &[(u16, u16)] = &[
    (62, 61),   // lit_furnace → furnace
    (74, 73),   // lit_redstone_ore → redstone_ore
    (76, 75),   // redstone_torch → unlit_redstone_torch
    (94, 93),   // powered_repeater → unpowered_repeater
    (124, 123), // lit_redstone_lamp → redstone_lamp
];

// =========================================================================
// Metadata → variant key conversion
// =========================================================================

/// The 16 dye colors in MC 1.8.9 order.
const DYE_COLORS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "silver",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// The 6 piston/log facing directions in MC 1.8.9 metadata order.
const FACING_6: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

fn facing_6(meta: u8) -> &'static str {
    FACING_6
        .get((meta & 0x07) as usize)
        .copied()
        .unwrap_or("down")
}

/// The 4 horizontal facing directions.
const FACING_H: [&str; 4] = ["east", "south", "west", "north"];

fn horizontal_index_name(meta: u8) -> &'static str {
    match meta & 0x03 {
        0 => "south",
        1 => "west",
        2 => "north",
        _ => "east",
    }
}

fn stairs_facing_name(meta: u8) -> &'static str {
    match meta & 0x03 {
        0 => "east",
        1 => "west",
        2 => "south",
        _ => "north",
    }
}

fn door_facing_name(meta: u8) -> &'static str {
    match meta & 0x03 {
        0 => "east",
        1 => "south",
        2 => "west",
        _ => "north",
    }
}

fn front_facing_name(meta: u8) -> &'static str {
    match meta & 0x07 {
        2 => "north",
        3 => "south",
        4 => "west",
        5 => "east",
        _ => "north",
    }
}

fn trapdoor_facing_name(meta: u8) -> &'static str {
    match meta & 0x03 {
        0 => "north",
        1 => "south",
        2 => "west",
        _ => "east",
    }
}

/// Convert (block_id, metadata) to a blockstate variant key string.
/// Returns "normal" for blocks with no metadata-based variants.
pub fn meta_to_variant_key(block_id: u16, meta: u8) -> String {
    match block_id {
        // Stone variants
        1 => match meta {
            0 => "normal".into(),
            1 => "granite".into(),
            2 => "granite_smooth".into(),
            3 => "diorite".into(),
            4 => "diorite_smooth".into(),
            5 => "andesite".into(),
            6 => "andesite_smooth".into(),
            _ => "normal".into(),
        },
        // Dirt variants
        3 => match meta {
            0 => "normal".into(),
            1 => "coarse_dirt".into(),
            2 => "podzol".into(),
            _ => "normal".into(),
        },
        // Sapling
        6 => format!(
            "type={}",
            ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"]
                [((meta & 0x07) as usize).min(5)]
        ),
        // Water/lava
        8 | 10 => "normal".into(),
        9 | 11 => "normal".into(),
        // Sand
        12 => match meta {
            0 => "normal".into(),
            1 => "red_sand".into(),
            _ => "normal".into(),
        },
        // Log species use separate blockstate files; metadata only selects axis here.
        17 => {
            let axis = (meta >> 2) & 0x03;
            let axis_name = ["y", "z", "x", "none"][axis as usize];
            format!("axis={}", axis_name)
        }
        // Leaves
        18 => {
            let leaf_type = meta & 0x03;
            let decay = if meta & 0x04 != 0 { "true" } else { "false" };
            let type_name = ["oak", "spruce", "birch", "jungle"][leaf_type as usize];
            format!("check_decay={},variant={}", decay, type_name)
        }
        // Dispenser/dropper — facing + triggered
        23 | 158 => {
            let facing = facing_6(meta);
            let triggered = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("facing={},triggered={}", facing, triggered)
        }
        // Sandstone
        24 => match meta {
            0 => "normal".into(),
            1 => "chiseled".into(),
            2 => "smooth".into(),
            _ => "normal".into(),
        },
        // Bed — facing + part
        26 => {
            let facing = horizontal_index_name(meta);
            let part = if meta & 0x08 != 0 { "head" } else { "foot" };
            format!("facing={},part={}", facing, part)
        }
        // Sticky piston / piston — facing + extended
        29 | 33 => {
            let facing = facing_6(meta);
            let extended = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("facing={},extended={}", facing, extended)
        }
        // Piston head metadata stores facing and normal/sticky type. `short`
        // exists only in the transient tile-entity render state.
        34 => {
            let facing = facing_6(meta);
            let piston_type = if meta & 0x08 != 0 { "sticky" } else { "normal" };
            format!("facing={},short=false,type={}", facing, piston_type)
        }
        // Piston extension (block 36) — facing + extended
        36 => {
            let facing = facing_6(meta);
            format!("facing={}", facing)
        }
        // Wool — color
        35 => format!("color={}", DYE_COLORS[(meta & 0x0f) as usize]),
        // Tallgrass — type
        31 => match meta {
            0 => "normal".into(),
            1 => "grass".into(),
            2 => "fern".into(),
            _ => "grass".into(),
        },
        // Yellow flower (dandelion)
        37 => "normal".into(),
        // Red flower — type
        38 => match meta {
            0 => "poppy".into(),
            1 => "blue_orchid".into(),
            2 => "allium".into(),
            3 => "houstonia".into(),
            4 => "red_tulip".into(),
            5 => "orange_tulip".into(),
            6 => "white_tulip".into(),
            7 => "pink_tulip".into(),
            8 => "oxeye_daisy".into(),
            _ => "poppy".into(),
        },
        // Stone slab — half + variant
        44 => {
            let half = if meta & 0x08 != 0 { "top" } else { "bottom" };
            let variant = match meta & 0x07 {
                0 => "stone",
                1 => "sandstone",
                2 => "wood", // cobblestone
                3 => "cobblestone",
                4 => "brick",
                5 => "stone_brick",
                6 => "nether_brick",
                7 => "quartz",
                _ => "stone",
            };
            format!("half={},variant={}", half, variant)
        }
        // Wooden slab
        126 => {
            let half = if meta & 0x08 != 0 { "top" } else { "bottom" };
            let variant = match meta & 0x07 {
                0 => "wooden_slab",
                _ => "wooden_slab",
            };
            format!("half={},variant={}", half, variant)
        }
        // Double slabs
        43 => match meta & 0x07 {
            0 => "stone".into(),
            1 => "sandstone".into(),
            2 => "wood".into(),
            3 => "cobblestone".into(),
            4 => "brick".into(),
            5 => "stone_brick".into(),
            6 => "nether_brick".into(),
            7 => "quartz".into(),
            _ => "stone".into(),
        },
        125 => "wooden_slab".into(),
        // Torch — facing
        50 | 76 | 75 => match meta {
            // BlockTorch.getStateFromMeta: both 0 (invalid/default) and 5
            // resolve to FACING=UP. Using the old `normal` key misses modern
            // blockstate JSON and falls back to its first wall-facing variant.
            0 => "facing=up".into(),
            1 => "facing=east".into(),
            2 => "facing=west".into(),
            3 => "facing=south".into(),
            4 => "facing=north".into(),
            5 => "facing=up".into(),
            _ => "facing=up".into(),
        },
        // Stairs — facing + half
        53 | 67 | 108 | 109 | 114 | 128 | 134 | 135 | 136 | 156 | 163 | 164 | 180 => {
            let facing = stairs_facing_name(meta);
            let half = if meta & 0x04 != 0 { "top" } else { "bottom" };
            let shape = match meta >> 4 {
                0 => "straight",
                1 => "inner_left",
                2 => "inner_right",
                3 => "outer_left",
                4 => "outer_right",
                _ => "straight",
            };
            format!("facing={},half={},shape={}", facing, half, shape)
        }
        // Furnace — facing
        61 | 62 => {
            let facing = front_facing_name(meta);
            format!("facing={}", facing)
        }
        // Standing sign
        63 => format!("rotation={}", meta & 0x0f),
        // Wall sign — facing: meta 2=north, 3=south, 4=west, 5=east
        68 => {
            let facing = match meta & 0x07 {
                2 => "north",
                3 => "south",
                4 => "west",
                5 => "east",
                _ => "north",
            };
            format!("facing={}", facing)
        }
        // Doors — facing + half + hinge + open
        64 | 71 | 193 | 194 | 195 | 196 | 197 => {
            let facing = door_facing_name(meta);
            if meta & 0x08 != 0 {
                // Upper half
                let hinge = if meta & 0x04 != 0 { "left" } else { "right" };
                let powered = if meta & 0x02 != 0 { "true" } else { "false" };
                format!("half=upper,hinge={},powered={}", hinge, powered)
            } else {
                // Lower half
                let open = if meta & 0x04 != 0 { "true" } else { "false" };
                format!("facing={},half=lower,open={}", facing, open)
            }
        }
        // Rail
        66 => match meta & 0x0f {
            0 => "shape=north_south".into(),
            1 => "shape=east_west".into(),
            2 => "shape=ascending_east".into(),
            3 => "shape=ascending_west".into(),
            4 => "shape=ascending_north".into(),
            5 => "shape=ascending_south".into(),
            6 => "shape=south_east".into(),
            7 => "shape=south_west".into(),
            8 => "shape=north_west".into(),
            9 => "shape=north_east".into(),
            _ => "shape=north_south".into(),
        },
        // Powered/detector/activator rail
        27 | 28 | 157 => {
            let shape = match meta & 0x07 {
                0 => "north_south",
                1 => "east_west",
                2 => "ascending_east",
                3 => "ascending_west",
                4 => "ascending_north",
                5 => "ascending_south",
                _ => "north_south",
            };
            let powered = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("powered={},shape={}", powered, shape)
        }
        // Lever — facing
        69 => {
            let facing = match meta & 0x07 {
                0 => "down",
                1 => "east",
                2 => "west",
                3 => "south",
                4 => "north",
                5 => "up",
                _ => "down",
            };
            let powered = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("facing={},powered={}", facing, powered)
        }
        // Button — facing + powered
        77 | 143 => {
            let facing = facing_6(meta);
            let powered = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("facing={},powered={}", facing, powered)
        }
        // Trapdoor — facing + half + open
        96 | 167 => {
            let facing = trapdoor_facing_name(meta);
            let half = if meta & 0x08 != 0 { "top" } else { "bottom" };
            let open = if meta & 0x04 != 0 { "true" } else { "false" };
            format!("facing={},half={},open={}", facing, half, open)
        }
        // Cactus — age
        81 => format!("age={}", meta & 0x0f),
        // Reeds (sugar cane) — age
        83 => format!("age={}", meta & 0x0f),
        // Wheat — age
        59 => format!("age={}", meta & 0x07),
        // Carrots — age
        141 => format!("age={}", (meta >> 2) & 0x07),
        // Potatoes — age
        142 => format!("age={}", (meta >> 2) & 0x07),
        // Nether wart — age
        115 => format!("age={}", meta & 0x03),
        // Pumpkin/melon stem — age
        104 | 105 => format!("age={}", meta & 0x07),
        // Farmland — moisture
        60 => format!("moisture={}", meta & 0x07),
        // Cake — bites
        92 => format!("bites={}", meta & 0x07),
        // Vine — north/south/east/west
        106 => {
            let mut props = Vec::new();
            if meta & 0x01 != 0 {
                props.push("south=true");
            } else {
                props.push("south=false");
            }
            if meta & 0x02 != 0 {
                props.push("west=true");
            } else {
                props.push("west=false");
            }
            if meta & 0x04 != 0 {
                props.push("north=true");
            } else {
                props.push("north=false");
            }
            if meta & 0x08 != 0 {
                props.push("east=true");
            } else {
                props.push("east=false");
            }
            props.join(",")
        }
        // Fence gate — facing + open
        107 | 183 | 184 | 185 | 186 | 187 => {
            let facing = horizontal_index_name(meta);
            let open = if meta & 0x04 != 0 { "true" } else { "false" };
            let powered = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("facing={},open={},powered={}", facing, open, powered)
        }
        // Ladder — facing
        65 => {
            let facing = front_facing_name(meta);
            format!("facing={}", facing)
        }
        // Snow layer — layers
        78 => format!("layers={}", (meta & 0x07) + 1),
        // Cactus — age (already handled above, this is for completeness)
        // Cocoa — age + facing
        127 => {
            let age = (meta >> 2) & 0x03;
            let facing = match meta & 0x03 {
                0 => "south",
                1 => "west",
                2 => "north",
                3 => "east",
                _ => "south",
            };
            format!("age={},facing={}", age, facing)
        }
        // Anvil — facing + damage
        145 => {
            let facing = if meta & 0x01 != 0 { "north" } else { "east" }; // simplified
            let damage = match meta & 0x0C {
                0 => "undamaged",
                4 => "slightly_damaged",
                8 => "very_damaged",
                _ => "undamaged",
            };
            format!("damage={},facing={}", damage, facing)
        }
        // Quartz block — variant
        155 => match meta & 0x07 {
            0 => "normal".into(),
            1 => "chiseled".into(),
            2 => "lines_y".into(),
            3 => "lines_x".into(),
            4 => "lines_z".into(),
            _ => "normal".into(),
        },
        // Hay block — axis
        170 => {
            let axis = match meta & 0x03 {
                0 => "y",
                1 => "z",
                2 => "x",
                _ => "y",
            };
            format!("axis={}", axis)
        }
        // Log2 species also use separate blockstate files.
        162 => {
            let axis = (meta >> 2) & 0x03;
            let axis_name = ["y", "z", "x", "none"][axis as usize];
            format!("axis={}", axis_name)
        }
        // Leaves2 — type (acacia/dark_oak)
        161 => {
            let leaf_type = meta & 0x01;
            let decay = if meta & 0x04 != 0 { "true" } else { "false" };
            let type_name = if leaf_type == 0 { "acacia" } else { "dark_oak" };
            format!("check_decay={},variant={}", decay, type_name)
        }
        // Stained hardened clay (terracotta) — color
        159 => format!("color={}", DYE_COLORS[(meta & 0x0f) as usize]),
        // Stained glass pane — color
        160 => format!("color={}", DYE_COLORS[(meta & 0x0f) as usize]),
        // Carpet — color
        171 => format!("color={}", DYE_COLORS[(meta & 0x0f) as usize]),
        // Double plant — type + half
        175 => {
            let type_name = match meta & 0x07 {
                0 => "sunflower",
                1 => "syringa",
                2 => "grass",
                3 => "fern",
                4 => "rose",
                5 => "paeonia",
                _ => "sunflower",
            };
            let half = if meta & 0x08 != 0 { "upper" } else { "lower" };
            format!("half={},variant={}", half, type_name)
        }
        // Pumpkin / Jack-o-lantern — facing
        86 | 91 => {
            let facing = match meta & 0x07 {
                0 => "south",
                1 => "west",
                2 => "north",
                3 => "east",
                _ => "south",
            };
            format!("facing={}", facing)
        }
        // End portal frame — facing + eye
        120 => {
            let facing = match meta & 0x07 {
                0 => "south",
                1 => "west",
                2 => "north",
                3 => "east",
                _ => "south",
            };
            let eye = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("eye={},facing={}", eye, facing)
        }
        // Monster egg — variant
        97 => match meta & 0x07 {
            0 => "stone",
            1 => "cobblestone",
            2 => "stone_brick",
            3 => "mossy_brick",
            4 => "cracked_brick",
            5 => "chiseled_brick",
            _ => "stone",
        }
        .into(),
        // Stonebrick — variant
        98 => match meta & 0x07 {
            0 => "normal",
            1 => "mossy",
            2 => "cracked",
            3 => "chiseled",
            4 => "circle",
            _ => "normal",
        }
        .into(),
        // Redstone wire — north/south/east/west + power
        55 => {
            // Simplified: just use the basic wire shape
            "normal".into()
        }
        // Repeater — facing + powered + delay (simplified)
        93 | 94 => {
            let facing = match meta & 0x03 {
                0 => "north",
                1 => "east",
                2 => "south",
                3 => "west",
                _ => "north",
            };
            let powered = if block_id == 94 { "true" } else { "false" };
            format!("facing={},powered={}", facing, powered)
        }
        // Comparator
        149 | 150 => {
            let facing = match meta & 0x03 {
                0 => "north",
                1 => "east",
                2 => "south",
                3 => "west",
                _ => "north",
            };
            let powered = if block_id == 150 { "true" } else { "false" };
            let mode = match (meta >> 2) & 0x03 {
                0 => "compare",
                _ => "subtract",
            };
            format!("facing={},mode={},powered={}", facing, mode, powered)
        }
        // Hopper — facing
        154 => {
            let facing = match meta & 0x07 {
                0 => "down",
                2 => "north",
                3 => "south",
                4 => "west",
                5 => "east",
                _ => "down",
            };
            let enabled = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("enabled={},facing={}", enabled, facing)
        }
        // Skull — facing + nodrop
        144 => {
            let facing = match meta & 0x07 {
                0 => "up",
                1 => "north",
                2 => "south",
                3 => "east",
                4 => "west",
                _ => "up",
            };
            let nodrop = if meta & 0x08 != 0 { "true" } else { "false" };
            format!("facing={},nodrop={}", facing, nodrop)
        }
        // Cobblestone wall — variant + connections (simplified)
        139 => match meta {
            0 => "normal".into(),
            1 => "mossy".into(),
            _ => "normal".into(),
        },
        // Mushroom block — variant
        99 | 100 => {
            let variant = match meta & 0x0F {
                0 => "all_inside",
                14 => "all_outside",
                15 => "stem",
                _ => "all_inside",
            };
            format!("variant={}", variant)
        }
        // Sponge — wet
        19 => match meta {
            0 => "normal".into(),
            1 => "wet".into(),
            _ => "normal".into(),
        },
        // Daylight detector — inverted
        151 => match meta {
            0 => "normal".into(),
            1 => "inverted".into(),
            _ => "normal".into(),
        },
        // Default: "normal" variant
        _ => "normal".into(),
    }
}

/// Try to reverse-map a variant key string back to a metadata value.
/// This is used when building the model cache from the blockstate JSON.
fn variant_key_to_meta(block_id: u16, variant_key: &str) -> Option<u8> {
    // For "normal" variant, meta is 0
    if variant_key == "normal" {
        return Some(0);
    }

    // Parse the variant key into property=value pairs
    let props: HashMap<&str, &str> = variant_key
        .split(',')
        .filter_map(|kv| {
            let mut parts = kv.split('=');
            let key = parts.next()?.trim();
            let val = parts.next()?.trim();
            Some((key, val))
        })
        .collect();

    // For most blocks, the variant key encodes the metadata directly.
    // We reconstruct it by reversing the logic in meta_to_variant_key.

    // Connection-state variants (fences, walls, glass panes, iron bars)
    // These don't map to metadata — use old shape system for world rendering
    // Cache the disconnected variant (all false) at meta=0 for inventory icons
    if props.contains_key("east")
        || props.contains_key("north")
        || props.contains_key("south")
        || props.contains_key("west")
    {
        let all_disconnected = ["east", "north", "south", "west"]
            .iter()
            .all(|k| props.get(*k) != Some(&"true"));
        if all_disconnected {
            return Some(0);
        }
        return None;
    }

    // Color blocks (wool, stained_clay, stained_glass_pane, carpet)
    if let Some(&color) = props.get("color") {
        return DYE_COLORS.iter().position(|c| *c == color).map(|i| i as u8);
    }

    // Stone variants
    if block_id == 1 {
        return match props.get("variant").copied() {
            Some("granite") => Some(1),
            Some("granite_smooth") => Some(2),
            Some("diorite") => Some(3),
            Some("diorite_smooth") => Some(4),
            Some("andesite") => Some(5),
            Some("andesite_smooth") => Some(6),
            _ => Some(0),
        };
    }

    // Facing-based blocks (simplified — just try to match)
    if let Some(facing) = props.get("facing") {
        let facing_val = match *facing {
            "down" => 0u8,
            "up" => 1,
            "north" => 2,
            "south" => 3,
            "west" => 4,
            "east" => 5,
            _ => 0,
        };
        // For horizontal-facing only (stairs, furnace, etc.)
        let horizontal_val = match *facing {
            "east" => 0,
            "south" => 1,
            "west" => 2,
            "north" => 3,
            _ => 0,
        };
        return match block_id {
            // Stairs (facing + half + shape)
            53 | 67 | 108 | 109 | 114 | 128 | 134 | 135 | 136 | 156 => {
                let half = if props.get("half") == Some(&"top") {
                    0x04
                } else {
                    0
                };
                let shape_bits = match props.get("shape").copied() {
                    Some("straight") => 0,
                    Some("inner_left") => 1 << 4,
                    Some("inner_right") => 2 << 4,
                    Some("outer_left") => 3 << 4,
                    Some("outer_right") => 4 << 4,
                    _ => 0,
                };
                Some(horizontal_val | half | shape_bits)
            }
            // Furnace
            61 | 62 => Some(horizontal_val),
            // Piston
            29 | 33 => {
                let ext = if props.get("extended") == Some(&"true") {
                    0x08
                } else {
                    0
                };
                Some(facing_val | ext)
            }
            // Piston head — skip short variants (determined by piston state, not metadata)
            34 => {
                if props.get("short") == Some(&"true") {
                    return None; // short variant is dynamically selected by piston logic
                }
                let sticky = if props.get("type") == Some(&"sticky") {
                    0x08
                } else {
                    0
                };
                Some(facing_val | sticky)
            }
            // Hopper
            154 => Some(facing_val),
            // Door
            64 | 71 => {
                if props.get("half") == Some(&"upper") {
                    let hinge = if props.get("hinge") == Some(&"left") {
                        0x04
                    } else {
                        0
                    };
                    Some(0x08 | hinge)
                } else {
                    let open = if props.get("open") == Some(&"true") {
                        0x04
                    } else {
                        0
                    };
                    Some(horizontal_val | open)
                }
            }
            // Trapdoor
            96 => {
                let half = if props.get("half") == Some(&"top") {
                    0x08
                } else {
                    0
                };
                let open = if props.get("open") == Some(&"true") {
                    0x04
                } else {
                    0
                };
                Some(horizontal_val | half | open)
            }
            // Button/Lever
            77 | 143 | 69 => {
                let powered = if props.get("powered") == Some(&"true") {
                    0x08
                } else {
                    0
                };
                Some(facing_val | powered)
            }
            // Ladder — facing: meta 2=north, 3=south, 4=west, 5=east
            65 => Some(match *facing {
                "north" => 2,
                "south" => 3,
                "west" => 4,
                "east" => 5,
                _ => 2,
            }),
            // Wall sign — facing: meta 2=north, 3=south, 4=west, 5=east
            68 => Some(match *facing {
                "north" => 2,
                "south" => 3,
                "west" => 4,
                "east" => 5,
                _ => 2,
            }),
            // Fence gate
            107 => {
                let open = if props.get("open") == Some(&"true") {
                    0x04
                } else {
                    0
                };
                Some(horizontal_val | open)
            }
            // Pumpkin/Jack-o-lantern
            86 | 91 => Some(match *facing {
                "south" => 0,
                "west" => 1,
                "north" => 2,
                "east" => 3,
                _ => 0,
            }),
            // Torch / Redstone Torch (standing + wall)
            50 | 76 => Some(match *facing {
                "east" => 1,
                "west" => 2,
                "south" => 3,
                "north" => 4,
                "up" => 5,
                _ => 0,
            }),
            // End portal frame
            120 => {
                let eye = if props.get("eye") == Some(&"true") {
                    0x08
                } else {
                    0
                };
                Some(
                    match *facing {
                        "south" => 0,
                        "west" => 1,
                        "north" => 2,
                        "east" => 3,
                        _ => 0,
                    } | eye,
                )
            }
            _ => Some(0),
        };
    }

    // Slab — half + variant
    if let Some(half) = props.get("half") {
        let half_bit = if *half == "top" || *half == "upper" {
            0x08
        } else {
            0
        };
        if let Some(variant) = props.get("variant") {
            let var_bits = match *variant {
                "stone" => 0,
                "sandstone" => 1,
                "wood" => 2, // actually cobblestone slab
                "cobblestone" => 3,
                "brick" => 4,
                "stone_brick" => 5,
                "nether_brick" => 6,
                "quartz" => 7,
                "wooden_slab" => 0,
                _ => 0,
            };
            return Some(half_bit | var_bits);
        }
        return Some(half_bit);
    }

    // Log — axis + variant
    if let Some(axis) = props.get("axis") {
        let axis_val = match *axis {
            "y" => 0u8,
            "z" => 1,
            "x" => 2,
            "none" => 3,
            _ => 0,
        };
        if let Some(variant) = props.get("variant") {
            let type_val = match *variant {
                "oak" => 0,
                "spruce" => 1,
                "birch" => 2,
                "jungle" => 3,
                "acacia" => 0,
                "dark_oak" => 1,
                _ => 0,
            };
            return Some(type_val | (axis_val << 2));
        }
        return Some(axis_val);
    }

    // Rail — shape
    if let Some(shape) = props.get("shape") {
        return Some(match *shape {
            "north_south" => 0,
            "east_west" => 1,
            "ascending_east" => 2,
            "ascending_west" => 3,
            "ascending_north" => 4,
            "ascending_south" => 5,
            "south_east" => 6,
            "south_west" => 7,
            "north_west" => 8,
            "north_east" => 9,
            _ => 0,
        });
    }

    // Snow layer — layers
    if let Some(layers) = props.get("layers") {
        return layers
            .parse::<u8>()
            .ok()
            .map(|v| v.saturating_sub(1) & 0x07);
    }

    // Age (crops)
    if let Some(age) = props.get("age") {
        return age.parse::<u8>().ok();
    }

    // Rotation (standing sign)
    if let Some(rotation) = props.get("rotation") {
        return rotation.parse::<u8>().ok();
    }

    // Default: can't decode, use 0
    Some(0)
}

/// Check if a block uses connection-state (east/north/south/west) variants.
/// These blocks are rendered by the old shape.rs system in the world.
/// Check whether a block has a JSON model (blockstate + model file).
/// Non-solid blocks without custom shapes rely on this check to be rendered.
pub fn has_json_model(block_id: u16) -> bool {
    for meta in 0u8..16 {
        if blockstate_name_for_state(block_id, meta).is_some() {
            return true;
        }
    }
    false
}

pub fn is_connection_state_block(block_id: u16) -> bool {
    matches!(
        block_id,
        36 | 53
            | 54
            | 55
            | 63
            | 68
            | 64
            | 67
            | 69
            | 71
            | 77
            | 85
            | 96
            | 101
            | 102
            | 107
            | 108
            | 109
            | 113
            | 114
            | 128
            | 130
            | 134
            | 135
            | 136
            | 139
            | 143
            | 146
            | 156
            | 160
            | 163
            | 164
            | 167
            | 180
            | 183
            | 184
            | 185
            | 186
            | 187
            | 188
            | 189
            | 190
            | 191
            | 192
            | 193
            | 194
            | 195
            | 196
            | 197
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_names_cover_split_vanilla_metadata_files() {
        assert_eq!(blockstate_name_for_state(1, 1).as_deref(), Some("granite"));
        assert_eq!(
            blockstate_name_for_state(5, 1).as_deref(),
            Some("spruce_planks")
        );
        assert_eq!(
            blockstate_name_for_state(35, 14).as_deref(),
            Some("red_wool")
        );
        assert_eq!(
            blockstate_name_for_state(95, 11).as_deref(),
            Some("blue_stained_glass")
        );
    }

    #[test]
    fn double_plant_keeps_species_in_low_bits_and_half_in_high_bit() {
        assert_eq!(
            blockstate_name_for_state(175, 0x04).as_deref(),
            Some("double_rose")
        );
        assert_eq!(
            blockstate_name_for_state(175, 0x0c).as_deref(),
            Some("double_rose")
        );

        let variants = vec![
            (
                "half=lower".to_string(),
                "double_rose_bottom".to_string(),
                0.0,
                0.0,
            ),
            (
                "half=upper".to_string(),
                "double_rose_top".to_string(),
                0.0,
                0.0,
            ),
        ];
        assert_eq!(
            select_state_variant(175, 0x0c, &variants).unwrap().1,
            "double_rose_top"
        );
    }

    #[test]
    fn cache_bakes_distinct_metadata_textures() {
        let mut registry = ModelRegistry::new();
        registry.load_from_pack("assets/minecraft");
        let cache = BlockModelCache::build(&mut registry, HashMap::new());

        let texture = |id, meta| {
            cache
                .get_model(id, meta)
                .and_then(|model| model.faces.first())
                .map(|face| face.texture.as_str())
        };
        assert_eq!(texture(1, 1), Some("blocks/stone_granite"));
        assert_eq!(texture(5, 1), Some("blocks/planks_spruce"));
        assert_eq!(texture(35, 14), Some("blocks/wool_colored_red"));
        assert_eq!(texture(95, 11), Some("blocks/glass_blue"));
    }

    #[test]
    fn log_metadata_keeps_species_in_low_bits_and_axis_in_high_bits() {
        assert_eq!(
            blockstate_name_for_state(17, 0b0101).as_deref(),
            Some("spruce_log")
        );
        assert_eq!(meta_to_variant_key(17, 0b0101), "axis=z");
        assert_eq!(meta_to_variant_key(17, 0b1010), "axis=x");
        assert_eq!(
            blockstate_name_for_state(162, 0b1001).as_deref(),
            Some("dark_oak_log")
        );
        assert_eq!(meta_to_variant_key(162, 0b1001), "axis=x");
        assert_eq!(
            variant_key_to_meta(17, "axis=z,variant=spruce"),
            Some(0b0101)
        );
        assert_eq!(
            variant_key_to_meta(162, "axis=x,variant=dark_oak"),
            Some(0b1001)
        );
    }

    #[test]
    fn piston_head_metadata_selects_facing_and_sticky_type() {
        assert_eq!(
            meta_to_variant_key(34, 1),
            "facing=up,short=false,type=normal"
        );
        assert_eq!(
            meta_to_variant_key(34, 0x0d),
            "facing=east,short=false,type=sticky"
        );
    }

    #[test]
    fn torch_north_south_metadata_matches_vanilla() {
        // BlockTorch.getStateFromMeta: 3 = SOUTH, 4 = NORTH.
        assert_eq!(meta_to_variant_key(50, 3), "facing=south");
        assert_eq!(meta_to_variant_key(50, 4), "facing=north");
        assert_eq!(variant_key_to_meta(50, "facing=south"), Some(3));
        assert_eq!(variant_key_to_meta(50, "facing=north"), Some(4));
    }

    #[test]
    fn horizontal_logs_put_end_grain_on_the_axis_faces() {
        let mut registry = ModelRegistry::new();
        registry.load_from_pack("assets/minecraft");
        let cache = BlockModelCache::build(&mut registry, HashMap::new());

        let z_log = cache.get_model(17, 0b0101).unwrap();
        assert!(z_log
            .faces
            .iter()
            .any(|face| { face.texture == "blocks/log_spruce_top" && face.normal[2].abs() > 0.9 }));
        let x_log = cache.get_model(162, 0b1001).unwrap();
        assert!(x_log.faces.iter().any(|face| {
            face.texture == "blocks/log_big_oak_top" && face.normal[0].abs() > 0.9
        }));
    }
}
