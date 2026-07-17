//! Lighting system — sky light + block light.
//!
//! MC 1.8.9-style lighting. Network chunks keep the server-provided sky and
//! block light nibbles. Locally changed chunks are recomputed with vanilla
//! opacity and propagation rules.
//!
//! Light values are stored per-block in chunks.
//! Light is computed locally (not just relying on server values).

use super::block::Block;
use super::chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

pub const MAX_LIGHT: u8 = 15;

/// MC 1.8.9 brightness table, generated from WorldProvider.generateLightBrightnessTable():
///   f1 = 1.0 - i/15.0
///   brightness[i] = (1.0 - f1) / (f1 * 3.0 + 1.0)
pub fn sky_light_brightness(level: u8) -> f32 {
    const TABLE: [f32; 16] = generate_brightness_table();
    TABLE[(level as usize).min(15)]
}

pub fn block_light_brightness(level: u8) -> f32 {
    sky_light_brightness(level)
}

const fn generate_brightness_table() -> [f32; 16] {
    let mut table = [0.0f32; 16];
    let mut i = 0;
    while i < 16 {
        let f1 = 1.0 - (i as f32) / 15.0;
        table[i] = (1.0 - f1) / (f1 * 3.0 + 1.0);
        i += 1;
    }
    table
}

/// Block light emission values matching MC 1.8.9.
pub fn block_light_emission(block: Block) -> u8 {
    match block {
        Block::Torch => 14,
        Block::Fire => 15,
        Block::Glowstone => 15,
        Block::LitFurnace => 13,
        Block::JackOLantern => 15,
        Block::RedstoneOre => 0,
        Block::LitRedstoneOre | Block::PoweredComparator => 9,
        Block::RedstoneTorch => 7,
        Block::UnlitRedstoneTorch => 0,
        Block::RedstoneWire => 0,
        Block::NetherPortal => 11,
        Block::FlowingLava | Block::StillLava => 15,
        Block::BrownMushroom => 1,
        Block::EndPortal => 15,
        Block::EndPortalFrame => 1,
        Block::BrewingStand => 1,
        Block::DragonEgg => 1,
        Block::Beacon => 15,
        Block::EnderChest => 7,
        Block::LitRedstoneLamp | Block::SeaLantern => 15,
        _ => 0,
    }
}

/// Light opacity (how much light this block absorbs).
/// MC 1.8.9: most solids = 255 (treated as 15 here), leaves = 1, water = 3, etc.
/// Return value 0-15. Values >= 15 block all light.
pub fn light_opacity(block: Block) -> u8 {
    match block {
        Block::Leaves | Block::Leaves2 | Block::Leaves3 => 1,
        Block::FlowingWater | Block::StillWater => 3,
        Block::Ice => 3,
        Block::Cobweb => 1,
        Block::Farmland
        | Block::StoneSlab
        | Block::DoubleStoneSlab
        | Block::WoodSlab
        | Block::DoubleWoodSlab
        | Block::StoneSlab2
        | Block::DoubleStoneSlab2
        | Block::OakStairs
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
        | Block::RedSandstoneStairs => 15,
        _ if block.is_solid() && block.properties().is_opaque => 15,
        _ => 0,
    }
}

/// Combined light level for rendering.
#[derive(Clone, Copy, Debug)]
pub struct LightLevel {
    pub sky: u8,
    pub block: u8,
}

impl LightLevel {
    /// Final brightness factor for rendering.
    /// Uses max of (sky with time-of-day) and block light.
    pub fn brightness(&self, sky_brightness: f32) -> f32 {
        let sky = sky_light_brightness(self.sky) * sky_brightness;
        let block = block_light_brightness(self.block);
        sky.max(block)
    }
}

/// Per-chunk light storage: 16 × 256 × 16 × 2 (sky + block).
#[derive(Clone)]
pub struct ChunkLight {
    pub sky: [[[u8; CHUNK_SIZE]; CHUNK_HEIGHT]; CHUNK_SIZE],
    pub block: [[[u8; CHUNK_SIZE]; CHUNK_HEIGHT]; CHUNK_SIZE],
}

