//! Swapchain resize handling and uploaded world mesh lifetime.

use ash::vk;

use super::DrawCmd;
use crate::world::chunk::CHUNK_SIZE;
use crate::world::mesh::ChunkMesh;

// Compile-time guarantees for the typed slice view over the staging buffer.
// `Vertex` is `Pod` (derived in `world::mesh::types`); these asserts ensure
// the 4-byte aligned staging offsets stay compatible with `Vertex`'s layout
// so `bytemuck::cast_slice_mut` produces a valid typed view.
const _: () = assert!(std::mem::size_of::<crate::world::mesh::Vertex>() % 4 == 0);
const _: () = assert!(std::mem::align_of::<crate::world::mesh::Vertex>() <= 4);

impl super::Renderer {
    pub(super) fn prepare_chunk_uploads(&mut self, frame: usize) {
        if self.chunk_upload_bytes.is_empty() {
            return;
        }
        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.chunk_upload_buffers[frame],
            &mut self.chunk_upload_allocs[frame],
            &mut self.chunk_upload_capacities[frame],
            vk::BufferUsageFlags::TRANSFER_SRC,
            &self.chunk_upload_bytes,
        );
    }

    pub(super) fn record_chunk_uploads(&mut self, cb: vk::CommandBuffer, frame: usize) {
        if self.chunk_upload_bytes.is_empty() {
            return;
        }
        let staging = self.chunk_upload_buffers[frame]
            .expect("chunk staging buffer must exist when uploads are pending");
        unsafe {
            if !self.chunk_vertex_upload_copies.is_empty() {
                self.device.cmd_copy_buffer(
                    cb,
                    staging,
                    self.chunk_vertex_buffer,
                    &self.chunk_vertex_upload_copies,
                );
            }
            if !self.chunk_index_upload_copies.is_empty() {
                self.device.cmd_copy_buffer(
                    cb,
                    staging,
                    self.chunk_index_buffer,
                    &self.chunk_index_upload_copies,
                );
            }
            let barriers = [
                vk::BufferMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    dst_access_mask: vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
                    buffer: self.chunk_vertex_buffer,
                    offset: 0,
                    size: vk::WHOLE_SIZE,
                    ..Default::default()
                },
                vk::BufferMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    dst_access_mask: vk::AccessFlags::INDEX_READ,
                    buffer: self.chunk_index_buffer,
                    offset: 0,
                    size: vk::WHOLE_SIZE,
                    ..Default::default()
                },
            ];
            self.device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::VERTEX_INPUT,
                vk::DependencyFlags::empty(),
                &[],
                &barriers,
                &[],
            );
        }
        self.chunk_upload_bytes.clear();
        self.chunk_vertex_upload_copies.clear();
        self.chunk_index_upload_copies.clear();
    }

    pub fn recreate_swapchain(&mut self) {
        self.swapchain.recreate(
            &self.device,
            self.resources.allocator_mut(),
            self._physical_device,
            self.render_pass,
        );
    }

    pub fn notify_resize(&mut self, width: u32, height: u32) {
        self.swapchain.notify_resize(width, height);
    }

    pub(super) fn destroy_draw_cmd(&mut self, cmd: DrawCmd) {
        match cmd.storage {
            super::ChunkStorage::Shared {
                first_vertex,
                vertex_count,
                first_index,
                index_count,
            } => {
                self.chunk_vertex_ranges.release(first_vertex, vertex_count);
                self.chunk_index_ranges.release(first_index, index_count);
            }
            super::ChunkStorage::Dedicated {
                vertex_buffer,
                index_buffer,
                vertex_alloc,
                index_alloc,
            } => {
                unsafe {
                    self.device.destroy_buffer(vertex_buffer, None);
                    self.device.destroy_buffer(index_buffer, None);
                }
                self.resources.free(vertex_alloc);
                self.resources.free(index_alloc);
            }
        }
    }

    /// Partial update: replace only the specified chunk meshes (by cx, cz).
    pub fn upload_world_partial(&mut self, meshes: &[ChunkMesh]) {
        let started = std::time::Instant::now();
        let uploaded_bytes = meshes.iter().fold(0u64, |total, mesh| {
            total
                .saturating_add(
                    (mesh.vertices.len() * std::mem::size_of::<crate::world::mesh::Vertex>())
                        as u64,
                )
                .saturating_add((mesh.indices.len() * std::mem::size_of::<u32>()) as u64)
        });
        for mesh in meshes {
            if let Some(idx) = self.draw_cmd_indices.remove(&(mesh.cx, mesh.cz)) {
                let old = self.draw_cmds.swap_remove(idx);
                if idx < self.draw_cmds.len() {
                    let moved = &self.draw_cmds[idx];
                    self.draw_cmd_indices.insert((moved.cx, moved.cz), idx);
                }
                self.pending_retired_draw_cmds.push(old);
            }

            if mesh.is_empty() {
                continue;
            }

            let cmd = self.draw_cmd_from_mesh(mesh);
            self.draw_cmds.push(cmd);
            self.draw_cmd_indices
                .insert((mesh.cx, mesh.cz), self.draw_cmds.len() - 1);
        }
        self.state.frame_profile.set_frame_chunk_upload_us(self.state.frame_profile.frame_chunk_upload_us().saturating_add(started.elapsed().as_micros() as u64));
        self.state.frame_profile.set_frame_chunk_upload_bytes(self.state.frame_profile.frame_chunk_upload_bytes().saturating_add(uploaded_bytes));
        self.state.frame_profile.set_frame_chunk_upload_count(self.state.frame_profile.frame_chunk_upload_count().saturating_add(meshes.len() as u32));
    }

    pub fn upload_world(&mut self, meshes: &[ChunkMesh]) {
        self.pending_retired_draw_cmds
            .extend(self.draw_cmds.drain(..));
        self.draw_cmd_indices.clear();

        for mesh in meshes {
            if !mesh.is_empty() {
                let cmd = self.draw_cmd_from_mesh(mesh);
                self.draw_cmds.push(cmd);
                self.draw_cmd_indices
                    .insert((mesh.cx, mesh.cz), self.draw_cmds.len() - 1);
            }
        }

        log::debug!(
            "chunk mesh upload complete: draw_commands={}",
            self.draw_cmds.len()
        );
    }

    fn draw_cmd_from_mesh(&mut self, mesh: &ChunkMesh) -> DrawCmd {
        let vsize =
            (mesh.vertices.len() * std::mem::size_of::<crate::world::mesh::Vertex>()) as u64;
        let isize = (mesh.indices.len() * std::mem::size_of::<u32>()) as u64;

        let ox = (mesh.cx * CHUNK_SIZE as i32) as f32;
        let oz = (mesh.cz * CHUNK_SIZE as i32) as f32;

        let vertex_count = u32::try_from(mesh.vertices.len()).unwrap_or(u32::MAX);
        let index_count = u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX);
        let shared_ranges = if vertex_count != u32::MAX && index_count != u32::MAX {
            self.chunk_vertex_ranges
                .allocate(vertex_count)
                .and_then(|first_vertex| {
                    if let Some(first_index) = self.chunk_index_ranges.allocate(index_count) {
                        Some((first_vertex, first_index))
                    } else {
                        self.chunk_vertex_ranges.release(first_vertex, vertex_count);
                        None
                    }
                })
        } else {
            None
        };

        let storage = if let Some((first_vertex, first_index)) = shared_ranges {
            let vertex_align = std::mem::align_of::<crate::world::mesh::Vertex>();
            let vertex_src_offset =
                (self.chunk_upload_bytes.len() + vertex_align - 1) & !(vertex_align - 1);
            self.chunk_upload_bytes.resize(vertex_src_offset, 0);
            self.chunk_upload_bytes
                .extend_from_slice(bytemuck::cast_slice(&mesh.vertices));
            // Apply the chunk's world-space offset in place through a typed
            // view over the staging bytes. Safe because `Vertex` is `Pod` and
            // the slice was just filled with exactly `mesh.vertices.len()`
            // vertices by `extend_from_slice` above.
            let vertex_dst: &mut [crate::world::mesh::Vertex] =
                bytemuck::cast_slice_mut(&mut self.chunk_upload_bytes[vertex_src_offset..]);
            for world_vertex in vertex_dst.iter_mut() {
                world_vertex.pos[0] += ox;
                world_vertex.pos[2] += oz;
            }
            self.chunk_vertex_upload_copies.push(vk::BufferCopy {
                src_offset: vertex_src_offset as u64,
                dst_offset: first_vertex as u64
                    * std::mem::size_of::<crate::world::mesh::Vertex>() as u64,
                size: vsize,
            });

            let index_align = std::mem::align_of::<u32>();
            let index_src_offset =
                (self.chunk_upload_bytes.len() + index_align - 1) & !(index_align - 1);
            self.chunk_upload_bytes.resize(index_src_offset, 0);
            self.chunk_upload_bytes
                .extend_from_slice(bytemuck::cast_slice(&mesh.indices));
            self.chunk_index_upload_copies.push(vk::BufferCopy {
                src_offset: index_src_offset as u64,
                dst_offset: first_index as u64 * std::mem::size_of::<u32>() as u64,
                size: isize,
            });
            super::ChunkStorage::Shared {
                first_vertex,
                vertex_count,
                first_index,
                index_count,
            }
        } else {
            let (vertex_buffer, vertex_alloc) = self.create_device_buffer(
                vsize,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                bytemuck::cast_slice(&mesh.vertices),
            );
            let (index_buffer, index_alloc) = self.create_device_buffer(
                isize,
                vk::BufferUsageFlags::INDEX_BUFFER,
                bytemuck::cast_slice(&mesh.indices),
            );
            super::ChunkStorage::Dedicated {
                vertex_buffer,
                index_buffer,
                vertex_alloc,
                index_alloc,
            }
        };

        DrawCmd {
            storage,
            index_count: mesh.indices.len() as u32,
            transparent_start: mesh.transparent_start,
            cx: mesh.cx,
            cz: mesh.cz,
            aabb_min: mesh.aabb_min,
            aabb_max: mesh.aabb_max,
        }
    }
}
