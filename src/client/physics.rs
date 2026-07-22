//! Player physics - AABB collision and movement.
//!
//! Values here are per 20 Hz Minecraft tick, matching the 1.8.9 entity code.

use crate::world::{block::Block, World};
use nalgebra::{Point3, Vector3};
use smallvec::{SmallVec, smallvec};

/// Inline capacity for collision-box collections. Most blocks produce at most
/// six collision boxes (e.g. Hopper/Cauldron), so this keeps the per-query
/// allocation off the heap in the common case.
type CollisionBoxes = SmallVec<[Aabb; 6]>;

/// Player physics constants (MC 1.8.9 values)
// Entity width/height/eye height are Java floats. Entity/AABB math promotes
// them to doubles, retaining the float rounding (for example 0.6F is not the
// same value as the f64 literal 0.6).
pub const PLAYER_WIDTH: f64 = 0.6_f32 as f64;
pub const PLAYER_HEIGHT: f64 = 1.8_f32 as f64;
pub const PLAYER_EYE_HEIGHT: f64 = 1.62_f32 as f64;

/// One Minecraft block spans this many texture/model pixels.
pub const PIXELS_PER_BLOCK: f64 = 16.0;
/// Same as [`PIXELS_PER_BLOCK`] but as `f32` for raycast math.
const PIXELS_PER_BLOCK_F32: f32 = 16.0;

/// Sneak edge-prevention step size (MC 1.8.9 Entity.moveEntity).
const SNEAK_EDGE_STEP: f64 = 0.05;

/// Maximum height a living entity can step up without jumping.
const STEP_HEIGHT: f64 = 0.6_f32 as f64;

/// Particle collision-box dimensions (width == height).
const PARTICLE_SIZE: f64 = 0.2_f32 as f64;

/// AABB for collision testing.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl Aabb {
    pub fn new(pos: &Point3<f64>, width: f64, height: f64) -> Self {
        let hw = width / 2.0;
        Aabb {
            min_x: pos.x - hw,
            min_y: pos.y,
            min_z: pos.z - hw,
            max_x: pos.x + hw,
            max_y: pos.y + height,
            max_z: pos.z + hw,
        }
    }

    /// Expand this AABB by the given velocity to get the swept volume.
    pub fn expand(&self, vel: &Vector3<f64>) -> Self {
        self.add_coord(vel.x, vel.y, vel.z)
    }

    pub fn offset(&self, x: f64, y: f64, z: f64) -> Self {
        Aabb {
            min_x: self.min_x + x,
            min_y: self.min_y + y,
            min_z: self.min_z + z,
            max_x: self.max_x + x,
            max_y: self.max_y + y,
            max_z: self.max_z + z,
        }
    }

    pub fn add_coord(&self, x: f64, y: f64, z: f64) -> Self {
        Aabb {
            min_x: self.min_x + x.min(0.0),
            min_y: self.min_y + y.min(0.0),
            min_z: self.min_z + z.min(0.0),
            max_x: self.max_x + x.max(0.0),
            max_y: self.max_y + y.max(0.0),
            max_z: self.max_z + z.max(0.0),
        }
    }

    pub fn overlaps_block(&self, bx: i32, by: i32, bz: i32) -> bool {
        let bx_f = bx as f64;
        let by_f = by as f64;
        let bz_f = bz as f64;
        self.min_x < bx_f + 1.0
            && self.max_x > bx_f
            && self.min_y < by_f + 1.0
            && self.max_y > by_f
            && self.min_z < bz_f + 1.0
            && self.max_z > bz_f
    }

    fn intersects(&self, other: &Aabb) -> bool {
        other.max_x > self.min_x
            && other.min_x < self.max_x
            && other.max_y > self.min_y
            && other.min_y < self.max_y
            && other.max_z > self.min_z
            && other.min_z < self.max_z
    }

    fn calculate_x_offset(&self, other: &Aabb, mut offset_x: f64) -> f64 {
        if other.max_y > self.min_y
            && other.min_y < self.max_y
            && other.max_z > self.min_z
            && other.min_z < self.max_z
        {
            if offset_x > 0.0 && other.max_x <= self.min_x {
                let d = self.min_x - other.max_x;
                if d < offset_x {
                    offset_x = d;
                }
            } else if offset_x < 0.0 && other.min_x >= self.max_x {
                let d = self.max_x - other.min_x;
                if d > offset_x {
                    offset_x = d;
                }
            }
        }
        offset_x
    }

    pub fn calculate_y_offset(&self, other: &Aabb, mut offset_y: f64) -> f64 {
        if other.max_x > self.min_x
            && other.min_x < self.max_x
            && other.max_z > self.min_z
            && other.min_z < self.max_z
        {
            if offset_y > 0.0 && other.max_y <= self.min_y {
                let d = self.min_y - other.max_y;
                if d < offset_y {
                    offset_y = d;
                }
            } else if offset_y < 0.0 && other.min_y >= self.max_y {
                let d = self.max_y - other.min_y;
                if d > offset_y {
                    offset_y = d;
                }
            }
        }
        offset_y
    }

    fn calculate_z_offset(&self, other: &Aabb, mut offset_z: f64) -> f64 {
        if other.max_x > self.min_x
            && other.min_x < self.max_x
            && other.max_y > self.min_y
            && other.min_y < self.max_y
        {
            if offset_z > 0.0 && other.max_z <= self.min_z {
                let d = self.min_z - other.max_z;
                if d < offset_z {
                    offset_z = d;
                }
            } else if offset_z < 0.0 && other.min_z >= self.max_z {
                let d = self.max_z - other.min_z;
                if d > offset_z {
                    offset_z = d;
                }
            }
        }
        offset_z
    }
}

fn collides_with_world(aabb: &Aabb, world: &World) -> bool {
    !get_colliding_boxes(aabb, world).is_empty()
}

fn block_aabb(bx: i32, by: i32, bz: i32) -> Aabb {
    Aabb {
        min_x: bx as f64,
        min_y: by as f64,
        min_z: bz as f64,
        max_x: bx as f64 + 1.0,
        max_y: by as f64 + 1.0,
        max_z: bz as f64 + 1.0,
    }
}

