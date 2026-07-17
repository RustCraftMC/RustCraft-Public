//! Non-full block shape generation — slabs, stairs, fences, snow layers, etc.
//!
//! Each shape is defined as a set of cuboid elements (from, to) with per-face
//! texture tiles. These replace the default 1×1×1 cube in the mesh builder.

use super::block::Block;

/// A cuboid element in block-space (0..16 coordinates).
#[derive(Clone, Debug)]
pub struct BlockElement {
    pub from: [f32; 3],
    pub to: [f32; 3],
    /// Texture tiles for each face (top, bottom, side)
    pub tile_top: usize,
    pub tile_bottom: usize,
    pub tile_side: usize,
    /// Per-face UVs: [top, bottom, north, south, west, east].
    /// Each is Some([p0_uv, p1_uv, p2_uv, p3_uv]) in 0..1 tile-local space,
    /// or None for auto-UV (proportional to face size).
    pub uvs: [Option<[[f32; 2]; 4]>; 6],
    /// Optional per-face atlas tiles in [top, bottom, north, south, west, east] order.
    pub face_tiles: [Option<usize>; 6],
    /// Faces emitted for this element. Flat model elements such as fire own a
    /// single facing rather than rendering a duplicate backface and edge faces.
    pub visible_faces: [bool; 6],
    /// Rotation around the block centre, used by standing signs.
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
}

impl BlockElement {
    pub fn new(from: [f32; 3], to: [f32; 3], tile: (usize, usize, usize)) -> Self {
        BlockElement {
            from,
            to,
            tile_top: tile.0,
            tile_bottom: tile.1,
            tile_side: tile.2,
            uvs: [None; 6],
            face_tiles: [None; 6],
            visible_faces: [true; 6],
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
        }
    }

    fn rotated_y(mut self, degrees: f32) -> Self {
        self.rotation_y = degrees;
        self
    }

    fn rotated_x(mut self, degrees: f32) -> Self {
        self.rotation_x = degrees;
        self
    }

    fn rotated_z(mut self, degrees: f32) -> Self {
        self.rotation_z = degrees;
        self
    }

    fn with_visible_face(mut self, face: usize) -> Self {
        self.visible_faces = [false; 6];
        self.visible_faces[face] = true;
        self
    }

    fn with_visible_faces(mut self, faces: [bool; 6]) -> Self {
        self.visible_faces = faces;
        self
    }

    fn with_face_tiles(mut self, tiles: [usize; 6]) -> Self {
        self.face_tiles = tiles.map(Some);
        self
    }

    fn with_face_uv_extents(mut self, extents: [(f32, f32); 6]) -> Self {
        self.uvs = extents.map(|(u, v)| Some([[0.0, 0.0], [u, 0.0], [u, v], [0.0, v]]));
        self
    }

    fn with_face_uvs(mut self, uvs: [[[f32; 2]; 4]; 6]) -> Self {
        self.uvs = uvs.map(Some);
        self
    }

    fn with_full_face_uvs(mut self) -> Self {
        self.uvs = [Some([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]); 6];
        self
    }

    pub fn face_tile(&self, face: usize, fallback: usize) -> usize {
        self.face_tiles[face].unwrap_or(fallback)
    }
}

