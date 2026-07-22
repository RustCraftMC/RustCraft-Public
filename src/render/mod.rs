//! Vulkan renderer for RustCraft.
//!
//! Modules:
//! - `vulkan` — Instance, device, surface, swapchain, depth buffer, sync
//! - `pipeline` — Render pass, graphics pipeline, descriptors
//! - `resources` — Buffers, textures, uniforms
//! - `rendering` — Frame drawing, mesh upload, swapchain recreation
//! - `swapchain` — Swapchain, depth buffer, framebuffer, and frame-sync management
//! - `resource_manager` — GPU allocator, uniform buffers, buffer/image helpers

pub(crate) mod custom_sky;
pub mod entity;
pub mod first_person;
pub mod gui;
pub mod hooks;
pub(crate) mod hud;
pub(crate) mod item_icons;
mod particle_mesh;
mod particles;
mod pipeline;
pub(crate) mod raii;
mod rendering;
pub(crate) mod resources;
mod resource_manager;
mod runtime_world;
mod screens;
pub mod shader_pack;
pub mod sky;
pub mod state;
mod swapchain;
mod ui_hooks;
pub mod upscaler;
mod vulkan;

pub use crate::client::interaction::SelectionBox;
use ash::vk;
pub use particles::ParticleSprite;

pub(crate) const MAX_FRAMES: usize = 4;
pub(crate) const GUI_TEXTURE_COUNT: usize = 11;

// ---------------------------------------------------------------------------
// GPU-side uniform block
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    /// rgb = fog color, w = enchanted-glint scroll seconds (basic.frag).
    fog_color: [f32; 4],
    fog_params: [f32; 4],
    grass_color: [f32; 4],
    /// x = brightness gamma (0..1), y = night-vision strength (0..1).
    lightmap_params: [f32; 4],
}

/// GPU-side uniform block for sky rendering.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyUniforms {
    zenith: [f32; 4],             // sky top color (rgb, unused a)
    horizon: [f32; 4],            // sky horizon color (rgb, unused a)
    sun_dir: [f32; 4],            // sun direction (xyz), sun brightness (w)
    fog_params: [f32; 4],         // x=viewport_w, y=moon_phase, z=viewport_h, w=daylight
    custom_sky: [f32; 4], // x=custom_sky_alpha (0=none), y=rotation_angle, z=reserved, w=reserved
    inv_view_proj: [[f32; 4]; 4], // inverse view-projection matrix (without camera translation)
}

// ---------------------------------------------------------------------------
// Batched chunk geometry
// ---------------------------------------------------------------------------

const CHUNK_VERTEX_ARENA_BYTES: u64 = 48 * 1024 * 1024;
const CHUNK_INDEX_ARENA_BYTES: u64 = 24 * 1024 * 1024;

#[derive(Clone, Copy)]
struct BufferRange {
    offset: u32,
    len: u32,
}

struct BufferRangeAllocator {
    free: Vec<BufferRange>,
}

impl BufferRangeAllocator {
    fn new(capacity: u32) -> Self {
        Self {
            free: vec![BufferRange {
                offset: 0,
                len: capacity,
            }],
        }
    }

    fn allocate(&mut self, len: u32) -> Option<u32> {
        if len == 0 {
            return Some(0);
        }
        let index = self.free.iter().position(|range| range.len >= len)?;
        let offset = self.free[index].offset;
        self.free[index].offset += len;
        self.free[index].len -= len;
        if self.free[index].len == 0 {
            self.free.remove(index);
        }
        Some(offset)
    }

    fn release(&mut self, offset: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.free.push(BufferRange { offset, len });
        self.free.sort_unstable_by_key(|range| range.offset);

        let mut write = 0usize;
        for read in 0..self.free.len() {
            let range = self.free[read];
            if write > 0 {
                let previous = &mut self.free[write - 1];
                if previous.offset.saturating_add(previous.len) == range.offset {
                    previous.len = previous.len.saturating_add(range.len);
                    continue;
                }
            }
            self.free[write] = range;
            write += 1;
        }
        self.free.truncate(write);
    }
}

enum ChunkStorage {
    Shared {
        first_vertex: u32,
        vertex_count: u32,
        first_index: u32,
        index_count: u32,
    },
    Dedicated {
        vertex_buffer: vk::Buffer,
        index_buffer: vk::Buffer,
        vertex_alloc: gpu_allocator::vulkan::Allocation,
        index_alloc: gpu_allocator::vulkan::Allocation,
    },
}

struct DrawCmd {
    storage: ChunkStorage,
    index_count: u32,
    transparent_start: u32,
    cx: i32,
    cz: i32,
    /// World-space AABB for frustum culling
    aabb_min: [f32; 3],
    aabb_max: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkIndirectCommand {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    vertex_offset: i32,
    first_instance: u32,
}

#[cfg(test)]
mod chunk_batch_tests {
    use super::{BufferRangeAllocator, ChunkIndirectCommand};

