use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;

/// Texture storage per icon.  Vanilla icons are 16×16, but resource packs
/// commonly supply 32×/64×/128× variants.  Keep those source pixels in the
/// atlas instead of shrinking them before the held-item pass samples them.
pub const ITEM_ATLAS_CELL: u32 = 128;
pub const ITEM_ATLAS_COLS: u32 = 32;
pub const ITEM_ATLAS_ROWS: u32 = 32;
pub const ITEM_ATLAS_W: u32 = ITEM_ATLAS_CELL * ITEM_ATLAS_COLS;
pub const ITEM_ATLAS_H: u32 = ITEM_ATLAS_CELL * ITEM_ATLAS_ROWS;

pub fn item_icon_path(item_id: u16, damage: u16) -> Option<&'static str> {
    Some(match item_id {
        50 => "blocks/torch_on",
        75 => "blocks/redstone_torch_off",
        76 => "blocks/redstone_torch_on",
        166 => "barrier",
        256 => "iron_shovel",
        257 => "iron_pickaxe",
        258 => "iron_axe",
        259 => "flint_and_steel",
        260 => "apple",
        261 => match damage {
            1 => "bow_pulling_0",
            2 => "bow_pulling_1",
            3 => "bow_pulling_2",
            _ => "bow_standby",
        },
        262 => "arrow",
        263 => {
            if damage == 1 {
                "charcoal"
            } else {
                "coal"
            }
        }
        264 => "diamond",
        265 => "iron_ingot",
        266 => "gold_ingot",
        267 => "iron_sword",
        268 => "wood_sword",
        269 => "wood_shovel",
        270 => "wood_pickaxe",
        271 => "wood_axe",
        272 => "stone_sword",
        273 => "stone_shovel",
        274 => "stone_pickaxe",
        275 => "stone_axe",
        276 => "diamond_sword",
        277 => "diamond_shovel",
        278 => "diamond_pickaxe",
        279 => "diamond_axe",
        280 => "stick",
        281 => "bowl",
        282 => "mushroom_stew",
        283 => "gold_sword",
        284 => "gold_shovel",
        285 => "gold_pickaxe",
        286 => "gold_axe",
        287 => "string",
        288 => "feather",
        289 => "gunpowder",
        290 => "wood_hoe",
        291 => "stone_hoe",
        292 => "iron_hoe",
        293 => "diamond_hoe",
        294 => "gold_hoe",
        295 => "seeds_wheat",
        296 => "wheat",
        297 => "bread",
        298 => "leather_helmet",
        299 => "leather_chestplate",
        300 => "leather_leggings",
        301 => "leather_boots",
        302 => "chainmail_helmet",
        303 => "chainmail_chestplate",
        304 => "chainmail_leggings",
        305 => "chainmail_boots",
        306 => "iron_helmet",
        307 => "iron_chestplate",
        308 => "iron_leggings",
        309 => "iron_boots",
        310 => "diamond_helmet",
        311 => "diamond_chestplate",
        312 => "diamond_leggings",
        313 => "diamond_boots",
        314 => "gold_helmet",
        315 => "gold_chestplate",
        316 => "gold_leggings",
        317 => "gold_boots",
        318 => "flint",
        319 => "porkchop_raw",
        320 => "porkchop_cooked",
        321 => "painting",
        322 => "apple_golden",
        323 => "sign",
        324 => "door_wood",
        325 => "bucket_empty",
        326 => "bucket_water",
        327 => "bucket_lava",
        328 => "minecart_normal",
        329 => "saddle",
        330 => "door_iron",
        331 => "redstone_dust",
        332 => "snowball",
        333 => "boat",
        334 => "leather",
        335 => "bucket_milk",
        336 => "brick",
        337 => "clay_ball",
        338 => "reeds",
        339 => "paper",
        340 => "book_normal",
        341 => "slimeball",
        342 => "minecart_chest",
        343 => "minecart_furnace",
        344 => "egg",
        345 => "compass",
        346 => "fishing_rod_uncast",
        347 => "clock",
        348 => "glowstone_dust",
        349 => match damage {
            1 => "fish_salmon_raw",
            2 => "fish_clownfish_raw",
            3 => "fish_pufferfish_raw",
            _ => "fish_cod_raw",
        },
        350 => {
            if damage == 1 {
                "fish_salmon_cooked"
            } else {
                "fish_cod_cooked"
            }
        }
        351 => dye_icon(damage),
        352 => "bone",
        353 => "sugar",
        354 => "cake",
        355 => "bed",
        356 => "repeater",
        357 => "cookie",
        358 => "map_filled",
        359 => "shears",
        360 => "melon",
        361 => "seeds_pumpkin",
        362 => "seeds_melon",
        363 => "beef_raw",
        364 => "beef_cooked",
        365 => "chicken_raw",
        366 => "chicken_cooked",
        367 => "rotten_flesh",
        368 => "ender_pearl",
        369 => "blaze_rod",
        370 => "ghast_tear",
        371 => "gold_nugget",
        372 => "nether_wart",
        373 => {
            if damage & 0x4000 != 0 {
                "potion_bottle_splash"
            } else {
                "potion_bottle_drinkable"
            }
        }
        374 => "potion_bottle_empty",
        375 => "spider_eye",
        376 => "spider_eye_fermented",
        377 => "blaze_powder",
        378 => "magma_cream",
        379 => "brewing_stand",
        380 => "cauldron",
        381 => "ender_eye",
        382 => "melon_speckled",
        383 => "spawn_egg",
        384 => "experience_bottle",
        385 => "fireball",
        386 => "book_writable",
        387 => "book_written",
        388 => "emerald",
        389 => "item_frame",
        390 => "flower_pot",
        391 => "carrot",
        392 => "potato",
        393 => "potato_baked",
        394 => "potato_poisonous",
        395 => "map_empty",
        396 => "carrot_golden",
        397 => match damage {
            1 => "entity/skeleton/wither_skeleton",
            2 => "entity/zombie/zombie",
            3 => "entity/steve",
            4 => "entity/creeper/creeper",
            _ => "entity/skeleton/skeleton",
        },
        398 => "carrot_on_a_stick",
        399 => "nether_star",
        400 => "pumpkin_pie",
        401 => "fireworks",
        402 => "fireworks_charge",
        403 => "book_enchanted",
        404 => "comparator",
        405 => "netherbrick",
        406 => "quartz",
        407 => "minecart_tnt",
        408 => "minecart_hopper",
        409 => "prismarine_shard",
        410 => "prismarine_crystals",
        411 => "rabbit_raw",
        412 => "rabbit_cooked",
        413 => "rabbit_stew",
        414 => "rabbit_foot",
        415 => "rabbit_hide",
        416 => "wooden_armorstand",
        417 => "iron_horse_armor",
        418 => "gold_horse_armor",
        419 => "diamond_horse_armor",
        420 => "lead",
        421 => "name_tag",
        422 => "minecart_command_block",
        423 => "mutton_raw",
        424 => "mutton_cooked",
        425 => "banner_base",
        427 => "door_spruce",
        428 => "door_birch",
        429 => "door_jungle",
        430 => "door_acacia",
        431 => "door_dark_oak",
        2256 => "record_13",
        2257 => "record_cat",
        2258 => "record_blocks",
        2259 => "record_chirp",
        2260 => "record_far",
        2261 => "record_mall",
        2262 => "record_mellohi",
        2263 => "record_stal",
        2264 => "record_strad",
        2265 => "record_ward",
        2266 => "record_11",
        2267 => "record_wait",
        // Non-full block items that need 2D icons (layer0 from vanilla item model)
        30 => "blocks/web",
        55 => "items/redstone_dust",
        65 => "blocks/ladder",
        81 => "blocks/cactus_side",
        83 => "items/reeds",
        85 => "blocks/planks_oak",
        92 => "items/cake",
        101 => "blocks/iron_bars",
        102 => "blocks/glass",
        106 => "blocks/vine",
        113 => "blocks/nether_brick",
        139 => match damage {
            1 => "blocks/cobblestone_mossy",
            _ => "blocks/cobblestone",
        },
        160 => match damage {
            0 => "blocks/glass_white",
            1 => "blocks/glass_orange",
            2 => "blocks/glass_magenta",
            3 => "blocks/glass_light_blue",
            4 => "blocks/glass_yellow",
            5 => "blocks/glass_lime",
            6 => "blocks/glass_pink",
            7 => "blocks/glass_gray",
            8 => "blocks/glass_silver",
            9 => "blocks/glass_cyan",
            10 => "blocks/glass_purple",
            11 => "blocks/glass_blue",
            12 => "blocks/glass_brown",
            13 => "blocks/glass_green",
            14 => "blocks/glass_red",
            15 => "blocks/glass_black",
            _ => "blocks/glass",
        },
        188 => "blocks/planks_spruce",
        189 => "blocks/planks_birch",
        190 => "blocks/planks_jungle",
        191 => "blocks/planks_big_oak",
        192 => "blocks/planks_acacia",
        _ => return None,
    })
}

