//! Item type definitions — full MC 1.8.9 item registry.
//!
//! Non-block items use IDs 256+. Block-item IDs mirror their block ID (1–197).
//! Some items have subtypes via damage/metadata (dye, fish, spawn_egg, etc.).

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Item {
    // Tools
    IronShovel,
    IronPickaxe,
    IronAxe,
    WoodenSword,
    WoodenShovel,
    WoodenPickaxe,
    WoodenAxe,
    StoneSword,
    StoneShovel,
    StonePickaxe,
    StoneAxe,
    DiamondSword,
    DiamondShovel,
    DiamondPickaxe,
    DiamondAxe,
    GoldenSword,
    GoldenShovel,
    GoldenPickaxe,
    GoldenAxe,
    IronSword,
    FlintSteel,
    Bow,
    Arrow,
    FishingRod,
    Shears,

    // Hoes
    WoodenHoe,
    StoneHoe,
    IronHoe,
    DiamondHoe,
    GoldenHoe,

    // Armor
    LeatherHelmet,
    LeatherChestplate,
    LeatherLeggings,
    LeatherBoots,
    ChainmailHelmet,
    ChainmailChestplate,
    ChainmailLeggings,
    ChainmailBoots,
    IronHelmet,
    IronChestplate,
    IronLeggings,
    IronBoots,
    DiamondHelmet,
    DiamondChestplate,
    DiamondLeggings,
    DiamondBoots,
    GoldenHelmet,
    GoldenChestplate,
    GoldenLeggings,
    GoldenBoots,

    // Food & produce
    Apple,
    GoldenApple,
    Bread,
    Cookie,
    MelonSlice,
    PumpkinPie,
    RawPorkchop,
    CookedPorkchop,
    RawBeef,
    CookedBeef,
    RawChicken,
    CookedChicken,
    RawMutton,
    CookedMutton,
    RawRabbit,
    CookedRabbit,
    RawFish,
    CookedFish,
    GoldenCarrot,
    BakedPotato,
    PoisonousPotato,
    MushroomStew,
    RabbitStew,
    RottenFlesh,
    SpiderEye,
    Cake,
    Bowl,

    // Seeds & crops (also food)
    WheatSeeds,
    Wheat,
    PumpkinSeeds,
    MelonSeeds,
    Carrot,
    Potato,
    Sugar,
    Reeds,

    // Potions & brewing
    Potion,
    SplashPotion,
    GlassBottle,
    BottleOEnchanting,
    ExperienceBottle,
    NetherWart,
    BlazeRod,
    BlazePowder,
    GhastTear,
    MagmaCream,
    GlowstoneDust,
    FermentedSpiderEye,
    SpeckledMelon,
    GoldNugget,

    // Materials
    Stick,
    String,
    Feather,
    Gunpowder,
    Leather,
    Paper,
    Book,
    SlimeBall,
    Diamond,
    IronIngot,
    GoldIngot,
    Coal,
    Bone,
    BoneMeal,
    Redstone,
    LapisLazuli,
    Emerald,
    NetherBrick,
    NetherQuartz,
    NetherStar,
    Flint,
    Brick,
    ClayBall,

    // Dyes (subtypes of ID 351)
    InkSac,
    RoseRed,
    CactusGreen,
    CocoaBeans,

    // Buckets & vehicles
    Bucket,
    WaterBucket,
    LavaBucket,
    MilkBucket,
    Minecart,
    ChestMinecart,
    FurnaceMinecart,
    TntMinecart,
    HopperMinecart,
    CommandBlockMinecart,
    Boat,
    Saddle,

    // Decoration & utility
    Painting,
    Sign,
    Bed,
    Repeater,
    Comparator,
    ItemFrame,
    FlowerPot,
    Skull,
    EmptyMap,
    Map,
    Compass,
    Clock,
    Fireworks,
    FireworkCharge,
    EnchantedBook,
    WritableBook,
    WrittenBook,
    FireCharge,
    Lead,
    NameTag,
    CarrotOnAStick,
    ArmorStand,

    // Doors (items)
    OakDoor,
    SpruceDoor,
    BirchDoor,
    JungleDoor,
    AcaciaDoor,
    DarkOakDoor,
    IronDoor,

    // Horse armor
    IronHorseArmor,
    GoldenHorseArmor,
    DiamondHorseArmor,

    // Spawn egg
    SpawnEgg,

    // Music discs
    Record13,
    RecordCat,
    RecordBlocks,
    RecordChirp,
    RecordFar,
    RecordMall,
    RecordMellohi,
    RecordStal,
    RecordStrad,
    RecordWard,
    Record11,
    RecordWait,

    // 1.8 additions
    PrismarineShard,
    PrismarineCrystals,
    Banner,

    // Fallback
    Unknown,
}

