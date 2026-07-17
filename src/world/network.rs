//! Server-driven world updates: chunk columns, block changes, and unloads.

use super::block::Block;
use super::chunk::{
    Chunk, CHUNK_SIZE, NIBBLE_SECTION_BYTES, SECTION_COUNT, SECTION_SIZE, SECTION_VOLUME,
};
use super::World;
use crate::world::mesh::ChunkMesh;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct ChunkDataOptions {
    pub has_sky_light: Option<bool>,
}

impl ChunkDataOptions {
    pub fn infer() -> Self {
        Self {
            has_sky_light: None,
        }
    }

    pub fn sky_light(has_sky_light: bool) -> Self {
        Self {
            has_sky_light: Some(has_sky_light),
        }
    }
}

impl World {
    pub fn clear_server_world(&mut self) -> Vec<ChunkMesh> {
        let old_chunks: Vec<(i32, i32)> = self.chunks.keys().copied().collect();
        self.chunks.clear();
        self.skulls.clear();
        self.chests.clear();
        self.light.clear();
        self.pending_mesh_queue.clear();
        self.pending_mesh_priority_queue.clear();
        self.pending_mesh_removals.clear();
        self.pending_mesh_inflight.clear();
        self.pending_mesh_dirty.clear();
        self.pending_mesh_priority_inflight.clear();
        self.pending_mesh_priority_dirty.clear();
        if let Ok(rx) = self.mesh_result_rx.lock() {
            while rx.try_recv().is_ok() {}
        }
        self.mark_snapshot_changed();
        old_chunks
            .into_iter()
            .map(|(cx, cz)| empty_mesh(cx, cz))
            .collect()
    }

