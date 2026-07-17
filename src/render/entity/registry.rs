use crate::entity::{EntityType, EntityVisualState};

pub fn model_for_entity(
    entity_type: EntityType,
    visual: EntityVisualState,
    slim: bool,
    skin_parts_mask: u8,
    has_cape: bool,
) -> Vec<super::mesh::ModelCuboid> {
    match entity_type {
        EntityType::Zombie | EntityType::Giant => {
            if visual.zombie_villager {
                super::models::zombie_villager_model()
            } else {
                super::models::zombie_model()
            }
        }
        EntityType::PigZombie => super::models::zombie_model(),
        EntityType::Witch => super::models::witch_model(),
        EntityType::Villager => super::models::villager_model(),
        EntityType::Skeleton => super::models::skeleton_model(),
        EntityType::Creeper => super::models::creeper_model(),
        EntityType::Pig => super::models::pig_model(),
        EntityType::Sheep => super::models::sheep_model(),
        EntityType::Cow | EntityType::Mooshroom => super::models::cow_model(),
        EntityType::Spider | EntityType::CaveSpider => super::models::spider_model(),
        EntityType::Enderman => super::models::enderman_model(),
        EntityType::Slime | EntityType::LavaSlime => super::models::slime_model(),
        EntityType::Chicken => super::models::chicken_model(),
        EntityType::Squid => super::models::squid_model(),
        EntityType::Wolf => super::models::wolf_model(),
        EntityType::Ocelot => super::models::ocelot_model(),
        EntityType::Horse => super::models::horse_model(),
        EntityType::Rabbit => super::models::rabbit_model(),
        EntityType::Bat => super::models::bat_model(visual),
        EntityType::SnowMan => super::models::snowman_model(),
        EntityType::Ghast => super::models::ghast_model(),
        EntityType::WitherBoss => super::models::wither_model(),
        EntityType::EnderDragon => super::models::ender_dragon_model(),
        EntityType::Blaze => super::models::blaze_model(),
        EntityType::Silverfish => super::models::silverfish_model(),
        EntityType::Endermite => super::models::endermite_model(),
        EntityType::Guardian => super::models::guardian_model(),
        EntityType::IronGolem => super::models::iron_golem_model(),
        EntityType::ArmorStand => super::models::armor_stand_model(visual.armor_stand_flags),
        EntityType::Player => super::models::player_model(slim, skin_parts_mask, has_cape),
        // Item/Projectile/Falling block entities
        EntityType::Item => super::models::item_model(),
        EntityType::XPOrb => super::models::xp_orb_model(),
        EntityType::Arrow | EntityType::WitherSkull => super::models::arrow_model(),
        EntityType::Snowball
        | EntityType::ThrownEgg
        | EntityType::EnderPearl
        | EntityType::ThrownPotion
        | EntityType::ThrownExpBottle
        | EntityType::EnderEye
        | EntityType::FireworkRocket
        | EntityType::LeashKnot => super::models::projectile_model(),
        EntityType::Fireball | EntityType::SmallFireball => super::models::projectile_model(),
        EntityType::PrimedTnt
        | EntityType::FallingBlock
        | EntityType::Painting
        | EntityType::ItemFrame => super::models::falling_block_model(),
        EntityType::Boat => super::models::boat_model(),
        EntityType::MinecartEmpty
        | EntityType::MinecartChest
        | EntityType::MinecartFurnace
        | EntityType::MinecartTNT
        | EntityType::MinecartHopper
        | EntityType::MinecartSpawner
        | EntityType::MinecartCommand => super::models::minecart_model(),
        EntityType::LightningBolt | EntityType::Unknown => Vec::new(),
    }
}

