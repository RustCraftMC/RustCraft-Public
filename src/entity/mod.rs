//! Entity system — MC 1.8.9 entity definitions and ECS-like storage.
//!
//! Entities are stored in a flat Vec with an ID-based lookup.
//! Each entity has position, velocity, rotation, bounding box, and type-specific data.

pub mod types;

use crate::net::metadata::{EntityMetadata, MetadataValue};
use crate::net::packet::EntityProperty;
use crate::net::slot::Slot;
use crate::util::wrap_degrees;
use nalgebra::{Point3, Vector3};

/// Unique entity ID (assigned by server).
pub type EntityId = i32;

/// Entity type enum matching MC 1.8.9 entity IDs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntityType {
    // Objects
    Item,            // 1
    XPOrb,           // 2
    Painting,        // 9
    Arrow,           // 10
    ThrownEgg,       // 7
    Snowball,        // 11
    Fireball,        // 12
    SmallFireball,   // 13
    EnderPearl,      // 14
    EnderEye,        // 15
    ThrownPotion,    // 16
    ThrownExpBottle, // 17
    ItemFrame,       // 18
    WitherSkull,     // 19
    PrimedTnt,       // 20
    FallingBlock,    // 21
    FireworkRocket,  // 22
    ArmorStand,      // 30
    LeashKnot,       // 8
    Boat,            // 41
    MinecartEmpty,   // 42
    MinecartChest,   // 43
    MinecartFurnace, // 44
    MinecartTNT,     // 45
    MinecartHopper,  // 46
    MinecartSpawner, // 47
    MinecartCommand, // 40

    // Hostile mobs
    Creeper,     // 50
    Skeleton,    // 51
    Spider,      // 52
    Giant,       // 53
    Zombie,      // 54
    Slime,       // 55
    Ghast,       // 56
    PigZombie,   // 57
    Enderman,    // 58
    CaveSpider,  // 59
    Silverfish,  // 60
    Blaze,       // 61
    LavaSlime,   // 62
    EnderDragon, // 63
    WitherBoss,  // 64
    Witch,       // 66
    Endermite,   // 67
    Guardian,    // 68

    // Passive mobs
    Pig,       // 90
    Sheep,     // 91
    Cow,       // 92
    Chicken,   // 93
    Squid,     // 94
    Wolf,      // 95
    Mooshroom, // 96
    SnowMan,   // 97
    Ocelot,    // 98
    IronGolem, // 99
    Horse,     // 100
    Rabbit,    // 101
    Villager,  // 120
    Bat,       // 65

    // Players
    Player, // not spawned via packets

    // Other
    LightningBolt, // not spawned via packets

    Unknown,
}

impl EntityType {
    /// Map the object-type byte from S0E Spawn Object. This is a different
    /// namespace from the EntityList IDs used by S0F Spawn Mob.
    pub fn from_object_id(id: i32, object_data: i32) -> Self {
        match id {
            1 => EntityType::Boat,
            2 => EntityType::Item,
            10 => match object_data {
                0 => EntityType::MinecartEmpty,
                1 => EntityType::MinecartChest,
                2 => EntityType::MinecartFurnace,
                3 => EntityType::MinecartTNT,
                4 => EntityType::MinecartSpawner,
                5 => EntityType::MinecartHopper,
                6 => EntityType::MinecartCommand,
                _ => EntityType::MinecartEmpty,
            },
            50 => EntityType::PrimedTnt,
            60 => EntityType::Arrow,
            61 => EntityType::Snowball,
            62 => EntityType::ThrownEgg,
            63 => EntityType::Fireball,
            64 => EntityType::SmallFireball,
            65 => EntityType::EnderPearl,
            66 => EntityType::WitherSkull,
            70 => EntityType::FallingBlock,
            71 => EntityType::ItemFrame,
            72 => EntityType::EnderEye,
            73 => EntityType::ThrownPotion,
            75 => EntityType::ThrownExpBottle,
            76 => EntityType::FireworkRocket,
            77 => EntityType::LeashKnot,
            78 => EntityType::ArmorStand,
            _ => EntityType::Unknown,
        }
    }

    pub fn from_id(id: i32) -> Self {
        match id {
            1 => EntityType::Item,
            2 => EntityType::XPOrb,
            7 => EntityType::ThrownEgg,
            8 => EntityType::LeashKnot,
            9 => EntityType::Painting,
            10 => EntityType::Arrow,
            11 => EntityType::Snowball,
            12 => EntityType::Fireball,
            13 => EntityType::SmallFireball,
            14 => EntityType::EnderPearl,
            15 => EntityType::EnderEye,
            16 => EntityType::ThrownPotion,
            17 => EntityType::ThrownExpBottle,
            18 => EntityType::ItemFrame,
            19 => EntityType::WitherSkull,
            20 => EntityType::PrimedTnt,
            21 => EntityType::FallingBlock,
            22 => EntityType::FireworkRocket,
            30 => EntityType::ArmorStand,
            40 => EntityType::MinecartCommand,
            41 => EntityType::Boat,
            42 => EntityType::MinecartEmpty,
            43 => EntityType::MinecartChest,
            44 => EntityType::MinecartFurnace,
            45 => EntityType::MinecartTNT,
            46 => EntityType::MinecartHopper,
            47 => EntityType::MinecartSpawner,
            48 => EntityType::Unknown, // Mob base
            49 => EntityType::Unknown, // Monster base
            50 => EntityType::Creeper,
            51 => EntityType::Skeleton,
            52 => EntityType::Spider,
            53 => EntityType::Giant,
            54 => EntityType::Zombie,
            55 => EntityType::Slime,
            56 => EntityType::Ghast,
            57 => EntityType::PigZombie,
            58 => EntityType::Enderman,
            59 => EntityType::CaveSpider,
            60 => EntityType::Silverfish,
            61 => EntityType::Blaze,
            62 => EntityType::LavaSlime,
            63 => EntityType::EnderDragon,
            64 => EntityType::WitherBoss,
            65 => EntityType::Bat,
            66 => EntityType::Witch,
            67 => EntityType::Endermite,
            68 => EntityType::Guardian,
            99 => EntityType::IronGolem,
            90 => EntityType::Pig,
            91 => EntityType::Sheep,
            92 => EntityType::Cow,
            93 => EntityType::Chicken,
            94 => EntityType::Squid,
            95 => EntityType::Wolf,
            96 => EntityType::Mooshroom,
            97 => EntityType::SnowMan,
            98 => EntityType::Ocelot,
            100 => EntityType::Horse,
            101 => EntityType::Rabbit,
            120 => EntityType::Villager,
            _ => EntityType::Unknown,
        }
    }

    pub fn is_mob(self) -> bool {
        matches!(
            self,
            EntityType::Creeper
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
                | EntityType::Horse
                | EntityType::Rabbit
                | EntityType::Bat
                | EntityType::Villager
                | EntityType::IronGolem
                | EntityType::ArmorStand
        )
    }

    pub fn is_passive(self) -> bool {
        matches!(
            self,
            EntityType::Pig
                | EntityType::Sheep
                | EntityType::Cow
                | EntityType::Chicken
                | EntityType::Squid
                | EntityType::Wolf
                | EntityType::Mooshroom
                | EntityType::SnowMan
                | EntityType::Ocelot
                | EntityType::Horse
                | EntityType::Rabbit
                | EntityType::Bat
                | EntityType::Villager
                | EntityType::IronGolem
        )
    }