impl ChunkLight {
    pub fn new() -> Self {
        Self {
            sky: [[[MAX_LIGHT; CHUNK_SIZE]; CHUNK_HEIGHT]; CHUNK_SIZE],
            block: [[[0; CHUNK_SIZE]; CHUNK_HEIGHT]; CHUNK_SIZE],
        }
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> LightLevel {
        LightLevel {
            sky: self.sky[x][y][z],
            block: self.block[x][y][z],
        }
    }

    fn from_network(chunk: &super::chunk::Chunk) -> Self {
        let mut light = Self::new();
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    let (sky, block) = chunk.light_at(x, y, z);
                    light.sky[x][y][z] = sky;
                    light.block[x][y][z] = block;
                }
            }
        }
        light
    }
}

#[derive(Clone, Copy)]
enum LightKind {
    Sky,
    Block,
}

pub struct WorldLight {
    chunks: HashMap<(i32, i32), Arc<ChunkLight>>,
}

impl WorldLight {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    /// Compute lighting for a chunk using MC 1.8.9-style propagation.
    /// Cross-chunk boundaries are handled by querying neighbor chunks'
    /// light data to ensure seamless lighting at chunk edges.
    pub fn compute_chunk(&mut self, chunk: &super::chunk::Chunk) {
        self.compute_chunk_with_validity(chunk, chunk.has_valid_network_light());
    }

    /// Vanilla 1.8.9 `World.checkLightFor`: update only the radius-17 area
    /// influenced by a changed block. Existing server nibbles are the baseline,
    /// so an edit never blanks whole chunks or introduces order-dependent seams.
    pub fn update_around(
        &mut self,
        chunks: &HashMap<(i32, i32), Arc<super::chunk::Chunk>>,
        origin: (i32, i32, i32),
    ) -> HashSet<(i32, i32)> {
        const RADIUS: i32 = 17;
        if origin.1 < 0 || origin.1 >= CHUNK_HEIGHT as i32 {
            return HashSet::new();
        }

        let min_cx = (origin.0 - RADIUS - 1).div_euclid(CHUNK_SIZE as i32);
        let max_cx = (origin.0 + RADIUS + 1).div_euclid(CHUNK_SIZE as i32);
        let min_cz = (origin.2 - RADIUS - 1).div_euclid(CHUNK_SIZE as i32);
        let max_cz = (origin.2 + RADIUS + 1).div_euclid(CHUNK_SIZE as i32);
        for cx in min_cx..=max_cx {
            for cz in min_cz..=max_cz {
                let Some(chunk) = chunks.get(&(cx, cz)) else {
                    // Vanilla returns false when the radius-17 area is not
                    // loaded. Preserve authoritative server light at the edge.
                    return HashSet::new();
                };
                self.chunks
                    .entry((cx, cz))
                    .or_insert_with(|| Arc::new(ChunkLight::from_network(chunk)));
            }
        }

        let mut column_heights = HashMap::new();
        for wx in origin.0 - RADIUS..=origin.0 + RADIUS {
            for wz in origin.2 - RADIUS..=origin.2 + RADIUS {
                let mut height = 0;
                for y in (0..CHUNK_HEIGHT as i32).rev() {
                    if light_opacity(block_at(chunks, wx, y, wz)) > 0 {
                        height = y + 1;
                        break;
                    }
                }
                column_heights.insert((wx, wz), height);
            }
        }

        let mut changed =
            self.update_kind(chunks, origin, RADIUS, LightKind::Block, &column_heights);
        if chunks
            .get(&(
                origin.0.div_euclid(CHUNK_SIZE as i32),
                origin.2.div_euclid(CHUNK_SIZE as i32),
            ))
            .is_some_and(|chunk| chunk.has_sky_light())
        {
            changed.extend(self.update_kind(
                chunks,
                origin,
                RADIUS,
                LightKind::Sky,
                &column_heights,
            ));
        }
        changed
    }

