use super::App;
use crate::audio::{self, AudioBackend};
use crate::client;
use crate::client::keybind::Action;
use crate::entity::EntityType;
use crate::world;
use crate::world::block::Block;

fn block_is_replaceable_for_placement(block: Block, state: u16) -> bool {
    matches!(
        block,
        Block::Air
            | Block::TallGrass
            | Block::DeadBush
            | Block::LargeFlower
            | Block::Vine
            | Block::Fire
    ) || (block == Block::SnowLayer && state & 0x07 == 0)
}

/// `ItemSnow.onItemUse` stacks a layer in place before falling back to the
/// normal `ItemBlock` placement path. A full eight-layer block falls through
/// and is therefore placed on top instead.
fn stacked_snow_layer_state(
    block: Block,
    state: u16,
    face: client::physics::BlockFace,
) -> Option<u16> {
    if block != Block::SnowLayer
        || (face != client::physics::BlockFace::Top
            && !block_is_replaceable_for_placement(block, state))
    {
        return None;
    }
    let metadata = state & 0x07;
    (metadata < 7).then_some((state & !0x07) | (metadata + 1))
}

fn block_may_consume_activation(block: Block) -> bool {
    matches!(
        block,
        Block::Anvil
            | Block::Beacon
            | Block::Bed
            | Block::BrewingStand
            | Block::StoneButton
            | Block::WoodenButton
            | Block::Cake
            | Block::Cauldron
            | Block::Chest
            | Block::TrappedChest
            | Block::CommandBlock
            | Block::DaylightDetector
            | Block::Dispenser
            | Block::Dropper
            | Block::OakDoor
            | Block::SpruceDoor
            | Block::BirchDoor
            | Block::JungleDoor
            | Block::AcaciaDoor
            | Block::DarkOakDoor
            | Block::DragonEgg
            | Block::EnchantingTable
            | Block::EnderChest
            | Block::OakFence
            | Block::SpruceFence
            | Block::BirchFence
            | Block::JungleFence
            | Block::DarkOakFence
            | Block::AcaciaFence
            | Block::OakFenceGate
            | Block::SpruceFenceGate
            | Block::BirchFenceGate
            | Block::JungleFenceGate
            | Block::DarkOakFenceGate
            | Block::AcaciaFenceGate
            | Block::FlowerPot
            | Block::Furnace
            | Block::LitFurnace
            | Block::Hopper
            | Block::Jukebox
            | Block::Lever
            | Block::NoteBlock
            | Block::UnpoweredComparator
            | Block::PoweredComparator
            | Block::UnpoweredRepeater
            | Block::PoweredRepeater
            | Block::StandingSign
            | Block::WallSign
            | Block::Trapdoor
            | Block::IronTrapdoor
            | Block::CraftingTable
    )
}

fn entity_prevents_block_placement(entity_type: EntityType) -> bool {
    entity_type.is_mob()
        || matches!(
            entity_type,
            EntityType::Player
                | EntityType::Boat
                | EntityType::MinecartEmpty
                | EntityType::MinecartChest
                | EntityType::MinecartFurnace
                | EntityType::MinecartTNT
                | EntityType::MinecartSpawner
                | EntityType::MinecartHopper
                | EntityType::MinecartCommand
                | EntityType::PrimedTnt
                | EntityType::FallingBlock
        )
}

/// Vanilla `EntityLivingBase.getHorizontalFacing` quadrant from the MC
/// protocol yaw in degrees (0 = south, clockwise from above).
/// The order is S-W-N-E, matching `EnumFacing.getHorizontal`.
fn horizontal_facing_quadrant(mc_yaw_degrees: f32) -> usize {
    ((mc_yaw_degrees * 4.0 / 360.0 + 0.5).floor() as i32 & 3) as usize
}

/// Vanilla `BlockSlab`/`BlockStairs.onBlockPlaced` half selection: the upper
/// half is chosen when clicking the bottom face, or a side face above its
/// vertical midpoint.
fn placement_selects_upper_half(face: client::physics::BlockFace, hit_y: f32) -> bool {
    match face {
        client::physics::BlockFace::Bottom => true,
        client::physics::BlockFace::Top => false,
        _ => hit_y > 0.5,
    }
}

/// C08 stores each cursor component as `(int)(value * 16)`. Placement
/// prediction must use that same quantized value, otherwise a hit just above
/// 0.5 can predict an upper stair while the server receives exactly 0.5.
fn protocol_hit_fraction(value: f32) -> f32 {
    ((value.clamp(0.0, 1.0) * 16.0) as u8) as f32 / 16.0
}

