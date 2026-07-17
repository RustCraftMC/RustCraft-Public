//! 3D particle mesh system — generates world-space camera-facing billboard quads.
//!
//! All particles are rendered as quads that always face the player, similar to
//! vanilla MC's particle rendering. Block-crack/block-dust particles use the
//! block texture from the main atlas; special particles (flame, smoke, heart,
//! etc.) use sub-sprites from particles.png that are loaded into the atlas as
//! `particle_N` tiles.
//!
//! Mesh is generated per-frame and uploaded to the GPU, then drawn in the
//! transparent world pass (alpha blending, no depth write).

use nalgebra::Vector3;

use crate::render::entity::mesh::EntityVertex;

/// Vanilla MC particle sprite indices in the particles.png 16x16 grid.
mod sprite {
    pub const BUBBLE: u32 = 32;
    pub const FLAME: u32 = 48;
    pub const LAVA: u32 = 49;
    pub const NOTE: u32 = 64;
    pub const CRIT: u32 = 65;
    pub const CRIT_MAGIC: u32 = 66;
    pub const HEART: u32 = 80;
    pub const ANGRY_VILLAGER: u32 = 81;
    pub const HAPPY_VILLAGER: u32 = 82;
    pub const DRIP_LAVA: u32 = 112;
    pub const DRIP_WATER: u32 = 113;
    pub const SPELL: u32 = 128;
    pub const SPELL_INSTANT: u32 = 144;
    pub const FIREWORK: u32 = 160;
    pub const ENCHANTMENT: u32 = 225;
}

use crate::client::particles::{Particle, ParticleKind};