pub fn item_icon_index(item_id: u16, damage: u16) -> Option<usize> {
    item_icon_path(item_id, damage)
        .and_then(|path| item_icon_entries().iter().position(|entry| *entry == path))
}

#[derive(Clone, Copy, Debug)]
pub struct ItemIconLayer {
    pub path: &'static str,
    pub color: [f32; 4],
}

pub fn item_icon_layers(item_id: u16, damage: u16) -> Vec<ItemIconLayer> {
    let white = [1.0; 4];
    match item_id {
        298..=301 => {
            let base = item_icon_path(item_id, damage).unwrap();
            let overlay = match item_id {
                298 => "leather_helmet_overlay",
                299 => "leather_chestplate_overlay",
                300 => "leather_leggings_overlay",
                _ => "leather_boots_overlay",
            };
            vec![
                ItemIconLayer {
                    path: base,
                    color: rgb_layer(0xA06540),
                },
                ItemIconLayer {
                    path: overlay,
                    color: white,
                },
            ]
        }
        373 => vec![
            ItemIconLayer {
                path: item_icon_path(item_id, damage).unwrap(),
                color: white,
            },
            ItemIconLayer {
                path: "potion_overlay",
                color: rgb_layer(potion_color(damage)),
            },
        ],
        383 => {
            let (primary, secondary) = spawn_egg_colors(damage);
            vec![
                ItemIconLayer {
                    path: "spawn_egg",
                    color: rgb_layer(primary),
                },
                ItemIconLayer {
                    path: "spawn_egg_overlay",
                    color: rgb_layer(secondary),
                },
            ]
        }
        402 => vec![
            ItemIconLayer {
                path: "fireworks_charge",
                color: white,
            },
            ItemIconLayer {
                path: "fireworks_charge_overlay",
                color: white,
            },
        ],
        _ => item_icon_path(item_id, damage)
            .map(|path| vec![ItemIconLayer { path, color: white }])
            .unwrap_or_default(),
    }
}

