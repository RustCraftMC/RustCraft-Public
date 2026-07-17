//! Swapchain resize handling and uploaded world mesh lifetime.

use ash::vk;

use super::DrawCmd;
use crate::world::chunk::CHUNK_SIZE;
use crate::world::mesh::ChunkMesh;

impl super::Renderer {
    pub(super) fn prepare_chunk_uploads(&mut self, frame: usize) {
        if self.chunk_upload_bytes.is_empty() {
            return;
        }
        super::resources::upload_dynamic_buffer(
            &self.device,
            &mut self.allocator,
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

    pub fn recreate_swapchain(&mut self, window_size: (u32, u32)) {
        if window_size.0 == 0 || window_size.1 == 0 {
            return;
        }
        unsafe {
            self.device.device_wait_idle().ok();
        }
        self.window_size = window_size;

        unsafe {
            for fb in &self.framebuffers {
                self.device.destroy_framebuffer(*fb, None);
            }
            for view in self.depth_image_views.drain(..) {
                self.device.destroy_image_view(view, None);
            }
            for image in self.depth_images.drain(..) {
                self.device.destroy_image(image, None);
            }
            for allocation in self.depth_allocs.drain(..) {
                self.allocator.free(allocation).ok();
            }
            for v in &self.swapchain_image_views {
                self.device.destroy_image_view(*v, None);
            }
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
        }

        let (sc, images, views, format, extent) = Self::create_swapchain(
            &self.surface_fn,
            &self.swapchain_fn,
            &self.device,
            self._physical_device,
            self.surface,
            window_size,
            vk::SwapchainKHR::null(),
        );
        self.swapchain = sc;
        self._swapchain_images = images;
        self.swapchain_image_views = views;
        self._swapchain_format = format;
        self.swapchain_extent = extent;

        let (depth_images, depth_image_views, depth_allocs) = Self::create_depth_buffers(
            &self.device,
            &mut self.allocator,
            self.depth_format,
            extent,
            self.swapchain_image_views.len(),
        );
        self.depth_images = depth_images;
        self.depth_image_views = depth_image_views;
        self.depth_allocs = depth_allocs;

        self.framebuffers = Self::create_framebuffers(
            &self.device,
            self.render_pass,
            &self.swapchain_image_views,
            &self.depth_image_views,
            extent,
        );
        self.needs_recreate = false;
        log::info!(
            "swapchain recreated: extent={}x{}",
            extent.width,
            extent.height
        );
    }

    pub fn notify_resize(&mut self, width: u32, height: u32) {
        self.window_size = (width, height);
        self.needs_recreate = true;
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
                self.allocator.free(vertex_alloc).ok();
                self.allocator.free(index_alloc).ok();
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
        self.state.frame_chunk_upload_us = self
            .state
            .frame_chunk_upload_us
            .saturating_add(started.elapsed().as_micros() as u64);
        self.state.frame_chunk_upload_bytes = self
            .state
            .frame_chunk_upload_bytes
            .saturating_add(uploaded_bytes);
        self.state.frame_chunk_upload_count = self
            .state
            .frame_chunk_upload_count
            .saturating_add(meshes.len() as u32);
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
            let vertex_src_offset = (self.chunk_upload_bytes.len() + 3) & !3;
            self.chunk_upload_bytes.resize(vertex_src_offset, 0);
            let vertex_end = vertex_src_offset + vsize as usize;
            self.chunk_upload_bytes.resize(vertex_end, 0);
            unsafe {
                let vertex_dst = self.chunk_upload_bytes.as_mut_ptr().add(vertex_src_offset)
                    as *mut crate::world::mesh::Vertex;
                for (index, vertex) in mesh.vertices.iter().enumerate() {
                    let mut world_vertex = *vertex;
                    world_vertex.pos[0] += ox;
                    world_vertex.pos[2] += oz;
                    std::ptr::write_unaligned(vertex_dst.add(index), world_vertex);
                }
            }
            self.chunk_vertex_upload_copies.push(vk::BufferCopy {
                src_offset: vertex_src_offset as u64,
                dst_offset: first_vertex as u64
                    * std::mem::size_of::<crate::world::mesh::Vertex>() as u64,
                size: vsize,
            });

            let index_src_offset = (self.chunk_upload_bytes.len() + 3) & !3;
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
