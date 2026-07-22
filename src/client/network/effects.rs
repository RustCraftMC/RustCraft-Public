use crate::audio::{self, AudioBackend};
use crate::client::particles::ParticleSystem;

/// Sound event names for world effect handling (S28). Extracted as `const` to
/// avoid re-allocating the same `&'static str` on each packet and to make the
/// mapping self-documenting.
const SOUND_RANDOM_CLICK: &str = "random.click";
const SOUND_RANDOM_BOW: &str = "random.bow";
const SOUND_RANDOM_DOOR_OPEN: &str = "random.door_open";
const SOUND_RANDOM_FIZZ: &str = "random.fizz";
const SOUND_RANDOM_DOOR_CLOSE: &str = "random.door_close";
const SOUND_MOB_GHAST_CHARGE: &str = "mob.ghast.charge";
const SOUND_MOB_GHAST_FIREBALL: &str = "mob.ghast.fireball";
const SOUND_MOB_ZOMBIE_WOOD: &str = "mob.zombie.wood";
const SOUND_MOB_ZOMBIE_METAL: &str = "mob.zombie.metal";
const SOUND_MOB_ZOMBIE_WOODBREAK: &str = "mob.zombie.woodbreak";
const SOUND_MOB_WITHER_SHOOT: &str = "mob.wither.shoot";
const SOUND_MOB_BAT_TAKEOFF: &str = "mob.bat.takeoff";
const SOUND_MOB_ZOMBIE_INFECT: &str = "mob.zombie.infect";
const SOUND_MOB_ZOMBIE_UNFECT: &str = "mob.zombie.unfect";
const SOUND_RANDOM_ANVIL_BREAK: &str = "random.anvil_break";
const SOUND_RANDOM_ANVIL_USE: &str = "random.anvil_use";
const SOUND_RANDOM_ANVIL_LAND: &str = "random.anvil_land";
const SOUND_MOB_GUARDIAN_CURSE: &str = "mob.guardian.curse";