/// Check if a block has non-full geometry and return its custom shape elements.
/// Returns None for full cube blocks.
pub fn block_elements(
    block: Block,
    state: u16,
    x: usize,
    y: usize,
    z: usize,
    neighbor: impl Fn(i32, i32, i32) -> Block + Copy,
    neighbor_state: impl Fn(i32, i32, i32) -> u16 + Copy,
) -> Option<Vec<BlockElement>> {
    let (top, bot, side) = block.tiles();
    let tile = (top, bot, side);
    let metadata = (state & 0x0f) as u8;

    match block {
        Block::DoubleStoneSlab | Block::DoubleWoodSlab => None,

        // Slabs: bit 3 marks the top half in MC 1.8.9.
        Block::StoneSlab | Block::StoneSlab2 | Block::WoodSlab => {
            let (from_y, to_y) = if metadata & 0x08 != 0 {
                (8.0, 16.0)
            } else {
                (0.0, 8.0)
            };
            Some(vec![BlockElement::new(
                [0.0, from_y, 0.0],
                [16.0, to_y, 16.0],
                tile,
            )])
        }

        // Stairs: 1.8.9 metadata stores facing + half; shape is derived from neighbor stairs.
        Block::OakStairs
        | Block::SpruceStairs
        | Block::BirchStairs
        | Block::JungleStairs
        | Block::AcaciaStairs
        | Block::DarkOakStairs
        | Block::BrickStairs
        | Block::StoneBrickStairs
        | Block::SandstoneStairs
        | Block::RedSandstoneStairs
        | Block::NetherBrickStairs
        | Block::QuartzStairs
        | Block::CobblestoneStairs => Some(stair_elements(state, x, y, z, tile, neighbor_state)),

        // Fences connect to solid blocks, fences, fence gates and walls.
        Block::OakFence
        | Block::SpruceFence
        | Block::BirchFence
        | Block::JungleFence
        | Block::DarkOakFence
        | Block::AcaciaFence
        | Block::NetherBrickFence => {
            let connects = horizontal_connectivity(x, y, z, neighbor, connects_to_fence);
            Some(fence_elements(connects, tile))
        }

        // Fence gates: bit 2 open, low two bits orientation.
        Block::OakFenceGate
        | Block::SpruceFenceGate
        | Block::BirchFenceGate
        | Block::JungleFenceGate
        | Block::DarkOakFenceGate
        | Block::AcaciaFenceGate => Some(fence_gate_elements(metadata, tile)),

        // Snow layer metadata stores layers - 1.
        Block::SnowLayer => Some(vec![BlockElement::new(
            [0.0, 0.0, 0.0],
            [16.0, ((metadata & 0x07) as f32 + 1.0) * 2.0, 16.0],
            tile,
        )]),

        // Glass panes and iron bars connect to panes, bars and solid faces.
        Block::GlassPane | Block::StainedGlassPane | Block::IronBars => {
            let connects = horizontal_connectivity(x, y, z, neighbor, connects_to_pane);
            Some(pane_elements(connects, tile))
        }

        // Carpet (thin layer)
        Block::Carpet => Some(vec![BlockElement::new(
            [0.0, 0.0, 0.0],
            [16.0, 1.0, 16.0],
            tile,
        )]),

        // Cobweb (cross shape)
        Block::Cobweb => Some(vec![
            BlockElement::new([6.0, 0.0, 6.0], [10.0, 16.0, 10.0], tile),
            BlockElement::new([0.0, 6.0, 0.0], [16.0, 10.0, 16.0], tile),
        ]),

        // Wall — thin wall segment
        Block::CobblestoneWall => {
            let connects = horizontal_connectivity(x, y, z, neighbor, connects_to_wall);
            Some(wall_elements(connects, tile))
        }

        // Door: lower half stores facing/open, upper half stores hinge/powered.
        Block::OakDoor
        | Block::SpruceDoor
        | Block::BirchDoor
        | Block::JungleDoor
        | Block::AcaciaDoor
        | Block::DarkOakDoor
        | Block::IronDoor => Some(door_elements(state, x, y, z, tile, neighbor_state)),

        // Trapdoor: bit 2 open, bit 3 top half, low two bits facing.
        Block::Trapdoor | Block::IronTrapdoor => Some(trapdoor_elements(metadata, tile)),

        // Lily pad — thin floating layer
        Block::LilyPad => Some(vec![BlockElement::new(
            [0.0, 0.0, 0.0],
            [16.0, 1.0, 16.0],
            tile,
        )]),

        // Cactus — center cylinder (approximated as a centered box)
        Block::Cactus => Some(vec![BlockElement::new(
            [2.0, 0.0, 2.0],
            [14.0, 16.0, 14.0],
            tile,
        )]),

        // Sugar cane — thin post
        Block::SugarCane => Some(vec![BlockElement::new(
            [5.0, 0.0, 5.0],
            [11.0, 16.0, 11.0],
            tile,
        )]),

        // BlockVine metadata: bit 0=south, 1=west, 2=north, 3=east.
        // UP is an actual-state property, derived from the normal cube above.
        // Vanilla's vine models use a 0.05-block inset and emit both sides of
        // every sheet; the double-sided faces are required because cutout vines
        // must be visible from either side of the supporting wall.
        Block::Vine => {
            let mut elements = Vec::with_capacity(5);
            if metadata & 0x01 != 0 {
                elements.push(
                    BlockElement::new([0.0, 0.0, 15.2], [16.0, 16.0, 15.2], tile)
                        .with_visible_faces([false, false, true, true, false, false]),
                );
            }
            if metadata & 0x02 != 0 {
                elements.push(
                    BlockElement::new([0.8, 0.0, 0.0], [0.8, 16.0, 16.0], tile)
                        .with_visible_faces([false, false, false, false, true, true]),
                );
            }
            if metadata & 0x04 != 0 {
                elements.push(
                    BlockElement::new([0.0, 0.0, 0.8], [16.0, 16.0, 0.8], tile)
                        .with_visible_faces([false, false, true, true, false, false]),
                );
            }
            if metadata & 0x08 != 0 {
                elements.push(
                    BlockElement::new([15.2, 0.0, 0.0], [15.2, 16.0, 16.0], tile)
                        .with_visible_faces([false, false, false, false, true, true]),
                );
            }
            if neighbor(x as i32, y as i32 + 1, z as i32)
                .properties()
                .is_opaque
            {
                elements.push(
                    BlockElement::new([0.0, 15.2, 0.0], [16.0, 15.2, 16.0], tile)
                        .with_visible_faces([true, true, false, false, false, false]),
                );
            }
            Some(elements)
        }

        // Torch — cross shape (two intersecting quads)
        Block::Torch | Block::RedstoneTorch | Block::UnlitRedstoneTorch => Some(vec![
            BlockElement::new([6.0, 0.0, 7.0], [10.0, 12.0, 9.0], tile),
            BlockElement::new([7.0, 0.0, 6.0], [9.0, 12.0, 10.0], tile),
        ]),

        // Flowers and plants — cross shape
        Block::Dandelion | Block::Flower | Block::BrownMushroom | Block::RedMushroom => Some(vec![
            BlockElement::new([2.0, 0.0, 5.0], [14.0, 12.0, 11.0], tile),
            BlockElement::new([5.0, 0.0, 2.0], [11.0, 12.0, 14.0], tile),
        ]),

        // Sapling — cross shape (smaller)
        Block::Sapling => Some(vec![
            BlockElement::new([4.0, 0.0, 6.0], [12.0, 12.0, 10.0], tile),
            BlockElement::new([6.0, 0.0, 4.0], [10.0, 12.0, 12.0], tile),
        ]),

        // Tall grass / dead bush — thin cross planes (like vanilla cross model)
        Block::TallGrass | Block::DeadBush => {
            Some(vec![
                // Thin plane 1: from [0.8, 0, 8] to [15.2, 16, 8] — near-zero thickness on Z axis
                BlockElement::new([0.8, 0.0, 8.0], [15.2, 16.0, 8.0], tile),
                // Thin plane 2: from [8, 0, 0.8] to [8, 16, 15.2] — near-zero thickness on X axis
                BlockElement::new([8.0, 0.0, 0.8], [8.0, 16.0, 15.2], tile),
            ])
        }

        // Ladder — flat panel on block face
        Block::Ladder => {
            match metadata & 0x07 {
                2 => Some(vec![BlockElement::new(
                    [0.0, 0.0, 14.0],
                    [16.0, 16.0, 16.0],
                    tile,
                )]), // FACING=north, attached to the south block (vanilla bounds)
                3 => Some(vec![BlockElement::new(
                    [0.0, 0.0, 0.0],
                    [16.0, 16.0, 2.0],
                    tile,
                )]), // south
                4 => Some(vec![BlockElement::new(
                    [14.0, 0.0, 0.0],
                    [16.0, 16.0, 16.0],
                    tile,
                )]), // west
                _ => Some(vec![BlockElement::new(
                    [0.0, 0.0, 0.0],
                    [2.0, 16.0, 16.0],
                    tile,
                )]), // east
            }
        }

        // Redstone wire — center dot plus arms to connectable redstone components.
        Block::RedstoneWire => Some(redstone_wire_elements(x, y, z, tile, neighbor_state)),

        // Mob spawner — slightly smaller than full block
        Block::MobSpawner => Some(vec![BlockElement::new(
            [1.0, 0.0, 1.0],
            [15.0, 16.0, 15.0],
            tile,
        )]),

        // Enchanting table — book on top
        Block::EnchantingTable => Some(vec![BlockElement::new(
            [0.0, 0.0, 0.0],
            [16.0, 12.0, 16.0],
            tile,
        )]),

        // Brewing stand — thin stand
        Block::BrewingStand => Some(vec![
            BlockElement::new([5.0, 0.0, 5.0], [11.0, 12.0, 11.0], tile),
            BlockElement::new([7.0, 12.0, 7.0], [9.0, 16.0, 9.0], tile),
        ]),

        // Cauldron — hollow box
        Block::Cauldron => Some(vec![BlockElement::new(
            [0.0, 0.0, 0.0],
            [16.0, 14.0, 16.0],
            tile,
        )]),

        // Flower pot — small pot
        Block::FlowerPot => Some(vec![BlockElement::new(
            [5.0, 0.0, 5.0],
            [11.0, 8.0, 11.0],
            tile,
        )]),

        // End portal frame — slightly smaller
        Block::EndPortalFrame => Some(vec![BlockElement::new(
            [0.0, 0.0, 0.0],
            [16.0, 12.0, 16.0],
            tile,
        )]),

        // Skull — small box on top, matching vanilla BlockSkull.setBlockBounds
        Block::Skull => Some(vec![BlockElement::new(
            [4.0, 0.0, 4.0],
            [12.0, 8.0, 12.0],
            tile,
        )]),

        // Anvil — slightly tapered
        Block::Anvil => Some(vec![BlockElement::new(
            [1.0, 0.0, 1.0],
            [15.0, 16.0, 15.0],
            tile,
        )]),

        // --- New shapes: previously invisible/wrong blocks ---

        // Crops — cross shape, height varies by growth stage
        Block::Wheat => {
            let stage = (metadata >> 3) & 0x07;
            let h = 4.0 + stage as f32 * 2.0;
            Some(cross_shape(tile, h.min(16.0)))
        }
        Block::Carrots | Block::Potatoes => {
            let stage = (metadata >> 3) & 0x07;
            let h = 4.0 + stage as f32 * 2.0;
            Some(cross_shape(tile, h.min(16.0)))
        }
        Block::NetherWart => {
            let stage = metadata & 0x03;
            let h = 4.0 + stage as f32 * 4.0;
            Some(cross_shape(tile, h.min(16.0)))
        }

        // Stems — thin cross
        Block::PumpkinStem | Block::MelonStem => {
            let stage = metadata & 0x07;
            let h = 4.0 + stage as f32;
            Some(cross_shape(tile, h.min(16.0)))
        }

        // Rails — flat panel on ground
        Block::Rail | Block::PoweredRail | Block::DetectorRail | Block::ActivatorRail => {
            Some(rail_shape(block, metadata, tile))
        }

        // Signs
        Block::StandingSign => Some(sign_post(metadata, tile, true)),
        Block::WallSign => Some(wall_sign(metadata, tile, true)),
        // Banners (same shape as signs)
        Block::StandingBanner => Some(sign_post(metadata, tile, false)),
        Block::WallBanner => Some(wall_sign(metadata, tile, false)),

        // Fire uses different geometry on a supporting surface and when
        // attached to flammable neighbours, matching BlockFire#getActualState.
        Block::Fire => Some(fire_shape(tile, x, y, z, neighbor)),

        // Portals — flat panel filling the block
        Block::NetherPortal => Some(portal_shape(tile)),
        Block::EndPortal => Some(portal_shape(tile)),

        // Large flowers — cross shape (upper/lower handled by metadata)
        Block::LargeFlower => Some(large_flower_shape(tile)),

        // Lever
        Block::Lever => Some(lever_shape(metadata, tile)),

        // Buttons
        Block::StoneButton | Block::WoodenButton => Some(button_shape(metadata, tile)),

        // Pressure plates
        Block::StonePressurePlate
        | Block::WoodenPressurePlate
        | Block::LightWeightedPressurePlate
        | Block::HeavyWeightedPressurePlate => Some(pressure_plate(tile)),

        // Farmland — slightly shorter than full block
        Block::Farmland => Some(farmland_shape(tile)),

        // Cake — 7/8 height
        Block::Cake => Some(cake_shape(tile)),

        // Bed — head/foot and facing from metadata; two blocks combine naturally.
        Block::Bed => Some(bed_shape(metadata, tile)),

        // Hopper — funnel shape
        Block::Hopper => Some(hopper_shape(tile)),

        // Pistons: metadata low 3 bits are facing; bit 3 means extended/short.
        Block::Piston | Block::StickyPiston => Some(piston_base_shape(metadata, tile)),
        Block::PistonHead => Some(piston_head_shape(metadata, tile)),
        Block::PistonExtension => Some(piston_extension_shape(metadata, tile)),

        // Redstone repeaters and comparators — flat base
        Block::UnpoweredRepeater | Block::PoweredRepeater => Some(repeater_shape(tile)),
        Block::UnpoweredComparator | Block::PoweredComparator => Some(repeater_shape(tile)),

        // Daylight detector — flat slab
        Block::DaylightDetector | Block::DaylightDetectorInverted => Some(daylight_shape(tile)),

        // Cocoa pod
        Block::Cocoa => Some(cocoa_shape(tile)),

        // Tripwire
        Block::Tripwire => Some(tripwire_shape(tile)),
        Block::TripwireHook => Some(tripwire_hook_shape(tile)),

        // Chests are rendered dynamically so their lids can animate.
        Block::Chest | Block::TrappedChest | Block::EnderChest => Some(Vec::new()),

        // Default: generates standard faces for all normal blocks
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct HorizontalConnects {
    north: bool,
    south: bool,
    west: bool,
    east: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HorizontalFacing {
    East,
    South,
    West,
    North,
}

impl HorizontalFacing {
    fn from_stairs_meta(meta: u8) -> Self {
        match meta & 0x03 {
            0 => Self::East,
            1 => Self::West,
            2 => Self::South,
            _ => Self::North,
        }
    }

    fn from_horizontal_meta(meta: u8) -> Self {
        match meta & 0x03 {
            0 => Self::South,
            1 => Self::West,
            2 => Self::North,
            _ => Self::East,
        }
    }

    fn from_door_meta(meta: u8) -> Self {
        match meta & 0x03 {
            0 => Self::East,
            1 => Self::South,
            2 => Self::West,
            _ => Self::North,
        }
    }

    fn offset(self) -> (i32, i32) {
        match self {
            Self::East => (1, 0),
            Self::South => (0, 1),
            Self::West => (-1, 0),
            Self::North => (0, -1),
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::North => Self::South,
        }
    }

    fn rotate_left(self) -> Self {
        match self {
            Self::East => Self::North,
            Self::North => Self::West,
            Self::West => Self::South,
            Self::South => Self::East,
        }
    }

    fn rotate_right(self) -> Self {
        match self {
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
            Self::North => Self::East,
        }
    }
}

fn state_meta(state: u16) -> u8 {
    (state & 0x0f) as u8
}

fn state_block(state: u16) -> Block {
    Block::from_state(state)
}

fn is_stairs(block: Block) -> bool {
    matches!(
        block,
        Block::OakStairs
            | Block::SpruceStairs
            | Block::BirchStairs
            | Block::JungleStairs
            | Block::AcaciaStairs
            | Block::DarkOakStairs
            | Block::BrickStairs
            | Block::StoneBrickStairs
            | Block::SandstoneStairs
            | Block::RedSandstoneStairs
            | Block::NetherBrickStairs
            | Block::QuartzStairs
            | Block::CobblestoneStairs
    )
}

fn stair_elements(
    state: u16,
    x: usize,
    y: usize,
    z: usize,
    tile: (usize, usize, usize),
    neighbor_state: impl Fn(i32, i32, i32) -> u16 + Copy,
) -> Vec<BlockElement> {
    let metadata = state_meta(state);
    let upside_down = metadata & 0x04 != 0;
    let facing = HorizontalFacing::from_stairs_meta(metadata);
    let mut elements = Vec::with_capacity(2);
    let (base_y0, base_y1, step_y0, step_y1) = if upside_down {
        (8.0, 16.0, 0.0, 8.0)
    } else {
        (0.0, 8.0, 8.0, 16.0)
    };
    elements.push(BlockElement::new(
        [0.0, base_y0, 0.0],
        [16.0, base_y1, 16.0],
        tile,
    ));

    let shape = stair_shape(facing, upside_down, x, y, z, neighbor_state);
    match shape {
        StairShape::Straight => elements.push(stair_half(facing, step_y0, step_y1, tile)),
        StairShape::OuterLeft => elements.push(stair_quarter(
            facing,
            facing.rotate_left(),
            step_y0,
            step_y1,
            tile,
        )),
        StairShape::OuterRight => elements.push(stair_quarter(
            facing,
            facing.rotate_right(),
            step_y0,
            step_y1,
            tile,
        )),
        StairShape::InnerLeft => {
            elements.push(stair_half(facing, step_y0, step_y1, tile));
            elements.push(stair_quarter(
                facing.opposite(),
                facing.rotate_left(),
                step_y0,
                step_y1,
                tile,
            ));
        }
        StairShape::InnerRight => {
            elements.push(stair_half(facing, step_y0, step_y1, tile));
            elements.push(stair_quarter(
                facing.opposite(),
                facing.rotate_right(),
                step_y0,
                step_y1,
                tile,
            ));
        }
    }
    elements
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StairShape {
    Straight,
    InnerLeft,
    InnerRight,
    OuterLeft,
    OuterRight,
}

fn stair_shape(
    facing: HorizontalFacing,
    upside_down: bool,
    x: usize,
    y: usize,
    z: usize,
    neighbor_state: impl Fn(i32, i32, i32) -> u16 + Copy,
) -> StairShape {
    // This is BlockStairs#getActualState in 1.8.9: corners are determined by
    // the stair directly in front/behind, with a same-facing side stair
    // suppressing the corner.  Looking only at left/right (as we used to) is
    // not equivalent and produces both wrong meshes and wrong collision AABBs.
    let (lx, lz) = facing.rotate_left().offset();
    let (rx, rz) = facing.rotate_right().offset();
    let same_stair = |state: u16, expected_facing: HorizontalFacing| {
        is_stairs(state_block(state))
            && (state_meta(state) & 0x04 != 0) == upside_down
            && HorizontalFacing::from_stairs_meta(state_meta(state)) == expected_facing
    };

    let (fx, fz) = facing.offset();
    let front = neighbor_state(x as i32 + fx, y as i32, z as i32 + fz);
    if is_stairs(state_block(front)) && (state_meta(front) & 0x04 != 0) == upside_down {
        let front_facing = HorizontalFacing::from_stairs_meta(state_meta(front));
        if front_facing == facing.rotate_left()
            && !same_stair(
                neighbor_state(x as i32 + rx, y as i32, z as i32 + rz),
                facing,
            )
        {
            return StairShape::OuterLeft;
        }
        if front_facing == facing.rotate_right()
            && !same_stair(
                neighbor_state(x as i32 + lx, y as i32, z as i32 + lz),
                facing,
            )
        {
            return StairShape::OuterRight;
        }
    }

    let (bx, bz) = facing.opposite().offset();
    let back = neighbor_state(x as i32 + bx, y as i32, z as i32 + bz);
    if is_stairs(state_block(back)) && (state_meta(back) & 0x04 != 0) == upside_down {
        let back_facing = HorizontalFacing::from_stairs_meta(state_meta(back));
        if back_facing == facing.rotate_left()
            && !same_stair(
                neighbor_state(x as i32 + lx, y as i32, z as i32 + lz),
                facing,
            )
        {
            return StairShape::InnerLeft;
        }
        if back_facing == facing.rotate_right()
            && !same_stair(
                neighbor_state(x as i32 + rx, y as i32, z as i32 + rz),
                facing,
            )
        {
            return StairShape::InnerRight;
        }
    }

    StairShape::Straight
}

fn stair_half(
    facing: HorizontalFacing,
    y0: f32,
    y1: f32,
    tile: (usize, usize, usize),
) -> BlockElement {
    match facing {
        HorizontalFacing::East => BlockElement::new([8.0, y0, 0.0], [16.0, y1, 16.0], tile),
        HorizontalFacing::West => BlockElement::new([0.0, y0, 0.0], [8.0, y1, 16.0], tile),
        HorizontalFacing::South => BlockElement::new([0.0, y0, 8.0], [16.0, y1, 16.0], tile),
        HorizontalFacing::North => BlockElement::new([0.0, y0, 0.0], [16.0, y1, 8.0], tile),
    }
}

fn stair_quarter(
    forward: HorizontalFacing,
    side: HorizontalFacing,
    y0: f32,
    y1: f32,
    tile: (usize, usize, usize),
) -> BlockElement {
    let (mut x0, mut x1): (f32, f32) = match forward {
        HorizontalFacing::East => (8.0, 16.0),
        HorizontalFacing::West => (0.0, 8.0),
        _ => (0.0, 16.0),
    };
    let (mut z0, mut z1): (f32, f32) = match forward {
        HorizontalFacing::South => (8.0, 16.0),
        HorizontalFacing::North => (0.0, 8.0),
        _ => (0.0, 16.0),
    };
    match side {
        HorizontalFacing::East => {
            x0 = x0.max(8.0);
            x1 = x1.min(16.0);
        }
        HorizontalFacing::West => {
            x0 = x0.max(0.0);
            x1 = x1.min(8.0);
        }
        HorizontalFacing::South => {
            z0 = z0.max(8.0);
            z1 = z1.min(16.0);
        }
        HorizontalFacing::North => {
            z0 = z0.max(0.0);
            z1 = z1.min(8.0);
        }
    }
    BlockElement::new([x0, y0, z0], [x1, y1, z1], tile)
}

fn fence_elements(connects: HorizontalConnects, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let mut elements = vec![BlockElement::new([6.0, 0.0, 6.0], [10.0, 16.0, 10.0], tile)];
    if connects.north {
        elements.push(BlockElement::new([6.0, 6.0, 0.0], [10.0, 9.0, 10.0], tile));
        elements.push(BlockElement::new(
            [6.0, 12.0, 0.0],
            [10.0, 15.0, 10.0],
            tile,
        ));
    }
    if connects.south {
        elements.push(BlockElement::new([6.0, 6.0, 6.0], [10.0, 9.0, 16.0], tile));
        elements.push(BlockElement::new(
            [6.0, 12.0, 6.0],
            [10.0, 15.0, 16.0],
            tile,
        ));
    }
    if connects.west {
        elements.push(BlockElement::new([0.0, 6.0, 6.0], [10.0, 9.0, 10.0], tile));
        elements.push(BlockElement::new(
            [0.0, 12.0, 6.0],
            [10.0, 15.0, 10.0],
            tile,
        ));
    }
    if connects.east {
        elements.push(BlockElement::new([6.0, 6.0, 6.0], [16.0, 9.0, 10.0], tile));
        elements.push(BlockElement::new(
            [6.0, 12.0, 6.0],
            [16.0, 15.0, 10.0],
            tile,
        ));
    }
    elements
}

fn fence_gate_elements(metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    if metadata & 0x01 == 0 {
        let mut elements = vec![
            BlockElement::new([0.0, 5.0, 6.0], [2.0, 16.0, 10.0], tile),
            BlockElement::new([14.0, 5.0, 6.0], [16.0, 16.0, 10.0], tile),
        ];
        if metadata & 0x04 == 0 {
            elements.push(BlockElement::new([0.0, 6.0, 6.0], [16.0, 9.0, 10.0], tile));
            elements.push(BlockElement::new(
                [0.0, 12.0, 6.0],
                [16.0, 15.0, 10.0],
                tile,
            ));
        }
        elements
    } else {
        let mut elements = vec![
            BlockElement::new([6.0, 5.0, 0.0], [10.0, 16.0, 2.0], tile),
            BlockElement::new([6.0, 5.0, 14.0], [10.0, 16.0, 16.0], tile),
        ];
        if metadata & 0x04 == 0 {
            elements.push(BlockElement::new([6.0, 6.0, 0.0], [10.0, 9.0, 16.0], tile));
            elements.push(BlockElement::new(
                [6.0, 12.0, 0.0],
                [10.0, 15.0, 16.0],
                tile,
            ));
        }
        elements
    }
}

fn pane_elements(connects: HorizontalConnects, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let mut elements = Vec::new();
    if !connects.north && !connects.south && !connects.west && !connects.east {
        elements.push(BlockElement::new([7.0, 0.0, 0.0], [9.0, 16.0, 16.0], tile));
        elements.push(BlockElement::new([0.0, 0.0, 7.0], [16.0, 16.0, 9.0], tile));
        return elements;
    }
    if connects.north {
        elements.push(BlockElement::new([7.0, 0.0, 0.0], [9.0, 16.0, 9.0], tile));
    }
    if connects.south {
        elements.push(BlockElement::new([7.0, 0.0, 7.0], [9.0, 16.0, 16.0], tile));
    }
    if connects.west {
        elements.push(BlockElement::new([0.0, 0.0, 7.0], [9.0, 16.0, 9.0], tile));
    }
    if connects.east {
        elements.push(BlockElement::new([7.0, 0.0, 7.0], [16.0, 16.0, 9.0], tile));
    }
    elements.push(BlockElement::new([7.0, 0.0, 7.0], [9.0, 16.0, 9.0], tile));
    elements
}

fn wall_elements(connects: HorizontalConnects, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let mut elements = vec![BlockElement::new([4.0, 0.0, 4.0], [12.0, 16.0, 12.0], tile)];
    if connects.north {
        elements.push(BlockElement::new([5.0, 0.0, 0.0], [11.0, 14.0, 8.0], tile));
    }
    if connects.south {
        elements.push(BlockElement::new([5.0, 0.0, 8.0], [11.0, 14.0, 16.0], tile));
    }
    if connects.west {
        elements.push(BlockElement::new([0.0, 0.0, 5.0], [8.0, 14.0, 11.0], tile));
    }
    if connects.east {
        elements.push(BlockElement::new([8.0, 0.0, 5.0], [16.0, 14.0, 11.0], tile));
    }
    elements
}

fn door_elements(
    state: u16,
    x: usize,
    y: usize,
    z: usize,
    tile: (usize, usize, usize),
    neighbor_state: impl Fn(i32, i32, i32) -> u16 + Copy,
) -> Vec<BlockElement> {
    let t = 3.0;
    let metadata = state_meta(state);
    let upper_half = metadata & 0x08 != 0;
    let lower = if upper_half {
        state_meta(neighbor_state(x as i32, y as i32 - 1, z as i32))
    } else {
        metadata
    };
    let upper = if upper_half {
        metadata
    } else {
        state_meta(neighbor_state(x as i32, y as i32 + 1, z as i32))
    };
    let facing = HorizontalFacing::from_door_meta(lower);
    let open = lower & 0x04 != 0;
    let hinge_right = upper & 0x01 != 0;
    let effective = if open {
        if hinge_right {
            facing.rotate_left()
        } else {
            facing.rotate_right()
        }
    } else {
        facing
    };
    let texture = if upper_half { tile.0 } else { tile.1 };
    let right_handed_model = if open { !hinge_right } else { hinge_right };
    vec![match effective {
        HorizontalFacing::South => BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, t], tile),
        HorizontalFacing::West => BlockElement::new([13.0, 0.0, 0.0], [16.0, 16.0, 16.0], tile),
        HorizontalFacing::North => BlockElement::new([0.0, 0.0, 13.0], [16.0, 16.0, 16.0], tile),
        HorizontalFacing::East => BlockElement::new([0.0, 0.0, 0.0], [t, 16.0, 16.0], tile),
    }
    .with_face_tiles([texture; 6])
    .with_face_uvs(door_face_uvs(effective, right_handed_model))]
}

/// Exact UVs from vanilla's `door_{top,bottom}[ _rh].json`, rotated with the
/// model rather than reusing a full 16x16 image for the narrow 3px edges.
fn door_face_uvs(facing: HorizontalFacing, right_handed: bool) -> [[[f32; 2]; 4]; 6] {
    let rect = |u0: f32, v0: f32, u1: f32, v1: f32| {
        [
            [u0 / 16.0, v0 / 16.0],
            [u1 / 16.0, v0 / 16.0],
            [u1 / 16.0, v1 / 16.0],
            [u0 / 16.0, v1 / 16.0],
        ]
    };
    // The unrotated (east-facing) model has a 3px X thickness.  Its west and
    // east faces are the visible door panels; the other faces are only edges.
    let canonical = [
        rect(13.0, 0.0, 16.0, 16.0),
        rect(13.0, 0.0, 16.0, 16.0),
        rect(3.0, 0.0, 0.0, 16.0),
        rect(0.0, 0.0, 3.0, 16.0),
        if right_handed {
            rect(16.0, 0.0, 0.0, 16.0)
        } else {
            rect(0.0, 0.0, 16.0, 16.0)
        },
        if right_handed {
            rect(0.0, 0.0, 16.0, 16.0)
        } else {
            rect(16.0, 0.0, 0.0, 16.0)
        },
    ];
    let source_for_destination = match facing {
        HorizontalFacing::East => [0, 1, 2, 3, 4, 5],
        HorizontalFacing::South => [0, 1, 4, 5, 3, 2],
        HorizontalFacing::West => [0, 1, 3, 2, 5, 4],
        HorizontalFacing::North => [0, 1, 5, 4, 2, 3],
    };
    source_for_destination.map(|source| canonical[source])
}

fn trapdoor_elements(metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let open = metadata & 0x04 != 0;
    let top = metadata & 0x08 != 0;
    let t = 3.0;
    let elem = if open {
        match metadata & 0x03 {
            0 => BlockElement::new([0.0, 0.0, 13.0], [16.0, 16.0, 16.0], tile),
            1 => BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, t], tile),
            2 => BlockElement::new([13.0, 0.0, 0.0], [16.0, 16.0, 16.0], tile),
            _ => BlockElement::new([0.0, 0.0, 0.0], [t, 16.0, 16.0], tile),
        }
    } else if top {
        BlockElement::new([0.0, 13.0, 0.0], [16.0, 16.0, 16.0], tile)
    } else {
        BlockElement::new([0.0, 0.0, 0.0], [16.0, t, 16.0], tile)
    };
    vec![elem]
}

