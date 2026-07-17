use nalgebra::{Point3, Vector3};
use rand::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleKind {
    // Block particles
    /// `block_state` uses RustCraft's `(block_id << 4) | metadata` layout.
    BlockCrack {
        block_state: u16,
    },
    /// `block_state` uses RustCraft's `(block_id << 4) | metadata` layout.
    BlockDust {
        block_state: u16,
    },
    ItemCrack {
        item_id: u16,
        damage: u16,
    },

    // Smoke variants
    Smoke,
    LargeSmoke,
    SmokeNormal,
    SmokePoof,

    // Fire/Lava
    Flame,
    LavaPop,
    Portal,
    EnchantTable,

    // Combat/Magic
    Crit,
    CritMagic,
    Magic,
    Spell,
    MobSpell,
    MobSpellAmbient,
    InstantSpell,
    WitchMagic,

    // Status/Emotion
    Heart,
    Note,
    AngryVillager,
    HappyVillager,
    Barrier,

    // Environmental
    Rain,
    WaterDrop,
    WaterSplash,
    WaterWake,
    Bubble,
    Suspended,
    DepthSuspend,
    DripWater,
    DripLava,
    SnowShovel,
    Snowball,
    Cloud,

    // Explosions
    Explosion,
    HugeExplosion,

    // Special
    EndRod,
    FireworkSpark,
    Slime,
    Footstep,
    MobAppearance,
    DamageHint,
    Redstone,
    Fountain,

    // Death/Damage
    DeathSmoke,
    DeathFall,
}

/// Physics properties for each particle kind, matching MCP 1.8.9 EffectRenderer.
#[derive(Clone, Copy, Debug)]
pub struct ParticlePhysics {
    /// Gravity multiplier (negative = rises, positive = falls, 0 = no gravity)
    pub gravity: f32,
    /// Velocity drag per tick (0.0 = no drag, 1.0 = full stop)
    pub drag: f32,
    /// Base size in blocks
    pub base_size: f32,
    /// Lifetime in seconds
    pub lifetime: f32,
    /// Whether particle is emissive (glows through fog)
    pub emissive: bool,
    /// Scale multiplier over lifetime (1.0 = constant, >1.0 = grows, <1.0 = shrinks)
    pub scale_over_life: f32,
    /// Whether to use random offset when spawning
    pub random_offset: bool,
    /// RGB color [r, g, b, a]
    pub color: [f32; 4],
}

