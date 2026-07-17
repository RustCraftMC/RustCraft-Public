use crate::client::particles::{Particle, ParticleKind};
use crate::client::player::Camera;

#[derive(Clone, Copy, Debug)]
pub struct ParticleSprite {
    pub pos: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
    pub tile: usize,
    pub alpha: f32,
}

impl ParticleSprite {
    fn from_particle(particle: &Particle) -> Self {
        let tile = match particle.kind {
            ParticleKind::BlockDust { block_state } | ParticleKind::BlockCrack { block_state } => {
                block_particle_tile(block_state)
            }
            ParticleKind::Rain => crate::assets::texture::tex_idx("environment/rain"),
            _ => 0,
        };
        let life = (particle.age / particle.lifetime).clamp(0.0, 1.0);
        let mut color = particle.color;
        let _physics = particle.kind.physics();
        color[3] *= match particle.kind {
            ParticleKind::Explosion | ParticleKind::HugeExplosion => 1.0 - life,
            ParticleKind::Flame | ParticleKind::LavaPop => (1.0 - life).sqrt(),
            ParticleKind::Portal => (1.0 - life * life) * (0.65 + 0.35 * (life * 20.0).sin().abs()),
            _ => 1.0 - life * life,
        };
        Self {
            pos: [
                particle.position.x,
                particle.position.y,
                particle.position.z,
            ],
            size: match particle.kind {
                ParticleKind::Explosion
                | ParticleKind::HugeExplosion
                | ParticleKind::LargeSmoke => particle.size * (1.0 + life),
                ParticleKind::Heart
                | ParticleKind::Note
                | ParticleKind::AngryVillager
                | ParticleKind::HappyVillager => particle.size * (1.0 + life * 0.35),
                ParticleKind::Portal => particle.size * (1.25 - life * 0.35),
                _ => particle.size,
            },
            color,
            tile,
            alpha: color[3],
        }
    }
}

pub fn sprites_from_particles(particles: &[Particle]) -> Vec<ParticleSprite> {
    particles
        .iter()
        .map(ParticleSprite::from_particle)
        .collect()
}

pub fn draw_particle_sprites(
    builder: &mut super::gui::GuiVertexBuilder,
    camera: &Camera,
    particles: &[ParticleSprite],
    screen_w: f32,
    screen_h: f32,
) {
    let view = camera.view_matrix();
    let proj = camera.projection_matrix();
    let view_proj = proj * view;

    for particle in particles {
        if particle.alpha <= 0.01 {
            continue;
        }
        let world = nalgebra::Vector4::new(particle.pos[0], particle.pos[1], particle.pos[2], 1.0);
        let clip = view_proj * world;
        if clip.w <= 0.01 || clip.z < 0.0 || clip.z > clip.w {
            continue;
        }

        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        let sx = (ndc_x * 0.5 + 0.5) * screen_w;
        let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * screen_h;
        let pixel_size = (screen_h * particle.size / clip.w).clamp(2.0, 18.0);
        let (uv_min, uv_max) = tile_uv(particle.tile);
        let color = particle.color;
        builder.add_quad(
            sx - pixel_size * 0.5,
            sy - pixel_size * 0.5,
            pixel_size,
            pixel_size,
            uv_min[0],
            uv_min[1],
            uv_max[0] - uv_min[0],
            uv_max[1] - uv_min[1],
            color,
        );
    }
}

pub fn project_world_to_screen(
    camera: &Camera,
    world_pos: [f32; 3],
    screen_w: f32,
    screen_h: f32,
) -> Option<([f32; 2], f32)> {
    let view = camera.view_matrix();
    let proj = camera.projection_matrix();
    let view_proj = proj * view;
    let world = nalgebra::Vector4::new(world_pos[0], world_pos[1], world_pos[2], 1.0);
    let clip = view_proj * world;
    if clip.w <= 0.01 || clip.z < 0.0 || clip.z > clip.w {
        return None;
    }

    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    let sx = (ndc_x * 0.5 + 0.5) * screen_w;
    let sy = (ndc_y * 0.5 + 0.5) * screen_h;
    Some(([sx, sy], clip.w))
}

fn block_particle_tile(block_state: u16) -> usize {
    crate::world::block::Block::from_state(block_state)
        .tiles()
        .2
}

fn tile_uv(tile: usize) -> ([f32; 2], [f32; 2]) {
    crate::assets::texture::tile_uv(tile)
}
