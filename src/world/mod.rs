//! World management — chunks, blocks, terrain generation.

pub mod block;
pub mod block_models;
pub mod chunk;
pub mod color;
pub mod item;
pub mod light;
pub mod material;
pub mod mesh;
pub mod network;
pub mod shape;

use block::Block;
use chunk::{Chunk, CHUNK_HEIGHT, CHUNK_SIZE};
use mesh::{ChunkMesh, MeshOptions};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{mpsc, Arc, Mutex};

const MAX_NORMAL_BACKGROUND_MESH_JOBS: usize = 2;
const MAX_PRIORITY_BACKGROUND_MESH_JOBS: usize = 1;
const MAX_MESH_RESULTS_PER_FRAME: usize =
    MAX_NORMAL_BACKGROUND_MESH_JOBS + MAX_PRIORITY_BACKGROUND_MESH_JOBS;
const MAX_MESH_REMOVALS_PER_FRAME: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MeshJobToken {
    epoch: u64,
    generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotRegionRevision {
    epoch: u64,
    chunks: Vec<((i32, i32), u64)>,
}

#[derive(Debug, Default)]
pub(crate) struct SnapshotChanges {
    pub all: bool,
    pub chunks: HashSet<(i32, i32)>,
    pub blocks: HashSet<(i32, i32, i32)>,
}

struct MeshBuildResult {
    token: MeshJobToken,
    mesh: ChunkMesh,
    light: Option<Arc<light::ChunkLight>>,
}

#[derive(Clone)]
struct MeshChunkSnapshot {
    chunk: Arc<Chunk>,
    network_light_valid: bool,
    local_light: Option<Arc<light::ChunkLight>>,
}

pub struct World {
    pub chunks: HashMap<(i32, i32), Arc<Chunk>>,
    pub skulls: HashMap<(i32, i32, i32), SkullBlockEntity>,
    pub chests: HashMap<(i32, i32, i32), ChestBlockEntity>,
    pub grass_color: color::ColorMap,
    pub foliage_color: color::ColorMap,
    pub light: light::WorldLight,
    /// Monotonic generation for block, biome, and light data exposed to Lua.
    /// Render-only state deliberately does not participate in this counter.
    snapshot_revision: u64,
    /// Full-world generation. A reset invalidates every cached scripting region.
    snapshot_epoch: u64,
    /// Last content generation for each chunk, including unloaded tombstones.
    snapshot_chunk_revisions: HashMap<(i32, i32), u64>,
    /// Exact block changes since the last scripting snapshot publication.
    snapshot_dirty_blocks: HashSet<(i32, i32, i32)>,
    /// Chunk-wide changes (load/unload/lighting) since the last publication.
    snapshot_dirty_chunks: HashSet<(i32, i32)>,
    snapshot_all_dirty: bool,
    pub mesh_options: MeshOptions,
    /// When true, chunk data changes skip mesh building (used during loading).
    pub defer_mesh_build: bool,
    /// Chunks that still need their mesh built (incremental build queue).
    pub pending_mesh_queue: VecDeque<(i32, i32)>,
    /// Interactive block changes get one dedicated slot without starving
    /// normal chunk streaming work.
    pending_mesh_priority_queue: VecDeque<(i32, i32)>,
    /// Chunks whose GPU mesh should be removed incrementally.
    pub pending_mesh_removals: VecDeque<(i32, i32)>,
    /// Full-world generation for background mesh jobs. World resets advance
    /// this even when a new world immediately reuses the same chunk position.
    mesh_epoch: u64,
    /// Per-position content generation within the current mesh epoch.
    mesh_generations: HashMap<(i32, i32), u64>,
    pending_mesh_inflight: HashMap<(i32, i32), MeshJobToken>,
    pending_mesh_dirty: HashSet<(i32, i32)>,
    pending_mesh_priority_inflight: HashSet<(i32, i32)>,
    pending_mesh_priority_dirty: HashSet<(i32, i32)>,
    mesh_result_tx: mpsc::Sender<MeshBuildResult>,
    mesh_result_rx: Mutex<mpsc::Receiver<MeshBuildResult>>,
}

impl World {
    pub fn new() -> Self {
        let grass_color = color::ColorMap::load("assets/minecraft/textures/colormap/grass.png")
            .expect("Failed to load grass color map");
        let foliage_color = color::ColorMap::load("assets/minecraft/textures/colormap/foliage.png")
            .expect("Failed to load foliage color map");

        let (mesh_result_tx, mesh_result_rx) = mpsc::channel();
        World {
            chunks: HashMap::new(),
            skulls: HashMap::new(),
            chests: HashMap::new(),
            grass_color,
            foliage_color,
            light: light::WorldLight::new(),
            snapshot_revision: 0,
            snapshot_epoch: 0,
            snapshot_chunk_revisions: HashMap::new(),
            snapshot_dirty_blocks: HashSet::new(),
            snapshot_dirty_chunks: HashSet::new(),
            snapshot_all_dirty: false,
            mesh_options: MeshOptions::default(),
            defer_mesh_build: false,
            pending_mesh_queue: VecDeque::new(),
            pending_mesh_priority_queue: VecDeque::new(),
            pending_mesh_removals: VecDeque::new(),
            mesh_epoch: 0,
            mesh_generations: HashMap::new(),
            pending_mesh_inflight: HashMap::new(),
            pending_mesh_dirty: HashSet::new(),
            pending_mesh_priority_inflight: HashSet::new(),
            pending_mesh_priority_dirty: HashSet::new(),
            mesh_result_tx,
            mesh_result_rx: Mutex::new(mesh_result_rx),
        }
    }

    pub fn get_block(&self, wx: i32, wy: i32, wz: i32) -> Block {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return Block::Air;
        }
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        let lx = wx.rem_euclid(CHUNK_SIZE as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_SIZE as i32) as usize;
        match self.chunks.get(&(cx, cz)) {
            Some(chunk) => chunk.get(lx, wy as usize, lz),
            None => Block::Air,
        }
    }

    pub fn apply_chest_event(&mut self, x: i32, y: i32, z: i32, viewers: u8) {
        if is_chest_block(self.get_block(x, y, z)) {
            self.chests.entry((x, y, z)).or_default().viewers = viewers as i32;
        }
    }

    /// Predict the local player's close immediately after C0D. The next
    /// server BlockAction remains authoritative. InventoryLargeChest invokes
    /// closeInventory on both halves, so mirror that for double chests.
    pub fn close_chest_for_local_viewer(&mut self, position: (i32, i32, i32)) {
        let block = self.get_block(position.0, position.1, position.2);
        if !is_chest_block(block) {
            return;
        }

        let mut positions = vec![position];
        if block != Block::EnderChest {
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let adjacent = (position.0 + dx, position.1, position.2 + dz);
                if self.get_block(adjacent.0, adjacent.1, adjacent.2) == block {
                    positions.push(adjacent);
                }
            }
        }
        for position in positions {
            if let Some(chest) = self.chests.get_mut(&position) {
                chest.viewers = chest.viewers.saturating_sub(1).max(0);
            }
        }
    }

    pub fn tick_chests(&mut self) {
        for chest in self.chests.values_mut() {
            chest.prev_lid_angle = chest.lid_angle;
            if chest.viewers > 0 && chest.lid_angle < 1.0 {
                chest.lid_angle = (chest.lid_angle + 0.1).min(1.0);
            } else if chest.viewers <= 0 && chest.lid_angle > 0.0 {
                chest.lid_angle = (chest.lid_angle - 0.1).max(0.0);
            }
        }
    }

    /// Changes whenever a cached scripting block snapshot may have gone stale.
    pub fn snapshot_revision(&self) -> u64 {
        self.snapshot_revision
    }

    /// Exact generation fingerprint for the chunks intersecting a block cube.
    /// A distant server update therefore does not invalidate the Lua block cache.
    pub fn snapshot_revision_around(
        &self,
        center: (i32, i32, i32),
        radius: i32,
    ) -> SnapshotRegionRevision {
        const MAX_EXACT_CHUNKS: i64 = 4_096;
        let radius = i64::from(radius.max(0));
        let min_cx = (i64::from(center.0) - radius).div_euclid(CHUNK_SIZE as i64);
        let max_cx = (i64::from(center.0) + radius).div_euclid(CHUNK_SIZE as i64);
        let min_cz = (i64::from(center.2) - radius).div_euclid(CHUNK_SIZE as i64);
        let max_cz = (i64::from(center.2) + radius).div_euclid(CHUNK_SIZE as i64);
        let chunk_count = (max_cx - min_cx + 1).saturating_mul(max_cz - min_cz + 1);
        if chunk_count > MAX_EXACT_CHUNKS {
            // This API normally receives the scripting radius (15). Fall back
            // to the global generation for pathological callers rather than
            // allocating or iterating an attacker-sized rectangle.
            return SnapshotRegionRevision {
                epoch: self.snapshot_revision,
                chunks: Vec::new(),
            };
        }
        let mut chunks = Vec::with_capacity(chunk_count as usize);
        for cx in min_cx..=max_cx {
            for cz in min_cz..=max_cz {
                let position = (cx as i32, cz as i32);
                chunks.push((
                    position,
                    self.snapshot_chunk_revisions
                        .get(&position)
                        .copied()
                        .unwrap_or(0),
                ));
            }
        }
        SnapshotRegionRevision {
            epoch: self.snapshot_epoch,
            chunks,
        }
    }

    pub(crate) fn take_snapshot_changes(&mut self) -> SnapshotChanges {
        SnapshotChanges {
            all: std::mem::take(&mut self.snapshot_all_dirty),
            chunks: std::mem::take(&mut self.snapshot_dirty_chunks),
            blocks: std::mem::take(&mut self.snapshot_dirty_blocks),
        }
    }

    fn advance_mesh_epoch(&mut self) {
        self.mesh_epoch = self
            .mesh_epoch
            .checked_add(1)
            .expect("background mesh epoch overflow");
        self.mesh_generations.clear();
    }

    /// Reject every background mesh built against a previous texture layout.
    /// Resource-pack reloads change atlas UVs without changing block data, so
    /// content-generation tokens alone cannot identify those stale results.
    pub fn invalidate_mesh_jobs_for_resource_reload(&mut self) {
        self.advance_mesh_epoch();
        self.pending_mesh_queue.clear();
        self.pending_mesh_priority_queue.clear();
        self.pending_mesh_removals.clear();
        self.pending_mesh_inflight.clear();
        self.pending_mesh_dirty.clear();
        self.pending_mesh_priority_inflight.clear();
        self.pending_mesh_priority_dirty.clear();
    }

    fn mesh_job_token(&self, position: (i32, i32)) -> MeshJobToken {
        MeshJobToken {
            epoch: self.mesh_epoch,
            generation: self.mesh_generations.get(&position).copied().unwrap_or(0),
        }
    }

    fn invalidate_chunk_mesh(&mut self, position: (i32, i32)) {
        let generation = self.mesh_generations.entry(position).or_insert(0);
        *generation = generation
            .checked_add(1)
            .expect("background mesh generation overflow");
    }

    fn mark_snapshot_changed(&mut self) {
        self.advance_mesh_epoch();
        self.snapshot_revision = self.snapshot_revision.wrapping_add(1);
        self.snapshot_epoch = self.snapshot_revision;
        self.snapshot_chunk_revisions.clear();
        self.snapshot_dirty_blocks.clear();
        self.snapshot_dirty_chunks.clear();
        self.snapshot_all_dirty = true;
    }

    fn mark_snapshot_chunk_changed(&mut self, cx: i32, cz: i32) {
        self.snapshot_revision = self.snapshot_revision.wrapping_add(1);
        if self.chunks.contains_key(&(cx, cz)) {
            self.snapshot_chunk_revisions
                .insert((cx, cz), self.snapshot_revision);
        } else {
            self.snapshot_chunk_revisions.remove(&(cx, cz));
        }
        self.snapshot_dirty_chunks.insert((cx, cz));
    }

    fn mark_snapshot_chunks_changed(&mut self, chunks: &[(i32, i32)]) {
        if chunks.is_empty() {
            return;
        }
        self.snapshot_revision = self.snapshot_revision.wrapping_add(1);
        let revision = self.snapshot_revision;
        for &(cx, cz) in chunks {
            if self.chunks.contains_key(&(cx, cz)) {
                self.snapshot_chunk_revisions.insert((cx, cz), revision);
            } else {
                self.snapshot_chunk_revisions.remove(&(cx, cz));
            }
            self.snapshot_dirty_chunks.insert((cx, cz));
        }
    }

    fn mark_snapshot_block_changed(&mut self, wx: i32, wy: i32, wz: i32) {
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        self.snapshot_revision = self.snapshot_revision.wrapping_add(1);
        self.snapshot_chunk_revisions
            .insert((cx, cz), self.snapshot_revision);
        self.snapshot_dirty_blocks.insert((wx, wy, wz));
    }

    /// Equivalent to 1.8.9 `Block.isNormalCube()` for the block-state model
    /// represented by this client. Normal cubes are opaque full blocks; all
    /// blocks with state-dependent or non-full shapes are excluded.
    pub fn is_normal_cube(&self, wx: i32, wy: i32, wz: i32) -> bool {
        let block = self.get_block(wx, wy, wz);
        block.properties().is_opaque
            && !crate::world::shape::has_custom_shape(block)
            // BlockCompressedPowered (the redstone block) is a full opaque
            // cube but overrides canProvidePower, so Block.isNormalCube is
            // false. Other vanilla power providers are custom-shape blocks.
            && block != Block::RedstoneBlock
    }

    pub fn get_block_state(&self, wx: i32, wy: i32, wz: i32) -> u16 {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return 0;
        }
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        let lx = wx.rem_euclid(CHUNK_SIZE as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_SIZE as i32) as usize;
        self.chunks
            .get(&(cx, cz))
            .map_or(0, |chunk| chunk.state(lx, wy as usize, lz))
    }

    pub fn get_block_metadata(&self, wx: i32, wy: i32, wz: i32) -> u8 {
        (self.get_block_state(wx, wy, wz) & 0x0f) as u8
    }

    /// Check if the eye position is underwater (inside a water block).
    pub fn is_water_at(&self, x: f32, y: f32, z: f32) -> bool {
        let wx = x.floor() as i32;
        let wy = y.floor() as i32;
        let wz = z.floor() as i32;
        matches!(
            self.get_block(wx, wy, wz),
            Block::FlowingWater | Block::StillWater
        )
    }

    pub fn set_block(&mut self, wx: i32, wy: i32, wz: i32, block: Block) -> bool {
        self.set_block_state(wx, wy, wz, block.to_id() << 4)
    }

    /// Mirrors vanilla `Chunk#setBlockState`: an identical complete state is a
    /// no-op, and cached light is invalidated only when opacity/emission changes.
    pub fn set_block_state(&mut self, wx: i32, wy: i32, wz: i32, block_state: u16) -> bool {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return false;
        }
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        let lx = wx.rem_euclid(CHUNK_SIZE as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_SIZE as i32) as usize;
        let Some(previous_state) = self
            .chunks
            .get(&(cx, cz))
            .map(|chunk| chunk.state(lx, wy as usize, lz))
        else {
            return false;
        };
        if previous_state == block_state {
            return false;
        }
        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            let chunk = Arc::make_mut(chunk);
            chunk.set_state(lx, wy as usize, lz, block_state);
            // Vanilla keeps the chunk light arrays and incrementally updates
            // them through World#checkLight. Do not invalidate an entire
            // server light snapshot for one changed voxel.
            self.mark_snapshot_block_changed(wx, wy, wz);
        }
        self.invalidate_chunk_mesh((cx, cz));
        if Block::from_state(block_state) != Block::Skull {
            self.skulls.remove(&(wx, wy, wz));
        }
        if is_chest_block(Block::from_state(block_state)) {
            self.chests.entry((wx, wy, wz)).or_default();
        } else {
            self.chests.remove(&(wx, wy, wz));
        }
        true
    }

    pub fn set_block_and_relight(&mut self, wx: i32, wy: i32, wz: i32, block: Block) -> bool {
        self.set_block_state_and_relight(wx, wy, wz, block.to_id() << 4)
    }

    pub fn set_block_state_and_relight(
        &mut self,
        wx: i32,
        wy: i32,
        wz: i32,
        block_state: u16,
    ) -> bool {
        let previous_state = self.get_block_state(wx, wy, wz);
        if !self.set_block_state(wx, wy, wz, block_state) {
            return false;
        }
        if block_change_affects_light(
            Block::from_state(previous_state),
            Block::from_state(block_state),
        ) {
            self.update_light_around_positions(&[(wx, wy, wz)]);
        }
        true
    }

    pub fn recompute_all_light(&mut self) {
        for chunk in self.chunks.values() {
            self.light.compute_chunk(chunk);
        }
        if !self.chunks.is_empty() {
            self.mark_snapshot_changed();
        }
    }

    pub fn recompute_chunk_light(&mut self, cx: i32, cz: i32) {
        if self.chunks.contains_key(&(cx, cz)) {
            let chunk = self
                .chunks
                .get(&(cx, cz))
                .expect("chunk existence checked above");
            self.light.compute_chunk(chunk);
            self.invalidate_chunk_mesh((cx, cz));
            self.mark_snapshot_chunk_changed(cx, cz);
        }
    }

    fn update_light_around_positions(&mut self, positions: &[(i32, i32, i32)]) {
        let mut changed_chunks = HashSet::new();
        for &position in positions {
            changed_chunks.extend(self.light.update_around(&self.chunks, position));
        }
        let mut changed_chunks = changed_chunks.into_iter().collect::<Vec<_>>();
        changed_chunks.sort_unstable();
        for &(cx, cz) in &changed_chunks {
            // Light can cross a chunk edge, so every chunk whose stored light
            // changed needs a mesh refresh. These remain normal-lane jobs.
            self.enqueue_chunk_mesh(cx, cz);
        }
        self.mark_snapshot_chunks_changed(&changed_chunks);
    }

    fn light_at(&self, wx: i32, wy: i32, wz: i32) -> light::LightLevel {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return light::LightLevel { sky: 15, block: 0 };
        }
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        let lx = wx.rem_euclid(CHUNK_SIZE as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_SIZE as i32) as usize;
        if let Some(local) = self.light.chunk(cx, cz) {
            return local.get(lx, wy as usize, lz);
        }
        if let Some(chunk) = self.chunks.get(&(cx, cz)) {
            if chunk.has_valid_network_light() {
                let (sky, block) = chunk.light_at(lx, wy as usize, lz);
                return light::LightLevel { sky, block };
            }
        }
        self.light.get(cx, cz, lx, wy as usize, lz)
    }

    /// Returns the lightmap value used by block and entity rendering at a world position.
    pub fn light_at_world(&self, wx: i32, wy: i32, wz: i32) -> light::LightLevel {
        self.light_at(wx, wy, wz)
    }

    pub fn set_smooth_lighting(&mut self, enabled: bool) {
        self.mesh_options.smooth_lighting = enabled;
    }

    pub fn smooth_lighting(&self) -> bool {
        self.mesh_options.smooth_lighting
    }

    pub fn set_sky_brightness(&mut self, brightness: f32) {
        self.mesh_options.sky_brightness = brightness;
    }

    /// Build meshes for all chunks. Vertices are in LOCAL chunk space.
    pub fn build_all_meshes(&self) -> Vec<ChunkMesh> {
        use rayon::prelude::*;
        self.chunks
            .par_iter()
            .map(|((_cx, _cz), chunk)| {
                mesh::build_chunk_mesh(
                    chunk,
                    |wx, wy, wz| self.get_block(wx, wy, wz),
                    |wx, wy, wz| self.light_at(wx, wy, wz),
                    |wx, wy, wz| self.get_block_state(wx, wy, wz),
                    self.mesh_options,
                )
            })
            .filter(|m| !m.is_empty())
            .collect()
    }

    /// Rebuild only the mesh for a specific chunk (and neighbors if on border).
    /// Returns list of updated meshes.
    pub fn rebuild_chunk_at(&mut self, wx: i32, _wy: i32, wz: i32) -> Vec<ChunkMesh> {
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);

        let mut to_rebuild = vec![(cx, cz)];
        to_rebuild.extend(chunk_and_border_neighbors(wx, wz));
        to_rebuild.sort_unstable();
        to_rebuild.dedup();

        let mut meshes = Vec::new();
        for (rcx, rcz) in to_rebuild {
            let position = (rcx, rcz);
            if !self.chunks.contains_key(&position) {
                continue;
            }
            // This synchronous mesh is built from the latest world state. Any
            // queued work is now redundant, while an already-running job must
            // remain tracked until its tokened result arrives and is rejected.
            self.invalidate_chunk_mesh(position);
            self.pending_mesh_queue.retain(|queued| *queued != position);
            self.pending_mesh_priority_queue
                .retain(|queued| *queued != position);
            self.pending_mesh_dirty.remove(&position);
            self.pending_mesh_priority_dirty.remove(&position);
            if let Some(chunk) = self.chunks.get(&(rcx, rcz)) {
                let mesh = mesh::build_chunk_mesh(
                    chunk,
                    |wx, wy, wz| self.get_block(wx, wy, wz),
                    |wx, wy, wz| self.light_at(wx, wy, wz),
                    |wx, wy, wz| self.get_block_state(wx, wy, wz),
                    self.mesh_options,
                );
                if !mesh.is_empty() {
                    meshes.push(mesh);
                }
            }
        }
        meshes
    }

    /// Build the changed chunk immediately for local placement prediction.
    /// This deliberately keeps the queued background job: the provisional
    /// mesh uses the last server light nibbles so it can be displayed now,
    /// then the background result replaces it with recomputed lighting.
    pub fn build_immediate_mesh_at_block(&self, wx: i32, wz: i32) -> Option<ChunkMesh> {
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        let chunk = self.chunks.get(&(cx, cz))?;
        let provisional_light = |lx: i32, ly: i32, lz: i32| {
            if ly < 0 || ly >= CHUNK_HEIGHT as i32 {
                return light::LightLevel { sky: 15, block: 0 };
            }
            let light_cx = lx.div_euclid(CHUNK_SIZE as i32);
            let light_cz = lz.div_euclid(CHUNK_SIZE as i32);
            let local_x = lx.rem_euclid(CHUNK_SIZE as i32) as usize;
            let local_z = lz.rem_euclid(CHUNK_SIZE as i32) as usize;
            if let Some(local) = self.light.chunk(light_cx, light_cz) {
                return local.get(local_x, ly as usize, local_z);
            }
            if let Some(light_chunk) = self.chunks.get(&(light_cx, light_cz)) {
                let (sky, block) = light_chunk.light_at(local_x, ly as usize, local_z);
                light::LightLevel { sky, block }
            } else {
                self.light
                    .get(light_cx, light_cz, local_x, ly as usize, local_z)
            }
        };
        Some(mesh::build_chunk_mesh(
            chunk,
            |x, y, z| self.get_block(x, y, z),
            provisional_light,
            |x, y, z| self.get_block_state(x, y, z),
            self.mesh_options,
        ))
    }

    /// Populate the pending mesh queue with all current chunks.
    /// Called once when transitioning from LoadingWorld to Playing.
    pub fn enqueue_all_chunks_for_mesh(&mut self) {
        self.advance_mesh_epoch();
        self.pending_mesh_queue.clear();
        self.pending_mesh_priority_queue.clear();
        self.pending_mesh_removals.clear();
        self.pending_mesh_inflight.clear();
        self.pending_mesh_dirty.clear();
        self.pending_mesh_priority_inflight.clear();
        self.pending_mesh_priority_dirty.clear();
        let chunks: Vec<(i32, i32)> = self.chunks.keys().copied().collect();
        for (cx, cz) in chunks {
            self.enqueue_chunk_mesh(cx, cz);
        }
    }

    pub fn enqueue_chunk_mesh(&mut self, cx: i32, cz: i32) {
        let position = (cx, cz);
        if !self.chunks.contains_key(&position) {
            return;
        }
        self.invalidate_chunk_mesh(position);
        if self.pending_mesh_inflight.contains_key(&position) {
            self.pending_mesh_dirty.insert(position);
        } else if !self.pending_mesh_queue.contains(&position)
            && !self.pending_mesh_priority_queue.contains(&position)
        {
            self.pending_mesh_queue.push_back((cx, cz));
        }
    }

    fn enqueue_chunk_mesh_priority(&mut self, cx: i32, cz: i32) {
        let position = (cx, cz);
        if !self.chunks.contains_key(&position) {
            return;
        }
        self.invalidate_chunk_mesh(position);
        if self.pending_mesh_inflight.contains_key(&position) {
            self.pending_mesh_dirty.insert(position);
            self.pending_mesh_priority_dirty.insert(position);
        } else {
            self.pending_mesh_queue.retain(|queued| *queued != position);
            if !self.pending_mesh_priority_queue.contains(&position) {
                self.pending_mesh_priority_queue.push_back(position);
            }
        }
    }

    pub fn enqueue_chunk_mesh_with_neighbors(&mut self, cx: i32, cz: i32) {
        self.enqueue_chunk_mesh(cx, cz);
        self.enqueue_chunk_mesh(cx - 1, cz);
        self.enqueue_chunk_mesh(cx + 1, cz);
        self.enqueue_chunk_mesh(cx, cz - 1);
        self.enqueue_chunk_mesh(cx, cz + 1);
    }

    pub fn enqueue_chunk_mesh_at_block(&mut self, wx: i32, wz: i32) {
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        for (ncx, ncz) in chunk_and_border_neighbors(wx, wz) {
            self.enqueue_chunk_mesh(ncx, ncz);
        }
        self.enqueue_chunk_mesh_priority(cx, cz);
    }

    fn enqueue_chunk_mesh_at_block_normal(&mut self, wx: i32, wz: i32) {
        let cx = wx.div_euclid(CHUNK_SIZE as i32);
        let cz = wz.div_euclid(CHUNK_SIZE as i32);
        self.enqueue_chunk_mesh(cx, cz);
        for (ncx, ncz) in chunk_and_border_neighbors(wx, wz) {
            self.enqueue_chunk_mesh(ncx, ncz);
        }
    }

    pub fn enqueue_chunk_mesh_removal(&mut self, cx: i32, cz: i32) {
        let position = (cx, cz);
        self.invalidate_chunk_mesh(position);
        self.pending_mesh_queue.retain(|queued| *queued != position);
        self.pending_mesh_priority_queue
            .retain(|queued| *queued != position);
        self.pending_mesh_inflight.remove(&position);
        self.pending_mesh_dirty.remove(&position);
        self.pending_mesh_priority_inflight.remove(&position);
        self.pending_mesh_priority_dirty.remove(&position);
        if !self.pending_mesh_removals.contains(&position) {
            self.pending_mesh_removals.push_back(position);
        }
    }

    pub fn has_pending_mesh_work(&self) -> bool {
        !self.pending_mesh_removals.is_empty()
            || !self.pending_mesh_priority_queue.is_empty()
            || !self.pending_mesh_queue.is_empty()
    }

    pub fn poll_finished_meshes(&mut self) -> Vec<ChunkMesh> {
        let mut meshes = Vec::new();
        let mut finished_lights = Vec::new();
        let results = {
            let Ok(rx) = self.mesh_result_rx.lock() else {
                return meshes;
            };
            let mut results = Vec::with_capacity(MAX_MESH_RESULTS_PER_FRAME);
            for _ in 0..MAX_MESH_RESULTS_PER_FRAME {
                let Ok(result) = rx.try_recv() else {
                    break;
                };
                results.push(result);
            }
            results
        };
        for result in results {
            let mesh = result.mesh;
            let position = (mesh.cx, mesh.cz);
            let Some(inflight_token) = self.pending_mesh_inflight.get(&position).copied() else {
                continue;
            };
            if result.token != inflight_token {
                continue;
            }
            self.pending_mesh_inflight.remove(&position);
            let was_priority = self.pending_mesh_priority_inflight.remove(&position);
            if !self.chunks.contains_key(&position) {
                self.pending_mesh_dirty.remove(&position);
                self.pending_mesh_priority_dirty.remove(&position);
                continue;
            }
            let dirty = self.pending_mesh_dirty.remove(&position);
            let priority_dirty = self.pending_mesh_priority_dirty.remove(&position);
            if dirty || result.token != self.mesh_job_token(position) {
                if dirty && (was_priority || priority_dirty) {
                    self.pending_mesh_queue.retain(|queued| *queued != position);
                    if !self.pending_mesh_priority_queue.contains(&position) {
                        self.pending_mesh_priority_queue.push_back(position);
                    }
                } else if dirty
                    && !self.pending_mesh_queue.contains(&position)
                    && !self.pending_mesh_priority_queue.contains(&position)
                {
                    self.pending_mesh_queue.push_back(position);
                }
                continue;
            }
            if let Some(light) = result.light {
                finished_lights.push((position, light));
            }
            meshes.push(mesh);
        }
        for ((cx, cz), light) in finished_lights {
            self.light.insert_chunk(cx, cz, light);
            self.mark_snapshot_chunk_changed(cx, cz);
        }
        meshes
    }

    pub fn schedule_background_meshes(&mut self) -> Vec<ChunkMesh> {
        let mut removals = Vec::new();
        for _ in 0..MAX_MESH_REMOVALS_PER_FRAME {
            let Some((cx, cz)) = self.pending_mesh_removals.pop_front() else {
                break;
            };
            removals.push(ChunkMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                cx,
                cz,
                transparent_start: 0,
                aabb_min: [f32::MAX; 3],
                aabb_max: [f32::MIN; 3],
            });
        }

        while self.pending_mesh_priority_inflight.len() < MAX_PRIORITY_BACKGROUND_MESH_JOBS {
            let Some((cx, cz)) = self.pending_mesh_priority_queue.pop_front() else {
                break;
            };
            let position = (cx, cz);
            if !self.chunks.contains_key(&position)
                || self.pending_mesh_inflight.contains_key(&position)
            {
                continue;
            }
            self.spawn_background_mesh_job(cx, cz, true);
        }

        while self
            .pending_mesh_inflight
            .len()
            .saturating_sub(self.pending_mesh_priority_inflight.len())
            < MAX_NORMAL_BACKGROUND_MESH_JOBS
        {
            let Some((cx, cz)) = self.pending_mesh_queue.pop_front() else {
                break;
            };
            let position = (cx, cz);
            if !self.chunks.contains_key(&position)
                || self.pending_mesh_inflight.contains_key(&position)
            {
                continue;
            }
            self.spawn_background_mesh_job(cx, cz, false);
        }

        removals
    }

    fn spawn_background_mesh_job(&mut self, cx: i32, cz: i32, priority: bool) {
        let position = (cx, cz);
        let token = self.mesh_job_token(position);
        self.pending_mesh_inflight.insert(position, token);
        if priority {
            self.pending_mesh_priority_inflight.insert(position);
        }
        let snapshot = self.mesh_snapshot(cx, cz);
        let options = self.mesh_options;
        let tx = self.mesh_result_tx.clone();
        rayon::spawn(move || {
            let result = build_mesh_from_snapshot(cx, cz, snapshot, options, token);
            let _ = tx.send(result);
        });
    }

    fn mesh_snapshot(&self, cx: i32, cz: i32) -> HashMap<(i32, i32), MeshChunkSnapshot> {
        let mut snapshot = HashMap::with_capacity(9);
        for dx in -1..=1 {
            for dz in -1..=1 {
                let (Some(snapshot_cx), Some(snapshot_cz)) =
                    (cx.checked_add(dx), cz.checked_add(dz))
                else {
                    continue;
                };
                let coord = (snapshot_cx, snapshot_cz);
                if let Some(chunk) = self.chunks.get(&coord) {
                    let local_light = self.light.shared_chunk(coord.0, coord.1);
                    let network_light_valid =
                        chunk.has_valid_network_light() && local_light.is_none();
                    snapshot.insert(
                        coord,
                        MeshChunkSnapshot {
                            chunk: chunk.clone(),
                            network_light_valid,
                            local_light,
                        },
                    );
                }
            }
        }
        snapshot
    }

    /// Build up to `budget` chunk meshes from the pending queue.
    /// Returns the built meshes for GPU upload.
    pub fn build_pending_meshes(&mut self, budget: usize) -> Vec<ChunkMesh> {
        let mut meshes = Vec::new();
        for _ in 0..budget {
            if let Some((cx, cz)) = self.pending_mesh_removals.pop_front() {
                meshes.push(ChunkMesh {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    cx,
                    cz,
                    transparent_start: 0,
                    aabb_min: [f32::MAX; 3],
                    aabb_max: [f32::MIN; 3],
                });
                continue;
            }
            let next = self
                .pending_mesh_priority_queue
                .pop_front()
                .or_else(|| self.pending_mesh_queue.pop_front());
            let Some((cx, cz)) = next else {
                break;
            };
            if let Some(chunk) = self.chunks.get(&(cx, cz)) {
                // Compute light before building mesh (deferred from network path)
                self.light.compute_chunk(chunk);
                let mesh = mesh::build_chunk_mesh(
                    chunk,
                    |wx, wy, wz| self.get_block(wx, wy, wz),
                    |wx, wy, wz| self.light_at(wx, wy, wz),
                    |wx, wy, wz| self.get_block_state(wx, wy, wz),
                    self.mesh_options,
                );
                // Deferred light computation can change the values exposed by
                // the scripting snapshot even though block data is stable.
                self.mark_snapshot_chunk_changed(cx, cz);
                if mesh.vertices.is_empty() {
                    log::debug!("chunk mesh is empty: chunk=({cx},{cz})");
                }
                meshes.push(mesh);
            }
        }
        meshes
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ChestBlockEntity {
    pub viewers: i32,
    pub prev_lid_angle: f32,
    pub lid_angle: f32,
}

pub(crate) fn is_chest_block(block: Block) -> bool {
    matches!(
        block,
        Block::Chest | Block::TrappedChest | Block::EnderChest
    )
}

#[derive(Clone, Debug, Default)]
pub struct SkullBlockEntity {
    pub skull_type: u8,
    pub rotation: u8,
    pub owner_uuid: Option<String>,
    pub owner_name: Option<String>,
    pub skin_property: Option<String>,
}

impl World {
    pub fn apply_block_entity_update(&mut self, x: i32, y: i32, z: i32, action: u8, nbt: &[u8]) {
        // Vanilla 1.8.9 S35 action 4 = skull tile entity.
        if action != 4 || self.get_block(x, y, z) != Block::Skull {
            return;
        }
        match crate::net::nbt::parse_root(nbt) {
            Ok(tag) => {
                if let Some(skull) = SkullBlockEntity::from_nbt(&tag) {
                    self.skulls.insert((x, y, z), skull);
                }
            }
            Err(err) => {
                log::warn!("failed to parse skull tile entity at ({x},{y},{z}): {err}")
            }
        }
    }
}

impl SkullBlockEntity {
    fn from_nbt(tag: &crate::net::nbt::NbtTag) -> Option<Self> {
        let root = tag.as_compound()?;
        let skull_type = nbt_u8(root, "SkullType").unwrap_or(0);
        let rotation = nbt_u8(root, "Rot").unwrap_or(0);

        let mut owner_uuid = None;
        let mut owner_name = None;
        let mut skin_property = None;

        if let Some(owner) = root
            .get("Owner")
            .and_then(crate::net::nbt::NbtTag::as_compound)
        {
            owner_uuid = owner
                .get("Id")
                .and_then(crate::net::nbt::NbtTag::as_str)
                .filter(|s| !s.is_empty())
                .map(normalize_uuid);
            owner_name = owner
                .get("Name")
                .and_then(crate::net::nbt::NbtTag::as_str)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);
            skin_property = owner
                .get("Properties")
                .and_then(crate::net::nbt::NbtTag::as_compound)
                .and_then(|props| props.get("textures"))
                .and_then(crate::net::nbt::NbtTag::as_list)
                .and_then(|textures| textures.first())
                .and_then(crate::net::nbt::NbtTag::as_compound)
                .and_then(|texture| texture.get("Value"))
                .and_then(crate::net::nbt::NbtTag::as_str)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);
        }

        if owner_name.is_none() {
            owner_name = root
                .get("ExtraType")
                .and_then(crate::net::nbt::NbtTag::as_str)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);
        }

        Some(Self {
            skull_type,
            rotation,
            owner_uuid,
            owner_name,
            skin_property,
        })
    }
}