fn element_aabb(bx: i32, by: i32, bz: i32, from: [f32; 3], to: [f32; 3]) -> Aabb {
    Aabb {
        min_x: bx as f64 + from[0] as f64 / PIXELS_PER_BLOCK,
        min_y: by as f64 + from[1] as f64 / PIXELS_PER_BLOCK,
        min_z: bz as f64 + from[2] as f64 / PIXELS_PER_BLOCK,
        max_x: bx as f64 + to[0] as f64 / PIXELS_PER_BLOCK,
        max_y: by as f64 + to[1] as f64 / PIXELS_PER_BLOCK,
        max_z: bz as f64 + to[2] as f64 / PIXELS_PER_BLOCK,
    }
}

fn is_wood_fence(block: Block) -> bool {
    matches!(
        block,
        Block::OakFence
            | Block::SpruceFence
            | Block::BirchFence
            | Block::JungleFence
            | Block::DarkOakFence
            | Block::AcaciaFence
    )
}

fn is_fence_gate(block: Block) -> bool {
    matches!(
        block,
        Block::OakFenceGate
            | Block::SpruceFenceGate
            | Block::BirchFenceGate
            | Block::JungleFenceGate
            | Block::DarkOakFenceGate
            | Block::AcaciaFenceGate
    )
}

fn is_vanilla_full_cube_neighbor(block: Block) -> bool {
    block != Block::Barrier
        && !matches!(
            block,
            Block::Pumpkin | Block::JackOLantern | Block::MelonBlock
        )
        && block.properties().is_opaque
        && !crate::world::shape::has_custom_shape(block)
}

fn fence_connects(fence: Block, neighbor: Block) -> bool {
    is_fence_gate(neighbor)
        || (is_wood_fence(fence) && is_wood_fence(neighbor))
        || (fence == Block::NetherBrickFence && neighbor == Block::NetherBrickFence)
        || is_vanilla_full_cube_neighbor(neighbor)
}

fn fence_collision_boxes(world: &World, block: Block, bx: i32, by: i32, bz: i32) -> CollisionBoxes {
    let north = fence_connects(block, world.get_block(bx, by, bz - 1));
    let south = fence_connects(block, world.get_block(bx, by, bz + 1));
    let west = fence_connects(block, world.get_block(bx - 1, by, bz));
    let east = fence_connects(block, world.get_block(bx + 1, by, bz));
    fence_boxes_for_connections(bx, by, bz, north, south, west, east)
}

fn fence_boxes_for_connections(
    bx: i32,
    by: i32,
    bz: i32,
    north: bool,
    south: bool,
    west: bool,
    east: bool,
) -> CollisionBoxes {
    let mut boxes = SmallVec::new();

    if north || south {
        boxes.push(element_aabb(
            bx,
            by,
            bz,
            [6.0, 0.0, if north { 0.0 } else { 6.0 }],
            [10.0, 24.0, if south { 16.0 } else { 10.0 }],
        ));
    }
    if west || east || (!north && !south) {
        boxes.push(element_aabb(
            bx,
            by,
            bz,
            [if west { 0.0 } else { 6.0 }, 0.0, 6.0],
            [if east { 16.0 } else { 10.0 }, 24.0, 10.0],
        ));
    }
    boxes
}

fn wall_connects(neighbor: Block) -> bool {
    neighbor == Block::CobblestoneWall
        || is_fence_gate(neighbor)
        || is_vanilla_full_cube_neighbor(neighbor)
}

fn wall_collision_box(world: &World, bx: i32, by: i32, bz: i32) -> Aabb {
    let north = wall_connects(world.get_block(bx, by, bz - 1));
    let south = wall_connects(world.get_block(bx, by, bz + 1));
    let west = wall_connects(world.get_block(bx - 1, by, bz));
    let east = wall_connects(world.get_block(bx + 1, by, bz));
    wall_box_for_connections(bx, by, bz, north, south, west, east)
}

fn wall_box_for_connections(
    bx: i32,
    by: i32,
    bz: i32,
    north: bool,
    south: bool,
    west: bool,
    east: bool,
) -> Aabb {
    let mut min_x = if west { 0.0 } else { 4.0 };
    let mut max_x = if east { 16.0 } else { 12.0 };
    let mut min_z = if north { 0.0 } else { 4.0 };
    let mut max_z = if south { 16.0 } else { 12.0 };

    if north && south && !west && !east {
        min_x = 5.0;
        max_x = 11.0;
    } else if !north && !south && west && east {
        min_z = 5.0;
        max_z = 11.0;
    }
    element_aabb(bx, by, bz, [min_x, 0.0, min_z], [max_x, 24.0, max_z])
}

fn block_collision_boxes(world: &World, bx: i32, by: i32, bz: i32) -> CollisionBoxes {
    block_collision_boxes_for_state(world, bx, by, bz, world.get_block_state(bx, by, bz))
}

pub(crate) fn chest_bounds(
    world: &World,
    block: Block,
    bx: i32,
    by: i32,
    bz: i32,
) -> ([f32; 3], [f32; 3]) {
    let same_chest = |x, y, z| world.get_block(x, y, z) == block;
    let (min_x, max_x, min_z, max_z) = if block != Block::EnderChest && same_chest(bx, by, bz - 1) {
        (1.0, 15.0, 0.0, 15.0)
    } else if block != Block::EnderChest && same_chest(bx, by, bz + 1) {
        (1.0, 15.0, 1.0, 16.0)
    } else if block != Block::EnderChest && same_chest(bx - 1, by, bz) {
        (0.0, 15.0, 1.0, 15.0)
    } else if block != Block::EnderChest && same_chest(bx + 1, by, bz) {
        (1.0, 16.0, 1.0, 15.0)
    } else {
        (1.0, 15.0, 1.0, 15.0)
    };
    ([min_x, 0.0, min_z], [max_x, 14.0, max_z])
}