pub fn atlas_name_for_entity(entity_type: EntityType, visual: EntityVisualState) -> &'static str {
    match entity_type {
        EntityType::Zombie | EntityType::Giant => {
            if visual.zombie_villager {
                "zombie_villager"
            } else {
                "zombie"
            }
        }
        EntityType::Skeleton => {
            if visual.skeleton_type == 1 {
                "wither_skeleton"
            } else {
                "skeleton"
            }
        }
        EntityType::PigZombie => "zombie_pigman",
        EntityType::Creeper => "creeper",
        EntityType::Witch => "witch",
        EntityType::Pig => "pig",
        EntityType::Sheep => "sheep",
        EntityType::Cow => "cow",
        EntityType::Mooshroom => "mooshroom",
        EntityType::Chicken => "chicken",
        EntityType::Squid => "squid",
        EntityType::Spider => "spider",
        EntityType::CaveSpider => "cave_spider",
        EntityType::Enderman => "enderman",
        EntityType::Slime => "slime",
        EntityType::LavaSlime => "magma_cube",
        EntityType::Ghast => "ghast",
        EntityType::Blaze => "blaze",
        EntityType::Silverfish => "silverfish",
        EntityType::Endermite => "endermite",
        EntityType::Guardian => {
            if visual.guardian_elder {
                "guardian_elder"
            } else {
                "guardian"
            }
        }
        EntityType::Villager => match visual.villager_profession {
            0 => "villager_farmer",
            1 => "villager_librarian",
            2 => "villager_priest",
            3 => "villager_smith",
            4 => "villager_butcher",
            _ => "villager",
        },
        EntityType::Wolf => {
            if visual.wolf_tamed {
                "wolf_tame"
            } else if visual.wolf_angry {
                "wolf_angry"
            } else {
                "wolf"
            }
        }
        EntityType::Ocelot => match visual.ocelot_skin {
            1 => "ocelot_black",
            2 => "ocelot_red",
            3 => "ocelot_siamese",
            _ => "ocelot",
        },
        EntityType::Horse => match visual.horse_type {
            1 => "horse_donkey",
            2 => "horse_mule",
            3 => "horse_zombie",
            4 => "horse_skeleton",
            _ => match visual.horse_variant & 255 {
                0 => "horse_white",
                1 => "horse_creamy",
                2 => "horse_chestnut",
                3 => "horse_brown",
                4 => "horse_black",
                5 => "horse_gray",
                6 => "horse_darkbrown",
                _ => "horse_white",
            },
        },
        EntityType::Rabbit => match visual.rabbit_type {
            0 => "rabbit_brown",
            1 => "rabbit_white",
            2 => "rabbit_black",
            3 => "rabbit_white_splotched",
            4 => "rabbit_gold",
            5 => "rabbit_salt",
            99 => "rabbit_caerbannog",
            _ => "rabbit_brown",
        },
        EntityType::Bat => "bat",
        EntityType::SnowMan => "snowman",
        EntityType::IronGolem => "iron_golem",
        EntityType::ArmorStand => "armor_stand",
        EntityType::EnderDragon => "ender_dragon",
        EntityType::WitherBoss => "wither",
        EntityType::Player => "player",
        // Item/projectile entities use a simple white texture
        EntityType::Item
        | EntityType::Arrow
        | EntityType::ThrownEgg
        | EntityType::Snowball
        | EntityType::Fireball
        | EntityType::SmallFireball
        | EntityType::EnderPearl
        | EntityType::ThrownPotion
        | EntityType::ThrownExpBottle
        | EntityType::EnderEye
        | EntityType::WitherSkull
        | EntityType::FireworkRocket
        | EntityType::LeashKnot
        | EntityType::PrimedTnt
        | EntityType::FallingBlock
        | EntityType::Boat
        | EntityType::MinecartEmpty
        | EntityType::MinecartChest
        | EntityType::MinecartFurnace
        | EntityType::MinecartTNT
        | EntityType::MinecartHopper
        | EntityType::MinecartSpawner
        | EntityType::MinecartCommand
        | EntityType::Painting
        | EntityType::ItemFrame
        | EntityType::LightningBolt => "__white",
        EntityType::XPOrb => "experience_orb",
        EntityType::Unknown => "__white",
    }
}