impl Item {
    pub fn to_id(self) -> u16 {
        match self {
            // Tools (swords, etc.)
            Self::IronSword => 267,
            Self::IronShovel => 256,
            Self::IronPickaxe => 257,
            Self::IronAxe => 258,
            Self::FlintSteel => 259,
            Self::Bow => 261,
            Self::Arrow => 262,
            Self::WoodenSword => 268,
            Self::WoodenShovel => 269,
            Self::WoodenPickaxe => 270,
            Self::WoodenAxe => 271,
            Self::StoneSword => 272,
            Self::StoneShovel => 273,
            Self::StonePickaxe => 274,
            Self::StoneAxe => 275,
            Self::DiamondSword => 276,
            Self::DiamondShovel => 277,
            Self::DiamondPickaxe => 278,
            Self::DiamondAxe => 279,
            Self::GoldenSword => 283,
            Self::GoldenShovel => 284,
            Self::GoldenPickaxe => 285,
            Self::GoldenAxe => 286,
            Self::Shears => 359,
            Self::FishingRod => 346,

            // Hoes
            Self::WoodenHoe => 290,
            Self::StoneHoe => 291,
            Self::IronHoe => 292,
            Self::DiamondHoe => 293,
            Self::GoldenHoe => 294,

            // Armor
            Self::LeatherHelmet => 298,
            Self::LeatherChestplate => 299,
            Self::LeatherLeggings => 300,
            Self::LeatherBoots => 301,
            Self::ChainmailHelmet => 302,
            Self::ChainmailChestplate => 303,
            Self::ChainmailLeggings => 304,
            Self::ChainmailBoots => 305,
            Self::IronHelmet => 306,
            Self::IronChestplate => 307,
            Self::IronLeggings => 308,
            Self::IronBoots => 309,
            Self::DiamondHelmet => 310,
            Self::DiamondChestplate => 311,
            Self::DiamondLeggings => 312,
            Self::DiamondBoots => 313,
            Self::GoldenHelmet => 314,
            Self::GoldenChestplate => 315,
            Self::GoldenLeggings => 316,
            Self::GoldenBoots => 317,

            // Food
            Self::Apple => 260,
            Self::GoldenApple => 322,
            Self::Bread => 297,
            Self::Cookie => 357,
            Self::MelonSlice => 360,
            Self::PumpkinPie => 400,
            Self::Cake => 354,
            Self::Bowl => 281,
            Self::MushroomStew => 282,
            Self::RabbitStew => 413,
            Self::RawPorkchop => 319,
            Self::CookedPorkchop => 320,
            Self::RawBeef => 363,
            Self::CookedBeef => 364,
            Self::RawChicken => 365,
            Self::CookedChicken => 366,
            Self::RawMutton => 423,
            Self::CookedMutton => 424,
            Self::RawRabbit => 411,
            Self::CookedRabbit => 412,
            Self::RawFish => 349,
            Self::CookedFish => 350,
            Self::GoldenCarrot => 396,
            Self::Carrot => 391,
            Self::Potato => 392,
            Self::BakedPotato => 393,
            Self::PoisonousPotato => 394,
            Self::RottenFlesh => 367,
            Self::SpiderEye => 375,
            Self::Sugar => 353,

            // Seeds & crops
            Self::WheatSeeds => 295,
            Self::Wheat => 296,
            Self::PumpkinSeeds => 361,
            Self::MelonSeeds => 362,
            Self::Reeds => 338,

            // Potions & brewing
            Self::Potion => 373,
            Self::SplashPotion => 438,
            Self::GlassBottle => 374,
            Self::BottleOEnchanting => 384,
            Self::ExperienceBottle => 384,
            Self::NetherWart => 372,
            Self::BlazeRod => 369,
            Self::BlazePowder => 377,
            Self::GhastTear => 370,
            Self::MagmaCream => 378,
            Self::GlowstoneDust => 348,
            Self::FermentedSpiderEye => 376,
            Self::SpeckledMelon => 382,
            Self::GoldNugget => 371,

            // Materials
            Self::Stick => 280,
            Self::String => 287,
            Self::Feather => 288,
            Self::Gunpowder => 289,
            Self::Leather => 334,
            Self::Paper => 339,
            Self::Book => 340,
            Self::SlimeBall => 341,
            Self::Diamond => 264,
            Self::IronIngot => 265,
            Self::GoldIngot => 266,
            Self::Coal => 263,
            Self::Bone => 352,
            Self::BoneMeal => 351,
            Self::Redstone => 331,
            Self::LapisLazuli => 351,
            Self::Emerald => 388,
            Self::NetherBrick => 405,
            Self::NetherQuartz => 406,
            Self::NetherStar => 399,
            Self::Flint => 318,
            Self::Brick => 336,
            Self::ClayBall => 337,

            // Dyes (subtypes of ID 351)
            Self::InkSac => 351,
            Self::RoseRed => 351,
            Self::CactusGreen => 351,
            Self::CocoaBeans => 351,

            // Buckets & vehicles
            Self::Bucket => 325,
            Self::WaterBucket => 326,
            Self::LavaBucket => 327,
            Self::MilkBucket => 335,
            Self::Minecart => 328,
            Self::ChestMinecart => 342,
            Self::FurnaceMinecart => 343,
            Self::TntMinecart => 407,
            Self::HopperMinecart => 408,
            Self::CommandBlockMinecart => 422,
            Self::Boat => 333,
            Self::Saddle => 329,

            // Decoration & utility
            Self::Painting => 321,
            Self::Sign => 323,
            Self::Bed => 355,
            Self::Repeater => 356,
            Self::Comparator => 404,
            Self::ItemFrame => 389,
            Self::FlowerPot => 390,
            Self::Skull => 397,
            Self::ArmorStand => 416,
            Self::EmptyMap => 395,
            Self::Map => 358,
            Self::Compass => 345,
            Self::Clock => 347,
            Self::Fireworks => 401,
            Self::FireworkCharge => 402,
            Self::EnchantedBook => 403,
            Self::WritableBook => 386,
            Self::WrittenBook => 387,
            Self::FireCharge => 385,
            Self::Lead => 420,
            Self::NameTag => 421,
            Self::CarrotOnAStick => 398,

            // Doors (items)
            Self::OakDoor => 324,
            Self::SpruceDoor => 427,
            Self::BirchDoor => 428,
            Self::JungleDoor => 429,
            Self::AcaciaDoor => 430,
            Self::DarkOakDoor => 431,
            Self::IronDoor => 330,

            // Horse armor
            Self::IronHorseArmor => 417,
            Self::GoldenHorseArmor => 418,
            Self::DiamondHorseArmor => 419,

            // Spawn egg
            Self::SpawnEgg => 383,

            // Music discs
            Self::Record13 => 2256,
            Self::RecordCat => 2257,
            Self::RecordBlocks => 2258,
            Self::RecordChirp => 2259,
            Self::RecordFar => 2260,
            Self::RecordMall => 2261,
            Self::RecordMellohi => 2262,
            Self::RecordStal => 2263,
            Self::RecordStrad => 2264,
            Self::RecordWard => 2265,
            Self::Record11 => 2266,
            Self::RecordWait => 2267,

            // 1.8 additions
            Self::PrismarineShard => 409,
            Self::PrismarineCrystals => 410,
            Self::Banner => 425,

            Self::Unknown => 0,
        }
    }