    /// Bounding box size (width, height) in blocks.
    pub fn bounding_box(self) -> (f32, f32) {
        match self {
            EntityType::Creeper => (0.6, 1.8),
            EntityType::Skeleton => (0.6, 1.95),
            EntityType::Zombie => (0.6, 1.95),
            EntityType::Spider => (1.4, 0.9),
            EntityType::Enderman => (0.6, 2.9),
            EntityType::Pig => (0.9, 0.9),
            EntityType::Sheep => (0.9, 1.3),
            EntityType::Cow => (0.9, 1.4),
            EntityType::Chicken => (0.4, 0.3),
            EntityType::Bat => (0.5, 0.9),
            EntityType::Wolf => (0.6, 0.85),
            EntityType::Ocelot => (0.6, 0.7),
            EntityType::Rabbit => (0.4, 0.5),
            EntityType::Mooshroom => (0.9, 1.4),
            EntityType::SnowMan => (0.7, 1.9),
            EntityType::Squid => (0.8, 0.8),
            EntityType::Blaze => (0.6, 1.8),
            EntityType::Ghast => (4.0, 4.0),
            EntityType::WitherBoss => (0.9, 3.5),
            EntityType::EnderDragon => (16.0, 8.0),
            EntityType::Silverfish => (0.4, 0.3),
            EntityType::Endermite => (0.4, 0.3),
            EntityType::Guardian => (0.85, 0.85),
            EntityType::Villager => (0.6, 1.8),
            EntityType::IronGolem => (1.4, 2.7),
            EntityType::ArmorStand => (0.5, 1.975),
            EntityType::Horse => (1.4, 1.6),
            EntityType::Slime => (2.04, 2.04),
            EntityType::Boat => (1.5, 0.6),
            EntityType::MinecartEmpty
            | EntityType::MinecartChest
            | EntityType::MinecartFurnace
            | EntityType::MinecartTNT => (0.98, 0.7),
            EntityType::Arrow => (0.5, 0.5),
            EntityType::Snowball | EntityType::EnderPearl | EntityType::ThrownEgg => (0.25, 0.25),
            EntityType::PrimedTnt => (0.98, 0.98),
            EntityType::Item => (0.25, 0.25),
            EntityType::XPOrb => (0.5, 0.5),
            EntityType::Player => (0.6, 1.8),
            _ => (0.6, 1.8),
        }
    }
}

/// A single entity instance.
#[derive(Clone, Debug)]
pub struct Entity {
    pub entity_id: EntityId,
    pub entity_type: EntityType,
    pub uuid: Option<String>,
    pub position: Point3<f32>,
    pub prev_position: Point3<f32>,
    pub render_position: Point3<f32>,
    pub chasing_position: Point3<f32>,
    pub prev_chasing_position: Point3<f32>,
    pub render_chasing_position: Point3<f32>,
    target_position: Point3<f32>,
    target_yaw: f32,
    target_pitch: f32,
    lerp_steps: u8,
    /// EntityBoat.isBoatEmpty. NetHandlerPlayClient switches this when the
    /// local player mounts/dismounts, changing the vanilla interpolation path.
    boat_is_empty: bool,
    pub velocity: Vector3<f32>,
    pub yaw: f32,
    pub body_yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub skin_parts: u8,
    pub on_ground: bool,
    /// Entity flag 4 (`Entity.isEating`), also set while a player blocks.
    pub using_item: bool,
    pub ticks_alive: u32,
    pub hover_start: f32,
    pub current_item: Option<i16>,
    pub equipment: [Option<Slot>; 5],
    pub metadata: Vec<EntityMetadata>,
    pub last_status: Option<i8>,
    pub hurt_time: f32,
    pub death_time: f32,
    /// Suppresses another C02 attack on this target until the server confirms
    /// the preceding one. This closes the kill/despawn packet race.
    attack_pending_time: f32,
    pub swing_time: f32,
    pub critical_time: f32,
    /// Cumulative walk phase (radians-ish, advanced by distance traveled).
    pub limb_swing: f32,
    /// Walk speed (0..1), scales limb swing amplitude.
    pub limb_swing_amount: f32,
    pub distance_walked_modified: f32,
    pub prev_distance_walked_modified: f32,
    pub camera_yaw: f32,
    pub prev_camera_yaw: f32,
    pub vehicle_id: Option<EntityId>,
    pub leash_holder: Option<EntityId>,
    pub active_effects: Vec<EntityEffectState>,
    pub attributes: HashMap<String, EntityAttribute>,
    pub visual: EntityVisualState,
    // Entity-specific data
    pub data: EntityData,
}

#[derive(Clone, Debug)]
pub struct EntityEffectState {
    pub effect_id: i8,
    pub amplifier: i8,
    pub duration: i32,
    pub hide_particles: bool,
}

#[derive(Clone, Debug)]
pub struct EntityAttribute {
    pub key: String,
    pub base: f64,
    pub value: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct EntityVisualState {
    pub is_child: bool,
    pub zombie_villager: bool,
    pub zombie_converting: bool,
    pub skeleton_type: u8,
    pub wolf_tamed: bool,
    pub wolf_angry: bool,
    pub wolf_begging: bool,
    pub wolf_collar: u8,
    pub ocelot_skin: u8,
    pub horse_type: u8,
    pub horse_saddled: bool,
    pub horse_variant: u32,
    pub horse_armor: u8,
    pub guardian_elder: bool,
    pub slime_size: u8,
    pub bat_hanging: bool,
    pub villager_profession: u8,
    pub armor_stand_flags: u8,
    pub armor_stand_rotations: [[f32; 3]; 6],
    pub creeper_charged: bool,
    pub sheep_color: u8,
    pub pig_saddled: bool,
    pub rabbit_type: u8,
}

impl Default for EntityVisualState {
    fn default() -> Self {
        Self {
            is_child: false,
            zombie_villager: false,
            zombie_converting: false,
            skeleton_type: 0,
            wolf_tamed: false,
            wolf_angry: false,
            wolf_begging: false,
            wolf_collar: 0,
            ocelot_skin: 0,
            horse_type: 0,
            horse_saddled: false,
            horse_variant: 0,
            horse_armor: 0,
            guardian_elder: false,
            slime_size: 0,
            bat_hanging: false,
            villager_profession: 0,
            armor_stand_flags: 0,
            armor_stand_rotations: [
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [-10.0, 0.0, -10.0],
                [-15.0, 0.0, 10.0],
                [-1.0, 0.0, -1.0],
                [1.0, 0.0, 1.0],
            ],
            creeper_charged: false,
            sheep_color: 0,
            pig_saddled: false,
            rabbit_type: 0,
        }
    }
}

/// Type-specific entity data.
#[derive(Clone, Debug)]
pub enum EntityData {
    None,
    Item {
        item_id: u16,
        count: u8,
        damage: u16,
        nbt: Option<Vec<u8>>,
    },
    XPOrb {
        value: i32,
    },
    Mob {
        health: f32,
        max_health: f32,
    },
    Player {
        name: String,
        gamemode: u8,
        /// The signed GameProfile `textures` value captured for this entity.
        /// Player-info entries may be removed while the entity remains alive.
        skin_property: Option<String>,
    },
    Living {
        health: f32,
        max_health: f32,
        absorption: f32,
    },
}

impl Entity {
    pub fn new(entity_id: EntityId, entity_type: EntityType, position: Point3<f32>) -> Self {
        Entity {
            entity_id,
            entity_type,
            uuid: None,
            position,
            prev_position: position,
            render_position: position,
            chasing_position: position,
            prev_chasing_position: position,
            render_chasing_position: position,
            target_position: position,
            target_yaw: 0.0,
            target_pitch: 0.0,
            lerp_steps: 0,
            boat_is_empty: true,
            velocity: Vector3::zeros(),
            yaw: 0.0,
            body_yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            skin_parts: 0x7f,
            on_ground: false,
            using_item: false,
            ticks_alive: 0,
            hover_start: (rand_float(position.x * 13.37 + position.z * 7.31)
                * 2.0
                * std::f32::consts::PI),
            current_item: None,
            equipment: std::array::from_fn(|_| None),
            metadata: Vec::new(),
            last_status: None,
            hurt_time: 0.0,
            death_time: 0.0,
            attack_pending_time: 0.0,
            swing_time: 0.0,
            critical_time: 0.0,
            limb_swing: 0.0,
            limb_swing_amount: 0.0,
            distance_walked_modified: 0.0,
            prev_distance_walked_modified: 0.0,
            camera_yaw: 0.0,
            prev_camera_yaw: 0.0,
            vehicle_id: None,
            leash_holder: None,
            active_effects: Vec::new(),
            attributes: HashMap::new(),
            visual: EntityVisualState::default(),
            data: if entity_type.is_mob() {
                let max_health = entity_type.properties().max_health.max(1.0);
                EntityData::Mob {
                    health: max_health,
                    max_health,
                }
            } else {
                EntityData::None
            },
        }
    }

    pub fn set_equipment(&mut self, slot: i16, item: Slot) {
        if let Some(target) = self.equipment.get_mut(slot.max(0) as usize) {
            if slot == 0 {
                self.current_item = (!item.is_empty()).then_some(item.item_id);
            }
            *target = if item.is_empty() { None } else { Some(item) };
        }
    }