    #[test]
    fn released_chunk_ranges_are_merged_and_reused() {
        let mut allocator = BufferRangeAllocator::new(100);
        let a = allocator.allocate(20).unwrap();
        let b = allocator.allocate(30).unwrap();
        let c = allocator.allocate(50).unwrap();
        assert_eq!((a, b, c), (0, 20, 50));
        assert!(allocator.allocate(1).is_none());

        allocator.release(b, 30);
        allocator.release(a, 20);
        allocator.release(c, 50);
        assert_eq!(allocator.allocate(100), Some(0));
    }

    #[test]
    fn indirect_command_matches_vulkan_layout() {
        assert_eq!(std::mem::size_of::<ChunkIndirectCommand>(), 20);
    }
}

#[derive(Default)]
struct GuiBufferSlot {
    vertex_buffer: Option<vk::Buffer>,
    index_buffer: Option<vk::Buffer>,
    vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    vertex_capacity: u64,
    index_capacity: u64,
    vertex_hash: u64,
    index_hash: u64,
    content_generation: Option<u64>,
}

type GuiBuilderSet = (
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
    gui::GuiVertexBuilder,
);

#[derive(Default)]
struct DynamicGpuMesh {
    vertex_buffer: Option<vk::Buffer>,
    index_buffer: Option<vk::Buffer>,
    vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    vertex_capacity: u64,
    index_capacity: u64,
    index_count: u32,
}

struct EntityGpuMesh {
    state_hash: [Option<u64>; MAX_FRAMES],
    last_seen: u64,
    body: [DynamicGpuMesh; MAX_FRAMES],
    held_block: [DynamicGpuMesh; MAX_FRAMES],
    held_item: [DynamicGpuMesh; MAX_FRAMES],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct EntityModelKey {
    entity_type: crate::entity::EntityType,
    variant: u16,
}

impl Default for EntityGpuMesh {
    fn default() -> Self {
        Self {
            state_hash: [None; MAX_FRAMES],
            last_seen: 0,
            body: std::array::from_fn(|_| DynamicGpuMesh::default()),
            held_block: std::array::from_fn(|_| DynamicGpuMesh::default()),
            held_item: std::array::from_fn(|_| DynamicGpuMesh::default()),
        }
    }
}

fn destroy_dynamic_gpu_mesh(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    mesh: &mut DynamicGpuMesh,
) {
    unsafe {
        if let Some(buffer) = mesh.vertex_buffer.take() {
            device.destroy_buffer(buffer, None);
        }
        if let Some(buffer) = mesh.index_buffer.take() {
            device.destroy_buffer(buffer, None);
        }
    }
    if let Some(allocation) = mesh.vertex_alloc.take() {
        allocator.free(allocation).ok();
    }
    if let Some(allocation) = mesh.index_alloc.take() {
        allocator.free(allocation).ok();
    }
    mesh.index_count = 0;
}

fn destroy_entity_gpu_mesh(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    mesh: &mut EntityGpuMesh,
) {
    for frame in 0..MAX_FRAMES {
        destroy_dynamic_gpu_mesh(device, allocator, &mut mesh.body[frame]);
        destroy_dynamic_gpu_mesh(device, allocator, &mut mesh.held_block[frame]);
        destroy_dynamic_gpu_mesh(device, allocator, &mut mesh.held_item[frame]);
    }
}

// ---------------------------------------------------------------------------
// Renderer — all Vulkan state
// ---------------------------------------------------------------------------

pub struct Renderer {
    // Core
    _entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,
    _physical_device: vk::PhysicalDevice,
    queue: vk::Queue,
    // Debug messenger — only present when the validation layer is enabled.
    // `debug_messenger` is vk::DebugUtilsMessengerEXT::null() when disabled.
    debug_utils: Option<ash::ext::debug_utils::Instance>,
    debug_messenger: vk::DebugUtilsMessengerEXT,

    // Swapchain subsystem (surface, swapchain, depth, framebuffers, sync)
    swapchain: swapchain::SwapchainManager,

    // Pipeline
    render_pass: vk::RenderPass,
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    // Transparent pass pipeline: alpha blending with depth writes enabled.
    transparent_pipeline: vk::Pipeline,
    descriptor_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: Vec<vk::DescriptorSet>,

    // Sky pipeline
    sky_pipeline: vk::Pipeline,
    sky_pipeline_layout: vk::PipelineLayout,
    sky_uniform_buffer: vk::Buffer,
    sky_uniform_alloc: gpu_allocator::vulkan::Allocation,
    sky_descriptor_pool: vk::DescriptorPool,
    sky_descriptor_sets: Vec<vk::DescriptorSet>,
    sky_vertex_buffer: vk::Buffer,
    sky_vertex_alloc: gpu_allocator::vulkan::Allocation,

    // Entity pipeline
    entity_pipeline: vk::Pipeline,
    entity_pipeline_layout: vk::PipelineLayout,
    particle_pipeline: vk::Pipeline,
    particle_pipeline_layout: vk::PipelineLayout,
    entity_vertex_buffer: Option<vk::Buffer>,
    entity_index_buffer: Option<vk::Buffer>,
    entity_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    entity_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    entity_index_count: u32,
    entity_vertex_capacity: u64,
    entity_index_capacity: u64,