fn predicted_item_block_state(
    block: Block,
    item_id: u16,
    damage: u16,
    face: client::physics::BlockFace,
    mc_yaw_degrees: f32,
    hit_y: f32,
) -> u16 {
    let mut metadata = (damage & 0x0f) as u8;
    let quadrant = horizontal_facing_quadrant(mc_yaw_degrees);
    if matches!(
        block,
        Block::Log | Block::Log2 | Block::Log3 | Block::HayBlock
    ) {
        metadata = (metadata & 0x03)
            | match face {
                client::physics::BlockFace::East | client::physics::BlockFace::West => 0x04,
                client::physics::BlockFace::North | client::physics::BlockFace::South => 0x08,
                _ => 0,
            };
    } else if matches!(block, Block::StoneSlab | Block::WoodSlab) {
        metadata &= 0x07;
        if placement_selects_upper_half(face, hit_y) {
            metadata |= 0x08;
        }
    } else if matches!(
        block,
        Block::OakStairs
            | Block::SpruceStairs
            | Block::BirchStairs
            | Block::JungleStairs
            | Block::AcaciaStairs
            | Block::DarkOakStairs
            | Block::CobblestoneStairs
            | Block::BrickStairs
            | Block::StoneBrickStairs
            | Block::NetherBrickStairs
            | Block::SandstoneStairs
            | Block::QuartzStairs
            | Block::RedSandstoneStairs
    ) {
        // BlockStairs.onBlockPlaced: FACING = placer.getHorizontalFacing(),
        // meta = 5 - facing.getIndex() (S=2, W=1, N=3, E=0).
        metadata = [2, 1, 3, 0][quadrant];
        if placement_selects_upper_half(face, hit_y) {
            metadata |= 0x04;
        }
    } else if matches!(
        block,
        Block::Furnace | Block::LitFurnace | Block::Chest | Block::TrappedChest | Block::EnderChest
    ) {
        // BlockFurnace/BlockChest.onBlockPlaced: FACING = opposite of the
        // placer's horizontal facing, meta = facing.getIndex()
        // (S→N=2, W→E=5, N→S=3, E→W=4).
        metadata = [2, 5, 3, 4][quadrant];
    } else if matches!(block, Block::Pumpkin | Block::JackOLantern) {
        // BlockPumpkin.onBlockPlaced: FACING = opposite horizontal facing,
        // meta = facing.getHorizontalIndex() (S→N=2, W→E=3, N→S=0, E→W=1).
        metadata = [2, 3, 0, 1][quadrant];
    } else if block == Block::Ladder {
        // BlockLadder.onBlockPlaced: FACING = the clicked horizontal face,
        // meta = facing.getIndex(). Non-horizontal clicks fall back to the
        // vanilla neighbour search, which the server resolves.
        metadata = match face {
            client::physics::BlockFace::North => 2,
            client::physics::BlockFace::South => 3,
            client::physics::BlockFace::West => 4,
            client::physics::BlockFace::East => 5,
            _ => metadata,
        };
    } else if matches!(
        block,
        Block::Torch | Block::RedstoneTorch | Block::UnlitRedstoneTorch
    ) {
        // BlockTorch.onBlockPlaced: FACING = the clicked face when the torch
        // can attach there (meta E=1, W=2, S=3, N=4, standing=5).
        metadata = match face {
            client::physics::BlockFace::East => 1,
            client::physics::BlockFace::West => 2,
            client::physics::BlockFace::South => 3,
            client::physics::BlockFace::North => 4,
            client::physics::BlockFace::Top => 5,
            client::physics::BlockFace::Bottom => metadata,
        };
    } else if block == Block::Lever {
        // BlockLever.onBlockPlaced: FACING = EnumOrientation.forFacings(face,
        // playerHorizontalFacing). EnumOrientation metadata:
        //   DOWN_X=0, EAST=1, WEST=2, SOUTH=3, NORTH=4, UP_Z=5, UP_X=6, DOWN_Z=7
        metadata = match face {
            client::physics::BlockFace::East => 1,
            client::physics::BlockFace::West => 2,
            client::physics::BlockFace::South => 3,
            client::physics::BlockFace::North => 4,
            client::physics::BlockFace::Top => {
                // UP_Z if player faces Z axis (S/N), UP_X if X axis (W/E)
                if quadrant == 0 || quadrant == 2 {
                    5
                } else {
                    6
                }
            }
            client::physics::BlockFace::Bottom => {
                // DOWN_Z if player faces Z axis (S/N), DOWN_X if X axis (W/E)
                if quadrant == 0 || quadrant == 2 {
                    7
                } else {
                    0
                }
            }
        };
    } else if matches!(block, Block::StoneButton | Block::WoodenButton) {
        // BlockButton.onBlockPlaced: FACING = clicked face.
        // Button meta: DOWN=0, EAST=1, WEST=2, SOUTH=3, NORTH=4, UP=5
        metadata = match face {
            client::physics::BlockFace::Bottom => 0,
            client::physics::BlockFace::East => 1,
            client::physics::BlockFace::West => 2,
            client::physics::BlockFace::South => 3,
            client::physics::BlockFace::North => 4,
            client::physics::BlockFace::Top => 5,
        };
    } else if matches!(block, Block::Trapdoor | Block::IronTrapdoor) {
        // BlockTrapDoor.onBlockPlaced: FACING = horizontal face,
        // OPEN=false, HALF = hitY > 0.5 ? TOP : BOTTOM.
        // Trapdoor meta: N=0, S=1, W=2, E=3, bit2=open, bit3=half
        metadata = match face {
            client::physics::BlockFace::North => 0,
            client::physics::BlockFace::South => 1,
            client::physics::BlockFace::West => 2,
            client::physics::BlockFace::East => 3,
            _ => 0,
        };
        if hit_y > 0.5 {
            metadata |= 0x08;
        }
    } else if matches!(
        block,
        Block::OakFenceGate
            | Block::SpruceFenceGate
            | Block::BirchFenceGate
            | Block::JungleFenceGate
            | Block::AcaciaFenceGate
            | Block::DarkOakFenceGate
    ) {
        // BlockFenceGate.onBlockPlaced: FACING = placer.getHorizontalFacing()
        // (NOT .getOpposite()), OPEN=false, POWERED=false.
        // Meta: S=0, W=1, N=2, E=3
        metadata = quadrant as u8;
    } else if matches!(block, Block::UnpoweredRepeater | Block::PoweredRepeater) {
        // BlockRedstoneRepeater.onBlockPlaced: FACING = opposite player facing,
        // DELAY from damage, LOCKED=false.
        // Meta: S=0, W=1, N=2, E=3, bits 2-3 = delay-1
        metadata = [2, 3, 0, 1][quadrant] | ((damage.min(3) as u8) << 2);
    } else if matches!(block, Block::UnpoweredComparator | Block::PoweredComparator) {
        // BlockRedstoneComparator.onBlockPlaced: FACING = opposite player facing,
        // MODE=COMPARE, POWERED=false.
        // Meta: S=0, W=1, N=2, E=3
        metadata = [2, 3, 0, 1][quadrant];
    } else if block == Block::Anvil {
        // BlockAnvil.onBlockPlaced: FACING = player facing.rotateY() (90° CW),
        // DAMAGE from item damage bits 2-3.
        // Meta: S=0, W=1, N=2, E=3, bits 2-3 = damage level
        metadata = [2, 3, 0, 1][quadrant] | ((damage.min(3) as u8 & 0x03) << 2);
    } else if block == Block::EndPortalFrame {
        // BlockEndPortalFrame.onBlockPlaced: FACING = opposite player facing,
        // EYE=false.
        // Meta: S=0, W=1, N=2, E=3, bit 2 = eye
        metadata = [2, 3, 0, 1][quadrant];
    } else if block == Block::Bed {
        // ItemBed places foot block: FACING = player horizontal facing (same
        // direction as pumpkin/fence gate convention? No — bed uses
        // getHorizontalFacing().getOpposite()?).
        // Actually ItemBed sets FACING = placer.getHorizontalFacing().getOpposite().
        // Meta: S=0, W=1, N=2, E=3, bit 3 = part (0=foot)
        metadata = [2, 3, 0, 1][quadrant];
    } else if block == Block::TripwireHook {
        // BlockTripWireHook.onBlockPlaced: FACING = face if horizontal,
        // ATTACHED=false, POWERED=false.
        // Meta: S=0, W=1, N=2, E=3
        metadata = match face {
            client::physics::BlockFace::South => 0,
            client::physics::BlockFace::West => 1,
            client::physics::BlockFace::North => 2,
            client::physics::BlockFace::East => 3,
            _ => 2, // default NORTH
        };
    } else if block == Block::Cocoa {
        // BlockCocoa.onBlockPlaced: FACING = face.getOpposite() if horizontal,
        // AGE=0.
        // Meta: S=0, W=1, N=2, E=3, bits 2-3 = age
        metadata = match face {
            client::physics::BlockFace::South => 2, // face is S, so placed against S, cocoa faces N
            client::physics::BlockFace::North => 0, // face is N, so placed against N, cocoa faces S
            client::physics::BlockFace::East => 1,  // face is E, cocoa faces W
            client::physics::BlockFace::West => 3,  // face is W, cocoa faces E
            _ => 2,
        };
    } else if block == Block::StandingSign {
        // ItemSign: rotation = floor(yaw * 16 / 360 + 0.5) & 15
        metadata = ((mc_yaw_degrees / 22.5 + 0.5).floor() as i32 & 15) as u8;
    } else if block == Block::WallSign {
        // BlockWallSign: FACING = face index (2=N,3=S,4=W,5=E).
        metadata = match face {
            client::physics::BlockFace::North => 2,
            client::physics::BlockFace::South => 3,
            client::physics::BlockFace::West => 4,
            client::physics::BlockFace::East => 5,
            _ => 2,
        };
    } else if block == Block::StandingBanner {
        // Same as standing sign: rotation = floor(yaw * 16 / 360 + 0.5) & 15
        metadata = ((mc_yaw_degrees / 22.5 + 0.5).floor() as i32 & 15) as u8;
    } else if block == Block::WallBanner {
        // Same encoding as wall sign: 2=N,3=S,4=W,5=E
        metadata = match face {
            client::physics::BlockFace::North => 2,
            client::physics::BlockFace::South => 3,
            client::physics::BlockFace::West => 4,
            client::physics::BlockFace::East => 5,
            _ => 2,
        };
    } else if block == Block::Skull {
        // Floor skull: FACING = UP (1), rotation stored in TileEntity.
        // Wall skull: FACING = face index.
        metadata = match face {
            client::physics::BlockFace::North => 2,
            client::physics::BlockFace::South => 3,
            client::physics::BlockFace::West => 4,
            client::physics::BlockFace::East => 5,
            _ => 1, // UP = floor skull
        };
    } else if matches!(
        block,
        Block::Carpet
            | Block::Wool
            | Block::StainedClay
            | Block::StainedGlass
            | Block::StainedGlassPane
    ) {
        // Color comes directly from item damage.
        metadata = (damage & 0x0f) as u8;
    }
    (item_id << 4) | u16::from(metadata)
}

/// `BlockPistonBase.getFacingFromEntity`, also used by dispensers and droppers.
fn predicted_entity_facing_state(
    block: Block,
    item_id: u16,
    placement_pos: (i32, i32, i32),
    player_pos: nalgebra::Point3<f64>,
    eye_y: f64,
    mc_yaw_degrees: f32,
) -> u16 {
    let (x, y, z) = placement_pos;
    let metadata = if (player_pos.x as f32 - x as f32).abs() < 2.0
        && (player_pos.z as f32 - z as f32).abs() < 2.0
    {
        if eye_y - f64::from(y) > 2.0 {
            1 // UP
        } else if f64::from(y) - eye_y > 0.0 {
            0 // DOWN
        } else {
            [2, 5, 3, 4][horizontal_facing_quadrant(mc_yaw_degrees)]
        }
    } else {
        [2, 5, 3, 4][horizontal_facing_quadrant(mc_yaw_degrees)]
    };
    (item_id << 4) | metadata
}

/// Shared vanilla `Block.SoundType` lookup used by every local block action.
fn block_dig_sound(block: Block) -> &'static str {
    block.sound_type().dig_event()
}

fn block_step_sound(block: Block) -> &'static str {
    block.sound_type().step_event()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ItemUseTickPlan {
    release: bool,
    queued_presses: u32,
    held_repeat: bool,
    discard_attacks: bool,
}

fn plan_item_use_tick(
    active: bool,
    held: bool,
    queued_presses: u32,
    repeat_delay: u8,
    repeat_ready: bool,
    hitting_block: bool,
) -> ItemUseTickPlan {
    if active {
        // Minecraft.runTick chooses the using-item branch once. It may release
        // the current use, but always drains queued presses without restarting.
        return ItemUseTickPlan {
            release: !held,
            discard_attacks: true,
            ..ItemUseTickPlan::default()
        };
    }

    // Minecraft.rightClickMouse returns before applying its delay or sending
    // C08 while PlayerControllerMP is actively damaging a block. Queued
    // presses are still consumed by runTick, and a held press may retry later.
    if hitting_block {
        return ItemUseTickPlan::default();
    }

    if queued_presses > 0 {
        // KeyBinding.isPressed bypasses rightClickDelayTimer. The outer
        // non-using branch is not re-evaluated after the first press starts use.
        return ItemUseTickPlan {
            queued_presses,
            ..ItemUseTickPlan::default()
        };
    }

    ItemUseTickPlan {
        held_repeat: held && repeat_delay == 0 && repeat_ready,
        ..ItemUseTickPlan::default()
    }
}

impl App {
    /// Consume right-click input at the start of a fixed tick. C08/C07 must be
    /// immediately followed by that tick's C03-family packet; sending them from
    /// render-frame callbacks leaves them after the previous movement packet
    /// and triggers Grim's Post check.
    pub(super) fn tick_item_use_input(&mut self) {
        // Minecraft.runTick decrements rightClickDelayTimer before consuming
        // queued mouse/key presses.
        self.item_place_delay = self.item_place_delay.saturating_sub(1);

        let queued_presses = std::mem::take(&mut self.use_presses_pending);
        let held = self.inventory.selected_item();
        let held_item_id = held.item_id;
        let held_item_damage = held.damage;
        let repeat_ready = self.food_cooldown == 0
            || (!is_food(held_item_id) && !is_drinkable(held_item_id, held_item_damage));
        let plan = plan_item_use_tick(
            self.item_use_active,
            self.use_held,
            queued_presses,
            self.item_place_delay,
            repeat_ready,
            self.dig.active_pos().is_some(),
        );

        // Minecraft.runTick drains attack presses without dispatching them
        // when the player was already using an item at the start of the tick.
        if plan.discard_attacks {
            self.pending_attacks = 0;
        }
        if plan.release {
            self.release_item_use();
            return;
        }
        for _ in 0..plan.queued_presses {
            self.use_item();
        }
        if plan.held_repeat {
            self.use_item();
        }
    }