    fn update_kind(
        &mut self,
        chunks: &HashMap<(i32, i32), Arc<super::chunk::Chunk>>,
        origin: (i32, i32, i32),
        radius: i32,
        kind: LightKind,
        column_heights: &HashMap<(i32, i32), i32>,
    ) -> HashSet<(i32, i32)> {
        let mut queue = VecDeque::from([origin]);
        let mut queued = HashSet::from([origin]);
        let mut changed_chunks = HashSet::new();
        while let Some(pos) = queue.pop_front() {
            queued.remove(&pos);
            let old = self.light_at_world(pos, kind);
            let next = self.raw_light(chunks, pos, kind, column_heights);
            if old == next {
                continue;
            }
            self.set_light_at_world(pos, kind, next);
            changed_chunks.insert((
                pos.0.div_euclid(CHUNK_SIZE as i32),
                pos.2.div_euclid(CHUNK_SIZE as i32),
            ));
            for neighbor in world_neighbors(pos) {
                if neighbor.1 < 0 || neighbor.1 >= CHUNK_HEIGHT as i32 {
                    continue;
                }
                let distance = (neighbor.0 - origin.0).abs()
                    + (neighbor.1 - origin.1).abs()
                    + (neighbor.2 - origin.2).abs();
                if distance <= radius && queued.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        changed_chunks
    }

    fn raw_light(
        &self,
        chunks: &HashMap<(i32, i32), Arc<super::chunk::Chunk>>,
        pos: (i32, i32, i32),
        kind: LightKind,
        column_heights: &HashMap<(i32, i32), i32>,
    ) -> u8 {
        if matches!(kind, LightKind::Sky)
            && pos.1
                >= column_heights
                    .get(&(pos.0, pos.2))
                    .copied()
                    .unwrap_or(CHUNK_HEIGHT as i32)
        {
            return MAX_LIGHT;
        }
        let block = block_at(chunks, pos.0, pos.1, pos.2);
        let mut result = match kind {
            LightKind::Sky => 0,
            LightKind::Block => block_light_emission(block),
        };
        let mut opacity = light_opacity(block);
        if opacity >= MAX_LIGHT && result > 0 {
            opacity = 1;
        }
        opacity = opacity.max(1);
        if opacity >= MAX_LIGHT {
            return 0;
        }
        for neighbor in world_neighbors(pos) {
            result = result.max(self.light_at_world(neighbor, kind).saturating_sub(opacity));
            if result >= MAX_LIGHT {
                return MAX_LIGHT;
            }
        }
        result
    }

    fn light_at_world(&self, pos: (i32, i32, i32), kind: LightKind) -> u8 {
        if pos.1 < 0 || pos.1 >= CHUNK_HEIGHT as i32 {
            return if matches!(kind, LightKind::Sky) && pos.1 >= CHUNK_HEIGHT as i32 {
                MAX_LIGHT
            } else {
                0
            };
        }
        let cx = pos.0.div_euclid(CHUNK_SIZE as i32);
        let cz = pos.2.div_euclid(CHUNK_SIZE as i32);
        let lx = pos.0.rem_euclid(CHUNK_SIZE as i32) as usize;
        let lz = pos.2.rem_euclid(CHUNK_SIZE as i32) as usize;
        self.chunks.get(&(cx, cz)).map_or(0, |chunk| match kind {
            LightKind::Sky => chunk.sky[lx][pos.1 as usize][lz],
            LightKind::Block => chunk.block[lx][pos.1 as usize][lz],
        })
    }

    fn set_light_at_world(&mut self, pos: (i32, i32, i32), kind: LightKind, value: u8) {
        let cx = pos.0.div_euclid(CHUNK_SIZE as i32);
        let cz = pos.2.div_euclid(CHUNK_SIZE as i32);
        let lx = pos.0.rem_euclid(CHUNK_SIZE as i32) as usize;
        let lz = pos.2.rem_euclid(CHUNK_SIZE as i32) as usize;
        let Some(chunk) = self.chunks.get_mut(&(cx, cz)) else {
            return;
        };
        let chunk = Arc::make_mut(chunk);
        match kind {
            LightKind::Sky => chunk.sky[lx][pos.1 as usize][lz] = value,
            LightKind::Block => chunk.block[lx][pos.1 as usize][lz] = value,
        }
    }

    /// Computes from an immutable validity decision captured with a mesh job.
    /// The live chunk flag can change while a worker is running; using it again
    /// would mix two world generations inside one mesh.
    pub(super) fn compute_chunk_with_validity(
        &mut self,
        chunk: &super::chunk::Chunk,
        network_light_valid: bool,
    ) {
        let cx = chunk.cx;
        let cz = chunk.cz;

        let mut light = ChunkLight::new();

        if network_light_valid {
            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_HEIGHT {
                    for z in 0..CHUNK_SIZE {
                        let (sky, block) = chunk.light_at(x, y, z);
                        light.sky[x][y][z] = sky;
                        light.block[x][y][z] = block;
                    }
                }
            }
            self.chunks.insert((cx, cz), Arc::new(light));
            return;
        }
        if chunk.has_sky_light() {
            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let mut sky = MAX_LIGHT;
                    for y in (0..CHUNK_HEIGHT).rev() {
                        sky = sky.saturating_sub(light_opacity(chunk.get(x, y, z)));
                        light.sky[x][y][z] = sky;
                    }
                }
            }
        }