pub fn item_icon_entry_index(path: &str) -> Option<usize> {
    item_icon_entries().iter().position(|entry| *entry == path)
}

fn rgb_layer(rgb: u32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        1.0,
    ]
}

fn potion_color(damage: u16) -> u32 {
    match damage & 0x3fff {
        8193 | 8225 => 0x7CAFC6, // regeneration
        8194 | 8226 => 0x932423, // swiftness
        8195 => 0x5A6C81,        // fire resistance
        8196 | 8228 => 0xFF9900, // poison
        8197 | 8229 => 0xCD5CAB, // healing
        8198 => 0xE49A3A,        // night vision
        8200 => 0x4E9331,        // weakness
        8201 | 8233 => 0xF82423, // strength
        8202 => 0x1F1FA1,        // slowness
        8204 => 0x430A09,        // harming
        8205 => 0x2E5299,        // water breathing
        8206 => 0x7F8392,        // invisibility
        8234 => 0x786297,        // leaping
        _ => 0x385DC6,
    }
}

fn spawn_egg_colors(entity_id: u16) -> (u32, u32) {
    match entity_id {
        50 => (894731, 0),
        51 => (12698049, 4802889),
        52 => (3419431, 11013646),
        54 => (44975, 7969893),
        55 => (5349438, 8306542),
        56 => (16382457, 12369084),
        57 => (15373203, 5009705),
        58 => (1447446, 0),
        59 => (803406, 11013646),
        60 => (7237230, 3158064),
        61 => (16167425, 16775294),
        62 => (3407872, 16579584),
        65 => (4996656, 986895),
        66 => (3407872, 5349438),
        67 => (1447446, 7237230),
        68 => (5931634, 15826224),
        90 => (15771042, 14377823),
        91 => (15198183, 16758197),
        92 => (4470310, 10592673),
        93 => (10592673, 16711680),
        94 => (2243405, 7375001),
        95 => (14144467, 13545366),
        96 => (10489616, 12040119),
        98 => (15720061, 5653556),
        100 => (12623485, 15656192),
        101 => (10051392, 7555121),
        120 => (5651507, 12422002),
        _ => (0xFFFFFF, 0xC0C0C0),
    }
}

pub fn item_icon_uv_rect(index: usize) -> [f32; 4] {
    let col = index % ITEM_ATLAS_COLS as usize;
    let row = index / ITEM_ATLAS_COLS as usize;
    let u0 = col as f32 * ITEM_ATLAS_CELL as f32 / ITEM_ATLAS_W as f32;
    let v0 = row as f32 * ITEM_ATLAS_CELL as f32 / ITEM_ATLAS_H as f32;
    let u1 = (col + 1) as f32 * ITEM_ATLAS_CELL as f32 / ITEM_ATLAS_W as f32;
    let v1 = (row + 1) as f32 * ITEM_ATLAS_CELL as f32 / ITEM_ATLAS_H as f32;
    [u0, v0, u1, v1]
}

pub fn item_icon_entries() -> &'static [&'static str] {
    ITEM_ICON_ENTRIES
}

/// Animation metadata parsed from a `.mcmeta` sidecar file.  When present the
/// atlas builder crops an animated sprite sheet down to its first frame so that
/// the texture is not squished into the atlas cell.
struct AnimationInfo {
    frame_width: Option<u32>,
    frame_height: Option<u32>,
}

fn read_animation_info(
    resolver: &mut crate::assets::resolver::AssetResolver,
    png_path: &str,
) -> Option<AnimationInfo> {
    let mcmeta_path = format!("{png_path}.mcmeta");
    let bytes = resolver.read_bytes(&mcmeta_path)?;
    let root: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let anim = root.get("animation")?;
    let frame_width = anim.get("width").and_then(|v| v.as_i64()).map(|v| v as u32);
    let frame_height = anim
        .get("height")
        .and_then(|v| v.as_i64())
        .map(|v| v as u32);
    Some(AnimationInfo {
        frame_width,
        frame_height,
    })
}