    pub(super) fn tick_block_interaction(&mut self) -> Vec<crate::world::mesh::ChunkMesh> {
        self.flush_pending_dig_cancel();
        if self.item_use_active {
            // Minecraft.sendClickBlockToController requires !isUsingItem for
            // every use action, including food, drink, bow and sword block.
            // Cancel an old dig but emit no C0A or new digging packets.
            self.cancel_digging_now();
            return Vec::new();
        }
        if self.inventory_open || self.chat_open {
            self.cancel_digging_now();
            return Vec::new();
        }

        let eye = self.player.eye_position();
        let eye = nalgebra::Point3::new(eye.x as f32, eye.y as f32, eye.z as f32);
        let hit =
            client::interaction::target_hit_from(&self.world, eye, self.player.camera.front, 4.5);
        let old_block = hit
            .map(|hit| self.world.get_block(hit.pos.0, hit.pos.1, hit.pos.2))
            .unwrap_or(world::block::Block::Air);

        // Minecraft.sendClickBlockToController only calls
        // onPlayerDamageBlock for a held attack aimed at a non-air block.
        // A completed sustained break then consumes five full ticks in
        // blockHitDelay; no new START or damage progress is emitted then.
        if !self.attack_held || hit.is_none() || old_block == world::block::Block::Air {
            self.cancel_digging_now();
            return Vec::new();
        }
        // PlayerControllerMP.clickBlock handles a new creative click before
        // onPlayerDamageBlock, so it must not be delayed by the five ticks
        // left by the previous creative break. The delay applies only to the
        // sustained-held path in onPlayerDamageBlock.
        let creative_click =
            self.session.gamemode == 1 && self.input_ctrl.input.is_just_pressed(Action::Attack);
        if self.block_hit_delay > 0 && !creative_click {
            self.block_hit_delay -= 1;
            let hit = hit.expect("validated above");
            self.spawn_block_hit_feedback(hit);
            client::network::send_animation(&self.net_ctrl.connection);
            return Vec::new();
        }

        if self.session.gamemode == 1 && self.attack_held {
            // MCP PlayerControllerMP.onPlayerDestroyBlock: creative players
            // cannot destroy blocks with a sword. Do not optimistically clear
            // the local world, because the server correctly rejects that dig
            // and the client would be left with a ghost block.
            if is_sword(self.inventory.selected_item().item_id) {
                self.cancel_digging_now();
                return Vec::new();
            }
            if old_block.properties().hardness < 0.0 {
                self.cancel_digging_now();
                return Vec::new();
            }
            if let Some(hit) = hit {
                client::network::send_animation(&self.net_ctrl.connection);
                client::network::send_digging_start(&self.net_ctrl.connection, hit.pos, hit.face);
                return self.finish_digging(hit, false);
            }
        }

        let per_tick_progress = self.block_dig_progress_per_tick(old_block);
        let held_item = self
            .inventory
            .selected_item()
            .view_with_meta(Some(&self.inventory.slot_meta[self.inventory.selected]));
        let update = self.dig.tick(
            hit,
            self.attack_held,
            self.session.gamemode == 1,
            per_tick_progress,
            &held_item,
        );

        let restarted = update.cancel.is_some();
        if let Some(cancel) = update.cancel {
            // PlayerControllerMP uses the newly targeted face when swapping
            // blocks, and EnumFacing.DOWN when a held dig is simply released.
            let face = update
                .start
                .map(|start| start.face)
                .unwrap_or(client::physics::BlockFace::Bottom);
            client::network::send_digging_cancel(&self.net_ctrl.connection, cancel.pos, face);
        }
        if let Some(start) = update.start {
            // Initial clickMouse swings before START. A held-target/item
            // change instead runs clickBlock first (ABORT -> START), then the
            // outer sendClickBlockToController swing.
            if !restarted {
                client::network::send_animation(&self.net_ctrl.connection);
            }
            client::network::send_digging_start(&self.net_ctrl.connection, start.pos, start.face);
            if restarted {
                client::network::send_animation(&self.net_ctrl.connection);
            }
            self.spawn_block_hit_feedback(start);
        }
        if let Some(hit) = update.hit_particle {
            self.spawn_block_hit_feedback(hit);
        }
        if let Some(pos) = update.hit_sound {
            let sound_type = old_block.sound_type();
            self.audio.play(audio::SoundEvent {
                name: block_step_sound(old_block).to_string(),
                category: audio::SoundCategory::Blocks,
                // PlayerControllerMP: (stepSound.volume + 1) / 8.
                volume: (sound_type.volume() + 1.0) / 8.0,
                pitch: sound_type.pitch() * 0.5,
                position: Some([pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5]),
            });
        }
        if let Some(finish) = update.finish {
            let meshes = self.finish_digging(finish, update.start.is_none());
            // During sustained mining, Minecraft sends its C0A after the
            // STOP_DESTROY_BLOCK packet for the completing tick.
            if update.start.is_none() {
                client::network::send_animation(&self.net_ctrl.connection);
            }
            return meshes;
        }
        if update.start.is_none() && self.dig.active_pos().is_some() {
            // sendClickBlockToController swings once for every held mining tick.
            client::network::send_animation(&self.net_ctrl.connection);
        }

        Vec::new()
    }

    pub(super) fn attack_targeted_entity(&mut self) -> bool {
        // Minecraft.runTick drains attack presses while any item is in use.
        if self.item_use_active {
            return false;
        }

        let eye = self.player.eye_position();
        let eye = nalgebra::Point3::new(eye.x as f32, eye.y as f32, eye.z as f32);
        let entity_target = client::interaction::target_entity_from(
            &self.entities,
            eye,
            self.player.camera.front,
            3.0,
            self.session.entity_id,
        );
        let Some(entity_target) = entity_target else {
            return false;
        };
        // Status 3 is terminal for a living entity. Keep this separate from
        // the one-second visual timer: a server/plugin can leave the corpse
        // tracked for longer, but attacking it would be rejected as invalid.
        if let Some(entity) = self.entities.get(entity_target.entity_id) {
            if entity.last_status == Some(3) || entity.death_time > 0.0 || entity.attack_pending() {
                return false;
            }
            if let crate::entity::EntityData::Mob { health, .. }
            | crate::entity::EntityData::Living { health, .. } = &entity.data
            {
                if *health <= 0.0 {
                    return false;
                }
            }
        }
        let block_distance =
            client::interaction::target_hit_from(&self.world, eye, self.player.camera.front, 4.5)
                .map(|hit| hit.distance)
                .unwrap_or(f32::MAX);
        if block_distance < entity_target.distance {
            return false;
        }

        // Vanilla Minecraft.clickMouse always swings before dispatching the
        // click. Entity attacks additionally swing inside attackEntity, but
        // that second swing is handled by the server response (EntityStatus 2).
        if let Some(renderer) = &mut self.renderer {
            renderer.trigger_hand_swing();
        }
        // PlayerControllerMP.attackEntity never brackets C02 with release-use
        // and resume-use packets. Interleaving those C07/C08 packets with an
        // active dig trips packet-order simulation checks on servers.
        client::network::send_use_entity_attack(&self.net_ctrl.connection, entity_target.entity_id);
        // EntityLivingBase.attackEntityFrom returns false in a remote world, so
        // ordinary mobs do not run the attacker's knockback slowdown locally.
        // EntityOtherPlayerMP overrides it to true; the remaining listed types
        // mirror the legacy entities Grim/vanilla can damage client-side.
        let applies_local_attack =
            self.entities
                .get(entity_target.entity_id)
                .is_some_and(|entity| {
                    !entity.entity_type.is_mob()
                        || matches!(
                            entity.entity_type,
                            crate::entity::EntityType::Player
                                | crate::entity::EntityType::Painting
                                | crate::entity::EntityType::EnderDragon
                        )
                });
        let knockback_level = self
            .inventory
            .selected_enchantment_level(ENCHANTMENT_KNOCKBACK);
        let velocity_before = self.player.velocity;
        let sprinting_before = self.player.sprinting;
        if applies_local_attack {
            self.player.on_attack_entity(knockback_level);
        }
        log::debug!(
            target: "rustcraft::movement",
            "local attack: target_id={}, target_type={:?}, applies_local_attack={}, knockback_level={}, sprinting_before={}, movement_sprinting={}, velocity_before=({:.6},{:.6},{:.6}), velocity_after=({:.6},{:.6},{:.6})",
            entity_target.entity_id,
            self.entities
                .get(entity_target.entity_id)
                .map(|entity| entity.entity_type),
            applies_local_attack,
            knockback_level,
            sprinting_before,
            self.player.movement_sprinting(),
            velocity_before.x,
            velocity_before.y,
            velocity_before.z,
            self.player.velocity.x,
            self.player.velocity.y,
            self.player.velocity.z,
        );
        if let Some(mut entity) = self.entities.get_mut(entity_target.entity_id) {
            // Target hurt animation is authoritative: the server emits
            // EntityStatus(2) only when this attack actually dealt damage.
            entity.mark_attack_pending();
        }
        // Vanilla EntityPlayer.attackTargetEntityWithCurrentItem runs on the
        // client too: the held weapon takes hitEntity durability locally.
        let wear = attack_tool_wear(self.inventory.selected_item().item_id);
        self.damage_held_item(wear);
        // Hurt sound is played by the server via EntityStatus status=2
        true
    }

