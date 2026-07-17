use super::Block;

/// Properties for a block type (used by the rendering and physics systems).
pub struct BlockProperties {
    pub id: u16,
    pub name: &'static str,
    pub hardness: f32,
    pub resistance: f32,
    pub is_opaque: bool,
    pub light_level: u8,
    pub slipperiness: f32,
}

impl Block {
    #[inline]
    pub fn properties(self) -> BlockProperties {
        match self {
            Block::Air => BlockProperties {
                id: 0,
                name: "air",
                hardness: 0.0,
                resistance: 0.0,
                is_opaque: false,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Stone => BlockProperties {
                id: 1,
                name: "stone",
                hardness: 1.5,
                resistance: 10.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Grass => BlockProperties {
                id: 2,
                name: "grass",
                hardness: 0.6,
                resistance: 0.6,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Dirt => BlockProperties {
                id: 3,
                name: "dirt",
                hardness: 0.5,
                resistance: 0.5,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Cobblestone => BlockProperties {
                id: 4,
                name: "cobblestone",
                hardness: 2.0,
                resistance: 10.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Planks => BlockProperties {
                id: 5,
                name: "planks",
                hardness: 2.0,
                resistance: 5.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Bedrock => BlockProperties {
                id: 7,
                name: "bedrock",
                hardness: -1.0,
                resistance: 18000000.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Sand => BlockProperties {
                id: 12,
                name: "sand",
                hardness: 0.5,
                resistance: 0.5,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Gravel => BlockProperties {
                id: 13,
                name: "gravel",
                hardness: 0.6,
                resistance: 0.6,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::GoldOre => BlockProperties {
                id: 14,
                name: "gold_ore",
                hardness: 3.0,
                resistance: 3.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::IronOre => BlockProperties {
                id: 15,
                name: "iron_ore",
                hardness: 3.0,
                resistance: 3.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::CoalOre => BlockProperties {
                id: 16,
                name: "coal_ore",
                hardness: 3.0,
                resistance: 3.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Log => BlockProperties {
                id: 17,
                name: "log",
                hardness: 2.0,
                resistance: 2.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Glass => BlockProperties {
                id: 20,
                name: "glass",
                hardness: 0.3,
                resistance: 0.3,
                is_opaque: false,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::DiamondOre => BlockProperties {
                id: 56,
                name: "diamond_ore",
                hardness: 3.0,
                resistance: 3.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::DiamondBlock => BlockProperties {
                id: 57,
                name: "diamond_block",
                hardness: 5.0,
                resistance: 6.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::FlowingWater => BlockProperties {
                id: 8,
                name: "flowing_water",
                hardness: 100.0,
                resistance: 500.0,
                is_opaque: false,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::StillWater => BlockProperties {
                id: 9,
                name: "water",
                hardness: 100.0,
                resistance: 500.0,
                is_opaque: false,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::FlowingLava => BlockProperties {
                id: 10,
                name: "flowing_lava",
                hardness: 100.0,
                resistance: 500.0,
                is_opaque: false,
                light_level: 15,
                slipperiness: 0.6,
            },
            Block::StillLava => BlockProperties {
                id: 11,
                name: "lava",
                hardness: 100.0,
                resistance: 500.0,
                is_opaque: false,
                light_level: 15,
                slipperiness: 0.6,
            },
            Block::Glowstone => BlockProperties {
                id: 89,
                name: "glowstone",
                hardness: 0.3,
                resistance: 0.3,
                is_opaque: true,
                light_level: 15,
                slipperiness: 0.6,
            },
            Block::Ice => BlockProperties {
                id: 79,
                name: "ice",
                hardness: 0.5,
                resistance: 0.5,
                is_opaque: false,
                light_level: 0,
                slipperiness: 0.98,
            },
            Block::SnowBlock => BlockProperties {
                id: 80,
                name: "snow_block",
                hardness: 0.2,
                resistance: 0.2,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Netherrack => BlockProperties {
                id: 87,
                name: "netherrack",
                hardness: 0.4,
                resistance: 0.4,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::Obsidian => BlockProperties {
                id: 49,
                name: "obsidian",
                hardness: 50.0,
                resistance: 6000.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::EmeraldOre => BlockProperties {
                id: 129,
                name: "emerald_ore",
                hardness: 3.0,
                resistance: 3.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            Block::CoalBlock => BlockProperties {
                id: 173,
                name: "coal_block",
                hardness: 5.0,
                resistance: 6.0,
                is_opaque: true,
                light_level: 0,
                slipperiness: 0.6,
            },
            _ => {
                // Comprehensive defaults based on block category
                let (hardness, resistance, is_opaque, light, slip) = match self {
                    // --- Light sources ---
                    Block::Torch => (0.0, 0.0, false, 14, 0.6),
                    Block::Fire => (0.0, 0.0, false, 15, 0.6),
                    Block::RedstoneTorch | Block::UnlitRedstoneTorch => (0.0, 0.0, false, 7, 0.6),
                    Block::LitFurnace => (3.5, 3.5, true, 13, 0.6),
                    Block::JackOLantern => (1.0, 1.0, true, 15, 0.6),
                    Block::LitRedstoneOre => (3.0, 3.0, true, 9, 0.6),
                    Block::Beacon => (3.0, 3.0, false, 15, 0.6),
                    Block::NetherPortal => (0.0, 0.0, false, 11, 0.6),
                    // BlockEndPortal and bedrock have block hardness -1 in 1.8.9.
                    Block::EndPortal => (-1.0, 6000000.0, false, 15, 0.6),
                    // BlockEndPortalFrame is also unbreakable in 1.8.9.
                    Block::EndPortalFrame => (-1.0, 6000000.0, false, 1, 0.6),
                    Block::BrewingStand => (0.5, 0.5, false, 1, 0.6),
                    Block::BrownMushroom => (0.0, 0.0, false, 1, 0.6),
                    Block::RedstoneLamp | Block::LitRedstoneLamp => (0.3, 0.3, true, 15, 0.6),

                    // --- Liquids (already handled above, but just in case) ---
                    Block::FlowingWater | Block::StillWater => (100.0, 500.0, false, 0, 0.6),
                    Block::FlowingLava | Block::StillLava => (100.0, 500.0, false, 15, 0.6),

                    // --- Leaves (NOT opaque — fix for sky light) ---
                    Block::Leaves | Block::Leaves2 | Block::Leaves3 => (0.2, 0.2, false, 0, 0.6),

                    // --- Glass-like (NOT opaque) ---
                    Block::GlassPane | Block::StainedGlassPane => (0.3, 0.3, false, 0, 0.6),
                    Block::PackedIce => (0.5, 0.5, true, 0, 0.98),
                    Block::StainedGlass => (0.3, 0.3, false, 0, 0.6),
                    Block::Prismarine => (1.5, 6.0, true, 0, 0.6),
                    Block::SeaLantern => (0.3, 0.3, true, 15, 0.6),
                    // BlockSlime constructor: slipperiness = 0.8F.
                    Block::SlimeBlock => (0.0, 0.0, false, 0, 0.8),
                    Block::Barrier => (-1.0, 18000000.0, false, 0, 0.6),

                    // --- Plants (no hardness, not opaque) ---
                    Block::Sapling => (0.0, 0.0, false, 0, 0.6),
                    Block::Dandelion | Block::Flower => (0.0, 0.0, false, 0, 0.6),
                    Block::RedMushroom => (0.0, 0.0, false, 0, 0.6),
                    Block::TallGrass | Block::DeadBush => (0.0, 0.0, false, 0, 0.6),
                    Block::Wheat | Block::Carrots | Block::Potatoes => (0.0, 0.0, false, 0, 0.6),
                    Block::NetherWart => (0.0, 0.0, false, 0, 0.6),
                    Block::PumpkinStem | Block::MelonStem => (0.0, 0.0, false, 0, 0.6),
                    Block::LargeFlower => (0.0, 0.0, false, 0, 0.6),
                    Block::LilyPad => (0.0, 0.0, false, 0, 0.6),
                    Block::Vine => (0.2, 0.2, false, 0, 0.6),
                    Block::Cobweb => (4.0, 4.0, false, 0, 0.6),

                    // --- Rails (flat, not opaque) ---
                    Block::Rail
                    | Block::PoweredRail
                    | Block::DetectorRail
                    | Block::ActivatorRail => (0.7, 0.7, false, 0, 0.6),

                    // --- Signs (not opaque) ---
                    Block::StandingSign | Block::WallSign => (1.0, 1.0, false, 0, 0.6),
                    Block::StandingBanner | Block::WallBanner => (1.0, 1.0, false, 0, 0.6),

                    // --- Redstone (not opaque) ---
                    Block::RedstoneWire => (0.0, 0.0, false, 0, 0.6),
                    Block::Lever => (0.5, 0.5, false, 0, 0.6),
                    Block::StoneButton | Block::WoodenButton => (0.5, 0.5, false, 0, 0.6),
                    Block::StonePressurePlate | Block::WoodenPressurePlate => {
                        (0.5, 0.5, false, 0, 0.6)
                    }
                    Block::LightWeightedPressurePlate | Block::HeavyWeightedPressurePlate => {
                        (0.5, 0.5, false, 0, 0.6)
                    }
                    Block::UnpoweredRepeater | Block::PoweredRepeater => (0.0, 0.0, false, 0, 0.6),
                    Block::UnpoweredComparator | Block::PoweredComparator => {
                        (0.0, 0.0, false, 0, 0.6)
                    }
                    Block::DaylightDetector | Block::DaylightDetectorInverted => {
                        (0.2, 0.2, false, 0, 0.6)
                    }
                    Block::Tripwire | Block::TripwireHook => (0.0, 0.0, false, 0, 0.6),

                    // --- Non-full blocks (not opaque, same hardness as base material) ---
                    Block::StoneSlab | Block::StoneSlab2 => (2.0, 10.0, false, 0, 0.6),
                    Block::DoubleStoneSlab | Block::DoubleStoneSlab2 => (2.0, 10.0, true, 0, 0.6),
                    Block::WoodSlab => (2.0, 5.0, false, 0, 0.6),
                    Block::DoubleWoodSlab => (2.0, 5.0, true, 0, 0.6),
                    Block::OakStairs
                    | Block::SpruceStairs
                    | Block::BirchStairs
                    | Block::JungleStairs
                    | Block::AcaciaStairs
                    | Block::DarkOakStairs => (2.0, 5.0, false, 0, 0.6),
                    Block::CobblestoneStairs | Block::StoneBrickStairs => {
                        (2.0, 10.0, false, 0, 0.6)
                    }
                    Block::BrickStairs => (2.0, 6.0, false, 0, 0.6),
                    Block::NetherBrickStairs => (2.0, 10.0, false, 0, 0.6),
                    Block::SandstoneStairs => (0.8, 4.0, false, 0, 0.6),
                    Block::RedSandstoneStairs => (0.8, 4.0, false, 0, 0.6),
                    Block::QuartzStairs => (0.8, 4.0, false, 0, 0.6),
                    Block::OakFence
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
                    | Block::NetherBrickFence => (2.0, 5.0, false, 0, 0.6),
                    Block::CobblestoneWall => (2.0, 10.0, false, 0, 0.6),
                    Block::IronBars => (5.0, 6.0, false, 0, 0.6),
                    Block::OakDoor
                    | Block::SpruceDoor
                    | Block::BirchDoor
                    | Block::JungleDoor
                    | Block::AcaciaDoor
                    | Block::DarkOakDoor
                    | Block::IronDoor => (1.0, 1.0, false, 0, 0.6),
                    Block::Trapdoor | Block::IronTrapdoor => (3.0, 3.0, false, 0, 0.6),
                    Block::Ladder => (0.4, 0.4, false, 0, 0.6),
                    Block::SnowLayer => (0.1, 0.1, false, 0, 0.6),
                    Block::Carpet => (0.1, 0.1, false, 0, 0.6),
                    Block::Bed => (0.2, 0.2, false, 0, 0.6),
                    Block::Cactus => (0.4, 0.4, false, 0, 0.6),
                    Block::SugarCane => (0.0, 0.0, false, 0, 0.6),
                    Block::Cake => (0.5, 0.5, false, 0, 0.6),
                    Block::Farmland => (0.6, 0.6, false, 0, 0.6),
                    Block::PistonHead | Block::PistonExtension => (0.5, 0.5, false, 0, 0.6),
                    Block::Hopper => (3.0, 3.0, false, 0, 0.6),
                    Block::Cauldron => (2.0, 2.0, false, 0, 0.6),
                    Block::EnchantingTable => (5.0, 5.0, false, 0, 0.6),
                    Block::FlowerPot => (0.0, 0.0, false, 0, 0.6),
                    Block::Skull => (1.0, 1.0, false, 0, 0.6),
                    Block::Anvil => (5.0, 120.0, false, 0, 0.6),
                    Block::DragonEgg => (3.0, 9.0, false, 0, 0.6),
                    Block::Cocoa => (0.2, 0.2, false, 0, 0.6),

                    // --- Stone-variant blocks (opaque, hardness 1.5-3.0) ---
                    Block::StoneGranite | Block::StoneDiorite | Block::StoneAndesite => {
                        (1.5, 10.0, true, 0, 0.6)
                    }
                    Block::Cobblestone | Block::MossyCobblestone => (2.0, 10.0, true, 0, 0.6),
                    Block::StoneBricks | Block::BrownMushroomBlock | Block::RedMushroomBlock => {
                        (1.5, 10.0, true, 0, 0.6)
                    }
                    Block::Bricks => (2.0, 6.0, true, 0, 0.6),
                    Block::NetherBrick => (2.0, 10.0, true, 0, 0.6),
                    Block::EndStone => (3.0, 9.0, true, 0, 0.6),
                    Block::QuartzBlock => (0.8, 4.0, true, 0, 0.6),
                    Block::QuartzOre => (3.0, 3.0, true, 0, 0.6),
                    Block::Sandstone => (0.8, 4.0, true, 0, 0.6),
                    Block::RedSandstone => (0.8, 4.0, true, 0, 0.6),
                    Block::HardenedClay | Block::StainedClay => (1.25, 4.2, true, 0, 0.6),
                    Block::Clay => (0.6, 0.6, true, 0, 0.6),
                    Block::MonsterEgg => (0.75, 0.75, true, 0, 0.6),
                    Block::Sponge => (0.6, 0.6, true, 0, 0.6),
                    Block::HayBlock => (0.5, 0.5, true, 0, 0.6),

                    // --- Wood blocks ---
                    Block::Log2 | Block::Log3 => (2.0, 2.0, true, 0, 0.6),
                    Block::Bookshelf => (1.5, 1.5, true, 0, 0.6),
                    Block::CraftingTable => (2.5, 2.5, true, 0, 0.6),
                    Block::Chest | Block::TrappedChest | Block::EnderChest => {
                        (2.5, 2.5, false, 0, 0.6)
                    }
                    Block::Furnace => (3.5, 3.5, true, 0, 0.6),
                    Block::Dispenser | Block::Dropper => (3.5, 3.5, true, 0, 0.6),
                    Block::NoteBlock => (0.8, 0.8, true, 0, 0.6),
                    Block::Jukebox => (2.0, 6.0, true, 0, 0.6),
                    Block::Piston | Block::StickyPiston => (0.5, 0.5, false, 0, 0.6),
                    Block::CommandBlock => (22.5, 6000000.0, true, 0, 0.6),

                    // --- Metal blocks ---
                    Block::GoldBlock => (3.0, 30.0, true, 0, 0.6),
                    Block::IronBlock => (5.0, 10.0, true, 0, 0.6),
                    Block::EmeraldBlock => (5.0, 6.0, true, 0, 0.6),
                    Block::RedstoneBlock => (5.0, 10.0, true, 0, 0.6),
                    Block::LapisBlock => (3.0, 3.0, true, 0, 0.6),

                    // --- Ores ---
                    Block::LapisOre => (3.0, 3.0, true, 0, 0.6),
                    Block::RedstoneOre => (3.0, 3.0, true, 0, 0.6),

                    // --- Sand/gravel ---
                    Block::Sand => (0.5, 0.5, true, 0, 0.6),
                    Block::Gravel => (0.6, 0.6, true, 0, 0.6),

                    // --- Snow/ice ---
                    Block::SnowBlock => (0.2, 0.2, true, 0, 0.6),

                    // --- Plants ---
                    // --- Misc ---
                    Block::Tnt => (0.0, 0.0, true, 0, 0.6),
                    Block::Pumpkin => (1.0, 1.0, true, 0, 0.6),
                    Block::MelonBlock => (1.0, 1.0, true, 0, 0.6),
                    Block::Mycelium => (0.6, 0.6, true, 0, 0.6),
                    Block::GrassSnowy => (0.6, 0.6, true, 0, 0.6),
                    Block::SoulSand => (0.5, 0.5, true, 0, 0.6),
                    Block::MobSpawner => (5.0, 5.0, false, 0, 0.6),
                    // --- Default ---
                    _ => (1.0, 1.0, true, 0, 0.6),
                };
                BlockProperties {
                    id: self.to_id(),
                    name: "block",
                    hardness,
                    resistance,
                    is_opaque,
                    light_level: light,
                    slipperiness: slip,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Block;

    #[test]
    fn vanilla_unbreakable_blocks_have_negative_hardness() {
        for block in [
            Block::Bedrock,
            Block::EndPortal,
            Block::EndPortalFrame,
            Block::Barrier,
        ] {
            assert!(block.properties().hardness < 0.0, "{block:?}");
        }
    }
}