    pub fn move_relative(&mut self, dx: f32, dy: f32, dz: f32, lerp_steps: u8) -> bool {
        if self.entity_type == EntityType::Boat {
            let target = self.target_position + Vector3::new(dx, dy, dz);
            if !self.boat_is_empty && (target - self.position).norm_squared() <= 1.0 {
                return false;
            }
            self.target_position = target;
            self.lerp_steps = if self.boat_is_empty {
                lerp_steps.saturating_add(5)
            } else {
                3
            };
            return true;
        }
        if self.is_minecart() {
            self.target_position += Vector3::new(dx, dy, dz);
            self.lerp_steps = lerp_steps.saturating_add(2);
            return true;
        }
        if self.uses_packet_interpolation() {
            self.target_position.x += dx;
            self.target_position.y += dy;
            self.target_position.z += dz;
            self.lerp_steps = self.lerp_steps.max(lerp_steps);
        } else {
            self.position += Vector3::new(dx, dy, dz);
            self.target_position = self.position;
        }
        true
    }

    pub fn teleport(&mut self, position: Point3<f32>, lerp_steps: u8) -> bool {
        if self.entity_type == EntityType::Boat {
            if !self.boat_is_empty && (position - self.position).norm_squared() <= 1.0 {
                return false;
            }
            self.target_position = position;
            self.lerp_steps = if self.boat_is_empty {
                lerp_steps.saturating_add(5)
            } else {
                3
            };
            return true;
        }
        if self.is_minecart() {
            self.target_position = position;
            self.lerp_steps = lerp_steps.saturating_add(2);
            return true;
        }
        if self.uses_packet_interpolation() && lerp_steps != 0 {
            self.target_position = position;
            self.lerp_steps = lerp_steps;
        } else {
            self.position = position;
            self.prev_position = position;
            self.render_position = position;
            self.target_position = position;
            self.lerp_steps = 0;
        }
        true
    }

    /// Apply rotation from S14/S18. EntityLivingBase stores this as a target
    /// and consumes it over the packet's interpolation increment count; base
    /// entities apply it immediately. Boats and minecarts retain their
    /// dedicated interpolation path until it is modelled separately.
    pub fn set_remote_rotation(&mut self, yaw: f32, pitch: f32, lerp_steps: u8) {
        if self.entity_type == EntityType::Boat {
            self.target_yaw = yaw;
            self.target_pitch = pitch;
            self.lerp_steps = if self.boat_is_empty {
                lerp_steps.saturating_add(5)
            } else {
                3
            };
        } else if self.is_minecart() {
            self.target_yaw = yaw;
            self.target_pitch = pitch;
            self.lerp_steps = lerp_steps.saturating_add(2);
        } else if self.entity_type.is_mob() || self.entity_type == EntityType::Player {
            self.target_yaw = yaw;
            self.target_pitch = pitch;
            self.lerp_steps = self.lerp_steps.max(lerp_steps);
        } else {
            self.yaw = yaw;
            self.pitch = pitch;
            self.target_yaw = yaw;
            self.target_pitch = pitch;
        }
    }

    pub fn set_boat_empty(&mut self, empty: bool) {
        if self.entity_type == EntityType::Boat {
            self.boat_is_empty = empty;
        }
    }

    fn uses_packet_interpolation(&self) -> bool {
        self.entity_type.is_mob()
            || self.entity_type == EntityType::Player
            || matches!(
                self.entity_type,
                EntityType::Boat
                    | EntityType::MinecartEmpty
                    | EntityType::MinecartChest
                    | EntityType::MinecartFurnace
                    | EntityType::MinecartTNT
                    | EntityType::MinecartHopper
                    | EntityType::MinecartSpawner
                    | EntityType::MinecartCommand
            )
    }

    fn is_minecart(&self) -> bool {
        matches!(
            self.entity_type,
            EntityType::MinecartEmpty
                | EntityType::MinecartChest
                | EntityType::MinecartFurnace
                | EntityType::MinecartTNT
                | EntityType::MinecartHopper
                | EntityType::MinecartSpawner
                | EntityType::MinecartCommand
        )
    }

    pub fn apply_metadata(&mut self, metadata: Vec<EntityMetadata>) {
        for incoming in metadata {
            self.apply_metadata_value(&incoming);
            if let Some(existing) = self
                .metadata
                .iter_mut()
                .find(|entry| entry.index == incoming.index)
            {
                *existing = incoming;
            } else {
                self.metadata.push(incoming);
            }
        }
    }