    fn interact_targeted_entity(&mut self) -> bool {
        let eye = self.player.eye_position();
        let eye = nalgebra::Point3::new(eye.x as f32, eye.y as f32, eye.z as f32);
        let entity_target = client::interaction::target_entity_from(
            &self.entities,
            eye,
            self.player.camera.front,
            3.0,
            self.session.entity_id,
        );
        let Some(entity_target) = entity_target else {
            return false;
        };
        // Check if entity is closer than any targeted block
        let block_distance =
            client::interaction::target_hit_from(&self.world, eye, self.player.camera.front, 4.5)
                .map(|hit| hit.distance)
                .unwrap_or(f32::MAX);
        if block_distance < entity_target.distance {
            return false;
        }
        let eye = self.player.eye_position();
        let hit_point = nalgebra::Point3::new(eye.x as f32, eye.y as f32, eye.z as f32)
            + self.player.camera.front.normalize() * entity_target.distance;
        let Some(entity) = self.entities.get(entity_target.entity_id) else {
            return false;
        };
        let relative_hit = [
            hit_point.x - entity.position.x,
            hit_point.y - entity.position.y,
            hit_point.z - entity.position.z,
        ];
        // rightClickMouse first sends INTERACT_AT, then falls back to INTERACT
        // when the client-side entity handler does not consume that hit.
        client::network::send_use_entity_interact_at(
            &self.net_ctrl.connection,
            entity_target.entity_id,
            relative_hit,
        );
        client::network::send_use_entity_interact(&self.net_ctrl.connection, entity_target.entity_id);
        // Do NOT consume the right-click — let the item-use path
        // (sword blocking, eating, bow drawing, …) also run so that
        // the player can block while still sending the interact
        // packet to the server (vanilla does the same for
        // non-interactable entities like players and hostile mobs).
        false
    }

    fn finish_digging(
        &mut self,
        hit: client::physics::RaycastHit,
        send_stop: bool,
    ) -> Vec<crate::world::mesh::ChunkMesh> {
        let old_block = self.world.get_block(hit.pos.0, hit.pos.1, hit.pos.2);
        if old_block.properties().hardness < 0.0 {
            return Vec::new();
        }
        if self.session.gamemode == 1 && is_sword(self.inventory.selected_item().item_id) {
            return Vec::new();
        }
        self.particles.spawn_block_break(
            self.world.get_block_state(hit.pos.0, hit.pos.1, hit.pos.2),
            nalgebra::Point3::new(hit.pos.0 as f32, hit.pos.1 as f32, hit.pos.2 as f32),
        );
        // Always play dig sound locally for the player who broke the block.
        // The server also sends Effect 2001 to broadcast the sound to nearby players.
        let pitch = (rand_f32() - rand_f32()) * 0.2 + 1.0;
        self.audio.play(audio::SoundEvent {
            name: block_dig_sound(old_block).to_string(),
            category: audio::SoundCategory::Blocks,
            volume: 1.0,
            pitch,
            position: Some([hit.pos.0 as f32, hit.pos.1 as f32, hit.pos.2 as f32]),
        });

        if self.session.gamemode != 1 {
            if send_stop {
                client::network::send_digging_finish(&self.net_ctrl.connection, hit.pos, hit.face);
            }
            // Vanilla PlayerControllerMP.onPlayerDestroyBlock calls
            // ItemStack.onBlockDestroyed outside creative, wearing down the
            // held tool locally; the server re-syncs the slot if it differs.
            let wear = block_break_tool_wear(self.inventory.selected_item().item_id, old_block);
            self.damage_held_item(wear);
        }

        // Creative clickBlock and sustained STOP completions set the delay.
        // A survival block broken instantly by the initial START does not.
        if self.session.gamemode == 1 || send_stop {
            self.block_hit_delay = 5;
        }

        // Vanilla PlayerControllerMP.onPlayerDestroyBlock removes the block
        // from WorldClient immediately in every editable game mode. The
        // server remains authoritative and restores it with S23 if rejected.
        self.world.apply_block_change(
            hit.pos.0,
            hit.pos.1,
            hit.pos.2,
            world::block::Block::Air.to_id() << 4,
        );
        self.world
            .build_immediate_mesh_at_block(hit.pos.0, hit.pos.2)
            .into_iter()
            .collect()
    }

    pub(super) fn cancel_digging(&mut self) {
        if let Some(hit) = self.dig.cancel() {
            // Input callbacks run on render frames, not at the fixed tick
            // boundary. Defer the packet so it cannot land after a C03.
            self.pending_dig_cancel = Some((hit.pos, client::physics::BlockFace::Bottom));
        }
    }

    /// Cancel initiated by the fixed-tick interaction phase. This is already
    /// before player movement, so it can be sent immediately like vanilla.
    fn cancel_digging_now(&mut self) {
        if let Some(hit) = self.dig.cancel() {
            client::network::send_digging_cancel(
                &self.net_ctrl.connection,
                hit.pos,
                client::physics::BlockFace::Bottom,
            );
        }
    }

    pub(super) fn flush_pending_dig_cancel(&mut self) {
        if let Some((pos, face)) = self.pending_dig_cancel.take() {
            client::network::send_digging_cancel(&self.net_ctrl.connection, pos, face);
        }
    }

    fn spawn_block_hit_feedback(&mut self, hit: client::physics::RaycastHit) {
        self.particles.spawn_block_hit(
            self.world.get_block_state(hit.pos.0, hit.pos.1, hit.pos.2),
            nalgebra::Point3::new(hit.pos.0 as f32, hit.pos.1 as f32, hit.pos.2 as f32),
            hit.face,
        );
    }