fn rail_shape(block: Block, metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let shape = if block == Block::Rail {
        metadata & 0x0f
    } else {
        metadata & 0x07
    };
    match shape {
        2 | 3 | 4 | 5 => {
            return flat_panel(tile);
        }
        6 => rail_corner(true, true, tile),   // south_east
        7 => rail_corner(false, true, tile),  // south_west
        8 => rail_corner(false, false, tile), // north_west
        9 => rail_corner(true, false, tile),  // north_east
        _ => flat_panel(tile),
    }
}

fn rail_corner(east: bool, south: bool, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let x_range = if east { (7.0, 16.0) } else { (0.0, 9.0) };
    let z_range = if south { (7.0, 16.0) } else { (0.0, 9.0) };
    vec![
        BlockElement::new([x_range.0, 0.0, 7.0], [x_range.1, 1.0, 9.0], tile),
        BlockElement::new([7.0, 0.0, z_range.0], [9.0, 1.0, z_range.1], tile),
    ]
}

fn redstone_wire_elements(
    x: usize,
    y: usize,
    z: usize,
    tile: (usize, usize, usize),
    neighbor_state: impl Fn(i32, i32, i32) -> u16 + Copy,
) -> Vec<BlockElement> {
    let mut connects = HorizontalConnects {
        north: redstone_connects(neighbor_state(x as i32, y as i32, z as i32 - 1)),
        south: redstone_connects(neighbor_state(x as i32, y as i32, z as i32 + 1)),
        west: redstone_connects(neighbor_state(x as i32 - 1, y as i32, z as i32)),
        east: redstone_connects(neighbor_state(x as i32 + 1, y as i32, z as i32)),
    };
    if !connects.north && !connects.south && !connects.west && !connects.east {
        connects.north = true;
        connects.south = true;
        connects.west = true;
        connects.east = true;
    }

    let mut elements = vec![BlockElement::new([6.0, 0.0, 6.0], [10.0, 1.0, 10.0], tile)];
    if connects.north {
        elements.push(BlockElement::new([6.0, 0.0, 0.0], [10.0, 1.0, 6.0], tile));
    }
    if connects.south {
        elements.push(BlockElement::new([6.0, 0.0, 10.0], [10.0, 1.0, 16.0], tile));
    }
    if connects.west {
        elements.push(BlockElement::new([0.0, 0.0, 6.0], [6.0, 1.0, 10.0], tile));
    }
    if connects.east {
        elements.push(BlockElement::new([10.0, 0.0, 6.0], [16.0, 1.0, 10.0], tile));
    }
    elements
}