fn nbt_u8(map: &HashMap<String, crate::net::nbt::NbtTag>, key: &str) -> Option<u8> {
    map.get(key)
        .and_then(crate::net::nbt::NbtTag::as_i32)
        .map(|value| value.clamp(0, u8::MAX as i32) as u8)
}

fn normalize_uuid(value: &str) -> String {
    let compact: String = value.chars().filter(|c| *c != '-').collect();
    if compact.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &compact[0..8],
            &compact[8..12],
            &compact[12..16],
            &compact[16..20],
            &compact[20..32]
        )
    } else {
        value.to_owned()
    }
}

fn chunk_and_border_neighbors(wx: i32, wz: i32) -> Vec<(i32, i32)> {
    let cx = wx.div_euclid(CHUNK_SIZE as i32);
    let cz = wz.div_euclid(CHUNK_SIZE as i32);
    let lx = wx.rem_euclid(CHUNK_SIZE as i32);
    let lz = wz.rem_euclid(CHUNK_SIZE as i32);
    let mut out = Vec::with_capacity(4);
    if lx == 0 {
        out.push((cx - 1, cz));
    }
    if lx == CHUNK_SIZE as i32 - 1 {
        out.push((cx + 1, cz));
    }
    if lz == 0 {
        out.push((cx, cz - 1));
    }
    if lz == CHUNK_SIZE as i32 - 1 {
        out.push((cx, cz + 1));
    }
    out
}

