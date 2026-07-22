//! Frame drawing.

use ash::vk;

use super::DrawCmd;
use super::SelectionBox;
use super::Uniforms;
use crate::client::player::Camera;
use crate::render::entity::mesh::EntityPose;
use crate::render::sky::SkyGradient;
use crate::world::chunk::CHUNK_SIZE;

fn append_world_mesh(
    vertices: &mut Vec<crate::world::mesh::Vertex>,
    indices: &mut Vec<u32>,
    mut source_vertices: Vec<crate::world::mesh::Vertex>,
    source_indices: Vec<u32>,
) {
    let base = vertices.len() as u32;
    vertices.append(&mut source_vertices);
    indices.extend(source_indices.into_iter().map(|index| base + index));
}

/// Encode a sign's block position into a stable u64 key id.
///
/// Uses FNV hashing so the same `[i32; 3]` position always maps to the same
/// id, letting sign atlas lookups avoid per-frame `format!` allocations on the
/// three integer coordinates.
fn sign_position_key_id(position: [i32; 3]) -> u64 {
    use std::hash::Hasher;
    let mut h = fnv::FnvHasher::default();
    h.write_i32(position[0]);
    h.write_i32(position[1]);
    h.write_i32(position[2]);
    h.finish()
}

fn upload_cached_gpu_mesh(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    mesh: &mut super::DynamicGpuMesh,
    vertex_bytes: &[u8],
    index_bytes: &[u8],
    index_count: usize,
) {
    if index_count == 0 {
        mesh.index_count = 0;
        return;
    }
    super::resources::upload_dynamic_buffer(
        device,
        allocator,
        &mut mesh.vertex_buffer,
        &mut mesh.vertex_alloc,
        &mut mesh.vertex_capacity,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        vertex_bytes,
    );
    super::resources::upload_dynamic_buffer(
        device,
        allocator,
        &mut mesh.index_buffer,
        &mut mesh.index_alloc,
        &mut mesh.index_capacity,
        vk::BufferUsageFlags::INDEX_BUFFER,
        index_bytes,
    );
    mesh.index_count = index_count as u32;
}