fn block_collision_boxes_for_state(
    world: &World,
    bx: i32,
    by: i32,
    bz: i32,
    block_state: u16,
) -> CollisionBoxes {
    let block = Block::from_state(block_state);
    if block == Block::Air || block.is_liquid() {
        return SmallVec::new();
    }

    // Decorative/plant blocks have no collision (walk through them)
    if is_non_collidable(block) {
        return SmallVec::new();
    }

    // These bounds are collision-specific in vanilla and differ from (or are
    // absent from) the render model used by block_elements.
    if block == Block::LilyPad {
        return smallvec![element_aabb(
            bx,
            by,
            bz,
            [0.0, 0.0, 0.0],
            [16.0, 0.25, 16.0],
        )];
    }
    if block == Block::FlowerPot {
        return smallvec![element_aabb(bx, by, bz, [5.0, 0.0, 5.0], [11.0, 6.0, 11.0])];
    }
    let metadata = (block_state & 0x0f) as u8;
    match block {
        Block::Chest | Block::TrappedChest | Block::EnderChest => {
            let (from, to) = chest_bounds(world, block, bx, by, bz);
            return smallvec![element_aabb(bx, by, bz, from, to)];
        }
        Block::SnowLayer => {
            // BlockSnow collision is one layer shorter than its selection and
            // render bounds. A single layer therefore has no collision.
            if metadata & 7 == 0 {
                return SmallVec::new();
            }
            return smallvec![element_aabb(
                bx,
                by,
                bz,
                [0.0, 0.0, 0.0],
                [16.0, ((metadata & 7) * 2) as f32, 16.0],
            )];
        }
        Block::Cactus => {
            return smallvec![element_aabb(
                bx,
                by,
                bz,
                [1.0, 0.0, 1.0],
                [15.0, 15.0, 15.0],
            )];
        }
        Block::SoulSand => {
            return smallvec![element_aabb(
                bx,
                by,
                bz,
                [0.0, 0.0, 0.0],
                [16.0, 14.0, 16.0],
            )];
        }
        Block::Farmland | Block::MobSpawner => return smallvec![block_aabb(bx, by, bz)],
        Block::Bed => {
            return smallvec![element_aabb(bx, by, bz, [0.0, 0.0, 0.0], [16.0, 9.0, 16.0])];
        }
        Block::Cake => {
            let min_x = 1.0 + (metadata & 7) as f32 * 2.0;
            return smallvec![element_aabb(
                bx,
                by,
                bz,
                [min_x, 0.0, 1.0],
                [15.0, 8.0, 15.0],
            )];
        }
        Block::BrewingStand => {
            return smallvec![
                element_aabb(bx, by, bz, [7.0, 0.0, 7.0], [9.0, 14.0, 9.0]),
                element_aabb(bx, by, bz, [0.0, 0.0, 0.0], [16.0, 2.0, 16.0]),
            ];
        }
        Block::Cauldron => {
            return smallvec![
                element_aabb(bx, by, bz, [0.0, 0.0, 0.0], [16.0, 5.0, 16.0]),
                element_aabb(bx, by, bz, [0.0, 0.0, 0.0], [2.0, 16.0, 16.0]),
                element_aabb(bx, by, bz, [0.0, 0.0, 0.0], [16.0, 16.0, 2.0]),
                element_aabb(bx, by, bz, [14.0, 0.0, 0.0], [16.0, 16.0, 16.0]),
                element_aabb(bx, by, bz, [0.0, 0.0, 14.0], [16.0, 16.0, 16.0]),
            ];
        }
        Block::Hopper => {
            return smallvec![
                element_aabb(bx, by, bz, [0.0, 0.0, 0.0], [16.0, 10.0, 16.0]),
                element_aabb(bx, by, bz, [0.0, 0.0, 0.0], [2.0, 16.0, 16.0]),
                element_aabb(bx, by, bz, [0.0, 0.0, 0.0], [16.0, 16.0, 2.0]),
                element_aabb(bx, by, bz, [14.0, 0.0, 0.0], [16.0, 16.0, 16.0]),
                element_aabb(bx, by, bz, [0.0, 0.0, 14.0], [16.0, 16.0, 16.0]),
            ];
        }
        Block::EndPortalFrame => {
            let mut boxes = smallvec![element_aabb(
                bx,
                by,
                bz,
                [0.0, 0.0, 0.0],
                [16.0, 13.0, 16.0],
            )];
            if metadata & 4 != 0 {
                boxes.push(element_aabb(
                    bx,
                    by,
                    bz,
                    [5.0, 13.0, 5.0],
                    [11.0, 16.0, 11.0],
                ));
            }
            return boxes;
        }
        Block::Anvil => {
            return if metadata & 1 == 0 {
                smallvec![element_aabb(
                    bx,
                    by,
                    bz,
                    [2.0, 0.0, 0.0],
                    [14.0, 16.0, 16.0],
                )]
            } else {
                smallvec![element_aabb(
                    bx,
                    by,
                    bz,
                    [0.0, 0.0, 2.0],
                    [16.0, 16.0, 14.0],
                )]
            };
        }
        _ => {}
    }
    if is_wood_fence(block) || block == Block::NetherBrickFence {
        return fence_collision_boxes(world, block, bx, by, bz);
    }
    if block == Block::CobblestoneWall {
        return smallvec![wall_collision_box(world, bx, by, bz)];
    }
    if is_fence_gate(block) {
        if metadata & 4 != 0 {
            return SmallVec::new();
        }
        return if metadata & 1 == 0 {
            smallvec![element_aabb(
                bx,
                by,
                bz,
                [0.0, 0.0, 6.0],
                [16.0, 24.0, 10.0],
            )]
        } else {
            smallvec![element_aabb(
                bx,
                by,
                bz,
                [6.0, 0.0, 0.0],
                [10.0, 24.0, 16.0],
            )]
        };
    }

    if let Some(elements) = crate::world::shape::block_elements(
        block,
        block_state,
        0,
        0,
        0,
        |dx, dy, dz| world.get_block(bx + dx, by + dy, bz + dz),
        |dx, dy, dz| world.get_block_state(bx + dx, by + dy, bz + dz),
    ) {
        elements
            .into_iter()
            .map(|elem| element_aabb(bx, by, bz, elem.from, elem.to))
            .collect()
    } else {
        smallvec![block_aabb(bx, by, bz)]
    }
}

fn is_non_collidable(block: Block) -> bool {
    matches!(
        block,
        Block::Dandelion
            | Block::Flower
            | Block::LargeFlower
            | Block::TallGrass
            | Block::DeadBush
            | Block::Sapling
            | Block::BrownMushroom
            | Block::RedMushroom
            | Block::Torch
            | Block::RedstoneTorch
            | Block::UnlitRedstoneTorch
            | Block::Fire
            | Block::RedstoneWire
            | Block::Vine
            | Block::Cobweb
            | Block::SugarCane
            | Block::Wheat
            | Block::Carrots
            | Block::Potatoes
            | Block::NetherWart
            | Block::PumpkinStem
            | Block::MelonStem
            | Block::Rail
            | Block::PoweredRail
            | Block::DetectorRail
            | Block::ActivatorRail
            | Block::Tripwire
            | Block::StandingSign
            | Block::WallSign
            | Block::Lever
            | Block::StoneButton
            | Block::WoodenButton
            | Block::StonePressurePlate
            | Block::WoodenPressurePlate
            | Block::LightWeightedPressurePlate
            | Block::HeavyWeightedPressurePlate
            | Block::NetherPortal
            | Block::EndPortal
    )
}