#[inline]
fn block_change_affects_light(previous: Block, next: Block) -> bool {
    light::light_opacity(previous) != light::light_opacity(next)
        || light::block_light_emission(previous) != light::block_light_emission(next)
}

fn build_mesh_from_snapshot(
    cx: i32,
    cz: i32,
    chunks: HashMap<(i32, i32), MeshChunkSnapshot>,
    options: MeshOptions,
    token: MeshJobToken,
) -> MeshBuildResult {
    let Some(center) = chunks.get(&(cx, cz)) else {
        return MeshBuildResult {
            token,
            mesh: ChunkMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                cx,
                cz,
                transparent_start: 0,
                aabb_min: [f32::MAX; 3],
                aabb_max: [f32::MIN; 3],
            },
            light: None,
        };
    };

    // Meshes are generated off-thread, so they cannot borrow World::light.
    // Recreate the same source used by the synchronous path from this chunk
    // snapshot: server nibbles first, then local propagation for invalidated
    // chunks. This prevents a changed chunk from falling back to sky=15 and
    // emission-only block light.
    let chunk = center.chunk.as_ref();
    let center_light_was_missing = !center.network_light_valid && center.local_light.is_none();
    let mut missing_light = chunks
        .iter()
        .filter_map(|(&position, source)| {
            (!source.network_light_valid && source.local_light.is_none()).then_some(position)
        })
        .collect::<Vec<_>>();
    let mut snapshot_light = light::WorldLight::new();
    if !missing_light.is_empty() {
        // Seed immutable server/local sources first. The validity decision and
        // Arc are captured on the main thread, so a later invalidation cannot
        // change the meaning of a running job; its token will reject the result.
        for (&position, source) in &chunks {
            if source.network_light_valid {
                snapshot_light.compute_chunk_with_validity(&source.chunk, true);
            } else if let Some(local) = &source.local_light {
                snapshot_light.insert_chunk(position.0, position.1, local.clone());
            }
        }

        // A light level reaches at most 15 blocks while a chunk is 16 wide.
        // Two deterministic sweeps across this 3x3 snapshot cover both paths:
        // outward from the target and inward from neighboring/diagonal sources.
        // This avoids HashMap iteration order changing cross-chunk lighting.
        missing_light.sort_unstable_by_key(|&(lcx, lcz)| {
            (
                (i64::from(lcx) - i64::from(cx)).abs() + (i64::from(lcz) - i64::from(cz)).abs(),
                lcx,
                lcz,
            )
        });
        for &(lcx, lcz) in &missing_light {
            snapshot_light.compute_chunk_with_validity(&chunks[&(lcx, lcz)].chunk, false);
        }
        for &(lcx, lcz) in missing_light.iter().rev() {
            snapshot_light.compute_chunk_with_validity(&chunks[&(lcx, lcz)].chunk, false);
        }
    }

    let get_chunk = |wx: i32, wz: i32| {
        let ncx = wx.div_euclid(CHUNK_SIZE as i32);
        let ncz = wz.div_euclid(CHUNK_SIZE as i32);
        chunks.get(&(ncx, ncz))
    };
    let local = |wx: i32, wz: i32| {
        (
            wx.rem_euclid(CHUNK_SIZE as i32) as usize,
            wz.rem_euclid(CHUNK_SIZE as i32) as usize,
        )
    };

    let mesh = mesh::build_chunk_mesh(
        chunk,
        |wx, wy, wz| {
            if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
                return Block::Air;
            }
            let (lx, lz) = local(wx, wz);
            get_chunk(wx, wz).map_or(Block::Air, |source| source.chunk.get(lx, wy as usize, lz))
        },
        |wx, wy, wz| {
            if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
                return light::LightLevel { sky: 0, block: 0 };
            }
            let (lx, lz) = local(wx, wz);
            let ncx = wx.div_euclid(CHUNK_SIZE as i32);
            let ncz = wz.div_euclid(CHUNK_SIZE as i32);
            get_chunk(wx, wz).map_or(light::LightLevel { sky: 0, block: 0 }, |source| {
                if source.network_light_valid {
                    let (sky, block) = source.chunk.light_at(lx, wy as usize, lz);
                    light::LightLevel { sky, block }
                } else if let Some(local) = &source.local_light {
                    local.get(lx, wy as usize, lz)
                } else {
                    snapshot_light.get(ncx, ncz, lx, wy as usize, lz)
                }
            })
        },
        |wx, wy, wz| {
            if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
                return 0;
            }
            let (lx, lz) = local(wx, wz);
            get_chunk(wx, wz).map_or(0, |source| source.chunk.state(lx, wy as usize, lz))
        },
        options,
    );
    let light = center_light_was_missing
        .then(|| snapshot_light.take_chunk(cx, cz))
        .flatten();
    MeshBuildResult { token, mesh, light }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn begin_tracked_mesh_job(world: &mut World, position: (i32, i32)) -> MeshJobToken {
        world.enqueue_chunk_mesh(position.0, position.1);
        assert_eq!(world.pending_mesh_queue.pop_front(), Some(position));
        let token = world.mesh_job_token(position);
        assert!(world
            .pending_mesh_inflight
            .insert(position, token)
            .is_none());
        token
    }

    fn begin_tracked_priority_mesh_job(world: &mut World, position: (i32, i32)) -> MeshJobToken {
        world.enqueue_chunk_mesh_priority(position.0, position.1);
        assert_eq!(
            world.pending_mesh_priority_queue.pop_front(),
            Some(position)
        );
        let token = world.mesh_job_token(position);
        assert!(world
            .pending_mesh_inflight
            .insert(position, token)
            .is_none());
        assert!(world.pending_mesh_priority_inflight.insert(position));
        token
    }

    fn send_test_mesh_result(world: &World, position: (i32, i32), token: MeshJobToken) {
        world
            .mesh_result_tx
            .send(MeshBuildResult {
                token,
                mesh: ChunkMesh {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    cx: position.0,
                    cz: position.1,
                    transparent_start: 0,
                    aabb_min: [f32::MAX; 3],
                    aabb_max: [f32::MIN; 3],
                },
                light: None,
            })
            .unwrap();
    }

    #[test]
    fn resource_reload_rejects_meshes_with_the_previous_atlas_uvs() {
        let mut world = World::new();
        let position = (0, 0);
        world.chunks.insert(position, Arc::new(Chunk::new(0, 0)));
        let old_token = begin_tracked_mesh_job(&mut world, position);
        world.pending_mesh_dirty.insert(position);
        world.pending_mesh_priority_queue.push_back(position);
        world.pending_mesh_priority_inflight.insert(position);
        world.pending_mesh_priority_dirty.insert(position);
        world.pending_mesh_removals.push_back(position);

        world.invalidate_mesh_jobs_for_resource_reload();
        send_test_mesh_result(&world, position, old_token);

        assert!(world.pending_mesh_queue.is_empty());
        assert!(world.pending_mesh_priority_queue.is_empty());
        assert!(world.pending_mesh_removals.is_empty());
        assert!(world.pending_mesh_inflight.is_empty());
        assert!(world.pending_mesh_dirty.is_empty());
        assert!(world.pending_mesh_priority_inflight.is_empty());
        assert!(world.pending_mesh_priority_dirty.is_empty());
        assert!(world.poll_finished_meshes().is_empty());
    }

    #[test]
    fn scripting_snapshot_revision_tracks_world_content_changes() {
        let mut world = World::new();
        world.chunks.insert((0, 0), Arc::new(Chunk::new(0, 0)));

        let initial = world.snapshot_revision();
        world.set_block_state(0, 64, 0, 1 << 4);
        assert_ne!(world.snapshot_revision(), initial);

        let after_block = world.snapshot_revision();
        world.recompute_chunk_light(0, 0);
        assert_ne!(world.snapshot_revision(), after_block);

        let after_light = world.snapshot_revision();
        world.unload_chunk(0, 0);
        assert_ne!(world.snapshot_revision(), after_light);
    }

    #[test]
    fn identical_server_block_state_is_a_complete_no_op() {
        let mut chunk = Chunk::new(0, 0);
        chunk.set_state(1, 64, 1, Block::Stone.to_id() << 4);
        let mut world = World::new();
        world.chunks.insert((0, 0), Arc::new(chunk));
        let shared_snapshot = world.chunks[&(0, 0)].clone();
        world.take_snapshot_changes();
        let revision = world.snapshot_revision();

        assert!(!world.set_block_state(1, 64, 1, Block::Stone.to_id() << 4));
        assert!(Arc::ptr_eq(
            &shared_snapshot,
            world.chunks.get(&(0, 0)).unwrap()
        ));
        assert_eq!(world.snapshot_revision(), revision);
        let changes = world.take_snapshot_changes();
        assert!(!changes.all && changes.blocks.is_empty() && changes.chunks.is_empty());

        world.apply_block_change(1, 64, 1, Block::Stone.to_id() << 4);
        assert!(world.pending_mesh_queue.is_empty());
    }

    #[test]
    fn nearby_snapshot_revision_ignores_distant_chunk_changes() {
        let mut world = World::new();
        world.chunks.insert((0, 0), Arc::new(Chunk::new(0, 0)));
        world.chunks.insert((20, 20), Arc::new(Chunk::new(20, 20)));
        let nearby = world.snapshot_revision_around((0, 64, 0), 15);

        assert!(world.set_block_state(320, 64, 320, Block::Stone.to_id() << 4));
        assert_eq!(world.snapshot_revision_around((0, 64, 0), 15), nearby);

        assert!(world.set_block_state(0, 64, 0, Block::Stone.to_id() << 4));
        assert_ne!(world.snapshot_revision_around((0, 64, 0), 15), nearby);
    }

    #[test]
    fn equal_light_properties_preserve_server_light_nibbles() {
        let mut chunk = Chunk::new(0, 0);
        chunk.set_state(1, 64, 1, Block::Stone.to_id() << 4);
        chunk.finish_network_light(true, true);
        let mut world = World::new();
        world.chunks.insert((0, 0), Arc::new(chunk));

        assert!(world.set_block_state(1, 64, 1, Block::Dirt.to_id() << 4));
        assert!(world.chunks[&(0, 0)].has_valid_network_light());

        assert!(world.set_block_state(1, 64, 1, Block::Torch.to_id() << 4));
        assert!(!world.chunks[&(0, 0)].has_valid_network_light());
    }

    #[test]
    fn light_changes_recompute_and_invalidate_every_reachable_chunk() {
        let mut world = World::new();
        for cx in -1..=1 {
            let mut chunk = Chunk::new(cx, 0);
            chunk.finish_network_light(true, true);
            world.chunks.insert((cx, 0), Arc::new(chunk));
        }
        let neighbor_revision = world.snapshot_revision_around((16, 64, 8), 0);

        assert!(world.set_block_state_and_relight(8, 64, 8, Block::Torch.to_id() << 4));
        assert!(!world.chunks[&(1, 0)].has_valid_network_light());
        assert_ne!(
            world.snapshot_revision_around((16, 64, 8), 0),
            neighbor_revision
        );
        assert!(world.light.chunk(1, 0).unwrap().get(0, 64, 8).block > 0);

        assert!(world.set_block_state_and_relight(8, 64, 8, Block::Air.to_id() << 4));
        assert_eq!(world.light.chunk(1, 0).unwrap().get(0, 64, 8).block, 0);
    }

    #[test]
    fn network_block_change_defers_full_light_work_to_the_mesh_job() {
        let mut world = World::new();
        for cx in -1..=1 {
            let mut chunk = Chunk::new(cx, 0);
            chunk.finish_network_light(true, true);
            world.chunks.insert((cx, 0), Arc::new(chunk));
        }

        world.apply_block_change(8, 64, 8, Block::Torch.to_id() << 4);
        assert!(world.light.chunk(0, 0).is_none());
        assert!(!world.chunks[&(0, 0)].has_valid_network_light());
        assert!(world.pending_mesh_priority_queue.contains(&(0, 0)));
        assert!(!world.pending_mesh_queue.contains(&(0, 0)));

        let result = build_mesh_from_snapshot(
            0,
            0,
            world.mesh_snapshot(0, 0),
            MeshOptions::default(),
            world.mesh_job_token((0, 0)),
        );
        assert!(result.light.is_some());
    }

    #[test]
    fn background_light_crosses_chunk_edges_in_both_directions() {
        let mut world = World::new();
        for cx in 0..=1 {
            let mut chunk = Chunk::new(cx, 0);
            chunk.finish_network_light(true, true);
            world.chunks.insert((cx, 0), Arc::new(chunk));
        }

        world.apply_block_change(15, 64, 8, Block::Torch.to_id() << 4);
        let lit = build_mesh_from_snapshot(
            1,
            0,
            world.mesh_snapshot(1, 0),
            MeshOptions::default(),
            world.mesh_job_token((1, 0)),
        )
        .light
        .expect("the invalid neighboring chunk must return computed light");
        assert!(lit.get(0, 64, 8).block > 0);

        world.apply_block_change(15, 64, 8, Block::Air.to_id() << 4);
        let dark = build_mesh_from_snapshot(
            1,
            0,
            world.mesh_snapshot(1, 0),
            MeshOptions::default(),
            world.mesh_job_token((1, 0)),
        )
        .light
        .expect("removing the source must recompute neighboring light");
        assert_eq!(dark.get(0, 64, 8).block, 0);
    }

    #[test]
    fn installed_local_light_is_reused_by_later_mesh_jobs() {
        let mut chunk = Chunk::new(0, 0);
        chunk.set_state(1, 64, 1, Block::Torch.to_id() << 4);
        let mut world = World::new();
        world.chunks.insert((0, 0), Arc::new(chunk));
        world.recompute_chunk_light(0, 0);

        let result = build_mesh_from_snapshot(
            0,
            0,
            world.mesh_snapshot(0, 0),
            MeshOptions::default(),
            world.mesh_job_token((0, 0)),
        );

        assert!(result.light.is_none());
    }

    #[test]
    fn mesh_job_uses_the_captured_network_light_validity() {
        let mut chunk = Chunk::new(0, 0);
        chunk.set_block_light(1, 64, 1, 11);
        chunk.finish_network_light(true, true);
        let mut world = World::new();
        world.chunks.insert((0, 0), Arc::new(chunk));
        let snapshot = world.mesh_snapshot(0, 0);

        world.chunks[&(0, 0)].invalidate_network_light();
        let result = build_mesh_from_snapshot(
            0,
            0,
            snapshot,
            MeshOptions::default(),
            world.mesh_job_token((0, 0)),
        );

        assert!(result.light.is_none());
    }

    #[test]
    fn snapshot_revision_handles_extreme_coordinates_and_radius() {
        let world = World::new();
        let revision = world.snapshot_revision_around((i32::MAX, 0, i32::MIN), i32::MAX);
        assert!(revision.chunks.is_empty());
        assert_eq!(revision.epoch, world.snapshot_revision());
    }

    #[test]
    fn unload_reload_rejects_the_old_mesh_job_without_clearing_the_new_one() {
        let position = (0, 0);
        let mut world = World::new();
        world
            .chunks
            .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        let old_token = begin_tracked_mesh_job(&mut world, position);

        world.unload_chunk(position.0, position.1);
        world
            .chunks
            .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        let new_token = begin_tracked_mesh_job(&mut world, position);
        assert_ne!(old_token, new_token);

        send_test_mesh_result(&world, position, old_token);
        assert!(world.poll_finished_meshes().is_empty());
        assert_eq!(world.pending_mesh_inflight.get(&position), Some(&new_token));

        send_test_mesh_result(&world, position, new_token);
        assert_eq!(world.poll_finished_meshes().len(), 1);
        assert!(!world.pending_mesh_inflight.contains_key(&position));
    }

    #[test]
    fn synchronous_mutation_and_rebuild_reject_an_older_mesh_result() {
        let position = (0, 0);
        let mut world = World::new();
        world
            .chunks
            .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        let old_token = begin_tracked_mesh_job(&mut world, position);

        assert!(world.set_block_state(1, 64, 1, Block::Stone.to_id() << 4));
        assert!(!world.rebuild_chunk_at(1, 64, 1).is_empty());
        assert_ne!(old_token, world.mesh_job_token(position));

        send_test_mesh_result(&world, position, old_token);
        assert!(world.poll_finished_meshes().is_empty());
        assert!(!world.pending_mesh_inflight.contains_key(&position));
        assert!(!world.pending_mesh_queue.contains(&position));
    }

    #[test]
    fn world_epoch_rejects_a_result_sent_after_world_reset() {
        let position = (0, 0);
        let mut world = World::new();
        world
            .chunks
            .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        let old_token = begin_tracked_mesh_job(&mut world, position);

        world.clear_server_world();
        assert!(world.pending_mesh_queue.is_empty());
        assert!(world.pending_mesh_priority_queue.is_empty());
        assert!(world.pending_mesh_inflight.is_empty());
        assert!(world.pending_mesh_priority_inflight.is_empty());
        assert!(world.pending_mesh_dirty.is_empty());
        assert!(world.pending_mesh_priority_dirty.is_empty());
        world
            .chunks
            .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        let new_token = begin_tracked_mesh_job(&mut world, position);
        assert_ne!(old_token.epoch, new_token.epoch);

        // Models an old rayon job that completes only after clear_server_world
        // has drained the results which were already available.
        send_test_mesh_result(&world, position, old_token);
        assert!(world.poll_finished_meshes().is_empty());
        assert_eq!(world.pending_mesh_inflight.get(&position), Some(&new_token));
    }

    #[test]
    fn dirty_inflight_mesh_result_requeues_the_latest_generation() {
        let position = (0, 0);
        let backlog = (1, 0);
        let mut world = World::new();
        world
            .chunks
            .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        world
            .chunks
            .insert(backlog, Arc::new(Chunk::new(backlog.0, backlog.1)));
        let old_token = begin_tracked_mesh_job(&mut world, position);
        world.enqueue_chunk_mesh(backlog.0, backlog.1);

        world.enqueue_chunk_mesh(position.0, position.1);
        assert!(world.pending_mesh_dirty.contains(&position));
        assert_ne!(old_token, world.mesh_job_token(position));

        send_test_mesh_result(&world, position, old_token);
        assert!(world.poll_finished_meshes().is_empty());
        assert!(!world.pending_mesh_inflight.contains_key(&position));
        assert!(!world.pending_mesh_dirty.contains(&position));
        assert_eq!(world.pending_mesh_queue.front(), Some(&backlog));
        assert_eq!(world.pending_mesh_queue.back(), Some(&position));
    }

    #[test]
    fn interactive_block_change_overtakes_streaming_mesh_backlog() {
        let changed = (0, 0);
        let backlog = (5, 5);
        let mut world = World::new();
        for position in [changed, backlog] {
            world
                .chunks
                .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        }
        world.enqueue_chunk_mesh(backlog.0, backlog.1);

        world.enqueue_chunk_mesh_at_block(8, 8);

        assert_eq!(world.pending_mesh_priority_queue.front(), Some(&changed));
        assert!(!world.pending_mesh_queue.contains(&changed));
        assert!(world.pending_mesh_queue.contains(&backlog));
    }

    #[test]
    fn priority_mesh_job_does_not_consume_normal_streaming_slots() {
        let priority = (0, 0);
        let normal_a = (1, 0);
        let normal_b = (2, 0);
        let mut world = World::new();
        for position in [priority, normal_a, normal_b] {
            world
                .chunks
                .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        }
        world.enqueue_chunk_mesh(normal_a.0, normal_a.1);
        world.enqueue_chunk_mesh(normal_b.0, normal_b.1);
        world.enqueue_chunk_mesh_priority(priority.0, priority.1);

        world.schedule_background_meshes();

        assert_eq!(world.pending_mesh_inflight.len(), 3);
        assert_eq!(world.pending_mesh_priority_inflight.len(), 1);
        assert!(world.pending_mesh_priority_inflight.contains(&priority));
        assert!(world.pending_mesh_inflight.contains_key(&normal_a));
        assert!(world.pending_mesh_inflight.contains_key(&normal_b));
    }

    #[test]
    fn multi_block_change_stays_on_the_normal_mesh_lane() {
        let position = (0, 0);
        let mut chunk = Chunk::new(position.0, position.1);
        chunk.set_state(8, 64, 8, Block::Stone.to_id() << 4);
        let mut world = World::new();
        world.chunks.insert(position, Arc::new(chunk));
        let raw_position = (8 << 12) | (8 << 8) | 64;

        world.apply_multi_block_change(
            position.0,
            position.1,
            &[(raw_position, Block::Dirt.to_id() << 4)],
        );

        assert!(world.pending_mesh_priority_queue.is_empty());
        assert!(world.pending_mesh_queue.contains(&position));
    }

    #[test]
    fn dirty_priority_mesh_job_requeues_on_the_priority_lane() {
        let position = (0, 0);
        let mut world = World::new();
        world
            .chunks
            .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        let old_token = begin_tracked_priority_mesh_job(&mut world, position);

        world.enqueue_chunk_mesh(position.0, position.1);
        assert!(world.pending_mesh_dirty.contains(&position));

        send_test_mesh_result(&world, position, old_token);
        assert!(world.poll_finished_meshes().is_empty());
        assert_eq!(world.pending_mesh_priority_queue.front(), Some(&position));
        assert!(!world.pending_mesh_queue.contains(&position));
    }

    #[test]
    fn interactive_update_upgrades_an_inflight_normal_mesh_job() {
        let position = (0, 0);
        let mut world = World::new();
        world
            .chunks
            .insert(position, Arc::new(Chunk::new(position.0, position.1)));
        let old_token = begin_tracked_mesh_job(&mut world, position);

        world.enqueue_chunk_mesh_priority(position.0, position.1);
        assert!(world.pending_mesh_priority_dirty.contains(&position));

        send_test_mesh_result(&world, position, old_token);
        assert!(world.poll_finished_meshes().is_empty());
        assert_eq!(world.pending_mesh_priority_queue.front(), Some(&position));
        assert!(!world.pending_mesh_queue.contains(&position));
    }
}