impl ParticleKind {
    pub fn physics(self) -> ParticlePhysics {
        match self {
            // Block particles - fall with block texture
            ParticleKind::BlockCrack { .. } => ParticlePhysics {
                gravity: 1.0,
                drag: 0.0,
                base_size: 0.15,
                lifetime: 0.7,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.6, 0.6, 0.6, 1.0],
            },
            ParticleKind::BlockDust { .. } => ParticlePhysics {
                gravity: 0.67,
                drag: 0.0,
                base_size: 0.15,
                lifetime: 0.7,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.6, 0.6, 0.6, 1.0],
            },
            ParticleKind::ItemCrack { .. } => ParticlePhysics {
                gravity: 1.0,
                drag: 0.0,
                base_size: 0.15,
                lifetime: 0.7,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 1.0, 1.0, 1.0],
            },

            // Smoke variants
            ParticleKind::Smoke => ParticlePhysics {
                gravity: -0.02,
                drag: 0.0,
                base_size: 0.22,
                lifetime: 0.8,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.2, 0.2, 0.2, 1.0],
            },
            ParticleKind::LargeSmoke => ParticlePhysics {
                gravity: -0.02,
                drag: 0.0,
                base_size: 0.55,
                lifetime: 1.2,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.2, 0.2, 0.2, 1.0],
            },
            ParticleKind::SmokeNormal => ParticlePhysics {
                gravity: -0.01,
                drag: 0.0,
                base_size: 0.22,
                lifetime: 0.8,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: true,
                color: [0.2, 0.2, 0.2, 1.0],
            },
            ParticleKind::SmokePoof => ParticlePhysics {
                gravity: -0.015,
                drag: 0.0,
                base_size: 0.55,
                lifetime: 1.0,
                emissive: false,
                scale_over_life: 1.3,
                random_offset: false,
                color: [0.2, 0.2, 0.2, 1.0],
            },

            // Fire/Lava
            ParticleKind::Flame => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.28,
                lifetime: 1.1,
                emissive: true,
                scale_over_life: 0.8,
                random_offset: false,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            ParticleKind::LavaPop => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.24,
                lifetime: 0.7,
                emissive: true,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            ParticleKind::Portal => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.12,
                lifetime: 2.25,
                emissive: true,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.75, 0.22, 0.83, 1.0],
            },
            ParticleKind::EnchantTable => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.08,
                lifetime: 0.75,
                emissive: true,
                scale_over_life: 0.6,
                random_offset: false,
                color: [0.45, 0.15, 0.90, 0.85],
            },

            // Combat/Magic
            ParticleKind::Crit => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.22,
                lifetime: 0.35,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.75, 0.75, 0.75, 1.0],
            },
            ParticleKind::CritMagic => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.22,
                lifetime: 0.35,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.23, 0.6, 0.75, 1.0],
            },
            ParticleKind::Magic => ParticlePhysics {
                gravity: 0.35,
                drag: 0.0,
                base_size: 0.10,
                lifetime: 0.65,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.62, 0.25, 0.95, 0.85],
            },
            ParticleKind::Spell => ParticlePhysics {
                gravity: -0.01,
                drag: 0.0,
                base_size: 0.10,
                lifetime: 0.6,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.80, 0.80, 1.00, 0.50],
            },
            ParticleKind::MobSpell => ParticlePhysics {
                gravity: -0.01,
                drag: 0.0,
                base_size: 0.12,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.50, 0.50, 0.50, 0.45],
            },
            ParticleKind::MobSpellAmbient => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.08,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.60, 0.60, 0.60, 0.30],
            },
            ParticleKind::InstantSpell => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.10,
                lifetime: 0.4,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.75, 0.75, 1.00, 0.60],
            },
            ParticleKind::WitchMagic => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.10,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.30, 0.90, 0.30, 0.70],
            },

            // Status/Emotion
            ParticleKind::Heart => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.45,
                lifetime: 0.8,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            ParticleKind::Note => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.45,
                lifetime: 0.3,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            ParticleKind::AngryVillager => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.45,
                lifetime: 0.8,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            ParticleKind::HappyVillager => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.2,
                lifetime: 1.0,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            ParticleKind::Barrier => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.12,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.70, 0.0, 0.0, 0.80],
            },

            // Environmental
            ParticleKind::Rain => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.08,
                lifetime: 0.6,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.75, 0.85, 1.0, 0.52],
            },
            ParticleKind::WaterDrop => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.08,
                lifetime: 0.7,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.25, 0.55, 1.0, 0.78],
            },
            ParticleKind::WaterSplash => ParticlePhysics {
                gravity: 0.55,
                drag: 0.0,
                base_size: 0.10,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 0.8,
                random_offset: false,
                color: [0.30, 0.55, 1.0, 0.70],
            },
            ParticleKind::WaterWake => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.06,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 0.7,
                random_offset: false,
                color: [0.35, 0.60, 1.0, 0.65],
            },
            ParticleKind::Bubble => ParticlePhysics {
                gravity: -0.10,
                drag: 0.0,
                base_size: 0.08,
                lifetime: 0.8,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            ParticleKind::Suspended => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.04,
                lifetime: 1.5,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.40, 0.60, 0.90, 0.35],
            },
            ParticleKind::DepthSuspend => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.03,
                lifetime: 2.0,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.10, 0.10, 0.30, 0.25],
            },
            ParticleKind::DripWater => ParticlePhysics {
                gravity: 0.28,
                drag: 0.0,
                base_size: 0.04,
                lifetime: 1.2,
                emissive: false,
                scale_over_life: 0.5,
                random_offset: false,
                color: [0.25, 0.50, 1.0, 0.80],
            },
            ParticleKind::DripLava => ParticlePhysics {
                gravity: 0.28,
                drag: 0.0,
                base_size: 0.04,
                lifetime: 1.2,
                emissive: true,
                scale_over_life: 0.5,
                random_offset: false,
                color: [1.0, 0.40, 0.0, 0.90],
            },
            ParticleKind::SnowShovel => ParticlePhysics {
                gravity: 0.80,
                drag: 0.0,
                base_size: 0.08,
                lifetime: 0.6,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.92, 0.96, 1.0, 0.92],
            },
            ParticleKind::Snowball => ParticlePhysics {
                gravity: 0.35,
                drag: 0.0,
                base_size: 0.10,
                lifetime: 0.65,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.92, 0.96, 1.0, 0.92],
            },
            ParticleKind::Cloud => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.30,
                lifetime: 2.0,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.90, 0.90, 0.90, 0.20],
            },

            // Explosions
            ParticleKind::Explosion => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.6,
                lifetime: 1.0,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.85, 0.85, 0.85, 1.0],
            },
            ParticleKind::HugeExplosion => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 3.0,
                lifetime: 0.4,
                emissive: true,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.75, 0.75, 0.75, 1.0],
            },

            // Special
            ParticleKind::EndRod => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.06,
                lifetime: 1.0,
                emissive: true,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.90, 0.75, 1.0, 0.85],
            },
            ParticleKind::FireworkSpark => ParticlePhysics {
                gravity: 0.08,
                drag: 0.0,
                base_size: 0.08,
                lifetime: 0.8,
                emissive: true,
                scale_over_life: 0.8,
                random_offset: false,
                color: [1.0, 0.70, 0.30, 0.90],
            },
            ParticleKind::Slime => ParticlePhysics {
                gravity: 0.35,
                drag: 0.0,
                base_size: 0.12,
                lifetime: 0.6,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.30, 0.80, 0.30, 0.80],
            },
            ParticleKind::Footstep => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.06,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.60, 0.55, 0.50, 0.40],
            },
            ParticleKind::MobAppearance => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.50,
                lifetime: 1.5,
                emissive: true,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.25, 0.50, 1.0, 0.90],
            },
            ParticleKind::DamageHint => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.15,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 0.30, 0.30, 0.90],
            },
            ParticleKind::Redstone => ParticlePhysics {
                gravity: 0.0,
                drag: 0.0,
                base_size: 0.08,
                lifetime: 0.5,
                emissive: true,
                scale_over_life: 1.0,
                random_offset: false,
                color: [1.0, 0.05, 0.02, 0.92],
            },
            ParticleKind::Fountain => ParticlePhysics {
                gravity: 0.50,
                drag: 0.0,
                base_size: 0.06,
                lifetime: 0.8,
                emissive: false,
                scale_over_life: 0.6,
                random_offset: false,
                color: [0.30, 0.55, 1.0, 0.70],
            },

            // Death/Damage
            ParticleKind::DeathSmoke => ParticlePhysics {
                gravity: -0.02,
                drag: 0.0,
                base_size: 0.18,
                lifetime: 0.8,
                emissive: false,
                scale_over_life: 1.2,
                random_offset: false,
                color: [0.40, 0.40, 0.40, 0.70],
            },
            ParticleKind::DeathFall => ParticlePhysics {
                gravity: 0.80,
                drag: 0.0,
                base_size: 0.10,
                lifetime: 0.5,
                emissive: false,
                scale_over_life: 1.0,
                random_offset: false,
                color: [0.70, 0.20, 0.10, 0.80],
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub kind: ParticleKind,
    pub position: Point3<f32>,
    pub velocity: Vector3<f32>,
    pub age: f32,
    pub lifetime: f32,
    pub size: f32,
    pub color: [f32; 4],
    /// Rotation in radians (used by some particles like note, angry villager)
    pub rotation: f32,
    /// Stable 0..3 texture offset selected when the particle is constructed.
    /// EntityFX calls these particleTextureJitterX/Y.
    pub texture_jitter: [f32; 2],
    /// Set by Entity.moveEntity-style world collision resolution.
    pub on_ground: bool,
}