pub fn get_colliding_boxes(aabb: &Aabb, world: &World) -> CollisionBoxes {
    let min_bx = aabb.min_x.floor() as i32;
    let min_by = aabb.min_y.floor() as i32 - 1;
    let min_bz = aabb.min_z.floor() as i32;
    let max_bx = aabb.max_x.floor() as i32;
    let max_by = aabb.max_y.floor() as i32;
    let max_bz = aabb.max_z.floor() as i32;
    let mut boxes = SmallVec::new();

    // World.getCollidingBoundingBoxes iterates X -> Y -> Z. Keep the same
    // insertion order because moveEntity clips against this list in order.
    for bx in min_bx..=max_bx {
        for by in min_by..=max_by {
            for bz in min_bz..=max_bz {
                for block_box in block_collision_boxes(world, bx, by, bz) {
                    if block_box.intersects(aabb) {
                        boxes.push(block_box);
                    }
                }
            }
        }
    }

    boxes
}

fn get_movement_collisions(aabb: &Aabb, world: &World, entity_boxes: &[Aabb]) -> CollisionBoxes {
    let mut boxes = get_colliding_boxes(aabb, world);
    boxes.extend(
        entity_boxes
            .iter()
            .copied()
            .filter(|entity_box| entity_box.intersects(aabb)),
    );
    boxes
}

/// Move a non-stepping particle through block collision boxes.
///
/// This is the standard Y -> X -> Z portion of Minecraft 1.8.9's
/// `Entity.moveEntity`. Particles have no step height and never use sneak edge
/// prevention, but otherwise share the same clipping and collision flags.
pub fn move_particle_with_collision(
    pos: &mut Point3<f32>,
    velocity: &mut Vector3<f32>,
    delta: Vector3<f32>,
    world: &World,
    on_ground: &mut bool,
) {
    let original = delta.cast::<f64>();
    let mut x = original.x;
    let mut y = original.y;
    let mut z = original.z;
    let mut bb = Aabb::new(&pos.cast::<f64>(), PARTICLE_SIZE, PARTICLE_SIZE);
    let colliding = get_colliding_boxes(&bb.add_coord(x, y, z), world);

    for block_box in &colliding {
        y = block_box.calculate_y_offset(&bb, y);
    }
    bb = bb.offset(0.0, y, 0.0);
    for block_box in &colliding {
        x = block_box.calculate_x_offset(&bb, x);
    }
    bb = bb.offset(x, 0.0, 0.0);
    for block_box in &colliding {
        z = block_box.calculate_z_offset(&bb, z);
    }
    bb = bb.offset(0.0, 0.0, z);

    pos.x = ((bb.min_x + bb.max_x) * 0.5) as f32;
    pos.y = bb.min_y as f32;
    pos.z = ((bb.min_z + bb.max_z) * 0.5) as f32;

    let collided_x = original.x != x;
    let collided_y = original.y != y;
    let collided_z = original.z != z;
    *on_ground = collided_y && original.y < 0.0;
    if collided_x {
        velocity.x = 0.0;
    }
    if collided_y {
        velocity.y = 0.0;
    }
    if collided_z {
        velocity.z = 0.0;
    }
}

/// Vanilla `World.canBlockBePlaced` tests the prospective block's default
/// collision box before `ItemBlock.onItemUse` mutates the client world.
pub fn block_state_intersects_aabb(
    world: &World,
    block_state: u16,
    pos: (i32, i32, i32),
    aabb: &Aabb,
) -> bool {
    block_collision_boxes_for_state(world, pos.0, pos.1, pos.2, block_state)
        .into_iter()
        .any(|block_box| block_box.intersects(aabb))
}