fn append_entity_shadow(
    vertices: &mut Vec<crate::render::entity::mesh::EntityVertex>,
    indices: &mut Vec<u32>,
    billboard: &super::EntityBillboard,
) {
    use crate::render::entity::mesh::EntityVertex;

    if !billboard.entity_type.is_mob()
        && billboard.entity_type != crate::entity::EntityType::Item
        && billboard.entity_type != crate::entity::EntityType::XPOrb
    {
        return;
    }
    let (width, _) = billboard.entity_type.bounding_box();
    let size = width * 0.75;
    let position = billboard.position;
    let base = vertices.len() as u32;
    let normal = [0.0, 1.0, 0.0];
    let color = [0.0, 0.0, 0.0, 0.35];
    for point in [
        [position[0] - size, 0.01, position[2] - size],
        [position[0] + size, 0.01, position[2] - size],
        [position[0] + size, 0.01, position[2] + size],
        [position[0] - size, 0.01, position[2] + size],
    ] {
        vertices.push(EntityVertex {
            position: point,
            normal,
            uv: [0.0, 0.0],
            color,
        });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn append_player_skull_mesh(
    vertices: &mut Vec<crate::render::entity::mesh::EntityVertex>,
    indices: &mut Vec<u32>,
    skull: &super::SkullRenderEntry,
    region: &super::entity::atlas::MobTextureRegion,
) {
    let [bx, by, bz] = skull.position;
    let (min, max) = skull_bounds(skull);
    let center = [
        bx as f32 + (min[0] + max[0]) * 0.5,
        by as f32 + (min[1] + max[1]) * 0.5,
        bz as f32 + (min[2] + max[2]) * 0.5,
    ];
    let yaw = skull_yaw(skull);
    let (sin_yaw, cos_yaw) = yaw.sin_cos();

    let mut transform = |p: [f32; 3]| -> [f32; 3] {
        let local = [
            bx as f32 + p[0] - center[0],
            by as f32 + p[1] - center[1],
            bz as f32 + p[2] - center[2],
        ];
        [
            center[0] + local[0] * cos_yaw - local[2] * sin_yaw,
            center[1] + local[1],
            center[2] + local[0] * sin_yaw + local[2] * cos_yaw,
        ]
    };
    let rotate_normal = |n: [f32; 3]| -> [f32; 3] {
        [
            n[0] * cos_yaw - n[2] * sin_yaw,
            n[1],
            n[0] * sin_yaw + n[2] * cos_yaw,
        ]
    };

    let faces = [
        (
            [
                [min[0], min[1], min[2]],
                [max[0], min[1], min[2]],
                [max[0], max[1], min[2]],
                [min[0], max[1], min[2]],
            ],
            [0.0, 0.0, -1.0],
            [24.0, 8.0, 32.0, 16.0],
        ),
        (
            [
                [max[0], min[1], max[2]],
                [min[0], min[1], max[2]],
                [min[0], max[1], max[2]],
                [max[0], max[1], max[2]],
            ],
            [0.0, 0.0, 1.0],
            [8.0, 8.0, 16.0, 16.0],
        ),
        (
            [
                [min[0], max[1], max[2]],
                [min[0], max[1], min[2]],
                [max[0], max[1], min[2]],
                [max[0], max[1], max[2]],
            ],
            [0.0, 1.0, 0.0],
            [8.0, 0.0, 16.0, 8.0],
        ),
        (
            [
                [min[0], min[1], min[2]],
                [min[0], min[1], max[2]],
                [max[0], min[1], max[2]],
                [max[0], min[1], min[2]],
            ],
            [0.0, -1.0, 0.0],
            [16.0, 0.0, 24.0, 8.0],
        ),
        (
            [
                [min[0], min[1], max[2]],
                [min[0], min[1], min[2]],
                [min[0], max[1], min[2]],
                [min[0], max[1], max[2]],
            ],
            [-1.0, 0.0, 0.0],
            [16.0, 8.0, 24.0, 16.0],
        ),
        (
            [
                [max[0], min[1], min[2]],
                [max[0], min[1], max[2]],
                [max[0], max[1], max[2]],
                [max[0], max[1], min[2]],
            ],
            [1.0, 0.0, 0.0],
            [0.0, 8.0, 8.0, 16.0],
        ),
    ];
    let tex_w = region.tex_width.max(1) as f32;
    let tex_h = region.tex_height.max(1) as f32;

    for (corners, normal, uv_px) in faces {
        let base = vertices.len() as u32;
        let u0 = uv_px[0] / tex_w;
        let v0 = uv_px[1] / tex_h;
        let u1 = uv_px[2] / tex_w;
        let v1 = uv_px[3] / tex_h;
        let uvs = [(u0, v1), (u1, v1), (u1, v0), (u0, v0)];
        let normal = rotate_normal(normal);
        for (corner, (u, v)) in corners.into_iter().zip(uvs) {
            let (atlas_u, atlas_v) = region.local_to_atlas(u, v);
            vertices.push(crate::render::entity::mesh::EntityVertex {
                position: transform(corner),
                normal,
                uv: [atlas_u, atlas_v],
                color: [1.0, 1.0, 1.0, 1.0],
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn skull_bounds(skull: &super::SkullRenderEntry) -> ([f32; 3], [f32; 3]) {
    match skull.block_metadata & 0x07 {
        2 => ([0.25, 0.25, 0.50], [0.75, 0.75, 1.00]), // north wall, faces south
        3 => ([0.25, 0.25, 0.00], [0.75, 0.75, 0.50]), // south wall, faces north
        4 => ([0.50, 0.25, 0.25], [1.00, 0.75, 0.75]), // west wall, faces east
        5 => ([0.00, 0.25, 0.25], [0.50, 0.75, 0.75]), // east wall, faces west
        _ => ([0.25, 0.00, 0.25], [0.75, 0.50, 0.75]), // standing skull
    }
}

fn skull_yaw(skull: &super::SkullRenderEntry) -> f32 {
    match skull.block_metadata & 0x07 {
        2 => 0.0,
        3 => std::f32::consts::PI,
        4 => -std::f32::consts::FRAC_PI_2,
        5 => std::f32::consts::FRAC_PI_2,
        _ => (skull.rotation as f32) * (std::f32::consts::TAU / 16.0),
    }
}

/// Self-contained job for generating one mob-entity mesh on a background thread.
struct MobMeshJob {
    entity_id: i32,
    cuboids: std::sync::Arc<Vec<super::entity::mesh::ModelCuboid>>,
    world_position: nalgebra::Point3<f32>,
    pose: super::entity::mesh::EntityPose,
    atlas_uv: Option<[f32; 4]>,
    cape_atlas_uv: Option<[f32; 4]>,
    armor_layers: Vec<ArmorMeshLayer>,
    hurt_alpha: f32,
    sky_light: u8,
    block_light: u8,
    entity_type: crate::entity::EntityType,
    position: [f32; 3],
    held_item_and_damage: Option<(u16, u16)>,
    state_hash: u64,
}

struct ArmorMeshLayer {
    cuboids: Vec<super::entity::mesh::ModelCuboid>,
    atlas_uv: [f32; 4],
    color: [f32; 4],
}

fn generate_mob_mesh_in_parallel(job: &MobMeshJob) -> MobMeshResult {
    // Separate cape cuboids so they can be mapped to the cape atlas region.
    let mut body_cuboids: Vec<_> = job
        .cuboids
        .iter()
        .filter(|c| c.part_type != super::entity::mesh::PartType::Cape)
        .cloned()
        .collect();
    let cape_cuboids: Vec<_> = job
        .cuboids
        .iter()
        .filter(|c| c.part_type == super::entity::mesh::PartType::Cape)
        .cloned()
        .collect();

    let (mut body_vertices, mut body_indices) =
        super::entity::mesh::generate_entity_mesh(&body_cuboids, job.world_position, &job.pose);

    let (mut cape_vertices, cape_indices) = if !cape_cuboids.is_empty() {
        super::entity::mesh::generate_entity_mesh(&cape_cuboids, job.world_position, &job.pose)
    } else {
        (Vec::new(), Vec::new())
    };

    if let Some([u_min, v_min, u_max, v_max]) = job.atlas_uv {
        let ur = u_max - u_min;
        let vr = v_max - v_min;
        for v in &mut body_vertices {
            v.uv = [u_min + v.uv[0] * ur, v_min + v.uv[1] * vr];
        }
    }
    if let Some([u_min, v_min, u_max, v_max]) = job.cape_atlas_uv {
        let ur = u_max - u_min;
        let vr = v_max - v_min;
        for v in &mut cape_vertices {
            v.uv = [u_min + v.uv[0] * ur, v_min + v.uv[1] * vr];
        }
    }

    for layer in &job.armor_layers {
        let (mut vertices, indices) = super::entity::mesh::generate_entity_mesh(
            &layer.cuboids,
            job.world_position,
            &job.pose,
        );
        let [u_min, v_min, u_max, v_max] = layer.atlas_uv;
        let ur = u_max - u_min;
        let vr = v_max - v_min;
        for vertex in &mut vertices {
            vertex.uv = [u_min + vertex.uv[0] * ur, v_min + vertex.uv[1] * vr];
            for (channel, tint) in vertex.color.iter_mut().zip(layer.color) {
                *channel *= tint;
            }
        }
        let base_vertex = body_vertices.len() as u32;
        body_indices.extend(indices.into_iter().map(|index| index + base_vertex));
        body_vertices.extend(vertices);
    }

    // Offset cape indices to follow body vertices
    let body_len = body_vertices.len() as u32;
    let cape_indices: Vec<u32> = cape_indices.iter().map(|i| *i + body_len).collect();

    body_vertices.extend(cape_vertices);
    let all_indices = [body_indices, cape_indices].concat();

    if job.hurt_alpha > 0.0 {
        let ha = job.hurt_alpha;
        for v in &mut body_vertices {
            v.color[0] = v.color[0] * (1.0 - ha) + ha;
            v.color[1] *= 1.0 - ha;
            v.color[2] *= 1.0 - ha;
        }
    }

    let packed_light = 16.0 + job.sky_light.min(15) as f32 * 16.0 + job.block_light.min(15) as f32;
    for v in &mut body_vertices {
        v.color[3] = packed_light;
    }

    let (mut hb_v, mut hb_i) = (Vec::new(), Vec::new());
    let (mut hi_v, mut hi_i) = (Vec::new(), Vec::new());

    let supports_held = matches!(
        job.entity_type,
        crate::entity::EntityType::Player
            | crate::entity::EntityType::Zombie
            | crate::entity::EntityType::PigZombie
            | crate::entity::EntityType::Skeleton
            | crate::entity::EntityType::Witch
            | crate::entity::EntityType::Giant
            | crate::entity::EntityType::ArmorStand
    );
    if supports_held {
        if let Some((id, dmg)) = job.held_item_and_damage.filter(|&(id, _)| id != 0) {
            let (op_v, op_i, tr_v, tr_i, use_item) =
                crate::render::hud::hand::generate_entity_held_item_mesh(
                    job.entity_type,
                    id,
                    dmg,
                    job.position,
                    &job.pose,
                    job.cuboids.as_ref(),
                );
            if use_item {
                append_world_mesh(&mut hi_v, &mut hi_i, op_v, op_i);
                append_world_mesh(&mut hi_v, &mut hi_i, tr_v, tr_i);
            } else {
                append_world_mesh(&mut hb_v, &mut hb_i, op_v, op_i);
                append_world_mesh(&mut hb_v, &mut hb_i, tr_v, tr_i);
            }
        }
    }

    MobMeshResult {
        entity_id: job.entity_id,
        state_hash: job.state_hash,
        entity_type: job.entity_type,
        position: job.position,
        body_vertices,
        body_indices: all_indices,
        held_block_vertices: hb_v,
        held_block_indices: hb_i,
        held_item_vertices: hi_v,
        held_item_indices: hi_i,
    }
}

struct MobMeshResult {
    entity_id: i32,
    state_hash: u64,
    entity_type: crate::entity::EntityType,
    position: [f32; 3],
    body_vertices: Vec<super::entity::mesh::EntityVertex>,
    body_indices: Vec<u32>,
    held_block_vertices: Vec<crate::world::mesh::Vertex>,
    held_block_indices: Vec<u32>,
    held_item_vertices: Vec<crate::world::mesh::Vertex>,
    held_item_indices: Vec<u32>,
}

impl super::Renderer {
    fn prepare_chunk_indirect_commands(&mut self, frame: usize, camera: &Camera) {
        self.transparent_draw_indices.clear();
        self.transparent_draw_indices
            .extend(self.visible_chunk_indices.iter().copied().filter(|&index| {
                let command = &self.draw_cmds[index];
                command.index_count > command.transparent_start
            }));
        let cam_x = camera.position.x;
        let cam_z = camera.position.z;
        self.transparent_draw_indices.sort_unstable_by(|&a, &b| {
            let a = &self.draw_cmds[a];
            let b = &self.draw_cmds[b];
            let dist_a = {
                let dx = cam_x - (a.cx * CHUNK_SIZE as i32) as f32 - 8.0;
                let dz = cam_z - (a.cz * CHUNK_SIZE as i32) as f32 - 8.0;
                dx * dx + dz * dz
            };
            let dist_b = {
                let dx = cam_x - (b.cx * CHUNK_SIZE as i32) as f32 - 8.0;
                let dz = cam_z - (b.cz * CHUNK_SIZE as i32) as f32 - 8.0;
                dx * dx + dz * dz
            };
            dist_b.total_cmp(&dist_a)
        });

        self.chunk_indirect_commands.clear();
        for &index in &self.visible_chunk_indices {
            let command = &self.draw_cmds[index];
            if command.transparent_start == 0 {
                continue;
            }
            if let super::ChunkStorage::Shared {
                first_vertex,
                first_index,
                ..
            } = command.storage
            {
                self.chunk_indirect_commands
                    .push(super::ChunkIndirectCommand {
                        index_count: command.transparent_start,
                        instance_count: 1,
                        first_index,
                        vertex_offset: first_vertex as i32,
                        first_instance: 0,
                    });
            }
        }
        self.chunk_opaque_indirect_count = self.chunk_indirect_commands.len() as u32;
        self.chunk_transparent_indirect_offset = (self.chunk_indirect_commands.len()
            * std::mem::size_of::<super::ChunkIndirectCommand>())
            as u64;

        for &index in &self.transparent_draw_indices {
            let command = &self.draw_cmds[index];
            if let super::ChunkStorage::Shared {
                first_vertex,
                first_index,
                ..
            } = command.storage
            {
                self.chunk_indirect_commands
                    .push(super::ChunkIndirectCommand {
                        index_count: command.index_count - command.transparent_start,
                        instance_count: 1,
                        first_index: first_index + command.transparent_start,
                        vertex_offset: first_vertex as i32,
                        first_instance: 0,
                    });
            }
        }
        self.chunk_transparent_indirect_count =
            self.chunk_indirect_commands.len() as u32 - self.chunk_opaque_indirect_count;

        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.chunk_indirect_buffers[frame],
            &mut self.chunk_indirect_allocs[frame],
            &mut self.chunk_indirect_capacities[frame],
            vk::BufferUsageFlags::INDIRECT_BUFFER,
            bytemuck::cast_slice(&self.chunk_indirect_commands),
        );
    }

    unsafe fn record_shared_chunk_draws(
        &self,
        cb: vk::CommandBuffer,
        frame: usize,
        offset: u64,
        draw_count: u32,
    ) {
        if draw_count == 0 {
            return;
        }
        let Some(indirect_buffer) = self.chunk_indirect_buffers[frame] else {
            return;
        };
        self.device
            .cmd_bind_vertex_buffers(cb, 0, &[self.chunk_vertex_buffer], &[0]);
        self.device
            .cmd_bind_index_buffer(cb, self.chunk_index_buffer, 0, vk::IndexType::UINT32);
        self.device.cmd_push_constants(
            cb,
            self.pipeline_layout,
            vk::ShaderStageFlags::VERTEX,
            0,
            bytemuck::bytes_of(&[0.0f32; 3]),
        );
        let stride = std::mem::size_of::<super::ChunkIndirectCommand>() as u32;
        if self.multi_draw_indirect {
            self.device
                .cmd_draw_indexed_indirect(cb, indirect_buffer, offset, draw_count, stride);
        } else {
            for index in 0..draw_count {
                self.device.cmd_draw_indexed_indirect(
                    cb,
                    indirect_buffer,
                    offset + index as u64 * stride as u64,
                    1,
                    stride,
                );
            }
        }
    }

    pub fn draw_frame(&mut self, camera: &Camera, menu: u32, underwater: bool, in_world: bool) {
        let t0 = std::time::Instant::now();
        self.current_camera = Some(camera.clone());
        if self.swapchain.needs_recreate {
            self.recreate_swapchain();
            if self.swapchain.needs_recreate {
                return;
            }
        }

        let frame = self.swapchain.current_frame;
        let fence = self.swapchain.in_flight_fences[frame];

        let t_fence_start = std::time::Instant::now();
        let fence_us = unsafe {
            match self.device.wait_for_fences(&[fence], true, 1_000_000_000) {
                Ok(()) => t_fence_start.elapsed().as_micros() as u64,
                Err(e) => {
                    log::error!("wait_for_fences failed (device likely lost): {e:?}");
                    self.swapchain.needs_recreate = true;
                    return;
                }
            }
        };
        for command in std::mem::take(&mut self.retired_draw_cmds[frame]) {
            self.destroy_draw_cmd(command);
        }

        let acquire_started = std::time::Instant::now();
        let image_index = match self.swapchain.acquire_next_image(&self.device) {
            Ok((idx, _)) => idx,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain();
                return;
            }
            Err(e) => {
                log::error!("failed to acquire swapchain image: {e:?}");
                return;
            }
        };
        self.state.frame_profile.set_frame_acquire_us(acquire_started.elapsed().as_micros() as u64);

        unsafe {
            if let Err(e) = self.device.reset_fences(&[fence]) {
                log::error!("reset_fences failed: {e:?}");
                return;
            }
        }
        self.prepare_chunk_uploads(frame);
        self.prepare_local_skin_upload(frame);

        let view = camera.view_matrix();
        let proj = camera.projection_matrix_at(camera.partial_tick);
        let view_proj = proj * view;
        let mut sky = SkyGradient::environment(
            self.state.hud.day_time(),
            self.state.settings.render_distance(),
            self.state.hud.dimension(),
        );
        let rain = if self.state.hud.raining() {
            self.state.hud.rain_level()
        } else {
            0.0
        };
        let thunder = if self.state.hud.raining() {
            self.state.hud.thunder_level()
        } else {
            0.0
        };
        let sun_brightness = SkyGradient::sun_brightness(self.state.hud.day_time() as f32, rain, thunder);
        let storm = if self.state.hud.raining() {
            (self.state.hud.rain_level() + self.state.hud.thunder_level() * 0.6).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if storm > 0.0 {
            for c in &mut sky.clear_color[0..3] {
                *c *= 1.0 - storm * 0.28;
            }
            for c in &mut sky.fog_color[0..3] {
                *c *= 1.0 - storm * 0.22;
            }
        }
        // Underwater fog: match vanilla MC 1.8.9 fog settings
        // Vanilla uses GL_EXP2 with density 0.1, giving ~20 blocks visibility.
        // We approximate with linear fog: start=2, end=20, dark blue tint.
        if underwater {
            sky.clear_color = [0.02, 0.02, 0.2, 1.0];
            sky.fog_color = [0.02, 0.02, 0.2, 1.0];
            sky.fog_params = [2.0, 20.0, sky.fog_params[2], sky.fog_params[3]];
        }
        // Scroll clock for the enchanted-item glint (RenderItem.renderEffect
        // uses 3000ms and 4873ms timers). Wrap on their common multiple so
        // both `fract(t / 3.0)` and `fract(t / 4.873)` stay continuous, and
        // keep the value small enough for full f32 millisecond precision.
        let glint_seconds = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            % 14_619_000) as f32
            / 1000.0;
        let uniforms = Uniforms {
            view: view.into(),
            proj: proj.into(),
            view_proj: view_proj.into(),
            light_dir: [
                sky.light_dir[0],
                sky.light_dir[1],
                sky.light_dir[2],
                sun_brightness,
            ],
            fog_color: [
                sky.fog_color[0],
                sky.fog_color[1],
                sky.fog_color[2],
                glint_seconds,
            ],
            fog_params: [
                sky.fog_params[0],
                sky.fog_params[1],
                camera.position.y,
                sun_brightness,
            ],
            grass_color: [
                crate::world::material::GRASS_COLOR[0],
                crate::world::material::GRASS_COLOR[1],
                crate::world::material::GRASS_COLOR[2],
                self.state.hud.dimension() as f32,
            ],
        };
        let ualloc = self.resources.uniform_alloc(frame);
        unsafe {
            let ptr = match self.device.map_memory(
                ualloc.memory(),
                ualloc.offset(),
                std::mem::size_of::<Uniforms>() as u64,
                vk::MemoryMapFlags::empty(),
            ) {
                Ok(ptr) => ptr,
                Err(e) => {
                    log::error!("map_memory for uniforms failed: {e:?}");
                    return;
                }
            };
            std::ptr::copy_nonoverlapping(
                &uniforms as *const _ as *const u8,
                ptr as *mut u8,
                std::mem::size_of::<Uniforms>(),
            );
            self.device.unmap_memory(ualloc.memory());
        }

        let frustum = camera.frustum();

        // Animate block textures at 20 Hz and stage only changed tiles. The
        // transfer is part of this frame's command buffer and never waits for a
        // dedicated render-thread fence.
        self.prepare_block_animation_uploads(frame);
        self.visible_chunk_indices.clear();
        self.visible_chunk_indices
            .extend(
                self.draw_cmds
                    .iter()
                    .enumerate()
                    .filter_map(|(index, command)| {
                        frustum
                            .test_aabb(command.aabb_min, command.aabb_max)
                            .then_some(index)
                    }),
            );
        self.prepare_chunk_indirect_commands(frame, camera);
        let cb = self.command_buffers[frame];

        let t_mesh = std::time::Instant::now();
        self.state.frame_profile.set_entity_cache_hits(0);
        self.state.frame_profile.set_entity_cache_misses(0);
        self.state.frame_profile.set_entity_visible_count(0);
        self.state.frame_profile.set_entity_hash_us(0);
        self.state.frame_profile.set_entity_lookup_us(0);
        self.state.frame_profile.set_entity_append_us(0);
        self.upload_entity_meshes_cached(&frustum, camera);
        self.state.frame_profile.set_frame_entity_us(t_mesh.elapsed().as_micros() as u64);
        let t_p = std::time::Instant::now();
        self.upload_particle_mesh(camera);
        self.state.frame_profile.set_frame_particle_us(t_p.elapsed().as_micros() as u64);
        let t_n = std::time::Instant::now();
        self.upload_nametag_mesh(camera);
        self.state.frame_profile.set_frame_nametag_us(t_n.elapsed().as_micros() as u64);
        self.prepare_entity_atlas_upload(frame);
        self.upload_block_selection();
        let t_l = std::time::Instant::now();
        self.upload_local_player_meshes(camera);
        self.state.frame_profile.set_frame_local_us(t_l.elapsed().as_micros() as u64);
        self.state.frame_profile.set_frame_mesh_us(t_mesh.elapsed().as_micros() as u64);
        let t_gui = std::time::Instant::now();
        let mut gui_builders = if self.gui_pipeline != vk::Pipeline::null() {
            use super::gui::GuiVertexBuilder;

            const GUI_BUILD_INTERVAL: std::time::Duration =
                std::time::Duration::from_micros(16_667); // 60 Hz
            let rebuild_gui = self.last_gui_build.elapsed() >= GUI_BUILD_INTERVAL;

            let sw = self.swapchain.swapchain_extent.width as f32;
            let sh = self.swapchain.swapchain_extent.height as f32;
            let metrics = super::gui::widgets::MenuMetrics::new(
                sw,
                sh,
                self.state.settings.gui_scale(),
                self.gui_mouse_pos,
            );

            let (
                mut overlay_gui,
                mut background_gui,
                mut widget_gui,
                mut inventory_gui,
                mut generic54_gui,
                mut font_gui,
                mut block_gui,
                mut item_gui,
                mut icons_gui,
                mut creative_gui,
            ) = self.gui_builder_cache.take().unwrap_or_else(|| {
                (
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                    GuiVertexBuilder::new(),
                )
            });
            if rebuild_gui {
                self.last_gui_build = std::time::Instant::now();
                for builder in [
                    &mut overlay_gui,
                    &mut background_gui,
                    &mut widget_gui,
                    &mut inventory_gui,
                    &mut generic54_gui,
                    &mut font_gui,
                    &mut block_gui,
                    &mut item_gui,
                    &mut icons_gui,
                    &mut creative_gui,
                ] {
                    builder.clear();
                }
                self.draw_menu_screen(
                    menu,
                    in_world,
                    &metrics,
                    &mut overlay_gui,
                    &mut background_gui,
                    &mut widget_gui,
                    &mut inventory_gui,
                    &mut generic54_gui,
                    &mut font_gui,
                    &mut block_gui,
                    &mut item_gui,
                    &mut icons_gui,
                    &mut creative_gui,
                );
                self.last_button_hits.clone_from(&widget_gui.button_hits);
                self.upload_font_atlas();
            }
            self.update_gui_uniforms();
            Some((
                overlay_gui,
                background_gui,
                widget_gui,
                inventory_gui,
                generic54_gui,
                font_gui,
                block_gui,
                item_gui,
                icons_gui,
                creative_gui,
            ))
        } else {
            None
        };
        self.state.frame_profile.set_frame_gui_us(t_gui.elapsed().as_micros() as u64);

        let command_started = std::time::Instant::now();
        unsafe {
            if let Err(e) = self.device.begin_command_buffer(
                cb,
                &vk::CommandBufferBeginInfo {
                    flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                    ..Default::default()
                },
            ) {
                log::error!("begin_command_buffer failed: {e:?}");
                return;
            }
            self.record_chunk_uploads(cb, frame);
            self.record_block_animation_uploads(cb, frame);
            self.record_entity_skin_upload(cb, frame);
            self.record_local_skin_upload(cb, frame);

            let clears = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: sky.clear_color,
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];
            self.device.cmd_begin_render_pass(
                cb,
                &vk::RenderPassBeginInfo {
                    render_pass: self.render_pass,
                    framebuffer: self.swapchain.framebuffers[image_index as usize],
                    render_area: vk::Rect2D {
                        extent: self.swapchain.swapchain_extent,
                        ..Default::default()
                    },
                    clear_value_count: 2,
                    p_clear_values: clears.as_ptr(),
                    ..Default::default()
                },
                vk::SubpassContents::INLINE,
            );

            if menu == 0 {
                self.draw_panorama(cb, frame);
            } else {
                // ---- Sky rendering ----
                let day_f = self.state.hud.day_time() as f32;
                let moon_phase =
                    crate::render::sky::SkyGradient::moon_phase(self.state.hud.day_time()) as f32;
                let celestial_visibility = if self.state.hud.dimension() == 0 {
                    1.0 - storm
                } else {
                    0.0
                };
                self.update_sky_uniforms(
                    camera,
                    crate::render::sky::SkyGradient::zenith_color(day_f),
                    crate::render::sky::SkyGradient::horizon_color(day_f),
                    crate::render::sky::SkyGradient::sun_direction(day_f),
                    celestial_visibility,
                    moon_phase,
                    self.swapchain.swapchain_extent.width as f32,
                    self.swapchain.swapchain_extent.height as f32,
                    sky.fog_params[2],
                    SkyGradient::daylight_factor(day_f),
                );
                self.draw_sky(cb, frame);

                self.device
                    .cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

                self.device.cmd_set_viewport(
                    cb,
                    0,
                    &[vk::Viewport {
                        width: self.swapchain.swapchain_extent.width as f32,
                        height: self.swapchain.swapchain_extent.height as f32,
                        max_depth: 1.0,
                        ..Default::default()
                    }],
                );
                self.device.cmd_set_scissor(
                    cb,
                    0,
                    &[vk::Rect2D {
                        extent: self.swapchain.swapchain_extent,
                        ..Default::default()
                    }],
                );

                // Draw world chunks — opaque pass
                self.device
                    .cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
                self.device.cmd_bind_descriptor_sets(
                    cb,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout,
                    0,
                    &[self.descriptor_sets[frame]],
                    &[],
                );

                self.record_shared_chunk_draws(cb, frame, 0, self.chunk_opaque_indirect_count);
                for &index in &self.visible_chunk_indices {
                    let cmd = &self.draw_cmds[index];
                    let opaque_count = cmd.transparent_start;
                    if opaque_count == 0 {
                        continue;
                    }
                    let super::ChunkStorage::Dedicated {
                        vertex_buffer,
                        index_buffer,
                        ..
                    } = &cmd.storage
                    else {
                        continue;
                    };
                    self.device
                        .cmd_bind_vertex_buffers(cb, 0, &[*vertex_buffer], &[0]);
                    self.device
                        .cmd_bind_index_buffer(cb, *index_buffer, 0, vk::IndexType::UINT32);
                    let offset: [f32; 3] = [
                        (cmd.cx * CHUNK_SIZE as i32) as f32,
                        0.0,
                        (cmd.cz * CHUNK_SIZE as i32) as f32,
                    ];
                    self.device.cmd_push_constants(
                        cb,
                        self.pipeline_layout,
                        vk::ShaderStageFlags::VERTEX,
                        0,
                        bytemuck::bytes_of(&offset),
                    );
                    self.device.cmd_draw_indexed(cb, opaque_count, 1, 0, 0, 0);
                }

                // Draw entities
                if self.entity_index_count > 0 {
                    if let (Some(vb), Some(ib)) =
                        (self.entity_vertex_buffer, self.entity_index_buffer)
                    {
                        self.device.cmd_bind_pipeline(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.entity_pipeline,
                        );
                        self.device.cmd_bind_descriptor_sets(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.entity_pipeline_layout,
                            0,
                            &[self.entity_descriptor_sets[frame]],
                            &[],
                        );
                        self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                        self.device
                            .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                        self.device
                            .cmd_draw_indexed(cb, self.entity_index_count, 1, 0, 0, 0);
                    }
                }

                if !self.visible_entity_ids.is_empty() {
                    self.device.cmd_bind_pipeline(
                        cb,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.entity_pipeline,
                    );
                    self.device.cmd_bind_descriptor_sets(
                        cb,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.entity_pipeline_layout,
                        0,
                        &[self.entity_descriptor_sets[frame]],
                        &[],
                    );
                    for entity_id in &self.visible_entity_ids {
                        let Some(mesh) = self.entity_gpu_meshes.get(entity_id) else {
                            continue;
                        };
                        if mesh.body[frame].index_count == 0 {
                            continue;
                        }
                        let body = &mesh.body[frame];
                        if let (Some(vertex_buffer), Some(index_buffer)) =
                            (body.vertex_buffer, body.index_buffer)
                        {
                            self.device
                                .cmd_bind_vertex_buffers(cb, 0, &[vertex_buffer], &[0]);
                            self.device.cmd_bind_index_buffer(
                                cb,
                                index_buffer,
                                0,
                                vk::IndexType::UINT32,
                            );
                            self.device
                                .cmd_draw_indexed(cb, body.index_count, 1, 0, 0, 0);
                        }
                    }

                    for (uses_item_atlas, select_mesh) in [(false, 0u8), (true, 1u8)] {
                        self.device.cmd_bind_pipeline(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline,
                        );
                        let descriptor = if uses_item_atlas {
                            self.fp_item_descriptor_sets[frame]
                        } else {
                            self.descriptor_sets[frame]
                        };
                        self.device.cmd_bind_descriptor_sets(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline_layout,
                            0,
                            &[descriptor],
                            &[],
                        );
                        let offset = [0.0f32; 3];
                        self.device.cmd_push_constants(
                            cb,
                            self.pipeline_layout,
                            vk::ShaderStageFlags::VERTEX,
                            0,
                            bytemuck::bytes_of(&offset),
                        );
                        for entity_id in &self.visible_entity_ids {
                            let Some(entity) = self.entity_gpu_meshes.get(entity_id) else {
                                continue;
                            };
                            let mesh = if select_mesh == 0 {
                                &entity.held_block[frame]
                            } else {
                                &entity.held_item[frame]
                            };
                            if mesh.index_count == 0 {
                                continue;
                            }
                            if let (Some(vertex_buffer), Some(index_buffer)) =
                                (mesh.vertex_buffer, mesh.index_buffer)
                            {
                                self.device
                                    .cmd_bind_vertex_buffers(cb, 0, &[vertex_buffer], &[0]);
                                self.device.cmd_bind_index_buffer(
                                    cb,
                                    index_buffer,
                                    0,
                                    vk::IndexType::UINT32,
                                );
                                self.device
                                    .cmd_draw_indexed(cb, mesh.index_count, 1, 0, 0, 0);
                            }
                        }
                    }
                }

                // Draw the local player in third person with the player's own skin.
                if self.state.settings.camera_mode() != 0 && self.fp_arm_index_count > 0 {
                    if let (Some(vb), Some(ib)) =
                        (self.fp_arm_vertex_buffer, self.fp_arm_index_buffer)
                    {
                        self.device.cmd_bind_pipeline(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.entity_pipeline,
                        );
                        self.device.cmd_bind_descriptor_sets(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.entity_pipeline_layout,
                            0,
                            &[self.skin_descriptor_sets[frame]],
                            &[],
                        );
                        self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                        self.device
                            .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                        self.device
                            .cmd_draw_indexed(cb, self.fp_arm_index_count, 1, 0, 0, 0);
                    }
                }

                // Draw the local player's held item in the same world-depth pass.
                if self.state.settings.camera_mode() != 0 && self.fp_block_op_index_count > 0 {
                    if let (Some(vb), Some(ib)) = (
                        self.fp_block_op_vertex_buffer,
                        self.fp_block_op_index_buffer,
                    ) {
                        self.device.cmd_bind_pipeline(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline,
                        );
                        let desc_set = if self.fp_block_uses_item_atlas {
                            self.fp_item_descriptor_sets[frame]
                        } else {
                            self.descriptor_sets[frame]
                        };
                        self.device.cmd_bind_descriptor_sets(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline_layout,
                            0,
                            &[desc_set],
                            &[],
                        );
                        self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                        self.device
                            .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                        let offset = [0.0f32; 3];
                        self.device.cmd_push_constants(
                            cb,
                            self.pipeline_layout,
                            vk::ShaderStageFlags::VERTEX,
                            0,
                            bytemuck::bytes_of(&offset),
                        );
                        self.device
                            .cmd_draw_indexed(cb, self.fp_block_op_index_count, 1, 0, 0, 0);
                    }
                }

                for (uses_item_atlas, vertex_buffer, index_buffer, index_count) in [
                    (
                        false,
                        self.entity_held_block_vertex_buffer,
                        self.entity_held_block_index_buffer,
                        self.entity_held_block_index_count,
                    ),
                    (
                        true,
                        self.entity_held_item_vertex_buffer,
                        self.entity_held_item_index_buffer,
                        self.entity_held_item_index_count,
                    ),
                ] {
                    if index_count == 0 {
                        continue;
                    }
                    if let (Some(vb), Some(ib)) = (vertex_buffer, index_buffer) {
                        self.device.cmd_bind_pipeline(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline,
                        );
                        let descriptor = if uses_item_atlas {
                            self.fp_item_descriptor_sets[frame]
                        } else {
                            self.descriptor_sets[frame]
                        };
                        self.device.cmd_bind_descriptor_sets(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline_layout,
                            0,
                            &[descriptor],
                            &[],
                        );
                        self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                        self.device
                            .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                        let offset = [0.0f32; 3];
                        self.device.cmd_push_constants(
                            cb,
                            self.pipeline_layout,
                            vk::ShaderStageFlags::VERTEX,
                            0,
                            bytemuck::bytes_of(&offset),
                        );
                        self.device.cmd_draw_indexed(cb, index_count, 1, 0, 0, 0);
                    }
                }

                // Draw block selection wireframe + crack overlay (same pipeline as entities)
                if self.block_index_count > 0 {
                    if let (Some(vb), Some(ib)) =
                        (self.block_vertex_buffer, self.block_index_buffer)
                    {
                        if self.entity_pipeline != vk::Pipeline::null() {
                            self.device.cmd_bind_pipeline(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.entity_pipeline,
                            );
                            self.device.cmd_bind_descriptor_sets(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.entity_pipeline_layout,
                                0,
                                &[self.entity_descriptor_sets[frame]],
                                &[],
                            );
                            self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                            self.device
                                .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                            self.device
                                .cmd_draw_indexed(cb, self.block_index_count, 1, 0, 0, 0);
                        }
                    }
                }

                // Draw world chunks — transparent pass (water, glass, leaves)
                if self.transparent_pipeline != vk::Pipeline::null() {
                    if !self.transparent_draw_indices.is_empty() {
                        self.device.cmd_bind_pipeline(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.transparent_pipeline,
                        );
                        self.device.cmd_bind_descriptor_sets(
                            cb,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline_layout,
                            0,
                            &[self.descriptor_sets[frame]],
                            &[],
                        );

                        self.record_shared_chunk_draws(
                            cb,
                            frame,
                            self.chunk_transparent_indirect_offset,
                            self.chunk_transparent_indirect_count,
                        );

                        for &index in &self.transparent_draw_indices {
                            let cmd = &self.draw_cmds[index];
                            let transparent_count = cmd.index_count - cmd.transparent_start;
                            let transparent_offset = cmd.transparent_start;
                            let super::ChunkStorage::Dedicated {
                                vertex_buffer,
                                index_buffer,
                                ..
                            } = &cmd.storage
                            else {
                                continue;
                            };
                            self.device
                                .cmd_bind_vertex_buffers(cb, 0, &[*vertex_buffer], &[0]);
                            self.device.cmd_bind_index_buffer(
                                cb,
                                *index_buffer,
                                0,
                                vk::IndexType::UINT32,
                            );
                            let offset: [f32; 3] = [
                                (cmd.cx * CHUNK_SIZE as i32) as f32,
                                0.0,
                                (cmd.cz * CHUNK_SIZE as i32) as f32,
                            ];
                            self.device.cmd_push_constants(
                                cb,
                                self.pipeline_layout,
                                vk::ShaderStageFlags::VERTEX,
                                0,
                                bytemuck::bytes_of(&offset),
                            );
                            self.device.cmd_draw_indexed(
                                cb,
                                transparent_count,
                                1,
                                transparent_offset,
                                0,
                                0,
                            );
                        }
                    }

                    if self.state.settings.camera_mode() != 0 && self.fp_block_tr_index_count > 0 {
                        if let (Some(vb), Some(ib)) = (
                            self.fp_block_tr_vertex_buffer,
                            self.fp_block_tr_index_buffer,
                        ) {
                            let desc_set = if self.fp_block_uses_item_atlas {
                                self.fp_item_descriptor_sets[frame]
                            } else {
                                self.descriptor_sets[frame]
                            };
                            self.device.cmd_bind_descriptor_sets(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.pipeline_layout,
                                0,
                                &[desc_set],
                                &[],
                            );
                            self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                            self.device
                                .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                            let offset = [0.0f32; 3];
                            self.device.cmd_push_constants(
                                cb,
                                self.pipeline_layout,
                                vk::ShaderStageFlags::VERTEX,
                                0,
                                bytemuck::bytes_of(&offset),
                            );
                            self.device.cmd_draw_indexed(
                                cb,
                                self.fp_block_tr_index_count,
                                1,
                                0,
                                0,
                                0,
                            );
                        }
                    }

                    // Draw 3D particle mesh (world-space billboards)
                    if self.particle_index_count > 0 {
                        if let (Some(vb), Some(ib)) =
                            (self.particle_vertex_buffer, self.particle_index_buffer)
                        {
                            self.device.cmd_bind_pipeline(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.particle_pipeline,
                            );
                            self.device.cmd_bind_descriptor_sets(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.particle_pipeline_layout,
                                0,
                                &[self.descriptor_sets[frame]],
                                &[],
                            );
                            self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                            self.device
                                .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                            self.device
                                .cmd_draw_indexed(cb, self.particle_index_count, 1, 0, 0, 0);
                        }
                    }

                    // Draw nametag mesh (billboard quads, depth test OFF)
                    if self.nametag_index_count > 0 {
                        if let (Some(vb), Some(ib)) =
                            (self.nametag_vertex_buffer, self.nametag_index_buffer)
                        {
                            if self.nametag_pipeline != vk::Pipeline::null() {
                                self.device.cmd_bind_pipeline(
                                    cb,
                                    vk::PipelineBindPoint::GRAPHICS,
                                    self.nametag_pipeline,
                                );
                                self.device.cmd_bind_descriptor_sets(
                                    cb,
                                    vk::PipelineBindPoint::GRAPHICS,
                                    self.nametag_pipeline_layout,
                                    0,
                                    &[self.entity_descriptor_sets[frame]],
                                    &[],
                                );
                                self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                                self.device
                                    .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                                self.device.cmd_draw_indexed(
                                    cb,
                                    self.nametag_index_count,
                                    1,
                                    0,
                                    0,
                                    0,
                                );
                            }
                        }
                    }
                }

                // Draw first person hand
                if self.state.settings.camera_mode() == 0
                    && !self.state.hud.chat_open()
                    && !self.state.inventory.inventory_open()
                    && self.state.hud.health() > 0.0
                {
                    let clear_rect = vk::ClearRect {
                        rect: vk::Rect2D {
                            offset: vk::Offset2D { x: 0, y: 0 },
                            extent: self.swapchain.swapchain_extent,
                        },
                        base_array_layer: 0,
                        layer_count: 1,
                    };
                    let clear_depth = vk::ClearAttachment {
                        aspect_mask: vk::ImageAspectFlags::DEPTH,
                        color_attachment: 0,
                        clear_value: vk::ClearValue {
                            depth_stencil: vk::ClearDepthStencilValue {
                                depth: 1.0,
                                stencil: 0,
                            },
                        },
                    };
                    self.device
                        .cmd_clear_attachments(cb, &[clear_depth], &[clear_rect]);

                    // Draw arm (Entity Pipeline)
                    if self.fp_arm_index_count > 0 {
                        if let (Some(vb), Some(ib)) =
                            (self.fp_arm_vertex_buffer, self.fp_arm_index_buffer)
                        {
                            self.device.cmd_bind_pipeline(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.entity_pipeline,
                            );
                            self.device.cmd_bind_descriptor_sets(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.entity_pipeline_layout,
                                0,
                                &[self.skin_descriptor_sets[frame]],
                                &[],
                            );
                            self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                            self.device
                                .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                            self.device
                                .cmd_draw_indexed(cb, self.fp_arm_index_count, 1, 0, 0, 0);
                        }
                    }

                    // Draw block (World Pipeline) - opaque
                    if self.fp_block_op_index_count > 0 {
                        if let (Some(vb), Some(ib)) = (
                            self.fp_block_op_vertex_buffer,
                            self.fp_block_op_index_buffer,
                        ) {
                            self.device.cmd_bind_pipeline(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.pipeline,
                            );

                            let desc_set = if self.fp_block_uses_item_atlas {
                                self.fp_item_descriptor_sets[frame]
                            } else {
                                self.descriptor_sets[frame]
                            };

                            self.device.cmd_bind_descriptor_sets(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.pipeline_layout,
                                0,
                                &[desc_set],
                                &[],
                            );
                            self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                            self.device
                                .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                            let offset: [f32; 3] = [0.0, 0.0, 0.0];
                            self.device.cmd_push_constants(
                                cb,
                                self.pipeline_layout,
                                vk::ShaderStageFlags::VERTEX,
                                0,
                                bytemuck::bytes_of(&offset),
                            );
                            self.device.cmd_draw_indexed(
                                cb,
                                self.fp_block_op_index_count,
                                1,
                                0,
                                0,
                                0,
                            );
                        }
                    }

                    // Draw block (World Pipeline) - transparent
                    if self.fp_block_tr_index_count > 0
                        && self.transparent_pipeline != vk::Pipeline::null()
                    {
                        if let (Some(vb), Some(ib)) = (
                            self.fp_block_tr_vertex_buffer,
                            self.fp_block_tr_index_buffer,
                        ) {
                            self.device.cmd_bind_pipeline(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.transparent_pipeline,
                            );

                            let desc_set = if self.fp_block_uses_item_atlas {
                                self.fp_item_descriptor_sets[frame]
                            } else {
                                self.descriptor_sets[frame]
                            };

                            self.device.cmd_bind_descriptor_sets(
                                cb,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.pipeline_layout,
                                0,
                                &[desc_set],
                                &[],
                            );
                            self.device.cmd_bind_vertex_buffers(cb, 0, &[vb], &[0]);
                            self.device
                                .cmd_bind_index_buffer(cb, ib, 0, vk::IndexType::UINT32);
                            let offset: [f32; 3] = [0.0, 0.0, 0.0];
                            self.device.cmd_push_constants(
                                cb,
                                self.pipeline_layout,
                                vk::ShaderStageFlags::VERTEX,
                                0,
                                bytemuck::bytes_of(&offset),
                            );
                            self.device.cmd_draw_indexed(
                                cb,
                                self.fp_block_tr_index_count,
                                1,
                                0,
                                0,
                                0,
                            );
                        }
                    }
                }
            }

            // Rebind world pipeline for GUI
            if self.entity_pipeline != vk::Pipeline::null() {
                self.device
                    .cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            }

            if let Some((
                ref mut overlay_gui,
                ref mut background_gui,
                ref mut widget_gui,
                ref mut inventory_gui,
                ref mut generic54_gui,
                ref mut font_gui,
                ref mut block_gui,
                ref mut item_gui,
                ref mut icons_gui,
                ref mut creative_gui,
            )) = gui_builders.as_mut()
            {
                self.draw_gui_options_background(cb, frame, background_gui);
                // Full-screen menu overlay (drawn behind all controls)
                self.draw_gui_overlay(cb, frame, overlay_gui);
                self.draw_gui_creative(cb, frame, creative_gui);
                self.draw_gui_widget(cb, frame, widget_gui);
                self.draw_gui_inventory(cb, frame, inventory_gui);
                self.draw_gui_generic54(cb, frame, generic54_gui);
                self.draw_gui_blocks(cb, frame, block_gui);
                self.draw_gui_items(cb, frame, item_gui);
                self.draw_gui_icons(cb, frame, icons_gui);
                self.draw_gui(cb, frame, font_gui);
            }

            // Underwater overlay: full-screen animated texture
            if self.state.settings.underwater() {
                let mut underwater_gui = super::gui::GuiVertexBuilder::new();
                self.draw_gui_underwater(
                    cb,
                    frame,
                    &mut underwater_gui,
                    self.state.settings.underwater_yaw(),
                    self.state.settings.underwater_pitch(),
                );
            }

            self.device.cmd_end_render_pass(cb);
            if let Err(e) = self.device.end_command_buffer(cb) {
                log::error!("end_command_buffer failed: {e:?}");
                self.swapchain.needs_recreate = true;
                return;
            }
        }
        self.state.frame_profile.set_frame_command_us(command_started.elapsed().as_micros() as u64);

        if gui_builders.is_some() {
            self.gui_builder_cache = gui_builders.take();
        }

        let submit_started = std::time::Instant::now();
        unsafe {
            if let Err(e) = self.device.queue_submit(
                self.queue,
                &[vk::SubmitInfo {
                    wait_semaphore_count: 1,
                    p_wait_semaphores: &self.swapchain.image_available[frame],
                    p_wait_dst_stage_mask: &vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    command_buffer_count: 1,
                    p_command_buffers: &cb,
                    signal_semaphore_count: 1,
                    p_signal_semaphores: &self.swapchain.render_finished[frame],
                    ..Default::default()
                }],
                fence,
            ) {
                log::error!("queue_submit failed (device likely lost): {e:?}");
                self.swapchain.needs_recreate = true;
                return;
            }
        }
        self.state.frame_profile.set_frame_submit_us(submit_started.elapsed().as_micros() as u64);
        self.retired_draw_cmds[frame].append(&mut self.pending_retired_draw_cmds);

        let needs_resize = self.swapchain.present(self.queue, image_index, frame);
        let present_started = std::time::Instant::now();
        if needs_resize {
            self.swapchain.needs_recreate = true;
        }
        self.state.frame_profile.set_frame_present_us(present_started.elapsed().as_micros() as u64);

        let cpu_us = t0.elapsed().as_micros() as u64;
        self.state.frame_profile.set_frame_gpu_us(fence_us);
        self.state.frame_profile.set_frame_cpu_us(cpu_us.saturating_sub(fence_us));

        self.swapchain.advance_frame();
    }

    fn sync_player_skin_atlas(&mut self) {
        let content_hash = self.state.hud.player_skin_content_hash();
        if self.synced_player_skin_content_hash == Some(content_hash) {
            return;
        }

        let layout_changed = self.player_skin_layout_hash != self.state.hud.player_skin_layout_hash();
        let atlas_changed = self.entity_atlas.as_mut().is_some_and(|atlas| {
            let changed = atlas.sync_player_skins(&self.state.hud.pending_player_skins());
            if atlas.take_full_upload_required() {
                self.entity_atlas_full_upload_pending = true;
            }
            changed
        });
        if atlas_changed {
            self.entity_skin_upload_pending = true;
            if layout_changed {
                self.player_skin_atlas_generation =
                    self.player_skin_atlas_generation.wrapping_add(1);
            }
        }
        self.synced_player_skin_content_hash = Some(content_hash);
        self.player_skin_layout_hash = self.state.hud.player_skin_layout_hash();
    }

    fn prepare_entity_atlas_upload(&mut self, frame: usize) {
        if !self.entity_skin_upload_pending && !self.entity_atlas_full_upload_pending {
            return;
        }
        let Some(atlas) = self.entity_atlas.as_ref() else {
            return;
        };
        let pixels = if self.entity_atlas_full_upload_pending {
            atlas.pixels.as_slice()
        } else {
            atlas.player_skin_pixels()
        };
        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.entity_skin_upload_buffers[frame],
            &mut self.entity_skin_upload_allocs[frame],
            &mut self.entity_skin_upload_capacities[frame],
            vk::BufferUsageFlags::TRANSFER_SRC,
            pixels,
        );
    }

    fn record_entity_skin_upload(&mut self, cb: vk::CommandBuffer, frame: usize) {
        if !self.entity_skin_upload_pending && !self.entity_atlas_full_upload_pending {
            return;
        }
        let Some(staging) = self.entity_skin_upload_buffers[frame] else {
            return;
        };
        unsafe {
            self.device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::SHADER_READ,
                    dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    old_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    image: self.entity_texture_image,
                    subresource_range: super::color_subresource(),
                    ..Default::default()
                }],
            );
            let (y, width, height) = if self.entity_atlas_full_upload_pending {
                (
                    0,
                    super::entity::atlas::ENTITY_ATLAS_SIZE,
                    super::entity::atlas::ENTITY_ATLAS_SIZE,
                )
            } else {
                (
                    super::entity::atlas::PLAYER_SKIN_ATLAS_Y,
                    super::entity::atlas::ENTITY_ATLAS_SIZE,
                    super::entity::atlas::PLAYER_SKIN_ATLAS_HEIGHT,
                )
            };
            self.device.cmd_copy_buffer_to_image(
                cb,
                staging,
                self.entity_texture_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy {
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        layer_count: 1,
                        ..Default::default()
                    },
                    image_offset: vk::Offset3D {
                        x: 0,
                        y: y as i32,
                        z: 0,
                    },
                    image_extent: vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    },
                    ..Default::default()
                }],
            );
            self.device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    dst_access_mask: vk::AccessFlags::SHADER_READ,
                    old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    image: self.entity_texture_image,
                    subresource_range: super::color_subresource(),
                    ..Default::default()
                }],
            );
        }
        self.entity_skin_upload_pending = false;
        self.entity_atlas_full_upload_pending = false;
    }

    fn prepare_local_skin_upload(&mut self, frame: usize) {
        if !self.local_skin_upload_pending {
            return;
        }
        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.local_skin_upload_buffers[frame],
            &mut self.local_skin_upload_allocs[frame],
            &mut self.local_skin_upload_capacities[frame],
            vk::BufferUsageFlags::TRANSFER_SRC,
            self.state.settings.local_skin().pixels.as_raw(),
        );
    }

    fn record_local_skin_upload(&mut self, cb: vk::CommandBuffer, frame: usize) {
        if !self.local_skin_upload_pending {
            return;
        }
        let Some(staging) = self.local_skin_upload_buffers[frame] else {
            return;
        };
        let (width, height) = self.state.settings.local_skin().dimensions();
        unsafe {
            self.device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::SHADER_READ,
                    dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    old_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    image: self.skin_texture_image,
                    subresource_range: super::color_subresource(),
                    ..Default::default()
                }],
            );
            self.device.cmd_copy_buffer_to_image(
                cb,
                staging,
                self.skin_texture_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy {
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        layer_count: 1,
                        ..Default::default()
                    },
                    image_extent: vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    },
                    ..Default::default()
                }],
            );
            self.device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    dst_access_mask: vk::AccessFlags::SHADER_READ,
                    old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    image: self.skin_texture_image,
                    subresource_range: super::color_subresource(),
                    ..Default::default()
                }],
            );
        }
        self.local_skin_upload_pending = false;
    }

    /// Upload cape pixels into the entity atlas "player/cape" slot and
    /// schedule a full atlas GPU re-upload.
    pub fn upload_cape_to_atlas(&mut self, pixels: &[u8]) {
        if let Some(ref mut atlas) = self.entity_atlas {
            atlas.upload_cape(pixels, 64, 32);
            self.entity_atlas_full_upload_pending = true;
        }
    }

    fn upload_entity_meshes_cached(
        &mut self,
        frustum: &crate::client::player::Frustum,
        camera: &crate::client::player::Camera,
    ) {
        use super::entity::mesh::{
            generate_entity_mesh, AnimationFamily, EntityPose, EntityVertex,
        };
        use super::entity::models::{atlas_name_for_entity, model_for_entity};
        use nalgebra::Point3;
        use std::hash::Hasher;

        let skin_started = std::time::Instant::now();
        self.sync_player_skin_atlas();
        self.state.frame_profile.set_entity_skin_sync_us(skin_started.elapsed().as_micros() as u64);

        self.entity_frame_generation = self.entity_frame_generation.wrapping_add(1);
        let generation = self.entity_frame_generation;
        self.visible_entity_ids.clear();
        self.state.frame_profile.set_entity_visible_count(0);
        self.state.frame_profile.set_entity_culled_count(0);
        self.state.frame_profile.set_entity_cache_hits(0);
        self.state.frame_profile.set_entity_cache_misses(0);
        self.state.frame_profile.set_entity_append_us(0);

        let profile = self.state.settings.debug_overlay();
        let mut hash_ns = 0u128;
        let mut lookup_ns = 0u128;
        let mut generate_ns = 0u128;
        let mut upload_ns = 0u128;
        let item_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f32();
        let loop_started = std::time::Instant::now();
        let mut mob_jobs: Vec<MobMeshJob> = Vec::new();
        let mut entity_invisible: std::collections::HashMap<i32, bool> =
            std::collections::HashMap::new();

        for billboard in self.state.hud.entity_billboards() {
            self.entity_gpu_meshes
                .entry(billboard.entity_id)
                .or_default()
                .last_seen = generation;

            let (width, height) = billboard.entity_type.bounding_box();
            let half = width.max(height) * 0.5;
            let position = billboard.position;
            if !frustum.test_aabb(
                [position[0] - half, position[1], position[2] - half],
                [position[0] + half, position[1] + height, position[2] + half],
            ) {
                self.state.frame_profile.inc_entity_culled_count();
                continue;
            }
            self.state.frame_profile.inc_entity_visible_count();
            self.visible_entity_ids.push(billboard.entity_id);

            let hash_started = profile.then(std::time::Instant::now);
            let mut state_hasher = fnv::FnvHasher::default();
            state_hasher.write_u64(super::entity_mesh_state_hash(billboard));
            state_hasher.write_u64(self.entity_atlas_generation);
            if billboard.skin_key.is_some() {
                state_hasher.write_u64(self.player_skin_atlas_generation);
            }
            state_hasher.write_u32(self.state.settings.entity_shadows() as u32);
            // sub_tick drives the continuous bob/rotation of dropped items only
            // (see generate_dropped_item_mesh). Writing it for every entity made
            // all visible mobs/players/projectiles cache-miss every 50 ms.
            if billboard.kind == super::EntityBillboardKind::Item {
                let sub_tick = (item_time * 20.0) as u32;
                state_hasher.write_u32(sub_tick);
            }
            let state_hash = state_hasher.finish();
            if let Some(started) = hash_started {
                hash_ns += started.elapsed().as_nanos();
            }

            let lookup_started = profile.then(std::time::Instant::now);
            let cached = self
                .entity_gpu_meshes
                .get(&billboard.entity_id)
                .is_some_and(|mesh| mesh.state_hash[self.swapchain.current_frame] == Some(state_hash));
            if let Some(started) = lookup_started {
                lookup_ns += started.elapsed().as_nanos();
            }
            if cached {
                self.state.frame_profile.inc_entity_cache_hits();
                continue;
            }

            let generate_started = profile.then(std::time::Instant::now);
            let mut body_vertices = Vec::new();
            let mut body_indices = Vec::new();
            let mut held_block_vertices = Vec::new();
            let mut held_block_indices = Vec::new();
            let mut held_item_vertices = Vec::new();
            let mut held_item_indices = Vec::new();

            if billboard.kind == super::EntityBillboardKind::Item {
                if let Some(item_id) = billboard.item_id {
                    let item_glint = billboard
                        .item_nbt
                        .as_deref()
                        .map(|nbt| {
                            crate::net::nbt::parse_root(nbt)
                                .ok()
                                .and_then(|root| {
                                    root.as_compound().map(|c| {
                                        c.get("ench")
                                            .and_then(|t| t.as_list())
                                            .is_some_and(|l| !l.is_empty())
                                            || c.get("StoredEnchantments")
                                                .and_then(|t| t.as_list())
                                                .is_some_and(|l| !l.is_empty())
                                    })
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    let (op_verts, op_idx, tr_verts, tr_idx, uses_item_atlas) =
                        crate::render::hud::hand::generate_dropped_item_mesh(
                            item_id,
                            billboard.item_damage.unwrap_or(0),
                            position,
                            billboard.age_ticks,
                            billboard.hover_start,
                            item_time,
                            item_glint,
                        );
                    if uses_item_atlas {
                        append_world_mesh(
                            &mut held_item_vertices,
                            &mut held_item_indices,
                            op_verts,
                            op_idx,
                        );
                        append_world_mesh(
                            &mut held_item_vertices,
                            &mut held_item_indices,
                            tr_verts,
                            tr_idx,
                        );
                    } else {
                        append_world_mesh(
                            &mut held_block_vertices,
                            &mut held_block_indices,
                            op_verts,
                            op_idx,
                        );
                        append_world_mesh(
                            &mut held_block_vertices,
                            &mut held_block_indices,
                            tr_verts,
                            tr_idx,
                        );
                    }
                }
            } else if billboard.entity_type == crate::entity::EntityType::Arrow {
                if let Some(region) = self
                    .entity_atlas
                    .as_ref()
                    .and_then(|atlas| atlas.region_for("arrow"))
                    .copied()
                {
                    append_arrow_entity_mesh(
                        &mut body_vertices,
                        &mut body_indices,
                        position,
                        billboard.yaw,
                        billboard.pitch,
                        region,
                    );
                } else {
                    let (op_verts, op_idx, tr_verts, tr_idx, _) =
                        crate::render::hud::hand::generate_arrow_entity_mesh(
                            position,
                            billboard.yaw,
                            billboard.pitch,
                        );
                    append_world_mesh(
                        &mut held_item_vertices,
                        &mut held_item_indices,
                        op_verts,
                        op_idx,
                    );
                    append_world_mesh(
                        &mut held_item_vertices,
                        &mut held_item_indices,
                        tr_verts,
                        tr_idx,
                    );
                }
            } else if let Some((item_id, item_damage, scale)) = match billboard.entity_type {
                crate::entity::EntityType::Snowball => Some((332, 0, 0.5)),
                crate::entity::EntityType::ThrownEgg => Some((344, 0, 0.5)),
                crate::entity::EntityType::EnderPearl => Some((368, 0, 0.5)),
                crate::entity::EntityType::ThrownPotion => Some((373, 0, 0.5)),
                crate::entity::EntityType::ThrownExpBottle => Some((384, 0, 0.5)),
                crate::entity::EntityType::EnderEye => Some((381, 0, 0.5)),
                crate::entity::EntityType::Fireball => Some((385, 0, 2.0)),
                crate::entity::EntityType::SmallFireball => Some((385, 0, 0.5)),
                _ => None,
            } {
                let (op_verts, op_idx, tr_verts, tr_idx, uses_item_atlas) =
                    crate::render::hud::hand::generate_projectile_item_mesh(
                        item_id,
                        item_damage,
                        position,
                        scale,
                    );
                if uses_item_atlas {
                    append_world_mesh(
                        &mut held_item_vertices,
                        &mut held_item_indices,
                        op_verts,
                        op_idx,
                    );
                    append_world_mesh(
                        &mut held_item_vertices,
                        &mut held_item_indices,
                        tr_verts,
                        tr_idx,
                    );
                } else {
                    append_world_mesh(
                        &mut held_block_vertices,
                        &mut held_block_indices,
                        op_verts,
                        op_idx,
                    );
                    append_world_mesh(
                        &mut held_block_vertices,
                        &mut held_block_indices,
                        tr_verts,
                        tr_idx,
                    );
                }
            } else {
                let model_key = super::entity_model_key(billboard);
                let cuboids = std::sync::Arc::clone(
                    self.entity_model_cache.entry(model_key).or_insert_with(|| {
                        std::sync::Arc::new(model_for_entity(
                            billboard.entity_type,
                            billboard.visual,
                            billboard.slim,
                            billboard.skin_parts_mask,
                            billboard.has_cape,
                        ))
                    }),
                );
                let pose = EntityPose {
                    model_scale: match billboard.entity_type {
                        crate::entity::EntityType::Giant => 6.0,
                        crate::entity::EntityType::CaveSpider => 0.7,
                        crate::entity::EntityType::Guardian if billboard.visual.guardian_elder => {
                            2.35
                        }
                        crate::entity::EntityType::ArmorStand
                            if billboard.visual.armor_stand_flags & 0x01 != 0 =>
                        {
                            0.5
                        }
                        _ => 1.0,
                    },
                    animation_family: match billboard.entity_type {
                        crate::entity::EntityType::Player
                        | crate::entity::EntityType::Zombie
                        | crate::entity::EntityType::PigZombie
                        | crate::entity::EntityType::Giant
                        | crate::entity::EntityType::Skeleton
                        | crate::entity::EntityType::Enderman
                        | crate::entity::EntityType::Witch
                        | crate::entity::EntityType::Villager => AnimationFamily::Biped,
                        crate::entity::EntityType::ArmorStand => AnimationFamily::ArmorStand,
                        crate::entity::EntityType::Pig
                        | crate::entity::EntityType::Sheep
                        | crate::entity::EntityType::Cow
                        | crate::entity::EntityType::Mooshroom
                        | crate::entity::EntityType::Wolf
                        | crate::entity::EntityType::Ocelot
                        | crate::entity::EntityType::Horse => AnimationFamily::Quadruped,
                        _ => AnimationFamily::Generic,
                    },
                    body_yaw: (180.0 - billboard.yaw).to_radians(),
                    head_yaw: (billboard.yaw - billboard.head_yaw).to_radians(),
                    pitch: billboard.pitch.to_radians(),
                    limb_swing: billboard.limb_swing,
                    limb_swing_amount: billboard.limb_swing_amount,
                    swing_progress: billboard.swing_alpha,
                    death_progress: billboard.death_alpha,
                    age_ticks: billboard.age_ticks,
                    holding_item: billboard.equipment[0]
                        .or_else(|| billboard.held_item.map(|id| (id, 0)))
                        .is_some_and(|(id, _)| id != 0),
                    armor_stand_rotations: billboard.visual.armor_stand_rotations,
                    sneaking: billboard.sneaking,
                    riding: billboard.riding,
                    blocking: billboard.blocking,
                    cape_rotation: billboard.cape_rotation,
                };
                let atlas_uv = self.entity_atlas.as_ref().and_then(|atlas| {
                    let atlas_name = billboard.skin_key.as_deref().unwrap_or_else(|| {
                        atlas_name_for_entity(billboard.entity_type, billboard.visual)
                    });
                    // Never leave atlas_uv=None: local 0..1 UVs sample the whole
                    // 4096 atlas and scramble every mob texture.
                    atlas
                        .region_for(atlas_name)
                        .or_else(|| {
                            (billboard.entity_type == crate::entity::EntityType::Player)
                                .then(|| atlas.region_for("player"))
                                .flatten()
                        })
                        .or_else(|| atlas.region_for("__white"))
                        .map(|r| [r.u_min, r.v_min, r.u_max, r.v_max])
                });
                let cape_atlas_uv = self.entity_atlas.as_ref().and_then(|atlas| {
                    let player_cape = billboard
                        .skin_key
                        .as_ref()
                        .and_then(|key| atlas.region_for(&format!("cape/{key}")));
                    player_cape
                        .or_else(|| {
                            billboard
                                .skin_key
                                .is_none()
                                .then(|| atlas.region_for("player/cape"))
                                .flatten()
                        })
                        .map(|r| [r.u_min, r.v_min, r.u_max, r.v_max])
                });
                let armor_layers = if billboard.entity_type == crate::entity::EntityType::Player {
                    billboard
                        .equipment
                        .iter()
                        .enumerate()
                        .skip(1)
                        .filter_map(|(equipment_slot, equipment)| {
                            let (item_id, _) = (*equipment)?;
                            let (material, armor_slot) =
                                crate::client::armor::armor_material_and_slot(item_id)?;
                            if equipment_slot != 4 - armor_slot {
                                return None;
                            }
                            let texture_layer = if equipment_slot == 2 { 2 } else { 1 };
                            let texture_name =
                                crate::client::armor::armor_texture_name(material, texture_layer);
                            let region = self.entity_atlas.as_ref()?.region_for(texture_name)?;
                            let color = if material == crate::client::armor::ArmorMaterial::Leather
                            {
                                [
                                    0xa0 as f32 / 255.0,
                                    0x65 as f32 / 255.0,
                                    0x40 as f32 / 255.0,
                                    1.0,
                                ]
                            } else {
                                [1.0; 4]
                            };
                            Some(ArmorMeshLayer {
                                cuboids: super::entity::models::player_armor_model(equipment_slot),
                                atlas_uv: [region.u_min, region.v_min, region.u_max, region.v_max],
                                color,
                            })
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                entity_invisible.insert(billboard.entity_id, billboard.invisible);
                mob_jobs.push(MobMeshJob {
                    entity_id: billboard.entity_id,
                    cuboids,
                    world_position: Point3::new(position[0], position[1], position[2]),
                    pose,
                    atlas_uv,
                    cape_atlas_uv,
                    armor_layers,
                    hurt_alpha: billboard.hurt_alpha,
                    sky_light: billboard.sky_light,
                    block_light: billboard.block_light,
                    entity_type: billboard.entity_type,
                    position: billboard.position,
                    held_item_and_damage: billboard.equipment[0]
                        .or_else(|| billboard.held_item.map(|id| (id, 0)))
                        .filter(|(id, _)| *id != 0),
                    state_hash,
                });
                if let Some(started) = generate_started {
                    generate_ns += started.elapsed().as_nanos();
                }
                continue;
            }

            if billboard.invisible {
                body_vertices.clear();
                body_indices.clear();
                held_block_vertices.clear();
                held_block_indices.clear();
                held_item_vertices.clear();
                held_item_indices.clear();
            }
            if self.state.settings.entity_shadows() && !billboard.invisible {
                append_entity_shadow(&mut body_vertices, &mut body_indices, billboard);
            }
            if let Some(started) = generate_started {
                generate_ns += started.elapsed().as_nanos();
            }

            let upload_started = std::time::Instant::now();
            let Some(mesh) = self.entity_gpu_meshes.get_mut(&billboard.entity_id) else {
                log::warn!(
                    "visible entity cache entry missing for entity_id={}; skipping upload",
                    billboard.entity_id
                );
                continue;
            };
            // Pre-populate all MAX_FRAMES slots so a sudden burst of entities
            // (e.g. camera rotation) never repeats the same regeneration
            // across the next two frame cycles.
            for slot in 0..super::MAX_FRAMES {
                upload_cached_gpu_mesh(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut mesh.body[slot],
                    bytemuck::cast_slice(&body_vertices),
                    bytemuck::cast_slice(&body_indices),
                    body_indices.len(),
                );
                upload_cached_gpu_mesh(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut mesh.held_block[slot],
                    bytemuck::cast_slice(&held_block_vertices),
                    bytemuck::cast_slice(&held_block_indices),
                    held_block_indices.len(),
                );
                upload_cached_gpu_mesh(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut mesh.held_item[slot],
                    bytemuck::cast_slice(&held_item_vertices),
                    bytemuck::cast_slice(&held_item_indices),
                    held_item_indices.len(),
                );
                mesh.state_hash[slot] = Some(state_hash);
            }
            upload_ns += upload_started.elapsed().as_nanos();
            self.state.frame_profile.inc_entity_cache_misses();
        }

        // Generate mob/player meshes in parallel across the Rayon thread pool,
        // then upload them sequentially.  The scan loop above only collected
        // the read-only data (cuboids, pose, atlas UV info) for each cache-miss
        // entity — the expensive world-space vertex generation happens here.
        if !mob_jobs.is_empty() {
            let par_started = std::time::Instant::now();
            use rayon::prelude::*;
            let results: Vec<MobMeshResult> = mob_jobs
                .par_iter()
                .map(generate_mob_mesh_in_parallel)
                .collect();
            generate_ns += par_started.elapsed().as_nanos();

            for result in results {
                let mut body_v = result.body_vertices;
                let mut body_i = result.body_indices;
                let mut hb_v = result.held_block_vertices;
                let mut hb_i = result.held_block_indices;
                let mut hi_v = result.held_item_vertices;
                let mut hi_i = result.held_item_indices;

                let invisible = entity_invisible
                    .get(&result.entity_id)
                    .copied()
                    .unwrap_or(false);
                if invisible {
                    body_v.clear();
                    body_i.clear();
                    hb_v.clear();
                    hb_i.clear();
                    hi_v.clear();
                    hi_i.clear();
                }
                if self.state.settings.entity_shadows()
                    && !invisible
                    && (result.entity_type.is_mob()
                        || result.entity_type == crate::entity::EntityType::Item
                        || result.entity_type == crate::entity::EntityType::XPOrb)
                {
                    let (width, _) = result.entity_type.bounding_box();
                    let size = width * 0.75;
                    let pos = result.position;
                    let base = body_v.len() as u32;
                    for point in [
                        [pos[0] - size, 0.01, pos[2] - size],
                        [pos[0] + size, 0.01, pos[2] - size],
                        [pos[0] + size, 0.01, pos[2] + size],
                        [pos[0] - size, 0.01, pos[2] + size],
                    ] {
                        body_v.push(super::entity::mesh::EntityVertex {
                            position: point,
                            normal: [0.0, 1.0, 0.0],
                            uv: [0.0, 0.0],
                            color: [0.0, 0.0, 0.0, 0.35],
                        });
                    }
                    body_i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                }

                let upload_per = std::time::Instant::now();
                let Some(mesh) = self.entity_gpu_meshes.get_mut(&result.entity_id) else {
                    log::warn!(
                        "visible mob entity cache entry missing for entity_id={}; skipping upload",
                        result.entity_id
                    );
                    continue;
                };
                for slot in 0..super::MAX_FRAMES {
                    upload_cached_gpu_mesh(
                        &self.device,
                        self.resources.allocator_mut(),
                        &mut mesh.body[slot],
                        bytemuck::cast_slice(&body_v),
                        bytemuck::cast_slice(&body_i),
                        body_i.len(),
                    );
                    upload_cached_gpu_mesh(
                        &self.device,
                        self.resources.allocator_mut(),
                        &mut mesh.held_block[slot],
                        bytemuck::cast_slice(&hb_v),
                        bytemuck::cast_slice(&hb_i),
                        hb_i.len(),
                    );
                    upload_cached_gpu_mesh(
                        &self.device,
                        self.resources.allocator_mut(),
                        &mut mesh.held_item[slot],
                        bytemuck::cast_slice(&hi_v),
                        bytemuck::cast_slice(&hi_i),
                        hi_i.len(),
                    );
                    mesh.state_hash[slot] = Some(result.state_hash);
                }
                self.state.frame_profile.inc_entity_cache_misses();
                upload_ns += upload_per.elapsed().as_nanos();
            }
        }

        self.state.frame_profile.set_entity_loop_us(loop_started.elapsed().as_micros() as u64);

        let prune_started = std::time::Instant::now();
        self.stale_entity_ids.clear();
        self.stale_entity_ids.extend(
            self.entity_gpu_meshes
                .iter()
                .filter_map(|(&entity_id, mesh)| {
                    (mesh.last_seen != generation).then_some(entity_id)
                }),
        );
        for entity_id in self.stale_entity_ids.drain(..) {
            if let Some(mut mesh) = self.entity_gpu_meshes.remove(&entity_id) {
                super::destroy_entity_gpu_mesh(&self.device, self.resources.allocator_mut(), &mut mesh);
            }
        }
        self.state.frame_profile.set_entity_prune_us(prune_started.elapsed().as_micros() as u64);

        self.state.frame_profile.set_entity_hash_us((hash_ns / 1_000) as u64);
        self.state.frame_profile.set_entity_lookup_us((lookup_ns / 1_000) as u64);
        self.state.frame_profile.set_entity_generate_us((generate_ns / 1_000) as u64);
        self.state.frame_profile.set_entity_upload_us((upload_ns / 1_000) as u64);
        self.state.frame_profile.set_entity_batch_reused(self.state.frame_profile.entity_cache_misses() == 0);

        let extras_started = std::time::Instant::now();
        self.upload_entity_extras(camera);
        self.state.frame_profile.set_entity_extras_us(extras_started.elapsed().as_micros() as u64);
    }

    fn upload_entity_extras(&mut self, camera: &crate::client::player::Camera) {
        use super::entity::mesh::EntityVertex;
        use std::hash::Hasher;

        // Vanilla only renders sign text within 64 blocks. Filtering before
        // atlas allocation prevents distant or stale tile entities from
        // consuming every runtime text slot. Coordinate order makes the atlas
        // layout deterministic even though sign_data originates in a HashMap.
        let mut visible_signs: Vec<_> = self.state.hud.sign_entries().iter()
            .filter(|sign| {
                let dx = sign.position[0] as f32 + 0.5 - camera.position.x;
                let dy = sign.position[1] as f32 + 0.5 - camera.position.y;
                let dz = sign.position[2] as f32 + 0.5 - camera.position.z;
                dx * dx + dy * dy + dz * dz <= 4096.0
            })
            .cloned()
            .collect();
        visible_signs.sort_unstable_by_key(|sign| sign.position);

        let mut hasher = fnv::FnvHasher::default();
        hasher.write_u64(self.entity_atlas_generation);
        hasher.write_u64(self.player_skin_atlas_generation);
        for skull in self.state.hud.skull_entries() {
            hasher.write_i32(skull.position[0]);
            hasher.write_i32(skull.position[1]);
            hasher.write_i32(skull.position[2]);
            hasher.write_u8(skull.block_metadata);
            hasher.write_u8(skull.skull_type);
            hasher.write_u8(skull.rotation);
            hasher.write(skull.skin_key.as_bytes());
        }
        for chest in self.state.hud.chest_entries() {
            for value in chest.position {
                hasher.write_i32(value);
            }
            hasher.write_u16(chest.block.to_id());
            hasher.write_u8(chest.metadata);
            hasher.write_u32(chest.lid_angle.to_bits());
            hasher.write_u8(chest.double_x as u8);
            hasher.write_u8(chest.double_z as u8);
            hasher.write_u8(chest.sky_light);
            hasher.write_u8(chest.block_light);
        }
        for sign in &visible_signs {
            for value in &sign.position {
                hasher.write_i32(*value);
            }
            hasher.write_u8(sign.wall_mounted as u8);
            hasher.write_u8(sign.metadata);
            for line in &sign.lines {
                hasher.write(line.as_bytes());
                hasher.write_u8(0);
            }
        }
        let extras_hash = hasher.finish();
        if extras_hash == self.entity_state_hash
            && (self.entity_index_count == 0 || self.entity_vertex_buffer.is_some())
        {
            return;
        }
        self.entity_state_hash = extras_hash;
        self.entity_mesh_vertices.clear();
        self.entity_mesh_indices.clear();

        if let Some(atlas) = self.entity_atlas.as_ref() {
            for skull in self.state.hud.skull_entries() {
                if let Some(region) = atlas.region_for(&skull.skin_key) {
                    append_player_skull_mesh(
                        &mut self.entity_mesh_vertices,
                        &mut self.entity_mesh_indices,
                        skull,
                        region,
                    );
                }
            }
            for chest in self.state.hud.chest_entries() {
                // Static texture keys for the six chest variants — avoids the
                // per-frame `format!` that used to build these lookup strings.
                let texture: &'static str = if chest.double_x || chest.double_z {
                    match chest.block {
                        crate::world::block::Block::TrappedChest => "chest_trapped_double",
                        crate::world::block::Block::EnderChest => "chest_ender_double",
                        _ => "chest_normal_double",
                    }
                } else {
                    match chest.block {
                        crate::world::block::Block::TrappedChest => "chest_trapped",
                        crate::world::block::Block::EnderChest => "chest_ender",
                        _ => "chest_normal",
                    }
                };
                if let Some(region) = atlas.region_for(texture) {
                    append_chest_entity_mesh(
                        &mut self.entity_mesh_vertices,
                        &mut self.entity_mesh_indices,
                        chest,
                        *region,
                    );
                }
            }
        }

        let mut sign_hasher = fnv::FnvHasher::default();
        sign_hasher.write_u64(self.entity_atlas_generation);
        for sign in &visible_signs {
            for value in &sign.position {
                sign_hasher.write_i32(*value);
            }
            for line in &sign.lines {
                sign_hasher.write(line.as_bytes());
                sign_hasher.write_u8(0);
            }
        }
        let sign_atlas_hash = sign_hasher.finish();
        // Reusable key buffer — avoids the per-sign `format!` allocation on the
        // hot path. The position is hashed to a u64 id so the key stays short.
        use std::fmt::Write as _;
        let mut sign_key_buf = String::with_capacity(24);
        if sign_atlas_hash != self.sign_atlas_hash {
            let mut atlas_changed = false;
            if let Some(atlas) = self.entity_atlas.as_mut() {
                atlas.clear_sign_texts();
                // Nametags must be cleared too so pack_sign_text below can
                // find free 180x80 cells in the atlas.  upload_nametag_mesh
                // will rebuild them later in this frame.
                atlas.clear_nametag_texts();
                self.nametag_text_hash = !sign_atlas_hash;
            }
            for sign in &visible_signs {
                let position = sign.position;
                sign_key_buf.clear();
                let _ = write!(sign_key_buf, "sign_{}", sign_position_key_id(position));
                let texture = build_sign_texture(&mut self.font, &sign.lines);
                if self
                    .entity_atlas
                    .as_mut()
                    .and_then(|atlas| atlas.pack_sign_text(&sign_key_buf, &texture.0, texture.1, texture.2))
                    .is_some()
                {
                    atlas_changed = true;
                } else {
                    log::warn!("entity texture atlas has no room for sign text '{}'", sign_key_buf);
                }
            }
            if atlas_changed {
                self.entity_atlas_full_upload_pending = true;
            }
            self.sign_atlas_hash = sign_atlas_hash;
        }

        for sign in &visible_signs {
            let position = sign.position;
            sign_key_buf.clear();
            let _ = write!(sign_key_buf, "sign_{}", sign_position_key_id(position));
            let Some(region) = self
                .entity_atlas
                .as_ref()
                .and_then(|atlas| atlas.region_for(&sign_key_buf))
            else {
                continue;
            };
            let (center, normal) = sign_text_plane(sign);
            let right = [normal[2], 0.0, -normal[0]];
            let half_width = 15.0 / 32.0;
            let half_height = 5.0 / 24.0;
            let texel = 0.5 / super::entity::atlas::ENTITY_ATLAS_SIZE as f32;
            let (u_min, v_min) = (region.u_min + texel, region.v_min + texel);
            let (u_max, v_max) = (region.u_max - texel, region.v_max - texel);
            let base = self.entity_mesh_vertices.len() as u32;
            for (point, uv) in [
                (
                    [
                        center[0] - right[0] * half_width,
                        center[1] + half_height,
                        center[2] - right[2] * half_width,
                    ],
                    [u_min, v_min],
                ),
                (
                    [
                        center[0] + right[0] * half_width,
                        center[1] + half_height,
                        center[2] + right[2] * half_width,
                    ],
                    [u_max, v_min],
                ),
                (
                    [
                        center[0] + right[0] * half_width,
                        center[1] - half_height,
                        center[2] + right[2] * half_width,
                    ],
                    [u_max, v_max],
                ),
                (
                    [
                        center[0] - right[0] * half_width,
                        center[1] - half_height,
                        center[2] - right[2] * half_width,
                    ],
                    [u_min, v_max],
                ),
            ] {
                self.entity_mesh_vertices.push(EntityVertex {
                    position: point,
                    normal,
                    uv,
                    color: [1.0; 4],
                });
            }
            self.entity_mesh_indices.extend_from_slice(&[
                base,
                base + 2,
                base + 1,
                base,
                base + 3,
                base + 2,
            ]);
        }

        self.entity_index_count = self.entity_mesh_indices.len() as u32;
        if self.entity_index_count == 0 {
            return;
        }
        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.entity_vertex_buffer,
            &mut self.entity_vertex_alloc,
            &mut self.entity_vertex_capacity,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            bytemuck::cast_slice(&self.entity_mesh_vertices),
        );
        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.entity_index_buffer,
            &mut self.entity_index_alloc,
            &mut self.entity_index_capacity,
            vk::BufferUsageFlags::INDEX_BUFFER,
            bytemuck::cast_slice(&self.entity_mesh_indices),
        );
    }

    /// Hash all state written into the combined entity buffers.
    fn combined_entity_mesh_hash(&self) -> u64 {
        use std::hash::Hasher;

        let mut h = fnv::FnvHasher::default();
        h.write_u32(self.state.settings.entity_shadows() as u32);
        h.write_usize(self.state.hud.entity_billboards().len());
        for billboard in self.state.hud.entity_billboards() {
            h.write_u64(super::entity_mesh_state_hash(billboard));
            // Dropped item meshes bake their bob/rotation from wall-clock time.
            // Keep their animation correct; static entity batches still reuse.
            if billboard.kind == super::EntityBillboardKind::Item {
                let frame_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                h.write_u64(frame_time);
            }
        }
        h.write_usize(self.state.hud.skull_entries().len());
        for skull in self.state.hud.skull_entries() {
            h.write_i32(skull.position[0]);
            h.write_i32(skull.position[1]);
            h.write_i32(skull.position[2]);
            h.write_u8(skull.block_metadata);
            h.write_u8(skull.skull_type);
            h.write_u8(skull.rotation);
            h.write(skull.skin_key.as_bytes());
        }
        h.write_usize(self.state.hud.sign_entries().len());
        for sign in self.state.hud.sign_entries() {
            h.write_i32(sign.position[0]);
            h.write_i32(sign.position[1]);
            h.write_i32(sign.position[2]);
            h.write_u8(sign.wall_mounted as u8);
            h.write_u8(sign.metadata);
            for line in &sign.lines {
                h.write(line.as_bytes());
            }
        }
        h.finish()
    }

    /// Generate and upload 3D entity meshes from the billboard list.
    fn upload_entity_meshes(&mut self, frustum: &crate::client::player::Frustum) {
        use super::entity::mesh::{generate_entity_mesh, AnimationFamily, EntityVertex};
        use super::entity::models::{atlas_name_for_entity, model_for_entity};
        use nalgebra::Point3;

        let skin_started = std::time::Instant::now();
        self.sync_player_skin_atlas();
        self.state.frame_profile.set_entity_skin_sync_us(skin_started.elapsed().as_micros() as u64);

        // Billboard/UI state may change without affecting geometry.  Hash only
        // the combined mesh input, then reuse the existing GPU buffers when it
        // matches.  This avoids re-copying every cached entity mesh.
        let mesh_hash = self.combined_entity_mesh_hash();
        if mesh_hash == self.entity_state_hash
            && self.state.hud.entity_billboards().is_empty() == (self.entity_index_count == 0)
            && self.entity_vertex_buffer.is_some()
        {
            self.state.frame_profile.set_entity_batch_reused(true);
            return;
        }
        self.state.frame_profile.set_entity_batch_reused(false);
        self.entity_state_hash = mesh_hash;

        self.entity_index_count = 0;
        self.entity_held_block_index_count = 0;
        self.entity_held_item_index_count = 0;

        self.entity_mesh_vertices.clear();
        self.entity_mesh_indices.clear();
        self.entity_held_block_vertices.clear();
        self.entity_held_block_indices.clear();
        self.entity_held_item_vertices.clear();
        self.entity_held_item_indices.clear();

        let all_vertices = &mut self.entity_mesh_vertices;
        let all_indices = &mut self.entity_mesh_indices;
        let held_block_vertices = &mut self.entity_held_block_vertices;
        let held_block_indices = &mut self.entity_held_block_indices;
        let held_item_vertices = &mut self.entity_held_item_vertices;
        let held_item_indices = &mut self.entity_held_item_indices;

        let t_loop = std::time::Instant::now();
        let profile_entity_loop = cfg!(debug_assertions);
        let mut hash_us = 0;
        let mut lookup_us = 0;
        let mut append_us = 0;
        self.state.frame_profile.set_entity_culled_count(0);
        let active_entity_ids: fnv::FnvHashMap<i32, ()> = self.state.hud.entity_billboards().iter()
            .map(|billboard| (billboard.entity_id, ()))
            .collect();
        self.entity_mesh_cache
            .retain(|entity_id, _| active_entity_ids.contains_key(entity_id));
        // Get atlas reference for UV remapping
        let atlas = self.entity_atlas.as_ref();

        for billboard in self.state.hud.entity_billboards() {
            let (w, h) = billboard.entity_type.bounding_box();
            let half = w.max(h) * 0.5;
            let pos = billboard.position;
            if !frustum.test_aabb(
                [pos[0] - half, pos[1], pos[2] - half],
                [pos[0] + half, pos[1] + h, pos[2] + half],
            ) {
                self.state.frame_profile.inc_entity_culled_count();
                continue;
            }
            self.state.frame_profile.inc_entity_visible_count();

            if billboard.kind == super::EntityBillboardKind::Item {
                if let Some(item_id) = billboard.item_id {
                    let item_damage = billboard.item_damage.unwrap_or(0);
                    let time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f32();
                    let item_glint = billboard
                        .item_nbt
                        .as_deref()
                        .map(|nbt| {
                            crate::net::nbt::parse_root(nbt)
                                .ok()
                                .and_then(|root| {
                                    root.as_compound().map(|c| {
                                        c.get("ench")
                                            .and_then(|t| t.as_list())
                                            .is_some_and(|l| !l.is_empty())
                                            || c.get("StoredEnchantments")
                                                .and_then(|t| t.as_list())
                                                .is_some_and(|l| !l.is_empty())
                                    })
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    let (op_verts, op_idx, tr_verts, tr_idx, uses_item_atlas) =
                        crate::render::hud::hand::generate_dropped_item_mesh(
                            item_id,
                            item_damage,
                            pos,
                            billboard.age_ticks,
                            billboard.hover_start,
                            time,
                            item_glint,
                        );
                    if uses_item_atlas {
                        append_world_mesh(held_item_vertices, held_item_indices, op_verts, op_idx);
                        append_world_mesh(held_item_vertices, held_item_indices, tr_verts, tr_idx);
                    } else {
                        append_world_mesh(
                            held_block_vertices,
                            held_block_indices,
                            op_verts,
                            op_idx,
                        );
                        append_world_mesh(
                            held_block_vertices,
                            held_block_indices,
                            tr_verts,
                            tr_idx,
                        );
                    }
                    continue;
                }
            }

            if billboard.entity_type == crate::entity::EntityType::Arrow {
                let (op_verts, op_idx, tr_verts, tr_idx, uses_item_atlas) =
                    crate::render::hud::hand::generate_arrow_entity_mesh(
                        pos,
                        billboard.yaw,
                        billboard.pitch,
                    );
                if uses_item_atlas {
                    append_world_mesh(held_item_vertices, held_item_indices, op_verts, op_idx);
                    append_world_mesh(held_item_vertices, held_item_indices, tr_verts, tr_idx);
                } else {
                    append_world_mesh(held_block_vertices, held_block_indices, op_verts, op_idx);
                    append_world_mesh(held_block_vertices, held_block_indices, tr_verts, tr_idx);
                }
                continue;
            }

            // Per-entity state hash for caching.
            let hash_start = profile_entity_loop.then(std::time::Instant::now);
            let entity_hash = super::entity_mesh_state_hash(billboard);
            if let Some(start) = hash_start {
                hash_us += start.elapsed().as_micros() as u64;
            }

            let lookup_start = profile_entity_loop.then(std::time::Instant::now);
            if let Some((cached_hash, cached_verts, cached_idx)) =
                self.entity_mesh_cache.get(&billboard.entity_id)
            {
                if let Some(start) = lookup_start {
                    lookup_us += start.elapsed().as_micros() as u64;
                }
                if *cached_hash == entity_hash {
                    self.state.frame_profile.inc_entity_cache_hits();
                    let append_start = profile_entity_loop.then(std::time::Instant::now);
                    let base_idx = all_vertices.len() as u32;
                    all_vertices.reserve(cached_verts.len());
                    all_indices.reserve(cached_idx.len());
                    all_vertices.extend_from_slice(cached_verts);
                    for &index in cached_idx {
                        all_indices.push(base_idx + index);
                    }
                    if let Some(start) = append_start {
                        append_us += start.elapsed().as_micros() as u64;
                    }
                    continue;
                }
            } else if let Some(start) = lookup_start {
                lookup_us += start.elapsed().as_micros() as u64;
            }
            let cuboids = model_for_entity(
                billboard.entity_type,
                billboard.visual,
                billboard.slim,
                billboard.skin_parts_mask,
                billboard.has_cape,
            );
            let world_pos = Point3::new(pos[0], pos[1], pos[2]);

            let pose = EntityPose {
                model_scale: match billboard.entity_type {
                    crate::entity::EntityType::Giant => 6.0,
                    crate::entity::EntityType::CaveSpider => 0.7,
                    crate::entity::EntityType::Guardian if billboard.visual.guardian_elder => 2.35,
                    crate::entity::EntityType::ArmorStand
                        if billboard.visual.armor_stand_flags & 0x01 != 0 =>
                    {
                        0.5
                    }
                    _ => 1.0,
                },
                animation_family: match billboard.entity_type {
                    crate::entity::EntityType::Player
                    | crate::entity::EntityType::Zombie
                    | crate::entity::EntityType::PigZombie
                    | crate::entity::EntityType::Giant
                    | crate::entity::EntityType::Skeleton
                    | crate::entity::EntityType::Enderman
                    | crate::entity::EntityType::Witch
                    | crate::entity::EntityType::Villager => AnimationFamily::Biped,
                    crate::entity::EntityType::ArmorStand => AnimationFamily::ArmorStand,
                    crate::entity::EntityType::Pig
                    | crate::entity::EntityType::Sheep
                    | crate::entity::EntityType::Cow
                    | crate::entity::EntityType::Mooshroom
                    | crate::entity::EntityType::Wolf
                    | crate::entity::EntityType::Ocelot
                    | crate::entity::EntityType::Horse => AnimationFamily::Quadruped,
                    _ => AnimationFamily::Generic,
                },
                body_yaw: (180.0 - billboard.yaw).to_radians(),
                head_yaw: (billboard.yaw - billboard.head_yaw).to_radians(),
                pitch: billboard.pitch.to_radians(),
                limb_swing: billboard.limb_swing,
                limb_swing_amount: billboard.limb_swing_amount,
                swing_progress: billboard.swing_alpha,
                death_progress: billboard.death_alpha,
                age_ticks: billboard.age_ticks,
                holding_item: billboard.equipment[0]
                    .or_else(|| billboard.held_item.map(|id| (id, 0)))
                    .is_some_and(|(id, _)| id != 0),
                armor_stand_rotations: billboard.visual.armor_stand_rotations,
                sneaking: billboard.sneaking,
                riding: billboard.riding,
                blocking: billboard.blocking,
                cape_rotation: billboard.cape_rotation,
            };

            let base_idx = all_vertices.len() as u32;
            let (mut verts, idxs) = generate_entity_mesh(&cuboids, world_pos, &pose);

            // Remap UVs from mob-local (0.0–1.0) to atlas coordinates
            if let Some(atlas) = atlas {
                let atlas_name = billboard.skin_key.as_deref().unwrap_or_else(|| {
                    atlas_name_for_entity(billboard.entity_type, billboard.visual)
                });
                let region = atlas
                    .region_for(atlas_name)
                    .or_else(|| {
                        (billboard.entity_type == crate::entity::EntityType::Player)
                            .then(|| atlas.region_for("player"))
                            .flatten()
                    })
                    .or_else(|| atlas.region_for("__white"));
                if let Some(region) = region {
                    for v in &mut verts {
                        let (au, av) = region.local_to_atlas(v.uv[0], v.uv[1]);
                        v.uv = [au, av];
                    }
                }
            }

            // Apply hurt tint
            if billboard.hurt_alpha > 0.0 {
                for v in &mut verts {
                    v.color[0] =
                        v.color[0] * (1.0 - billboard.hurt_alpha) + 1.0 * billboard.hurt_alpha;
                    v.color[1] *= 1.0 - billboard.hurt_alpha;
                    v.color[2] *= 1.0 - billboard.hurt_alpha;
                }
            }

            // Cache the generated mesh for this entity — move ownership into the
            // cache and read it back by reference to avoid cloning the
            // vertex/index buffers on every cache miss.
            self.entity_mesh_cache
                .insert(billboard.entity_id, (entity_hash, verts, idxs));
            self.state.frame_profile.inc_entity_cache_misses();
            let cached = self
                .entity_mesh_cache
                .get(&billboard.entity_id)
                .expect("entity mesh just inserted");

            // Append to output buffers
            all_vertices.extend_from_slice(&cached.1);
            all_indices.extend(cached.2.iter().map(|i| base_idx + i));

            let supports_held_item = matches!(
                billboard.entity_type,
                crate::entity::EntityType::Player
                    | crate::entity::EntityType::Zombie
                    | crate::entity::EntityType::PigZombie
                    | crate::entity::EntityType::Skeleton
                    | crate::entity::EntityType::Witch
                    | crate::entity::EntityType::Giant
                    | crate::entity::EntityType::ArmorStand
            );
            if supports_held_item {
                if let Some((item_id, item_damage)) = billboard.equipment[0]
                    .or_else(|| billboard.held_item.map(|id| (id, 0)))
                    .filter(|(id, _)| *id != 0)
                {
                    let (op_verts, op_idx, tr_verts, tr_idx, uses_item_atlas) =
                        crate::render::hud::hand::generate_entity_held_item_mesh(
                            billboard.entity_type,
                            item_id,
                            item_damage,
                            pos,
                            &pose,
                            &cuboids,
                        );
                    if uses_item_atlas {
                        append_world_mesh(held_item_vertices, held_item_indices, op_verts, op_idx);
                        append_world_mesh(held_item_vertices, held_item_indices, tr_verts, tr_idx);
                    } else {
                        append_world_mesh(
                            held_block_vertices,
                            held_block_indices,
                            op_verts,
                            op_idx,
                        );
                        append_world_mesh(
                            held_block_vertices,
                            held_block_indices,
                            tr_verts,
                            tr_idx,
                        );
                    }
                }
            }
        }

        if let Some(atlas) = atlas {
            for skull in self.state.hud.skull_entries() {
                let [x, y, z] = skull.position;
                let (min, max) = skull_bounds(skull);
                if !frustum.test_aabb(
                    [x as f32 + min[0], y as f32 + min[1], z as f32 + min[2]],
                    [x as f32 + max[0], y as f32 + max[1], z as f32 + max[2]],
                ) {
                    continue;
                }
                if let Some(region) = atlas.region_for(&skull.skin_key) {
                    append_player_skull_mesh(all_vertices, all_indices, skull, region);
                }
            }
        }

        // Generate shadow quads under entities
        if self.state.settings.entity_shadows() {
            for billboard in self.state.hud.entity_billboards() {
                if !billboard.entity_type.is_mob()
                    && billboard.entity_type != crate::entity::EntityType::Item
                    && billboard.entity_type != crate::entity::EntityType::XPOrb
                {
                    continue;
                }
                let (bw, _bh) = billboard.entity_type.bounding_box();
                let shadow_size = bw * 0.75;
                let shadow_y = 0.01; // Slightly above ground to avoid z-fighting
                let pos = billboard.position;
                let shadow_color = [0.0, 0.0, 0.0, 0.35];

                let v_start = all_vertices.len() as u32;
                // Shadow quad on XZ plane at y=shadow_y
                all_vertices.push(EntityVertex {
                    position: [pos[0] - shadow_size, shadow_y, pos[2] - shadow_size],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                    color: shadow_color,
                });
                all_vertices.push(EntityVertex {
                    position: [pos[0] + shadow_size, shadow_y, pos[2] - shadow_size],
                    normal: [0.0, 1.0, 0.0],
                    uv: [1.0, 0.0],
                    color: shadow_color,
                });
                all_vertices.push(EntityVertex {
                    position: [pos[0] + shadow_size, shadow_y, pos[2] + shadow_size],
                    normal: [0.0, 1.0, 0.0],
                    uv: [1.0, 1.0],
                    color: shadow_color,
                });
                all_vertices.push(EntityVertex {
                    position: [pos[0] - shadow_size, shadow_y, pos[2] + shadow_size],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 1.0],
                    color: shadow_color,
                });
                all_indices.extend_from_slice(&[
                    v_start,
                    v_start + 1,
                    v_start + 2,
                    v_start,
                    v_start + 2,
                    v_start + 3,
                ]);
            }
        }

        // Reusable sign key buffer — avoids per-frame `format!` allocations.
        use std::fmt::Write as _;
        let mut sign_key_buf = String::with_capacity(24);
        // Sign text — generate texture plates and pack into entity atlas
        if !self.state.hud.sign_entries().is_empty() {
            if let Some(ref mut atlas) = self.entity_atlas {
                let mut atlas_changed = false;
                for sign in self.state.hud.sign_entries() {
                    let pos = sign.position;
                    sign_key_buf.clear();
                    let _ = write!(sign_key_buf, "sign_{}", sign_position_key_id(pos));
                    // pack_sign_text replaces an existing region in-place, so text
                    // updates are visible without changing the mesh UVs.
                    let tex = build_sign_texture(&mut self.font, &sign.lines);
                    atlas_changed |= atlas
                        .pack_sign_text(&sign_key_buf, &tex.0, tex.1, tex.2)
                        .is_some();
                }
                if atlas_changed {
                    self.entity_atlas_full_upload_pending = true;
                }
            }
        }

        // Sign text meshes — world-space quads placed on the board face using
        // the same block metadata transforms as TileEntitySignRenderer.
        if let Some(camera) = self.current_camera.as_ref() {
            for sign in self.state.hud.sign_entries() {
                let pos = sign.position;
                let dx = pos[0] as f32 + 0.5 - camera.position.x;
                let dy = pos[1] as f32 + 0.5 - camera.position.y;
                let dz = pos[2] as f32 + 0.5 - camera.position.z;
                if dx * dx + dy * dy + dz * dz > 4096.0 {
                    continue;
                } // Vanilla tile entities use a 64-block render distance.

                sign_key_buf.clear();
                let _ = write!(sign_key_buf, "sign_{}", sign_position_key_id(pos));
                if let Some(region) = self
                    .entity_atlas
                    .as_ref()
                    .and_then(|a| a.region_for(&sign_key_buf))
                {
                    let (center, normal) = sign_text_plane(sign);
                    let right = [normal[2], 0.0, -normal[0]];
                    // Vanilla uses a 90px wide text area at a scale of 1/96,
                    // yielding a 15/16 × 5/12 plate on the 1 × 1/2 board.
                    let hw = 15.0 / 32.0;
                    let hh = 5.0 / 24.0;
                    let bg = all_vertices.len() as u32;
                    // Texture coordinates on an atlas border can sample the
                    // adjacent region at an interpolated edge pixel.  Keep the
                    // sign plate half a texel inside its allocated cell.
                    let texel = 0.5 / crate::render::entity::atlas::ENTITY_ATLAS_SIZE as f32;
                    let (u0, v0) = (region.u_min + texel, region.v_min + texel);
                    let (u1, v1) = (region.u_max - texel, region.v_max - texel);
                    let n = normal;
                    all_vertices.push(EntityVertex {
                        position: [
                            center[0] - right[0] * hw,
                            center[1] + hh,
                            center[2] - right[2] * hw,
                        ],
                        normal: n,
                        uv: [u0, v0],
                        color: [1.0; 4],
                    });
                    all_vertices.push(EntityVertex {
                        position: [
                            center[0] + right[0] * hw,
                            center[1] + hh,
                            center[2] + right[2] * hw,
                        ],
                        normal: n,
                        uv: [u1, v0],
                        color: [1.0; 4],
                    });
                    all_vertices.push(EntityVertex {
                        position: [
                            center[0] + right[0] * hw,
                            center[1] - hh,
                            center[2] + right[2] * hw,
                        ],
                        normal: n,
                        uv: [u1, v1],
                        color: [1.0; 4],
                    });
                    all_vertices.push(EntityVertex {
                        position: [
                            center[0] - right[0] * hw,
                            center[1] - hh,
                            center[2] - right[2] * hw,
                        ],
                        normal: n,
                        uv: [u0, v1],
                        color: [1.0; 4],
                    });
                    all_indices.extend_from_slice(&[bg, bg + 2, bg + 1, bg, bg + 3, bg + 2]);
                }
            }
        }

        self.state.frame_profile.set_entity_loop_us(t_loop.elapsed().as_micros() as u64);
        self.state.frame_profile.set_entity_hash_us(hash_us);
        self.state.frame_profile.set_entity_lookup_us(lookup_us);
        self.state.frame_profile.set_entity_append_us(append_us);

        if all_vertices.is_empty() && held_block_indices.is_empty() && held_item_indices.is_empty()
        {
            return;
        }

        let t_upload = std::time::Instant::now();
        if !all_indices.is_empty() {
            super::resources::upload_dynamic_buffer(
                &self.device,
                self.resources.allocator_mut(),
                &mut self.entity_vertex_buffer,
                &mut self.entity_vertex_alloc,
                &mut self.entity_vertex_capacity,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                bytemuck::cast_slice(&all_vertices),
            );
            super::resources::upload_dynamic_buffer(
                &self.device,
                self.resources.allocator_mut(),
                &mut self.entity_index_buffer,
                &mut self.entity_index_alloc,
                &mut self.entity_index_capacity,
                vk::BufferUsageFlags::INDEX_BUFFER,
                bytemuck::cast_slice(&all_indices),
            );
            self.entity_index_count = all_indices.len() as u32;
        }

        if !held_block_indices.is_empty() {
            super::resources::upload_dynamic_buffer(
                &self.device,
                self.resources.allocator_mut(),
                &mut self.entity_held_block_vertex_buffer,
                &mut self.entity_held_block_vertex_alloc,
                &mut self.entity_held_block_vertex_capacity,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                bytemuck::cast_slice(&held_block_vertices),
            );
            super::resources::upload_dynamic_buffer(
                &self.device,
                self.resources.allocator_mut(),
                &mut self.entity_held_block_index_buffer,
                &mut self.entity_held_block_index_alloc,
                &mut self.entity_held_block_index_capacity,
                vk::BufferUsageFlags::INDEX_BUFFER,
                bytemuck::cast_slice(&held_block_indices),
            );
            self.entity_held_block_index_count = held_block_indices.len() as u32;
        }
        if !held_item_indices.is_empty() {
            super::resources::upload_dynamic_buffer(
                &self.device,
                self.resources.allocator_mut(),
                &mut self.entity_held_item_vertex_buffer,
                &mut self.entity_held_item_vertex_alloc,
                &mut self.entity_held_item_vertex_capacity,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                bytemuck::cast_slice(&held_item_vertices),
            );
            super::resources::upload_dynamic_buffer(
                &self.device,
                self.resources.allocator_mut(),
                &mut self.entity_held_item_index_buffer,
                &mut self.entity_held_item_index_alloc,
                &mut self.entity_held_item_index_capacity,
                vk::BufferUsageFlags::INDEX_BUFFER,
                bytemuck::cast_slice(&held_item_indices),
            );
            self.entity_held_item_index_count = held_item_indices.len() as u32;
        }
        self.state.frame_profile.set_entity_upload_us(t_upload.elapsed().as_micros() as u64);
    }

    /// Build and upload the 3D particle mesh from the current particle_list.
    fn upload_particle_mesh(&mut self, camera: &crate::client::player::Camera) {
        use std::hash::Hasher;

        let mut hasher = fnv::FnvHasher::default();
        hasher.write_u64(self.particle_generation);
        for value in [
            camera.right.x,
            camera.right.y,
            camera.right.z,
            camera.up.x,
            camera.up.y,
            camera.up.z,
            camera.front.x,
            camera.front.y,
            camera.front.z,
        ] {
            hasher.write_i32((value * 10_000.0) as i32);
        }
        let mesh_hash = hasher.finish();
        if mesh_hash == self.particle_mesh_hash
            && (self.particle_list.is_empty() || self.particle_vertex_buffer.is_some())
        {
            self.state.frame_profile.set_particle_batch_reused(true);
            return;
        }
        self.particle_mesh_hash = mesh_hash;
        self.state.frame_profile.set_particle_batch_reused(false);
        self.particle_index_count = 0;

        if self.particle_list.is_empty() {
            return;
        }

        let view_front = if camera.reverse_view {
            -camera.front
        } else {
            camera.front
        };
        let (vertices, indices) = super::particle_mesh::build_particle_mesh(
            &self.particle_list,
            camera.right,
            camera.up,
            view_front,
            crate::assets::texture::tex_idx,
        );

        if indices.is_empty() {
            return;
        }

        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.particle_vertex_buffer,
            &mut self.particle_vertex_alloc,
            &mut self.particle_vertex_capacity,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            bytemuck::cast_slice(&vertices),
        );
        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.particle_index_buffer,
            &mut self.particle_index_alloc,
            &mut self.particle_index_capacity,
            vk::BufferUsageFlags::INDEX_BUFFER,
            bytemuck::cast_slice(&indices),
        );
        self.particle_index_count = indices.len() as u32;
    }

    /// Font size used when rasterising nametag text into the entity atlas.
    const NAMETAG_FONT_SIZE: f32 = 14.0;
    /// World-space scale applied to the nametag billboard quad.
    const NAMETAG_SCALE: f32 = 0.01333333;
    /// Maximum nametag visibility distance (blocks) when the entity is sneaking.
    const NAMETAG_SNEAK_VISIBILITY: f32 = 32.0;
    /// Maximum nametag visibility distance (blocks) when the entity is standing.
    const NAMETAG_NORMAL_VISIBILITY: f32 = 64.0;
    /// Vertical offset (blocks) above the entity's bounding-box top.
    const NAMETAG_HEIGHT_OFFSET: f32 = 0.5;

    /// Build and upload nametag billboard quads for entities with visible names.
    fn upload_nametag_mesh(&mut self, camera: &crate::client::player::Camera) {
        use super::entity::mesh::EntityVertex;
        use std::hash::Hasher;

        self.nametag_index_count = 0;

        if self.state.hud.entity_billboards().is_empty() {
            return;
        }

        let mut active_names: Vec<String> = self.state.hud.entity_billboards().iter()
            .filter(|entity| entity.name_visible)
            .filter_map(|entity| entity.name.clone())
            .collect();
        active_names.sort_unstable();
        active_names.dedup();
        let mut name_hasher = fnv::FnvHasher::default();
        name_hasher.write_usize(active_names.len());
        for name in &active_names {
            name_hasher.write(name.as_bytes());
            name_hasher.write_u8(0);
        }
        let name_hash = name_hasher.finish();

        // Reusable nametag key buffer — avoids per-frame `format!` allocations.
        use std::fmt::Write as _;
        let mut nametag_key_buf = String::with_capacity(48);

        if name_hash != self.nametag_text_hash {
            self.nametag_text_hash = name_hash;
            if let Some(atlas) = self.entity_atlas.as_mut() {
                atlas.clear_nametag_texts();
            }
            let mut atlas_modified = false;
            for name in &active_names {
                let font_size = Self::NAMETAG_FONT_SIZE;
                nametag_key_buf.clear();
                let _ = write!(nametag_key_buf, "nametag_{}", name);
                let texture = build_nametag_texture(&mut self.font, name, font_size);
                if self
                    .entity_atlas
                    .as_mut()
                    .and_then(|atlas| {
                        atlas.pack_nametag_text(&nametag_key_buf, &texture.0, texture.1, texture.2)
                    })
                    .is_some()
                {
                    atlas_modified = true;
                }
            }
            if atlas_modified {
                self.entity_atlas_full_upload_pending = true;
            }
        }

        let mut vertices: Vec<EntityVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for entity in self.state.hud.entity_billboards() {
            if !entity.name_visible {
                continue;
            }
            let name = match &entity.name {
                Some(n) => n,
                None => continue,
            };
            let max_dist = if entity.sneaking { Self::NAMETAG_SNEAK_VISIBILITY } else { Self::NAMETAG_NORMAL_VISIBILITY };
            let dx = camera.position.x - entity.position[0];
            let dy = camera.position.y - (entity.position[1] + entity.height * 0.5);
            let dz = camera.position.z - entity.position[2];
            let dist_sq = dx * dx + dy * dy + dz * dz;
            if dist_sq > max_dist * max_dist {
                continue;
            }

            let center = [
                entity.position[0],
                entity.position[1] + entity.height + Self::NAMETAG_HEIGHT_OFFSET,
                entity.position[2],
            ];

            let font_size = Self::NAMETAG_FONT_SIZE;
            nametag_key_buf.clear();
            let _ = write!(nametag_key_buf, "nametag_{}", name);

            let region = self
                .entity_atlas
                .as_ref()
                .and_then(|atlas| atlas.region_for(&nametag_key_buf));

            let Some(region) = region else {
                continue;
            };

            let scale = Self::NAMETAG_SCALE;
            let half_w = region.tex_width as f32 * 0.5 * scale;
            let half_h = region.tex_height as f32 * 0.5 * scale;

            let right = camera.right;
            let up = camera.up;

            let normal = [0.0, 0.0, 0.0]; // emissive — no face lighting
            let color = [1.0, 1.0, 1.0, 1.0];

            let base = vertices.len() as u32;

            // Bottom-left
            vertices.push(EntityVertex {
                position: [
                    center[0] + right.x * (-half_w) + up.x * (-half_h),
                    center[1] + right.y * (-half_w) + up.y * (-half_h),
                    center[2] + right.z * (-half_w) + up.z * (-half_h),
                ],
                normal,
                uv: [region.u_min, region.v_max],
                color,
            });
            // Bottom-right
            vertices.push(EntityVertex {
                position: [
                    center[0] + right.x * half_w + up.x * (-half_h),
                    center[1] + right.y * half_w + up.y * (-half_h),
                    center[2] + right.z * half_w + up.z * (-half_h),
                ],
                normal,
                uv: [region.u_max, region.v_max],
                color,
            });
            // Top-right
            vertices.push(EntityVertex {
                position: [
                    center[0] + right.x * half_w + up.x * half_h,
                    center[1] + right.y * half_w + up.y * half_h,
                    center[2] + right.z * half_w + up.z * half_h,
                ],
                normal,
                uv: [region.u_max, region.v_min],
                color,
            });
            // Top-left
            vertices.push(EntityVertex {
                position: [
                    center[0] + right.x * (-half_w) + up.x * half_h,
                    center[1] + right.y * (-half_w) + up.y * half_h,
                    center[2] + right.z * (-half_w) + up.z * half_h,
                ],
                normal,
                uv: [region.u_min, region.v_min],
                color,
            });

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        if vertices.is_empty() {
            return;
        }

        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.nametag_vertex_buffer,
            &mut self.nametag_vertex_alloc,
            &mut self.nametag_vertex_capacity,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            bytemuck::cast_slice(&vertices),
        );
        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.nametag_index_buffer,
            &mut self.nametag_index_alloc,
            &mut self.nametag_index_capacity,
            vk::BufferUsageFlags::INDEX_BUFFER,
            bytemuck::cast_slice(&indices),
        );
        self.nametag_index_count = indices.len() as u32;
    }

    /// Generate and upload local player meshes (1st person hand or 3rd person full body).
    fn upload_local_player_meshes(&mut self, camera: &crate::client::player::Camera) {
        use std::hash::Hasher;

        // Vanilla renders the arm-tracking rotation between game ticks. Using
        // the current tick value directly makes the hand visibly step at 20 Hz,
        // especially while airborne camera pitch is changing.
        let partial_tick = camera.partial_tick.clamp(0.0, 1.0);
        let arm_pitch = self.state.hud.first_person_prev_arm_pitch()
            + crate::util::wrap_degrees(
                self.state.hud.first_person_arm_pitch() - self.state.hud.first_person_prev_arm_pitch(),
            ) * partial_tick;
        let arm_yaw = self.state.hud.first_person_prev_arm_yaw()
            + crate::util::wrap_degrees(
                self.state.hud.first_person_arm_yaw() - self.state.hud.first_person_prev_arm_yaw(),
            ) * partial_tick;

        let mut hasher = fnv::FnvHasher::default();
        hasher.write_u8(self.state.settings.camera_mode());
        hasher.write_u32(self.state.hud.chat_open() as u32);
        hasher.write_u32(self.state.inventory.inventory_open() as u32);
        hasher.write_u32(self.state.hud.health().to_bits());
        hasher.write_u16(self.state.hud.hand_item_id());
        hasher.write_u16(self.state.hud.hand_item_damage());
        hasher.write_u32(self.state.hud.hand_swing_progress().to_bits());
        hasher.write_u32(self.hand_equip_progress.to_bits());
        hasher.write_u8(self.state.hud.hand_use_kind());
        hasher.write_u32(self.state.hud.hand_use_progress().to_bits());
        // First-person geometry is baked into dynamic vertex buffers. Lua can
        // change these matrices/flags without changing any vanilla hand state;
        // omitting them here lets an older mesh survive after a script callback
        // has produced the new pose.
        for transform in [
            &self.state.hud.first_person_arm_transform(),
            &self.state.hud.first_person_item_transform(),
        ] {
            for value in transform.iter() {
                hasher.write_u32(value.to_bits());
            }
        }
        let flags = &self.state.hud.fp_vanilla_flags();
        for enabled in [
            flags.base,
            flags.equip,
            flags.swing,
            flags.use_transform,
            flags.block_transform,
            flags.bow_transform,
            flags.eat_drink_transform,
            flags.bob,
        ] {
            hasher.write_u8(u8::from(enabled));
        }
        hasher.write_u32(self.state.settings.local_skin_slim() as u32);
        hasher.write_u8(self.state.settings.skin_parts());
        for value in [
            camera.position.x,
            camera.position.y,
            camera.position.z,
            camera.yaw,
            camera.pitch,
            camera.fov,
            camera.aspect,
            camera.fov_modifier,
            camera.bob_phase,
            camera.bob_amount,
            camera.bob_pitch,
            camera.hurt_time,
            arm_pitch,
            arm_yaw,
        ] {
            hasher.write_u32(value.to_bits());
        }
        if let Some(billboard) = &self.state.hud.local_player_billboard() {
            hasher.write_u64(super::entity_mesh_state_hash(billboard));
        }
        let mesh_hash = hasher.finish();
        if self.local_mesh_hash == Some(mesh_hash) {
            self.state.frame_profile.set_local_batch_reused(true);
            return;
        }
        self.local_mesh_hash = Some(mesh_hash);
        self.state.frame_profile.set_local_batch_reused(false);

        self.fp_arm_index_count = 0;
        self.fp_block_op_index_count = 0;
        self.fp_block_tr_index_count = 0;

        if self.state.settings.camera_mode() == 0
            && (self.state.hud.chat_open() || self.state.inventory.inventory_open() || self.state.hud.health() <= 0.0)
        {
            return;
        }

        if self.state.settings.camera_mode() != 0 {
            // Third person uses the unified entity mesh path so skin, cape,
            // armor and held items can each bind their own atlas material.
            return;
        }

        if self.state.hud.hand_item_id() == 0 {
            // Render arm
            let pose = crate::render::first_person::FirstPersonPose {
                swing_progress: self.state.hud.hand_swing_progress(),
                equip_progress: 1.0 - self.hand_equip_progress,
                render_arm_pitch: arm_pitch,
                render_arm_yaw: arm_yaw,
                use_kind: self.state.hud.hand_use_kind(),
                use_progress: self.state.hud.hand_use_progress(),
                script_transform: self.state.hud.first_person_arm_transform(),
                vanilla_flags: self.state.hud.fp_vanilla_flags().clone(),
                glint: false,
            };
            let (arm_verts, arm_idx) =
                crate::render::entity::player_model::generate_first_person_arm_mesh(
                    camera,
                    self.state.settings.local_skin_slim(),
                    self.state.settings.skin_parts() & 0x08 != 0,
                    &pose,
                );

            if !arm_idx.is_empty() {
                self.fp_arm_index_count = arm_idx.len() as u32;
                super::resources::upload_dynamic_buffer(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut self.fp_arm_vertex_buffer,
                    &mut self.fp_arm_vertex_alloc,
                    &mut self.fp_arm_vertex_capacity,
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    bytemuck::cast_slice(&arm_verts),
                );

                super::resources::upload_dynamic_buffer(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut self.fp_arm_index_buffer,
                    &mut self.fp_arm_index_alloc,
                    &mut self.fp_arm_index_capacity,
                    vk::BufferUsageFlags::INDEX_BUFFER,
                    bytemuck::cast_slice(&arm_idx),
                );
            }
        } else {
            // Render held block
            let pose = crate::render::first_person::FirstPersonPose {
                swing_progress: self.state.hud.hand_swing_progress(),
                equip_progress: 1.0 - self.hand_equip_progress,
                render_arm_pitch: arm_pitch,
                render_arm_yaw: arm_yaw,
                use_kind: self.state.hud.hand_use_kind(),
                use_progress: self.state.hud.hand_use_progress(),
                script_transform: self.state.hud.first_person_item_transform(),
                vanilla_flags: self.state.hud.fp_vanilla_flags().clone(),
                glint: self.state.hud.hand_item_nbt().as_deref()
                    .map(|nbt| {
                        crate::net::nbt::parse_root(nbt)
                            .ok()
                            .and_then(|root| {
                                root.as_compound().map(|c| {
                                    c.get("ench")
                                        .and_then(|t| t.as_list())
                                        .is_some_and(|l| !l.is_empty())
                                        || c.get("StoredEnchantments")
                                            .and_then(|t| t.as_list())
                                            .is_some_and(|l| !l.is_empty())
                                })
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false),
            };
            let (op_verts, op_idx, tr_verts, tr_idx, uses_item_atlas) =
                crate::render::hud::hand::generate_held_block_mesh(
                    camera,
                    self.state.hud.hand_item_id(),
                    self.state.hud.hand_item_damage(),
                    &pose,
                );
            self.fp_block_uses_item_atlas = uses_item_atlas;

            if !op_idx.is_empty() {
                self.fp_block_op_index_count = op_idx.len() as u32;
                super::resources::upload_dynamic_buffer(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut self.fp_block_op_vertex_buffer,
                    &mut self.fp_block_op_vertex_alloc,
                    &mut self.fp_block_op_vertex_capacity,
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    bytemuck::cast_slice(&op_verts),
                );

                super::resources::upload_dynamic_buffer(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut self.fp_block_op_index_buffer,
                    &mut self.fp_block_op_index_alloc,
                    &mut self.fp_block_op_index_capacity,
                    vk::BufferUsageFlags::INDEX_BUFFER,
                    bytemuck::cast_slice(&op_idx),
                );
            }

            if !tr_idx.is_empty() {
                self.fp_block_tr_index_count = tr_idx.len() as u32;
                super::resources::upload_dynamic_buffer(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut self.fp_block_tr_vertex_buffer,
                    &mut self.fp_block_tr_vertex_alloc,
                    &mut self.fp_block_tr_vertex_capacity,
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    bytemuck::cast_slice(&tr_verts),
                );

                super::resources::upload_dynamic_buffer(
                    &self.device,
                    self.resources.allocator_mut(),
                    &mut self.fp_block_tr_index_buffer,
                    &mut self.fp_block_tr_index_alloc,
                    &mut self.fp_block_tr_index_capacity,
                    vk::BufferUsageFlags::INDEX_BUFFER,
                    bytemuck::cast_slice(&tr_idx),
                );
            }
        }
    }

    /// Generate and upload block selection wireframe + dig crack overlay as 3D geometry.
    /// Rendered in the entity pass with depth testing for proper occlusion.
    fn upload_block_selection(&mut self) {
        if !self.block_dirty && self.block_vertex_buffer.is_some() {
            return;
        }
        self.block_dirty = false;

        use super::entity::mesh::EntityVertex;

        self.block_index_count = 0;

        let mut all_vertices: Vec<EntityVertex> = Vec::new();
        let mut all_indices: Vec<u32> = Vec::new();

        // ── Selection wireframe (MC 1.8.9: color(0,0,0,0.4), lineWidth 2px) ──
        {
            let wire_color = [0.0, 0.0, 0.0, 0.4];
            let w = 0.01; // half-thickness in world units
            let inset = 0.002;
            let white_region = self
                .entity_atlas
                .as_ref()
                .and_then(|a| a.region_for("__white"))
                .copied();

            for selection in self.state.hud.block_selection_boxes() {
                let mn = [
                    selection.min[0] - inset,
                    selection.min[1] - inset,
                    selection.min[2] - inset,
                ];
                let mx = [
                    selection.max[0] + inset,
                    selection.max[1] + inset,
                    selection.max[2] + inset,
                ];

                let emit_edge = |verts: &mut Vec<EntityVertex>,
                                 idxs: &mut Vec<u32>,
                                 a: [f32; 3],
                                 b: [f32; 3],
                                 by: usize,
                                 bz: usize| {
                    let s = verts.len() as u32;
                    let mut p0 = a;
                    let mut p1 = a;
                    let mut p2 = b;
                    let mut p3 = b;
                    p0[by] -= w;
                    p0[bz] -= w;
                    p1[by] += w;
                    p1[bz] -= w;
                    p2[by] += w;
                    p2[bz] += w;
                    p3[by] -= w;
                    p3[bz] += w;
                    let n = [0.0f32; 3];
                    let uv_white = if let Some(r) = white_region {
                        let (u, v) = r.local_to_atlas(0.5, 0.5);
                        [u, v]
                    } else {
                        [0.0, 0.0]
                    };
                    verts.push(EntityVertex {
                        position: p0,
                        normal: n,
                        uv: uv_white,
                        color: wire_color,
                    });
                    verts.push(EntityVertex {
                        position: p1,
                        normal: n,
                        uv: uv_white,
                        color: wire_color,
                    });
                    verts.push(EntityVertex {
                        position: p2,
                        normal: n,
                        uv: uv_white,
                        color: wire_color,
                    });
                    verts.push(EntityVertex {
                        position: p3,
                        normal: n,
                        uv: uv_white,
                        color: wire_color,
                    });
                    idxs.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
                };

                // Bottom face edges (y = min)
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mn[0], mn[1], mn[2]],
                    [mx[0], mn[1], mn[2]],
                    1,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mx[0], mn[1], mn[2]],
                    [mx[0], mn[1], mx[2]],
                    1,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mx[0], mn[1], mx[2]],
                    [mn[0], mn[1], mx[2]],
                    1,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mn[0], mn[1], mx[2]],
                    [mn[0], mn[1], mn[2]],
                    1,
                    2,
                );
                // Top face edges (y = max)
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mn[0], mx[1], mn[2]],
                    [mx[0], mx[1], mn[2]],
                    1,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mx[0], mx[1], mn[2]],
                    [mx[0], mx[1], mx[2]],
                    1,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mx[0], mx[1], mx[2]],
                    [mn[0], mx[1], mx[2]],
                    1,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mn[0], mx[1], mx[2]],
                    [mn[0], mx[1], mn[2]],
                    1,
                    2,
                );
                // Vertical edges
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mn[0], mn[1], mn[2]],
                    [mn[0], mx[1], mn[2]],
                    0,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mx[0], mn[1], mn[2]],
                    [mx[0], mx[1], mn[2]],
                    0,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mx[0], mn[1], mx[2]],
                    [mx[0], mx[1], mx[2]],
                    0,
                    2,
                );
                emit_edge(
                    &mut all_vertices,
                    &mut all_indices,
                    [mn[0], mn[1], mx[2]],
                    [mn[0], mx[1], mx[2]],
                    0,
                    2,
                );
            }
        }

        // ── Dig crack overlay (vanilla destroy_stage_0..9) ──
        if let (Some(pos), Some(atlas)) = (self.state.hud.dig_position(), self.entity_atlas.as_ref()) {
            let progress = self.state.hud.dig_progress().clamp(0.0, 1.0);
            if progress > 0.0 {
                let stage = (progress * 10.0).floor().min(9.0) as u32;
                // Destroy stages are a fixed 0..9 set — index a static table
                // instead of formatting the stage number every frame.
                const DESTROY_STAGE_KEYS: [&str; 10] = [
                    "destroy_0", "destroy_1", "destroy_2", "destroy_3", "destroy_4",
                    "destroy_5", "destroy_6", "destroy_7", "destroy_8", "destroy_9",
                ];
                let atlas_key = DESTROY_STAGE_KEYS[stage as usize];
                if let Some(region) = atlas.region_for(atlas_key) {
                    // Selection boxes are already in world coordinates.  Apply the
                    // crack to every cuboid so stairs, fences and multipart blocks
                    // receive the same destroy overlay as their real geometry.
                    let fallback = SelectionBox {
                        min: [pos[0] as f32, pos[1] as f32, pos[2] as f32],
                        max: [
                            pos[0] as f32 + 1.0,
                            pos[1] as f32 + 1.0,
                            pos[2] as f32 + 1.0,
                        ],
                    };
                    // Iterate the selection boxes by reference instead of cloning
                    // the whole list every frame.
                    let selection_boxes = self.state.hud.block_selection_boxes();
                    let bounds_list: &[SelectionBox] = if selection_boxes.is_empty() {
                        std::slice::from_ref(&fallback)
                    } else {
                        selection_boxes.as_slice()
                    };
                    for bounds in bounds_list {
                        let bx = bounds.min[0];
                        let by = bounds.min[1];
                        let bz = bounds.min[2];
                        let sx = bounds.max[0] - bounds.min[0];
                        let sy = bounds.max[1] - bounds.min[1];
                        let sz = bounds.max[2] - bounds.min[2];

                        // Render slightly outside the block surface so the crack
                        // overlay passes the depth test.
                        let outset = 0.001;
                        let crack_color = [1.0, 1.0, 1.0, 0.6];

                        let emit_face = |verts: &mut Vec<EntityVertex>,
                                         idxs: &mut Vec<u32>,
                                         corners: [[f32; 3]; 4],
                                         face_w: f32,
                                         face_h: f32| {
                            let s = verts.len() as u32;
                            let uv = [[0.0, 0.0], [face_w, 0.0], [face_w, face_h], [0.0, face_h]];
                            for (i, corner) in corners.iter().enumerate() {
                                let (au, av) = region.local_to_atlas(uv[i][0], uv[i][1]);
                                verts.push(EntityVertex {
                                    position: *corner,
                                    normal: [0.0; 3],
                                    uv: [au, av],
                                    color: crack_color,
                                });
                            }
                            idxs.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
                        };

                        // South (+Z)
                        emit_face(
                            &mut all_vertices,
                            &mut all_indices,
                            [
                                [bx - outset, by - outset, bz + sz + outset],
                                [bx + sx + outset, by - outset, bz + sz + outset],
                                [bx + sx + outset, by + sy + outset, bz + sz + outset],
                                [bx - outset, by + sy + outset, bz + sz + outset],
                            ],
                            sx,
                            sy,
                        );
                        // North (-Z)
                        emit_face(
                            &mut all_vertices,
                            &mut all_indices,
                            [
                                [bx + sx + outset, by - outset, bz - outset],
                                [bx - outset, by - outset, bz - outset],
                                [bx - outset, by + sy + outset, bz - outset],
                                [bx + sx + outset, by + sy + outset, bz - outset],
                            ],
                            sx,
                            sy,
                        );
                        // Top (+Y)
                        emit_face(
                            &mut all_vertices,
                            &mut all_indices,
                            [
                                [bx - outset, by + sy + outset, bz - outset],
                                [bx - outset, by + sy + outset, bz + sz + outset],
                                [bx + sx + outset, by + sy + outset, bz + sz + outset],
                                [bx + sx + outset, by + sy + outset, bz - outset],
                            ],
                            sx,
                            sz,
                        );
                        // Bottom (-Y)
                        emit_face(
                            &mut all_vertices,
                            &mut all_indices,
                            [
                                [bx - outset, by - outset, bz + sz + outset],
                                [bx - outset, by - outset, bz - outset],
                                [bx + sx + outset, by - outset, bz - outset],
                                [bx + sx + outset, by - outset, bz + sz + outset],
                            ],
                            sx,
                            sz,
                        );
                        // East (+X)
                        emit_face(
                            &mut all_vertices,
                            &mut all_indices,
                            [
                                [bx + sx + outset, by - outset, bz - outset],
                                [bx + sx + outset, by - outset, bz + sz + outset],
                                [bx + sx + outset, by + sy + outset, bz + sz + outset],
                                [bx + sx + outset, by + sy + outset, bz - outset],
                            ],
                            sz,
                            sy,
                        );
                        // West (-X)
                        emit_face(
                            &mut all_vertices,
                            &mut all_indices,
                            [
                                [bx - outset, by - outset, bz + sz + outset],
                                [bx - outset, by - outset, bz - outset],
                                [bx - outset, by + sy + outset, bz - outset],
                                [bx - outset, by + sy + outset, bz + sz + outset],
                            ],
                            sz,
                            sy,
                        );
                    }
                }
            }
        }

        if all_vertices.is_empty() {
            return;
        }

        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.block_vertex_buffer,
            &mut self.block_vertex_alloc,
            &mut self.block_vertex_capacity,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            bytemuck::cast_slice(&all_vertices),
        );
        super::resources::upload_dynamic_buffer(
            &self.device,
            self.resources.allocator_mut(),
            &mut self.block_index_buffer,
            &mut self.block_index_alloc,
            &mut self.block_index_capacity,
            vk::BufferUsageFlags::INDEX_BUFFER,
            bytemuck::cast_slice(&all_indices),
        );
        self.block_index_count = all_indices.len() as u32;
    }
}

