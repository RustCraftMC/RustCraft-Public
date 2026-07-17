//! Entity type properties — mob stats, AI types, render data.

use super::EntityType;

/// Properties for an entity type.
pub struct EntityProperties {
    pub max_health: f32,
    pub movement_speed: f32,
    pub knockback_resistance: f32,
    pub attack_damage: f32,
    pub armor: f32,
    pub follow_range: f32,
    pub can_fly: bool,
    pub is_hostile: bool,
    /// Mob category for spawning rules.
    pub category: MobCategory,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobCategory {
    None,
    Monster,
    Creature,
    WaterCreature,
    Ambient,
}

impl EntityType {
    /// Mirrors `Entity.canBeCollidedWith`: base entities such as dropped items,
    /// XP orbs, and arrows are not eligible for the client's mouse-over ray.
    pub fn can_be_collided_with(self) -> bool {
        matches!(
            self,
            EntityType::Painting
                | EntityType::ItemFrame
                | EntityType::LeashKnot
                | EntityType::Fireball
                | EntityType::PrimedTnt
                | EntityType::FallingBlock
                | EntityType::ArmorStand
                | EntityType::Boat
                | EntityType::MinecartEmpty
                | EntityType::MinecartChest
                | EntityType::MinecartFurnace
                | EntityType::MinecartTNT
                | EntityType::MinecartHopper
                | EntityType::MinecartSpawner
                | EntityType::MinecartCommand
                | EntityType::Creeper
                | EntityType::Skeleton
                | EntityType::Spider
                | EntityType::Giant
                | EntityType::Zombie
                | EntityType::Slime
                | EntityType::Ghast
                | EntityType::PigZombie
                | EntityType::Enderman
                | EntityType::CaveSpider
                | EntityType::Silverfish
                | EntityType::Blaze
                | EntityType::LavaSlime
                | EntityType::EnderDragon
                | EntityType::WitherBoss
                | EntityType::Witch
                | EntityType::Endermite
                | EntityType::Guardian
                | EntityType::Pig
                | EntityType::Sheep
                | EntityType::Cow
                | EntityType::Chicken
                | EntityType::Squid
                | EntityType::Wolf
                | EntityType::Mooshroom
                | EntityType::SnowMan
                | EntityType::Ocelot
                | EntityType::IronGolem
                | EntityType::Horse
                | EntityType::Rabbit
                | EntityType::Villager
                | EntityType::Bat
                | EntityType::Player
        )
    }