fn crop_to_first_frame(img: image::RgbaImage, info: &AnimationInfo) -> image::RgbaImage {
    let fw = info.frame_width.unwrap_or(img.width());
    let fh = info.frame_height.unwrap_or(img.height());
    if fw == img.width() && fh == img.height() {
        return img;
    }
    if fw == 0 || fh == 0 {
        return img;
    }
    image::imageops::crop_imm(&img, 0, 0, fw.min(img.width()), fh.min(img.height())).to_image()
}

pub fn build_item_icon_atlas(resolver: &mut crate::assets::resolver::AssetResolver) -> Vec<u8> {
    let mut atlas = image::RgbaImage::new(ITEM_ATLAS_W, ITEM_ATLAS_H);
    for (idx, name) in item_icon_entries().iter().enumerate() {
        let path = item_icon_resource_path(name);
        let Some(bytes) = resolver.read_bytes(&path) else {
            continue;
        };
        let Some(mut img) = decode_item_icon(name, &bytes) else {
            continue;
        };
        if let Some(info) = read_animation_info(resolver, &path) {
            img = crop_to_first_frame(img, &info);
        }
        let img = if img.width() != ITEM_ATLAS_CELL || img.height() != ITEM_ATLAS_CELL {
            image::imageops::resize(
                &img,
                ITEM_ATLAS_CELL,
                ITEM_ATLAS_CELL,
                image::imageops::FilterType::Nearest,
            )
        } else {
            img
        };
        let x0 = idx as u32 % ITEM_ATLAS_COLS * ITEM_ATLAS_CELL;
        let y0 = idx as u32 / ITEM_ATLAS_COLS * ITEM_ATLAS_CELL;
        for y in 0..img.height().min(ITEM_ATLAS_CELL) {
            for x in 0..img.width().min(ITEM_ATLAS_CELL) {
                atlas.put_pixel(x0 + x, y0 + y, *img.get_pixel(x, y));
            }
        }
    }
    atlas.into_raw()
}

fn item_icon_resource_path(name: &str) -> String {
    if name.contains('/') {
        format!("minecraft/textures/{}.png", name)
    } else {
        format!("minecraft/textures/items/{}.png", name)
    }
}

fn dye_icon(damage: u16) -> &'static str {
    match damage {
        0 => "dye_powder_black",
        1 => "dye_powder_red",
        2 => "dye_powder_green",
        3 => "dye_powder_brown",
        4 => "dye_powder_blue",
        5 => "dye_powder_purple",
        6 => "dye_powder_cyan",
        7 => "dye_powder_silver",
        8 => "dye_powder_gray",
        9 => "dye_powder_pink",
        10 => "dye_powder_lime",
        11 => "dye_powder_yellow",
        12 => "dye_powder_light_blue",
        13 => "dye_powder_magenta",
        14 => "dye_powder_orange",
        _ => "dye_powder_white",
    }
}