/// Move the player with collision detection.
/// This mirrors MC 1.8.9 Entity.moveEntity including sneak edge-prevention.
/// Returns `true` if the player collided horizontally.
pub fn move_with_collision(
    pos: &mut Point3<f64>,
    vel: &mut Vector3<f64>,
    delta: &Vector3<f64>,
    width: f64,
    height: f64,
    world: &World,
    entity_boxes: &[Aabb],
    on_ground: &mut bool,
    sneaking: bool,
) -> bool {
    let original_delta = *delta;
    let mut x = delta.x;
    let mut y = delta.y;
    let mut z = delta.z;
    let bb = Aabb::new(pos, width, height);

    // MC 1.8.9 sneak edge-prevention (Entity.moveEntity lines 626-693).
    // When sneaking on the ground, reduce horizontal movement if there is no
    // supporting block below the destination — this keeps the player from
    // walking off block edges while sneaking.
    if *on_ground && sneaking {
        let step = SNEAK_EDGE_STEP;

        // Check X axis
        while x != 0.0
            && get_movement_collisions(&bb.offset(x, -1.0, 0.0), world, entity_boxes).is_empty()
        {
            if x.abs() < step {
                x = 0.0;
            } else if x > 0.0 {
                x -= step;
            } else {
                x += step;
            }
        }

        // Check Z axis
        while z != 0.0
            && get_movement_collisions(&bb.offset(0.0, -1.0, z), world, entity_boxes).is_empty()
        {
            if z.abs() < step {
                z = 0.0;
            } else if z > 0.0 {
                z -= step;
            } else {
                z += step;
            }
        }

        // Check combined X+Z
        while x != 0.0
            && z != 0.0
            && get_movement_collisions(&bb.offset(x, -1.0, z), world, entity_boxes).is_empty()
        {
            if x.abs() < step {
                x = 0.0;
            } else if x > 0.0 {
                x -= step;
            } else {
                x += step;
            }
            if z.abs() < step {
                z = 0.0;
            } else if z > 0.0 {
                z -= step;
            } else {
                z += step;
            }
        }
    }

    // Vanilla updates d3/d5 while sneak edge-prevention reduces x/z.  Those
    // reduced values, rather than the original requested movement, are used by
    // both horizontal-collision flags and the step-up paths.
    let d3 = x;
    let d4 = original_delta.y;
    let d5 = z;
    let mut bb = bb;

    // --- Standard collision resolution (Y → X → Z) ---
    let colliding = get_movement_collisions(&bb.add_coord(x, y, z), world, entity_boxes);

    for block_box in &colliding {
        y = block_box.calculate_y_offset(&bb, y);
    }
    bb = bb.offset(0.0, y, 0.0);

    for block_box in &colliding {
        x = block_box.calculate_x_offset(&bb, x);
    }
    bb = bb.offset(x, 0.0, 0.0);

    for block_box in &colliding {
        z = block_box.calculate_z_offset(&bb, z);
    }
    bb = bb.offset(0.0, 0.0, z);

    // --- Step-up (MC 1.8.9 Entity.moveEntity lines 721-813) ---
    // stepHeight: for players = 0.6 (EntityLivingBase.stepHeight)
    let step_height = STEP_HEIGHT;
    // flag1: "was on ground OR will land this tick"
    let flag1 = *on_ground || (d4 != y && d4 < 0.0);
    if step_height > 0.0 && flag1 && (d3 != x || d5 != z) {
        let d11 = x; // save resolved no-step X
        let d7 = y; // save resolved no-step Y
        let d8 = z; // save resolved no-step Z
        let no_step_bb = bb;
        let saved_bb = Aabb::new(pos, width, height);

        // --- Path 1: move up + horizontally, resolve Y on expanded box ---
        y = step_height;
        let list = get_movement_collisions(&saved_bb.add_coord(d3, y, d5), world, entity_boxes);
        let mut bb1 = saved_bb;
        let mut d15 = d3;
        let mut d16 = d5;
        let mut d9 = y;
        for block_box in &list {
            d9 = block_box.calculate_y_offset(&saved_bb.add_coord(d3, 0.0, d5), d9);
        }
        bb1 = bb1.offset(0.0, d9, 0.0);
        for block_box in &list {
            d15 = block_box.calculate_x_offset(&bb1, d15);
        }
        bb1 = bb1.offset(d15, 0.0, 0.0);
        for block_box in &list {
            d16 = block_box.calculate_z_offset(&bb1, d16);
        }
        bb1 = bb1.offset(0.0, 0.0, d16);

        // --- Path 2: move up from original, no horizontal pre-offset ---
        let mut bb2 = saved_bb;
        let mut d18 = d3;
        let mut d19 = d5;
        let mut d17 = y;
        for block_box in &list {
            d17 = block_box.calculate_y_offset(&saved_bb, d17);
        }
        bb2 = bb2.offset(0.0, d17, 0.0);
        for block_box in &list {
            d18 = block_box.calculate_x_offset(&bb2, d18);
        }
        bb2 = bb2.offset(d18, 0.0, 0.0);
        for block_box in &list {
            d19 = block_box.calculate_z_offset(&bb2, d19);
        }
        bb2 = bb2.offset(0.0, 0.0, d19);

        // Pick the better path
        let d20 = d15 * d15 + d16 * d16;
        let d10 = d18 * d18 + d19 * d19;
        let mut step_down;
        if d20 > d10 {
            x = d15;
            z = d16;
            bb = bb1;
            step_down = -d9;
        } else {
            x = d18;
            z = d19;
            bb = bb2;
            step_down = -d17;
        }

        // Vanilla reuses the swept collision list and clips the selected path's
        // negative rise, placing the BB back onto the stepped surface.
        for block_box in &list {
            step_down = block_box.calculate_y_offset(&bb, step_down);
        }
        y = step_down;
        bb = bb.offset(0.0, y, 0.0);

        // Fallback: if step-up didn't improve horizontal movement, revert
        // to the original no-step resolution (vanilla lines 806-812)
        if d11 * d11 + d8 * d8 >= x * x + z * z {
            x = d11;
            y = d7;
            z = d8;
            bb = no_step_bb;
        }
    }

    pos.x = (bb.min_x + bb.max_x) * 0.5;
    pos.y = bb.min_y;
    pos.z = (bb.min_z + bb.max_z) * 0.5;

    let collided_x = d3 != x;
    let collided_y = d4 != y;
    let collided_z = d5 != z;
    // on_ground = collided vertically while originally moving downward
    // (vanilla: isCollidedVertically && d4 < 0.0D, where d4 = original_delta.y)
    *on_ground = collided_y && d4 < 0.0;

    if collided_x {
        vel.x = 0.0;
    }
    if collided_y {
        vel.y = 0.0;
    }
    if collided_z {
        vel.z = 0.0;
    }

    collided_x || collided_z
}

#[cfg(test)]
mod collision_tests {
    use super::*;
    use crate::world::chunk::Chunk;

    fn test_world(block: Block, metadata: u8) -> World {
        let mut world = World::new();
        world.chunks.insert((0, 0), Chunk::new(0, 0).into());
        world.set_block_state(0, 0, 0, (block.to_id() << 4) | metadata as u16);
        world
    }

    #[test]
    fn vanilla_fence_collision_uses_two_full_height_strips() {
        let boxes = fence_boxes_for_connections(0, 0, 0, true, false, false, true);
        assert_eq!(boxes.len(), 2);
        assert_eq!((boxes[0].min_x, boxes[0].min_z), (0.375, 0.0));
        assert_eq!((boxes[0].max_x, boxes[0].max_z), (0.625, 0.625));
        assert_eq!((boxes[1].min_x, boxes[1].min_z), (0.375, 0.375));
        assert_eq!((boxes[1].max_x, boxes[1].max_z), (1.0, 0.625));
        assert!(boxes.iter().all(|aabb| aabb.max_y == 1.5));
    }

    #[test]
    fn vanilla_straight_wall_narrows_the_cross_axis() {
        let north_south = wall_box_for_connections(0, 0, 0, true, true, false, false);
        assert_eq!((north_south.min_x, north_south.max_x), (0.3125, 0.6875));
        assert_eq!((north_south.min_z, north_south.max_z), (0.0, 1.0));
        assert_eq!(north_south.max_y, 1.5);

        let east_west = wall_box_for_connections(0, 0, 0, false, false, true, true);
        assert_eq!((east_west.min_x, east_west.max_x), (0.0, 1.0));
        assert_eq!((east_west.min_z, east_west.max_z), (0.3125, 0.6875));
        assert_eq!(east_west.max_y, 1.5);
    }

