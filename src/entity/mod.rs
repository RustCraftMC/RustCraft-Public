//! Entity system — MC 1.8.9 entity definitions and real hecs multi-component storage.
//!
//! Protocol entities live in a `hecs::World` as sparse components (Position, Velocity,
//! Rotation, …). `Entity` is an owned DTO used for construction, network mutation
//! writeback, and tests — it is never stored as a monocomponent in the World.

pub mod types;

use crate::net::metadata::{EntityMetadata, MetadataValue};
use crate::net::packet::EntityProperty;
use crate::net::slot::Slot;
use crate::util::wrap_degrees;
use nalgebra::{Point3, Vector3};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

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

// ============================================================================
// ECS Components (hecs 0.11)
//
// hecs::Component is auto-implemented for all 'static + Send + Sync types.
// These components are the real World storage. `Entity` is only a DTO assembled
// for call sites and tests — never inserted as a monocomponent.
// ============================================================================

/// Protocol entity id stored on the hecs entity (mirrors `id_map` key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolId(pub EntityId);

/// Optional profile UUID for remote players / named entities.
#[derive(Clone, Debug, Default)]
pub struct Identity {
    pub uuid: Option<String>,
}

/// World-space position caches (current, previous, render, chasing).
#[derive(Clone, Debug)]
pub struct Position {
    pub current: Point3<f32>,
    pub prev: Point3<f32>,
    pub render: Point3<f32>,
    pub chasing: Point3<f32>,
    pub prev_chasing: Point3<f32>,
    pub render_chasing: Point3<f32>,
}

/// Per-tick velocity in blocks/second.
#[derive(Clone, Debug)]
pub struct Velocity(pub Vector3<f32>);

/// Rotation angles in degrees.
#[derive(Clone, Copy, Debug)]
pub struct Rotation {
    pub yaw: f32,
    pub body_yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
}

/// Remote packet interpolation targets and remaining steps.
#[derive(Clone, Debug)]
pub struct Interpolation {
    pub target_position: Point3<f32>,
    pub target_yaw: f32,
    pub target_pitch: f32,
    pub lerp_steps: u8,
}

/// Dense boolean / counter flags.
#[derive(Clone, Copy, Debug)]
pub struct Flags {
    pub on_ground: bool,
    pub using_item: bool,
    pub skin_parts: u8,
    pub ticks_alive: u32,
}

/// Visual timers and limb animation state.
#[derive(Clone, Debug)]
pub struct Animation {
    pub hurt_time: f32,
    pub death_time: f32,
    pub swing_time: f32,
    pub critical_time: f32,
    pub limb_swing: f32,
    pub limb_swing_amount: f32,
    pub distance_walked_modified: f32,
    pub prev_distance_walked_modified: f32,
    pub camera_yaw: f32,
    pub prev_camera_yaw: f32,
    pub last_status: Option<i8>,
    pub hover_start: f32,
    #[cfg(not(feature = "anti-cheat"))]
    pub attack_pending_time: f32,
    #[cfg(not(feature = "anti-cheat"))]
    pub boat_is_empty: bool,
}

/// Riding / leash attachments.
#[derive(Clone, Debug, Default)]
pub struct Attachment {
    pub vehicle_id: Option<EntityId>,
    pub leash_holder: Option<EntityId>,
}

/// Equipment slots: [main_hand, boots, leggings, chestplate, helmet] + held item id.
#[derive(Clone, Debug)]
pub struct Equipment {
    pub slots: [Option<Slot>; 5],
    pub current_item: Option<i16>,
}

/// Server-sent metadata entries.
#[derive(Clone, Debug)]
pub struct Metadata(pub Vec<EntityMetadata>);

/// Visual state derived from metadata (zombie type, horse variant, etc.).
#[derive(Clone, Copy, Debug)]
pub struct VisualState(pub EntityVisualState);

/// Active potion effects.
#[derive(Clone, Debug)]
pub struct ActiveEffects(pub Vec<EntityEffectState>);

/// Attribute snapshot from S20 EntityProperties.
#[derive(Clone, Debug)]
pub struct Attributes(pub HashMap<String, EntityAttribute>);

/// Marker component identifying the local player's entity in the hecs::World.
///
/// The Player data itself continues to be owned by `App::player` for now
/// (Task 9). The marker is the first step of the migration onto the ECS World.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalPlayer;

// EntityType (defined above) doubles as a component.
// EntityData (defined below) doubles as a data component.

