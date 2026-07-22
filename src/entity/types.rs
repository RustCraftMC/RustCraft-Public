//! Entity type properties — mob stats, AI types, render data.
//!
//! The `EntityTypeRegistry` provides ID-based lookup for entity properties,
//! supporting both the built-in MC 1.8.9 types and custom mod-registered types.
//! The `EntityType` enum remains the primary way to refer to known types, while
//! the registry enables extension without modifying the enum.

use super::EntityType;
use std::collections::HashMap;

/// Properties for an entity type.
#[derive(Clone, Debug)]
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

/// Full type info stored in the registry for each entity type.
#[derive(Clone, Debug)]
pub struct EntityTypeInfo {
    /// Protocol entity ID (MC 1.8.9).
    pub id: i32,
    /// Human-readable name.
    pub name: String,
    /// Combat/movement properties.
    pub properties: EntityProperties,
    /// Bounding box (width, height) in blocks.
    pub bounding_box: (f32, f32),
    /// Whether this is a mob type (spawned via S0F Spawn Mob).
    pub is_mob: bool,
    /// Whether this is a passive mob.
    pub is_passive: bool,
    /// Whether this entity can be targeted by the crosshair raycast.
    pub can_be_collided_with: bool,
}

/// Registry for entity type data. Supports both built-in MC 1.8.9 types
/// (pre-registered at construction) and custom types added by mods.
pub struct EntityTypeRegistry {
    by_id: HashMap<i32, EntityTypeInfo>,
    by_name: HashMap<String, i32>,
}

impl EntityTypeRegistry {
    /// Create a new registry with all built-in MC 1.8.9 entity types pre-registered.
    pub fn new() -> Self {
        let mut registry = EntityTypeRegistry {
            by_id: HashMap::new(),
            by_name: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    /// Register a custom entity type. Returns false if the ID is already taken.
    pub fn register_custom(&mut self, info: EntityTypeInfo) -> bool {
        if self.by_id.contains_key(&info.id) {
            return false;
        }
        self.by_name.insert(info.name.clone(), info.id);
        self.by_id.insert(info.id, info);
        true
    }

    /// Look up entity type info by protocol ID.
    pub fn get_by_id(&self, id: i32) -> Option<&EntityTypeInfo> {
        self.by_id.get(&id)
    }

    /// Look up entity type info by name.
    pub fn get_by_name(&self, name: &str) -> Option<&EntityTypeInfo> {
        self.by_name.get(name).and_then(|id| self.by_id.get(id))
    }

    /// Number of registered entity types.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    fn register_builtins(&mut self) {
        let builtins: &[(i32, &str, EntityType)] = &[
            (1, "Item", EntityType::Item),
            (2, "XPOrb", EntityType::XPOrb),
            (7, "ThrownEgg", EntityType::ThrownEgg),
            (8, "LeashKnot", EntityType::LeashKnot),
            (9, "Painting", EntityType::Painting),
            (10, "Arrow", EntityType::Arrow),
            (11, "Snowball", EntityType::Snowball),
            (12, "Fireball", EntityType::Fireball),
            (13, "SmallFireball", EntityType::SmallFireball),
            (14, "EnderPearl", EntityType::EnderPearl),
            (15, "EnderEye", EntityType::EnderEye),
            (16, "ThrownPotion", EntityType::ThrownPotion),
            (17, "ThrownExpBottle", EntityType::ThrownExpBottle),
            (18, "ItemFrame", EntityType::ItemFrame),
            (19, "WitherSkull", EntityType::WitherSkull),
            (20, "PrimedTnt", EntityType::PrimedTnt),
            (21, "FallingBlock", EntityType::FallingBlock),
            (22, "FireworkRocket", EntityType::FireworkRocket),
            (30, "ArmorStand", EntityType::ArmorStand),
            (40, "MinecartCommand", EntityType::MinecartCommand),
            (41, "Boat", EntityType::Boat),
            (42, "MinecartEmpty", EntityType::MinecartEmpty),
            (43, "MinecartChest", EntityType::MinecartChest),
            (44, "MinecartFurnace", EntityType::MinecartFurnace),
            (45, "MinecartTNT", EntityType::MinecartTNT),
            (46, "MinecartHopper", EntityType::MinecartHopper),
            (47, "MinecartSpawner", EntityType::MinecartSpawner),
            (50, "Creeper", EntityType::Creeper),
            (51, "Skeleton", EntityType::Skeleton),
            (52, "Spider", EntityType::Spider),
            (53, "Giant", EntityType::Giant),
            (54, "Zombie", EntityType::Zombie),
            (55, "Slime", EntityType::Slime),
            (56, "Ghast", EntityType::Ghast),
            (57, "PigZombie", EntityType::PigZombie),
            (58, "Enderman", EntityType::Enderman),
            (59, "CaveSpider", EntityType::CaveSpider),
            (60, "Silverfish", EntityType::Silverfish),
            (61, "Blaze", EntityType::Blaze),
            (62, "LavaSlime", EntityType::LavaSlime),
            (63, "EnderDragon", EntityType::EnderDragon),
            (64, "WitherBoss", EntityType::WitherBoss),
            (65, "Bat", EntityType::Bat),
            (66, "Witch", EntityType::Witch),
            (67, "Endermite", EntityType::Endermite),
            (68, "Guardian", EntityType::Guardian),
            (90, "Pig", EntityType::Pig),
            (91, "Sheep", EntityType::Sheep),
            (92, "Cow", EntityType::Cow),
            (93, "Chicken", EntityType::Chicken),
            (94, "Squid", EntityType::Squid),
            (95, "Wolf", EntityType::Wolf),
            (96, "Mooshroom", EntityType::Mooshroom),
            (97, "SnowMan", EntityType::SnowMan),
            (98, "Ocelot", EntityType::Ocelot),
            (99, "IronGolem", EntityType::IronGolem),
            (100, "Horse", EntityType::Horse),
            (101, "Rabbit", EntityType::Rabbit),
            (120, "Villager", EntityType::Villager),
        ];

        for &(id, name, entity_type) in builtins {
            let info = EntityTypeInfo {
                id,
                name: name.to_string(),
                properties: entity_type.properties(),
                bounding_box: entity_type.bounding_box(),
                is_mob: entity_type.is_mob(),
                is_passive: entity_type.is_passive(),
                can_be_collided_with: entity_type.can_be_collided_with(),
            };
            self.by_name.insert(name.to_string(), id);
            self.by_id.insert(id, info);
        }
    }
}

impl Default for EntityTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
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