        // Step 2b: Flood-fill sky light — includes cross-chunk sampling at edges.
        let mut sky_queue: VecDeque<(usize, usize, usize, u8)> = VecDeque::new();

        // Seed queue: local chunk + neighbor boundary blocks
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    let lvl = light.sky[x][y][z];
                    if lvl > 0 {
                        sky_queue.push_back((x, y, z, lvl));
                    }
                }
            }
        }
        // Also seed from neighboring chunks at the shared face
        for y in 0..CHUNK_HEIGHT {
            for i in 0..CHUNK_SIZE {
                for &(dx, dz, sx, sz) in &[
                    (-1, 0, CHUNK_SIZE - 1, i),
                    (1, 0, 0, i),
                    (0, -1, i, CHUNK_SIZE - 1),
                    (0, 1, i, 0),
                ] {
                    let neighbor = self.get(cx + dx, cz + dz, sx, y, sz);
                    let lx = match dx {
                        -1 => 0,
                        1 => CHUNK_SIZE - 1,
                        _ => i,
                    };
                    let lz = match dz {
                        -1 => 0,
                        1 => CHUNK_SIZE - 1,
                        _ => i,
                    };
                    let attenuation = light_opacity(chunk.get(lx, y, lz)).max(1);
                    let incoming = neighbor.sky.saturating_sub(attenuation);
                    if incoming > light.sky[lx][y][lz] {
                        light.sky[lx][y][lz] = incoming;
                        if incoming > 1 {
                            sky_queue.push_back((lx, y, lz, incoming));
                        }
                    }
                }
            }
        }

        while let Some((x, y, z, level)) = sky_queue.pop_front() {
            if level <= 1 {
                continue;
            }
            for (nx, ny, nz) in neighbors(x, y, z) {
                let opacity = light_opacity(chunk.get(nx, ny, nz));
                if opacity >= MAX_LIGHT {
                    continue;
                }
                let neighbor_light = level.saturating_sub(opacity.max(1));
                if neighbor_light > light.sky[nx][ny][nz] {
                    light.sky[nx][ny][nz] = neighbor_light;
                    if neighbor_light > 1 {
                        sky_queue.push_back((nx, ny, nz, neighbor_light));
                    }
                }
            }
        }

        // Step 3: Block light — seed & propagate.
        let mut queue: VecDeque<(usize, usize, usize, u8)> = VecDeque::new();

        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    let emission = block_light_emission(chunk.get(x, y, z));
                    light.block[x][y][z] = emission;
                    if emission > 0 {
                        queue.push_back((x, y, z, emission));
                    }
                }
            }
        }
        // Seed neighbor block light at boundaries
        for y in 0..CHUNK_HEIGHT {
            for i in 0..CHUNK_SIZE {
                for &(dx, dz, sx, sz) in &[
                    (-1, 0, CHUNK_SIZE - 1, i),
                    (1, 0, 0, i),
                    (0, -1, i, CHUNK_SIZE - 1),
                    (0, 1, i, 0),
                ] {
                    let neighbor = self.get(cx + dx, cz + dz, sx, y, sz);
                    let lx = match dx {
                        -1 => 0,
                        1 => CHUNK_SIZE - 1,
                        _ => i,
                    };
                    let lz = match dz {
                        -1 => 0,
                        1 => CHUNK_SIZE - 1,
                        _ => i,
                    };
                    let attenuation = light_opacity(chunk.get(lx, y, lz)).max(1);
                    let incoming = neighbor.block.saturating_sub(attenuation);
                    if incoming > light.block[lx][y][lz] {
                        light.block[lx][y][lz] = incoming;
                        if incoming > 1 {
                            queue.push_back((lx, y, lz, incoming));
                        }
                    }
                }
            }
        }

        while let Some((x, y, z, level)) = queue.pop_front() {
            if level <= 1 {
                continue;
            }
            for (nx, ny, nz) in neighbors(x, y, z) {
                let opacity = light_opacity(chunk.get(nx, ny, nz));
                if opacity >= MAX_LIGHT {
                    continue;
                }
                let neighbor_light = level.saturating_sub(opacity.max(1));
                if neighbor_light > light.block[nx][ny][nz] {
                    light.block[nx][ny][nz] = neighbor_light;
                    if neighbor_light > 1 {
                        queue.push_back((nx, ny, nz, neighbor_light));
                    }
                }
            }
        }

        self.chunks.insert((cx, cz), Arc::new(light));
    }

    /// Recompute light in a chunk after a block change.
    pub fn recompute_at(&mut self, _cx: i32, _cz: i32, chunk: &super::chunk::Chunk) {
        self.compute_chunk(chunk);
    }

    pub fn get(&self, cx: i32, cz: i32, x: usize, y: usize, z: usize) -> LightLevel {
        self.chunks
            .get(&(cx, cz))
            .map(|chunk| chunk.get(x, y, z))
            // A missing chunk cannot contribute light. Treating it as full
            // skylight leaks daylight through unloaded chunk borders into
            // enclosed caves while meshes are built asynchronously.
            .unwrap_or(LightLevel { sky: 0, block: 0 })
    }

    /// Borrows one cached chunk so bulk readers can avoid a hash lookup for
    /// every individual block.
    pub fn chunk(&self, cx: i32, cz: i32) -> Option<&ChunkLight> {
        self.chunks.get(&(cx, cz)).map(Arc::as_ref)
    }

    pub(super) fn shared_chunk(&self, cx: i32, cz: i32) -> Option<Arc<ChunkLight>> {
        self.chunks.get(&(cx, cz)).cloned()
    }

    pub fn get_at_world(&self, wx: i32, wy: i32, wz: i32) -> LightLevel {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return LightLevel { sky: 0, block: 0 };
        }
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        let lx = wx.rem_euclid(CHUNK_SIZE as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_SIZE as i32) as usize;
        self.get(cx, cz, lx, wy as usize, lz)
    }

    pub fn remove_chunk(&mut self, cx: i32, cz: i32) {
        self.chunks.remove(&(cx, cz));
    }

    pub(super) fn take_chunk(&mut self, cx: i32, cz: i32) -> Option<Arc<ChunkLight>> {
        self.chunks.remove(&(cx, cz))
    }

    pub(super) fn insert_chunk(&mut self, cx: i32, cz: i32, light: Arc<ChunkLight>) {
        self.chunks.insert((cx, cz), light);
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
    }
}