    pub fn max_stack(self) -> u8 {
        match self {
            // Tools & weapons
            Self::IronSword
            | Self::WoodenSword
            | Self::StoneSword
            | Self::DiamondSword
            | Self::GoldenSword
            | Self::IronShovel
            | Self::WoodenShovel
            | Self::StoneShovel
            | Self::DiamondShovel
            | Self::GoldenShovel
            | Self::IronPickaxe
            | Self::WoodenPickaxe
            | Self::StonePickaxe
            | Self::DiamondPickaxe
            | Self::GoldenPickaxe
            | Self::IronAxe
            | Self::WoodenAxe
            | Self::StoneAxe
            | Self::DiamondAxe
            | Self::GoldenAxe
            | Self::WoodenHoe
            | Self::StoneHoe
            | Self::IronHoe
            | Self::DiamondHoe
            | Self::GoldenHoe
            | Self::FlintSteel
            | Self::Bow
            | Self::FishingRod
            | Self::Shears => 1,

            // Armor
            Self::LeatherHelmet
            | Self::LeatherChestplate
            | Self::LeatherLeggings
            | Self::LeatherBoots
            | Self::ChainmailHelmet
            | Self::ChainmailChestplate
            | Self::ChainmailLeggings
            | Self::ChainmailBoots
            | Self::IronHelmet
            | Self::IronChestplate
            | Self::IronLeggings
            | Self::IronBoots
            | Self::DiamondHelmet
            | Self::DiamondChestplate
            | Self::DiamondLeggings
            | Self::DiamondBoots
            | Self::GoldenHelmet
            | Self::GoldenChestplate
            | Self::GoldenLeggings
            | Self::GoldenBoots => 1,

            // Potions & bottles
            Self::Potion
            | Self::SplashPotion
            | Self::GlassBottle
            | Self::BottleOEnchanting
            | Self::ExperienceBottle => 1,

            // Buckets
            Self::Bucket | Self::WaterBucket | Self::LavaBucket | Self::MilkBucket => 16,

            // Minecarts, boats, saddle
            Self::Minecart
            | Self::ChestMinecart
            | Self::FurnaceMinecart
            | Self::TntMinecart
            | Self::HopperMinecart
            | Self::CommandBlockMinecart
            | Self::Boat
            | Self::Saddle => 1,

            // Special items
            Self::Cake | Self::MushroomStew | Self::RabbitStew => 1,
            Self::EnchantedBook | Self::WrittenBook | Self::WritableBook => 1,
            Self::ArmorStand => 16,
            Self::Bed
            | Self::EmptyMap
            | Self::Map
            | Self::Compass
            | Self::Clock
            | Self::Lead
            | Self::NameTag
            | Self::CarrotOnAStick => 1,
            Self::Fireworks | Self::FireworkCharge => 1,
            Self::Skull => 1,
            Self::Sign => 16,
            Self::FireCharge => 1,
            Self::Banner => 16,

            // Music discs
            Self::Record13
            | Self::RecordCat
            | Self::RecordBlocks
            | Self::RecordChirp
            | Self::RecordFar
            | Self::RecordMall
            | Self::RecordMellohi
            | Self::RecordStal
            | Self::RecordStrad
            | Self::RecordWard
            | Self::Record11
            | Self::RecordWait => 1,

            // Everything else stacks to 64
            _ => 64,
        }
    }
}