pub struct ParticleSystem {
    particles: Vec<Particle>,
    max_particles: usize,
    spawn_sequence: u64,
    generation: u64,
    replace_cursor: usize,
}

impl ParticleSystem {
    pub fn new(max_particles: usize) -> Self {
        Self {
            particles: Vec::with_capacity(max_particles.min(4096)),
            max_particles,
            spawn_sequence: 0,
            generation: 0,
            replace_cursor: 0,
        }
    }

    pub fn spawn(&mut self, particle: Particle) {
        if self.max_particles == 0 {
            return;
        }
        if self.particles.len() >= self.max_particles {
            self.particles[self.replace_cursor] = particle;
            self.replace_cursor = (self.replace_cursor + 1) % self.particles.len();
        } else {
            self.particles.push(particle);
        }
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn spawn_block_break(&mut self, block_state: u16, origin: Point3<f32>) {
        let kind = ParticleKind::BlockCrack { block_state };
        let mut rng = rand::thread_rng();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    let offset = Vector3::new(
                        (x as f32 + 0.5) / 4.0,
                        (y as f32 + 0.5) / 4.0,
                        (z as f32 + 0.5) / 4.0,
                    );
                    let input_velocity = offset - Vector3::new(0.5, 0.5, 0.5);
                    self.spawn(vanilla_digging_particle(
                        kind,
                        origin + offset,
                        input_velocity,
                        1.0,
                        &mut rng,
                    ));
                }
            }
        }
    }

    pub fn spawn_block_hit(
        &mut self,
        block_state: u16,
        origin: Point3<f32>,
        face: crate::client::physics::BlockFace,
    ) {
        let mut rng = rand::thread_rng();
        let margin = 0.1;
        let mut position = origin
            + Vector3::new(
                margin + rng.gen::<f32>() * (1.0 - margin * 2.0),
                margin + rng.gen::<f32>() * (1.0 - margin * 2.0),
                margin + rng.gen::<f32>() * (1.0 - margin * 2.0),
            );
        match face {
            crate::client::physics::BlockFace::Bottom => position.y = origin.y - margin,
            crate::client::physics::BlockFace::Top => position.y = origin.y + 1.0 + margin,
            crate::client::physics::BlockFace::North => position.z = origin.z - margin,
            crate::client::physics::BlockFace::South => position.z = origin.z + 1.0 + margin,
            crate::client::physics::BlockFace::West => position.x = origin.x - margin,
            crate::client::physics::BlockFace::East => position.x = origin.x + 1.0 + margin,
        }

        self.spawn(vanilla_digging_particle(
            ParticleKind::BlockCrack { block_state },
            position,
            Vector3::zeros(),
            0.6,
            &mut rng,
        ));
    }

    pub fn spawn_explosion(&mut self, origin: Point3<f32>, radius: f32, record_count: usize) {
        let amount = (record_count / 2).clamp(16, 96);
        for i in 0..amount {
            let seed = i as f32 + radius * 5.17;
            let dir = Vector3::new(
                pseudo(seed) * 2.0 - 1.0,
                pseudo(seed + 2.0) * 2.0 - 1.0,
                pseudo(seed + 5.0) * 2.0 - 1.0,
            )
            .try_normalize(1.0e-4)
            .unwrap_or_else(|| Vector3::new(0.0, 1.0, 0.0));
            let kind = ParticleKind::Explosion;
            let physics = kind.physics();
            self.spawn(Particle {
                kind,
                position: origin + dir * radius.min(4.0) * pseudo(seed + 11.0),
                velocity: dir * (0.6 + radius * 0.08),
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn huge explosion (TNT, end crystal, etc.)
    pub fn spawn_huge_explosion(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::HugeExplosion;
        let physics = kind.physics();
        for i in 0..64 {
            let seed = i as f32;
            let dir = Vector3::new(
                pseudo(seed) * 2.0 - 1.0,
                pseudo(seed + 2.0) * 2.0 - 1.0,
                pseudo(seed + 5.0) * 2.0 - 1.0,
            )
            .try_normalize(1.0e-4)
            .unwrap_or_else(|| Vector3::new(0.0, 1.0, 0.0));
            self.spawn(Particle {
                kind,
                position: origin + dir * pseudo(seed + 1.0) * 2.0,
                velocity: dir * 0.4,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn smoke particles around an entity (fire, damage, etc.)
    pub fn spawn_entity_smoke(&mut self, origin: Point3<f32>, large: bool) {
        let kind = if large {
            ParticleKind::LargeSmoke
        } else {
            ParticleKind::SmokeNormal
        };
        let physics = kind.physics();
        for i in 0..4 {
            let seed = i as f32 + origin.x * 0.37 + origin.z * 0.11;
            let offset = Vector3::new(
                pseudo(seed) * 0.6 - 0.3,
                pseudo(seed + 2.0) * 0.5,
                pseudo(seed + 5.0) * 0.6 - 0.3,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.1,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn footstep particle (for mob walking)
    pub fn spawn_footstep(&mut self, origin: Point3<f32>, _block_id: u16) {
        let kind = ParticleKind::Footstep;
        let physics = kind.physics();
        for i in 0..3 {
            let seed = i as f32 + origin.x * 0.37 + origin.z * 0.11;
            let offset = Vector3::new(
                pseudo(seed) * 0.4 - 0.2,
                0.01,
                pseudo(seed + 3.0) * 0.4 - 0.2,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: Vector3::new(
                    pseudo(seed + 1.0) * 0.02 - 0.01,
                    0.0,
                    pseudo(seed + 4.0) * 0.02 - 0.01,
                ),
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn water/lava drip from above
    pub fn spawn_drip(&mut self, origin: Point3<f32>, is_lava: bool) {
        let kind = if is_lava {
            ParticleKind::DripLava
        } else {
            ParticleKind::DripWater
        };
        let physics = kind.physics();
        self.spawn(Particle {
            kind,
            position: origin,
            velocity: Vector3::new(0.0, -0.15, 0.0),
            age: 0.0,
            lifetime: physics.lifetime,
            size: physics.base_size,
            color: physics.color,
            rotation: 0.0,
            texture_jitter: [0.0, 0.0],
            on_ground: false,
        });
    }

    /// Spawn splash particles when entity enters/exits water
    pub fn spawn_water_splash(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::WaterSplash;
        let physics = kind.physics();
        for i in 0..8 {
            let seed = i as f32 + origin.x * 0.37;
            let angle = pseudo(seed) * std::f32::consts::TAU;
            let speed = 0.2 + pseudo(seed + 1.0) * 0.3;
            self.spawn(Particle {
                kind,
                position: origin
                    + Vector3::new(
                        pseudo(seed + 2.0) * 0.3 - 0.15,
                        0.01,
                        pseudo(seed + 5.0) * 0.3 - 0.15,
                    ),
                velocity: Vector3::new(
                    angle.cos() * speed * 0.5,
                    0.3 + pseudo(seed + 3.0) * 0.4,
                    angle.sin() * speed * 0.5,
                ),
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn cloud particles
    pub fn spawn_cloud(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::Cloud;
        let physics = kind.physics();
        for i in 0..8 {
            let seed = i as f32 + origin.x * 0.17 + origin.z * 0.31;
            let offset = Vector3::new(
                pseudo(seed) * 2.0 - 1.0,
                pseudo(seed + 2.0) * 0.3,
                pseudo(seed + 5.0) * 2.0 - 1.0,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.02,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn enchantment table particles
    pub fn spawn_enchant_table(&mut self, origin: Point3<f32>, target: Point3<f32>) {
        let kind = ParticleKind::EnchantTable;
        let physics = kind.physics();
        for i in 0..16 {
            let seed = i as f32;
            let offset = Vector3::new(
                pseudo(seed) * 1.5 - 0.75,
                pseudo(seed + 2.0) * 1.5 + 0.5,
                pseudo(seed + 5.0) * 1.5 - 0.75,
            );
            let dir = (target - (origin + offset))
                .try_normalize(1.0e-4)
                .unwrap_or_else(|| Vector3::new(0.0, 1.0, 0.0));
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: dir * 0.05,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: pseudo(seed + 7.0) * 6.28,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn redstone dust particles
    pub fn spawn_redstone(&mut self, origin: Point3<f32>, color: Option<[f32; 3]>) {
        let kind = ParticleKind::Redstone;
        let physics = kind.physics();
        let c = color.unwrap_or([physics.color[0], physics.color[1], physics.color[2]]);
        for i in 0..3 {
            let seed = i as f32 + origin.x * 0.37;
            let offset = Vector3::new(
                pseudo(seed) * 0.3 - 0.15,
                pseudo(seed + 2.0) * 0.3,
                pseudo(seed + 5.0) * 0.3 - 0.15,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.05,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: [c[0], c[1], c[2], physics.color[3]],
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn fountain particles (when bucket empties)
    pub fn spawn_fountain(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::Fountain;
        let physics = kind.physics();
        for i in 0..12 {
            let seed = i as f32;
            let angle = pseudo(seed) * std::f32::consts::TAU;
            let speed = 0.3 + pseudo(seed + 1.0) * 0.5;
            self.spawn(Particle {
                kind,
                position: origin + Vector3::new(0.0, 0.1, 0.0),
                velocity: Vector3::new(
                    angle.cos() * speed * 0.3,
                    0.5 + pseudo(seed + 2.0) * 0.5,
                    angle.sin() * speed * 0.3,
                ),
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn snow shovel particles (breaking snow layer)
    pub fn spawn_snow_shovel(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::SnowShovel;
        let physics = kind.physics();
        for i in 0..8 {
            let seed = i as f32 + origin.x * 0.37;
            let offset = Vector3::new(
                pseudo(seed) * 0.5 - 0.25,
                pseudo(seed + 2.0) * 0.3,
                pseudo(seed + 5.0) * 0.5 - 0.25,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.8,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn slime particles (when slime jumps or splits)
    pub fn spawn_slime(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::Slime;
        let physics = kind.physics();
        for i in 0..4 {
            let seed = i as f32 + origin.x * 0.37;
            let offset = Vector3::new(
                pseudo(seed) * 0.6 - 0.3,
                0.0,
                pseudo(seed + 3.0) * 0.6 - 0.3,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.5,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn angry villager particles
    pub fn spawn_angry_villager(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::AngryVillager;
        let physics = kind.physics();
        for i in 0..4 {
            let seed = i as f32;
            let offset = Vector3::new(
                pseudo(seed) * 0.6 - 0.3,
                pseudo(seed + 2.0) * 0.4,
                pseudo(seed + 5.0) * 0.6 - 0.3,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.1,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: pseudo(seed + 7.0) * 6.28,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn happy villager particles
    pub fn spawn_happy_villager(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::HappyVillager;
        let physics = kind.physics();
        for i in 0..6 {
            let seed = i as f32;
            let offset = Vector3::new(
                pseudo(seed) * 0.8 - 0.4,
                pseudo(seed + 2.0) * 0.5,
                pseudo(seed + 5.0) * 0.8 - 0.4,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.15,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: pseudo(seed + 7.0) * 6.28,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn firework spark particles
    pub fn spawn_firework_spark(&mut self, origin: Point3<f32>, color: [f32; 3]) {
        let kind = ParticleKind::FireworkSpark;
        let physics = kind.physics();
        for i in 0..16 {
            let seed = i as f32;
            let dir = Vector3::new(
                pseudo(seed) * 2.0 - 1.0,
                pseudo(seed + 2.0) * 2.0 - 1.0,
                pseudo(seed + 5.0) * 2.0 - 1.0,
            )
            .try_normalize(1.0e-4)
            .unwrap_or_else(|| Vector3::new(0.0, 1.0, 0.0));
            self.spawn(Particle {
                kind,
                position: origin,
                velocity: dir * (0.5 + pseudo(seed + 1.0) * 1.0),
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: [color[0], color[1], color[2], physics.color[3]],
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn end rod particles
    pub fn spawn_end_rod(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::EndRod;
        let physics = kind.physics();
        for i in 0..4 {
            let seed = i as f32;
            let offset = Vector3::new(
                pseudo(seed) * 0.4 - 0.2,
                pseudo(seed + 2.0) * 0.2,
                pseudo(seed + 5.0) * 0.4 - 0.2,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: Vector3::new(0.0, 0.04 + pseudo(seed + 3.0) * 0.02, 0.0),
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn spell particles around an entity (potion effects)
    pub fn spawn_mob_spell(&mut self, origin: Point3<f32>, color: [f32; 3], ambient: bool) {
        let kind = if ambient {
            ParticleKind::MobSpellAmbient
        } else {
            ParticleKind::MobSpell
        };
        let physics = kind.physics();
        for i in 0..6 {
            let seed = i as f32 + origin.x * 0.37;
            let offset = Vector3::new(
                pseudo(seed) * 0.5 - 0.25,
                pseudo(seed + 2.0) * 0.8,
                pseudo(seed + 5.0) * 0.5 - 0.25,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: Vector3::new(
                    pseudo(seed + 1.0) * 0.02 - 0.01,
                    0.02,
                    pseudo(seed + 4.0) * 0.02 - 0.01,
                ),
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: [color[0], color[1], color[2], physics.color[3]],
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn instant spell particles
    pub fn spawn_instant_spell(&mut self, origin: Point3<f32>, color: [f32; 3]) {
        let kind = ParticleKind::InstantSpell;
        let physics = kind.physics();
        for i in 0..8 {
            let seed = i as f32 + origin.x * 0.37;
            let angle = pseudo(seed) * std::f32::consts::TAU;
            let r = 0.2 + pseudo(seed + 1.0) * 0.3;
            let offset = Vector3::new(angle.cos() * r, pseudo(seed + 2.0) * 0.5, angle.sin() * r);
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.15,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: [color[0], color[1], color[2], physics.color[3]],
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn witch magic particles
    pub fn spawn_witch_magic(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::WitchMagic;
        let physics = kind.physics();
        for i in 0..8 {
            let seed = i as f32;
            let offset = Vector3::new(
                pseudo(seed) * 0.6 - 0.3,
                pseudo(seed + 2.0) * 0.6,
                pseudo(seed + 5.0) * 0.6 - 0.3,
            );
            self.spawn(Particle {
                kind,
                position: origin + offset,
                velocity: offset * 0.1,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: physics.color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    /// Spawn damage hint particles (sword critical, etc.)
    pub fn spawn_damage_hint(&mut self, origin: Point3<f32>, _amount: f32) {
        let kind = ParticleKind::DamageHint;
        let physics = kind.physics();
        self.spawn(Particle {
            kind,
            position: origin + Vector3::new(0.0, 0.5, 0.0),
            velocity: Vector3::new(0.0, 0.3, 0.0),
            age: 0.0,
            lifetime: physics.lifetime,
            size: physics.base_size,
            color: physics.color,
            rotation: 0.0,
            texture_jitter: [0.0, 0.0],
            on_ground: false,
        });
    }

    /// Spawn barrier particles
    pub fn spawn_barrier(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::Barrier;
        let physics = kind.physics();
        self.spawn(Particle {
            kind,
            position: origin + Vector3::new(0.5, 0.5, 0.5),
            velocity: Vector3::new(0.0, 0.0, 0.0),
            age: 0.0,
            lifetime: physics.lifetime,
            size: physics.base_size,
            color: physics.color,
            rotation: 0.0,
            texture_jitter: [0.0, 0.0],
            on_ground: false,
        });
    }

    /// Spawn elder guardian mob appearance effect
    pub fn spawn_mob_appearance(&mut self, origin: Point3<f32>) {
        let kind = ParticleKind::MobAppearance;
        let physics = kind.physics();
        self.spawn(Particle {
            kind,
            position: origin + Vector3::new(0.0, 0.5, 0.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),
            age: 0.0,
            lifetime: physics.lifetime,
            size: physics.base_size,
            color: physics.color,
            rotation: 0.0,
            texture_jitter: [0.0, 0.0],
            on_ground: false,
        });
    }

    pub fn spawn_from_server(
        &mut self,
        particle_id: i32,
        origin: Point3<f32>,
        offset: Vector3<f32>,
        speed: f32,
        count: i32,
        data: &[i32],
    ) {
        let kind = match particle_id {
            0 => ParticleKind::Explosion,
            1 | 2 => ParticleKind::HugeExplosion,
            3 => ParticleKind::FireworkSpark,
            4 => ParticleKind::Bubble,
            5 => ParticleKind::WaterSplash,
            6 => ParticleKind::WaterWake,
            7 => ParticleKind::Suspended,
            8 => ParticleKind::DepthSuspend,
            9 => ParticleKind::Crit,
            10 => ParticleKind::CritMagic,
            11 => ParticleKind::SmokeNormal,
            12 => ParticleKind::LargeSmoke,
            13 => ParticleKind::Spell,
            14 => ParticleKind::InstantSpell,
            15 => ParticleKind::MobSpell,
            16 => ParticleKind::MobSpellAmbient,
            17 => ParticleKind::WitchMagic,
            18 => ParticleKind::DripWater,
            19 => ParticleKind::DripLava,
            20 => ParticleKind::AngryVillager,
            21 => ParticleKind::HappyVillager,
            22 => ParticleKind::Suspended,
            23 => ParticleKind::Note,
            24 => ParticleKind::Portal,
            25 => ParticleKind::EnchantTable,
            26 => ParticleKind::Flame,
            27 => ParticleKind::LavaPop,
            28 => ParticleKind::Footstep,
            29 => ParticleKind::Cloud,
            30 => ParticleKind::Redstone,
            31 => ParticleKind::Snowball,
            32 => ParticleKind::SnowShovel,
            33 => ParticleKind::Slime,
            34 => ParticleKind::Heart,
            35 => ParticleKind::Barrier,
            36 => {
                let item_id = data.first().copied().unwrap_or(0).max(0) as u16;
                let damage = data.get(1).copied().unwrap_or(0).max(0) as u16;
                ParticleKind::ItemCrack { item_id, damage }
            }
            37 => {
                let state_id = data.first().copied().unwrap_or(0).max(0) as u16;
                let block_state = ((state_id & 4095) << 4) | ((state_id >> 12) & 15);
                ParticleKind::BlockCrack { block_state }
            }
            38 => {
                let state_id = data.first().copied().unwrap_or(0).max(0) as u16;
                let block_state = ((state_id & 4095) << 4) | ((state_id >> 12) & 15);
                ParticleKind::BlockDust { block_state }
            }
            39 => ParticleKind::WaterDrop,
            40 => ParticleKind::DamageHint,
            41 => ParticleKind::MobAppearance,
            _ => return,
        };

        let physics = kind.physics();
        if count == 0 {
            let color = server_particle_color(kind, offset, physics.color);
            let velocity = server_particle_velocity(kind, offset, speed);
            self.spawn(Particle {
                kind,
                position: origin,
                velocity,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color,
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
            return;
        }

        let packet_seed = self.spawn_sequence as f32 * 101.37;
        self.spawn_sequence = self.spawn_sequence.wrapping_add(1);
        let amount = count.max(0).min(self.max_particles as i32);
        for i in 0..amount {
            let seed = i as f32
                + particle_id as f32 * 37.17
                + origin.x * 3.11
                + origin.y * 5.73
                + origin.z * 7.19
                + packet_seed;
            let position_jitter = Vector3::new(
                gaussian(seed + 0.13),
                gaussian(seed + 11.71),
                gaussian(seed + 23.29),
            );
            let velocity_jitter = Vector3::new(
                gaussian(seed + 37.43),
                gaussian(seed + 53.87),
                gaussian(seed + 71.03),
            );
            let physics = kind.physics();
            let pos = origin
                + Vector3::new(
                    position_jitter.x * offset.x,
                    position_jitter.y * offset.y,
                    position_jitter.z * offset.z,
                );

            self.spawn(Particle {
                kind,
                position: pos,
                velocity: velocity_jitter * speed * 20.0,
                age: 0.0,
                lifetime: physics.lifetime,
                size: physics.base_size,
                color: server_particle_color(kind, offset, physics.color),
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.tick_internal(dt, None);
    }

    pub fn tick_in_world(&mut self, dt: f32, world: &crate::world::World) {
        self.tick_internal(dt, Some(world));
    }

    fn tick_internal(&mut self, dt: f32, world: Option<&crate::world::World>) {
        if self.particles.is_empty() {
            return;
        }
        let tick_scale = dt * 20.0;
        for particle in &mut self.particles {
            particle.age += dt;
            let physics = particle.kind.physics();
            let (vertical_acceleration, damping): (f32, f32) = match particle.kind {
                ParticleKind::Smoke
                | ParticleKind::LargeSmoke
                | ParticleKind::SmokeNormal
                | ParticleKind::SmokePoof
                | ParticleKind::Explosion
                | ParticleKind::Spell
                | ParticleKind::MobSpell
                | ParticleKind::MobSpellAmbient
                | ParticleKind::InstantSpell
                | ParticleKind::WitchMagic => (1.6, 0.96),
                ParticleKind::Bubble => (0.8, 0.85),
                ParticleKind::Crit | ParticleKind::CritMagic | ParticleKind::Magic => (-8.0, 0.70),
                ParticleKind::Heart | ParticleKind::AngryVillager | ParticleKind::HappyVillager => {
                    (0.0, 0.86)
                }
                ParticleKind::Note => (0.0, 0.66),
                ParticleKind::Flame | ParticleKind::Cloud | ParticleKind::Redstone => (0.0, 0.96),
                ParticleKind::Portal | ParticleKind::EnchantTable => (0.0, 1.0),
                _ => (-16.0 * physics.gravity, 0.98),
            };
            particle.velocity.y += vertical_acceleration * dt;
            let movement = particle.velocity * dt;
            if let Some(world) = world {
                crate::client::physics::move_particle_with_collision(
                    &mut particle.position,
                    &mut particle.velocity,
                    movement,
                    world,
                    &mut particle.on_ground,
                );
            } else {
                particle.position += movement;
                particle.on_ground = false;
            }
            particle.velocity *= damping.powf(tick_scale);
            if particle.on_ground {
                particle.velocity.x *= 0.7_f32.powf(tick_scale);
                particle.velocity.z *= 0.7_f32.powf(tick_scale);
            }
            if matches!(
                particle.kind,
                ParticleKind::Crit | ParticleKind::CritMagic | ParticleKind::Magic
            ) {
                particle.color[1] *= 0.96_f32.powf(tick_scale);
                particle.color[2] *= 0.90_f32.powf(tick_scale);
            }
            if physics.drag > 0.0 {
                particle.velocity *= (1.0 - physics.drag).max(0.0).powf(tick_scale);
            }
        }
        self.particles.retain(|p| p.age < p.lifetime);
        self.replace_cursor = 0;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Construct EntityDiggingFX with the randomisation performed by EntityFX.
/// Stored velocity is converted from vanilla blocks/tick to blocks/second.
fn vanilla_digging_particle(
    kind: ParticleKind,
    position: Point3<f32>,
    input_velocity: Vector3<f32>,
    scale: f32,
    rng: &mut impl Rng,
) -> Particle {
    let texture_jitter = [rng.gen::<f32>() * 3.0, rng.gen::<f32>() * 3.0];
    let particle_scale = (rng.gen::<f32>() * 0.5 + 0.5) * scale;
    let max_age_ticks = (4.0 / (rng.gen::<f32>() * 0.9 + 0.1)) as u32;

    let mut motion = input_velocity
        + Vector3::new(
            (rng.gen::<f32>() * 2.0 - 1.0) * 0.4,
            (rng.gen::<f32>() * 2.0 - 1.0) * 0.4,
            (rng.gen::<f32>() * 2.0 - 1.0) * 0.4,
        );
    let random_speed = (rng.gen::<f32>() + rng.gen::<f32>() + 1.0) * 0.15;
    let length = motion.norm();
    if length > f32::EPSILON {
        motion *= random_speed * 0.4 / length;
    }
    motion.y += 0.1;

    // EntityFX.multiplyVelocity(0.2) is used only for hit particles. Its Y
    // component preserves the constructor's +0.1 upward bias.
    if scale < 1.0 {
        motion.x *= 0.2;
        motion.y = (motion.y - 0.1) * 0.2 + 0.1;
        motion.z *= 0.2;
    }

    Particle {
        kind,
        position,
        velocity: motion * 20.0,
        age: 0.0,
        lifetime: max_age_ticks.max(1) as f32 / 20.0,
        // EntityDiggingFX halves EntityFX.particleScale. The renderer's full
        // quad width is 0.2 * particleScale.
        size: 0.2 * particle_scale,
        color: [0.6, 0.6, 0.6, 1.0],
        rotation: 0.0,
        texture_jitter,
        on_ground: false,
    }
}

fn pseudo(seed: f32) -> f32 {
    (seed.sin() * 43758.547).fract().abs()
}

fn gaussian(seed: f32) -> f32 {
    let u1 = pseudo(seed).max(1.0e-6);
    let u2 = pseudo(seed + 19.19);
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

fn server_particle_velocity(
    kind: ParticleKind,
    packet_offset: Vector3<f32>,
    speed: f32,
) -> Vector3<f32> {
    match kind {
        ParticleKind::Note => Vector3::new(0.0, 4.0, 0.0),
        ParticleKind::Heart | ParticleKind::AngryVillager => Vector3::new(0.0, 2.0, 0.0),
        ParticleKind::Redstone
        | ParticleKind::MobSpell
        | ParticleKind::MobSpellAmbient
        | ParticleKind::WitchMagic => Vector3::zeros(),
        _ => packet_offset * speed * 20.0,
    }
}

fn server_particle_color(
    kind: ParticleKind,
    packet_offset: Vector3<f32>,
    fallback: [f32; 4],
) -> [f32; 4] {
    match kind {
        ParticleKind::MobSpell => [packet_offset.x, packet_offset.y, packet_offset.z, 1.0],
        ParticleKind::MobSpellAmbient => [packet_offset.x, packet_offset.y, packet_offset.z, 0.15],
        ParticleKind::Redstone => {
            let red = if packet_offset.x == 0.0 {
                1.0
            } else {
                packet_offset.x
            };
            [red, packet_offset.y, packet_offset.z, 1.0]
        }
        ParticleKind::Note => {
            let note = packet_offset.x;
            [
                (note * std::f32::consts::TAU).sin() * 0.65 + 0.35,
                ((note + 1.0 / 3.0) * std::f32::consts::TAU).sin() * 0.65 + 0.35,
                ((note + 2.0 / 3.0) * std::f32::consts::TAU).sin() * 0.65 + 0.35,
                1.0,
            ]
        }
        _ => fallback,
    }
}

/// Public hash function for deterministic pseudo-random positions.
pub fn particle_pos_hash(seed: f32) -> f32 {
    pseudo(seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_particle_ids_match_minecraft_1_8_9() {
        let expected = [
            ParticleKind::Explosion,
            ParticleKind::HugeExplosion,
            ParticleKind::HugeExplosion,
            ParticleKind::FireworkSpark,
            ParticleKind::Bubble,
            ParticleKind::WaterSplash,
            ParticleKind::WaterWake,
            ParticleKind::Suspended,
            ParticleKind::DepthSuspend,
            ParticleKind::Crit,
            ParticleKind::CritMagic,
            ParticleKind::SmokeNormal,
            ParticleKind::LargeSmoke,
            ParticleKind::Spell,
            ParticleKind::InstantSpell,
            ParticleKind::MobSpell,
            ParticleKind::MobSpellAmbient,
            ParticleKind::WitchMagic,
            ParticleKind::DripWater,
            ParticleKind::DripLava,
            ParticleKind::AngryVillager,
            ParticleKind::HappyVillager,
            ParticleKind::Suspended,
            ParticleKind::Note,
            ParticleKind::Portal,
            ParticleKind::EnchantTable,
            ParticleKind::Flame,
            ParticleKind::LavaPop,
            ParticleKind::Footstep,
            ParticleKind::Cloud,
            ParticleKind::Redstone,
            ParticleKind::Snowball,
            ParticleKind::SnowShovel,
            ParticleKind::Slime,
            ParticleKind::Heart,
            ParticleKind::Barrier,
            ParticleKind::ItemCrack {
                item_id: 5,
                damage: 2,
            },
            ParticleKind::BlockCrack {
                block_state: (5 << 4) | 2,
            },
            ParticleKind::BlockDust {
                block_state: (5 << 4) | 2,
            },
            ParticleKind::WaterDrop,
            ParticleKind::DamageHint,
            ParticleKind::MobAppearance,
        ];

        for (id, expected_kind) in expected.into_iter().enumerate() {
            let mut particles = ParticleSystem::new(4);
            particles.spawn_from_server(
                id as i32,
                Point3::origin(),
                Vector3::zeros(),
                0.0,
                0,
                if id == 36 { &[5, 2] } else { &[5 | (2 << 12)] },
            );
            assert_eq!(
                particles.particles().first().map(|p| p.kind),
                Some(expected_kind)
            );
        }
    }

    #[test]
    fn zero_count_uses_packet_offsets_as_velocity() {
        let mut particles = ParticleSystem::new(4);
        let origin = Point3::new(2.0, 3.0, 4.0);
        let offset = Vector3::new(0.25, -0.5, 1.0);
        particles.spawn_from_server(26, origin, offset, 2.0, 0, &[]);

        let particle = &particles.particles()[0];
        assert_eq!(particle.position, origin);
        assert_eq!(particle.velocity, offset * 40.0);
    }

    #[test]
    fn particle_tick_uses_vanilla_gravity_and_drag() {
        let mut particles = ParticleSystem::new(4);
        let kind = ParticleKind::BlockCrack {
            block_state: 1 << 4,
        };
        let physics = kind.physics();
        particles.spawn(Particle {
            kind,
            position: Point3::origin(),
            velocity: Vector3::zeros(),
            age: 0.0,
            lifetime: 1.0,
            size: physics.base_size,
            color: physics.color,
            rotation: 0.0,
            texture_jitter: [0.0, 0.0],
            on_ground: false,
        });

        particles.tick(0.05);

        let particle = &particles.particles()[0];
        assert!((particle.position.y + 0.04).abs() < 1.0e-6);
        assert!((particle.velocity.y + 0.784).abs() < 1.0e-6);
    }

    #[test]
    fn block_break_uses_vanilla_grid_with_randomized_entity_fx_properties() {
        let mut particles = ParticleSystem::new(128);
        let origin = Point3::new(10.0, 20.0, 30.0);
        let block_state = (5 << 4) | 2;

        particles.spawn_block_break(block_state, origin);

        assert_eq!(particles.particles().len(), 64);
        assert!(particles.particles().iter().all(|particle| {
            particle.kind == (ParticleKind::BlockCrack { block_state })
                && (0.1..0.2).contains(&particle.size)
                && (0.2..=2.0).contains(&particle.lifetime)
                && particle
                    .texture_jitter
                    .iter()
                    .all(|jitter| (0.0..3.0).contains(jitter))
        }));

        let distinct_velocities = particles
            .particles()
            .windows(2)
            .filter(|pair| (pair[0].velocity - pair[1].velocity).norm() > 1.0e-4)
            .count();
        assert!(distinct_velocities > 48);
    }

    #[test]
    fn block_hit_spawns_one_scaled_particle_outside_the_hit_face() {
        let mut particles = ParticleSystem::new(8);
        let origin = Point3::new(4.0, 5.0, 6.0);

        particles.spawn_block_hit(1 << 4, origin, crate::client::physics::BlockFace::North);

        assert_eq!(particles.particles().len(), 1);
        let particle = &particles.particles()[0];
        assert!((particle.position.z - (origin.z - 0.1)).abs() < 1.0e-6);
        assert!((0.06..0.12).contains(&particle.size));
    }
}