fn append_chest_entity_mesh(
    vertices: &mut Vec<crate::render::entity::mesh::EntityVertex>,
    indices: &mut Vec<u32>,
    chest: &crate::render::ChestRenderEntry,
    region: crate::render::entity::atlas::MobTextureRegion,
) {
    use nalgebra::{Matrix4, Rotation3, Vector3};

    let double = chest.double_x || chest.double_z;
    let source_center = if double { [1.0, 0.5] } else { [0.5, 0.5] };
    let target_center = if chest.double_z {
        [0.5, 1.0]
    } else {
        source_center
    };
    let angle = match chest.metadata & 7 {
        2 => 180.0_f32,
        4 => -90.0,
        5 => 90.0,
        _ => 0.0,
    };
    let [x, y, z] = chest.position;
    let base = Matrix4::new_translation(&Vector3::new(x as f32, y as f32, z as f32))
        * Matrix4::new_translation(&Vector3::new(target_center[0], 0.0, target_center[1]))
        * Rotation3::from_axis_angle(&Vector3::y_axis(), angle.to_radians()).to_homogeneous()
        * Matrix4::new_translation(&Vector3::new(-source_center[0], 0.0, -source_center[1]));

    let eased = 1.0 - (1.0 - chest.lid_angle.clamp(0.0, 1.0)).powi(3);
    let lid = base
        * Matrix4::new_translation(&Vector3::new(0.0, 9.0 / 16.0, 1.0 / 16.0))
        * Rotation3::from_axis_angle(&Vector3::x_axis(), -eased * std::f32::consts::FRAC_PI_2)
            .to_homogeneous()
        * Matrix4::new_translation(&Vector3::new(0.0, -9.0 / 16.0, -1.0 / 16.0));

    let (width, texture_width) = if double { (30.0, 128.0) } else { (14.0, 64.0) };
    append_chest_box(
        vertices,
        indices,
        &base,
        [1.0 / 16.0, 0.0, 1.0 / 16.0],
        [(1.0 + width) / 16.0, 10.0 / 16.0, 15.0 / 16.0],
        [0.0, 19.0],
        [width, 10.0, 14.0],
        [texture_width, 64.0],
        region,
        chest.sky_light,
        chest.block_light,
    );
    append_chest_box(
        vertices,
        indices,
        &lid,
        [1.0 / 16.0, 9.0 / 16.0, 1.0 / 16.0],
        [(1.0 + width) / 16.0, 14.0 / 16.0, 15.0 / 16.0],
        [0.0, 0.0],
        [width, 5.0, 14.0],
        [texture_width, 64.0],
        region,
        chest.sky_light,
        chest.block_light,
    );
    let knob_center = if double { 16.0 } else { 8.0 };
    append_chest_box(
        vertices,
        indices,
        &lid,
        [(knob_center - 1.0) / 16.0, 7.0 / 16.0, 15.0 / 16.0],
        [(knob_center + 1.0) / 16.0, 11.0 / 16.0, 1.0],
        [0.0, 0.0],
        [2.0, 4.0, 1.0],
        [texture_width, 64.0],
        region,
        chest.sky_light,
        chest.block_light,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_chest_box(
    vertices: &mut Vec<crate::render::entity::mesh::EntityVertex>,
    indices: &mut Vec<u32>,
    transform: &nalgebra::Matrix4<f32>,
    from: [f32; 3],
    to: [f32; 3],
    texture_offset: [f32; 2],
    dimensions: [f32; 3],
    texture_size: [f32; 2],
    region: crate::render::entity::atlas::MobTextureRegion,
    sky_light: u8,
    block_light: u8,
) {
    use crate::render::entity::mesh::EntityVertex;
    use nalgebra::{Vector3, Vector4};

    let [w, h, d] = dimensions;
    let [u, v] = texture_offset;
    let uv_rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        [
            [x0 / texture_size[0], y0 / texture_size[1]],
            [x1 / texture_size[0], y0 / texture_size[1]],
            [x1 / texture_size[0], y1 / texture_size[1]],
            [x0 / texture_size[0], y1 / texture_size[1]],
        ]
    };
    let faces = [
        (
            [0.0, 1.0, 0.0],
            [
                [from[0], to[1], to[2]],
                [to[0], to[1], to[2]],
                [to[0], to[1], from[2]],
                [from[0], to[1], from[2]],
            ],
            uv_rect(u + d, v, u + d + w, v + d),
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [from[0], from[1], from[2]],
                [to[0], from[1], from[2]],
                [to[0], from[1], to[2]],
                [from[0], from[1], to[2]],
            ],
            uv_rect(u + d + w, v, u + d + w + w, v + d),
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [to[0], from[1], from[2]],
                [from[0], from[1], from[2]],
                [from[0], to[1], from[2]],
                [to[0], to[1], from[2]],
            ],
            uv_rect(u + d + w + d, v + d, u + d + w + d + w, v + d + h),
        ),
        (
            [0.0, 0.0, 1.0],
            [
                [from[0], from[1], to[2]],
                [to[0], from[1], to[2]],
                [to[0], to[1], to[2]],
                [from[0], to[1], to[2]],
            ],
            uv_rect(u + d, v + d, u + d + w, v + d + h),
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [from[0], from[1], from[2]],
                [from[0], from[1], to[2]],
                [from[0], to[1], to[2]],
                [from[0], to[1], from[2]],
            ],
            uv_rect(u, v + d, u + d, v + d + h),
        ),
        (
            [1.0, 0.0, 0.0],
            [
                [to[0], from[1], to[2]],
                [to[0], from[1], from[2]],
                [to[0], to[1], from[2]],
                [to[0], to[1], to[2]],
            ],
            uv_rect(u + d + w, v + d, u + d + w + d, v + d + h),
        ),
    ];
    let normal_matrix = transform.fixed_view::<3, 3>(0, 0);
    let packed_light = 16.0 + (sky_light as f32 * 16.0 + block_light as f32);
    for (normal, points, uvs) in faces {
        let base = vertices.len() as u32;
        let transformed_normal =
            (normal_matrix * Vector3::new(normal[0], normal[1], normal[2])).normalize();
        for (point, uv) in points.into_iter().zip(uvs) {
            let world = transform * Vector4::new(point[0], point[1], point[2], 1.0);
            let (atlas_u, atlas_v) = region.local_to_atlas(uv[0], uv[1]);
            vertices.push(EntityVertex {
                position: [world.x, world.y, world.z],
                normal: [
                    transformed_normal.x,
                    transformed_normal.y,
                    transformed_normal.z,
                ],
                uv: [atlas_u, atlas_v],
                color: [1.0, 1.0, 1.0, packed_light],
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn append_arrow_entity_mesh(
    vertices: &mut Vec<crate::render::entity::mesh::EntityVertex>,
    indices: &mut Vec<u32>,
    position: [f32; 3],
    yaw_deg: f32,
    pitch_deg: f32,
    region: crate::render::entity::atlas::MobTextureRegion,
) {
    use crate::render::entity::mesh::EntityVertex;
    use nalgebra::{Matrix4, Rotation3, Scale3, Vector3, Vector4};

    let transform = Matrix4::new_translation(&Vector3::new(position[0], position[1], position[2]))
        * Rotation3::from_axis_angle(&Vector3::y_axis(), (yaw_deg - 90.0).to_radians())
            .to_homogeneous()
        * Rotation3::from_axis_angle(&Vector3::z_axis(), pitch_deg.to_radians()).to_homogeneous()
        * Rotation3::from_axis_angle(&Vector3::x_axis(), 45.0_f32.to_radians()).to_homogeneous()
        * Scale3::new(0.05625, 0.05625, 0.05625).to_homogeneous()
        * Matrix4::new_translation(&Vector3::new(-4.0, 0.0, 0.0));

    let mut push_quad =
        |matrix: &Matrix4<f32>, points: [[f32; 3]; 4], normal: [f32; 3], uvs: [[f32; 2]; 4]| {
            let base = vertices.len() as u32;
            let normal_matrix = matrix.fixed_view::<3, 3>(0, 0);
            let transformed_normal =
                (normal_matrix * Vector3::new(normal[0], normal[1], normal[2])).normalize();
            for (point, uv) in points.into_iter().zip(uvs) {
                let world = matrix * Vector4::new(point[0], point[1], point[2], 1.0);
                let (u, v) = region.local_to_atlas(uv[0], uv[1]);
                vertices.push(EntityVertex {
                    position: [world.x, world.y, world.z],
                    normal: [
                        transformed_normal.x,
                        transformed_normal.y,
                        transformed_normal.z,
                    ],
                    uv: [u, v],
                    color: [1.0; 4],
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };

    let cap_uvs = [
        [0.0, 5.0 / 32.0],
        [5.0 / 32.0, 5.0 / 32.0],
        [5.0 / 32.0, 10.0 / 32.0],
        [0.0, 10.0 / 32.0],
    ];
    push_quad(
        &transform,
        [
            [-7.0, -2.0, -2.0],
            [-7.0, -2.0, 2.0],
            [-7.0, 2.0, 2.0],
            [-7.0, 2.0, -2.0],
        ],
        [1.0, 0.0, 0.0],
        cap_uvs,
    );
    push_quad(
        &transform,
        [
            [-7.0, 2.0, -2.0],
            [-7.0, 2.0, 2.0],
            [-7.0, -2.0, 2.0],
            [-7.0, -2.0, -2.0],
        ],
        [-1.0, 0.0, 0.0],
        cap_uvs,
    );

    let shaft_uvs = [[0.0, 0.0], [0.5, 0.0], [0.5, 5.0 / 32.0], [0.0, 5.0 / 32.0]];
    for quarter_turn in 1..=4 {
        let rotated = transform
            * Rotation3::from_axis_angle(
                &Vector3::x_axis(),
                (quarter_turn as f32 * 90.0).to_radians(),
            )
            .to_homogeneous();
        push_quad(
            &rotated,
            [
                [-8.0, -2.0, 0.0],
                [8.0, -2.0, 0.0],
                [8.0, 2.0, 0.0],
                [-8.0, 2.0, 0.0],
            ],
            [0.0, 0.0, 1.0],
            shaft_uvs,
        );
    }
}

/// Return the centre and outward normal of the visible sign-board face.
/// Coordinates match the custom sign geometry in `world::shape` and the
/// metadata transforms from vanilla's TileEntitySignRenderer.
fn sign_text_plane(sign: &crate::render::hud::entities::SignEntry) -> ([f32; 3], [f32; 3]) {
    let [x, y, z] = sign.position;
    if !sign.wall_mounted {
        // Vulkan mesh rotation uses the opposite sign from vanilla's OpenGL
        // glRotate call. The board starts with its front along +Z.
        let angle = ((sign.metadata & 0x0f) as f32) * 22.5_f32.to_radians();
        let normal = [-angle.sin(), 0.0, angle.cos()];
        return (
            [
                x as f32 + 0.5 + normal[0] * (1.0 / 24.0 + 0.005),
                y as f32 + 13.333_333 / 16.0,
                z as f32 + 0.5 + normal[2] * (1.0 / 24.0 + 0.005),
            ],
            normal,
        );
    }

    let normal = match sign.metadata & 0x07 {
        2 => [0.0, 0.0, -1.0], // north-facing, attached to the south wall
        3 => [0.0, 0.0, 1.0],  // south-facing, attached to the north wall
        4 => [-1.0, 0.0, 0.0], // west-facing, attached to the east wall
        5 => [1.0, 0.0, 0.0],  // east-facing, attached to the west wall
        _ => [0.0, 0.0, 1.0],  // vanilla renderer's zero-rotation fallback
    };
    (
        [
            // The visible board surface is 19/48 block from the centre.
            // Vanilla places text another 0.005 block toward the viewer.
            x as f32 + 0.5 - normal[0] * (19.0 / 48.0) + normal[0] * 0.005,
            y as f32 + 8.333_333 / 16.0,
            z as f32 + 0.5 - normal[2] * (19.0 / 48.0) + normal[2] * 0.005,
        ],
        normal,
    )
}

fn build_sign_texture(
    font: &mut crate::ui::font::FontRenderer,
    lines: &[String],
) -> (Vec<u8>, u32, u32) {
    // Keep the vanilla 90x40 logical text area, but rasterize at 2x so CJK
    // glyphs remain readable on the world-space sign.
    const SCALE: u32 = 2;
    let w = 90 * SCALE;
    let h = 40 * SCALE;
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let font_size = 10.0 * SCALE as f32;
    for (i, line) in lines.iter().enumerate().take(4) {
        if line.is_empty() {
            continue;
        }
        let mut glyphs = Vec::new();
        let mut chars = line.chars();
        while let Some(ch) = chars.next() {
            if ch == '\u{00a7}' {
                chars.next();
                continue;
            }
            let (metrics, bitmap) = font.font.rasterize(ch, font_size);
            glyphs.push((metrics, bitmap));
        }
        let line_width = glyphs
            .iter()
            .map(|(metrics, _)| metrics.advance_width)
            .sum::<f32>()
            .ceil() as i32;
        let mut cx = ((w as i32 - line_width) / 2).max(0);
        let baseline = (i as u32 * 10 + 9) as i32 * SCALE as i32;
        for (metrics, bitmap) in glyphs {
            if metrics.width == 0 || metrics.height == 0 {
                cx += metrics.advance_width.ceil() as i32;
                continue;
            }
            let dx = cx + metrics.xmin;
            let dy = baseline - metrics.ymin - metrics.height as i32;
            for py in 0..metrics.height {
                for px in 0..metrics.width {
                    let alpha = bitmap[py * metrics.width + px];
                    let tx = (dx + px as i32) as u32;
                    let ty = (dy + py as i32) as u32;
                    if alpha > 0 && tx < w && ty < h {
                        let ti = ((ty * w + tx) * 4) as usize;
                        // Black text with smooth alpha from font rasterizer.
                        pixels[ti] = 0;
                        pixels[ti + 1] = 0;
                        pixels[ti + 2] = 0;
                        pixels[ti + 3] = alpha;
                    }
                }
            }
            cx += (metrics.advance_width.ceil() as i32).max(1);
            if cx >= w as i32 {
                break;
            }
        }
    }
    (pixels, w, h)
}

#[cfg(test)]
mod tests {
    use super::sign_text_plane;
    use crate::render::hud::entities::SignEntry;

    fn standing_sign(metadata: u8) -> SignEntry {
        SignEntry {
            position: [0, 0, 0],
            lines: Vec::new(),
            wall_mounted: false,
            metadata,
        }
    }

    #[test]
    fn standing_sign_text_uses_the_board_rotation() {
        let (_, north) = sign_text_plane(&standing_sign(0));
        let (_, west) = sign_text_plane(&standing_sign(4));
        let (_, south) = sign_text_plane(&standing_sign(8));

        assert_eq!(north, [0.0, 0.0, 1.0]);
        assert!((west[0] + 1.0).abs() < 0.000_01 && west[2].abs() < 0.000_01);
        assert!(south[0].abs() < 0.000_01 && (south[2] + 1.0).abs() < 0.000_01);
    }

    #[test]
    fn wall_sign_text_sits_just_in_front_of_the_vanilla_board() {
        let sign = SignEntry {
            position: [0, 0, 0],
            lines: Vec::new(),
            wall_mounted: true,
            metadata: 2,
        };
        let (centre, normal) = sign_text_plane(&sign);
        assert_eq!(normal, [0.0, 0.0, -1.0]);
        assert!((centre[1] - 8.333_333 / 16.0).abs() < 0.000_01);
        assert!((centre[2] - (0.5 + 19.0 / 48.0 - 0.005)).abs() < 0.000_01);
    }

    #[test]
    fn sign_texture_has_pixels_for_cjk_text() {
        let mut font = crate::ui::font::FontRenderer::new();
        font.preload_ascii();
        let lines = [
            "你".to_string(),
            "好".to_string(),
            "".to_string(),
            "".to_string(),
        ];
        let (pixels, w, h) = super::build_sign_texture(&mut font, &lines);
        assert_eq!(w, 180);
        assert_eq!(h, 80);
        let has_nonzero = pixels.iter().any(|&b| b > 0);
        assert!(
            has_nonzero,
            "sign texture should have non-zero pixels for CJK text"
        );
    }
}

fn build_nametag_texture(
    font: &mut crate::ui::font::FontRenderer,
    name: &str,
    font_size: f32,
) -> (Vec<u8>, u32, u32) {
    let padding = 2u32;
    let mut glyphs = Vec::new();
    let mut chars = name.chars();
    let mut color = [255, 255, 255];
    while let Some(ch) = chars.next() {
        if ch == '\u{00a7}' {
            if let Some(code) = chars.next() {
                color = nametag_color(code).unwrap_or([255, 255, 255]);
            }
            continue;
        }
        let (metrics, bitmap) = font.font.rasterize(ch, font_size);
        glyphs.push((metrics, bitmap, color));
    }
    let text_width = glyphs
        .iter()
        .map(|(metrics, _, _)| metrics.advance_width)
        .sum::<f32>()
        .ceil() as u32;
    let w = (text_width + padding * 2).clamp(16, 192);
    let h = 20u32;
    let mut pixels = vec![0u8; (w * h * 4) as usize];

    // Fill background with black at alpha 0.25 (vanilla nametag background)
    for py in 0..h {
        for px in 0..w {
            let ti = ((py * w + px) * 4) as usize;
            pixels[ti] = 0;
            pixels[ti + 1] = 0;
            pixels[ti + 2] = 0;
            pixels[ti + 3] = 64; // 0.25 alpha
        }
    }

    // Rasterize white text on top
    let baseline = 15i32;
    let mut cx = padding as i32;
    for (metrics, bitmap, color) in glyphs {
        if metrics.width == 0 || metrics.height == 0 {
            cx += (font_size * 0.5) as i32;
            continue;
        }
        let dx = cx + metrics.xmin;
        let dy = baseline - metrics.ymin - metrics.height as i32;
        for py in 0..metrics.height {
            for px in 0..metrics.width {
                let alpha = bitmap[py * metrics.width + px];
                let tx = (dx + px as i32) as u32;
                let ty = (dy + py as i32) as u32;
                if tx < w && ty < h {
                    let ti = ((ty * w + tx) * 4) as usize;
                    let bg_a = pixels[ti + 3] as u32;
                    let fg_a = alpha as u32;
                    let blended = bg_a + fg_a - (bg_a * fg_a) / 255;
                    pixels[ti] = color[0];
                    pixels[ti + 1] = color[1];
                    pixels[ti + 2] = color[2];
                    pixels[ti + 3] = blended as u8;
                }
            }
        }
        cx += (metrics.advance_width as i32).max(1);
    }
    (pixels, w, h)
}

fn nametag_color(code: char) -> Option<[u8; 3]> {
    Some(match code.to_ascii_lowercase() {
        '0' => [0, 0, 0],
        '1' => [0, 0, 170],
        '2' => [0, 170, 0],
        '3' => [0, 170, 170],
        '4' => [170, 0, 0],
        '5' => [170, 0, 170],
        '6' => [255, 170, 0],
        '7' => [170, 170, 170],
        '8' => [85, 85, 85],
        '9' => [85, 85, 255],
        'a' => [85, 255, 85],
        'b' => [85, 255, 255],
        'c' => [255, 85, 85],
        'd' => [255, 85, 255],
        'e' => [255, 255, 85],
        'f' | 'r' => [255, 255, 255],
        _ => return None,
    })
}