    pub(super) fn place_selected_block(&mut self) {
        let eye = self.player.eye_position();
        let eye = nalgebra::Point3::new(eye.x as f32, eye.y as f32, eye.z as f32);
        let Some(hit) = client::physics::raycast(&eye, &self.player.camera.front, 4.5, &self.world)
        else {
            return;
        };

        // PlayerControllerMP.onPlayerRightClick derives hitX/hitY/hitZ from
        // the exact ray hit, relative to the clicked block. Both the local
        // placement prediction and the C08 cursor position need them.
        let hit_point = eye + self.player.camera.front.normalize() * hit.distance;
        let hit_frac = (
            protocol_hit_fraction(hit_point.x - hit.pos.0 as f32),
            protocol_hit_fraction(hit_point.y - hit.pos.1 as f32),
            protocol_hit_fraction(hit_point.z - hit.pos.2 as f32),
        );

        let clicked_block = self.world.get_block(hit.pos.0, hit.pos.1, hit.pos.2);

        // Vanilla PlayerControllerMP always sends C08 for a block hit before
        // attempting ItemBlock placement.  Empty-hand activation must not be
        // subject to the prospective-placement collision check below.
        let Some(block) = self.inventory.selected_block() else {
            self.inventory.pending_chest_position =
                crate::world::is_chest_block(clicked_block).then_some(hit.pos);
            client::network::send_block_placement_slot(
                &self.net_ctrl.connection,
                &self.inventory.protocol_slot_for_selected_item(),
                hit.pos,
                hit.face,
                hit_frac,
            );
            if let Some(renderer) = &mut self.renderer {
                renderer.trigger_hand_swing();
            }
            return;
        };

        let clicked_state = self.world.get_block_state(hit.pos.0, hit.pos.1, hit.pos.2);
        // PlayerControllerMP skips ItemBlock.onItemUse when the clicked block
        // consumes activation. Sneaking with a held item bypasses activation.
        let predicts_item_use =
            self.input_ctrl.input.is_held(Action::Sneak) || !block_may_consume_activation(clicked_block);
        let held_slot = self.inventory.protocol_slot_for_selected_item();
        let mut predicted_placement = None;
        if predicts_item_use {
            let stacked_snow = stacked_snow_layer_state(clicked_block, clicked_state, hit.face);
            let placement_pos = if stacked_snow.is_some()
                || block_is_replaceable_for_placement(clicked_block, clicked_state)
            {
                hit.pos
            } else {
                let n = hit.face.normal();
                (hit.pos.0 + n.0, hit.pos.1 + n.1, hit.pos.2 + n.2)
            };
            if (0..crate::world::chunk::CHUNK_HEIGHT as i32).contains(&placement_pos.1) {
                let replaced_state =
                    self.world
                        .get_block_state(placement_pos.0, placement_pos.1, placement_pos.2);
                let can_replace = stacked_snow.is_some()
                    || block_is_replaceable_for_placement(
                        self.world
                            .get_block(placement_pos.0, placement_pos.1, placement_pos.2),
                        replaced_state,
                    );
                if can_replace {
                    let held = self.inventory.selected_item();
                    let block_state = stacked_snow.unwrap_or_else(|| {
                        if matches!(
                            block,
                            Block::Piston | Block::StickyPiston | Block::Dispenser | Block::Dropper
                        ) {
                            predicted_entity_facing_state(
                                block,
                                held.item_id,
                                placement_pos,
                                self.player.position,
                                self.player.eye_position().y,
                                self.player.camera.mc_yaw_degrees(),
                            )
                        } else {
                            predicted_item_block_state(
                                block,
                                held.item_id,
                                held.damage,
                                hit.face,
                                self.player.camera.mc_yaw_degrees(),
                                hit_frac.1,
                            )
                        }
                    });
                    // ItemSnow checks the incremented layer's collision box;
                    // ordinary ItemBlocks use their predicted state (which
                    // encodes the correct metadata, e.g. slab half) so the
                    // game never lets you place a block inside the player.
                    let collision_state = stacked_snow.unwrap_or(block_state);
                    let player_box = client::physics::Aabb::new(
                        &self.player.position,
                        client::physics::PLAYER_WIDTH,
                        client::physics::PLAYER_HEIGHT,
                    );
                    let mut blocked = client::physics::block_state_intersects_aabb(
                        &self.world,
                        collision_state,
                        placement_pos,
                        &player_box,
                    );
                    for (_entity_id, entity) in self.entities.iter() {
                        if blocked
                            || Some(entity.entity_id) == self.session.entity_id
                            || !entity_prevents_block_placement(entity.entity_type)
                        {
                            continue;
                        }
                        let (width, height) = entity.entity_type.bounding_box();
                        let entity_pos = nalgebra::Point3::new(
                            f64::from(entity.position.x),
                            f64::from(entity.position.y),
                            f64::from(entity.position.z),
                        );
                        let entity_box = client::physics::Aabb::new(
                            &entity_pos,
                            f64::from(width),
                            f64::from(height),
                        );
                        blocked = client::physics::block_state_intersects_aabb(
                            &self.world,
                            collision_state,
                            placement_pos,
                            &entity_box,
                        );
                    }
                    if !blocked {
                        predicted_placement = Some((placement_pos, block_state));
                    }
                }
            }
        }

        if predicts_item_use && predicted_placement.is_none() {
            // Placement was blocked by entity collision or the target block
            // was not replaceable. Don't send C08 — vanilla ItemBlock.onItemUse
            // returns false and the server-side code takes no action anyway,
            // so sending the packet would cause the server to force-place the
            // block and shove the player out.
            return;
        }

        self.inventory.pending_chest_position =
            (!predicts_item_use && crate::world::is_chest_block(clicked_block)).then_some(hit.pos);
        client::network::send_block_placement_slot(
            &self.net_ctrl.connection,
            &held_slot,
            hit.pos,
            hit.face,
            hit_frac,
        );
        if let Some((placement_pos, block_state)) = predicted_placement {
            // ItemBlock.onItemUse mutates WorldClient immediately. Besides
            // hiding round-trip latency, the next movement tick now collides
            // with the new block and cannot walk into a pending placement.
            self.world.apply_block_change(
                placement_pos.0,
                placement_pos.1,
                placement_pos.2,
                block_state,
            );
            if let Some(mesh) = self
                .world
                .build_immediate_mesh_at_block(placement_pos.0, placement_pos.2)
            {
                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_world_partial(&[mesh]);
                }
            }
            if self.session.gamemode != 1 {
                self.inventory.remove_selected_one();
            }
        }
        if self.net_ctrl.connection.is_some() && self.session.gamemode == 1 {
            client::network::send_creative_inventory_action_slot(
                &self.net_ctrl.connection,
                36 + self.inventory.selected as i16,
                &held_slot,
            );
        }
        // MCP: rightClickMouse — reset equip animation when block is placed
        self.item_place_delay = 4;
        if let Some(renderer) = &mut self.renderer {
            renderer.reset_equipped_progress();
            renderer.trigger_hand_swing();
        }
    }

    // =====================================================================
    // Item use system — eating, drinking, bow, throwing, buckets, tools
    // =====================================================================

    /// Called when the player presses right-click. Dispatches based on item type.
    /// If the held item is a block, places it (existing behavior).
    /// Otherwise starts the appropriate item-use action (eating, bow, throw, etc.).
    pub(super) fn use_item(&mut self) {
        // Match Minecraft.rightClickMouse: an active block dig owns the
        // interaction for this tick, so no entity interaction or C08 is sent.
        if self.dig.active_pos().is_some() {
            return;
        }
        // A blocking sword is a pure use-item action. Do not emit entity
        // INTERACT/INTERACT_AT packets merely because an entity is under the
        // crosshair; that is not part of maintaining the block action.
        let (item_id, item_damage) = {
            let held = self.inventory.selected_item();
            (held.item_id, held.damage)
        };
        if self.use_release_pending {
            return;
        }
        // Minecraft.rightClickMouse applies this delay to every right-click,
        // including swords and other non-block items. Queued presses bypass it;
        // only held auto-repeat waits for it to reach zero.
        self.item_place_delay = 4;
        if !is_sword(item_id) && self.interact_targeted_entity() {
            return;
        }

        // Placeable blocks and block-like items (redstone dust id 331, etc.).
        if self.inventory.selected_block().is_some() {
            self.place_selected_block();
            return;
        }

        // Non-block item — determine use type
        if item_id == 386 {
            self.open_writable_book();
        } else if is_sword(item_id) {
            // Sword blocking begins immediately with the same use-item packet
            // vanilla sends to the server, and remains active while held.
            self.send_item_use_packet();
            self.item_use_active = true;
            self.item_use_timer = 0.0;
        } else if is_food(item_id) {
            self.send_item_use_packet();
            // ItemFood only calls setItemInUse when the player can eat.
            if self.player.food_level < 20 || item_id == 322 {
                self.item_use_active = true;
                self.item_use_timer = 0.0;
            }
        } else if is_drinkable(item_id, item_damage) {
            self.send_item_use_packet();
            self.item_use_active = true;
            self.item_use_timer = 0.0;
        } else if is_splash_potion(item_id, item_damage) {
            self.send_item_use_packet();
            self.spawn_predicted_throwable(item_id, item_damage);
            self.item_use_active = false;
        } else if item_id == 261 {
            self.send_item_use_packet();
            // ItemBow only calls setItemInUse in creative or with an arrow.
            if self.session.gamemode == 1
                || self
                    .inventory
                    .slots
                    .iter()
                    .any(|stack| stack.item_id == 262 && !stack.is_empty())
            {
                self.item_use_active = true;
                self.item_use_timer = 0.0;
            }
        } else if is_throwable(item_id) {
            // Instant throw
            self.send_item_use_packet();
            self.spawn_predicted_throwable(item_id, item_damage);
            self.item_use_active = false;
        } else if item_id == 325 || item_id == 326 || item_id == 327 {
            // ItemBucket has no onItemUse handler. Vanilla first sends the
            // targeted C08, then sends its face=255 use-item C08 so the server
            // can raycast and place/collect the liquid in onItemRightClick.
            if let Some(hit) = client::physics::raycast(
                &self.player.camera.position,
                &self.player.camera.front,
                4.5,
                &self.world,
            ) {
                self.send_block_placement_for_item();
                let block = self.world.get_block(hit.pos.0, hit.pos.1, hit.pos.2);
                if !block_may_consume_activation(block) {
                    self.send_item_use_packet();
                }
            } else {
                self.send_item_use_packet();
            }
        } else {
            // Flint & steel, shears, etc. — send block placement if targeting a block
            if client::physics::raycast(
                &self.player.camera.position,
                &self.player.camera.front,
                4.5,
                &self.world,
            )
            .is_some()
            {
                self.send_block_placement_for_item();
                if let Some(renderer) = &mut self.renderer {
                    renderer.trigger_hand_swing();
                }
            } else {
                // No target — just send use item packet
                self.send_item_use_packet();
            }
        }
    }

    /// Tick item use progress (eating, drinking, bow drawing, sword blocking).
    /// Called every frame while use_held.
    ///
    /// Vanilla MC: the CLIENT only plays the animation and tracks use_progress.
    /// The SERVER decides when food is consumed (32 ticks) and sends the result.
    /// For sword blocking and bow drawing the client just holds the active state.
    pub(super) fn tick_item_use(&mut self, dt: f32) {
        if !self.item_use_active {
            return;
        }

        let held = self.inventory.selected_item();
        let item_id = held.item_id;

        if is_food(item_id) || is_drinkable(item_id, held.damage) {
            self.item_use_timer += dt;
            let progress = (self.item_use_timer / 1.6).clamp(0.0, 1.0);

            // Play eating sound every 4 ticks (MC 1.8.9)
            let prev_tick = ((self.item_use_timer - dt) * 20.0) as i32;
            let cur_tick = (self.item_use_timer * 20.0) as i32;
            if is_food(item_id) && cur_tick > prev_tick && cur_tick % 4 == 0 {
                self.audio.play(audio::SoundEvent {
                    name: "random.eat".to_string(),
                    category: audio::SoundCategory::Players,
                    volume: 0.5 + rand_f32() * 0.1,
                    pitch: 0.8 + rand_f32() * 0.4,
                    position: None,
                });
            }
            // Complete eating animation at 1.6s with cooldown gap.
            // Inventory consumption for offline only (server handles it in online).
            if self.item_use_timer >= 1.6 {
                self.item_use_timer = 0.0;
                self.food_cooldown = 4;
                if self.net_ctrl.connection.is_none() {
                    // Offline: consume immediately. Online: the server sends
                    // S19 status 9 when the food is consumed; the client keeps
                    // item_use_active (and the 0.2 movement slowdown) until
                    // that response arrives. Clearing it locally without C07
                    // would cause NoSlow, and sending C07 from the tick loop
                    // would cause Post timing violations.
                    self.item_use_active = false;
                    self.audio.play(audio::SoundEvent {
                        name: "random.burp".to_string(),
                        category: audio::SoundCategory::Players,
                        volume: 0.5,
                        pitch: 0.9 + rand_f32() * 0.2,
                        position: None,
                    });
                    let held = self.inventory.selected_item();
                    if held.item_id == item_id {
                        self.inventory.remove_selected_one();
                    }
                }
            }
        } else if item_id == 261 {
            // Bow drawing — accumulate time, cap at 72 ticks (3.6s)
            self.item_use_timer = (self.item_use_timer + dt).min(3.6);
        } else if is_sword(item_id) {
            // Sword blocking — keep active while holding right-click.
            // Timer ticks to sync with renderer; no timeout.
            self.item_use_timer += dt;
        } else {
            // Unknown item use — keep active (e.g. shields etc.)
            self.item_use_timer += dt;
        }
    }

    /// Called when the player releases right-click. If drawing a bow, fire it.
    pub(super) fn release_item_use(&mut self) {
        if !self.item_use_active {
            return;
        }

        let held = self.inventory.selected_item();
        let held_item_id = held.item_id;
        let held_item_damage = held.damage;
        let bow_charge = self.item_use_timer;
        // Vanilla ends every continuous use with C07 RELEASE_USE_ITEM. The
        // server consumes food/drink or fires a charged bow from this state.
        if held_item_id == 261
            || is_sword(held_item_id)
            || is_food(held_item_id)
            || is_drinkable(held_item_id, held_item_damage)
        {
            client::network::send_release_use_item(&self.net_ctrl.connection);
            self.use_release_pending = true;
            if held_item_id == 261 {
                self.spawn_predicted_arrow(bow_charge);
            }
        }

        self.item_use_active = false;
        self.item_use_timer = 0.0;
    }

    /// Send a "use item" network packet (right-click without block target).
    pub(super) fn send_item_use_packet(&self) {
        let held = self.inventory.protocol_slot_for_selected_item();
        client::network::send_use_item(&self.net_ctrl.connection, &held);
    }

    fn next_predicted_entity_id(&mut self) -> i32 {
        let id = self.next_predicted_entity_id;
        self.next_predicted_entity_id = self.next_predicted_entity_id.saturating_sub(1);
        id
    }

    fn spawn_predicted_arrow(&mut self, charge_seconds: f32) {
        let charge = (charge_seconds * 20.0 / 20.0).clamp(0.0, 1.0);
        let power = ((charge * charge + charge * 2.0) / 3.0).min(1.0);
        if power < 0.1 {
            return;
        }
        let front = self.player.camera.front.normalize();
        let eye = self.player.eye_position();
        let yaw = self.player.camera.mc_yaw_degrees().to_radians();
        // EntityArrow(shooter, velocity): start at eye height, offset toward
        // the bow hand horizontally and 0.1 block downward.
        let position = nalgebra::Point3::new(
            eye.x as f32 - yaw.cos() * 0.16,
            eye.y as f32 - 0.1,
            eye.z as f32 - yaw.sin() * 0.16,
        );
        let mut entity = crate::entity::Entity::new(
            self.next_predicted_entity_id(),
            EntityType::Arrow,
            position,
        );
        entity.velocity = front * (power * 3.0);
        entity.yaw = entity.velocity.x.atan2(entity.velocity.z).to_degrees();
        entity.pitch = entity
            .velocity
            .y
            .atan2(
                (entity.velocity.x * entity.velocity.x + entity.velocity.z * entity.velocity.z)
                    .sqrt(),
            )
            .to_degrees();
        entity.body_yaw = entity.yaw;
        self.entities.spawn(entity);
    }

    fn spawn_predicted_throwable(&mut self, item_id: u16, damage: u16) {
        let entity_type = match item_id {
            332 => EntityType::Snowball,
            344 => EntityType::ThrownEgg,
            368 => EntityType::EnderPearl,
            381 => EntityType::EnderEye,
            384 => EntityType::ThrownExpBottle,
            373 => EntityType::ThrownPotion,
            _ => return,
        };
        let front = self.player.camera.front.normalize();
        let position = self.player.camera.position + front * 0.16;
        let mut entity =
            crate::entity::Entity::new(self.next_predicted_entity_id(), entity_type, position);
        entity.velocity = front * 1.5;
        entity.velocity.y -= 0.1;
        entity.current_item = Some(item_id as i16);
        if matches!(
            entity_type,
            EntityType::ThrownPotion | EntityType::ThrownExpBottle
        ) {
            entity.current_item = Some(damage as i16);
        }
        self.entities.spawn(entity);
    }

    pub(super) fn spawn_predicted_dropped_item(&mut self, drop_stack: bool) {
        let (item_id, count, damage, nbt) = {
            let held = self.inventory.selected_item();
            (
                held.item_id,
                held.count,
                held.damage,
                self.inventory.slot_meta[self.inventory.selected]
                    .nbt
                    .clone(),
            )
        };
        if item_id <= 0 || count <= 0 {
            return;
        }
        let front = self.player.camera.front.normalize();
        let position =
            self.player.camera.position + front * 0.3 - nalgebra::Vector3::new(0.0, 0.3, 0.0);
        let mut entity =
            crate::entity::Entity::new(self.next_predicted_entity_id(), EntityType::Item, position);
        entity.velocity = front * 0.3 + nalgebra::Vector3::new(0.0, 0.2, 0.0);
        entity.data = crate::entity::EntityData::Item {
            item_id: item_id as u16,
            count: if drop_stack { count as u8 } else { 1 },
            damage: damage as u16,
            nbt,
        };
        self.entities.spawn(entity);
    }

    /// Send a block placement packet for the currently held non-block item.
    fn send_block_placement_for_item(&mut self) {
        let eye = self.player.eye_position();
        let eye = nalgebra::Point3::new(eye.x as f32, eye.y as f32, eye.z as f32);
        let Some(hit) = client::physics::raycast(&eye, &self.player.camera.front, 4.5, &self.world)
        else {
            return;
        };
        let hit_point = eye + self.player.camera.front.normalize() * hit.distance;
        let hit_frac = (
            (hit_point.x - hit.pos.0 as f32).clamp(0.0, 1.0),
            (hit_point.y - hit.pos.1 as f32).clamp(0.0, 1.0),
            (hit_point.z - hit.pos.2 as f32).clamp(0.0, 1.0),
        );
        let held = self.inventory.protocol_slot_for_selected_item();
        let clicked_block = self.world.get_block(hit.pos.0, hit.pos.1, hit.pos.2);
        self.inventory.pending_chest_position = (!self.input_ctrl.input.is_held(Action::Sneak)
            && crate::world::is_chest_block(clicked_block))
        .then_some(hit.pos);
        client::network::send_block_placement_slot(
            &self.net_ctrl.connection,
            &held,
            hit.pos,
            hit.face,
            hit_frac,
        );
    }

    // =====================================================================
    // Tool mechanics — vanilla 1.8.9 dig-speed model
    // =====================================================================

    /// Vanilla `Block.getPlayerRelativeBlockHardness`: the fraction of block
    /// damage accumulated per tick. 0 for unbreakable blocks, `INFINITY`
    /// (instant) for zero-hardness blocks.
    pub(super) fn block_dig_progress_per_tick(&self, block: Block) -> f32 {
        let hardness = block.properties().hardness;
        if hardness < 0.0 {
            return 0.0;
        }
        let speed = self.dig_efficiency(block);
        let divisor = if self.can_harvest_block(block) {
            30.0
        } else {
            100.0
        };
        if hardness == 0.0 {
            return f32::INFINITY;
        }
        speed / hardness / divisor
    }

    /// Vanilla `EntityPlayer.getToolDigEfficiency`: raw tool strength plus
    /// Efficiency enchantment, Haste / Mining Fatigue potion effects, and the
    /// underwater (without Aqua Affinity) and off-ground /5 penalties.
    pub(super) fn dig_efficiency(&self, block: Block) -> f32 {
        let held = self.inventory.selected_item();
        let mut f = held_tool(held.item_id).strength_vs_block(block);

        if f > 1.0 {
            // EnchantmentHelper.getEfficiencyModifier: f += level² + 1.
            let efficiency = i32::from(
                self.inventory
                    .selected_enchantment_level(ENCHANTMENT_EFFICIENCY),
            );
            if efficiency > 0 && !held.is_empty() {
                f += (efficiency * efficiency + 1) as f32;
            }
        }

        // Haste: ×(1 + 0.2·(amplifier + 1)).
        if let Some(effect) = self
            .player
            .active_effects
            .iter()
            .find(|effect| effect.effect_id == POTION_HASTE)
        {
            f *= 1.0 + (effect.amplifier as f32 + 1.0) * 0.2;
        }

        // Mining fatigue: fixed multiplier table per amplifier.
        if let Some(effect) = self
            .player
            .active_effects
            .iter()
            .find(|effect| effect.effect_id == POTION_MINING_FATIGUE)
        {
            f *= match effect.amplifier {
                0 => 0.3,
                1 => 0.09,
                2 => 0.0027,
                _ => 8.1e-4,
            };
        }

        // isInsideOfMaterial(Material.water) without Aqua Affinity on armor.
        if self.player_eye_in_water()
            && self
                .inventory
                .max_armor_enchantment_level(ENCHANTMENT_AQUA_AFFINITY)
                <= 0
        {
            f /= 5.0;
        }

        if !self.player.on_ground {
            f /= 5.0;
        }

        f
    }

    /// Vanilla `InventoryPlayer.canHeldItemHarvest`: blocks whose material
    /// does not require a tool are always harvestable, otherwise the held
    /// item's `canHarvestBlock` decides between the /30 and /100 dig paths.
    pub(super) fn can_harvest_block(&self, block: Block) -> bool {
        if !block.material().requires_tool() {
            return true;
        }
        held_tool(self.inventory.selected_item().item_id).can_harvest_block(block)
    }

    /// Vanilla `Entity.isInsideOfMaterial(Material.water)`: the eye must be
    /// inside a water block and below its liquid surface
    /// (`BlockLiquid.getLiquidHeightPercent` minus 1/9).
    fn player_eye_in_water(&self) -> bool {
        let eye = self.player.eye_position();
        let (bx, by, bz) = (
            eye.x.floor() as i32,
            eye.y.floor() as i32,
            eye.z.floor() as i32,
        );
        if !matches!(
            self.world.get_block(bx, by, bz),
            Block::FlowingWater | Block::StillWater
        ) {
            return false;
        }
        let mut meta = self.world.get_block_metadata(bx, by, bz);
        if meta >= 8 {
            meta = 0;
        }
        let filled = (meta as f32 + 1.0) / 9.0 - 0.111_111_11;
        (eye.y as f32) < (by as f32 + 1.0) - filled
    }

    /// Vanilla `ItemStack.damageItem`: applies local durability wear with the
    /// Unbreaking negation roll and breaks the item when damage exceeds its
    /// maximum. The server stays authoritative and re-syncs the slot via S2F.
    pub(super) fn damage_held_item(&mut self, amount: i32) {
        if amount <= 0 || self.session.gamemode == 1 {
            return;
        }
        let held = *self.inventory.selected_item();
        if held.is_empty() {
            return;
        }
        let max_damage = crate::client::inventory::max_damage(held.item_id);
        if max_damage == 0 {
            return;
        }
        // ItemStack.attemptDamageItem: Unbreaking may negate each point
        // (EnchantmentDurability.negateDamage: nextInt(level + 1) > 0).
        let unbreaking = i32::from(
            self.inventory
                .selected_enchantment_level(ENCHANTMENT_UNBREAKING),
        );
        let mut remaining = amount;
        if unbreaking > 0 {
            for _ in 0..amount {
                if (rand_f32() * (unbreaking as f32 + 1.0)) as i32 > 0 {
                    remaining -= 1;
                }
            }
            if remaining <= 0 {
                return;
            }
        }
        let stack = self.inventory.selected_item_mut();
        stack.damage = stack.damage.saturating_add(remaining as u16);
        if stack.damage > max_damage {
            // EntityLivingBase.renderBrokenItemStack: break sound, then the
            // stack loses one item (tools always have a size of one).
            stack.count = stack.count.saturating_sub(1);
            stack.damage = 0;
            if stack.count == 0 {
                *stack = crate::client::inventory::ItemStack::EMPTY;
            }
            self.audio.play(audio::SoundEvent {
                name: "random.break".to_string(),
                category: audio::SoundCategory::Players,
                volume: 0.8,
                pitch: 0.8 + rand_f32() * 0.4,
                position: None,
            });
        }
    }
}