    entity_held_block_vertex_buffer: Option<vk::Buffer>,
    entity_held_block_index_buffer: Option<vk::Buffer>,
    entity_held_block_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    entity_held_block_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    entity_held_block_index_count: u32,
    entity_held_block_vertex_capacity: u64,
    entity_held_block_index_capacity: u64,
    entity_held_item_vertex_buffer: Option<vk::Buffer>,
    entity_held_item_index_buffer: Option<vk::Buffer>,
    entity_held_item_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    entity_held_item_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    entity_held_item_index_count: u32,
    entity_held_item_vertex_capacity: u64,
    entity_held_item_index_capacity: u64,
    // Pre-allocated entity mesh builder vectors (reused per frame)
    entity_mesh_vertices: Vec<entity::mesh::EntityVertex>,
    entity_mesh_indices: Vec<u32>,
    entity_held_block_vertices: Vec<crate::world::mesh::Vertex>,
    entity_held_block_indices: Vec<u32>,
    entity_held_item_vertices: Vec<crate::world::mesh::Vertex>,
    entity_held_item_indices: Vec<u32>,
    // Block selection wireframe + crack overlay (separate buffers from entities)
    block_vertex_buffer: Option<vk::Buffer>,
    block_index_buffer: Option<vk::Buffer>,
    block_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    block_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    block_index_count: u32,
    block_vertex_capacity: u64,
    block_index_capacity: u64,
    pub block_dirty: bool,
    /// Pending texture atlas to upload on next frame (resource pack reload)
    pub pending_block_atlas: Option<(
        crate::assets::texture::TextureAtlas,
        crate::render::entity::atlas::EntityTextureAtlas,
    )>,
    // Particle mesh (per-frame generated billboard quads)
    particle_vertex_buffer: Option<vk::Buffer>,
    particle_index_buffer: Option<vk::Buffer>,
    particle_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    particle_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    particle_index_count: u32,
    particle_vertex_capacity: u64,
    particle_index_capacity: u64,
    particle_generation: u64,
    particle_mesh_hash: u64,
    // Nametag mesh (billboard quads above entity heads, depth_test=OFF)
    nametag_pipeline: vk::Pipeline,
    nametag_pipeline_layout: vk::PipelineLayout,
    nametag_vertex_buffer: Option<vk::Buffer>,
    nametag_index_buffer: Option<vk::Buffer>,
    nametag_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    nametag_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    nametag_index_count: u32,
    nametag_vertex_capacity: u64,
    nametag_index_capacity: u64,
    nametag_text_hash: u64,
    // First-person hand meshes
    fp_arm_vertex_buffer: Option<vk::Buffer>,
    fp_arm_index_buffer: Option<vk::Buffer>,
    fp_arm_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    fp_arm_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    fp_arm_index_count: u32,
    fp_arm_vertex_capacity: u64,
    fp_arm_index_capacity: u64,

    pub fp_block_uses_item_atlas: bool,

    fp_block_op_vertex_buffer: Option<vk::Buffer>,
    fp_block_op_index_buffer: Option<vk::Buffer>,
    fp_block_op_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    fp_block_op_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    fp_block_op_index_count: u32,
    fp_block_op_vertex_capacity: u64,
    fp_block_op_index_capacity: u64,

    fp_block_tr_vertex_buffer: Option<vk::Buffer>,
    fp_block_tr_index_buffer: Option<vk::Buffer>,
    fp_block_tr_vertex_alloc: Option<gpu_allocator::vulkan::Allocation>,
    fp_block_tr_index_alloc: Option<gpu_allocator::vulkan::Allocation>,
    fp_block_tr_index_count: u32,
    fp_block_tr_vertex_capacity: u64,
    fp_block_tr_index_capacity: u64,
    local_mesh_hash: Option<u64>,
    // Entity texture atlas
    entity_texture_image: vk::Image,
    entity_texture_view: vk::ImageView,
    entity_texture_sampler: vk::Sampler,
    entity_texture_alloc: Option<gpu_allocator::vulkan::Allocation>,
    entity_descriptor_pool: vk::DescriptorPool,
    entity_descriptor_sets: Vec<vk::DescriptorSet>,
    entity_atlas: Option<entity::atlas::EntityTextureAtlas>,
    entity_skin_upload_buffers: Vec<Option<vk::Buffer>>,
    entity_skin_upload_allocs: Vec<Option<gpu_allocator::vulkan::Allocation>>,
    entity_skin_upload_capacities: Vec<u64>,
    entity_skin_upload_pending: bool,
    entity_atlas_full_upload_pending: bool,
    block_atlas: Option<crate::assets::texture::TextureAtlas>,
    block_animation_buffers: Vec<Option<vk::Buffer>>,
    block_animation_allocs: Vec<Option<gpu_allocator::vulkan::Allocation>>,
    block_animation_capacities: Vec<u64>,
    block_animation_upload_bytes: Vec<u8>,
    block_animation_uploads: Vec<crate::assets::texture::AtlasAnimationUpload>,
    block_animation_last_tick: std::time::Instant,