    fn apply_metadata_value(&mut self, metadata: &EntityMetadata) {
        if metadata.index == 0 {
            if let MetadataValue::Byte(flags) = &metadata.value {
                self.using_item = (*flags as u8 & 0x10) != 0;
            }
        }
        if metadata.index == 12 {
            if let MetadataValue::Byte(value) = &metadata.value {
                self.visual.is_child = *value != 0;
            }
        }

        if self.entity_type == EntityType::Item && metadata.index == 10 {
            if let MetadataValue::Slot(slot) = &metadata.value {
                if !slot.is_empty() {
                    self.data = EntityData::Item {
                        item_id: slot.item_id_u16(),
                        count: slot.count,
                        damage: slot.damage.max(0) as u16,
                        nbt: if slot.nbt.as_ref().is_some_and(|n| !n.is_empty()) {
                            slot.nbt.clone()
                        } else {
                            None
                        },
                    };
                }
            }
        }

        match self.entity_type {
            EntityType::Zombie => match metadata.index {
                13 => {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.zombie_villager = *value != 0;
                    }
                }
                14 => {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.zombie_converting = *value != 0;
                    }
                }
                _ => {}
            },
            EntityType::Skeleton => {
                if metadata.index == 13 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.skeleton_type = (*value).max(0) as u8;
                    }
                }
            }
            EntityType::Wolf => match metadata.index {
                16 => {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        let flags = *value as u8;
                        self.visual.wolf_tamed = flags & 0x04 != 0;
                        self.visual.wolf_angry = flags & 0x02 != 0;
                    }
                }
                19 => {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.wolf_begging = *value != 0;
                    }
                }
                20 => {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.wolf_collar = (*value).max(0) as u8;
                    }
                }
                _ => {}
            },
            EntityType::Ocelot => {
                if metadata.index == 18 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.ocelot_skin = (*value).max(0) as u8;
                    }
                }
            }
            EntityType::Horse => match metadata.index {
                16 => {
                    if let MetadataValue::Int(value) = &metadata.value {
                        self.visual.horse_saddled = *value & 4 != 0;
                    }
                }
                19 => {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.horse_type = (*value).max(0) as u8;
                    }
                }
                20 => {
                    if let MetadataValue::Int(value) = &metadata.value {
                        self.visual.horse_variant = (*value).max(0) as u32;
                    }
                }
                22 => {
                    if let MetadataValue::Int(value) = &metadata.value {
                        self.visual.horse_armor = (*value).max(0) as u8;
                    }
                }
                _ => {}
            },
            EntityType::Guardian => {
                if metadata.index == 16 {
                    if let MetadataValue::Int(value) = &metadata.value {
                        self.visual.guardian_elder = *value & 4 != 0;
                    }
                }
            }
            EntityType::Slime | EntityType::LavaSlime => {
                if metadata.index == 16 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.slime_size = (*value).max(1) as u8;
                    }
                }
            }
            EntityType::Bat => {
                if metadata.index == 16 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.bat_hanging = *value & 1 != 0;
                    }
                }
            }
            EntityType::Villager => {
                if metadata.index == 16 {
                    if let MetadataValue::Int(value) = &metadata.value {
                        self.visual.villager_profession = (*value).max(0) as u8;
                    }
                }
            }
            EntityType::ArmorStand => {
                if metadata.index == 10 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.armor_stand_flags = *value as u8;
                    }
                } else if (11..=16).contains(&metadata.index) {
                    if let MetadataValue::Rotation(rotation) = &metadata.value {
                        self.visual.armor_stand_rotations[(metadata.index - 11) as usize] =
                            *rotation;
                    }
                }
            }
            EntityType::Creeper => {
                if metadata.index == 16 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.creeper_charged = *value & 1 != 0;
                    }
                }
            }
            EntityType::Sheep => {
                if metadata.index == 16 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.sheep_color = *value as u8 & 0x0f;
                    }
                }
            }
            EntityType::Pig => {
                if metadata.index == 16 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.pig_saddled = *value & 1 != 0;
                    }
                }
            }
            EntityType::Rabbit => {
                if metadata.index == 16 {
                    if let MetadataValue::Byte(value) = &metadata.value {
                        self.visual.rabbit_type = *value as u8;
                    }
                }
            }
            _ => {}
        }

        if metadata.index == 6 {
            if let MetadataValue::Float(health) = &metadata.value {
                match &mut self.data {
                    EntityData::Mob {
                        health: mob_health, ..
                    }
                    | EntityData::Living {
                        health: mob_health, ..
                    } => *mob_health = *health,
                    _ => {}
                }
                // Only a positive authoritative health update proves this
                // target survived the preceding attack. A kill can be preceded
                // by a hurt packet/metadata update, so zero must keep the
                // pending gate closed until destruction arrives.
                if *health > 0.0 {
                    self.attack_pending_time = 0.0;
                }
            }
        }

        if self.entity_type == EntityType::Player && metadata.index == 10 {
            if let MetadataValue::Byte(parts) = &metadata.value {
                self.skin_parts = *parts as u8 & 0x7f;
            }
        }
    }

    pub fn set_attachment(&mut self, vehicle_id: i32, leash: bool) {
        if leash {
            self.leash_holder = (vehicle_id >= 0).then_some(vehicle_id);
        } else {
            self.vehicle_id = (vehicle_id >= 0).then_some(vehicle_id);
        }
    }

    pub fn apply_status(&mut self, status: i8) {
        self.last_status = Some(status);
        match status {
            // A server can send hurt before its following death/despawn
            // update. Do not use it as an attack acknowledgement or the next
            // click can race the server's entity removal.
            2 => self.hurt_time = 0.45,
            // EntityLivingBase.onDeathUpdate keeps a dead living entity for
            // exactly 20 game ticks. Keep the same one-second visual window;
            // a non-zero value also makes the entity ineligible for attacks.
            3 => self.death_time = 1.0,
            _ => {}
        }
    }

    pub fn mark_attack_pending(&mut self) {
        // One-tick local guard: prevents the hurt animation from flickering if
        // a C02 packet is lost and the server never emits EntityStatus(2).
        // Attack validation (reach, invulnerability frames, CPS) is entirely
        // the server's responsibility — vanilla has NO client-held cooldown.
        self.attack_pending_time = 0.05;
    }

    pub fn attack_pending(&self) -> bool {
        self.attack_pending_time > 0.0
    }

    pub fn apply_animation(&mut self, animation: u8) {
        match animation {
            0 => self.swing_time = 0.35,
            4 | 5 => self.critical_time = 0.45,
            _ => {}
        }
    }

    pub fn add_effect(&mut self, effect: EntityEffectState) {
        if let Some(existing) = self
            .active_effects
            .iter_mut()
            .find(|active| active.effect_id == effect.effect_id)
        {
            *existing = effect;
        } else {
            self.active_effects.push(effect);
        }
    }

    pub fn remove_effect(&mut self, effect_id: i8) {
        self.active_effects
            .retain(|active| active.effect_id != effect_id);
    }

    pub fn apply_properties(&mut self, properties: Vec<EntityProperty>) {
        for property in properties {
            let mut value = property.value;
            for modifier in property
                .modifiers
                .iter()
                .filter(|modifier| modifier.operation == 0)
            {
                value += modifier.amount;
            }

            let after_add = value;
            for modifier in property
                .modifiers
                .iter()
                .filter(|modifier| modifier.operation == 1)
            {
                value += after_add * modifier.amount;
            }

            for modifier in property
                .modifiers
                .iter()
                .filter(|modifier| modifier.operation == 2)
            {
                value *= 1.0 + modifier.amount;
            }

            if property.key == "generic.maxHealth" {
                let max_health = value.max(1.0) as f32;
                match &mut self.data {
                    EntityData::Mob {
                        health,
                        max_health: current_max,
                    }
                    | EntityData::Living {
                        health,
                        max_health: current_max,
                        ..
                    } => {
                        *current_max = max_health;
                        *health = health.min(max_health);
                    }
                    // Vanilla's NetHandlerPlayClient.handleEntityProperties only
                    // updates the attribute map and never changes the entity's
                    // class. Replacing EntityData::Player here destroyed the
                    // player's name and skin profile, turning everyone into
                    // Steve as soon as the server sent S20PacketEntityProperties.
                    EntityData::None => {
                        self.data = EntityData::Living {
                            health: max_health,
                            max_health,
                            absorption: 0.0,
                        };
                    }
                    _ => {}
                }
            }

            self.attributes.insert(
                property.key.clone(),
                EntityAttribute {
                    key: property.key,
                    base: property.value,
                    value,
                },
            );
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.ticks_alive += 1;
        // Simple gravity
        if !self.on_ground {
            self.velocity.y -= 9.81 * dt;
        }
        self.position += self.velocity * dt;
        let horizontal_speed =
            (self.velocity.x * self.velocity.x + self.velocity.z * self.velocity.z).sqrt();
        self.limb_swing_amount = (horizontal_speed * 4.0).clamp(0.0, 1.0);
        self.limb_swing += horizontal_speed * dt * 6.5;
    }

    pub fn tick_visual(&mut self, dt: f32, world: &crate::world::World) {
        self.ticks_alive += 1;
        self.hurt_time = (self.hurt_time - dt).max(0.0);
        self.death_time = (self.death_time - dt).max(0.0);
        self.attack_pending_time = (self.attack_pending_time - dt).max(0.0);
        self.swing_time = (self.swing_time - dt).max(0.0);
        self.critical_time = (self.critical_time - dt).max(0.0);
        self.prev_position = self.position;
        if self.lerp_steps > 0 {
            let steps = self.lerp_steps as f32;
            self.position.x += (self.target_position.x - self.position.x) / steps;
            self.position.y += (self.target_position.y - self.position.y) / steps;
            self.position.z += (self.target_position.z - self.position.z) / steps;
            let yaw_delta = wrap_degrees(self.target_yaw - self.yaw);
            self.yaw += yaw_delta / steps;
            self.pitch += (self.target_pitch - self.pitch) / steps;
            self.lerp_steps -= 1;
        } else {
            let predicts_motion = self.predicts_local_motion();
            if predicts_motion && self.velocity.norm_squared() > 0.0000001 {
                let tick_fraction = dt * 20.0;
                let gravity_before_move = matches!(
                    self.entity_type,
                    EntityType::Item
                        | EntityType::XPOrb
                        | EntityType::PrimedTnt
                        | EntityType::FallingBlock
                );
                let (gravity, drag) = self.local_motion_constants();
                if gravity_before_move {
                    if let Some(gravity) = gravity {
                        self.velocity.y -= gravity * tick_fraction;
                    }
                }
                let displacement = self.velocity * tick_fraction;
                let arrow_hit = if self.entity_type == EntityType::Arrow {
                    let distance = displacement.norm();
                    (distance > 0.000_001)
                        .then(|| {
                            crate::client::physics::raycast(
                                &self.position,
                                &displacement,
                                distance,
                                world,
                            )
                        })
                        .flatten()
                        .map(|hit| (hit.distance, displacement / distance))
                } else {
                    None
                };

                if let Some((distance, direction)) = arrow_hit {
                    // EntityArrow stops 0.05 block before the block intercept
                    // and remains inGround until the server removes it.
                    self.position += direction * (distance - 0.05).max(0.0);
                    self.target_position = self.position;
                    self.velocity = Vector3::zeros();
                } else {
                    self.position += displacement;
                    self.target_position += displacement;

                    if self.entity_type == EntityType::Item {
                        self.clamp_to_ground(world);
                    }

                    // Vanilla EntityItem drag: air=0.98, ground H=slipperiness*0.98,
                    // water=0.8, lava=0.5. Vertical drag follows the same rules.
                    let liquid_drag = self.liquid_drag(world);
                    let (h_drag, v_drag) = if liquid_drag < 1.0 {
                        (liquid_drag, liquid_drag)
                    } else if self.on_ground {
                        (self.ground_drag(world) * drag, drag)
                    } else {
                        (drag, drag)
                    };
                    self.velocity.x *= drag_f32(h_drag, tick_fraction);
                    self.velocity.z *= drag_f32(h_drag, tick_fraction);
                    self.velocity.y *= drag_f32(v_drag, tick_fraction);
                    if !gravity_before_move {
                        if let Some(gravity) = gravity {
                            self.velocity.y -= gravity * tick_fraction;
                        }
                    }
                }
            } else {
                self.position = self.target_position;
            }
        }

        if self.entity_type == EntityType::Player {
            self.prev_chasing_position = self.chasing_position;
            let delta = self.position - self.chasing_position;
            for axis in 0..3 {
                if delta[axis].abs() > 10.0 {
                    self.chasing_position[axis] = self.position[axis];
                    self.prev_chasing_position[axis] = self.position[axis];
                }
            }
            self.chasing_position += (self.position - self.chasing_position) * 0.25;
        }

        let dx = self.position.x - self.prev_position.x;
        let dz = self.position.z - self.prev_position.z;
        let horizontal_distance = (dx * dx + dz * dz).sqrt();
        let target_amount = if dt > f32::EPSILON {
            (horizontal_distance / dt * 0.25).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.limb_swing_amount += (target_amount - self.limb_swing_amount) * 0.4;
        self.limb_swing += self.limb_swing_amount;
        self.prev_distance_walked_modified = self.distance_walked_modified;
        self.distance_walked_modified += horizontal_distance * 0.6;
        self.prev_camera_yaw = self.camera_yaw;
        let camera_yaw_target = if self.on_ground {
            horizontal_distance.min(0.1)
        } else {
            0.0
        };
        self.camera_yaw += (camera_yaw_target - self.camera_yaw) * 0.4;

        let is_projectile = matches!(
            self.entity_type,
            EntityType::Arrow
                | EntityType::Snowball
                | EntityType::ThrownEgg
                | EntityType::EnderPearl
                | EntityType::ThrownPotion
                | EntityType::ThrownExpBottle
                | EntityType::Fireball
                | EntityType::SmallFireball
                | EntityType::WitherSkull
                | EntityType::FireworkRocket
        );

        if is_projectile && self.velocity.norm_squared() > 0.000001 {
            let vx = self.velocity.x;
            let vy = self.velocity.y;
            let vz = self.velocity.z;
            // EntityArrow/EntityThrowable set rotationYaw from (motionX,
            // motionZ), and pitch from upward motion over horizontal speed.
            self.yaw = vx.atan2(vz).to_degrees();
            self.pitch = vy.atan2((vx * vx + vz * vz).sqrt()).to_degrees();
            self.body_yaw = self.yaw;
        } else {
            let mut desired_body_yaw = self.body_yaw;
            if dx * dx + dz * dz > 0.0025000002 {
                desired_body_yaw = dz.atan2(dx).to_degrees() - 90.0;
            }
            if self.swing_time > 0.0 {
                desired_body_yaw = self.yaw;
            }
            self.body_yaw += wrap_degrees(desired_body_yaw - self.body_yaw) * 0.3;
            let head_delta = wrap_degrees(self.yaw - self.body_yaw).clamp(-75.0, 75.0);
            self.body_yaw = self.yaw - head_delta;
            if head_delta * head_delta > 2500.0 {
                self.body_yaw += head_delta * 0.2;
            }
        }
    }

    pub fn update_render_position(&mut self, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        self.render_position = Point3::new(
            self.prev_position.x + (self.position.x - self.prev_position.x) * alpha,
            self.prev_position.y + (self.position.y - self.prev_position.y) * alpha,
            self.prev_position.z + (self.position.z - self.prev_position.z) * alpha,
        );
        self.render_chasing_position = Point3::new(
            self.prev_chasing_position.x
                + (self.chasing_position.x - self.prev_chasing_position.x) * alpha,
            self.prev_chasing_position.y
                + (self.chasing_position.y - self.prev_chasing_position.y) * alpha,
            self.prev_chasing_position.z
                + (self.chasing_position.z - self.prev_chasing_position.z) * alpha,
        );
    }

    fn predicts_local_motion(&self) -> bool {
        matches!(
            self.entity_type,
            EntityType::Item
                | EntityType::XPOrb
                | EntityType::PrimedTnt
                | EntityType::FallingBlock
                | EntityType::Arrow
                | EntityType::Snowball
                | EntityType::ThrownEgg
                | EntityType::EnderPearl
                | EntityType::ThrownPotion
                | EntityType::ThrownExpBottle
                | EntityType::Fireball
                | EntityType::SmallFireball
                | EntityType::WitherSkull
                | EntityType::FireworkRocket
        )
    }

    /// `(gravity, drag)` from the corresponding 1.8.9 entity class.  Collision
    /// remains server-authoritative; movement packets reconcile this lightweight
    /// client prediction whenever it differs from the world simulation.
    fn local_motion_constants(&self) -> (Option<f32>, f32) {
        match self.entity_type {
            EntityType::Arrow => (Some(0.05), 0.99),
            EntityType::Snowball | EntityType::ThrownEgg | EntityType::EnderPearl => {
                (Some(0.03), 0.99)
            }
            EntityType::ThrownPotion => (Some(0.05), 0.99),
            EntityType::ThrownExpBottle => (Some(0.07), 0.99),
            EntityType::Fireball | EntityType::SmallFireball | EntityType::WitherSkull => {
                (None, 0.95)
            }
            // EntityItem, EntityTNTPrimed and EntityFallingBlock.
            EntityType::Item | EntityType::PrimedTnt | EntityType::FallingBlock => {
                (Some(0.04), 0.98)
            }
            EntityType::XPOrb => (Some(0.03), 0.98),
            // EntityFireworkRocket does not add gravity or drag.
            EntityType::FireworkRocket => (None, 1.0),
            _ => (None, 1.0),
        }
    }

    /// Clamp an Item entity's position so it does not visually fall through
    /// solid blocks during client-side prediction.  Vanilla EntityItem.onUpdate
    /// delegates real collision to moveEntity; this lightweight approximation
    /// only prevents obvious clipping through the ground.
    fn clamp_to_ground(&mut self, world: &crate::world::World) {
        let item_size = 0.125;
        let feet_y = self.position.y - item_size;
        let block_y = feet_y.floor() as i32;
        let block_x = self.position.x.floor() as i32;
        let block_z = self.position.z.floor() as i32;
        let block_below = world.get_block(block_x, block_y, block_z);
        if block_below.is_solid() {
            let surface = block_y as f32 + 1.0 + item_size;
            if self.position.y < surface {
                self.position.y = surface;
                self.target_position.y = surface;
                self.velocity.y = 0.0;
                self.on_ground = true;
            }
        } else {
            self.on_ground = false;
        }

        if self.on_ground {
            self.velocity.y *= -0.5;
        }
    }

    /// Vanilla EntityItem slipperiness multiplier for horizontal drag on ground.
    /// Returns the block slipperiness of the block directly below the entity.
    fn ground_drag(&self, world: &crate::world::World) -> f32 {
        let x = self.position.x.floor() as i32;
        let y = (self.position.y - 0.125).floor() as i32;
        let z = self.position.z.floor() as i32;
        world.get_block(x, y, z).properties().slipperiness
    }

    /// Returns the liquid drag multiplier for the block the entity is in,
    /// or 1.0 if the entity is not submerged in water or lava.
    fn liquid_drag(&self, world: &crate::world::World) -> f32 {
        use crate::world::block::Block;
        let x = self.position.x.floor() as i32;
        let y = self.position.y.floor() as i32;
        let z = self.position.z.floor() as i32;
        let block = world.get_block(x, y, z);
        if matches!(block, Block::FlowingWater | Block::StillWater) {
            0.8
        } else if matches!(block, Block::FlowingLava | Block::StillLava) {
            0.5
        } else {
            1.0
        }
    }
}

fn drag_f32(drag: f32, tick_fraction: f32) -> f32 {
    drag.powf(tick_fraction)
}

/// Entity manager — stores all active entities.
pub struct EntityManager {
    pub entities: HashMap<EntityId, Entity>,
}

use std::collections::HashMap;

impl EntityManager {
    pub fn new() -> Self {
        EntityManager {
            entities: HashMap::new(),
        }
    }

    pub fn spawn(&mut self, entity: Entity) {
        if entity.entity_id >= 0 {
            let position = entity.position;
            let entity_type = entity.entity_type;
            // Replace the short-lived local prediction once its authoritative
            // spawn packet arrives from the server.
            self.entities.retain(|id, predicted| {
                *id >= 0
                    || predicted.entity_type != entity_type
                    || predicted.ticks_alive >= 40
                    || (predicted.position - position).norm_squared() > 16.0
            });
        }
        self.entities.insert(entity.entity_id, entity);
    }

    pub fn despawn(&mut self, entity_id: EntityId) {
        self.entities.remove(&entity_id);
    }

    pub fn despawn_batch(&mut self, ids: &[EntityId]) {
        for id in ids {
            self.entities.remove(id);
        }
    }

    pub fn despawn_all(&mut self) {
        self.entities.clear();
    }

    pub fn get(&self, entity_id: EntityId) -> Option<&Entity> {
        self.entities.get(&entity_id)
    }

    pub fn get_mut(&mut self, entity_id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&entity_id)
    }

    pub fn tick_all(&mut self, dt: f32, world: &crate::world::World) {
        for entity in self.entities.values_mut() {
            entity.tick_visual(dt, world);
        }
        self.entities
            .retain(|id, entity| *id >= 0 || entity.ticks_alive < 40);
    }

    /// Spawn entity-specific particles (mob walking, fire smoke, spell effects, etc.)
    pub fn spawn_entity_particles(
        &self,
        particles: &mut crate::client::particles::ParticleSystem,
        world: &crate::world::World,
    ) {
        use crate::entity::EntityType;

        for entity in self.entities.values() {
            if entity.ticks_alive % 4 != 0 {
                continue;
            }

            let pos = entity.position;
            let feet_y = pos.y;

            // Get the block at entity's feet
            let block_at_feet = world.get_block(pos.x as i32, feet_y as i32, pos.z as i32);

            match entity.entity_type {
                // Mobs walking on various blocks - footstep particles
                EntityType::Zombie
                | EntityType::Skeleton
                | EntityType::Spider
                | EntityType::Creeper
                | EntityType::Pig
                | EntityType::Cow
                | EntityType::Chicken
                | EntityType::Wolf
                | EntityType::Horse
                | EntityType::Ocelot
                | EntityType::Rabbit
                | EntityType::Mooshroom
                | EntityType::Squid
                | EntityType::Bat => {
                    if entity.on_ground && entity.limb_swing_amount > 0.1 {
                        particles.spawn_footstep(
                            nalgebra::Point3::new(pos.x, feet_y, pos.z),
                            block_at_feet.to_id(),
                        );
                    }
                }

                // Sheep - walking + eating grass particles
                EntityType::Sheep => {
                    if entity.on_ground && entity.limb_swing_amount > 0.1 {
                        particles.spawn_footstep(
                            nalgebra::Point3::new(pos.x, feet_y, pos.z),
                            block_at_feet.to_id(),
                        );
                    }
                    // Check metadata for eating animation (metadata index 12, byte value != 0)
                    let is_eating = entity.metadata.iter().any(|m| {
                        m.index == 12
                            && matches!(&m.value, crate::net::metadata::MetadataValue::Byte(v) if *v != 0)
                    });
                    if is_eating {
                        particles.spawn_happy_villager(nalgebra::Point3::new(
                            pos.x,
                            pos.y + 0.8,
                            pos.z,
                        ));
                    }
                }

                // Villager - walking + occasional idle particles
                EntityType::Villager => {
                    if entity.on_ground && entity.limb_swing_amount > 0.1 {
                        particles.spawn_footstep(
                            nalgebra::Point3::new(pos.x, feet_y, pos.z),
                            block_at_feet.to_id(),
                        );
                    }
                }

                // Iron Golem - walking + damage particles on swing
                EntityType::IronGolem => {
                    if entity.on_ground && entity.limb_swing_amount > 0.1 {
                        particles.spawn_footstep(
                            nalgebra::Point3::new(pos.x, feet_y, pos.z),
                            block_at_feet.to_id(),
                        );
                    }
                }

                // Blaze - fire particles
                EntityType::Blaze => {
                    if entity.ticks_alive % 2 == 0 {
                        particles.spawn_entity_smoke(
                            nalgebra::Point3::new(
                                pos.x + (pseudo(pos.x * 0.37) * 0.4 - 0.2),
                                pos.y + 1.2 + pseudo(pos.z * 0.11) * 0.3,
                                pos.z + (pseudo(pos.x * 0.19) * 0.4 - 0.2),
                            ),
                            false,
                        );
                    }
                }

                // Ghast - large smoke
                EntityType::Ghast => {
                    if entity.ticks_alive % 8 == 0 {
                        particles.spawn_entity_smoke(
                            nalgebra::Point3::new(pos.x, pos.y + 2.0, pos.z),
                            true,
                        );
                    }
                }

                // Enderman - portal particles
                EntityType::Enderman => {
                    if entity.ticks_alive % 3 == 0 {
                        let seed = entity.ticks_alive as f32;
                        particles.spawn(crate::client::particles::Particle {
                            kind: crate::client::particles::ParticleKind::Portal,
                            position: nalgebra::Point3::new(
                                pos.x + pseudo(seed) * 0.4 - 0.2,
                                pos.y + 1.5 + pseudo(seed + 2.0) * 0.5,
                                pos.z + pseudo(seed + 5.0) * 0.4 - 0.2,
                            ),
                            velocity: nalgebra::Vector3::new(0.0, 0.02, 0.0),
                            age: 0.0,
                            lifetime: 0.8,
                            size: 0.06,
                            color: [0.55, 0.18, 0.95, 0.78],
                            rotation: 0.0,
                            texture_jitter: [0.0, 0.0],
                            on_ground: false,
                        });
                    }
                }

                // Witch - magic particles
                EntityType::Witch => {
                    if entity.ticks_alive % 6 == 0 {
                        particles.spawn_witch_magic(nalgebra::Point3::new(
                            pos.x,
                            pos.y + 1.0,
                            pos.z,
                        ));
                    }
                }

                // Slime / LavaSlime - slime particles + fire
                EntityType::Slime | EntityType::LavaSlime => {
                    if entity.on_ground && entity.limb_swing_amount > 0.1 {
                        particles.spawn_slime(nalgebra::Point3::new(pos.x, feet_y, pos.z));
                    }
                    if entity.entity_type == EntityType::LavaSlime && entity.ticks_alive % 4 == 0 {
                        particles.spawn(crate::client::particles::Particle {
                            kind: crate::client::particles::ParticleKind::LavaPop,
                            position: nalgebra::Point3::new(
                                pos.x + pseudo(entity.ticks_alive as f32) * 0.6 - 0.3,
                                pos.y + 0.2,
                                pos.z + pseudo(entity.ticks_alive as f32 + 3.0) * 0.6 - 0.3,
                            ),
                            velocity: nalgebra::Vector3::new(0.0, 0.04, 0.0),
                            age: 0.0,
                            lifetime: 0.7,
                            size: 0.06,
                            color: [1.0, 0.35, 0.02, 0.92],
                            rotation: 0.0,
                            texture_jitter: [0.0, 0.0],
                            on_ground: false,
                        });
                    }
                }

                // Snow golem - snow trail
                EntityType::SnowMan => {
                    if entity.on_ground && entity.limb_swing_amount > 0.1 {
                        if entity.ticks_alive % 2 == 0 {
                            particles.spawn(crate::client::particles::Particle {
                                kind: crate::client::particles::ParticleKind::SnowShovel,
                                position: nalgebra::Point3::new(
                                    pos.x + pseudo(entity.ticks_alive as f32) * 0.3 - 0.15,
                                    feet_y + 0.01,
                                    pos.z + pseudo(entity.ticks_alive as f32 + 3.0) * 0.3 - 0.15,
                                ),
                                velocity: nalgebra::Vector3::new(0.0, 0.02, 0.0),
                                age: 0.0,
                                lifetime: 0.6,
                                size: 0.06,
                                color: [0.92, 0.96, 1.0, 0.60],
                                rotation: 0.0,
                                texture_jitter: [0.0, 0.0],
                                on_ground: false,
                            });
                        }
                    }
                }

                // XP orbs - sparkle
                EntityType::XPOrb => {
                    if entity.ticks_alive % 3 == 0 {
                        let seed = entity.ticks_alive as f32;
                        particles.spawn(crate::client::particles::Particle {
                            kind: crate::client::particles::ParticleKind::HappyVillager,
                            position: nalgebra::Point3::new(
                                pos.x + pseudo(seed) * 0.3 - 0.15,
                                pos.y + pseudo(seed + 2.0) * 0.3,
                                pos.z + pseudo(seed + 5.0) * 0.3 - 0.15,
                            ),
                            velocity: nalgebra::Vector3::new(0.0, 0.02, 0.0),
                            age: 0.0,
                            lifetime: 0.6,
                            size: 0.06,
                            color: [0.30, 0.90, 0.30, 0.80],
                            rotation: 0.0,
                            texture_jitter: [0.0, 0.0],
                            on_ground: false,
                        });
                    }
                }

                _ => {}
            }

            // Apply potion effect particles for any entity
            if !entity.active_effects.is_empty() && entity.ticks_alive % 4 == 0 {
                for effect in &entity.active_effects {
                    if !effect.hide_particles {
                        let color = potion_effect_color(effect.effect_id);
                        particles.spawn_mob_spell(
                            nalgebra::Point3::new(pos.x, pos.y + 0.5, pos.z),
                            color,
                            effect.amplifier == 0,
                        );
                    }
                }
            }
        }
    }

    pub fn update_render_positions(&mut self, alpha: f32) {
        for entity in self.entities.values_mut() {
            entity.update_render_position(alpha);
        }
    }

    pub fn count(&self) -> usize {
        self.entities.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{Entity, EntityData, EntityType};
    use crate::net::packet::EntityProperty;
    use crate::net::slot::Slot;
    use nalgebra::Point3;

    #[test]
    fn max_health_property_preserves_player_profile_data() {
        // Vanilla EntityTrackerEntry sends S20PacketEntityProperties with
        // generic.maxHealth for every tracked player. It must never replace
        // the Player data that carries the name and skin profile.
        let mut entity = Entity::new(1, EntityType::Player, Point3::origin());
        entity.data = EntityData::Player {
            name: "RemotePlayer".to_string(),
            gamemode: 0,
            skin_property: Some("encoded-skin".to_string()),
        };

        entity.apply_properties(vec![EntityProperty {
            key: "generic.maxHealth".to_string(),
            value: 20.0,
            modifiers: Vec::new(),
        }]);

        let EntityData::Player {
            name,
            skin_property,
            ..
        } = &entity.data
        else {
            panic!("player data was replaced: {:?}", entity.data);
        };
        assert_eq!(name, "RemotePlayer");
        assert_eq!(skin_property.as_deref(), Some("encoded-skin"));
        assert!(entity.attributes.contains_key("generic.maxHealth"));
    }

    #[test]
    fn max_health_property_still_promotes_untyped_entities_to_living() {
        let mut entity = Entity::new(2, EntityType::Bat, Point3::origin());
        entity.data = EntityData::None;

        entity.apply_properties(vec![EntityProperty {
            key: "generic.maxHealth".to_string(),
            value: 6.0,
            modifiers: Vec::new(),
        }]);

        assert!(matches!(
            entity.data,
            EntityData::Living {
                health,
                max_health,
                ..
            } if health == 6.0 && max_health == 6.0
        ));
    }

    #[test]
    fn spawn_object_and_spawn_mob_use_separate_type_namespaces() {
        assert_eq!(EntityType::from_object_id(50, 0), EntityType::PrimedTnt);
        assert_eq!(EntityType::from_id(50), EntityType::Creeper);
        assert_eq!(EntityType::from_object_id(61, 0), EntityType::Snowball);
        assert_eq!(EntityType::from_id(61), EntityType::Blaze);
    }

    #[test]
    fn spawn_object_minecart_data_selects_the_vanilla_variant() {
        assert_eq!(EntityType::from_object_id(10, 3), EntityType::MinecartTNT);
        assert_eq!(
            EntityType::from_object_id(10, 5),
            EntityType::MinecartHopper
        );
        assert_eq!(
            EntityType::from_object_id(10, 99),
            EntityType::MinecartEmpty
        );
    }

    #[test]
    fn living_entities_interpolate_packet_rotation_over_three_ticks() {
        let world = crate::world::World::new();
        let mut entity = Entity::new(1, EntityType::Zombie, Point3::new(0.0, 0.0, 0.0));
        entity.set_remote_rotation(90.0, 30.0, 3);
        entity.tick_visual(1.0 / 20.0, &world);
        assert_eq!((entity.yaw, entity.pitch), (30.0, 10.0));
        entity.tick_visual(1.0 / 20.0, &world);
        assert_eq!((entity.yaw, entity.pitch), (60.0, 20.0));
        entity.tick_visual(1.0 / 20.0, &world);
        assert_eq!((entity.yaw, entity.pitch), (90.0, 30.0));
    }

    #[test]
    fn death_status_remains_terminal_after_its_visual_timer_finishes() {
        let world = crate::world::World::new();
        let mut entity = Entity::new(1, EntityType::Zombie, Point3::origin());
        entity.apply_status(3);
        assert_eq!(entity.last_status, Some(3));
        assert_eq!(entity.death_time, 1.0);

        entity.tick_visual(1.0, &world);
        assert_eq!(entity.death_time, 0.0);
        assert_eq!(entity.last_status, Some(3));
    }

    #[test]
    fn attack_is_gated_until_the_server_confirms_or_the_timeout_expires() {
        let world = crate::world::World::new();
        let mut entity = Entity::new(1, EntityType::Zombie, Point3::origin());
        entity.apply_status(2);
        entity.mark_attack_pending();
        assert!(entity.attack_pending());

        entity.apply_status(2);
        assert!(entity.attack_pending());

        entity.mark_attack_pending();
        entity.tick_visual(1.0, &world);
        assert!(!entity.attack_pending());
    }

    #[test]
    fn base_entities_apply_relative_packets_immediately() {
        let mut entity = Entity::new(1, EntityType::Item, Point3::new(1.0, 2.0, 3.0));
        entity.move_relative(0.25, -0.5, 1.0, 3);
        assert_eq!(entity.position, Point3::new(1.25, 1.5, 4.0));
    }

    #[test]
    fn empty_boat_uses_eight_packet_interpolation_steps() {
        let world = crate::world::World::new();
        let mut boat = Entity::new(1, EntityType::Boat, Point3::new(0.0, 0.0, 0.0));
        assert!(boat.move_relative(0.8, 0.0, 0.0, 3));
        boat.set_remote_rotation(80.0, 0.0, 3);
        boat.tick_visual(1.0 / 20.0, &world);
        assert_eq!(boat.position.x, 0.1);
        assert_eq!(boat.yaw, 10.0);
    }

    #[test]
    fn ridden_boat_ignores_small_server_corrections() {
        let mut boat = Entity::new(1, EntityType::Boat, Point3::new(5.0, 0.0, 0.0));
        boat.set_boat_empty(false);
        assert!(!boat.move_relative(0.5, 0.0, 0.0, 3));
        assert_eq!(boat.target_position, boat.position);
    }

    #[test]
    fn minecart_uses_five_packet_interpolation_steps() {
        let world = crate::world::World::new();
        let mut minecart = Entity::new(1, EntityType::MinecartCommand, Point3::new(0.0, 0.0, 0.0));
        assert!(minecart.move_relative(1.0, 0.0, 0.0, 3));
        minecart.set_remote_rotation(100.0, 25.0, 3);
        minecart.tick_visual(1.0 / 20.0, &world);
        assert_eq!(minecart.position.x, 0.2);
        assert_eq!((minecart.yaw, minecart.pitch), (20.0, 5.0));
    }

    #[test]
    fn projectile_prediction_uses_vanilla_class_constants() {
        let world = crate::world::World::new();
        let mut arrow = Entity::new(1, EntityType::Arrow, Point3::origin());
        arrow.velocity = nalgebra::Vector3::new(0.0, 1.0, 0.0);
        arrow.tick_visual(1.0 / 20.0, &world);
        assert_eq!(arrow.position.y, 1.0);
        assert_eq!(arrow.velocity.y, 1.0_f32 * 0.99 - 0.05);

        let mut potion = Entity::new(2, EntityType::ThrownPotion, Point3::origin());
        potion.velocity = nalgebra::Vector3::new(0.0, 1.0, 0.0);
        potion.tick_visual(1.0 / 20.0, &world);
        assert_eq!(potion.velocity.y, 1.0_f32 * 0.99 - 0.05);

        let mut fireball = Entity::new(3, EntityType::Fireball, Point3::origin());
        fireball.velocity = nalgebra::Vector3::new(0.0, 1.0, 0.0);
        fireball.tick_visual(1.0 / 20.0, &world);
        assert_eq!(fireball.velocity.y, 0.95);
    }

    #[test]
    fn projectile_rotation_follows_vanilla_motion_axes() {
        let world = crate::world::World::new();
        let mut projectile = Entity::new(1, EntityType::Arrow, Point3::origin());
        projectile.velocity = nalgebra::Vector3::new(1.0, 0.0, 0.0);
        projectile.tick_visual(1.0 / 20.0, &world);

        // EntityArrow.setThrowableHeading: yaw = atan2(motionX, motionZ).
        assert!((projectile.yaw - 90.0).abs() < 0.001);
        assert!(projectile.pitch < 0.0);
    }

    #[test]
    fn tnt_and_items_predict_with_their_vanilla_pre_move_gravity() {
        let world = crate::world::World::new();
        for entity_type in [EntityType::PrimedTnt, EntityType::Item] {
            let mut entity = Entity::new(1, entity_type, Point3::origin());
            entity.velocity = nalgebra::Vector3::new(0.0, 1.0, 0.0);
            entity.tick_visual(1.0 / 20.0, &world);
            assert_eq!(entity.position.y, 0.96);
            assert_eq!(entity.velocity.y, 0.96 * 0.98);
        }
    }

    #[test]
    fn clearing_main_hand_equipment_clears_the_rendered_held_item() {
        let mut entity = Entity::new(1, EntityType::Player, Point3::origin());
        let sword = Slot {
            item_id: 276,
            count: 1,
            damage: 0,
            nbt: None,
        };

        entity.set_equipment(0, sword);
        assert_eq!(entity.current_item, Some(276));

        entity.set_equipment(0, Slot::EMPTY);
        assert_eq!(entity.current_item, None);
        assert_eq!(entity.equipment[0], None);
    }

    #[test]
    fn armor_stand_metadata_preserves_flags_and_all_six_pose_rotations() {
        let mut entity = Entity::new(1, EntityType::ArmorStand, Point3::origin());
        entity.apply_metadata(vec![
            crate::net::metadata::EntityMetadata {
                index: 10,
                value: crate::net::metadata::MetadataValue::Byte(0x0d),
            },
            crate::net::metadata::EntityMetadata {
                index: 14,
                value: crate::net::metadata::MetadataValue::Rotation([12.0, 34.0, 56.0]),
            },
        ]);

        assert_eq!(entity.visual.armor_stand_flags, 0x0d);
        assert_eq!(entity.visual.armor_stand_rotations[3], [12.0, 34.0, 56.0]);
        assert_eq!(entity.visual.armor_stand_rotations[2], [-10.0, 0.0, -10.0]);
    }
}

/// Simple pseudo-random f32 for particle spawning.
fn pseudo(seed: f32) -> f32 {
    (seed.sin() * 43758.547).fract().abs()
}

fn rand_float(seed: f32) -> f32 {
    (seed.sin() * 43758.547).fract().abs()
}

/// Get the color for a potion effect ID (MCP EffectRenderer).
pub fn potion_effect_color(effect_id: i8) -> [f32; 3] {
    match effect_id {
        1 => [0.85, 0.50, 0.20],  // Speed - orange
        2 => [0.65, 0.65, 0.65],  // Slowness - gray
        3 => [0.55, 0.95, 0.55],  // Haste - lime green
        4 => [0.45, 0.45, 0.45],  // Mining Fatigue - dark gray
        5 => [0.50, 0.15, 0.15],  // Strength - dark red
        6 => [1.00, 0.30, 0.30],  // Instant Health - red
        7 => [1.00, 0.80, 0.20],  // Instant Damage - yellow
        8 => [0.30, 0.70, 1.00],  // Jump Boost - light blue
        9 => [0.40, 0.40, 0.40],  // Nausea - dark gray
        10 => [0.65, 0.20, 0.65], // Regeneration - purple
        11 => [0.40, 0.10, 0.10], // Resistance - dark red
        12 => [0.20, 0.20, 0.20], // Fire Resistance - very dark gray
        13 => [0.30, 0.30, 0.30], // Water Breathing - dark gray
        14 => [0.15, 0.15, 0.15], // Invisibility - nearly black
        20 => [0.30, 0.80, 0.30], // Poison - green
        21 => [0.20, 0.20, 0.20], // Wither - very dark gray
        22 => [0.80, 0.80, 1.00], // Health Boost - light blue
        23 => [0.60, 0.10, 0.60], // Absorption - purple
        24 => [0.90, 0.30, 0.15], // Saturation - orange-red
        _ => [0.50, 0.50, 0.50],  // Default - gray
    }
}

/// MC 1.8.9 potion effect display name translation key.
pub fn potion_effect_name(effect_id: i8) -> &'static str {
    match effect_id {
        1 => "potion.moveSpeed",
        2 => "potion.moveSlowdown",
        3 => "potion.digSpeed",
        4 => "potion.digSlowDown",
        5 => "potion.damageBoost",
        6 => "potion.heal",
        7 => "potion.harm",
        8 => "potion.jump",
        9 => "potion.confusion",
        10 => "potion.regeneration",
        11 => "potion.resistance",
        12 => "potion.fireResistance",
        13 => "potion.waterBreathing",
        14 => "potion.invisibility",
        15 => "potion.blindness",
        16 => "potion.nightVision",
        17 => "potion.hunger",
        18 => "potion.weakness",
        19 => "potion.poison",
        20 => "potion.wither",
        21 => "potion.healthBoost",
        22 => "potion.absorption",
        23 => "potion.saturation",
        _ => "potion.empty",
    }
}