// =====================================================================
// Item type helpers
// =====================================================================

pub(super) fn is_sword(id: u16) -> bool {
    matches!(id, 267 | 268 | 272 | 276 | 283)
}

/// Check if an item ID is a food item.
pub(super) fn is_food(id: u16) -> bool {
    matches!(
        id,
        260 | 282
            | 297
            | 322
            | 349
            | 350
            | 357
            | 360
            | 363
            | 364
            | 365
            | 366
            | 367
            | 391
            | 392
            | 393
            | 400
            | 411
            | 412
            | 413
            | 423
            | 424
            | 354
    )
}

/// Check if an item ID is a potion.
pub(super) fn is_potion(id: u16) -> bool {
    id == 373 // potion
}

fn is_splash_potion(id: u16, damage: u16) -> bool {
    is_potion(id) && damage & 0x4000 != 0
}

fn is_drinkable(id: u16, damage: u16) -> bool {
    id == 335 || (is_potion(id) && !is_splash_potion(id, damage))
}

/// Check if an item ID is a throwable projectile.
fn is_throwable(id: u16) -> bool {
    matches!(
        id,
        332  // snowball
        | 344 // egg
        | 368 // ender pearl
        | 381 // eye of ender
        | 384 // bottle o' enchanting
    )
}

// =====================================================================
// Vanilla tool model — ItemTool / ItemSword / ItemShears
// =====================================================================