fn redstone_connects(state: u16) -> bool {
    matches!(
        state_block(state),
        Block::RedstoneWire
            | Block::UnpoweredRepeater
            | Block::PoweredRepeater
            | Block::UnpoweredComparator
            | Block::PoweredComparator
            | Block::RedstoneTorch
            | Block::UnlitRedstoneTorch
            | Block::Lever
            | Block::StoneButton
            | Block::WoodenButton
            | Block::StonePressurePlate
            | Block::WoodenPressurePlate
            | Block::LightWeightedPressurePlate
            | Block::HeavyWeightedPressurePlate
            | Block::RedstoneBlock
            | Block::RedstoneLamp
            | Block::LitRedstoneLamp
            | Block::Dispenser
            | Block::Dropper
            | Block::Piston
            | Block::StickyPiston
    )
}

fn horizontal_connectivity(
    x: usize,
    y: usize,
    z: usize,
    neighbor: impl Fn(i32, i32, i32) -> Block + Copy,
    connects_to: fn(Block) -> bool,
) -> HorizontalConnects {
    HorizontalConnects {
        north: connects_to(neighbor(x as i32, y as i32, z as i32 - 1)),
        south: connects_to(neighbor(x as i32, y as i32, z as i32 + 1)),
        west: connects_to(neighbor(x as i32 - 1, y as i32, z as i32)),
        east: connects_to(neighbor(x as i32 + 1, y as i32, z as i32)),
    }
}

fn connects_to_fence(block: Block) -> bool {
    block.is_solid()
        || matches!(
            block,
            Block::OakFence
                | Block::SpruceFence
                | Block::BirchFence
                | Block::JungleFence
                | Block::DarkOakFence
                | Block::AcaciaFence
                | Block::NetherBrickFence
                | Block::OakFenceGate
                | Block::SpruceFenceGate
                | Block::BirchFenceGate
                | Block::JungleFenceGate
                | Block::DarkOakFenceGate
                | Block::AcaciaFenceGate
                | Block::CobblestoneWall
        )
}

fn connects_to_pane(block: Block) -> bool {
    block.properties().is_opaque && !has_custom_shape(block)
        || matches!(
            block,
            Block::GlassPane | Block::StainedGlassPane | Block::IronBars
        )
}