    #[test]
    fn registry_contains_all_builtin_types() {
        use super::EntityTypeRegistry;
        let registry = EntityTypeRegistry::new();
        assert!(registry.get_by_id(54).is_some()); // Zombie
        assert!(registry.get_by_id(50).is_some()); // Creeper
        assert!(registry.get_by_id(90).is_some()); // Pig
        assert!(registry.get_by_id(1).is_some()); // Item
    }

    #[test]
    fn registry_lookup_by_name() {
        use super::EntityTypeRegistry;
        let registry = EntityTypeRegistry::new();
        let info = registry.get_by_name("Zombie").unwrap();
        assert_eq!(info.id, 54);
        assert_eq!(info.properties.max_health, 20.0);
    }

    #[test]
    fn registry_rejects_duplicate_id() {
        use super::{EntityTypeInfo, EntityTypeRegistry, MobCategory};
        let mut registry = EntityTypeRegistry::new();
        let custom = EntityTypeInfo {
            id: 54, // Zombie already registered
            name: "CustomZombie".to_string(),
            properties: super::EntityProperties {
                max_health: 100.0,
                movement_speed: 0.5,
                knockback_resistance: 0.0,
                attack_damage: 10.0,
                armor: 5.0,
                follow_range: 32.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            bounding_box: (0.6, 1.95),
            is_mob: true,
            is_passive: false,
            can_be_collided_with: true,
        };
        assert!(!registry.register_custom(custom));
    }

    #[test]
    fn registry_accepts_new_id() {
        use super::{EntityTypeInfo, EntityTypeRegistry, MobCategory};
        let mut registry = EntityTypeRegistry::new();
        let custom = EntityTypeInfo {
            id: 1000,
            name: "CustomMob".to_string(),
            properties: super::EntityProperties {
                max_health: 50.0,
                movement_speed: 0.3,
                knockback_resistance: 0.0,
                attack_damage: 5.0,
                armor: 2.0,
                follow_range: 20.0,
                can_fly: false,
                is_hostile: true,
                category: MobCategory::Monster,
            },
            bounding_box: (0.6, 1.8),
            is_mob: true,
            is_passive: false,
            can_be_collided_with: true,
        };
        assert!(registry.register_custom(custom));
        assert_eq!(registry.get_by_id(1000).unwrap().properties.max_health, 50.0);
    }
}