const ITEM_ICON_ENTRIES: &[&str] = &[
    "blocks/torch_on",
    "blocks/redstone_torch_off",
    "blocks/redstone_torch_on",
    "barrier",
    "iron_shovel",
    "iron_pickaxe",
    "iron_axe",
    "flint_and_steel",
    "apple",
    "bow_standby",
    "bow_pulling_0",
    "bow_pulling_1",
    "bow_pulling_2",
    "arrow",
    "coal",
    "charcoal",
    "diamond",
    "iron_ingot",
    "gold_ingot",
    "iron_sword",
    "wood_sword",
    "wood_shovel",
    "wood_pickaxe",
    "wood_axe",
    "stone_sword",
    "stone_shovel",
    "stone_pickaxe",
    "stone_axe",
    "diamond_sword",
    "diamond_shovel",
    "diamond_pickaxe",
    "diamond_axe",
    "stick",
    "bowl",
    "mushroom_stew",
    "gold_sword",
    "gold_shovel",
    "gold_pickaxe",
    "gold_axe",
    "string",
    "feather",
    "gunpowder",
    "wood_hoe",
    "stone_hoe",
    "iron_hoe",
    "diamond_hoe",
    "gold_hoe",
    "seeds_wheat",
    "wheat",
    "bread",
    "leather_helmet",
    "leather_helmet_overlay",
    "leather_chestplate",
    "leather_chestplate_overlay",
    "leather_leggings",
    "leather_leggings_overlay",
    "leather_boots",
    "leather_boots_overlay",
    "chainmail_helmet",
    "chainmail_chestplate",
    "chainmail_leggings",
    "chainmail_boots",
    "iron_helmet",
    "iron_chestplate",
    "iron_leggings",
    "iron_boots",
    "diamond_helmet",
    "diamond_chestplate",
    "diamond_leggings",
    "diamond_boots",
    "gold_helmet",
    "gold_chestplate",
    "gold_leggings",
    "gold_boots",
    "flint",
    "porkchop_raw",
    "porkchop_cooked",
    "painting",
    "apple_golden",
    "sign",
    "door_wood",
    "bucket_empty",
    "bucket_water",
    "bucket_lava",
    "minecart_normal",
    "saddle",
    "door_iron",
    "redstone_dust",
    "snowball",
    "boat",
    "leather",
    "bucket_milk",
    "brick",
    "clay_ball",
    "reeds",
    "paper",
    "book_normal",
    "slimeball",
    "minecart_chest",
    "minecart_furnace",
    "egg",
    "compass",
    "fishing_rod_uncast",
    "clock",
    "glowstone_dust",
    "fish_cod_raw",
    "fish_salmon_raw",
    "fish_clownfish_raw",
    "fish_pufferfish_raw",
    "fish_cod_cooked",
    "fish_salmon_cooked",
    "dye_powder_black",
    "dye_powder_red",
    "dye_powder_green",
    "dye_powder_brown",
    "dye_powder_blue",
    "dye_powder_purple",
    "dye_powder_cyan",
    "dye_powder_silver",
    "dye_powder_gray",
    "dye_powder_pink",
    "dye_powder_lime",
    "dye_powder_yellow",
    "dye_powder_light_blue",
    "dye_powder_magenta",
    "dye_powder_orange",
    "dye_powder_white",
    "bone",
    "sugar",
    "cake",
    "bed",
    "repeater",
    "cookie",
    "map_filled",
    "shears",
    "melon",
    "seeds_pumpkin",
    "seeds_melon",
    "beef_raw",
    "beef_cooked",
    "chicken_raw",
    "chicken_cooked",
    "rotten_flesh",
    "ender_pearl",
    "blaze_rod",
    "ghast_tear",
    "gold_nugget",
    "nether_wart",
    "potion_bottle_drinkable",
    "potion_bottle_splash",
    "potion_overlay",
    "potion_bottle_empty",
    "spider_eye",
    "spider_eye_fermented",
    "blaze_powder",
    "magma_cream",
    "brewing_stand",
    "cauldron",
    "ender_eye",
    "melon_speckled",
    "spawn_egg",
    "spawn_egg_overlay",
    "experience_bottle",
    "fireball",
    "book_writable",
    "book_written",
    "emerald",
    "item_frame",
    "flower_pot",
    "carrot",
    "potato",
    "potato_baked",
    "potato_poisonous",
    "map_empty",
    "carrot_golden",
    "entity/skeleton/skeleton",
    "entity/skeleton/wither_skeleton",
    "entity/zombie/zombie",
    "entity/steve",
    "entity/creeper/creeper",
    "carrot_on_a_stick",
    "nether_star",
    "pumpkin_pie",
    "fireworks",
    "fireworks_charge",
    "fireworks_charge_overlay",
    "book_enchanted",
    "comparator",
    "netherbrick",
    "quartz",
    "minecart_tnt",
    "minecart_hopper",
    "prismarine_shard",
    "prismarine_crystals",
    "rabbit_raw",
    "rabbit_cooked",
    "rabbit_stew",
    "rabbit_foot",
    "rabbit_hide",
    "wooden_armorstand",
    "iron_horse_armor",
    "gold_horse_armor",
    "diamond_horse_armor",
    "lead",
    "name_tag",
    "minecart_command_block",
    "mutton_raw",
    "mutton_cooked",
    "banner_base",
    "door_spruce",
    "door_birch",
    "door_jungle",
    "door_acacia",
    "door_dark_oak",
    "record_13",
    "record_cat",
    "record_blocks",
    "record_chirp",
    "record_far",
    "record_mall",
    "record_mellohi",
    "record_stal",
    "record_strad",
    "record_ward",
    "record_11",
    "record_wait",
    // Block textures used as 2D item icons for non-full blocks
    "blocks/web",
    "blocks/ladder",
    "blocks/cactus_side",
    "items/reeds",
    "blocks/planks_oak",
    "blocks/planks_spruce",
    "blocks/planks_birch",
    "blocks/planks_jungle",
    "blocks/planks_big_oak",
    "blocks/planks_acacia",
    "items/cake",
    "blocks/iron_bars",
    "blocks/glass",
    "blocks/vine",
    "blocks/nether_brick",
    "blocks/cobblestone",
    "blocks/cobblestone_mossy",
    "blocks/glass_white",
    "blocks/glass_orange",
    "blocks/glass_magenta",
    "blocks/glass_light_blue",
    "blocks/glass_yellow",
    "blocks/glass_lime",
    "blocks/glass_pink",
    "blocks/glass_gray",
    "blocks/glass_silver",
    "blocks/glass_cyan",
    "blocks/glass_purple",
    "blocks/glass_blue",
    "blocks/glass_brown",
    "blocks/glass_green",
    "blocks/glass_red",
    "blocks/glass_black",
    "blocks/redstone_dust",
];

#[cfg(test)]
mod icon_tests {
    use super::*;

    #[test]
    fn every_mapped_icon_has_an_atlas_entry() {
        for item_id in 0..=2267 {
            for damage in 0..=15 {
                if let Some(path) = item_icon_path(item_id, damage) {
                    assert!(
                        item_icon_entries().contains(&path),
                        "missing atlas entry for item {item_id}:{damage} ({path})"
                    );
                }
            }
        }
    }