fn connects_to_wall(block: Block) -> bool {
    block.is_solid()
        || matches!(
            block,
            Block::CobblestoneWall
                | Block::OakFence
                | Block::SpruceFence
                | Block::BirchFence
                | Block::JungleFence
                | Block::DarkOakFence
                | Block::AcaciaFence
                | Block::NetherBrickFence
                | Block::OakFenceGate
                | Block::SpruceFenceGate
                | Block::BirchFenceGate
                | Block::JungleFenceGate
                | Block::DarkOakFenceGate
                | Block::AcaciaFenceGate
        )
}

/// Returns true if the block has non-full-cube geometry that should be rendered
/// even when the block is not solid (crops, rails, signs, flowers, etc.).
/// Used by the mesh builder to decide whether to skip a non-solid block.
pub fn has_custom_shape(block: Block) -> bool {
    matches!(
        block,
        Block::StandingSign | Block::WallSign
            | Block::StoneSlab | Block::StoneSlab2 | Block::WoodSlab
            | Block::OakStairs | Block::SpruceStairs | Block::BirchStairs
            | Block::JungleStairs | Block::AcaciaStairs | Block::DarkOakStairs
            | Block::BrickStairs | Block::StoneBrickStairs
            | Block::SandstoneStairs | Block::RedSandstoneStairs
            | Block::NetherBrickStairs | Block::QuartzStairs
            | Block::CobblestoneStairs
            | Block::OakFence | Block::SpruceFence | Block::BirchFence
            | Block::JungleFence | Block::DarkOakFence | Block::AcaciaFence
            | Block::OakFenceGate | Block::SpruceFenceGate | Block::BirchFenceGate
            | Block::JungleFenceGate | Block::DarkOakFenceGate | Block::AcaciaFenceGate
            | Block::NetherBrickFence
            | Block::CobblestoneWall
            | Block::GlassPane | Block::StainedGlassPane | Block::IronBars
            | Block::SnowLayer | Block::Carpet | Block::Cobweb
            | Block::OakDoor | Block::SpruceDoor | Block::BirchDoor | Block::JungleDoor
            | Block::AcaciaDoor | Block::DarkOakDoor | Block::IronDoor | Block::Trapdoor | Block::IronTrapdoor
            | Block::LilyPad | Block::Cactus | Block::SugarCane | Block::Vine
            | Block::Torch | Block::RedstoneTorch | Block::UnlitRedstoneTorch
            | Block::Dandelion | Block::Flower | Block::BrownMushroom | Block::RedMushroom
            | Block::Sapling | Block::TallGrass | Block::DeadBush
            | Block::Ladder | Block::RedstoneWire
            | Block::MobSpawner | Block::EnchantingTable | Block::BrewingStand
            | Block::Cauldron | Block::FlowerPot | Block::EndPortalFrame
            | Block::Anvil | Block::Skull
            // New: previously invisible blocks
            | Block::Wheat | Block::Carrots | Block::Potatoes | Block::NetherWart
            | Block::PumpkinStem | Block::MelonStem
            | Block::Rail | Block::PoweredRail | Block::DetectorRail | Block::ActivatorRail
            | Block::StandingSign | Block::WallSign
            | Block::StandingBanner | Block::WallBanner
            | Block::Fire | Block::NetherPortal | Block::EndPortal
            | Block::LargeFlower
            | Block::Lever | Block::StoneButton | Block::WoodenButton
            | Block::StonePressurePlate | Block::WoodenPressurePlate
            | Block::LightWeightedPressurePlate | Block::HeavyWeightedPressurePlate
            | Block::Farmland | Block::Cake | Block::Bed | Block::Hopper
            | Block::Piston | Block::StickyPiston | Block::PistonHead | Block::PistonExtension
            | Block::UnpoweredRepeater | Block::PoweredRepeater
            | Block::UnpoweredComparator | Block::PoweredComparator
            | Block::DaylightDetector | Block::DaylightDetectorInverted | Block::Cocoa
            | Block::Tripwire | Block::TripwireHook
            | Block::Chest | Block::TrappedChest | Block::EnderChest
    )
}

/// Cross-shaped plant geometry (flowers, crops, saplings, etc.)
fn cross_shape(tile: (usize, usize, usize), height: f32) -> Vec<BlockElement> {
    let h = height;
    vec![
        BlockElement::new([2.0, 0.0, 8.0], [14.0, h, 8.0], tile),
        BlockElement::new([8.0, 0.0, 2.0], [8.0, h, 14.0], tile),
    ]
}

/// Flat panel on the ground (rails, redstone wire, etc.)
fn flat_panel(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![BlockElement::new([0.0, 0.0, 0.0], [16.0, 1.0, 16.0], tile)]
}

fn chest_elements(
    block: Block,
    metadata: u8,
    tile: (usize, usize, usize),
    x: usize,
    y: usize,
    z: usize,
    neighbor: impl Fn(i32, i32, i32) -> Block + Copy,
) -> Vec<BlockElement> {
    use crate::assets::texture::tex_idx;
    if block != Block::EnderChest {
        let x = x as i32;
        let y = y as i32;
        let z = z as i32;
        let negative_neighbor = neighbor(x - 1, y, z) == block || neighbor(x, y, z - 1) == block;
        if negative_neighbor {
            return Vec::new();
        }
        let double_x = neighbor(x + 1, y, z) == block;
        let double_z = neighbor(x, y, z + 1) == block;
        if double_x || double_z {
            return large_chest_elements(block, metadata, tile, double_x);
        }
    }
    // Load 6 per-face tiles from the matching 64×64 entity texture.
    let variant = match block {
        Block::TrappedChest => "trapped",
        Block::EnderChest => "ender",
        _ => "normal",
    };
    let face_tiles = |part: &str| {
        let source = ["top", "bottom", "north", "south", "west", "east"]
            .map(|face| tex_idx(&format!("chest_{variant}_{part}_{face}")));
        // ModelChest is rendered with scale(1, -1, -1). Convert its model-space
        // faces to world-space before applying the metadata rotation below.
        tile_entity_model_face_tiles(source, 3)
    };
    let body = face_tiles("body");
    let lid = face_tiles("lid");
    let knob = face_tiles("knob");
    let rotation = match metadata & 7 {
        2 => 180.0,
        4 => 90.0,
        5 => -90.0,
        _ => 0.0,
    };
    vec![
        BlockElement::new(
            [1.0, 0.0, 1.0],
            [15.0, 10.0, 15.0],
            (body[0], body[1], body[2]),
        )
        .with_face_tiles(body)
        .with_face_uv_extents([
            (14.0 / 16.0, 14.0 / 16.0),
            (14.0 / 16.0, 14.0 / 16.0),
            (14.0 / 16.0, 10.0 / 16.0),
            (14.0 / 16.0, 10.0 / 16.0),
            (14.0 / 16.0, 10.0 / 16.0),
            (14.0 / 16.0, 10.0 / 16.0),
        ])
        .rotated_y(rotation),
        BlockElement::new(
            [1.0, 9.0, 1.0],
            [15.0, 14.0, 15.0],
            (lid[0], lid[1], lid[2]),
        )
        .with_face_tiles(lid)
        .with_face_uv_extents([
            (14.0 / 16.0, 14.0 / 16.0),
            (14.0 / 16.0, 14.0 / 16.0),
            (14.0 / 16.0, 5.0 / 16.0),
            (14.0 / 16.0, 5.0 / 16.0),
            (14.0 / 16.0, 5.0 / 16.0),
            (14.0 / 16.0, 5.0 / 16.0),
        ])
        .rotated_y(rotation),
        BlockElement::new(
            [7.0, 7.0, 15.0],
            [9.0, 11.0, 16.0],
            (knob[0], knob[1], knob[2]),
        )
        .with_face_tiles(knob)
        .with_face_uv_extents([
            (2.0 / 16.0, 1.0 / 16.0),
            (2.0 / 16.0, 1.0 / 16.0),
            (2.0 / 16.0, 4.0 / 16.0),
            (2.0 / 16.0, 4.0 / 16.0),
            (1.0 / 16.0, 4.0 / 16.0),
            (1.0 / 16.0, 4.0 / 16.0),
        ])
        .rotated_y(rotation),
    ]
}

fn large_chest_elements(
    block: Block,
    metadata: u8,
    tile: (usize, usize, usize),
    double_x: bool,
) -> Vec<BlockElement> {
    use crate::assets::texture::tex_idx;
    let variant = if block == Block::TrappedChest {
        "trapped"
    } else {
        "normal"
    };
    let model_tiles = |part: &str| {
        let source = ["top", "bottom", "north", "south", "west", "east"]
            .map(|face| tex_idx(&format!("chest_{variant}_double_{part}_{face}")));
        tile_entity_model_face_tiles(source, metadata)
    };
    let (body_from, body_to, lid_from, lid_to, knob_from, knob_to) = if double_x {
        let knob_z = if metadata & 7 == 2 {
            (0.0, 1.0)
        } else {
            (15.0, 16.0)
        };
        (
            [1.0, 0.0, 1.0],
            [31.0, 10.0, 15.0],
            [1.0, 9.0, 1.0],
            [31.0, 14.0, 15.0],
            [15.0, 7.0, knob_z.0],
            [17.0, 11.0, knob_z.1],
        )
    } else {
        let knob_x = if metadata & 7 == 4 {
            (0.0, 1.0)
        } else {
            (15.0, 16.0)
        };
        (
            [1.0, 0.0, 1.0],
            [15.0, 10.0, 31.0],
            [1.0, 9.0, 1.0],
            [15.0, 14.0, 31.0],
            [knob_x.0, 7.0, 15.0],
            [knob_x.1, 11.0, 17.0],
        )
    };
    [
        (body_from, body_to, model_tiles("body")),
        (lid_from, lid_to, model_tiles("lid")),
        (knob_from, knob_to, model_tiles("knob")),
    ]
    .into_iter()
    .map(|(from, to, tiles)| {
        BlockElement::new(from, to, tile)
            .with_face_tiles(tiles)
            .with_full_face_uvs()
    })
    .collect()
}