    // Skin texture (for first person arm)
    skin_texture_image: vk::Image,
    skin_texture_view: vk::ImageView,
    skin_texture_alloc: Option<gpu_allocator::vulkan::Allocation>,
    skin_descriptor_sets: Vec<vk::DescriptorSet>,
    local_skin_upload_buffers: Vec<Option<vk::Buffer>>,
    local_skin_upload_allocs: Vec<Option<gpu_allocator::vulkan::Allocation>>,
    local_skin_upload_capacities: Vec<u64>,
    local_skin_upload_pending: bool,
    /// RGBA cape pixels (64×32) to be uploaded into the entity atlas.
    cape_pixels: Option<Vec<u8>>,
    cape_upload_pending: bool,

    sun_texture_image: vk::Image,
    sun_texture_view: vk::ImageView,
    sun_texture_alloc: Option<gpu_allocator::vulkan::Allocation>,
    moon_texture_image: vk::Image,
    moon_texture_view: vk::ImageView,
    moon_texture_alloc: Option<gpu_allocator::vulkan::Allocation>,

    // Custom sky (OptiFine mcpatcher support)
    custom_sky_texture_image: vk::Image,
    custom_sky_texture_view: vk::ImageView,
    custom_sky_texture_alloc: Option<gpu_allocator::vulkan::Allocation>,
    custom_sky_data: Option<custom_sky::CustomSky>,

    // Panorama
    panorama_pipeline: vk::Pipeline,
    panorama_pipeline_layout: vk::PipelineLayout,
    panorama_descriptor_pool: vk::DescriptorPool,
    panorama_descriptor_sets: Vec<vk::DescriptorSet>,
    panorama_image: vk::Image,
    panorama_view: vk::ImageView,
    panorama_sampler: vk::Sampler,
    panorama_alloc: Option<gpu_allocator::vulkan::Allocation>,

    pub fp_item_descriptor_pool: vk::DescriptorPool,
    pub fp_item_descriptor_sets: Vec<vk::DescriptorSet>,
    pub hand_equip_progress: f32,
    pub hand_animation_last_update: std::time::Instant,
    panorama_uniform_buffer: vk::Buffer,
    panorama_uniform_alloc: gpu_allocator::vulkan::Allocation,

    // Framebuffers & commands
    command_buffers: Vec<vk::CommandBuffer>,
    command_pool: vk::CommandPool,

    // Resource subsystem (GPU allocator, uniform buffers)
    resources: resource_manager::ResourceManager,

    // Texture
    texture_image: vk::Image,
    texture_image_view: vk::ImageView,
    texture_sampler: vk::Sampler,
    texture_alloc: Option<gpu_allocator::vulkan::Allocation>,

    // Draw commands & mesh allocations
    draw_cmds: Vec<DrawCmd>,
    /// Maps chunk coordinates to their entry in `draw_cmds` so incremental
    /// mesh uploads do not scan every loaded chunk.
    draw_cmd_indices: fnv::FnvHashMap<(i32, i32), usize>,
    visible_chunk_indices: Vec<usize>,
    transparent_draw_indices: Vec<usize>,
    retired_draw_cmds: Vec<Vec<DrawCmd>>,
    pending_retired_draw_cmds: Vec<DrawCmd>,
    chunk_vertex_buffer: vk::Buffer,
    chunk_vertex_alloc: gpu_allocator::vulkan::Allocation,
    chunk_vertex_ranges: BufferRangeAllocator,
    chunk_index_buffer: vk::Buffer,
    chunk_index_alloc: gpu_allocator::vulkan::Allocation,
    chunk_index_ranges: BufferRangeAllocator,
    chunk_upload_buffers: Vec<Option<vk::Buffer>>,
    chunk_upload_allocs: Vec<Option<gpu_allocator::vulkan::Allocation>>,
    chunk_upload_capacities: Vec<u64>,
    chunk_upload_bytes: Vec<u8>,
    chunk_vertex_upload_copies: Vec<vk::BufferCopy>,
    chunk_index_upload_copies: Vec<vk::BufferCopy>,
    chunk_indirect_buffers: Vec<Option<vk::Buffer>>,
    chunk_indirect_allocs: Vec<Option<gpu_allocator::vulkan::Allocation>>,
    chunk_indirect_capacities: Vec<u64>,
    chunk_indirect_commands: Vec<ChunkIndirectCommand>,
    chunk_opaque_indirect_count: u32,
    chunk_transparent_indirect_offset: u64,
    chunk_transparent_indirect_count: u32,
    multi_draw_indirect: bool,
    // GUI geometry updates independently from the uncapped 3D renderer.
    last_gui_build: std::time::Instant,
    /// Tracks last entity state hash to skip mesh regen when unchanged.
    entity_state_hash: u64,
    /// Tracks sign text already packed into the entity atlas.
    sign_atlas_hash: u64,
    // Entity model cuboid cache (entity type plus geometry-affecting variant)
    entity_model_cache:
        std::collections::HashMap<EntityModelKey, std::sync::Arc<Vec<entity::mesh::ModelCuboid>>>,
    /// Per-entity mesh cache — key = entity_id, value = (state_hash, vertices, indices)
    entity_mesh_cache: fnv::FnvHashMap<i32, (u64, Vec<entity::mesh::EntityVertex>, Vec<u32>)>,
    entity_gpu_meshes: fnv::FnvHashMap<i32, EntityGpuMesh>,
    visible_entity_ids: Vec<i32>,
    stale_entity_ids: Vec<i32>,
    entity_frame_generation: u64,
    synced_player_skin_content_hash: Option<u64>,
    player_skin_layout_hash: u64,
    player_skin_atlas_generation: u64,
    entity_atlas_generation: u64,