pub fn handle_world_effect(
    particles: &mut ParticleSystem,
    audio: &mut impl AudioBackend,
    effect_id: i32,
    pos: [i32; 3],
    data: i32,
) {
    let origin = nalgebra::Point3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32);
    match effect_id {
        // Block break particles + sound
        2001 => {
            // RenderGlobal.playAuxSFX: ID is stored in the low 12 bits and
            // metadata in bits 12..19 (Block.getStateId layout).
            let block_id = (data & 4095).max(0) as u16;
            let metadata = ((data >> 12) & 255).clamp(0, 15) as u16;
            let block_state = (block_id << 4) | metadata;
            let block = crate::world::block::Block::from_id(block_id);
            particles.spawn_block_break(block_state, origin);
            let pitch = 0.8
                + ((pos[0] as u32).wrapping_mul(1234)
                    ^ (pos[1] as u32).wrapping_mul(5678)
                    ^ (pos[2] as u32).wrapping_mul(9012)) as f32
                    / u32::MAX as f32
                    * 0.4;
            audio.play(audio::SoundEvent {
                name: block_dig_sound(block).to_string(),
                category: audio::SoundCategory::Blocks,
                volume: 1.0,
                pitch,
                position: Some([pos[0] as f32, pos[1] as f32, pos[2] as f32]),
            });
        }
        // Piston extend/retract, piston break, dispenser
        2000 | 2002 | 2003 | 2004 => particles.spawn_from_server(
            1,
            origin + nalgebra::Vector3::new(0.5, 0.5, 0.5),
            nalgebra::Vector3::new(0.6, 0.6, 0.6),
            0.18,
            24,
            &[],
        ),
        // Sound effect IDs (1000-1022)
        1000 => audio.play(sound_with(SOUND_RANDOM_CLICK, 1.0, 1.0, pos)),
        1001 => audio.play(sound_with(SOUND_RANDOM_CLICK, 1.0, 1.2, pos)),
        1002 => audio.play(sound_with(SOUND_RANDOM_BOW, 1.0, 1.2, pos)),
        1003 => audio.play(sound_with(SOUND_RANDOM_DOOR_OPEN, 1.0, 1.0, pos)),
        1004 => audio.play(sound_with(SOUND_RANDOM_FIZZ, 0.5, 2.6, pos)),
        // 1005 starts/stops a record and is handled by the music subsystem.
        1006 => audio.play(sound_with(SOUND_RANDOM_DOOR_CLOSE, 1.0, 1.0, pos)),
        1007 => audio.play(sound_with(SOUND_MOB_GHAST_CHARGE, 10.0, 1.0, pos)),
        1008 => audio.play(sound_with(SOUND_MOB_GHAST_FIREBALL, 10.0, 1.0, pos)),
        1009 => audio.play(sound_with(SOUND_MOB_GHAST_FIREBALL, 2.0, 1.0, pos)),
        1010 => audio.play(sound_with(SOUND_MOB_ZOMBIE_WOOD, 2.0, 1.0, pos)),
        1011 => audio.play(sound_with(SOUND_MOB_ZOMBIE_METAL, 2.0, 1.0, pos)),
        1012 => audio.play(sound_with(SOUND_MOB_ZOMBIE_WOODBREAK, 2.0, 1.0, pos)),
        1014 => audio.play(sound_with(SOUND_MOB_WITHER_SHOOT, 2.0, 1.0, pos)),
        1015 => audio.play(sound_with(SOUND_MOB_BAT_TAKEOFF, 0.05, 1.0, pos)),
        1016 => audio.play(sound_with(SOUND_MOB_ZOMBIE_INFECT, 2.0, 1.0, pos)),
        1017 => audio.play(sound_with(SOUND_MOB_ZOMBIE_UNFECT, 2.0, 1.0, pos)),
        1020 => audio.play(sound_with(SOUND_RANDOM_ANVIL_BREAK, 1.0, 1.0, pos)),
        1021 => audio.play(sound_with(SOUND_RANDOM_ANVIL_USE, 1.0, 1.0, pos)),
        1022 => audio.play(sound_with(SOUND_RANDOM_ANVIL_LAND, 0.3, 1.0, pos)),
        // Smoke particles for extinguish fire
        1030 => {
            particles.spawn_from_server(
                2,
                origin + nalgebra::Vector3::new(0.5, 0.5, 0.5),
                nalgebra::Vector3::new(0.5, 0.5, 0.5),
                0.0,
                12,
                &[],
            );
            audio.play(sound(SOUND_RANDOM_FIZZ, audio::SoundCategory::Blocks, pos));
        }
        // Break ender eye
        1031 => {
            particles.spawn_from_server(
                28,
                origin + nalgebra::Vector3::new(0.5, 0.5, 0.5),
                nalgebra::Vector3::new(0.3, 0.3, 0.3),
                0.0,
                12,
                &[],
            );
        }
        // Spawn elder guardian (mob appearance)
        1032 => {
            particles.spawn_mob_appearance(origin);
            audio.play(sound(
                SOUND_MOB_GUARDIAN_CURSE,
                audio::SoundCategory::Hostile,
                pos,
            ));
        }
        // Break block with specific sound
        1033 => {
            particles.spawn_from_server(
                2,
                origin + nalgebra::Vector3::new(0.5, 0.5, 0.5),
                nalgebra::Vector3::new(0.5, 0.5, 0.5),
                0.0,
                12,
                &[],
            );
        }
        // Break block variant
        1034 => {
            particles.spawn_from_server(
                2,
                origin + nalgebra::Vector3::new(0.5, 0.5, 0.5),
                nalgebra::Vector3::new(0.5, 0.5, 0.5),
                0.0,
                12,
                &[],
            );
        }
        _ => {}
    }
}

fn sound(name: &str, category: audio::SoundCategory, pos: [i32; 3]) -> audio::SoundEvent {
    audio::SoundEvent {
        name: name.to_string(),
        category,
        volume: 1.0,
        pitch: 1.0,
        position: Some([pos[0] as f32, pos[1] as f32, pos[2] as f32]),
    }
}

fn sound_with(name: &str, volume: f32, pitch: f32, pos: [i32; 3]) -> audio::SoundEvent {
    audio::SoundEvent {
        name: name.to_string(),
        category: audio::SoundCategory::Blocks,
        volume,
        pitch,
        position: Some([pos[0] as f32, pos[1] as f32, pos[2] as f32]),
    }
}

fn block_dig_sound(block: crate::world::block::Block) -> &'static str {
    block.sound_type().dig_event()
}