/// Map ModelBox texture faces through the transforms used by vanilla tile
/// entity renderers: scale(1, -1, -1), then the cardinal metadata rotation.
fn tile_entity_model_face_tiles(source: [usize; 6], metadata: u8) -> [usize; 6] {
    match metadata & 7 {
        2 => [
            source[1], source[0], source[2], source[3], source[5], source[4],
        ],
        4 => [
            source[1], source[0], source[4], source[5], source[2], source[3],
        ],
        5 => [
            source[1], source[0], source[5], source[4], source[3], source[2],
        ],
        _ => [
            source[1], source[0], source[3], source[2], source[4], source[5],
        ],
    }
}

/// Standing sign geometry from ModelSign and TileEntitySignRenderer (1.8.9).
fn sign_model_tiles(part: &str) -> [usize; 6] {
    use crate::assets::texture::tex_idx;
    ["top", "bottom", "north", "south", "west", "east"]
        .map(|face| tex_idx(&format!("sign_{part}_{face}")))
}

fn sign_post(meta: u8, tile: (usize, usize, usize), use_sign_texture: bool) -> Vec<BlockElement> {
    // BlockStandingSign stores sixteen 22.5° clockwise rotations in metadata.
    // The tile-entity renderer scales the 24×12×2 board by 2/3 and translates
    // it by 1/2 block, so the board spans y=9⅓..17⅓ and the stick y=0..9⅓.
    let mut board = BlockElement::new(
        [0.0, 9.333_333, 7.333_333],
        [16.0, 17.333_334, 8.666_667],
        tile,
    )
    .rotated_y(((meta & 0x0f) as f32) * 22.5);
    let mut stick = BlockElement::new(
        [7.333_333, 0.0, 7.333_333],
        [8.666_667, 9.333_333, 8.666_667],
        tile,
    );
    if use_sign_texture {
        board = board
            .with_face_tiles(tile_entity_model_face_tiles(sign_model_tiles("board"), 3))
            .with_full_face_uvs();
        stick = stick
            .with_face_tiles(tile_entity_model_face_tiles(sign_model_tiles("stick"), 3))
            .with_full_face_uvs();
    }
    vec![stick, board]
}

/// Thin wall sign (board, no post). Metadata determines which wall it's on.
fn wall_sign(meta: u8, tile: (usize, usize, usize), use_sign_texture: bool) -> Vec<BlockElement> {
    // TileEntitySignRenderer translates by (0, -5/16, -7/16), then renders
    // ModelSign's 24x12x2 board scaled by 2/3. In block-model coordinates that
    // produces a 1/12-block thick board centred 7/16 from the block centre.
    let mut elements = match meta & 0x07 {
        2 => vec![BlockElement::new(
            [0.0, 4.333_333, 14.333_333],
            [16.0, 12.333_333, 15.666_667],
            tile,
        )], // facing north → on south wall
        3 => vec![BlockElement::new(
            [0.0, 4.333_333, 0.333_333],
            [16.0, 12.333_333, 1.666_667],
            tile,
        )], // facing south → on north wall
        4 => vec![BlockElement::new(
            [14.333_333, 4.333_333, 0.0],
            [15.666_667, 12.333_333, 16.0],
            tile,
        )], // facing west → on east wall
        5 => vec![BlockElement::new(
            [0.333_333, 4.333_333, 0.0],
            [1.666_667, 12.333_333, 16.0],
            tile,
        )], // facing east → on west wall
        _ => vec![BlockElement::new(
            [0.0, 4.333_333, 0.333_333],
            [16.0, 12.333_333, 1.666_667],
            tile,
        )], // vanilla renderer's zero-rotation fallback
    };
    if use_sign_texture {
        for board in &mut elements {
            let source = sign_model_tiles("board");
            let tiles = tile_entity_model_face_tiles(source, meta);
            board.face_tiles = tiles.map(Some);
            board.uvs = [Some([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]); 6];
        }
    }
    elements
}

/// Thin pressure plate
fn pressure_plate(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![BlockElement::new([1.0, 0.0, 1.0], [15.0, 1.0, 15.0], tile)]
}

/// Tiny button on a wall/floor/ceiling.
fn button_shape(metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let pressed = metadata & 0x08 != 0;
    let depth = if pressed { 1.0 } else { 2.0 };
    vec![match metadata & 0x07 {
        1 => BlockElement::new([0.0, 6.0, 5.0], [depth, 10.0, 11.0], tile), // east face
        2 => BlockElement::new([16.0 - depth, 6.0, 5.0], [16.0, 10.0, 11.0], tile), // west face
        3 => BlockElement::new([5.0, 6.0, 0.0], [11.0, 10.0, depth], tile), // south face
        4 => BlockElement::new([5.0, 6.0, 16.0 - depth], [11.0, 10.0, 16.0], tile), // north face
        5 => BlockElement::new([5.0, 0.0, 6.0], [11.0, depth, 10.0], tile), // floor
        _ => BlockElement::new([5.0, 16.0 - depth, 6.0], [11.0, 16.0, 10.0], tile), // ceiling
    }]
}

/// Lever, using the 1.8.9 orientation metadata for its base.
fn lever_shape(metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let mut elements = Vec::with_capacity(2);
    elements.push(match metadata & 0x07 {
        1 => BlockElement::new([0.0, 3.0, 5.0], [2.0, 13.0, 11.0], tile),
        2 => BlockElement::new([14.0, 3.0, 5.0], [16.0, 13.0, 11.0], tile),
        3 => BlockElement::new([5.0, 3.0, 0.0], [11.0, 13.0, 2.0], tile),
        4 => BlockElement::new([5.0, 3.0, 14.0], [11.0, 13.0, 16.0], tile),
        5 | 6 => BlockElement::new([5.0, 0.0, 5.0], [11.0, 2.0, 11.0], tile),
        _ => BlockElement::new([5.0, 14.0, 5.0], [11.0, 16.0, 11.0], tile),
    });
    elements.push(match metadata & 0x07 {
        1 => BlockElement::new([2.0, 6.0, 7.0], [7.0, 10.0, 9.0], tile),
        2 => BlockElement::new([9.0, 6.0, 7.0], [14.0, 10.0, 9.0], tile),
        3 => BlockElement::new([7.0, 6.0, 2.0], [9.0, 10.0, 7.0], tile),
        4 => BlockElement::new([7.0, 6.0, 9.0], [9.0, 10.0, 14.0], tile),
        5 | 6 => BlockElement::new([7.0, 2.0, 7.0], [9.0, 10.0, 9.0], tile),
        _ => BlockElement::new([7.0, 6.0, 7.0], [9.0, 14.0, 9.0], tile),
    });
    elements
}

/// Face-aligned full-tile UVs suitable for the thin one-sided fire sheets.
/// The default `[[0,0],[1,0],[1,1],[0,1]]` mapping is only correct for north
/// (2) and east (5); south (3) and west (4) require the horizontally-flipped
/// `[[1,0],[0,0],[0,1],[1,1]]` to reproduce the vanilla match.
fn fire_uvs(face: usize) -> [[[f32; 2]; 4]; 6] {
    // Vertex order in mesh builder differs per face (bottom-to-top sweep).
    // Match each face so the texture bottom always lands on the element bottom.
    let right = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let flipped = [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]];
    let mut out = [[[0.0, 0.0]; 4]; 6];
    out[face] = match face {
        2 => flipped,
        3 | 4 | 5 => right,
        _ => right,
    };
    out
}

/// Fire geometry follows the two paths used by BlockFire#getActualState:
/// floor fire above a solid or flammable support, and vertical sheets attached
/// to flammable neighbours. The texture animation itself is handled by the atlas.
fn fire_shape(
    tile: (usize, usize, usize),
    x: usize,
    y: usize,
    z: usize,
    neighbor: impl Fn(i32, i32, i32) -> Block + Copy,
) -> Vec<BlockElement> {
    let at = |dx, dy, dz| neighbor(x as i32 + dx, y as i32 + dy, z as i32 + dz);
    let supports_fire = |block: Block| {
        (block.properties().is_opaque && !has_custom_shape(block)) || is_flammable(block)
    };

    if supports_fire(at(0, -1, 0)) {
        // Exact fire_floor layout: eight one-sided sheets. The four inner
        // sheets use the vanilla +/-22.5 degree tilt; each texture is emitted
        // once, avoiding the opaque-looking overdraw from cuboid backfaces.
        return vec![
            BlockElement::new([0.0, 0.0, 8.8], [16.0, 22.4, 8.8], tile)
                .rotated_x(-22.5)
                .with_visible_face(3)
                .with_face_uvs(fire_uvs(3)),
            BlockElement::new([0.0, 0.0, 7.2], [16.0, 22.4, 7.2], tile)
                .rotated_x(22.5)
                .with_visible_face(2)
                .with_face_uvs(fire_uvs(2)),
            BlockElement::new([8.8, 0.0, 0.0], [8.8, 22.4, 16.0], tile)
                .rotated_z(-22.5)
                .with_visible_face(4)
                .with_face_uvs(fire_uvs(4)),
            BlockElement::new([7.2, 0.0, 0.0], [7.2, 22.4, 16.0], tile)
                .rotated_z(22.5)
                .with_visible_face(5)
                .with_face_uvs(fire_uvs(5)),
            BlockElement::new([0.0, 0.0, 15.99], [16.0, 22.4, 15.99], tile)
                .with_visible_face(3)
                .with_face_uvs(fire_uvs(3)),
            BlockElement::new([0.0, 0.0, 0.01], [16.0, 22.4, 0.01], tile)
                .with_visible_face(2)
                .with_face_uvs(fire_uvs(2)),
            BlockElement::new([0.01, 0.0, 0.0], [0.01, 22.4, 16.0], tile)
                .with_visible_face(4)
                .with_face_uvs(fire_uvs(4)),
            BlockElement::new([15.99, 0.0, 0.0], [15.99, 22.4, 16.0], tile)
                .with_visible_face(5)
                .with_face_uvs(fire_uvs(5)),
        ];
    }

    let mut elements = Vec::new();
    if is_flammable(at(0, 0, -1)) {
        elements.push(
            BlockElement::new([0.0, 1.0, 0.01], [16.0, 23.4, 0.01], tile)
                .with_visible_face(2)
                .with_face_uvs(fire_uvs(2)),
        );
    }
    if is_flammable(at(0, 0, 1)) {
        elements.push(
            BlockElement::new([0.0, 1.0, 15.99], [16.0, 23.4, 15.99], tile)
                .with_visible_face(3)
                .with_face_uvs(fire_uvs(3)),
        );
    }
    if is_flammable(at(-1, 0, 0)) {
        elements.push(
            BlockElement::new([0.01, 1.0, 0.0], [0.01, 23.4, 16.0], tile)
                .with_visible_face(4)
                .with_face_uvs(fire_uvs(4)),
        );
    }
    if is_flammable(at(1, 0, 0)) {
        elements.push(
            BlockElement::new([15.99, 1.0, 0.0], [15.99, 23.4, 16.0], tile)
                .with_visible_face(5)
                .with_face_uvs(fire_uvs(5)),
        );
    }

    // A transient server state without a valid attachment should still be
    // visible until the next block update removes it.
    if elements.is_empty() {
        cross_shape(tile, 22.4)
    } else {
        elements
    }
}