    pub fn apply_chunk_data(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        full_chunk: bool,
        primary_bit_mask: u16,
        data: &[u8],
        options: ChunkDataOptions,
    ) -> Vec<ChunkMesh> {
        let chunk = self
            .chunks
            .entry((chunk_x, chunk_z))
            .or_insert_with(|| Arc::new(Chunk::new(chunk_x, chunk_z)));
        let chunk = Arc::make_mut(chunk);

        if full_chunk {
            chunk.clear();
            let min_x = chunk_x * CHUNK_SIZE as i32;
            let min_z = chunk_z * CHUNK_SIZE as i32;
            self.skulls.retain(|(x, _, z), _| {
                !(*x >= min_x
                    && *x < min_x + CHUNK_SIZE as i32
                    && *z >= min_z
                    && *z < min_z + CHUNK_SIZE as i32)
            });
            self.chests.retain(|(x, _, z), _| {
                !(*x >= min_x
                    && *x < min_x + CHUNK_SIZE as i32
                    && *z >= min_z
                    && *z < min_z + CHUNK_SIZE as i32)
            });
        }

        let sections: Vec<usize> = (0..SECTION_COUNT)
            .filter(|section_y| primary_bit_mask & (1 << section_y) != 0)
            .collect();

        let mut offset = 0usize;
        read_block_states(chunk, &sections, data, &mut offset);
        read_block_light(chunk, &sections, data, &mut offset);

        let has_sky_light_data =
            resolve_sky_light_flag(options.has_sky_light, full_chunk, &sections, data);
        if has_sky_light_data {
            read_sky_light(chunk, &sections, data, &mut offset);
        }
        let dimension_has_sky = options.has_sky_light.unwrap_or(has_sky_light_data);
        let light_data_valid = !dimension_has_sky || has_sky_light_data;
        chunk.finish_network_light(dimension_has_sky, light_data_valid);

        if full_chunk {
            read_biomes(chunk, data, offset);
        }

        let mut discovered_chests = HashSet::new();
        for &section_y in &sections {
            let min_y = section_y * SECTION_SIZE;
            for y in min_y..min_y + SECTION_SIZE {
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        if super::is_chest_block(chunk.get(x, y, z)) {
                            discovered_chests.insert((
                                chunk_x * CHUNK_SIZE as i32 + x as i32,
                                y as i32,
                                chunk_z * CHUNK_SIZE as i32 + z as i32,
                            ));
                        }
                    }
                }
            }
        }
        self.chests.retain(|&position, _| {
            let (x, y, z) = position;
            x.div_euclid(CHUNK_SIZE as i32) != chunk_x
                || z.div_euclid(CHUNK_SIZE as i32) != chunk_z
                || !sections.contains(&(y as usize / SECTION_SIZE))
                || discovered_chests.contains(&position)
        });
        for position in discovered_chests {
            self.chests.entry(position).or_default();
        }

        // A replacement/section update makes any locally propagated cache for
        // this coordinate stale. Valid server nibbles remain the authoritative
        // fast path until a later local block change invalidates them.
        // Server chunk light is authoritative. Drop any incremental caches
        // touching this boundary so a reload cannot leave old propagated
        // values on one side of the chunk seam.
        for dx in -1..=1 {
            for dz in -1..=1 {
                self.light.remove_chunk(chunk_x + dx, chunk_z + dz);
            }
        }

        // Light is computed lazily during mesh building (build_pending_meshes).
        // This avoids blocking the main thread during network chunk loading.
        if !self.defer_mesh_build {
            self.enqueue_chunk_mesh_with_neighbors(chunk_x, chunk_z);
        }
        self.mark_snapshot_chunk_changed(chunk_x, chunk_z);
        Vec::new()
    }

    pub fn apply_block_change(
        &mut self,
        wx: i32,
        wy: i32,
        wz: i32,
        block_state: u16,
    ) -> Vec<ChunkMesh> {
        let previous_state = self.get_block_state(wx, wy, wz);
        let lighting_changed = super::block_change_affects_light(
            Block::from_state(previous_state),
            Block::from_state(block_state),
        );
        if self.set_block_state(wx, wy, wz, block_state) {
            if lighting_changed {
                self.update_light_around_positions(&[(wx, wy, wz)]);
            }
            self.enqueue_chunk_mesh_at_block(wx, wz);
        }
        Vec::new()
    }

    pub fn apply_multi_block_change(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        records: &[(u16, u16)],
    ) -> Vec<ChunkMesh> {
        let mut rebuild_positions = Vec::new();
        let mut lighting_positions = Vec::new();
        for (raw, block_state) in records {
            let lx = ((raw >> 12) & 15) as i32;
            let lz = ((raw >> 8) & 15) as i32;
            let y = (raw & 255) as i32;
            let wx = chunk_x * CHUNK_SIZE as i32 + lx;
            let wz = chunk_z * CHUNK_SIZE as i32 + lz;
            let previous_state = self.get_block_state(wx, y, wz);
            if previous_state == *block_state {
                continue;
            }
            let lighting_changed = super::block_change_affects_light(
                Block::from_state(previous_state),
                Block::from_state(*block_state),
            );
            if self.set_block_state(wx, y, wz, *block_state) {
                rebuild_positions.push((wx, wz));
                if lighting_changed {
                    lighting_positions.push((wx, y, wz));
                }
            }
        }
        if rebuild_positions.is_empty() {
            return Vec::new();
        }
        if !lighting_positions.is_empty() {
            self.update_light_around_positions(&lighting_positions);
        }
        for (wx, wz) in rebuild_positions {
            self.enqueue_chunk_mesh_at_block_normal(wx, wz);
        }
        Vec::new()
    }

    pub fn unload_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Vec<ChunkMesh> {
        self.chunks.remove(&(chunk_x, chunk_z));
        let min_x = chunk_x * CHUNK_SIZE as i32;
        let min_z = chunk_z * CHUNK_SIZE as i32;
        self.skulls.retain(|(x, _, z), _| {
            !(*x >= min_x
                && *x < min_x + CHUNK_SIZE as i32
                && *z >= min_z
                && *z < min_z + CHUNK_SIZE as i32)
        });
        self.chests.retain(|(x, _, z), _| {
            !(*x >= min_x
                && *x < min_x + CHUNK_SIZE as i32
                && *z >= min_z
                && *z < min_z + CHUNK_SIZE as i32)
        });
        self.light.remove_chunk(chunk_x, chunk_z);
        self.snapshot_chunk_revisions.remove(&(chunk_x, chunk_z));
        self.snapshot_dirty_chunks.remove(&(chunk_x, chunk_z));
        self.enqueue_chunk_mesh_removal(chunk_x, chunk_z);
        self.enqueue_chunk_mesh_with_neighbors(chunk_x, chunk_z);
        self.mark_snapshot_chunk_changed(chunk_x, chunk_z);
        Vec::new()
    }

    /// Evict chunks farther than `render_distance`+margin from the player.
    /// Returns meshes for removed chunks so the renderer can drop their GPU
    /// data. The margin (1 chunk) prevents thrashing at the boundary.
    pub fn unload_distant_chunks(
        &mut self,
        player_cx: i32,
        player_cz: i32,
        render_distance: u32,
    ) -> Vec<ChunkMesh> {
        let radius = render_distance as i32 + 1; // +1 chunk margin
        let radius_sq = radius * radius;
        let to_remove: Vec<(i32, i32)> = self
            .chunks
            .keys()
            .copied()
            .filter(|(cx, cz)| {
                let dx = cx - player_cx;
                let dz = cz - player_cz;
                dx * dx + dz * dz > radius_sq
            })
            .collect();
        let mut meshes = Vec::with_capacity(to_remove.len());
        for (cx, cz) in to_remove {
            meshes.extend(self.unload_chunk(cx, cz));
        }
        if !meshes.is_empty() {
            log::debug!(
                "evicted {} distant chunks (player=({},{}), rd={})",
                meshes.len(),
                player_cx,
                player_cz,
                render_distance
            );
        }
        meshes
    }

    /// Trim snapshot_chunk_revisions to only keep entries for loaded chunks.
    /// Tombstones for long-unloaded chunks accumulate indefinitely otherwise.
    pub fn trim_snapshot_tombstones(&mut self) {
        if self.snapshot_chunk_revisions.len() <= self.chunks.len() * 2 {
            return;
        }
        let loaded: std::collections::HashSet<(i32, i32)> = self.chunks.keys().copied().collect();
        self.snapshot_chunk_revisions
            .retain(|pos, _| loaded.contains(pos));
    }
}