    #[test]
    fn fence_material_connections_match_vanilla() {
        assert!(fence_connects(Block::OakFence, Block::BirchFence));
        assert!(!fence_connects(Block::OakFence, Block::NetherBrickFence));
        assert!(!fence_connects(Block::NetherBrickFence, Block::OakFence));
        assert!(fence_connects(
            Block::NetherBrickFence,
            Block::NetherBrickFence
        ));
        assert!(fence_connects(Block::OakFence, Block::OakFenceGate));
        assert!(!fence_connects(Block::OakFence, Block::Barrier));
    }

    #[test]
    fn snow_collision_is_one_layer_shorter_than_render_bounds() {
        assert!(block_collision_boxes(&test_world(Block::SnowLayer, 0), 0, 0, 0).is_empty());

        let boxes = block_collision_boxes(&test_world(Block::SnowLayer, 3), 0, 0, 0);
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0].max_y, 0.375);
    }

    #[test]
    fn prospective_placement_uses_the_new_blocks_shape_without_mutating_world() {
        let world = test_world(Block::Stone, 0);
        let player_box = Aabb::new(&Point3::new(1.5, 0.0, 0.5), PLAYER_WIDTH, PLAYER_HEIGHT);

        assert!(block_state_intersects_aabb(
            &world,
            Block::Stone.to_id() << 4,
            (1, 0, 0),
            &player_box,
        ));
        assert!(!block_state_intersects_aabb(
            &world,
            Block::SnowLayer.to_id() << 4,
            (1, 0, 0),
            &player_box,
        ));
        assert_eq!(world.get_block(1, 0, 0), Block::Air);
    }

    #[test]
    fn special_collision_boxes_do_not_reuse_render_geometry() {
        let cactus = block_collision_boxes(&test_world(Block::Cactus, 0), 0, 0, 0);
        assert_eq!((cactus[0].min_x, cactus[0].max_y), (0.0625, 0.9375));

        let soul_sand = block_collision_boxes(&test_world(Block::SoulSand, 0), 0, 0, 0);
        assert_eq!(soul_sand[0].max_y, 0.875);

        let cauldron = block_collision_boxes(&test_world(Block::Cauldron, 0), 0, 0, 0);
        assert_eq!(cauldron.len(), 5);
        assert_eq!(cauldron[0].max_y, 0.3125);

        let hopper = block_collision_boxes(&test_world(Block::Hopper, 0), 0, 0, 0);
        assert_eq!(hopper.len(), 5);
        assert_eq!(hopper[0].max_y, 0.625);
    }

    #[test]
    fn state_dependent_collision_boxes_match_vanilla() {
        let cake = block_collision_boxes(&test_world(Block::Cake, 3), 0, 0, 0);
        assert_eq!((cake[0].min_x, cake[0].max_y), (0.4375, 0.5));

        let eye_frame = block_collision_boxes(&test_world(Block::EndPortalFrame, 4), 0, 0, 0);
        assert_eq!(eye_frame.len(), 2);
        assert_eq!(eye_frame[0].max_y, 0.8125);
        assert_eq!(eye_frame[1].min_y, 0.8125);

        let open_gate = block_collision_boxes(&test_world(Block::OakFenceGate, 4), 0, 0, 0);
        assert!(open_gate.is_empty());
    }

    #[test]
    fn authoritative_aabb_retains_double_precision_at_world_border_scale() {
        let x = 30_000_000.0 + 1.0 / 1024.0;
        let bb = Aabb::new(&Point3::new(x, 64.0, -x), PLAYER_WIDTH, PLAYER_HEIGHT);
        let half_width = PLAYER_WIDTH / 2.0;
        assert_eq!(bb.min_x, x - half_width);
        assert_eq!(bb.max_z, -x + half_width);
        // The equivalent f32 conversion loses this fractional coordinate.
        assert_ne!(x as f32 as f64, x);
    }

    #[test]
    fn entity_dimensions_preserve_vanilla_float_rounding_before_double_math() {
        assert_eq!(PLAYER_WIDTH, 0.6_f32 as f64);
        assert_eq!(PLAYER_HEIGHT, 1.8_f32 as f64);
        assert_ne!(PLAYER_WIDTH, 0.6_f64);
    }

    #[test]
    fn movement_queries_append_intersecting_entity_boxes_after_blocks() {
        let world = test_world(Block::Stone, 0);
        let query = Aabb::new(&Point3::new(0.5, 0.9, 0.5), PLAYER_WIDTH, PLAYER_HEIGHT);
        let boat = Aabb::new(&Point3::new(0.5, 1.0, 0.5), 1.5_f32 as f64, 0.6_f32 as f64);
        let boxes = get_movement_collisions(&query, &world, &[boat]);
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes.last().unwrap().min_x, boat.min_x);
        assert_eq!(boxes.last().unwrap().max_y, boat.max_y);
    }

    #[test]
    fn raycast_uses_the_slab_shape_instead_of_its_full_voxel() {
        let world = test_world(Block::StoneSlab, 0);
        let direction = Vector3::new(1.0, 0.0, 0.0);

        let hit = raycast(&Point3::new(-1.0, 0.25, 0.5), &direction, 3.0, &world);
        assert_eq!(hit.unwrap().pos, (0, 0, 0));

        let above_slab = raycast(&Point3::new(-1.0, 0.75, 0.5), &direction, 3.0, &world);
        assert!(above_slab.is_none());
    }

    #[test]
    fn raycast_passes_through_fire_to_the_solid_block_behind_it() {
        let mut world = test_world(Block::Fire, 0);
        world.set_block(1, 0, 0, Block::Stone);

        let hit = raycast(
            &Point3::new(-1.0, 0.5, 0.5),
            &Vector3::new(1.0, 0.0, 0.0),
            4.0,
            &world,
        );

        assert_eq!(hit.unwrap().pos, (1, 0, 0));
    }

    #[test]
    fn plant_raycast_faces_use_vanilla_selection_bounds() {
        let cases = [
            (
                Point3::new(-1.0, 0.4, 0.5),
                Vector3::new(1.0, 0.0, 0.0),
                BlockFace::West,
            ),
            (
                Point3::new(2.0, 0.4, 0.5),
                Vector3::new(-1.0, 0.0, 0.0),
                BlockFace::East,
            ),
            (
                Point3::new(0.5, 0.4, -1.0),
                Vector3::new(0.0, 0.0, 1.0),
                BlockFace::North,
            ),
            (
                Point3::new(0.5, 0.4, 2.0),
                Vector3::new(0.0, 0.0, -1.0),
                BlockFace::South,
            ),
        ];

        for block in [Block::TallGrass, Block::LargeFlower] {
            let world = test_world(block, 0);
            for (origin, direction, expected_face) in cases {
                let hit = raycast(&origin, &direction, 4.0, &world).unwrap();
                assert_eq!(hit.pos, (0, 0, 0));
                assert_eq!(hit.face, expected_face, "block={block:?}");
            }
        }
    }

    #[test]
    fn raycast_faces_match_vanilla_aabb_intercepts() {
        let world = test_world(Block::Stone, 0);
        let cases = [
            (
                Point3::new(-1.0, 0.5, 0.5),
                Vector3::new(1.0, 0.0, 0.0),
                BlockFace::West,
            ),
            (
                Point3::new(2.0, 0.5, 0.5),
                Vector3::new(-1.0, 0.0, 0.0),
                BlockFace::East,
            ),
            (
                Point3::new(0.5, -1.0, 0.5),
                Vector3::new(0.0, 1.0, 0.0),
                BlockFace::Bottom,
            ),
            (
                Point3::new(0.5, 2.0, 0.5),
                Vector3::new(0.0, -1.0, 0.0),
                BlockFace::Top,
            ),
            (
                Point3::new(0.5, 0.5, -1.0),
                Vector3::new(0.0, 0.0, 1.0),
                BlockFace::North,
            ),
            (
                Point3::new(0.5, 0.5, 2.0),
                Vector3::new(0.0, 0.0, -1.0),
                BlockFace::South,
            ),
        ];

        for (origin, direction, expected_face) in cases {
            assert_eq!(
                raycast(&origin, &direction, 3.0, &world).unwrap().face,
                expected_face
            );
        }
    }
}