    // GUI rendering
    gui_pipeline: vk::Pipeline,
    gui_pipeline_layout: vk::PipelineLayout,
    gui_descriptor_layout: vk::DescriptorSetLayout,
    gui_descriptor_pool: vk::DescriptorPool,
    gui_descriptor_sets: Vec<vk::DescriptorSet>,
    // Widget texture atlas
    gui_widget_image: vk::Image,
    gui_widget_view: vk::ImageView,
    gui_widget_sampler: vk::Sampler,
    gui_widget_alloc: gpu_allocator::vulkan::Allocation,
    // Font atlas
    gui_font_image: vk::Image,
    gui_font_view: vk::ImageView,
    gui_font_sampler: vk::Sampler,
    gui_font_alloc: gpu_allocator::vulkan::Allocation,
    // Container GUI textures
    gui_inventory_image: vk::Image,
    gui_inventory_view: vk::ImageView,
    gui_inventory_size: [u32; 2],
    gui_inventory_sampler: vk::Sampler,
    gui_inventory_alloc: gpu_allocator::vulkan::Allocation,
    gui_generic54_image: vk::Image,
    gui_generic54_view: vk::ImageView,
    gui_generic54_sampler: vk::Sampler,
    gui_generic54_alloc: gpu_allocator::vulkan::Allocation,
    gui_items_image: vk::Image,
    gui_items_view: vk::ImageView,
    gui_items_sampler: vk::Sampler,
    gui_items_alloc: gpu_allocator::vulkan::Allocation,
    // Icons texture (icons.png - hearts, hunger, armor, XP bar)
    gui_icons_image: vk::Image,
    gui_icons_view: vk::ImageView,
    gui_icons_sampler: vk::Sampler,
    gui_icons_alloc: gpu_allocator::vulkan::Allocation,
    // Creative inventory texture atlas
    gui_creative_image: vk::Image,
    gui_creative_view: vk::ImageView,
    gui_creative_sampler: vk::Sampler,
    gui_creative_alloc: gpu_allocator::vulkan::Allocation,
    // Options background
    gui_options_bg_image: vk::Image,
    gui_options_bg_view: vk::ImageView,
    gui_options_bg_sampler: vk::Sampler,
    gui_options_bg_alloc: gpu_allocator::vulkan::Allocation,
    // Underwater overlay
    gui_underwater_image: vk::Image,
    gui_underwater_view: vk::ImageView,
    gui_underwater_sampler: vk::Sampler,
    gui_underwater_alloc: gpu_allocator::vulkan::Allocation,
    // Vertex/index buffers per frame and GUI texture layer.
    gui_buffers: Vec<GuiBufferSlot>,
    gui_builder_cache: Option<GuiBuilderSet>,
    player_preview_cache: entity::player_model::PlayerPreviewCache,
    gui_uniform_buffer: vk::Buffer,
    gui_uniform_alloc: gpu_allocator::vulkan::Allocation,
    gui_font_uploaded: bool,
    cached_gui_vp_w: f32,
    cached_gui_vp_h: f32,

    // Font for GUI text
    font: crate::ui::font::FontRenderer,

    // Button hit regions from last frame (for click detection)
    pub last_button_hits: Vec<crate::render::gui::ButtonHit>,
    gui_mouse_pos: [f32; 2],
    current_camera: Option<crate::client::player::Camera>,

    // Game / UI state — held separately for cleaner architecture
    pub state: state::GameRenderState,

    // Renderer-internal state
    particles: Vec<ParticleSprite>,
    particle_list: Vec<crate::client::particles::Particle>,