/// 1.8.9 enchantment / potion IDs used by the dig-speed model.
const ENCHANTMENT_AQUA_AFFINITY: i16 = 6;
const ENCHANTMENT_KNOCKBACK: i16 = 19;
const ENCHANTMENT_EFFICIENCY: i16 = 32;
const ENCHANTMENT_UNBREAKING: i16 = 34;
const POTION_HASTE: i8 = 3;
const POTION_MINING_FATIGUE: i8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolKind {
    Pickaxe,
    Axe,
    Shovel,
    Sword,
    Shears,
    None,
}

#[derive(Clone, Copy, Debug)]
struct HeldTool {
    kind: ToolKind,
    /// `ToolMaterial.getHarvestLevel()`: wood/gold 0, stone 1, iron 2, diamond 3.
    harvest_level: i32,
    /// `ToolMaterial.getEfficiencyOnProperMaterial()`: wood 2, stone 4,
    /// iron 6, diamond 8, gold 12.
    efficiency: f32,
}

const NO_TOOL: HeldTool = HeldTool {
    kind: ToolKind::None,
    harvest_level: 0,
    efficiency: 1.0,
};

fn held_tool(item_id: u16) -> HeldTool {
    let tool = |kind, harvest_level, efficiency| HeldTool {
        kind,
        harvest_level,
        efficiency,
    };
    match item_id {
        // Wood (harvest 0, efficiency 2)
        268 => tool(ToolKind::Sword, 0, 2.0),
        269 => tool(ToolKind::Shovel, 0, 2.0),
        270 => tool(ToolKind::Pickaxe, 0, 2.0),
        271 => tool(ToolKind::Axe, 0, 2.0),
        // Stone (harvest 1, efficiency 4)
        272 => tool(ToolKind::Sword, 1, 4.0),
        273 => tool(ToolKind::Shovel, 1, 4.0),
        274 => tool(ToolKind::Pickaxe, 1, 4.0),
        275 => tool(ToolKind::Axe, 1, 4.0),
        // Iron (harvest 2, efficiency 6)
        256 => tool(ToolKind::Shovel, 2, 6.0),
        257 => tool(ToolKind::Pickaxe, 2, 6.0),
        258 => tool(ToolKind::Axe, 2, 6.0),
        267 => tool(ToolKind::Sword, 2, 6.0),
        // Diamond (harvest 3, efficiency 8)
        276 => tool(ToolKind::Sword, 3, 8.0),
        277 => tool(ToolKind::Shovel, 3, 8.0),
        278 => tool(ToolKind::Pickaxe, 3, 8.0),
        279 => tool(ToolKind::Axe, 3, 8.0),
        // Gold (harvest 0, efficiency 12)
        283 => tool(ToolKind::Sword, 0, 12.0),
        284 => tool(ToolKind::Shovel, 0, 12.0),
        285 => tool(ToolKind::Pickaxe, 0, 12.0),
        286 => tool(ToolKind::Axe, 0, 12.0),
        // Shears
        359 => tool(ToolKind::Shears, 0, 1.0),
        _ => NO_TOOL,
    }
}

/// Vanilla `ItemStack.onBlockDestroyed` durability cost. Tools take 1 point
/// on blocks with non-zero hardness, swords take 2, and shears take 1 only
/// on their shearable block set (regardless of hardness).
fn block_break_tool_wear(item_id: u16, block: Block) -> i32 {
    use crate::world::block::material::VanillaMaterial as M;
    let hardness = block.properties().hardness;
    match held_tool(item_id).kind {
        ToolKind::Pickaxe | ToolKind::Axe | ToolKind::Shovel => {
            if hardness != 0.0 {
                1
            } else {
                0
            }
        }
        ToolKind::Sword => {
            if hardness != 0.0 {
                2
            } else {
                0
            }
        }
        ToolKind::Shears => {
            if block.material() == M::Leaves
                || matches!(
                    block,
                    Block::Cobweb | Block::TallGrass | Block::Vine | Block::Tripwire | Block::Wool
                )
            {
                1
            } else {
                0
            }
        }
        ToolKind::None => 0,
    }
}

/// Vanilla `ItemStack.hitEntity` durability cost: swords take 1 point,
/// mining tools take 2, everything else is free.
fn attack_tool_wear(item_id: u16) -> i32 {
    match held_tool(item_id).kind {
        ToolKind::Sword => 1,
        ToolKind::Pickaxe | ToolKind::Axe | ToolKind::Shovel => 2,
        ToolKind::Shears | ToolKind::None => 0,
    }
}

impl HeldTool {
    /// Vanilla `ItemStack.getStrVsBlock` per item class.
    fn strength_vs_block(self, block: Block) -> f32 {
        use crate::world::block::material::VanillaMaterial as M;
        let material = block.material();
        match self.kind {
            // ItemPickaxe: rock/iron/anvil materials, else the EFFECTIVE_ON
            // set (ice, packed ice, and all rails).
            ToolKind::Pickaxe => {
                if matches!(material, M::Rock | M::Iron | M::Anvil)
                    || matches!(
                        block,
                        Block::Ice
                            | Block::PackedIce
                            | Block::Rail
                            | Block::PoweredRail
                            | Block::DetectorRail
                            | Block::ActivatorRail
                    )
                {
                    self.efficiency
                } else {
                    1.0
                }
            }
            // ItemAxe: wood/plants/vine materials, else EFFECTIVE_ON
            // (pumpkins, melon, ladder).
            ToolKind::Axe => {
                if matches!(material, M::Wood | M::Plants | M::Vine)
                    || matches!(
                        block,
                        Block::Pumpkin | Block::JackOLantern | Block::MelonBlock | Block::Ladder
                    )
                {
                    self.efficiency
                } else {
                    1.0
                }
            }
            // ItemSpade only has its EFFECTIVE_ON set.
            ToolKind::Shovel => {
                if matches!(
                    block,
                    Block::Clay
                        | Block::Dirt
                        | Block::Farmland
                        | Block::Grass
                        | Block::GrassSnowy
                        | Block::Gravel
                        | Block::Mycelium
                        | Block::Sand
                        | Block::SnowBlock
                        | Block::SnowLayer
                        | Block::SoulSand
                ) {
                    self.efficiency
                } else {
                    1.0
                }
            }
            // ItemSword: web 15, plant-like materials 1.5.
            ToolKind::Sword => {
                if block == Block::Cobweb {
                    15.0
                } else if matches!(material, M::Plants | M::Vine | M::Leaves | M::Gourd) {
                    1.5
                } else {
                    1.0
                }
            }
            // ItemShears: web/leaves 15, wool 5.
            ToolKind::Shears => {
                if block == Block::Cobweb || material == M::Leaves {
                    15.0
                } else if block == Block::Wool {
                    5.0
                } else {
                    1.0
                }
            }
            ToolKind::None => 1.0,
        }
    }