/// Block position in world coordinates.
pub type BlockPos = (i32, i32, i32);

/// Which face of a block was hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockFace {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

impl BlockFace {
    /// Normal vector for this face.
    pub fn normal(&self) -> (i32, i32, i32) {
        match self {
            BlockFace::Top => (0, 1, 0),
            BlockFace::Bottom => (0, -1, 0),
            BlockFace::North => (0, 0, -1),
            BlockFace::South => (0, 0, 1),
            BlockFace::East => (1, 0, 0),
            BlockFace::West => (-1, 0, 0),
        }
    }
}

/// Result of a raycast against the world.
#[derive(Clone, Copy, Debug)]
pub struct RaycastHit {
    pub pos: BlockPos,
    pub face: BlockFace,
    pub distance: f32,
}

/// DDA raycast from origin along direction, up to max_dist blocks.
/// Returns the first solid block hit and which face was hit.
pub fn raycast(
    origin: &Point3<f32>,
    direction: &Vector3<f32>,
    max_dist: f32,
    world: &World,
) -> Option<RaycastHit> {
    if direction.magnitude_squared() < 1e-10 {
        return None;
    }

    let dir = direction.normalize();

    // Current block coordinates
    let mut x = origin.x.floor() as i32;
    let mut y = origin.y.floor() as i32;
    let mut z = origin.z.floor() as i32;

    // Step direction (-1 or +1)
    let step_x = if dir.x >= 0.0 { 1 } else { -1 };
    let step_y = if dir.y >= 0.0 { 1 } else { -1 };
    let step_z = if dir.z >= 0.0 { 1 } else { -1 };

    // tMax: distance to next block boundary on each axis
    let t_delta_x = if dir.x.abs() > 1e-10 {
        1.0 / dir.x.abs()
    } else {
        f32::MAX
    };
    let t_delta_y = if dir.y.abs() > 1e-10 {
        1.0 / dir.y.abs()
    } else {
        f32::MAX
    };
    let t_delta_z = if dir.z.abs() > 1e-10 {
        1.0 / dir.z.abs()
    } else {
        f32::MAX
    };

    let mut t_max_x = if dir.x.abs() > 1e-10 {
        let boundary = if step_x > 0 { (x + 1) as f32 } else { x as f32 };
        (boundary - origin.x) / dir.x
    } else {
        f32::MAX
    };
    let mut t_max_y = if dir.y.abs() > 1e-10 {
        let boundary = if step_y > 0 { (y + 1) as f32 } else { y as f32 };
        (boundary - origin.y) / dir.y
    } else {
        f32::MAX
    };
    let mut t_max_z = if dir.z.abs() > 1e-10 {
        let boundary = if step_z > 0 { (z + 1) as f32 } else { z as f32 };
        (boundary - origin.z) / dir.z
    } else {
        f32::MAX
    };

    // Step through the grid
    for _ in 0..(max_dist * 3.0) as i32 + 1 {
        let block = world.get_block(x, y, z);
        // BlockFire.isCollidable() is false, so vanilla ray tracing skips it.
        if block != Block::Air && !block.is_liquid() && block != Block::Fire {
            let state = world.get_block_state(x, y, z);
            let mut nearest = None;
            if matches!(
                block,
                Block::Chest | Block::TrappedChest | Block::EnderChest
            ) {
                let (local_min, local_max) = chest_bounds(world, block, x, y, z);
                nearest = ray_aabb_hit(
                    *origin,
                    dir,
                    [
                        x as f32 + local_min[0] / PIXELS_PER_BLOCK_F32,
                        y as f32 + local_min[1] / PIXELS_PER_BLOCK_F32,
                        z as f32 + local_min[2] / PIXELS_PER_BLOCK_F32,
                    ],
                    [
                        x as f32 + local_max[0] / PIXELS_PER_BLOCK_F32,
                        y as f32 + local_max[1] / PIXELS_PER_BLOCK_F32,
                        z as f32 + local_max[2] / PIXELS_PER_BLOCK_F32,
                    ],
                );
            } else if let Some((local_min, local_max)) = vanilla_raycast_bounds(block) {
                nearest = ray_aabb_hit(
                    *origin,
                    dir,
                    [
                        x as f32 + local_min[0],
                        y as f32 + local_min[1],
                        z as f32 + local_min[2],
                    ],
                    [
                        x as f32 + local_max[0],
                        y as f32 + local_max[1],
                        z as f32 + local_max[2],
                    ],
                );
            } else {
                let elements = crate::world::shape::block_elements(
                    block,
                    state,
                    0,
                    0,
                    0,
                    |dx, dy, dz| world.get_block(x + dx, y + dy, z + dz),
                    |dx, dy, dz| world.get_block_state(x + dx, y + dy, z + dz),
                );
                if let Some(elements) = elements {
                    for element in elements {
                        let min = [
                            x as f32 + element.from[0] / PIXELS_PER_BLOCK_F32,
                            y as f32 + element.from[1] / PIXELS_PER_BLOCK_F32,
                            z as f32 + element.from[2] / PIXELS_PER_BLOCK_F32,
                        ];
                        let max = [
                            x as f32 + element.to[0] / PIXELS_PER_BLOCK_F32,
                            y as f32 + element.to[1] / PIXELS_PER_BLOCK_F32,
                            z as f32 + element.to[2] / PIXELS_PER_BLOCK_F32,
                        ];
                        if let Some((distance, face)) = ray_aabb_hit(*origin, dir, min, max) {
                            if distance <= max_dist
                                && nearest
                                    .is_none_or(|(nearest_distance, _)| distance < nearest_distance)
                            {
                                nearest = Some((distance, face));
                            }
                        }
                    }
                } else {
                    nearest = ray_aabb_hit(
                        *origin,
                        dir,
                        [x as f32, y as f32, z as f32],
                        [x as f32 + 1.0, y as f32 + 1.0, z as f32 + 1.0],
                    );
                }
            }
            if let Some((distance, face)) = nearest {
                if distance <= max_dist {
                    return Some(RaycastHit {
                        pos: (x, y, z),
                        face,
                        distance,
                    });
                }
            }
        }

        // Advance to next block boundary
        let distance_to_next_cell;
        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                distance_to_next_cell = t_max_x;
                x += step_x;
                t_max_x += t_delta_x;
            } else {
                distance_to_next_cell = t_max_z;
                z += step_z;
                t_max_z += t_delta_z;
            }
        } else {
            if t_max_y < t_max_z {
                distance_to_next_cell = t_max_y;
                y += step_y;
                t_max_y += t_delta_y;
            } else {
                distance_to_next_cell = t_max_z;
                z += step_z;
                t_max_z += t_delta_z;
            }
        }

        if distance_to_next_cell > max_dist {
            break;
        }
    }

    None
}