/// Build a particle mesh: returns (vertices, indices) ready for GPU upload.
/// `tile_lookup` maps a texture name to atlas tile index — use tex_idx() for
/// block textures and `tex_idx(&format!("particle_{}", n))` for particle sprites.
pub fn build_particle_mesh(
    particles: &[Particle],
    camera_right: Vector3<f32>,
    camera_up: Vector3<f32>,
    camera_front: Vector3<f32>,
    tile_lookup: impl Fn(&str) -> usize,
) -> (Vec<EntityVertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(particles.len() * 4);
    let mut indices = Vec::with_capacity(particles.len() * 6);

    for p in particles {
        let life = (p.age / p.lifetime).clamp(0.0, 1.0);

        let alpha = compute_alpha(p.kind, life, p.color[3]);
        if alpha <= 0.01 {
            continue;
        }

        let (sprite_idx, mut color, size_mult, emissive) = particle_visual(p);
        color[3] = alpha;
        let block_type = p.kind;

        let size = compute_size(p.kind, p.size, life) * size_mult;

        if size <= 0.001 {
            continue;
        }

        let hs = size * 0.5;
        let (uv0, uv1) = match block_type {
            ParticleKind::BlockCrack { block_state } | ParticleKind::BlockDust { block_state } => {
                let tile = block_particle_tile(block_state);
                block_particle_uv(p, tile)
            }
            ParticleKind::HugeExplosion => {
                let frame = ((life * 15.0).floor() as u32).min(15);
                let tile_name = format!("explosion_{}", frame);
                crate::assets::texture::tile_uv(tile_lookup(&tile_name))
            }
            ParticleKind::Rain => crate::assets::texture::tile_uv(tile_lookup("environment/rain")),
            _ => {
                let tile_name = format!("particle_{}", sprite_idx);
                let actual_tile = tile_lookup(&tile_name);
                crate::assets::texture::tile_uv(actual_tile)
            }
        };

        let (sin_rotation, cos_rotation) = p.rotation.sin_cos();
        let right = (camera_right * cos_rotation + camera_up * sin_rotation) * hs;
        let up = (camera_up * cos_rotation - camera_right * sin_rotation) * hs;
        let center = p.position.coords;
        let normal = if emissive {
            [0.0, 0.0, 0.0]
        } else {
            [-camera_front.x, -camera_front.y, -camera_front.z]
        };
        let corners = [
            center - right - up,
            center + right - up,
            center + right + up,
            center - right + up,
        ];
        let base = vertices.len() as u32;
        for (corner, uv) in corners.into_iter().zip([
            [uv0[0], uv1[1]],
            [uv1[0], uv1[1]],
            [uv1[0], uv0[1]],
            [uv0[0], uv0[1]],
        ]) {
            vertices.push(EntityVertex {
                position: corner.into(),
                normal,
                uv,
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    (vertices, indices)
}

/// Compute alpha fade over lifetime (MCP-matching curves).
fn compute_alpha(kind: ParticleKind, life: f32, base_alpha: f32) -> f32 {
    let fade = match kind {
        ParticleKind::FireworkSpark => (1.0 - life).clamp(0.0, 1.0),
        ParticleKind::Footstep => (0.2 * (1.0 - life)).clamp(0.0, 0.2),
        _ => 1.0,
    };
    base_alpha * fade
}

/// Compute size over lifetime.
fn compute_size(kind: ParticleKind, base_size: f32, life: f32) -> f32 {
    let physics = kind.physics();
    let scale = physics.scale_over_life;
    match kind {
        ParticleKind::Smoke
        | ParticleKind::LargeSmoke
        | ParticleKind::SmokeNormal
        | ParticleKind::SmokePoof
        | ParticleKind::Explosion
        | ParticleKind::Cloud
        | ParticleKind::Crit
        | ParticleKind::CritMagic
        | ParticleKind::Magic
        | ParticleKind::Spell
        | ParticleKind::MobSpell
        | ParticleKind::MobSpellAmbient
        | ParticleKind::InstantSpell
        | ParticleKind::WitchMagic
        | ParticleKind::Redstone
        | ParticleKind::Heart
        | ParticleKind::Note
        | ParticleKind::AngryVillager
        | ParticleKind::HappyVillager => base_size * (life * 32.0).clamp(0.0, 1.0),
        ParticleKind::Portal => base_size * (1.0 - (1.0 - life) * (1.0 - life)),
        ParticleKind::Flame => base_size * (1.0 - life * life * 0.5),
        ParticleKind::HugeExplosion => base_size,
        _ if (scale - 1.0).abs() > 0.01 => base_size * (1.0 + life * (scale - 1.0)),
        _ => base_size,
    }
}

/// Determine the visual parameters for a particle.
/// Returns (sprite_index, color, size_multiplier, emissive)
fn particle_visual(p: &Particle) -> (u32, [f32; 4], f32, bool) {
    let color = p.color;
    let reverse_frame = (7_i32 - (p.age / p.lifetime * 8.0).floor() as i32).clamp(0, 7) as u32;
    let stable_variant = ((p.position.x * 31.0 + p.position.y * 17.0 + p.position.z * 13.0)
        .sin()
        .abs()
        * 8.0) as u32
        % 8;
    match p.kind {
        ParticleKind::BlockCrack { .. }
        | ParticleKind::BlockDust { .. }
        | ParticleKind::ItemCrack { .. } => (0, color, 1.0, false),
        ParticleKind::Smoke
        | ParticleKind::LargeSmoke
        | ParticleKind::SmokeNormal
        | ParticleKind::SmokePoof
        | ParticleKind::Explosion
        | ParticleKind::Cloud
        | ParticleKind::Redstone
        | ParticleKind::SnowShovel => (reverse_frame, color, 1.0, false),
        ParticleKind::Flame => (sprite::FLAME, color, 1.0, true),
        ParticleKind::Crit => (sprite::CRIT, color, 1.0, false),
        ParticleKind::CritMagic | ParticleKind::Magic => (sprite::CRIT_MAGIC, color, 1.0, false),
        ParticleKind::Spell | ParticleKind::MobSpell | ParticleKind::MobSpellAmbient => {
            (sprite::SPELL + reverse_frame, color, 1.0, false)
        }
        ParticleKind::InstantSpell | ParticleKind::WitchMagic => {
            (sprite::SPELL_INSTANT + reverse_frame, color, 1.0, false)
        }
        ParticleKind::Heart => (sprite::HEART, color, 1.0, false),
        ParticleKind::Note => (sprite::NOTE, color, 1.0, false),
        ParticleKind::Portal => (stable_variant, color, 1.0, true),
        ParticleKind::WaterDrop | ParticleKind::WaterSplash | ParticleKind::WaterWake => {
            (19 + reverse_frame.min(3), color, 1.0, false)
        }
        ParticleKind::Bubble => (sprite::BUBBLE, color, 1.0, false),
        ParticleKind::LavaPop => (sprite::LAVA, color, 1.0, true),
        ParticleKind::Snowball | ParticleKind::Slime => (0, color, 1.0, false),
        ParticleKind::HugeExplosion => (0, color, 1.0, true),
        ParticleKind::EnchantTable => (sprite::ENCHANTMENT + stable_variant * 3, color, 1.0, true),
        ParticleKind::EndRod => (sprite::FIREWORK + reverse_frame, color, 1.0, true),
        ParticleKind::FireworkSpark => (sprite::FIREWORK + reverse_frame, color, 1.0, true),
        ParticleKind::Footstep => (0, color, 1.0, false),
        ParticleKind::Suspended | ParticleKind::DepthSuspend => (0, color, 1.0, false),
        ParticleKind::DripWater => (sprite::DRIP_WATER, color, 1.0, false),
        ParticleKind::DripLava => (sprite::DRIP_LAVA, color, 1.0, true),
        ParticleKind::AngryVillager => (sprite::ANGRY_VILLAGER, color, 1.0, false),
        ParticleKind::HappyVillager => (sprite::HAPPY_VILLAGER, color, 1.0, false),
        ParticleKind::Barrier | ParticleKind::MobAppearance | ParticleKind::DamageHint => {
            (0, color, 1.0, false)
        }
        ParticleKind::Fountain => (19 + reverse_frame.min(3), color, 1.0, false),
        ParticleKind::DeathSmoke => (reverse_frame, color, 1.0, false),
        ParticleKind::DeathFall => (sprite::CRIT, color, 1.0, false),
        ParticleKind::Rain => (0, color, 1.0, false),
    }
}

fn block_particle_uv(particle: &Particle, tile: usize) -> ([f32; 2], [f32; 2]) {
    let (tile_min, tile_max) = crate::assets::texture::tile_uv(tile);
    let cell_u = (tile_max[0] - tile_min[0]) * 0.25;
    let cell_v = (tile_max[1] - tile_min[1]) * 0.25;
    let min = [
        tile_min[0] + particle.texture_jitter[0] * cell_u,
        tile_min[1] + particle.texture_jitter[1] * cell_v,
    ];
    (min, [min[0] + cell_u, min[1] + cell_v])
}

fn block_particle_tile(block_state: u16) -> usize {
    let block_id = block_state >> 4;
    let metadata = (block_state & 0x0f) as u8;
    if crate::world::block_models::BlockModelCache::is_available() {
        let cache = crate::world::block_models::BlockModelCache::global();
        if let Some(face) = cache
            .get_model(block_id, metadata)
            .and_then(|model| model.faces.first())
        {
            return cache.texture_index(&face.texture);
        }
    }
    crate::world::block::Block::from_state(block_state)
        .tiles()
        .2
}