    #[test]
    fn layered_items_include_their_overlay_textures() {
        let egg = item_icon_layers(383, 65);
        assert_eq!(egg.len(), 2);
        assert_eq!(egg[0].path, "spawn_egg");
        assert_eq!(egg[1].path, "spawn_egg_overlay");
        assert_ne!(egg[0].color, egg[1].color);

        let leather = item_icon_layers(298, 0);
        assert_eq!(leather.len(), 2);
        assert_eq!(leather[1].path, "leather_helmet_overlay");
    }

    #[test]
    fn high_resolution_extrusion_uses_one_pair_of_broad_faces() {
        let image = image::RgbaImage::from_pixel(128, 128, image::Rgba([255, 255, 255, 255]));
        let mesh = build_extruded_mesh(&image);

        let broad_face_vertices = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.normal[2].abs() > 0.9)
            .count();
        assert_eq!(broad_face_vertices, 8);
        assert!(mesh.vertices.len() < 3_000);
        assert!(mesh.vertices.iter().any(|vertex| vertex.uv == [1.0, 1.0]));
    }

    #[test]
    fn high_resolution_extrusion_keeps_sub_sixteenth_contours() {
        let mut image = image::RgbaImage::new(128, 128);
        image.put_pixel(37, 64, image::Rgba([255, 255, 255, 255]));
        let mesh = build_extruded_mesh(&image);
        let expected_x = -0.5 + 37.0 / 128.0;

        assert!(mesh.vertices.iter().any(|vertex| {
            vertex.normal == [-1.0, 0.0, 0.0] && (vertex.pos[0] - expected_x).abs() < 1.0e-6
        }));
    }

    #[test]
    fn missing_block_model_uses_vanilla_third_person_display_transform() {
        let info = ItemRenderInfo::missing_block_default();
        assert_eq!(info.third_person.rotation, [10.0, -45.0, 170.0]);
        assert_eq!(info.third_person.translation, [0.0, 0.09375, -0.171875]);
        assert_eq!(info.third_person.scale, [0.375; 3]);
    }
}

fn decode_item_icon(name: &str, bytes: &[u8]) -> Option<image::RgbaImage> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    if !name.starts_with("entity/") {
        return Some(image);
    }

    // Builtin skull items use the front face of an entity skin. Resource-pack
    // skins preserve the vanilla 64-pixel layout at integer scale factors.
    let unit = (image.width() / 64).max(1);
    let x = 8 * unit;
    let y = 8 * unit;
    let size = 8 * unit;
    if x + size > image.width() || y + size > image.height() {
        return None;
    }
    Some(image::imageops::crop_imm(&image, x, y, size, size).to_image())
}

#[derive(Clone, Debug)]
pub struct LocalVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct ExtrudedMesh {
    pub vertices: Vec<LocalVertex>,
    pub indices: Vec<u32>,
}