/// Blocks registered through BlockFire#setFireInfo in Minecraft 1.8.9.
fn is_flammable(block: Block) -> bool {
    matches!(
        block,
        Block::Planks
            | Block::WoodSlab
            | Block::DoubleWoodSlab
            | Block::OakFenceGate
            | Block::SpruceFenceGate
            | Block::BirchFenceGate
            | Block::JungleFenceGate
            | Block::DarkOakFenceGate
            | Block::AcaciaFenceGate
            | Block::OakFence
            | Block::SpruceFence
            | Block::BirchFence
            | Block::JungleFence
            | Block::DarkOakFence
            | Block::AcaciaFence
            | Block::OakStairs
            | Block::SpruceStairs
            | Block::BirchStairs
            | Block::JungleStairs
            | Block::DarkOakStairs
            | Block::AcaciaStairs
            | Block::Log
            | Block::Log2
            | Block::Log3
            | Block::Leaves
            | Block::Leaves2
            | Block::Leaves3
            | Block::Bookshelf
            | Block::Tnt
            | Block::TallGrass
            | Block::LargeFlower
            | Block::Dandelion
            | Block::Flower
            | Block::DeadBush
            | Block::Wool
            | Block::Vine
            | Block::CoalBlock
            | Block::HayBlock
            | Block::Carpet
    )
}

/// Bed: 9 px high with small legs on the foot block.
fn bed_shape(metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    use crate::assets::texture::tex_idx;
    let is_head = metadata & 0x08 != 0;
    let top = tex_idx(if is_head {
        "bed_head_top"
    } else {
        "bed_feet_top"
    });
    let side = tex_idx(if is_head {
        "bed_head_side"
    } else {
        "bed_feet_side"
    });
    let bed_tile = (top, top, side);
    let mut elements =
        vec![BlockElement::new([0.0, 3.0, 0.0], [16.0, 9.0, 16.0], bed_tile).with_full_face_uvs()];
    if metadata & 0x08 == 0 {
        for (x0, z0) in [(1.0, 1.0), (13.0, 1.0), (1.0, 13.0), (13.0, 13.0)] {
            elements.push(
                BlockElement::new([x0, 0.0, z0], [x0 + 2.0, 3.0, z0 + 2.0], bed_tile)
                    .with_full_face_uvs(),
            );
        }
    }
    elements
}

/// Hopper — funnel shape (simplified as a hollow box)
fn hopper_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![
        BlockElement::new([0.0, 10.0, 0.0], [16.0, 16.0, 16.0], tile), // top rim
        BlockElement::new([4.0, 4.0, 4.0], [12.0, 10.0, 12.0], tile),  // funnel
        BlockElement::new([6.0, 0.0, 6.0], [10.0, 4.0, 10.0], tile),   // spout
    ]
}

fn piston_base_shape(metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let face_tiles = piston_face_tiles(metadata, tile);
    if metadata & 0x08 == 0 {
        return vec![BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, 16.0], tile)
            .with_face_tiles(face_tiles)];
    }
    vec![match metadata & 0x07 {
        0 => BlockElement::new([0.0, 4.0, 0.0], [16.0, 16.0, 16.0], tile),
        1 => BlockElement::new([0.0, 0.0, 0.0], [16.0, 12.0, 16.0], tile),
        2 => BlockElement::new([0.0, 0.0, 4.0], [16.0, 16.0, 16.0], tile),
        3 => BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, 12.0], tile),
        4 => BlockElement::new([4.0, 0.0, 0.0], [16.0, 16.0, 16.0], tile),
        5 => BlockElement::new([0.0, 0.0, 0.0], [12.0, 16.0, 16.0], tile),
        _ => BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, 16.0], tile),
    }
    .with_face_tiles(face_tiles)]
}

/// Piston textures are directional: the moving head faces its metadata
/// direction, the base cap faces the opposite direction, and only the other
/// four faces use `piston_side`.
fn piston_face_tiles(metadata: u8, tile: (usize, usize, usize)) -> [usize; 6] {
    let mut faces = [tile.2; 6];
    let (front, back) = match metadata & 0x07 {
        0 => (1, 0), // down, up
        1 => (0, 1), // up, down
        2 => (2, 3), // north, south
        3 => (3, 2), // south, north
        4 => (4, 5), // west, east
        5 => (5, 4), // east, west
        _ => (2, 3),
    };
    faces[front] = tile.0;
    faces[back] = tile.1;
    faces
}

/// Piston head — plate plus arm oriented by metadata.
fn piston_head_shape(metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    let mut elements = vec![match metadata & 0x07 {
        0 => BlockElement::new([0.0, 0.0, 0.0], [16.0, 4.0, 16.0], tile),
        1 => BlockElement::new([0.0, 12.0, 0.0], [16.0, 16.0, 16.0], tile),
        2 => BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, 4.0], tile),
        3 => BlockElement::new([0.0, 0.0, 12.0], [16.0, 16.0, 16.0], tile),
        4 => BlockElement::new([0.0, 0.0, 0.0], [4.0, 16.0, 16.0], tile),
        5 => BlockElement::new([12.0, 0.0, 0.0], [16.0, 16.0, 16.0], tile),
        _ => BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, 4.0], tile),
    }];
    elements.push(match metadata & 0x07 {
        0 => BlockElement::new([6.0, 4.0, 6.0], [10.0, 16.0, 10.0], tile),
        1 => BlockElement::new([6.0, 0.0, 6.0], [10.0, 12.0, 10.0], tile),
        2 => BlockElement::new([6.0, 6.0, 4.0], [10.0, 10.0, 16.0], tile),
        3 => BlockElement::new([6.0, 6.0, 0.0], [10.0, 10.0, 12.0], tile),
        4 => BlockElement::new([4.0, 6.0, 6.0], [16.0, 10.0, 10.0], tile),
        5 => BlockElement::new([0.0, 6.0, 6.0], [12.0, 10.0, 10.0], tile),
        _ => BlockElement::new([6.0, 6.0, 4.0], [10.0, 10.0, 16.0], tile),
    });
    elements
}

/// Repeater/Comparator — flat base with small torches (simplified)
fn repeater_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![
        BlockElement::new([0.0, 0.0, 0.0], [16.0, 2.0, 16.0], tile), // base
    ]
}

/// Farmland — slightly shorter than a full block
fn farmland_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![BlockElement::new([0.0, 0.0, 0.0], [16.0, 15.0, 16.0], tile)]
}

/// Cake — 7/8 height, slightly inset
fn cake_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![BlockElement::new([1.0, 0.0, 1.0], [15.0, 14.0, 15.0], tile)]
}

/// Daylight detector — flat slab
fn daylight_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![BlockElement::new([0.0, 0.0, 0.0], [16.0, 6.0, 16.0], tile)]
}

/// Cocoa pod on a log (simplified)
fn cocoa_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![BlockElement::new(
        [4.0, 3.0, 11.0],
        [12.0, 12.0, 16.0],
        tile,
    )]
}

/// Tripwire — very thin line
fn tripwire_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![BlockElement::new([0.0, 1.0, 0.0], [16.0, 2.0, 16.0], tile)]
}

/// Tripwire hook
fn tripwire_hook_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![
        BlockElement::new([6.0, 6.0, 14.0], [10.0, 10.0, 16.0], tile), // hook base
        BlockElement::new([7.0, 4.0, 10.0], [9.0, 12.0, 14.0], tile),  // arm
    ]
}

/// Block 36 is only a moving-piston placeholder. Vanilla renders its stored
/// block through TileEntityPistonRenderer; it has no static chunk model.
fn piston_extension_shape(_metadata: u8, tile: (usize, usize, usize)) -> Vec<BlockElement> {
    // BlockPistonMoving occupies the full voxel while the arm is extending or
    // retracting.  Returning a full block prevents the player from walking into
    // the emulated piston box (Intave / vanilla both treat it as solid).
    vec![BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, 16.0], tile)]
}

/// Large flower (two-block tall plant) — cross shape, taller
fn large_flower_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    cross_shape(tile, 16.0)
}