fn neighbors(x: usize, y: usize, z: usize) -> impl Iterator<Item = (usize, usize, usize)> {
    let mut out: [(usize, usize, usize); 6] = [(0, 0, 0); 6];
    let mut count = 0;
    if x > 0 {
        out[count] = (x - 1, y, z);
        count += 1;
    }
    if x + 1 < CHUNK_SIZE {
        out[count] = (x + 1, y, z);
        count += 1;
    }
    if y > 0 {
        out[count] = (x, y - 1, z);
        count += 1;
    }
    if y + 1 < CHUNK_HEIGHT {
        out[count] = (x, y + 1, z);
        count += 1;
    }
    if z > 0 {
        out[count] = (x, y, z - 1);
        count += 1;
    }
    if z + 1 < CHUNK_SIZE {
        out[count] = (x, y, z + 1);
        count += 1;
    }
    out.into_iter().take(count)
}

fn block_at(
    chunks: &HashMap<(i32, i32), Arc<super::chunk::Chunk>>,
    wx: i32,
    wy: i32,
    wz: i32,
) -> Block {
    if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
        return Block::Air;
    }
    let cx = wx.div_euclid(CHUNK_SIZE as i32);
    let cz = wz.div_euclid(CHUNK_SIZE as i32);
    let lx = wx.rem_euclid(CHUNK_SIZE as i32) as usize;
    let lz = wz.rem_euclid(CHUNK_SIZE as i32) as usize;
    chunks
        .get(&(cx, cz))
        .map_or(Block::Air, |chunk| chunk.get(lx, wy as usize, lz))
}