fn build_extruded_mesh(rgba: &image::RgbaImage) -> ExtrudedMesh {
    const ALPHA_CUTOFF: u8 = 26;
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return ExtrudedMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
    }

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let half_thickness = 0.03125;
    let mut push_quad = |positions: [[f32; 3]; 4], normal: [f32; 3], uvs: [[f32; 2]; 4]| {
        let start = vertices.len() as u32;
        for (pos, uv) in positions.into_iter().zip(uvs) {
            vertices.push(LocalVertex { pos, normal, uv });
        }
        indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    };

    // Alpha discard preserves the full-resolution silhouette on the two broad
    // faces without generating a pair of quads for every opaque source pixel.
    push_quad(
        [
            [0.5, 0.5, half_thickness],
            [-0.5, 0.5, half_thickness],
            [-0.5, -0.5, half_thickness],
            [0.5, -0.5, half_thickness],
        ],
        [0.0, 0.0, 1.0],
        [[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
    );
    push_quad(
        [
            [-0.5, 0.5, -half_thickness],
            [0.5, 0.5, -half_thickness],
            [0.5, -0.5, -half_thickness],
            [-0.5, -0.5, -half_thickness],
        ],
        [0.0, 0.0, -1.0],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    );

    let opaque = |x: u32, y: u32| rgba.get_pixel(x, y)[3] >= ALPHA_CUTOFF;
    for row in 0..height {
        for column in 0..width {
            if !opaque(column, row) {
                continue;
            }
            let x_min = -0.5 + column as f32 / width as f32;
            let x_max = -0.5 + (column + 1) as f32 / width as f32;
            let y_min = 0.5 - (row + 1) as f32 / height as f32;
            let y_max = 0.5 - row as f32 / height as f32;
            let u = (column as f32 + 0.5) / width as f32;
            let v = (row as f32 + 0.5) / height as f32;
            let side_uvs = [[u, v]; 4];

            if column == 0 || !opaque(column - 1, row) {
                push_quad(
                    [
                        [x_min, y_max, -half_thickness],
                        [x_min, y_max, half_thickness],
                        [x_min, y_min, half_thickness],
                        [x_min, y_min, -half_thickness],
                    ],
                    [-1.0, 0.0, 0.0],
                    side_uvs,
                );
            }
            if column + 1 == width || !opaque(column + 1, row) {
                push_quad(
                    [
                        [x_max, y_max, half_thickness],
                        [x_max, y_max, -half_thickness],
                        [x_max, y_min, -half_thickness],
                        [x_max, y_min, half_thickness],
                    ],
                    [1.0, 0.0, 0.0],
                    side_uvs,
                );
            }
            if row == 0 || !opaque(column, row - 1) {
                push_quad(
                    [
                        [x_max, y_max, half_thickness],
                        [x_min, y_max, half_thickness],
                        [x_min, y_max, -half_thickness],
                        [x_max, y_max, -half_thickness],
                    ],
                    [0.0, 1.0, 0.0],
                    side_uvs,
                );
            }
            if row + 1 == height || !opaque(column, row + 1) {
                push_quad(
                    [
                        [x_max, y_min, -half_thickness],
                        [x_min, y_min, -half_thickness],
                        [x_min, y_min, half_thickness],
                        [x_max, y_min, half_thickness],
                    ],
                    [0.0, -1.0, 0.0],
                    side_uvs,
                );
            }
        }
    }

    ExtrudedMesh { vertices, indices }
}

#[derive(Clone, Copy, Debug)]
pub struct ItemCameraTransform {
    pub rotation: [f32; 3],
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

impl ItemCameraTransform {
    const IDENTITY: Self = Self {
        rotation: [0.0; 3],
        translation: [0.0; 3],
        scale: [1.0; 3],
    };

    const GENERATED_FIRST_PERSON: Self = Self {
        rotation: [0.0, -135.0, 25.0],
        translation: [0.0, 0.25, 0.125],
        scale: [1.7; 3],
    };

    const GENERATED_THIRD_PERSON: Self = Self {
        rotation: [-90.0, 0.0, 0.0],
        translation: [0.0, 0.0625, -0.1875],
        scale: [0.55; 3],
    };

    const BLOCK_THIRD_PERSON: Self = Self {
        rotation: [10.0, -45.0, 170.0],
        translation: [0.0, 0.09375, -0.171875],
        scale: [0.375; 3],
    };
}

#[derive(Clone, Copy, Debug)]
pub struct ItemRenderInfo {
    pub generated: bool,
    pub first_person: ItemCameraTransform,
    pub third_person: ItemCameraTransform,
}

impl ItemRenderInfo {
    fn block_default() -> Self {
        Self {
            generated: false,
            first_person: ItemCameraTransform::IDENTITY,
            third_person: ItemCameraTransform::IDENTITY,
        }
    }

    fn generated_default() -> Self {
        Self {
            generated: true,
            first_person: ItemCameraTransform::GENERATED_FIRST_PERSON,
            third_person: ItemCameraTransform::GENERATED_THIRD_PERSON,
        }
    }

    fn missing_block_default() -> Self {
        Self {
            generated: false,
            first_person: ItemCameraTransform::IDENTITY,
            third_person: ItemCameraTransform::BLOCK_THIRD_PERSON,
        }
    }
}

#[derive(Default, Deserialize)]
struct RawItemModel {
    #[serde(default)]
    parent: String,
    #[serde(default)]
    display: RawItemDisplay,
}

#[derive(Default, Deserialize)]
struct RawItemDisplay {
    firstperson: Option<RawItemTransform>,
    thirdperson: Option<RawItemTransform>,
}

#[derive(Deserialize)]
struct RawItemTransform {
    rotation: Option<[f32; 3]>,
    translation: Option<[f32; 3]>,
    scale: Option<[f32; 3]>,
}

impl RawItemTransform {
    fn into_camera_transform(self) -> ItemCameraTransform {
        let mut translation = self.translation.unwrap_or([0.0; 3]);
        for component in &mut translation {
            *component = (*component).clamp(-80.0, 80.0) / 16.0;
        }
        ItemCameraTransform {
            rotation: self.rotation.unwrap_or([0.0; 3]),
            translation,
            scale: self.scale.unwrap_or([1.0; 3]),
        }
    }
}

static EXTRUDED_MESH_CACHE: std::sync::Mutex<Option<HashMap<(u16, u16), Arc<ExtrudedMesh>>>> =
    std::sync::Mutex::new(None);
static ITEM_RENDER_INFO_CACHE: std::sync::Mutex<Option<HashMap<u16, ItemRenderInfo>>> =
    std::sync::Mutex::new(None);

/// Returns a shared immutable mesh so the first-person path never clones a
/// potentially large per-pixel mesh every frame.
pub fn get_cached_extruded_mesh(item_id: u16, damage: u16) -> Option<Arc<ExtrudedMesh>> {
    EXTRUDED_MESH_CACHE.lock().ok().and_then(|c| {
        c.as_ref().and_then(|m| {
            // Tool durability is not a texture variant.  The cache stores
            // visual variants 0..15, so all higher durability values must
            // resolve to the base icon instead of falling back to a slab.
            m.get(&(item_id, damage))
                .or_else(|| m.get(&(item_id, 0)))
                .cloned()
        })
    })
}

pub fn item_render_info(item_id: u16) -> ItemRenderInfo {
    ITEM_RENDER_INFO_CACHE
        .lock()
        .ok()
        .and_then(|cache| {
            cache
                .as_ref()
                .and_then(|items| items.get(&item_id).copied())
        })
        .unwrap_or_else(|| {
            if item_id > 255 {
                ItemRenderInfo::generated_default()
            } else {
                ItemRenderInfo::missing_block_default()
            }
        })
}

fn item_model_candidates(item_id: u16) -> Vec<String> {
    if item_id <= 255 {
        return crate::world::block_models::block_id_to_name(item_id)
            .map(|name| vec![name.to_string()])
            .unwrap_or_default();
    }

    let Some(icon_name) = item_icon_path(item_id, 0) else {
        return Vec::new();
    };
    let icon_name = icon_name.rsplit('/').next().unwrap_or(icon_name);
    let canonical = match item_id {
        261 => "bow",
        322 => "golden_apple",
        325 => "bucket",
        326 => "water_bucket",
        327 => "lava_bucket",
        335 => "milk_bucket",
        340 => "book",
        346 => "fishing_rod",
        358 => "filled_map",
        395 => "map",
        416 => "armor_stand",
        _ if icon_name.starts_with("wood_") => {
            return vec![
                icon_name.to_string(),
                icon_name.replacen("wood_", "wooden_", 1),
            ];
        }
        _ if icon_name.starts_with("gold_") => {
            return vec![
                icon_name.to_string(),
                icon_name.replacen("gold_", "golden_", 1),
            ];
        }
        _ => icon_name,
    };
    if canonical == icon_name {
        vec![icon_name.to_string()]
    } else {
        vec![canonical.to_string(), icon_name.to_string()]
    }
}

fn load_item_render_info(
    resolver: &mut crate::assets::resolver::AssetResolver,
    model_name: &str,
    depth: usize,
) -> Option<ItemRenderInfo> {
    if depth >= 8 {
        return None;
    }
    let path = format!("minecraft/models/item/{}.json", model_name);
    let raw: RawItemModel = serde_json::from_slice(&resolver.read_bytes(&path)?).ok()?;

    let mut info = if raw.parent == "builtin/generated" {
        ItemRenderInfo::generated_default()
    } else if let Some(parent) = raw.parent.strip_prefix("item/") {
        load_item_render_info(resolver, parent, depth + 1)
            .unwrap_or_else(ItemRenderInfo::block_default)
    } else {
        ItemRenderInfo::block_default()
    };
    if let Some(first_person) = raw.display.firstperson {
        info.first_person = first_person.into_camera_transform();
    }
    if let Some(third_person) = raw.display.thirdperson {
        info.third_person = third_person.into_camera_transform();
    }
    Some(info)
}

fn precompute_item_render_info(resolver: &mut crate::assets::resolver::AssetResolver) {
    let mut cache = HashMap::new();
    for item_id in 1..=431 {
        for model_name in item_model_candidates(item_id) {
            if let Some(info) = load_item_render_info(resolver, &model_name, 0) {
                cache.insert(item_id, info);
                break;
            }
        }
    }
    if let Ok(mut global) = ITEM_RENDER_INFO_CACHE.lock() {
        *global = Some(cache);
    }
}

pub fn precompute_item_meshes(base_path: &str) {
    let mut resolver = crate::assets::resolver::AssetResolver::new(base_path);
    precompute_item_meshes_with_resolver(&mut resolver);
}

pub fn precompute_item_meshes_with_resolver(resolver: &mut crate::assets::resolver::AssetResolver) {
    precompute_item_render_info(resolver);
    let mut cache = HashMap::new();
    let mut meshes_by_texture = HashMap::<&'static str, Arc<ExtrudedMesh>>::new();
    for item_id in 1..450 {
        for damage in 0..16 {
            if let Some(name) = item_icon_path(item_id, damage) {
                if let Some(mesh) = meshes_by_texture.get(name) {
                    cache.insert((item_id, damage), Arc::clone(mesh));
                    continue;
                }
                let path = item_icon_resource_path(name);
                if let Some(bytes) = resolver.read_bytes(&path) {
                    let Some(img) = decode_item_icon(name, &bytes) else {
                        continue;
                    };
                    // The broad faces use alpha discard at full texture resolution.
                    // Only the contour creates extra geometry, keeping 128x packs sharp
                    // without the area-proportional cost of per-pixel front/back quads.
                    let source = img;
                    let target_width = source.width().min(ITEM_ATLAS_CELL);
                    let target_height = source.height().min(ITEM_ATLAS_CELL);
                    let rgba = if source.width() != target_width || source.height() != target_height
                    {
                        image::imageops::resize(
                            &source,
                            target_width,
                            target_height,
                            image::imageops::FilterType::Nearest,
                        )
                    } else {
                        source
                    };
                    let mesh = Arc::new(build_extruded_mesh(&rgba));
                    meshes_by_texture.insert(name, Arc::clone(&mesh));
                    cache.insert((item_id, damage), mesh);
                }
            }
        }
    }
    if let Ok(mut c) = EXTRUDED_MESH_CACHE.lock() {
        *c = Some(cache);
    }
}