/// Bounds used by Block.collisionRayTrace for plants whose rendered crossed
/// quads are not their selectable AABB. Values retain vanilla f32 rounding.
fn vanilla_raycast_bounds(block: Block) -> Option<([f32; 3], [f32; 3])> {
    let centered = |radius: f32, height: f32| {
        (
            [0.5_f32 - radius, 0.0, 0.5_f32 - radius],
            [0.5_f32 + radius, height, 0.5_f32 + radius],
        )
    };

    match block {
        Block::TallGrass | Block::DeadBush => Some(centered(0.4, 0.8)),
        Block::Sapling => Some(centered(0.4, 0.4_f32 * 2.0)),
        Block::LargeFlower => Some(([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])),
        Block::Dandelion | Block::Flower => Some(centered(0.2, 0.2_f32 * 3.0)),
        Block::BrownMushroom | Block::RedMushroom => Some(centered(0.2, 0.2_f32 * 2.0)),
        Block::SugarCane => Some(centered(0.375, 1.0)),
        Block::Wheat | Block::Carrots | Block::Potatoes | Block::NetherWart => {
            Some(centered(0.5, 0.25))
        }
        _ => None,
    }
}

fn ray_aabb_hit(
    origin: Point3<f32>,
    direction: Vector3<f32>,
    min: [f32; 3],
    max: [f32; 3],
) -> Option<(f32, BlockFace)> {
    // Exact `AxisAlignedBB.calculateIntercept` plane order from 1.8.9. A ray
    // starting inside an AABB must choose its nearest exit face, not an
    // arbitrary entry face from a slab intersection.
    let mut nearest: Option<(f32, BlockFace)> = None;
    let mut consider = |distance: f32,
                        face: BlockFace,
                        a: f32,
                        b: f32,
                        a_range: [f32; 2],
                        b_range: [f32; 2]| {
        if distance < 0.0 || a < a_range[0] || a > a_range[1] || b < b_range[0] || b > b_range[1] {
            return;
        }
        if nearest.is_none_or(|(nearest_distance, _)| distance < nearest_distance) {
            nearest = Some((distance, face));
        }
    };

    if direction.x.abs() >= 1e-8 {
        let distance = (min[0] - origin.x) / direction.x;
        consider(
            distance,
            BlockFace::West,
            origin.y + direction.y * distance,
            origin.z + direction.z * distance,
            [min[1], max[1]],
            [min[2], max[2]],
        );
        let distance = (max[0] - origin.x) / direction.x;
        consider(
            distance,
            BlockFace::East,
            origin.y + direction.y * distance,
            origin.z + direction.z * distance,
            [min[1], max[1]],
            [min[2], max[2]],
        );
    }
    if direction.y.abs() >= 1e-8 {
        let distance = (min[1] - origin.y) / direction.y;
        consider(
            distance,
            BlockFace::Bottom,
            origin.x + direction.x * distance,
            origin.z + direction.z * distance,
            [min[0], max[0]],
            [min[2], max[2]],
        );
        let distance = (max[1] - origin.y) / direction.y;
        consider(
            distance,
            BlockFace::Top,
            origin.x + direction.x * distance,
            origin.z + direction.z * distance,
            [min[0], max[0]],
            [min[2], max[2]],
        );
    }
    if direction.z.abs() >= 1e-8 {
        let distance = (min[2] - origin.z) / direction.z;
        consider(
            distance,
            BlockFace::North,
            origin.x + direction.x * distance,
            origin.y + direction.y * distance,
            [min[0], max[0]],
            [min[1], max[1]],
        );
        let distance = (max[2] - origin.z) / direction.z;
        consider(
            distance,
            BlockFace::South,
            origin.x + direction.x * distance,
            origin.y + direction.y * distance,
            [min[0], max[0]],
            [min[1], max[1]],
        );
    }

    nearest
}