    /// Vanilla `Item.canHarvestBlock` per item class. Only consulted for
    /// tool-requiring materials (rock, iron, anvil, web, snow, crafted snow).
    fn can_harvest_block(self, block: Block) -> bool {
        use crate::world::block::material::VanillaMaterial as M;
        match self.kind {
            ToolKind::Pickaxe => match block {
                Block::Obsidian => self.harvest_level >= 3,
                Block::DiamondBlock
                | Block::DiamondOre
                | Block::EmeraldOre
                | Block::EmeraldBlock
                | Block::GoldBlock
                | Block::GoldOre
                | Block::RedstoneOre
                | Block::LitRedstoneOre => self.harvest_level >= 2,
                Block::IronBlock | Block::IronOre | Block::LapisBlock | Block::LapisOre => {
                    self.harvest_level >= 1
                }
                _ => matches!(block.material(), M::Rock | M::Iron | M::Anvil),
            },
            ToolKind::Shovel => matches!(block, Block::SnowLayer | Block::SnowBlock),
            ToolKind::Sword | ToolKind::Shears => block == Block::Cobweb,
            ToolKind::Axe | ToolKind::None => false,
        }
    }
}

static mut RAND_STATE: u32 = 0x6C078965;

fn rand_f32() -> f32 {
    unsafe {
        RAND_STATE ^= RAND_STATE << 13;
        RAND_STATE ^= RAND_STATE >> 17;
        RAND_STATE ^= RAND_STATE << 5;
        (RAND_STATE & 0x007FFFFF) as f32 / 8388607.0
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    #[test]
    fn vanilla_replaceable_blocks_keep_the_clicked_position() {
        assert!(block_is_replaceable_for_placement(Block::Air, 0));
        assert!(block_is_replaceable_for_placement(Block::TallGrass, 0));
        assert!(block_is_replaceable_for_placement(
            Block::SnowLayer,
            Block::SnowLayer.to_id() << 4,
        ));
        assert!(!block_is_replaceable_for_placement(
            Block::SnowLayer,
            (Block::SnowLayer.to_id() << 4) | 1,
        ));
        assert!(!block_is_replaceable_for_placement(Block::Stone, 0));
    }

    #[test]
    fn snow_layers_stack_in_place_until_the_eighth_layer() {
        let base = Block::SnowLayer.to_id() << 4;
        assert_eq!(
            stacked_snow_layer_state(Block::SnowLayer, base | 6, client::physics::BlockFace::Top),
            Some(base | 7)
        );
        assert_eq!(
            stacked_snow_layer_state(Block::SnowLayer, base | 7, client::physics::BlockFace::Top),
            None
        );
        assert_eq!(
            stacked_snow_layer_state(Block::SnowLayer, base, client::physics::BlockFace::East),
            Some(base | 1)
        );
        assert_eq!(
            stacked_snow_layer_state(Block::SnowLayer, base | 1, client::physics::BlockFace::East),
            None
        );
    }

    #[test]
    fn stair_half_prediction_uses_the_protocol_cursor_precision() {
        let hit_y = protocol_hit_fraction(0.5001);
        assert_eq!(hit_y, 0.5);
        assert!(!placement_selects_upper_half(
            client::physics::BlockFace::North,
            hit_y
        ));
        assert!(placement_selects_upper_half(
            client::physics::BlockFace::North,
            protocol_hit_fraction(0.5625)
        ));
    }

    #[test]
    fn predicted_logs_follow_the_clicked_axis() {
        let x_axis = predicted_item_block_state(
            Block::Log,
            Block::Log.to_id(),
            2,
            client::physics::BlockFace::East,
            0.0,
            0.5,
        );
        let z_axis = predicted_item_block_state(
            Block::Log,
            Block::Log.to_id(),
            2,
            client::physics::BlockFace::North,
            0.0,
            0.5,
        );
        assert_eq!(x_axis & 0x0f, 0x06);
        assert_eq!(z_axis & 0x0f, 0x0a);
    }

    #[test]
    fn predicted_stairs_face_the_placer_like_vanilla() {
        let meta = |mc_yaw: f32| {
            predicted_item_block_state(
                Block::OakStairs,
                Block::OakStairs.to_id(),
                0,
                client::physics::BlockFace::Top,
                mc_yaw,
                0.0,
            ) & 0x0f
        };
        // BlockStairs meta = 5 - FACING.getIndex(): S=2, W=1, N=3, E=0.
        assert_eq!(meta(0.0), 2);
        assert_eq!(meta(90.0), 1);
        assert_eq!(meta(180.0), 3);
        assert_eq!(meta(-180.0), 3);
        assert_eq!(meta(-90.0), 0);
    }

    #[test]
    fn predicted_stairs_and_slabs_flip_from_the_hit_height() {
        let stairs = |face, hit_y| {
            predicted_item_block_state(
                Block::OakStairs,
                Block::OakStairs.to_id(),
                0,
                face,
                0.0,
                hit_y,
            ) & 0x04
        };
        assert_eq!(stairs(client::physics::BlockFace::Top, 1.0), 0);
        assert_eq!(stairs(client::physics::BlockFace::Bottom, 0.0), 0x04);
        assert_eq!(stairs(client::physics::BlockFace::North, 0.25), 0);
        assert_eq!(stairs(client::physics::BlockFace::North, 0.75), 0x04);

        let slab = |face, hit_y| {
            predicted_item_block_state(
                Block::StoneSlab,
                Block::StoneSlab.to_id(),
                0,
                face,
                0.0,
                hit_y,
            ) & 0x08
        };
        assert_eq!(slab(client::physics::BlockFace::Top, 1.0), 0);
        assert_eq!(slab(client::physics::BlockFace::Bottom, 0.0), 0x08);
        assert_eq!(slab(client::physics::BlockFace::East, 0.75), 0x08);
    }

    #[test]
    fn predicted_directional_blocks_face_toward_the_placer() {
        let furnace = |mc_yaw: f32| {
            predicted_item_block_state(
                Block::Furnace,
                Block::Furnace.to_id(),
                0,
                client::physics::BlockFace::Top,
                mc_yaw,
                1.0,
            ) & 0x0f
        };
        // FACING = opposite horizontal facing, meta = EnumFacing.getIndex().
        assert_eq!(furnace(0.0), 2); // looking south → faces north
        assert_eq!(furnace(-90.0), 4); // looking east → faces west

        let pumpkin = |mc_yaw: f32| {
            predicted_item_block_state(
                Block::Pumpkin,
                Block::Pumpkin.to_id(),
                0,
                client::physics::BlockFace::Top,
                mc_yaw,
                1.0,
            ) & 0x0f
        };
        // meta = FACING.getHorizontalIndex() of the opposite facing.
        assert_eq!(pumpkin(0.0), 2);
        assert_eq!(pumpkin(90.0), 3);

        let ladder = predicted_item_block_state(
            Block::Ladder,
            Block::Ladder.to_id(),
            0,
            client::physics::BlockFace::South,
            0.0,
            0.5,
        ) & 0x0f;
        assert_eq!(ladder, 3);

        let wall_torch = predicted_item_block_state(
            Block::Torch,
            Block::Torch.to_id(),
            0,
            client::physics::BlockFace::East,
            0.0,
            0.5,
        ) & 0x0f;
        assert_eq!(wall_torch, 1);
        let standing_torch = predicted_item_block_state(
            Block::Torch,
            Block::Torch.to_id(),
            0,
            client::physics::BlockFace::Top,
            0.0,
            1.0,
        ) & 0x0f;
        assert_eq!(standing_torch, 5);
    }

    #[test]
    fn living_entities_and_vehicles_prevent_block_placement() {
        assert!(entity_prevents_block_placement(EntityType::Player));
        assert!(entity_prevents_block_placement(EntityType::Zombie));
        assert!(entity_prevents_block_placement(EntityType::Boat));
        assert!(!entity_prevents_block_placement(EntityType::Item));
        assert!(!entity_prevents_block_placement(EntityType::Arrow));
    }
}

#[cfg(test)]
mod item_use_input_tests {
    use super::{is_drinkable, is_splash_potion, plan_item_use_tick, ItemUseTickPlan};

    #[test]
    fn idle_press_and_release_starts_use_for_one_movement_tick() {
        // Both edges arrived between fixed ticks. Vanilla consumes the queued
        // press now, then notices the final released state on the next tick.
        assert_eq!(
            plan_item_use_tick(false, false, 1, 3, true, false),
            ItemUseTickPlan {
                queued_presses: 1,
                ..ItemUseTickPlan::default()
            }
        );
        assert_eq!(
            plan_item_use_tick(true, false, 0, 2, true, false),
            ItemUseTickPlan {
                release: true,
                discard_attacks: true,
                ..ItemUseTickPlan::default()
            }
        );
    }

    #[test]
    fn active_release_and_repress_keeps_blocking_when_final_state_is_held() {
        assert_eq!(
            plan_item_use_tick(true, true, 1, 0, true, false),
            ItemUseTickPlan {
                discard_attacks: true,
                ..ItemUseTickPlan::default()
            }
        );
    }

    #[test]
    fn queued_presses_bypass_delay_but_held_repeat_waits() {
        assert_eq!(
            plan_item_use_tick(false, false, 3, 4, true, false),
            ItemUseTickPlan {
                queued_presses: 3,
                ..ItemUseTickPlan::default()
            }
        );
        assert_eq!(
            plan_item_use_tick(false, true, 0, 1, true, false),
            ItemUseTickPlan::default()
        );
        assert_eq!(
            plan_item_use_tick(false, true, 0, 0, true, false),
            ItemUseTickPlan {
                held_repeat: true,
                ..ItemUseTickPlan::default()
            }
        );
        assert_eq!(
            plan_item_use_tick(false, true, 0, 0, false, false),
            ItemUseTickPlan::default()
        );
    }

    #[test]
    fn active_dig_consumes_right_clicks_without_using_or_repeating() {
        assert_eq!(
            plan_item_use_tick(false, true, 2, 0, true, true),
            ItemUseTickPlan::default()
        );
    }

    #[test]
    fn potion_metadata_and_milk_match_vanilla_use_actions() {
        assert!(is_drinkable(373, 0));
        assert!(!is_splash_potion(373, 0));
        assert!(is_splash_potion(373, 0x4000));
        assert!(!is_drinkable(373, 0x4000));
        assert!(is_drinkable(335, 0));
    }
}

// =====================================================================