fn world_neighbors(pos: (i32, i32, i32)) -> [(i32, i32, i32); 6] {
    [
        (pos.0 - 1, pos.1, pos.2),
        (pos.0 + 1, pos.1, pos.2),
        (pos.0, pos.1 - 1, pos.2),
        (pos.0, pos.1 + 1, pos.2),
        (pos.0, pos.1, pos.2 - 1),
        (pos.0, pos.1, pos.2 + 1),
    ]
}

#[cfg(test)]
mod tests {
    use super::{LightLevel, WorldLight};
    use crate::world::block::Block;
    use crate::world::chunk::{Chunk, CHUNK_SIZE};
    use crate::world::mesh::{build_chunk_mesh, MeshOptions};

    #[test]
    fn open_surface_mesh_keeps_full_skylight() {
        let mut chunk = Chunk::new(0, 0);
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for y in 0..64 {
                    chunk.set(x, y, z, Block::Stone);
                }
            }
        }

        let mut lighting = WorldLight::new();
        lighting.compute_chunk(&chunk);
        assert_eq!(lighting.get(0, 0, 8, 64, 8).sky, 15);

        let mesh = build_chunk_mesh(
            &chunk,
            |x, y, z| {
                if (0..16).contains(&x) && (0..256).contains(&y) && (0..16).contains(&z) {
                    chunk.get(x as usize, y as usize, z as usize)
                } else {
                    Block::Air
                }
            },
            |x, y, z| {
                if (0..16).contains(&x) && (0..256).contains(&y) && (0..16).contains(&z) {
                    lighting.get(0, 0, x as usize, y as usize, z as usize)
                } else {
                    LightLevel { sky: 15, block: 0 }
                }
            },
            |x, y, z| {
                if (0..16).contains(&x) && (0..256).contains(&y) && (0..16).contains(&z) {
                    chunk.state(x as usize, y as usize, z as usize)
                } else {
                    0
                }
            },
            MeshOptions::default(),
        );

        let top = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.normal[1] > 0.5 && (vertex.pos[1] - 64.0).abs() < 0.001)
            .collect::<Vec<_>>();
        assert!(!top.is_empty());
        assert!(top.iter().all(|vertex| vertex.sky_light >= 14.9));
        assert!(top.iter().all(|vertex| vertex.ambient_occlusion >= 0.99));
    }
}