/// Anti-cheat reconciliation state, gated by the `anti-cheat` cargo feature.
#[cfg(feature = "anti-cheat")]
#[derive(Clone, Debug, Default)]
pub struct AntiCheatState {
    /// Suppresses a repeated C02 attack until the server confirms the prior one.
    pub attack_pending_time: f32,
    /// EntityBoat.isBoatEmpty — switches the vanilla interpolation path.
    pub boat_is_empty: bool,
    /// Packet interpolation increment count (also mirrored on Interpolation).
    pub lerp_steps: u8,
    /// Item hover animation phase seed (also mirrored on Animation).
    pub hover_start: f32,
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
    #[cfg(not(feature = "anti-cheat"))]
    lerp_steps: u8,
    #[cfg(not(feature = "anti-cheat"))]
    boat_is_empty: bool,
    pub velocity: Vector3<f32>,
    pub yaw: f32,
    pub body_yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub skin_parts: u8,
    pub on_ground: bool,
    pub using_item: bool,
    pub ticks_alive: u32,
    #[cfg(not(feature = "anti-cheat"))]
    pub hover_start: f32,
    pub current_item: Option<i16>,
    pub equipment: [Option<Slot>; 5],
    pub metadata: Vec<EntityMetadata>,
    pub last_status: Option<i8>,
    pub hurt_time: f32,
    pub death_time: f32,
    #[cfg(not(feature = "anti-cheat"))]
    attack_pending_time: f32,
    pub swing_time: f32,
    pub critical_time: f32,
    pub limb_swing: f32,
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
    pub data: EntityData,
    #[cfg(feature = "anti-cheat")]
    pub anti_cheat: AntiCheatState,
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
            #[cfg(not(feature = "anti-cheat"))]
            lerp_steps: 0,
            #[cfg(not(feature = "anti-cheat"))]
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
            #[cfg(not(feature = "anti-cheat"))]
            hover_start: (rand_float(position.x * 13.37 + position.z * 7.31)
                * 2.0
                * std::f32::consts::PI),
            current_item: None,
            equipment: std::array::from_fn(|_| None),
            metadata: Vec::new(),
            last_status: None,
            hurt_time: 0.0,
            death_time: 0.0,
            #[cfg(not(feature = "anti-cheat"))]
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
            #[cfg(feature = "anti-cheat")]
            anti_cheat: AntiCheatState {
                attack_pending_time: 0.0,
                boat_is_empty: true,
                lerp_steps: 0,
                hover_start: rand_float(position.x * 13.37 + position.z * 7.31)
                    * 2.0
                    * std::f32::consts::PI,
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

    // ------------------------------------------------------------------------
    // Anti-cheat field accessors.
    //
    // These four fields move into the AntiCheatState component when the
    // `anti-cheat` cargo feature is enabled, and live directly on Entity
    // otherwise. The accessors keep the rest of the impl feature-agnostic.
    // ------------------------------------------------------------------------

    fn lerp_steps(&self) -> u8 {
        #[cfg(feature = "anti-cheat")]
        {
            self.anti_cheat.lerp_steps
        }
        #[cfg(not(feature = "anti-cheat"))]
        {
            self.lerp_steps
        }
    }

    fn set_lerp_steps(&mut self, steps: u8) {
        #[cfg(feature = "anti-cheat")]
        {
            self.anti_cheat.lerp_steps = steps;
        }
        #[cfg(not(feature = "anti-cheat"))]
        {
            self.lerp_steps = steps;
        }
    }

    fn boat_is_empty(&self) -> bool {
        #[cfg(feature = "anti-cheat")]
        {
            self.anti_cheat.boat_is_empty
        }
        #[cfg(not(feature = "anti-cheat"))]
        {
            self.boat_is_empty
        }
    }

    fn set_boat_is_empty(&mut self, empty: bool) {
        #[cfg(feature = "anti-cheat")]
        {
            self.anti_cheat.boat_is_empty = empty;
        }
        #[cfg(not(feature = "anti-cheat"))]
        {
            self.boat_is_empty = empty;
        }
    }

    fn attack_pending_time(&self) -> f32 {
        #[cfg(feature = "anti-cheat")]
        {
            self.anti_cheat.attack_pending_time
        }
        #[cfg(not(feature = "anti-cheat"))]
        {
            self.attack_pending_time
        }
    }

    fn set_attack_pending_time(&mut self, time: f32) {
        #[cfg(feature = "anti-cheat")]
        {
            self.anti_cheat.attack_pending_time = time;
        }
        #[cfg(not(feature = "anti-cheat"))]
        {
            self.attack_pending_time = time;
        }
    }

    /// Item hover animation phase seed. Lives on AntiCheatState under the
    /// `anti-cheat` feature, directly on Entity otherwise.
    pub fn hover_start(&self) -> f32 {
        #[cfg(feature = "anti-cheat")]
        {
            self.anti_cheat.hover_start
        }
        #[cfg(not(feature = "anti-cheat"))]
        {
            self.hover_start
        }
    }

    pub fn move_relative(&mut self, dx: f32, dy: f32, dz: f32, lerp_steps: u8) -> bool {
        if self.entity_type == EntityType::Boat {
            let target = self.target_position + Vector3::new(dx, dy, dz);
            if !self.boat_is_empty() && (target - self.position).norm_squared() <= 1.0 {
                return false;
            }
            self.target_position = target;
            self.set_lerp_steps(if self.boat_is_empty() {
                lerp_steps.saturating_add(5)
            } else {
                3
            });
            return true;
        }
        if self.is_minecart() {
            self.target_position += Vector3::new(dx, dy, dz);
            self.set_lerp_steps(lerp_steps.saturating_add(2));
            return true;
        }
        if self.uses_packet_interpolation() {
            self.target_position.x += dx;
            self.target_position.y += dy;
            self.target_position.z += dz;
            self.set_lerp_steps(self.lerp_steps().max(lerp_steps));
        } else {
            self.position += Vector3::new(dx, dy, dz);
            self.target_position = self.position;
        }
        true
    }

    pub fn teleport(&mut self, position: Point3<f32>, lerp_steps: u8) -> bool {
        if self.entity_type == EntityType::Boat {
            if !self.boat_is_empty() && (position - self.position).norm_squared() <= 1.0 {
                return false;
            }
            self.target_position = position;
            self.set_lerp_steps(if self.boat_is_empty() {
                lerp_steps.saturating_add(5)
            } else {
                3
            });
            return true;
        }
        if self.is_minecart() {
            self.target_position = position;
            self.set_lerp_steps(lerp_steps.saturating_add(2));
            return true;
        }
        if self.uses_packet_interpolation() && lerp_steps != 0 {
            self.target_position = position;
            self.set_lerp_steps(lerp_steps);
        } else {
            self.position = position;
            self.prev_position = position;
            self.render_position = position;
            self.target_position = position;
            self.set_lerp_steps(0);
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
            self.set_lerp_steps(if self.boat_is_empty() {
                lerp_steps.saturating_add(5)
            } else {
                3
            });
        } else if self.is_minecart() {
            self.target_yaw = yaw;
            self.target_pitch = pitch;
            self.set_lerp_steps(lerp_steps.saturating_add(2));
        } else if self.entity_type.is_mob() || self.entity_type == EntityType::Player {
            self.target_yaw = yaw;
            self.target_pitch = pitch;
            self.set_lerp_steps(self.lerp_steps().max(lerp_steps));
        } else {
            self.yaw = yaw;
            self.pitch = pitch;
            self.target_yaw = yaw;
            self.target_pitch = pitch;
        }
    }

    pub fn set_boat_empty(&mut self, empty: bool) {
        if self.entity_type == EntityType::Boat {
            self.set_boat_is_empty(empty);
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
                    self.set_attack_pending_time(0.0);
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
        self.set_attack_pending_time(0.05);
    }

    pub fn attack_pending(&self) -> bool {
        self.attack_pending_time() > 0.0
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
        self.set_attack_pending_time((self.attack_pending_time() - dt).max(0.0));
        self.swing_time = (self.swing_time - dt).max(0.0);
        self.critical_time = (self.critical_time - dt).max(0.0);
        self.prev_position = self.position;
        if self.lerp_steps() > 0 {
            let steps = self.lerp_steps() as f32;
            self.position.x += (self.target_position.x - self.position.x) / steps;
            self.position.y += (self.target_position.y - self.position.y) / steps;
            self.position.z += (self.target_position.z - self.position.z) / steps;
            let yaw_delta = wrap_degrees(self.target_yaw - self.yaw);
            self.yaw += yaw_delta / steps;
            self.pitch += (self.target_pitch - self.pitch) / steps;
            self.set_lerp_steps(self.lerp_steps() - 1);
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
            // Vanilla EntityLivingBase.renderYawOffset lags the facing yaw.
            // While walking, pull toward movement direction; while idle, servers
            // often only send EntityHeadLook — use head_yaw so the torso turns.
            let stationary = dx * dx + dz * dz <= 0.0025000002;
            let living = self.entity_type.is_mob() || self.entity_type == EntityType::Player;
            let facing_yaw = if stationary && living {
                self.head_yaw
            } else {
                self.yaw
            };

            let mut desired_body_yaw = facing_yaw;
            if !stationary {
                desired_body_yaw = dz.atan2(dx).to_degrees() - 90.0;
            }
            if self.swing_time > 0.0 {
                desired_body_yaw = facing_yaw;
            }
            self.body_yaw += wrap_degrees(desired_body_yaw - self.body_yaw) * 0.3;
            let head_delta = wrap_degrees(facing_yaw - self.body_yaw).clamp(-75.0, 75.0);
            self.body_yaw = facing_yaw - head_delta;
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

// ============================================================================
// EntityMut — writeback guard
// ============================================================================

/// Mutable guard returned by [`EntityManager::get_mut`].
///
/// Derefs to `&mut Entity`.  On drop, writes every field of the Entity DTO
/// back into the individual hecs components.  While alive it borrows the
/// EntityManager mutably, preventing conflicting spawns / despawns / queries.
pub struct EntityMut<'a> {
    entity: Entity,
    world: &'a hecs::World,
    handle: hecs::Entity,
}

impl<'a> Deref for EntityMut<'a> {
    type Target = Entity;
    #[inline]
    fn deref(&self) -> &Entity {
        &self.entity
    }
}

impl<'a> DerefMut for EntityMut<'a> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Entity {
        &mut self.entity
    }
}

impl<'a> Drop for EntityMut<'a> {
    fn drop(&mut self) {
        // Write back from Entity DTO into individual components.
        if let Ok(mut pos) = self.world.get::<&mut Position>(self.handle) {
            pos.current = self.entity.position;
            pos.prev = self.entity.prev_position;
            pos.render = self.entity.render_position;
            pos.chasing = self.entity.chasing_position;
            pos.prev_chasing = self.entity.prev_chasing_position;
            pos.render_chasing = self.entity.render_chasing_position;
        }
        if let Ok(mut vel) = self.world.get::<&mut Velocity>(self.handle) {
            vel.0 = self.entity.velocity;
        }
        if let Ok(mut rot) = self.world.get::<&mut Rotation>(self.handle) {
            rot.yaw = self.entity.yaw;
            rot.body_yaw = self.entity.body_yaw;
            rot.pitch = self.entity.pitch;
            rot.head_yaw = self.entity.head_yaw;
        }
        if let Ok(mut interp) = self.world.get::<&mut Interpolation>(self.handle) {
            interp.target_position = self.entity.target_position;
            interp.target_yaw = self.entity.target_yaw;
            interp.target_pitch = self.entity.target_pitch;
        }
        if let Ok(mut flags) = self.world.get::<&mut Flags>(self.handle) {
            flags.on_ground = self.entity.on_ground;
            flags.using_item = self.entity.using_item;
            flags.skin_parts = self.entity.skin_parts;
            flags.ticks_alive = self.entity.ticks_alive;
        }
        if let Ok(mut anim) = self.world.get::<&mut Animation>(self.handle) {
            anim.hurt_time = self.entity.hurt_time;
            anim.death_time = self.entity.death_time;
            anim.swing_time = self.entity.swing_time;
            anim.critical_time = self.entity.critical_time;
            anim.limb_swing = self.entity.limb_swing;
            anim.limb_swing_amount = self.entity.limb_swing_amount;
            anim.distance_walked_modified = self.entity.distance_walked_modified;
            anim.prev_distance_walked_modified = self.entity.prev_distance_walked_modified;
            anim.camera_yaw = self.entity.camera_yaw;
            anim.prev_camera_yaw = self.entity.prev_camera_yaw;
            anim.last_status = self.entity.last_status;
            anim.hover_start = self.entity.hover_start();
            #[cfg(not(feature = "anti-cheat"))]
            {
                anim.attack_pending_time = self.entity.attack_pending_time;
                anim.boat_is_empty = self.entity.boat_is_empty;
            }
        }
        #[cfg(feature = "anti-cheat")]
        if let Ok(mut ac) = self.world.get::<&mut AntiCheatState>(self.handle) {
            ac.attack_pending_time = self.entity.anti_cheat.attack_pending_time;
            ac.boat_is_empty = self.entity.anti_cheat.boat_is_empty;
            ac.lerp_steps = self.entity.anti_cheat.lerp_steps;
            ac.hover_start = self.entity.anti_cheat.hover_start;
        }
        if let Ok(mut attach) = self.world.get::<&mut Attachment>(self.handle) {
            attach.vehicle_id = self.entity.vehicle_id;
            attach.leash_holder = self.entity.leash_holder;
        }
        if let Ok(mut equip) = self.world.get::<&mut Equipment>(self.handle) {
            equip.slots = self.entity.equipment.clone();
            equip.current_item = self.entity.current_item;
        }
        if let Ok(mut md) = self.world.get::<&mut Metadata>(self.handle) {
            md.0 = self.entity.metadata.clone();
        }
        if let Ok(mut vs) = self.world.get::<&mut VisualState>(self.handle) {
            vs.0 = self.entity.visual;
        }
        if let Ok(mut eff) = self.world.get::<&mut ActiveEffects>(self.handle) {
            eff.0 = self.entity.active_effects.clone();
        }
        if let Ok(mut attr) = self.world.get::<&mut Attributes>(self.handle) {
            attr.0 = self.entity.attributes.clone();
        }
        if let Ok(mut data) = self.world.get::<&mut EntityData>(self.handle) {
            *data = self.entity.data.clone();
        }
    }
}

/// Entity manager — stores all active entities in a hecs::World keyed by
/// protocol EntityId.  Components are stored individually (Position, Velocity,
/// Rotation, …) and `Entity` is an owned DTO assembled on-the-fly for callers.
pub struct EntityManager {
    world: hecs::World,
    id_map: HashMap<EntityId, hecs::Entity>,
    /// hecs handle of the LocalPlayer marker entity. Kept outside `id_map`
    /// because the local player has no protocol EntityId of its own (the
    /// server only ever assigns ids to remote entities). Other systems
    /// discover the local player by querying `&LocalPlayer` on this handle.
    local_player_entity: Option<hecs::Entity>,
}

impl EntityManager {
    pub fn new() -> Self {
        EntityManager {
            world: hecs::World::new(),
            id_map: HashMap::new(),
            local_player_entity: None,
        }
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Reconstruct an owned [`Entity`] DTO from the individual components
    /// stored on the hecs entity identified by `handle`.
    fn assemble_entity(&self, handle: hecs::Entity) -> Option<Entity> {
        let protocol_id = self.world.get::<&ProtocolId>(handle).ok()?;
        let entity_type = *self.world.get::<&EntityType>(handle).ok()?;
        let identity = self.world.get::<&Identity>(handle).ok()?;
        let pos = self.world.get::<&Position>(handle).ok()?;
        let vel = self.world.get::<&Velocity>(handle).ok()?;
        let rot = self.world.get::<&Rotation>(handle).ok()?;
        let interp = self.world.get::<&Interpolation>(handle).ok()?;
        let flags = self.world.get::<&Flags>(handle).ok()?;
        let anim = self.world.get::<&Animation>(handle).ok()?;
        let attach = self.world.get::<&Attachment>(handle).ok()?;
        let equip = self.world.get::<&Equipment>(handle).ok()?;
        let metadata = self.world.get::<&Metadata>(handle).ok()?;
        let visual_state = self.world.get::<&VisualState>(handle).ok()?;
        let eff = self.world.get::<&ActiveEffects>(handle).ok()?;
        let attrs = self.world.get::<&Attributes>(handle).ok()?;
        let data = self.world.get::<&EntityData>(handle).ok()?;

        #[cfg(feature = "anti-cheat")]
        let ac = self.world.get::<&AntiCheatState>(handle).ok()?;

        Some(Entity {
            entity_id: protocol_id.0,
            entity_type,
            uuid: identity.uuid.clone(),
            position: pos.current,
            prev_position: pos.prev,
            render_position: pos.render,
            chasing_position: pos.chasing,
            prev_chasing_position: pos.prev_chasing,
            render_chasing_position: pos.render_chasing,
            target_position: interp.target_position,
            target_yaw: interp.target_yaw,
            target_pitch: interp.target_pitch,
            #[cfg(not(feature = "anti-cheat"))]
            lerp_steps: interp.lerp_steps,
            #[cfg(not(feature = "anti-cheat"))]
            boat_is_empty: anim.boat_is_empty,
            velocity: vel.0,
            yaw: rot.yaw,
            body_yaw: rot.body_yaw,
            pitch: rot.pitch,
            head_yaw: rot.head_yaw,
            skin_parts: flags.skin_parts,
            on_ground: flags.on_ground,
            using_item: flags.using_item,
            ticks_alive: flags.ticks_alive,
            #[cfg(not(feature = "anti-cheat"))]
            hover_start: anim.hover_start,
            current_item: equip.current_item,
            equipment: equip.slots.clone(),
            metadata: metadata.0.clone(),
            last_status: anim.last_status,
            hurt_time: anim.hurt_time,
            death_time: anim.death_time,
            #[cfg(not(feature = "anti-cheat"))]
            attack_pending_time: anim.attack_pending_time,
            swing_time: anim.swing_time,
            critical_time: anim.critical_time,
            limb_swing: anim.limb_swing,
            limb_swing_amount: anim.limb_swing_amount,
            distance_walked_modified: anim.distance_walked_modified,
            prev_distance_walked_modified: anim.prev_distance_walked_modified,
            camera_yaw: anim.camera_yaw,
            prev_camera_yaw: anim.prev_camera_yaw,
            vehicle_id: attach.vehicle_id,
            leash_holder: attach.leash_holder,
            active_effects: eff.0.clone(),
            attributes: attrs.0.clone(),
            visual: visual_state.0,
            data: EntityData::clone(&*data),
            #[cfg(feature = "anti-cheat")]
            anti_cheat: AntiCheatState::clone(&*ac),
        })
    }

    /// Decompose an [`Entity`] DTO into individual components and spawn them
    /// into the world. Used by [`spawn`] after all stale-predicate checks.
    ///
    /// hecs 0.11 supports at most 15-element bundles in a single `spawn` call,
    /// so we spawn the bulk (15 components) and insert the 16th (EntityData)
    /// separately.
    fn spawn_components(&mut self, entity: Entity) -> hecs::Entity {
        let e = self.world.spawn((
            ProtocolId(entity.entity_id),
            entity.entity_type,
            Identity { uuid: entity.uuid.clone() },
            Position {
                current: entity.position,
                prev: entity.prev_position,
                render: entity.render_position,
                chasing: entity.chasing_position,
                prev_chasing: entity.prev_chasing_position,
                render_chasing: entity.render_chasing_position,
            },
            Velocity(entity.velocity),
            Rotation {
                yaw: entity.yaw,
                body_yaw: entity.body_yaw,
                pitch: entity.pitch,
                head_yaw: entity.head_yaw,
            },
            Interpolation {
                target_position: entity.target_position,
                target_yaw: entity.target_yaw,
                target_pitch: entity.target_pitch,
                lerp_steps: entity.lerp_steps(),
            },
            Flags {
                on_ground: entity.on_ground,
                using_item: entity.using_item,
                skin_parts: entity.skin_parts,
                ticks_alive: entity.ticks_alive,
            },
            Animation {
                hurt_time: entity.hurt_time,
                death_time: entity.death_time,
                swing_time: entity.swing_time,
                critical_time: entity.critical_time,
                limb_swing: entity.limb_swing,
                limb_swing_amount: entity.limb_swing_amount,
                distance_walked_modified: entity.distance_walked_modified,
                prev_distance_walked_modified: entity.prev_distance_walked_modified,
                camera_yaw: entity.camera_yaw,
                prev_camera_yaw: entity.prev_camera_yaw,
                last_status: entity.last_status,
                hover_start: entity.hover_start(),
                #[cfg(not(feature = "anti-cheat"))]
                attack_pending_time: entity.attack_pending_time,
                #[cfg(not(feature = "anti-cheat"))]
                boat_is_empty: entity.boat_is_empty,
            },
            Attachment {
                vehicle_id: entity.vehicle_id,
                leash_holder: entity.leash_holder,
            },
            Equipment {
                slots: entity.equipment,
                current_item: entity.current_item,
            },
            Metadata(entity.metadata),
        ));
        // Insert remaining components that would exceed the 15-element limit.
        let _ = self.world.insert(e, (VisualState(entity.visual),));
        let _ = self.world.insert(e, (ActiveEffects(entity.active_effects),));
        let _ = self.world.insert(e, (Attributes(entity.attributes),));
        let _ = self.world.insert(e, (entity.data,));
        #[cfg(feature = "anti-cheat")]
        let _ = self.world.insert(e, (entity.anti_cheat,));
        e
    }

    /// Write the contents of an [`Entity`] DTO back into the individual hecs
    /// components for the entity identified by `handle`.  This duplicates the
    /// writeback logic in [`EntityMut::drop`] but without the guard overhead,
    /// and is used by batch operations such as [`tick_all`].
    fn writeback_components(&self, handle: hecs::Entity, entity: &Entity) {
        if let Ok(mut pos) = self.world.get::<&mut Position>(handle) {
            pos.current = entity.position;
            pos.prev = entity.prev_position;
            pos.render = entity.render_position;
            pos.chasing = entity.chasing_position;
            pos.prev_chasing = entity.prev_chasing_position;
            pos.render_chasing = entity.render_chasing_position;
        }
        if let Ok(mut vel) = self.world.get::<&mut Velocity>(handle) {
            vel.0 = entity.velocity;
        }
        if let Ok(mut rot) = self.world.get::<&mut Rotation>(handle) {
            rot.yaw = entity.yaw;
            rot.body_yaw = entity.body_yaw;
            rot.pitch = entity.pitch;
            rot.head_yaw = entity.head_yaw;
        }
        if let Ok(mut interp) = self.world.get::<&mut Interpolation>(handle) {
            interp.target_position = entity.target_position;
            interp.target_yaw = entity.target_yaw;
            interp.target_pitch = entity.target_pitch;
        }
        if let Ok(mut flags) = self.world.get::<&mut Flags>(handle) {
            flags.on_ground = entity.on_ground;
            flags.using_item = entity.using_item;
            flags.skin_parts = entity.skin_parts;
            flags.ticks_alive = entity.ticks_alive;
        }
        if let Ok(mut anim) = self.world.get::<&mut Animation>(handle) {
            anim.hurt_time = entity.hurt_time;
            anim.death_time = entity.death_time;
            anim.swing_time = entity.swing_time;
            anim.critical_time = entity.critical_time;
            anim.limb_swing = entity.limb_swing;
            anim.limb_swing_amount = entity.limb_swing_amount;
            anim.distance_walked_modified = entity.distance_walked_modified;
            anim.prev_distance_walked_modified = entity.prev_distance_walked_modified;
            anim.camera_yaw = entity.camera_yaw;
            anim.prev_camera_yaw = entity.prev_camera_yaw;
            anim.last_status = entity.last_status;
            anim.hover_start = entity.hover_start();
            #[cfg(not(feature = "anti-cheat"))]
            {
                anim.attack_pending_time = entity.attack_pending_time;
                anim.boat_is_empty = entity.boat_is_empty;
            }
        }
        #[cfg(feature = "anti-cheat")]
        if let Ok(mut ac) = self.world.get::<&mut AntiCheatState>(handle) {
            ac.attack_pending_time = entity.anti_cheat.attack_pending_time;
            ac.boat_is_empty = entity.anti_cheat.boat_is_empty;
            ac.lerp_steps = entity.anti_cheat.lerp_steps;
            ac.hover_start = entity.anti_cheat.hover_start;
        }
        if let Ok(mut attach) = self.world.get::<&mut Attachment>(handle) {
            attach.vehicle_id = entity.vehicle_id;
            attach.leash_holder = entity.leash_holder;
        }
        if let Ok(mut equip) = self.world.get::<&mut Equipment>(handle) {
            equip.slots.clone_from(&entity.equipment);
            equip.current_item = entity.current_item;
        }
        if let Ok(mut md) = self.world.get::<&mut Metadata>(handle) {
            md.0.clone_from(&entity.metadata);
        }
        if let Ok(mut vs) = self.world.get::<&mut VisualState>(handle) {
            vs.0 = entity.visual;
        }
        if let Ok(mut eff) = self.world.get::<&mut ActiveEffects>(handle) {
            eff.0.clone_from(&entity.active_effects);
        }
        if let Ok(mut attr) = self.world.get::<&mut Attributes>(handle) {
            attr.0.clone_from(&entity.attributes);
        }
        if let Ok(mut data) = self.world.get::<&mut EntityData>(handle) {
            *data = entity.data.clone();
        }
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Spawn (or replace) the LocalPlayer marker entity and return its handle.
    ///
    /// The marker carries only the zero-sized `LocalPlayer` tag; the Player
    /// data itself continues to be owned by `App::player` for now (Task 9).
    /// Calling this more than once replaces the previous marker so stale
    /// handles never linger across world reloads.
    pub fn spawn_local_player_marker(&mut self) -> hecs::Entity {
        if let Some(old) = self.local_player_entity.take() {
            let _ = self.world.despawn(old);
        }
        let entity = self.world.spawn((LocalPlayer,));
        self.local_player_entity = Some(entity);
        entity
    }

    /// Returns the hecs::Entity handle of the LocalPlayer marker, if present.
    pub fn local_player_entity(&self) -> Option<hecs::Entity> {
        self.local_player_entity
    }

    /// Remove the LocalPlayer marker entity, if any.
    pub fn despawn_local_player_marker(&mut self) {
        if let Some(entity) = self.local_player_entity.take() {
            let _ = self.world.despawn(entity);
        }
    }

    pub fn spawn(&mut self, entity: Entity) {
        if entity.entity_id >= 0 {
            let position = entity.position;
            let entity_type = entity.entity_type;
            // Replace the short-lived local prediction once its authoritative
            // spawn packet arrives from the server.
            let stale: Vec<EntityId> = self
                .id_map
                .iter()
                .filter(|(id, _)| **id < 0)
                .filter_map(|(id, hecs_entity)| {
                    let predicted = self.assemble_entity(*hecs_entity)?;
                    let keep = predicted.entity_type != entity_type
                        || predicted.ticks_alive >= 40
                        || (predicted.position - position).norm_squared() > 16.0;
                    (!keep).then_some(*id)
                })
                .collect();
            for id in stale {
                self.despawn(id);
            }
        }
        let entity_id = entity.entity_id;
        // Replace any existing entity with the same id.
        if let Some(old) = self.id_map.remove(&entity_id) {
            let _ = self.world.despawn(old);
        }
        let hecs_entity = self.spawn_components(entity);
        self.id_map.insert(entity_id, hecs_entity);
    }

    pub fn despawn(&mut self, entity_id: EntityId) {
        if let Some(hecs_entity) = self.id_map.remove(&entity_id) {
            let _ = self.world.despawn(hecs_entity);
        }
    }

    pub fn despawn_batch(&mut self, ids: &[EntityId]) {
        for id in ids {
            self.despawn(*id);
        }
    }

    pub fn despawn_all(&mut self) {
        self.world.clear();
        self.id_map.clear();
        self.local_player_entity = None;
    }

    /// Read-only access: returns an owned [`Entity`] DTO assembled from
    /// individual components, or `None` if the entity_id is unknown.
    pub fn get(&self, entity_id: EntityId) -> Option<Entity> {
        let handle = *self.id_map.get(&entity_id)?;
        self.assemble_entity(handle)
    }

    /// Mutable access: returns a guard that derefs to `&mut Entity`.
    /// Changes are written back to the individual hecs components when
    /// the guard is dropped.
    pub fn get_mut(&mut self, entity_id: EntityId) -> Option<EntityMut<'_>> {
        let handle = *self.id_map.get(&entity_id)?;
        let entity = self.assemble_entity(handle)?;
        Some(EntityMut {
            entity,
            world: &self.world,
            handle,
        })
    }

    /// Query iterator: iterate all entities matching a component query (read-only).
    ///
    /// Unlike [`iter`], this does NOT allocate a Vec — it streams through the
    /// archetypes.  Use `&ProtocolId` in the query tuple if you need the
    /// network EntityId.
    ///
    /// # Example
    /// ```ignore
    /// entities.for_each::<(&ProtocolId, &Position, &Rotation)>(|(id, pos, rot)| {
    ///     // read-only access
    /// });
    /// ```
    pub fn for_each<Q: hecs::Query>(&self, mut f: impl FnMut(Q::Item<'_>)) {
        for items in &mut self.world.query::<Q>() {
            f(items);
        }
    }

    /// Query iterator: iterate all entities matching a component query (mutable).
    ///
    /// See [`for_each`] for usage.
    pub fn for_each_mut<Q: hecs::Query>(&mut self, mut f: impl FnMut(Q::Item<'_>)) {
        for items in self.world.query_mut::<Q>() {
            f(items);
        }
    }

    /// Resolve the protocol EntityId to the underlying hecs::Entity handle.
    pub fn entity(&self, entity_id: EntityId) -> Option<hecs::Entity> {
        self.id_map.get(&entity_id).copied()
    }

    /// Snapshot iterator: returns owned [`Entity`] DTOs for every tracked
    /// entity.  Read-only, no writeback.
    pub fn iter(&self) -> Vec<(EntityId, Entity)> {
        self.id_map
            .iter()
            .filter_map(|(id, e)| {
                self.assemble_entity(*e)
                    .map(|entity| (*id, entity))
            })
            .collect()
    }

    /// Snapshot mutable iterator: returns owned [`Entity`] DTOs, each paired
    /// with an [`EntityMut`] guard so that any mutation is written back to
    /// the components on drop.
    ///
    /// **Important**: all guards share a borrow of `self.world`.  hecs'
    /// runtime borrow checker prevents two guards from simultaneously
    /// borrowing the *same* component on the *same* entity — but using them
    /// on *different* entities is perfectly safe.
    pub fn iter_mut(&mut self) -> Vec<(EntityId, EntityMut<'_>)> {
        let entries: Vec<(EntityId, hecs::Entity)> =
            self.id_map.iter().map(|(id, e)| (*id, *e)).collect();
        entries
            .into_iter()
            .filter_map(|(id, handle)| {
                let entity = self.assemble_entity(handle)?;
                Some((
                    id,
                    EntityMut {
                        entity,
                        world: &self.world,
                        handle,
                    },
                ))
            })
            .collect()
    }

    pub fn tick_all(&mut self, dt: f32, world: &crate::world::World) {
        // Collect handles up front so we never hold a borrow while mutating.
        let entries: Vec<(EntityId, hecs::Entity)> =
            self.id_map.iter().map(|(id, e)| (*id, *e)).collect();
        for (_, handle) in &entries {
            // Assemble DTO, call tick_visual on it, writeback.
            if let Some(mut entity) = self.assemble_entity(*handle) {
                entity.tick_visual(dt, world);
                self.writeback_components(*handle, &entity);
            }
        }
        // Remove expired local predictions (negative ProtocolId, >= 40 ticks old).
        let expired: Vec<EntityId> = {
            let mut r = Vec::new();
            self.for_each::<(hecs::Entity, &ProtocolId, &Flags)>(
                |(_, proto, flags)| {
                    if proto.0 < 0 && flags.ticks_alive >= 40 {
                        r.push(proto.0);
                    }
                },
            );
            r
        };
        for id in expired {
            self.despawn(id);
        }
    }

    /// Spawn entity-specific particles (mob walking, fire smoke, spell effects, etc.)
    pub fn spawn_entity_particles(
        &self,
        particles: &mut crate::client::particles::ParticleSystem,
        world: &crate::world::World,
    ) {
        self.for_each::<(&EntityType, &Position, &Flags, &Animation, &Metadata, &ActiveEffects)>(
            |(entity_type, pos, flags, anim, metadata, effects)| {
                if flags.ticks_alive % 4 != 0 {
                    return;
                }

                let feet_y = pos.current.y;
                let block_at_feet = world.get_block(
                    pos.current.x as i32,
                    feet_y as i32,
                    pos.current.z as i32,
                );

                match *entity_type {
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
                        if flags.on_ground && anim.limb_swing_amount > 0.1 {
                            particles.spawn_footstep(
                                nalgebra::Point3::new(pos.current.x, feet_y, pos.current.z),
                                block_at_feet.to_id(),
                            );
                        }
                    }
                    EntityType::Sheep => {
                        if flags.on_ground && anim.limb_swing_amount > 0.1 {
                            particles.spawn_footstep(
                                nalgebra::Point3::new(pos.current.x, feet_y, pos.current.z),
                                block_at_feet.to_id(),
                            );
                        }
                        let is_eating = metadata.0.iter().any(|m| {
                            m.index == 12
                                && matches!(&m.value, crate::net::metadata::MetadataValue::Byte(v) if *v != 0)
                        });
                        if is_eating {
                            particles.spawn_happy_villager(nalgebra::Point3::new(
                                pos.current.x,
                                pos.current.y + 0.8,
                                pos.current.z,
                            ));
                        }
                    }
                    EntityType::Villager => {
                        if flags.on_ground && anim.limb_swing_amount > 0.1 {
                            particles.spawn_footstep(
                                nalgebra::Point3::new(pos.current.x, feet_y, pos.current.z),
                                block_at_feet.to_id(),
                            );
                        }
                    }
                    EntityType::IronGolem => {
                        if flags.on_ground && anim.limb_swing_amount > 0.1 {
                            particles.spawn_footstep(
                                nalgebra::Point3::new(pos.current.x, feet_y, pos.current.z),
                                block_at_feet.to_id(),
                            );
                        }
                    }
                    EntityType::Blaze => {
                        if flags.ticks_alive % 2 == 0 {
                            particles.spawn_entity_smoke(
                                nalgebra::Point3::new(
                                    pos.current.x + (pseudo(pos.current.x * 0.37) * 0.4 - 0.2),
                                    pos.current.y + 1.2 + pseudo(pos.current.z * 0.11) * 0.3,
                                    pos.current.z + (pseudo(pos.current.x * 0.19) * 0.4 - 0.2),
                                ),
                                false,
                            );
                        }
                    }
                    EntityType::Ghast => {
                        if flags.ticks_alive % 8 == 0 {
                            particles.spawn_entity_smoke(
                                nalgebra::Point3::new(pos.current.x, pos.current.y + 2.0, pos.current.z),
                                true,
                            );
                        }
                    }
                    EntityType::Enderman => {
                        if flags.ticks_alive % 3 == 0 {
                            let seed = flags.ticks_alive as f32;
                            particles.spawn(crate::client::particles::Particle {
                                kind: crate::client::particles::ParticleKind::Portal,
                                position: nalgebra::Point3::new(
                                    pos.current.x + pseudo(seed) * 0.4 - 0.2,
                                    pos.current.y + 1.5 + pseudo(seed + 2.0) * 0.5,
                                    pos.current.z + pseudo(seed + 5.0) * 0.4 - 0.2,
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
                    EntityType::Witch => {
                        if flags.ticks_alive % 6 == 0 {
                            particles.spawn_witch_magic(nalgebra::Point3::new(
                                pos.current.x,
                                pos.current.y + 1.0,
                                pos.current.z,
                            ));
                        }
                    }
                    EntityType::Slime | EntityType::LavaSlime => {
                        if flags.on_ground && anim.limb_swing_amount > 0.1 {
                            particles.spawn_slime(nalgebra::Point3::new(pos.current.x, feet_y, pos.current.z));
                        }
                        if *entity_type == EntityType::LavaSlime && flags.ticks_alive % 4 == 0 {
                            particles.spawn(crate::client::particles::Particle {
                                kind: crate::client::particles::ParticleKind::LavaPop,
                                position: nalgebra::Point3::new(
                                    pos.current.x + pseudo(flags.ticks_alive as f32) * 0.6 - 0.3,
                                    pos.current.y + 0.2,
                                    pos.current.z + pseudo(flags.ticks_alive as f32 + 3.0) * 0.6 - 0.3,
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
                    EntityType::SnowMan => {
                        if flags.on_ground && anim.limb_swing_amount > 0.1 {
                            if flags.ticks_alive % 2 == 0 {
                                particles.spawn(crate::client::particles::Particle {
                                    kind: crate::client::particles::ParticleKind::SnowShovel,
                                    position: nalgebra::Point3::new(
                                        pos.current.x + pseudo(flags.ticks_alive as f32) * 0.3 - 0.15,
                                        feet_y + 0.01,
                                        pos.current.z + pseudo(flags.ticks_alive as f32 + 3.0) * 0.3 - 0.15,
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
                    EntityType::XPOrb => {
                        if flags.ticks_alive % 3 == 0 {
                            let seed = flags.ticks_alive as f32;
                            particles.spawn(crate::client::particles::Particle {
                                kind: crate::client::particles::ParticleKind::HappyVillager,
                                position: nalgebra::Point3::new(
                                    pos.current.x + pseudo(seed) * 0.3 - 0.15,
                                    pos.current.y + pseudo(seed + 2.0) * 0.3,
                                    pos.current.z + pseudo(seed + 5.0) * 0.3 - 0.15,
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

                if !effects.0.is_empty() && flags.ticks_alive % 4 == 0 {
                    for effect in &effects.0 {
                        if !effect.hide_particles {
                            let color = potion_effect_color(effect.effect_id);
                            particles.spawn_mob_spell(
                                nalgebra::Point3::new(pos.current.x, pos.current.y + 0.5, pos.current.z),
                                color,
                                effect.amplifier == 0,
                            );
                        }
                    }
                }
            },
        );
    }

    pub fn update_render_positions(&mut self, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        self.for_each_mut::<(&mut Position,)>(|(pos,)| {
            pos.render = Point3::new(
                pos.prev.x + (pos.current.x - pos.prev.x) * alpha,
                pos.prev.y + (pos.current.y - pos.prev.y) * alpha,
                pos.prev.z + (pos.current.z - pos.prev.z) * alpha,
            );
            pos.render_chasing = Point3::new(
                pos.prev_chasing.x + (pos.chasing.x - pos.prev_chasing.x) * alpha,
                pos.prev_chasing.y + (pos.chasing.y - pos.prev_chasing.y) * alpha,
                pos.prev_chasing.z + (pos.chasing.z - pos.prev_chasing.z) * alpha,
            );
        });
    }

    pub fn count(&self) -> usize {
        self.id_map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Animation, Entity, EntityData, EntityManager, EntityType, Position, Rotation,
    };
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
    fn stationary_mob_body_turns_toward_head_look() {
        let world = crate::world::World::new();
        let mut entity = Entity::new(1, EntityType::Zombie, Point3::new(0.0, 64.0, 0.0));
        entity.yaw = 0.0;
        entity.body_yaw = 0.0;
        entity.head_yaw = 90.0;
        entity.prev_position = entity.position;
        for _ in 0..20 {
            entity.tick_visual(1.0 / 20.0, &world);
        }
        let mut d = entity.head_yaw - entity.body_yaw;
        d = ((d + 180.0) % 360.0 + 360.0) % 360.0 - 180.0;
        assert!(
            d.abs() < 5.0,
            "body_yaw should track head_yaw while idle, delta={}",
            d.abs()
        );
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

    // ------------------------------------------------------------------
    // ECS integration tests — spawn → component → writeback cycle
    // ------------------------------------------------------------------

    #[test]
    fn spawn_stores_components_and_get_returns_assembled_entity() {
        let mut em = super::EntityManager::new();
        let mut entity = Entity::new(42, EntityType::Zombie, Point3::new(10.0, 20.0, 30.0));
        entity.yaw = 45.0;
        entity.pitch = 30.0;
        entity.on_ground = true;
        em.spawn(entity);

        let assembled = em.get(42).expect("entity should exist after spawn");
        assert_eq!(assembled.entity_id, 42);
        assert_eq!(assembled.entity_type, EntityType::Zombie);
        assert_eq!(assembled.position, Point3::new(10.0, 20.0, 30.0));
        assert_eq!(assembled.yaw, 45.0);
        assert_eq!(assembled.pitch, 30.0);
        assert!(assembled.on_ground);
    }

    #[test]
    fn for_each_queries_matching_entities() {
        let mut em = super::EntityManager::new();
        let e1 = Entity::new(1, EntityType::Cow, Point3::origin());
        let e2 = Entity::new(2, EntityType::Pig, Point3::new(5.0, 0.0, 5.0));
        em.spawn(e1);
        em.spawn(e2);

        let mut count = 0u32;
        em.for_each::<(&EntityType, &Position)>(|(et, pos)| {
            count += 1;
            if *et == EntityType::Cow {
                assert_eq!(pos.current, Point3::origin());
            }
            if *et == EntityType::Pig {
                assert_eq!(pos.current, Point3::new(5.0, 0.0, 5.0));
            }
        });
        assert_eq!(count, 2);
    }

    #[test]
    fn for_each_mut_writes_back_to_components() {
        let mut em = super::EntityManager::new();
        let mut entity = Entity::new(1, EntityType::Cow, Point3::origin());
        entity.yaw = 90.0;
        entity.pitch = 45.0;
        em.spawn(entity);

        em.for_each_mut::<(&mut Position, &mut Rotation)>(|(pos, rot)| {
            pos.current.x += 100.0;
            rot.yaw += 10.0;
        });

        let assembled = em.get(1).unwrap();
        assert_eq!(assembled.position.x, 100.0);
        assert_eq!(assembled.yaw, 100.0);
        assert_eq!(assembled.pitch, 45.0);
    }

    #[test]
    fn get_mut_guard_writes_back_on_drop() {
        let mut em = super::EntityManager::new();
        let mut entity = Entity::new(1, EntityType::Sheep, Point3::new(1.0, 2.0, 3.0));
        entity.velocity = nalgebra::Vector3::new(0.5, 0.0, 0.0);
        em.spawn(entity);

        {
            let mut guard = em.get_mut(1).unwrap();
            guard.position.x += 10.0;
            guard.velocity.x = 99.0;
            guard.yaw = 180.0;
        }

        let assembled = em.get(1).unwrap();
        assert_eq!(assembled.position.x, 11.0);
        assert_eq!(assembled.velocity.x, 99.0);
        assert_eq!(assembled.yaw, 180.0);
        assert_eq!(assembled.position.y, 2.0);
    }

    #[test]
    fn despawn_removes_entity_from_queries() {
        let mut em = super::EntityManager::new();
        em.spawn(Entity::new(1, EntityType::Cow, Point3::origin()));
        em.spawn(Entity::new(2, EntityType::Pig, Point3::new(5.0, 0.0, 5.0)));

        assert_eq!(em.count(), 2);
        em.despawn(1);
        assert_eq!(em.count(), 1);
        assert!(em.get(1).is_none());
        assert!(em.get(2).is_some());

        let mut count = 0u32;
        em.for_each::<(&EntityType,)>(|_| count += 1);
        assert_eq!(count, 1);
    }

    #[test]
    fn for_each_mut_cfg_components_work_with_anti_cheat() {
        let mut em = super::EntityManager::new();
        let entity = Entity::new(1, EntityType::Creeper, Point3::origin());
        em.spawn(entity);

        em.for_each_mut::<(&mut Animation,)>(|(anim,)| {
            anim.hurt_time = 5.0;
            anim.swing_time = 1.5;
        });

        let assembled = em.get(1).unwrap();
        assert_eq!(assembled.hurt_time, 5.0);
        assert_eq!(assembled.swing_time, 1.5);
    }

    #[test]
    fn update_render_positions_interpolates_correctly() {
        let mut em = super::EntityManager::new();
        let mut entity = Entity::new(1, EntityType::Cow, Point3::new(0.0, 0.0, 0.0));
        entity.prev_position = Point3::new(-10.0, 0.0, 0.0);
        entity.chasing_position = Point3::new(100.0, 0.0, 0.0);
        entity.prev_chasing_position = Point3::new(0.0, 0.0, 0.0);
        em.spawn(entity);

        em.update_render_positions(0.5);

        let assembled = em.get(1).unwrap();
        assert_eq!(assembled.render_position.x, -5.0);
        assert_eq!(assembled.render_chasing_position.x, 50.0);
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