    pub fn properties(self) -> EntityProperties {
        match self {
            // --- Hostile mobs ---
            EntityType::Creeper => EntityProperties {
                max_health: 20.0,
                movement_speed: 0.25,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Skeleton => EntityProperties {
                max_health: 20.0,
                movement_speed: 0.25,
                knockback_resistance: 0.0,
                attack_damage: 4.0,
                armor: 2.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Zombie => EntityProperties {
                max_health: 20.0,
                movement_speed: 0.23,
                knockback_resistance: 0.0,
                attack_damage: 3.0,
                armor: 2.0,
                follow_range: 40.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Spider => EntityProperties {
                max_health: 16.0,
                movement_speed: 0.3,
                knockback_resistance: 0.0,
                attack_damage: 2.0,
                armor: 2.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Enderman => EntityProperties {
                max_health: 40.0,
                movement_speed: 0.3,
                knockback_resistance: 0.0,
                attack_damage: 7.0,
                armor: 0.0,
                follow_range: 64.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Ghast => EntityProperties {
                max_health: 10.0,
                movement_speed: 0.1,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 64.0,
                can_fly: true,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Slime => EntityProperties {
                max_health: 16.0,
                movement_speed: 0.25,
                knockback_resistance: 0.0,
                attack_damage: 4.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Blaze => EntityProperties {
                max_health: 20.0,
                movement_speed: 0.23,
                knockback_resistance: 0.0,
                attack_damage: 6.0,
                armor: 2.0,
                follow_range: 48.0,
                can_fly: true,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Witch => EntityProperties {
                max_health: 26.0,
                movement_speed: 0.25,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Guardian => EntityProperties {
                max_health: 30.0,
                movement_speed: 0.5,
                knockback_resistance: 0.0,
                attack_damage: 6.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::EnderDragon => EntityProperties {
                max_health: 200.0,
                movement_speed: 0.3,
                knockback_resistance: 1.0,
                attack_damage: 6.0,
                armor: 0.0,
                follow_range: 80.0,
                can_fly: true,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::WitherBoss => EntityProperties {
                max_health: 300.0,
                movement_speed: 0.2,
                knockback_resistance: 1.0,
                attack_damage: 4.0,
                armor: 4.0,
                follow_range: 40.0,
                can_fly: true,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::CaveSpider => EntityProperties {
                max_health: 12.0,
                movement_speed: 0.3,
                knockback_resistance: 0.0,
                attack_damage: 2.0,
                armor: 2.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::Silverfish => EntityProperties {
                max_health: 8.0,
                movement_speed: 0.25,
                knockback_resistance: 0.0,
                attack_damage: 1.0,
                armor: 0.0,
                follow_range: 8.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::PigZombie => EntityProperties {
                max_health: 20.0,
                movement_speed: 0.23,
                knockback_resistance: 0.0,
                attack_damage: 5.0,
                armor: 2.0,
                follow_range: 40.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            EntityType::LavaSlime => EntityProperties {
                max_health: 16.0,
                movement_speed: 0.2,
                knockback_resistance: 0.0,
                attack_damage: 4.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },

            // --- Passive mobs ---
            EntityType::Pig => EntityProperties {
                max_health: 10.0,
                movement_speed: 0.25,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::Sheep => EntityProperties {
                max_health: 8.0,
                movement_speed: 0.23,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::Cow => EntityProperties {
                max_health: 10.0,
                movement_speed: 0.2,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::Chicken => EntityProperties {
                max_health: 4.0,
                movement_speed: 0.25,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::Wolf => EntityProperties {
                max_health: 8.0,
                movement_speed: 0.3,
                knockback_resistance: 0.0,
                attack_damage: 4.0,
                armor: 2.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::Horse => EntityProperties {
                max_health: 53.0,
                movement_speed: 0.2,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::Villager => EntityProperties {
                max_health: 20.0,
                movement_speed: 0.5,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::IronGolem => EntityProperties {
                max_health: 100.0,
                movement_speed: 0.25,
                knockback_resistance: 1.0,
                attack_damage: 7.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::ArmorStand => EntityProperties {
                max_health: 5.0,
                movement_speed: 0.0,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 0.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::None,
            },
            EntityType::Squid => EntityProperties {
                max_health: 10.0,
                movement_speed: 0.5,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::WaterCreature,
            },
            EntityType::Bat => EntityProperties {
                max_health: 6.0,
                movement_speed: 0.1,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: true,
                is_hostile: false,
                category: MobCategory::Ambient,
            },
            EntityType::Ocelot => EntityProperties {
                max_health: 10.0,
                movement_speed: 0.3,
                knockback_resistance: 0.0,
                attack_damage: 3.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::Mooshroom => EntityProperties {
                max_health: 10.0,
                movement_speed: 0.2,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::SnowMan => EntityProperties {
                max_health: 4.0,
                movement_speed: 0.2,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },
            EntityType::Rabbit => EntityProperties {
                max_health: 10.0,
                movement_speed: 0.3,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::Creature,
            },

            // --- Default ---
            _ => EntityProperties {
                max_health: 20.0,
                movement_speed: 0.25,
                knockback_resistance: 0.0,
                attack_damage: 0.0,
                armor: 0.0,
                follow_range: 16.0,
                can_fly: false,
                is_hostile: false,
                category: MobCategory::None,
            },
        }
    }

    /// Display name for the entity type.
    pub fn display_name(self) -> &'static str {
        match self {
            EntityType::Creeper => "Creeper",
            EntityType::Skeleton => "Skeleton",
            EntityType::Zombie => "Zombie",
            EntityType::Spider => "Spider",
            EntityType::Enderman => "Enderman",
            EntityType::Ghast => "Ghast",
            EntityType::Slime => "Slime",
            EntityType::Blaze => "Blaze",
            EntityType::Witch => "Witch",
            EntityType::Pig => "Pig",
            EntityType::Sheep => "Sheep",
            EntityType::Cow => "Cow",
            EntityType::Chicken => "Chicken",
            EntityType::Wolf => "Wolf",
            EntityType::Horse => "Horse",
            EntityType::Villager => "Villager",
            EntityType::IronGolem => "Iron Golem",
            EntityType::ArmorStand => "Armor Stand",
            EntityType::Squid => "Squid",
            EntityType::Bat => "Bat",
            EntityType::EnderDragon => "Ender Dragon",
            EntityType::WitherBoss => "Wither",
            EntityType::Player => "Player",
            EntityType::Item => "Item",
            EntityType::XPOrb => "XP Orb",
            EntityType::Arrow => "Arrow",
            EntityType::Snowball => "Snowball",
            EntityType::ThrownEgg => "Egg",
            EntityType::EnderPearl => "Ender Pearl",
            EntityType::Fireball => "Fireball",
            EntityType::SmallFireball => "Small Fireball",
            EntityType::WitherSkull => "Wither Skull",
            EntityType::ThrownPotion => "Potion",
            EntityType::ThrownExpBottle => "XP Bottle",
            EntityType::PrimedTnt => "TNT",
            EntityType::FallingBlock => "Falling Block",
            EntityType::Boat => "Boat",
            EntityType::MinecartEmpty => "Minecart",
            EntityType::MinecartChest => "Minecart with Chest",
            EntityType::MinecartFurnace => "Minecart with Furnace",
            EntityType::MinecartTNT => "Minecart with TNT",
            EntityType::MinecartHopper => "Minecart with Hopper",
            EntityType::MinecartSpawner => "Minecart with Spawner",
            EntityType::MinecartCommand => "Command Block Minecart",
            EntityType::Giant => "Giant",
            EntityType::PigZombie => "Zombie Pigman",
            EntityType::CaveSpider => "Cave Spider",
            EntityType::Silverfish => "Silverfish",
            EntityType::LavaSlime => "Magma Cube",
            EntityType::Endermite => "Endermite",
            EntityType::Guardian => "Guardian",
            EntityType::Mooshroom => "Mooshroom",
            EntityType::SnowMan => "Snow Golem",
            EntityType::Ocelot => "Ocelot",
            EntityType::Rabbit => "Rabbit",
            EntityType::Painting => "Painting",
            EntityType::LeashKnot => "Leash Knot",
            EntityType::ItemFrame => "Item Frame",
            EntityType::FireworkRocket => "Firework Rocket",
            _ => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EntityType;

    #[test]
    fn base_entities_are_not_mouse_over_targets() {
        assert!(!EntityType::Item.can_be_collided_with());
        assert!(!EntityType::XPOrb.can_be_collided_with());
        assert!(!EntityType::Arrow.can_be_collided_with());
        assert!(EntityType::Zombie.can_be_collided_with());
        assert!(EntityType::ItemFrame.can_be_collided_with());
    }
}