fn empty_mesh(cx: i32, cz: i32) -> ChunkMesh {
    ChunkMesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        cx,
        cz,
        transparent_start: 0,
        aabb_min: [f32::MAX; 3],
        aabb_max: [f32::MIN; 3],
    }
}

fn read_block_states(chunk: &mut Chunk, sections: &[usize], data: &[u8], offset: &mut usize) {
    for &section_y in sections {
        if *offset + SECTION_VOLUME * 2 > data.len() {
            break;
        }

        for idx in 0..SECTION_VOLUME {
            let lo = data[*offset + idx * 2] as u16;
            let hi = data[*offset + idx * 2 + 1] as u16;
            let block_state = lo | (hi << 8);
            let x = idx & 15;
            let z = (idx >> 4) & 15;
            let y = (idx >> 8) & 15;
            chunk.set_state(x, section_y * SECTION_SIZE + y, z, block_state);
        }

        *offset += SECTION_VOLUME * 2;
    }
}

fn read_block_light(chunk: &mut Chunk, sections: &[usize], data: &[u8], offset: &mut usize) {
    for &section_y in sections {
        if *offset + NIBBLE_SECTION_BYTES > data.len() {
            break;
        }
        let light_data = &data[*offset..*offset + NIBBLE_SECTION_BYTES];
        for idx in 0..SECTION_VOLUME {
            let x = idx & 15;
            let z = (idx >> 4) & 15;
            let y = (idx >> 8) & 15;
            chunk.set_block_light(
                x,
                section_y * SECTION_SIZE + y,
                z,
                nibble_at(light_data, idx),
            );
        }
        *offset += NIBBLE_SECTION_BYTES;
    }
}

fn read_sky_light(chunk: &mut Chunk, sections: &[usize], data: &[u8], offset: &mut usize) {
    for &section_y in sections {
        if *offset + NIBBLE_SECTION_BYTES > data.len() {
            break;
        }
        let light_data = &data[*offset..*offset + NIBBLE_SECTION_BYTES];
        for idx in 0..SECTION_VOLUME {
            let x = idx & 15;
            let z = (idx >> 4) & 15;
            let y = (idx >> 8) & 15;
            chunk.set_sky_light(
                x,
                section_y * SECTION_SIZE + y,
                z,
                nibble_at(light_data, idx),
            );
        }
        *offset += NIBBLE_SECTION_BYTES;
    }
}

fn read_biomes(chunk: &mut Chunk, data: &[u8], offset: usize) {
    if offset + CHUNK_SIZE * CHUNK_SIZE > data.len() {
        return;
    }

    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            chunk.set_biome(x, z, data[offset + z * CHUNK_SIZE + x]);
        }
    }
}

fn resolve_sky_light_flag(
    hint: Option<bool>,
    full_chunk: bool,
    sections: &[usize],
    data: &[u8],
) -> bool {
    let expected_without_sky = SECTION_VOLUME * 2 * sections.len()
        + NIBBLE_SECTION_BYTES * sections.len()
        + if full_chunk {
            CHUNK_SIZE * CHUNK_SIZE
        } else {
            0
        };
    let expected_with_sky = expected_without_sky + NIBBLE_SECTION_BYTES * sections.len();

    let inferred = data.len() >= expected_with_sky;
    hint.unwrap_or(inferred) && inferred
}

fn nibble_at(data: &[u8], idx: usize) -> u8 {
    let byte = data[idx / 2];
    if idx & 1 == 0 {
        byte & 0x0f
    } else {
        (byte >> 4) & 0x0f
    }
}