/// Portal frame (nether portal, end portal) — flat panel
fn portal_shape(tile: (usize, usize, usize)) -> Vec<BlockElement> {
    vec![BlockElement::new([0.0, 0.0, 0.0], [16.0, 16.0, 16.0], tile)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn air_state(_: i32, _: i32, _: i32) -> u16 {
        0
    }

    #[test]
    fn door_uses_upper_hinge_and_lower_open_state() {
        let lower_open_east = 0x04;
        let upper_hinge_left = 0x08;
        let elements = door_elements(lower_open_east, 0, 0, 0, (0, 0, 0), |_, dy, _| {
            if dy == 1 {
                upper_hinge_left
            } else {
                0
            }
        });
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].from, [0.0, 0.0, 0.0]);
        assert_eq!(elements[0].to, [16.0, 16.0, 3.0]);
    }

    #[test]
    fn stair_neighbor_can_create_outer_corner() {
        let east_stair = (Block::OakStairs.to_id() << 4) | 0; // facing east
        let north_stair = (Block::OakStairs.to_id() << 4) | 3;
        // Front neighbor (east, x+1) facing north → OuterLeft in vanilla.
        let elements = stair_elements(east_stair, 0, 0, 0, (0, 0, 0), |dx, _, dz| {
            if dx == 1 && dz == 0 {
                north_stair
            } else {
                0
            }
        });
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[1].from, [8.0, 8.0, 0.0]);
        assert_eq!(elements[1].to, [16.0, 16.0, 8.0]);
    }

    #[test]
    fn stair_back_neighbor_creates_the_matching_inner_corner() {
        for upside_down in [false, true] {
            let half = if upside_down { 0x04 } else { 0 };
            let east_stair = (Block::OakStairs.to_id() << 4) | half;
            let north_stair = (Block::OakStairs.to_id() << 4) | half | 3;
            let south_stair = (Block::OakStairs.to_id() << 4) | half | 2;

            let left = stair_shape(HorizontalFacing::East, upside_down, 0, 0, 0, |dx, _, dz| {
                if dx == -1 && dz == 0 {
                    north_stair
                } else {
                    0
                }
            });
            let right = stair_shape(HorizontalFacing::East, upside_down, 0, 0, 0, |dx, _, dz| {
                if dx == -1 && dz == 0 {
                    south_stair
                } else {
                    0
                }
            });

            assert_eq!(left, StairShape::InnerLeft, "state {east_stair:#x}");
            assert_eq!(right, StairShape::InnerRight, "state {east_stair:#x}");
        }
    }

    #[test]
    fn stair_metadata_direction_matches_vanilla_collision() {
        let east_stair = (Block::OakStairs.to_id() << 4) | 0;
        let elements = stair_elements(east_stair, 0, 0, 0, (0, 0, 0), air_state);
        assert_eq!(elements[1].from, [8.0, 8.0, 0.0]);
        assert_eq!(elements[1].to, [16.0, 16.0, 16.0]);
    }

    #[test]
    fn moving_piston_placeholder_occupies_the_full_voxel_for_collision() {
        assert!(!piston_extension_shape(1, (0, 0, 0)).is_empty());
    }

    #[test]
    fn ladder_metadata_uses_vanilla_opposite_support_face() {
        let north = block_elements(
            Block::Ladder,
            Block::Ladder.to_id() << 4 | 2,
            0,
            0,
            0,
            |_, _, _| Block::Air,
            air_state,
        )
        .unwrap();
        assert_eq!(north[0].from, [0.0, 0.0, 14.0]);
        assert_eq!(north[0].to, [16.0, 16.0, 16.0]);
    }

    #[test]
    fn vine_metadata_emits_each_attached_face_once() {
        let state = (Block::Vine.to_id() << 4) | 0x05;
        let vine =
            block_elements(Block::Vine, state, 0, 0, 0, |_, _, _| Block::Air, air_state).unwrap();
        assert_eq!(vine.len(), 2);
        // south (bit 0): single-sided sheet near south face
        assert_eq!(vine[0].from, [0.0, 0.0, 15.2]);
        assert_eq!(vine[0].to, [16.0, 16.0, 15.2]);
        assert_eq!(
            vine[0].visible_faces,
            [false, false, true, true, false, false]
        );
        // north (bit 2): single-sided sheet near north face
        assert_eq!(vine[1].from, [0.0, 0.0, 0.8]);
        assert_eq!(vine[1].to, [16.0, 16.0, 0.8]);
        assert_eq!(
            vine[1].visible_faces,
            [false, false, true, true, false, false]
        );
    }

    #[test]
    fn vine_faces_follow_each_metadata_bit_and_actual_top_state() {
        let state = Block::Vine.to_id() << 4 | 0x0f;
        let vine = block_elements(
            Block::Vine,
            state,
            0,
            0,
            0,
            |_, dy, _| if dy == 1 { Block::Stone } else { Block::Air },
            air_state,
        )
        .unwrap();

        assert_eq!(vine.len(), 5);
        assert_eq!(vine[0].to[2], 15.2); // south
        assert_eq!(vine[1].from[0], 0.8); // west
        assert_eq!(vine[2].from[2], 0.8); // north
        assert_eq!(vine[3].to[0], 15.2); // east
        assert_eq!(vine[4].from[1], 15.2); // UP actual state
        assert_eq!(
            vine[4].visible_faces,
            [true, true, false, false, false, false]
        );
    }

    #[test]
    fn standing_sign_matches_tile_entity_model_dimensions() {
        let elements = sign_post(0, (0, 0, 0), false);
        assert_eq!(elements[0].to[1], 9.333_333);
        assert_eq!(elements[1].from, [0.0, 9.333_333, 7.333_333]);
        assert_eq!(elements[1].to, [16.0, 17.333_334, 8.666_667]);
    }

    #[test]
    fn wall_sign_matches_tile_entity_renderer_offsets() {
        let north = wall_sign(2, (0, 0, 0), false);
        let south = wall_sign(3, (0, 0, 0), false);
        let west = wall_sign(4, (0, 0, 0), false);
        let east = wall_sign(5, (0, 0, 0), false);

        assert_eq!(north[0].from[2], 14.333_333);
        assert_eq!(north[0].to[2], 15.666_667);
        assert_eq!(south[0].from[2], 0.333_333);
        assert_eq!(south[0].to[2], 1.666_667);
        assert_eq!(west[0].from[0], 14.333_333);
        assert_eq!(west[0].to[0], 15.666_667);
        assert_eq!(east[0].from[0], 0.333_333);
        assert_eq!(east[0].to[0], 1.666_667);
    }

    #[test]
    fn tile_entity_model_faces_include_vanilla_axis_flips() {
        let source = [0, 1, 2, 3, 4, 5];
        assert_eq!(tile_entity_model_face_tiles(source, 2), [1, 0, 2, 3, 5, 4]);
        assert_eq!(tile_entity_model_face_tiles(source, 3), [1, 0, 3, 2, 4, 5]);
        assert_eq!(tile_entity_model_face_tiles(source, 4), [1, 0, 4, 5, 2, 3]);
        assert_eq!(tile_entity_model_face_tiles(source, 5), [1, 0, 5, 4, 3, 2]);
    }

    #[test]
    fn chest_uses_vanilla_three_part_closed_model() {
        let chest = chest_elements(Block::Chest, 3, (0, 0, 0), 0, 0, 0, |_, _, _| Block::Air);
        assert_eq!(chest.len(), 3);
        assert_eq!(chest[0].from, [1.0, 0.0, 1.0]);
        assert_eq!(chest[0].to, [15.0, 10.0, 15.0]);
        assert_eq!(chest[1].from, [1.0, 9.0, 1.0]);
        assert_eq!(chest[1].to, [15.0, 14.0, 15.0]);
        assert_eq!(chest[2].from, [7.0, 7.0, 15.0]);
        assert_eq!(chest[2].to, [9.0, 11.0, 16.0]);
    }

    #[test]
    fn double_chest_is_emitted_once_from_the_negative_half() {
        let positive = chest_elements(Block::Chest, 3, (0, 0, 0), 0, 0, 0, |x, _, _| {
            if x == 1 {
                Block::Chest
            } else {
                Block::Air
            }
        });
        assert_eq!(positive.len(), 3);
        assert_eq!(positive[0].to, [31.0, 10.0, 15.0]);

        let suppressed = chest_elements(Block::Chest, 3, (0, 0, 0), 1, 0, 0, |x, _, _| {
            if x == 0 {
                Block::Chest
            } else {
                Block::Air
            }
        });
        assert!(suppressed.is_empty());
    }

    #[test]
    fn double_chest_knob_tracks_the_vanilla_facing() {
        let along_x = |metadata| {
            chest_elements(Block::Chest, metadata, (0, 0, 0), 0, 0, 0, |x, _, _| {
                if x == 1 {
                    Block::Chest
                } else {
                    Block::Air
                }
            })
        };
        let along_z = |metadata| {
            chest_elements(Block::Chest, metadata, (0, 0, 0), 0, 0, 0, |_, _, z| {
                if z == 1 {
                    Block::Chest
                } else {
                    Block::Air
                }
            })
        };

        assert_eq!(along_x(2)[2].from[2], 0.0);
        assert_eq!(along_x(3)[2].from[2], 15.0);
        assert_eq!(along_z(4)[2].from[0], 0.0);
        assert_eq!(along_z(5)[2].from[0], 15.0);
    }

    #[test]
    fn fire_uses_tall_floor_or_attached_geometry() {
        let floor = fire_shape((0, 0, 0), 0, 1, 0, |_, dy, _| {
            if dy == 0 {
                Block::Stone
            } else {
                Block::Air
            }
        });
        assert!(floor.iter().all(|element| element.to[1] == 22.4));
        assert!(floor
            .iter()
            .all(|element| element.uvs.iter().all(Option::is_some)));

        let attached = fire_shape((0, 0, 0), 0, 1, 0, |_, dy, dz| {
            if dy == 1 && dz == -1 {
                Block::Planks
            } else {
                Block::Air
            }
        });
        assert_eq!(attached.len(), 1);
        assert_eq!(attached[0].from, [0.0, 1.0, 0.01]);
        assert_eq!(attached[0].to, [16.0, 23.4, 0.01]);
    }

    #[test]
    fn skull_has_a_renderable_custom_shape() {
        assert!(has_custom_shape(Block::Skull));
    }

    #[test]
    fn redstone_wire_only_extends_to_connectable_neighbors() {
        let wire = Block::RedstoneWire.to_id() << 4;
        let elements = redstone_wire_elements(0, 0, 0, (0, 0, 0), |dx, _, dz| {
            if dx == 1 || dz == -1 {
                wire
            } else {
                0
            }
        });
        assert_eq!(elements.len(), 3);
        assert!(elements.iter().any(|e| e.from == [10.0, 0.0, 6.0]));
        assert!(elements.iter().any(|e| e.from == [6.0, 0.0, 0.0]));
    }
}