pub fn item_from_id(id: u16) -> Item {
    match id {
        // Tools
        267 => Item::IronSword,
        256 => Item::IronShovel,
        257 => Item::IronPickaxe,
        258 => Item::IronAxe,
        259 => Item::FlintSteel,
        261 => Item::Bow,
        262 => Item::Arrow,
        346 => Item::FishingRod,
        359 => Item::Shears,

        // Wood tools
        268 => Item::WoodenSword,
        269 => Item::WoodenShovel,
        270 => Item::WoodenPickaxe,
        271 => Item::WoodenAxe,
        // Stone tools
        272 => Item::StoneSword,
        273 => Item::StoneShovel,
        274 => Item::StonePickaxe,
        275 => Item::StoneAxe,
        // Diamond tools
        276 => Item::DiamondSword,
        277 => Item::DiamondShovel,
        278 => Item::DiamondPickaxe,
        279 => Item::DiamondAxe,
        // Golden tools
        283 => Item::GoldenSword,
        284 => Item::GoldenShovel,
        285 => Item::GoldenPickaxe,
        286 => Item::GoldenAxe,

        // Hoes
        290 => Item::WoodenHoe,
        291 => Item::StoneHoe,
        292 => Item::IronHoe,
        293 => Item::DiamondHoe,
        294 => Item::GoldenHoe,

        // Armor
        298 => Item::LeatherHelmet,
        299 => Item::LeatherChestplate,
        300 => Item::LeatherLeggings,
        301 => Item::LeatherBoots,
        302 => Item::ChainmailHelmet,
        303 => Item::ChainmailChestplate,
        304 => Item::ChainmailLeggings,
        305 => Item::ChainmailBoots,
        306 => Item::IronHelmet,
        307 => Item::IronChestplate,
        308 => Item::IronLeggings,
        309 => Item::IronBoots,
        310 => Item::DiamondHelmet,
        311 => Item::DiamondChestplate,
        312 => Item::DiamondLeggings,
        313 => Item::DiamondBoots,
        314 => Item::GoldenHelmet,
        315 => Item::GoldenChestplate,
        316 => Item::GoldenLeggings,
        317 => Item::GoldenBoots,

        // Food
        260 => Item::Apple,
        322 => Item::GoldenApple,
        297 => Item::Bread,
        357 => Item::Cookie,
        360 => Item::MelonSlice,
        400 => Item::PumpkinPie,
        354 => Item::Cake,
        281 => Item::Bowl,
        282 => Item::MushroomStew,
        413 => Item::RabbitStew,
        319 => Item::RawPorkchop,
        320 => Item::CookedPorkchop,
        363 => Item::RawBeef,
        364 => Item::CookedBeef,
        365 => Item::RawChicken,
        366 => Item::CookedChicken,
        423 => Item::RawMutton,
        424 => Item::CookedMutton,
        411 => Item::RawRabbit,
        412 => Item::CookedRabbit,
        349 => Item::RawFish,
        350 => Item::CookedFish,
        396 => Item::GoldenCarrot,
        391 => Item::Carrot,
        392 => Item::Potato,
        393 => Item::BakedPotato,
        394 => Item::PoisonousPotato,
        367 => Item::RottenFlesh,
        375 => Item::SpiderEye,
        353 => Item::Sugar,

        // Seeds & crops
        295 => Item::WheatSeeds,
        296 => Item::Wheat,
        361 => Item::PumpkinSeeds,
        362 => Item::MelonSeeds,
        338 => Item::Reeds,

        // Potions & brewing
        373 => Item::Potion,
        438 => Item::SplashPotion,
        374 => Item::GlassBottle,
        384 => Item::BottleOEnchanting,
        372 => Item::NetherWart,
        369 => Item::BlazeRod,
        377 => Item::BlazePowder,
        370 => Item::GhastTear,
        378 => Item::MagmaCream,
        348 => Item::GlowstoneDust,
        376 => Item::FermentedSpiderEye,
        382 => Item::SpeckledMelon,
        371 => Item::GoldNugget,

        // Materials
        280 => Item::Stick,
        287 => Item::String,
        288 => Item::Feather,
        289 => Item::Gunpowder,
        334 => Item::Leather,
        339 => Item::Paper,
        340 => Item::Book,
        341 => Item::SlimeBall,
        264 => Item::Diamond,
        265 => Item::IronIngot,
        266 => Item::GoldIngot,
        263 => Item::Coal,
        352 => Item::Bone,
        331 => Item::Redstone,
        388 => Item::Emerald,
        405 => Item::NetherBrick,
        406 => Item::NetherQuartz,
        399 => Item::NetherStar,
        318 => Item::Flint,
        336 => Item::Brick,
        337 => Item::ClayBall,

        // Dyes (all 351)
        351 => Item::BoneMeal,

        // Buckets & vehicles
        325 => Item::Bucket,
        326 => Item::WaterBucket,
        327 => Item::LavaBucket,
        335 => Item::MilkBucket,
        328 => Item::Minecart,
        342 => Item::ChestMinecart,
        343 => Item::FurnaceMinecart,
        407 => Item::TntMinecart,
        408 => Item::HopperMinecart,
        422 => Item::CommandBlockMinecart,
        333 => Item::Boat,
        329 => Item::Saddle,

        // Decoration & utility
        321 => Item::Painting,
        323 => Item::Sign,
        355 => Item::Bed,
        356 => Item::Repeater,
        404 => Item::Comparator,
        389 => Item::ItemFrame,
        390 => Item::FlowerPot,
        397 => Item::Skull,
        416 => Item::ArmorStand,
        395 => Item::EmptyMap,
        358 => Item::Map,
        345 => Item::Compass,
        347 => Item::Clock,
        401 => Item::Fireworks,
        402 => Item::FireworkCharge,
        403 => Item::EnchantedBook,
        386 => Item::WritableBook,
        387 => Item::WrittenBook,
        385 => Item::FireCharge,
        420 => Item::Lead,
        421 => Item::NameTag,
        398 => Item::CarrotOnAStick,

        // Doors (items)
        324 => Item::OakDoor,
        427 => Item::SpruceDoor,
        428 => Item::BirchDoor,
        429 => Item::JungleDoor,
        430 => Item::AcaciaDoor,
        431 => Item::DarkOakDoor,
        330 => Item::IronDoor,

        // Horse armor
        417 => Item::IronHorseArmor,
        418 => Item::GoldenHorseArmor,
        419 => Item::DiamondHorseArmor,

        // Spawn egg
        383 => Item::SpawnEgg,

        // Music discs
        2256 => Item::Record13,
        2257 => Item::RecordCat,
        2258 => Item::RecordBlocks,
        2259 => Item::RecordChirp,
        2260 => Item::RecordFar,
        2261 => Item::RecordMall,
        2262 => Item::RecordMellohi,
        2263 => Item::RecordStal,
        2264 => Item::RecordStrad,
        2265 => Item::RecordWard,
        2266 => Item::Record11,
        2267 => Item::RecordWait,

        // 1.8 additions
        409 => Item::PrismarineShard,
        410 => Item::PrismarineCrystals,
        425 => Item::Banner,

        _ => Item::Unknown,
    }
}