    first_frame_done: std::cell::Cell<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct ServerListRow {
    pub name: String,
    pub address: String,
    pub online: bool,
    pub ping_ms: Option<u32>,
    pub players_online: Option<u32>,
    pub players_max: Option<u32>,
    pub version_name: Option<String>,
    pub description: Option<String>,
    pub error: Option<String>,
    /// Decoded server icon RGBA pixel data (64x64, 64*64*4 bytes).
    pub favicon_pixels: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Default)]
pub struct ModManagerRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub protocol_translator: bool,
    pub config_entries: usize,
    pub granted_permissions: Vec<String>,
    pub denied_permissions: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ModConfigRow {
    pub key: String,
    pub label: String,
    pub description: String,
    pub value: String,
    pub is_default: bool,
    pub can_previous: bool,
    pub can_next: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SidebarLine {
    pub display: String,
    pub score: i32,
}

#[derive(Clone, Debug)]
pub struct EntityBillboard {
    pub entity_id: i32,
    pub position: [f32; 3],
    /// Minecraft lightmap coordinates sampled at the entity's feet.
    pub sky_light: u8,
    pub block_light: u8,
    pub height: f32,
    pub width: f32,
    pub name: Option<String>,
    pub kind: EntityBillboardKind,
    pub entity_type: crate::entity::EntityType,
    pub health: Option<(f32, f32)>,
    pub held_item: Option<u16>,
    pub equipment: [Option<(u16, u16)>; 5],
    pub item_id: Option<u16>,
    pub item_damage: Option<u16>,
    pub item_nbt: Option<Vec<u8>>,
    pub swing_progress: f32,
    pub skin_key: Option<String>,
    pub slim: bool,
    pub skin_parts_mask: u8,
    pub has_cape: bool,
    /// LayerCape rotations in RustCraft model space (X, Y, Z radians).
    pub cape_rotation: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub limb_swing: f32,
    pub limb_swing_amount: f32,
    pub sneaking: bool,
    pub blocking: bool,
    /// Potion invisibility hides the model while particles still render.
    pub invisible: bool,
    pub riding: bool,
    /// Whether the nametag should be visible (non-empty name, not invisible, not being ridden).
    pub name_visible: bool,
    pub age_ticks: f32,
    pub hover_start: f32,
    pub velocity: [f32; 3],
    pub hurt_alpha: f32,
    pub death_alpha: f32,
    pub swing_alpha: f32,
    pub critical_alpha: f32,
    pub visual: crate::entity::EntityVisualState,
}

#[derive(Clone, Debug)]
pub struct PendingPlayerSkin {
    pub key: String,
    pub skin: std::sync::Arc<crate::assets::skin::PlayerSkin>,
    pub content_hash: u64,
    pub cape_pixels: Option<std::sync::Arc<Vec<u8>>>,
    pub cape_content_hash: u64,
}

fn entity_model_key(billboard: &EntityBillboard) -> EntityModelKey {
    let variant = match billboard.entity_type {
        crate::entity::EntityType::Player => {
            billboard.skin_parts_mask as u16
                | ((billboard.slim as u16) << 8)
                | ((billboard.has_cape as u16) << 9)
        }
        crate::entity::EntityType::Zombie | crate::entity::EntityType::Giant => {
            billboard.visual.zombie_villager as u16
        }
        crate::entity::EntityType::Bat => billboard.visual.bat_hanging as u16,
        crate::entity::EntityType::ArmorStand => billboard.visual.armor_stand_flags as u16,
        _ => 0,
    };
    EntityModelKey {
        entity_type: billboard.entity_type,
        variant,
    }
}

/// Hash only billboard fields that affect the 3D entity mesh.  UI-only state
/// can then update without forcing a rebuild of the combined GPU mesh.
pub(crate) fn entity_mesh_state_hash(billboard: &EntityBillboard) -> u64 {
    use std::hash::Hasher;

    let mut h = fnv::FnvHasher::default();
    h.write_i32(billboard.entity_id);
    h.write_u32(billboard.entity_type as u32);
    h.write_i64((billboard.position[0] * 50.0) as i64);
    h.write_i64((billboard.position[1] * 50.0) as i64);
    h.write_i64((billboard.position[2] * 50.0) as i64);
    h.write_u8(billboard.sky_light);
    h.write_u8(billboard.block_light);
    h.write_i32((billboard.yaw * 50.0) as i32);
    h.write_i32((billboard.head_yaw * 50.0) as i32);
    h.write_i32((billboard.pitch * 50.0) as i32);
    h.write_u32((billboard.limb_swing * 2.0) as u32);
    h.write_u32((billboard.limb_swing_amount * 5.0) as u32);
    h.write_u32((billboard.swing_alpha * 10.0) as u32);
    h.write_u32((billboard.age_ticks as u32) / 60);
    if billboard.kind == EntityBillboardKind::Item {
        h.write_u32((billboard.hover_start * 1000.0) as u32);
    }
    if billboard.entity_type == crate::entity::EntityType::Arrow {
        h.write_u32((billboard.yaw * 100.0) as u32);
        h.write_u32((billboard.pitch * 100.0) as u32);
    }
    h.write_u32((billboard.hurt_alpha * 50.0) as u32);
    h.write_u32((billboard.death_alpha * 50.0) as u32);
    h.write_u32(billboard.sneaking as u32);
    h.write_u32(billboard.riding as u32);
    h.write_u32(billboard.blocking as u32);
    h.write_u32(billboard.invisible as u32);
    h.write_u32(billboard.skin_parts_mask as u32);
    h.write_u32(billboard.slim as u32);
    for angle in billboard.cape_rotation {
        h.write_i32((angle * 100.0) as i32);
    }
    h.write_u32(billboard.visual.is_child as u32);
    h.write_u8(billboard.visual.skeleton_type);
    h.write_u32(billboard.visual.horse_type as u32);
    h.write_u32(billboard.visual.horse_variant);
    h.write_u32(billboard.visual.zombie_villager as u32);
    h.write_u32(billboard.visual.zombie_converting as u32);
    h.write_u32(billboard.visual.wolf_tamed as u32);
    h.write_u32(billboard.visual.wolf_angry as u32);
    h.write_u32(billboard.visual.wolf_begging as u32);
    h.write_u8(billboard.visual.wolf_collar);
    h.write_u8(billboard.visual.ocelot_skin);
    h.write_u32(billboard.visual.horse_saddled as u32);
    h.write_u8(billboard.visual.horse_armor);
    h.write_u32(billboard.visual.guardian_elder as u32);
    h.write_u8(billboard.visual.slime_size);
    h.write_u32(billboard.visual.bat_hanging as u32);
    h.write_u8(billboard.visual.villager_profession);
    h.write_u8(billboard.visual.armor_stand_flags);
    for rotation in billboard.visual.armor_stand_rotations {
        for value in rotation {
            h.write_u32(value.to_bits());
        }
    }
    h.write_u32(billboard.visual.creeper_charged as u32);
    h.write_u8(billboard.visual.sheep_color);
    h.write_u32(billboard.visual.pig_saddled as u32);
    h.write_u8(billboard.visual.rabbit_type);
    h.write_u16(billboard.item_id.unwrap_or_default());
    h.write_u16(billboard.item_damage.unwrap_or_default());
    for equipment in billboard.equipment {
        if let Some((item_id, item_damage)) = equipment {
            h.write_u8(1);
            h.write_u16(item_id);
            h.write_u16(item_damage);
        } else {
            h.write_u8(0);
        }
    }
    if let Some(held_item) = billboard.held_item {
        h.write_u16(held_item);
    }
    if let Some(skin_key) = &billboard.skin_key {
        h.write(skin_key.as_bytes());
    }
    h.finish()
}

#[derive(Clone, Debug)]
pub struct SkullRenderEntry {
    pub position: [i32; 3],
    pub block_metadata: u8,
    pub skull_type: u8,
    pub rotation: u8,
    pub skin_key: String,
}

#[derive(Clone, Copy, Debug)]
pub struct ChestRenderEntry {
    pub position: [i32; 3],
    pub block: crate::world::block::Block,
    pub metadata: u8,
    pub lid_angle: f32,
    pub double_x: bool,
    pub double_z: bool,
    pub sky_light: u8,
    pub block_light: u8,
}

#[derive(Clone, Debug)]
pub struct ControlBindingRow {
    pub action: crate::client::keybind::Action,
    pub label: String,
    pub binding: String,
    pub category: String,
    pub conflict: bool,
    pub listening: bool,
    pub is_default: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityBillboardKind {
    Player,
    Hostile,
    Passive,
    Item,
    XpOrb,
    Projectile,
    Other,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn color_subresource() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    }
}

fn depth_subresource() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::DEPTH,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    }
}

pub(crate) fn spirv_words(bytes: &[u8]) -> Vec<u32> {
    assert!(
        bytes.len() % 4 == 0,
        "SPIR-V bytecode length must be a multiple of 4"
    );
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

impl Renderer {
    /// Trigger the first-person hand swing animation (attack/use).
    pub fn trigger_hand_swing(&mut self) {
        self.state.hud.set_hand_swing_timer(0.3);
        self.state.hud.set_hand_swing_progress(0.0);
    }

    /// Reset equipped progress (item switch animation replay).
    /// Called when a block is placed or item is used. Vanilla MCP: `resetEquippedProgress`.
    pub fn reset_equipped_progress(&mut self) {
        self.hand_equip_progress = 0.0;
    }

    pub fn update_hand_state(&mut self, item_id: u16, item_damage: u16, nbt: Option<Vec<u8>>) {
        let now = std::time::Instant::now();
        let dt = now
            .duration_since(self.hand_animation_last_update)
            .as_secs_f32()
            .min(0.1);
        self.hand_animation_last_update = now;

        if self.state.hud.hand_swing_timer() > 0.0 {
            self.state.hud.set_hand_swing_timer((self.state.hud.hand_swing_timer() - dt).max(0.0));
            self.state.hud.set_hand_swing_progress(if self.state.hud.hand_swing_timer() > 0.0 {
                1.0 - self.state.hud.hand_swing_timer() / 0.3
            } else {
                0.0
            });
        } else {
            self.state.hud.set_hand_swing_progress(0.0);
        }

        // Bow damage 1..3 selects pulling textures; it is not an ItemStack
        // change. Treating each pull frame as a new item keeps equip progress
        // near zero and sinks the bow to the bottom of the screen.
        let same_item = self.state.hud.hand_item_id() == item_id
            && (item_id == crate::world::item::Item::Bow.to_id()
                || self.state.hud.hand_item_damage() == item_damage);
        let target = if same_item { 1.0 } else { 0.0 };
        let step = 8.0 * dt;
        if self.hand_equip_progress < target {
            self.hand_equip_progress = (self.hand_equip_progress + step).min(target);
        } else {
            self.hand_equip_progress = (self.hand_equip_progress - step).max(target);
        }

        if !same_item && self.hand_equip_progress <= 0.1 {
            self.state.hud.set_hand_item_id(item_id);
            self.state.hud.set_hand_item_damage(item_damage);
            self.state.hud.set_hand_item_nbt(nbt);
        }
    }

    /// Schedule a resource pack reload — textures will be re-uploaded next frame.
    pub fn schedule_resource_reload(
        &mut self,
        block_atlas: crate::assets::texture::TextureAtlas,
        entity_atlas: crate::render::entity::atlas::EntityTextureAtlas,
        item_atlas: Vec<u8>,
        resolver: &mut crate::assets::resolver::AssetResolver,
    ) {
        // Wait for all in-flight GPU work to finish before modifying any
        // resources.  Destroying an image / buffer that the GPU is still
        // reading from causes ERROR_DEVICE_LOST.
        unsafe {
            self.device
                .device_wait_idle()
                .expect("device_wait_idle during resource reload");
        }

        // Packed atlas dimensions can change with resource-pack resolution,
        // so replace the GPU image instead of writing into the old extent.
        let (block_image_raii, block_view_raii, block_alloc, block_sampler_raii) =
            resources::create_rgba_texture(
                &self.device,
                self.resources.allocator_mut(),
                self.command_pool,
                self.queue,
                &block_atlas.pixels,
                block_atlas.width,
                block_atlas.height,
                "texture",
            );
        let block_image = block_image_raii.into_handle();
        let block_view = block_view_raii.into_handle();
        let block_sampler = block_sampler_raii.into_handle();
        unsafe {
            self.device
                .destroy_image_view(self.texture_image_view, None);
            self.device.destroy_sampler(self.texture_sampler, None);
            self.device.destroy_image(self.texture_image, None);
        }
        if let Some(allocation) = self.texture_alloc.take() {
            self.resources.free(allocation);
        }
        self.texture_image = block_image;
        self.texture_image_view = block_view;
        self.texture_alloc = Some(block_alloc);
        self.texture_sampler = block_sampler;
        self.block_atlas = Some(block_atlas);
        self.block_animation_upload_bytes.clear();
        self.block_animation_uploads.clear();

        // The inventory, held items and dropped generated items share this
        // 128px-per-icon atlas, so update the existing image in place. Its
        // views and descriptor sets remain valid across a resource-pack reload.
        resources::reupload_gpu_image(
            &self.device,
            self.resources.allocator_mut(),
            self.command_pool,
            self.queue,
            self.gui_items_image,
            crate::render::item_icons::ITEM_ATLAS_W,
            crate::render::item_icons::ITEM_ATLAS_H,
            &item_atlas,
        );

        // Destroy old entity texture, create new one
        if self.entity_texture_image != vk::Image::null() {
            unsafe {
                self.device
                    .destroy_image_view(self.entity_texture_view, None);
            }
            unsafe {
                self.device.destroy_image(self.entity_texture_image, None);
            }
            if let Some(a) = self.entity_texture_alloc.take() {
                self.resources.free(a);
            }
            if self.entity_texture_sampler != vk::Sampler::null() {
                unsafe {
                    self.device
                        .destroy_sampler(self.entity_texture_sampler, None);
                }
            }
        }
        let (e_img_raii, e_view_raii, e_alloc, e_sampler_raii) = Self::create_entity_texture(
            &self.device,
            self.resources.allocator_mut(),
            self.command_pool,
            self.queue,
            &entity_atlas,
        );
        self.entity_texture_image = e_img_raii.into_handle();
        self.entity_texture_view = e_view_raii.into_handle();
        self.entity_texture_alloc = Some(e_alloc);
        self.entity_texture_sampler = e_sampler_raii.into_handle();
        self.entity_atlas = Some(entity_atlas);
        self.synced_player_skin_content_hash = None;
        self.player_skin_layout_hash = 0;
        self.entity_atlas_generation = self.entity_atlas_generation.wrapping_add(1);
        for mesh in self.entity_gpu_meshes.values_mut() {
            mesh.state_hash.fill(None);
        }

        // Update descriptor sets with new textures
        Self::write_descriptors(
            &self.device,
            &self.descriptor_sets,
            self.resources.uniform_buffers(),
            self.texture_image_view,
            self.texture_sampler,
        );
        self.refresh_gui_block_texture_descriptors();
        Self::write_descriptors(
            &self.device,
            &self.entity_descriptor_sets,
            self.resources.uniform_buffers(),
            self.entity_texture_view,
            self.entity_texture_sampler,
        );
        Self::write_descriptors(
            &self.device,
            &self.fp_item_descriptor_sets,
            self.resources.uniform_buffers(),
            self.gui_items_view,
            self.gui_items_sampler,
        );

        // Rebuild GUI textures at the pack's actual resolution and rebind
        // their descriptor sets (implemented in gui::runtime).
        self.reload_gui_textures(resolver);

        self.block_dirty = true;
        log::info!("resource pack textures uploaded to GPU");
    }
}