/// Vanilla MC 1.8.9 status icon index in inventory.png.
///
/// The indices come from each potion's `setIconIndex` call; they are not
/// derived from the effect ID. Instant health, instant damage, and saturation
/// deliberately have no status icon in vanilla.
pub fn potion_icon_index(effect_id: i8) -> Option<u32> {
    match effect_id {
        1 => Some(0),        // Speed
        2 => Some(1),        // Slowness
        3 => Some(2),        // Haste
        4 => Some(3),        // Mining Fatigue
        5 => Some(4),        // Strength
        8 => Some(10),       // Jump Boost
        9 => Some(11),       // Nausea
        10 => Some(7),       // Regeneration
        11 => Some(14),      // Resistance
        12 => Some(15),      // Fire Resistance
        13 => Some(16),      // Water Breathing
        14 => Some(8),       // Invisibility
        15 => Some(13),      // Blindness
        16 => Some(12),      // Night Vision
        17 => Some(9),       // Hunger
        18 => Some(5),       // Weakness
        19 => Some(6),       // Poison
        20 => Some(17),      // Wither
        21 | 22 => Some(18), // Health Boost / Absorption
        _ => None,
    }
}

/// Vanilla 1.8.9 duration string format (`StringUtils.ticksToElapsedTime`).
pub fn potion_duration_string(duration: i32) -> String {
    let seconds = duration.max(0) / 20;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Vanilla inventory effect suffix (level I is left unlabelled).
pub fn potion_amplifier_string(amplifier: i8) -> String {
    match amplifier {
        1 => "II".to_string(),
        2 => "III".to_string(),
        3 => "IV".to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod potion_display_tests {
    use super::{potion_amplifier_string, potion_duration_string, potion_icon_index};

    #[test]
    fn status_icons_match_vanilla_set_icon_index_values() {
        assert_eq!(potion_icon_index(1), Some(0));
        assert_eq!(potion_icon_index(10), Some(7));
        assert_eq!(potion_icon_index(22), Some(18));
        assert_eq!(potion_icon_index(6), None);
        assert_eq!(potion_icon_index(23), None);
    }

    #[test]
    fn effect_labels_use_vanilla_duration_and_amplifier_format() {
        assert_eq!(potion_duration_string(16), "0:00");
        assert_eq!(potion_duration_string(2_300), "1:55");
        assert_eq!(potion_amplifier_string(0), "");
        assert_eq!(potion_amplifier_string(1), "II");
        assert_eq!(potion_amplifier_string(4), "");
    }
}
