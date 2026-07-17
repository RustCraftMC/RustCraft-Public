use crate::audio::{self, AudioBackend};
use crate::client::particles::ParticleSystem;

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
        1000 => audio.play(sound_with("random.click", 1.0, 1.0, pos)),
        1001 => audio.play(sound_with("random.click", 1.0, 1.2, pos)),
        1002 => audio.play(sound_with("random.bow", 1.0, 1.2, pos)),
        1003 => audio.play(sound_with("random.door_open", 1.0, 1.0, pos)),
        1004 => audio.play(sound_with("random.fizz", 0.5, 2.6, pos)),
        // 1005 starts/stops a record and is handled by the music subsystem.
        1006 => audio.play(sound_with("random.door_close", 1.0, 1.0, pos)),
        1007 => audio.play(sound_with("mob.ghast.charge", 10.0, 1.0, pos)),
        1008 => audio.play(sound_with("mob.ghast.fireball", 10.0, 1.0, pos)),
        1009 => audio.play(sound_with("mob.ghast.fireball", 2.0, 1.0, pos)),
        1010 => audio.play(sound_with("mob.zombie.wood", 2.0, 1.0, pos)),
        1011 => audio.play(sound_with("mob.zombie.metal", 2.0, 1.0, pos)),
        1012 => audio.play(sound_with("mob.zombie.woodbreak", 2.0, 1.0, pos)),
        1014 => audio.play(sound_with("mob.wither.shoot", 2.0, 1.0, pos)),
        1015 => audio.play(sound_with("mob.bat.takeoff", 0.05, 1.0, pos)),
        1016 => audio.play(sound_with("mob.zombie.infect", 2.0, 1.0, pos)),
        1017 => audio.play(sound_with("mob.zombie.unfect", 2.0, 1.0, pos)),
        1020 => audio.play(sound_with("random.anvil_break", 1.0, 1.0, pos)),
        1021 => audio.play(sound_with("random.anvil_use", 1.0, 1.0, pos)),
        1022 => audio.play(sound_with("random.anvil_land", 0.3, 1.0, pos)),
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
            audio.play(sound("random.fizz", audio::SoundCategory::Blocks, pos));
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
                "mob.guardian.curse",
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

#[allow(dead_code)]
fn legacy_block_dig_sound(block: crate::world::block::Block) -> &'static str {
    use crate::world::block::Block;
    match block {
        Block::Stone
        | Block::Cobblestone
        | Block::Bedrock
        | Block::GoldOre
        | Block::IronOre
        | Block::CoalOre
        | Block::LapisOre
        | Block::DiamondOre
        | Block::RedstoneOre
        | Block::LitRedstoneOre
        | Block::Obsidian
        | Block::StoneBricks
        | Block::Bricks
        | Block::MossyCobblestone
        | Block::Sandstone
        | Block::DoubleStoneSlab
        | Block::StoneSlab
        | Block::StoneButton
        | Block::StonePressurePlate
        | Block::IronBlock
        | Block::GoldBlock
        | Block::DiamondBlock
        | Block::LapisBlock
        | Block::NetherBrick
        | Block::Dispenser
        | Block::Furnace
        | Block::LitFurnace
        | Block::MobSpawner
        | Block::Cauldron
        | Block::EnchantingTable
        | Block::BrewingStand
        | Block::IronBars
        | Block::GlassPane
        | Block::PoweredRail
        | Block::DetectorRail
        | Block::Rail
        | Block::StandingSign
        | Block::WallSign
        | Block::MonsterEgg
        | Block::NetherBrickFence
        | Block::NetherBrickStairs
        | Block::StoneBrickStairs
        | Block::BrickStairs
        | Block::CobblestoneStairs
        | Block::IronDoor
        | Block::Chest
        | Block::StickyPiston
        | Block::Piston
        | Block::PistonHead
        | Block::PistonExtension
        | Block::SnowBlock
        | Block::SnowLayer
        | Block::PackedIce
        | Block::Ice
        | Block::Clay
        | Block::QuartzOre => "dig.stone",

        Block::Grass
        | Block::Dirt
        | Block::Farmland
        | Block::Mycelium
        | Block::SoulSand
        | Block::TallGrass
        | Block::DeadBush
        | Block::HayBlock => "dig.grass",

        Block::Planks
        | Block::Log
        | Block::Log2
        | Block::Bookshelf
        | Block::CraftingTable
        | Block::OakDoor
        | Block::Trapdoor
        | Block::OakFence
        | Block::OakStairs
        | Block::WoodenPressurePlate
        | Block::NoteBlock
        | Block::Jukebox
        | Block::Torch
        | Block::UnlitRedstoneTorch
        | Block::RedstoneTorch
        | Block::Ladder
        | Block::Lever
        | Block::WoodenButton
        | Block::SugarCane
        | Block::Cactus
        | Block::Cake
        | Block::UnpoweredRepeater
        | Block::PoweredRepeater
        | Block::Pumpkin
        | Block::JackOLantern
        | Block::MelonBlock
        | Block::Vine
        | Block::LilyPad
        | Block::BrownMushroom
        | Block::RedMushroom
        | Block::NetherWart => "dig.wood",

        Block::Gravel => "dig.gravel",
        Block::Sand => "dig.sand",
        Block::Wool | Block::Cobweb => "dig.cloth",
        Block::Glass | Block::Tnt => "dig.glass",

        _ => "dig.stone",
    }
}
