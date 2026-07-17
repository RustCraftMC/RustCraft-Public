//! Buffer creation, texture atlas, uniform buffers.

use ash::vk;

use super::{color_subresource, Uniforms, MAX_FRAMES};
use crate::assets::texture;

pub(super) fn upload_dynamic_buffer(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    buffer: &mut Option<vk::Buffer>,
    allocation: &mut Option<gpu_allocator::vulkan::Allocation>,
    capacity: &mut u64,
    usage: vk::BufferUsageFlags,
    data: &[u8],
) {
    if data.is_empty() {
        return;
    }

    let size = data.len() as u64;
    if buffer.is_none() || *capacity < size {
        if let Some(old) = buffer.take() {
            unsafe {
                device.destroy_buffer(old, None);
            }
        }
        if let Some(old) = allocation.take() {
            allocator.free(old).ok();
        }

        let new_capacity = size.next_power_of_two().max(256);
        let buf = unsafe {
            device
                .create_buffer(
                    &vk::BufferCreateInfo {
                        size: new_capacity,
                        usage,
                        ..Default::default()
                    },
                    None,
                )
                .unwrap()
        };
        let reqs = unsafe { device.get_buffer_memory_requirements(buf) };
        let alloc = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "dynamic_mesh_buf",
                requirements: reqs,
                location: gpu_allocator::MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            device
                .bind_buffer_memory(buf, alloc.memory(), alloc.offset())
                .unwrap();
        }
        *buffer = Some(buf);
        *allocation = Some(alloc);
        *capacity = new_capacity;
    }

    let alloc = allocation
        .as_ref()
        .expect("dynamic buffer allocation must exist");
    unsafe {
        let ptr = device
            .map_memory(
                alloc.memory(),
                alloc.offset(),
                size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
        device.unmap_memory(alloc.memory());
    }
}

pub(super) fn create_rgba_texture(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    pixels: &[u8],
    width: u32,
    height: u32,
    allocation_name: &'static str,
) -> (
    vk::Image,
    vk::ImageView,
    gpu_allocator::vulkan::Allocation,
    vk::Sampler,
) {
    let size = (width * height * 4) as u64;
    assert_eq!(pixels.len() as u64, size);
    let image = unsafe {
        device.create_image(
            &vk::ImageCreateInfo {
                image_type: vk::ImageType::TYPE_2D,
                format: vk::Format::R8G8B8A8_SRGB,
                extent: vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                },
                mip_levels: 1,
                array_layers: 1,
                samples: vk::SampleCountFlags::TYPE_1,
                usage: vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::TRANSFER_SRC,
                ..Default::default()
            },
            None,
        )
    }
    .unwrap();
    let requirements = unsafe { device.get_image_memory_requirements(image) };
    let allocation = allocator
        .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
            name: allocation_name,
            requirements,
            location: gpu_allocator::MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
        })
        .unwrap();
    unsafe {
        device
            .bind_image_memory(image, allocation.memory(), allocation.offset())
            .unwrap();
    }

    let staging = unsafe {
        device.create_buffer(
            &vk::BufferCreateInfo {
                size,
                usage: vk::BufferUsageFlags::TRANSFER_SRC,
                ..Default::default()
            },
            None,
        )
    }
    .unwrap();
    let staging_requirements = unsafe { device.get_buffer_memory_requirements(staging) };
    let staging_allocation = allocator
        .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
            name: "texture_staging",
            requirements: staging_requirements,
            location: gpu_allocator::MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
        })
        .unwrap();
    unsafe {
        device
            .bind_buffer_memory(
                staging,
                staging_allocation.memory(),
                staging_allocation.offset(),
            )
            .unwrap();
        let mapped = device
            .map_memory(
                staging_allocation.memory(),
                staging_allocation.offset(),
                size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), mapped as *mut u8, pixels.len());
        device.unmap_memory(staging_allocation.memory());
    }

    let command_buffer = unsafe {
        device.allocate_command_buffers(&vk::CommandBufferAllocateInfo {
            command_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: 1,
            ..Default::default()
        })
    }
    .unwrap()[0];
    unsafe {
        device
            .begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo {
                    flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                    ..Default::default()
                },
            )
            .unwrap();
        device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[vk::ImageMemoryBarrier {
                dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                old_layout: vk::ImageLayout::UNDEFINED,
                new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                image,
                subresource_range: color_subresource(),
                ..Default::default()
            }],
        );
        device.cmd_copy_buffer_to_image(
            command_buffer,
            staging,
            image,
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
        device.cmd_pipeline_barrier(
            command_buffer,
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
                image,
                subresource_range: color_subresource(),
                ..Default::default()
            }],
        );
        device.end_command_buffer(command_buffer).unwrap();
        let fence = device
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .unwrap();
        device
            .queue_submit(
                queue,
                &[vk::SubmitInfo {
                    command_buffer_count: 1,
                    p_command_buffers: &command_buffer,
                    ..Default::default()
                }],
                fence,
            )
            .unwrap();
        device.wait_for_fences(&[fence], true, u64::MAX).unwrap();
        device.destroy_fence(fence, None);
        device.free_command_buffers(command_pool, &[command_buffer]);
        device.destroy_buffer(staging, None);
        allocator.free(staging_allocation).unwrap();
    }

    let view = unsafe {
        device.create_image_view(
            &vk::ImageViewCreateInfo {
                image,
                view_type: vk::ImageViewType::TYPE_2D,
                format: vk::Format::R8G8B8A8_SRGB,
                subresource_range: color_subresource(),
                ..Default::default()
            },
            None,
        )
    }
    .unwrap();
    let sampler = unsafe {
        device.create_sampler(
            &vk::SamplerCreateInfo {
                mag_filter: vk::Filter::NEAREST,
                min_filter: vk::Filter::NEAREST,
                address_mode_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                address_mode_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                anisotropy_enable: vk::FALSE,
                max_anisotropy: 1.0,
                ..Default::default()
            },
            None,
        )
    }
    .unwrap();

    (image, view, allocation, sampler)
}

impl super::Renderer {
    // --- Uniform buffers ---

    pub(super) fn create_uniform_buffers(
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
    ) -> (
        Vec<vk::Buffer>,
        Vec<gpu_allocator::vulkan::Allocation>,
        Vec<*mut std::ffi::c_void>,
    ) {
        let mut bufs = Vec::with_capacity(MAX_FRAMES);
        let mut allocs = Vec::with_capacity(MAX_FRAMES);
        let mut mapped = Vec::with_capacity(MAX_FRAMES);
        let size = std::mem::size_of::<Uniforms>() as u64;
        for _ in 0..MAX_FRAMES {
            let buf = unsafe {
                device.create_buffer(
                    &vk::BufferCreateInfo {
                        size,
                        usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
                        ..Default::default()
                    },
                    None,
                )
            }
            .unwrap();
            let reqs = unsafe { device.get_buffer_memory_requirements(buf) };
            let alloc = allocator
                .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                    name: "uniform",
                    requirements: reqs,
                    location: gpu_allocator::MemoryLocation::CpuToGpu,
                    linear: true,
                    allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                })
                .unwrap();
            unsafe {
                device
                    .bind_buffer_memory(buf, alloc.memory(), alloc.offset())
                    .unwrap();
                let ptr = device
                    .map_memory(
                        alloc.memory(),
                        alloc.offset(),
                        size,
                        vk::MemoryMapFlags::empty(),
                    )
                    .unwrap();
                mapped.push(ptr);
            };
            bufs.push(buf);
            allocs.push(alloc);
        }
        (bufs, allocs, mapped)
    }

    // --- Standalone device buffer helper (used before Renderer is constructed) ---

    pub(super) fn create_buffer_standalone(
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        size: u64,
        usage: vk::BufferUsageFlags,
        data: &[u8],
    ) -> (vk::Buffer, gpu_allocator::vulkan::Allocation) {
        let buf = unsafe {
            device.create_buffer(
                &vk::BufferCreateInfo {
                    size,
                    usage,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let reqs = unsafe { device.get_buffer_memory_requirements(buf) };
        let alloc = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "staging",
                requirements: reqs,
                location: gpu_allocator::MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            device
                .bind_buffer_memory(buf, alloc.memory(), alloc.offset())
                .unwrap()
        };
        if !data.is_empty() {
            unsafe {
                let mapped = device
                    .map_memory(
                        alloc.memory(),
                        alloc.offset(),
                        size,
                        vk::MemoryMapFlags::empty(),
                    )
                    .unwrap();
                std::ptr::copy_nonoverlapping(data.as_ptr(), mapped as *mut u8, data.len());
                device.unmap_memory(alloc.memory());
            }
        }
        (buf, alloc)
    }

    pub(super) fn create_empty_gpu_buffer_standalone(
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        size: u64,
        usage: vk::BufferUsageFlags,
        name: &'static str,
    ) -> (vk::Buffer, gpu_allocator::vulkan::Allocation) {
        let buffer = unsafe {
            device.create_buffer(
                &vk::BufferCreateInfo {
                    size,
                    usage,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        let allocation = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name,
                requirements,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            device
                .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .unwrap();
        }
        (buffer, allocation)
    }

    // --- Device buffer helper ---

    pub(super) fn create_device_buffer(
        &mut self,
        size: u64,
        usage: vk::BufferUsageFlags,
        data: &[u8],
    ) -> (vk::Buffer, gpu_allocator::vulkan::Allocation) {
        if size == 0 {
            return (
                vk::Buffer::null(),
                gpu_allocator::vulkan::Allocation::default(),
            );
        }
        let buf = unsafe {
            self.device.create_buffer(
                &vk::BufferCreateInfo {
                    size,
                    usage,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let reqs = unsafe { self.device.get_buffer_memory_requirements(buf) };
        let alloc = self
            .allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "mesh_buf",
                requirements: reqs,
                location: gpu_allocator::MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            self.device
                .bind_buffer_memory(buf, alloc.memory(), alloc.offset())
                .unwrap();
            let ptr = self
                .device
                .map_memory(
                    alloc.memory(),
                    alloc.offset(),
                    size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
            self.device.unmap_memory(alloc.memory());
        }
        (buf, alloc)
    }

    // --- Texture atlas ---

    pub(super) fn create_texture_atlas(
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
        resolver: &mut crate::assets::resolver::AssetResolver,
    ) -> (
        vk::Image,
        vk::ImageView,
        gpu_allocator::vulkan::Allocation,
        vk::Sampler,
        texture::TextureAtlas,
    ) {
        let atlas = texture::TextureAtlas::load_with_resolver(resolver);
        texture::init_texture_map(&atlas);

        let mut model_registry = crate::assets::model::ModelRegistry::new();
        model_registry.load_with_resolver(resolver);
        model_registry.texture_map = atlas.name_to_index.clone();
        let model_cache = crate::world::block_models::BlockModelCache::build(
            &mut model_registry,
            atlas.name_to_index.clone(),
        );
        crate::world::block_models::BlockModelCache::init(model_cache);

        let (image, view, allocation, sampler) = create_rgba_texture(
            device,
            allocator,
            command_pool,
            queue,
            &atlas.pixels,
            atlas.width,
            atlas.height,
            "texture",
        );
        (image, view, allocation, sampler, atlas)
    }

    /// Advance block animations at vanilla's 20 Hz client-tick cadence and
    /// stage only tiles whose frame actually changed. The upload is recorded
    /// into the normal frame command buffer by `record_block_animation_uploads`.
    pub(super) fn prepare_block_animation_uploads(&mut self, frame: usize) {
        const CLIENT_TICK: std::time::Duration = std::time::Duration::from_millis(50);
        const MAX_CATCH_UP_TICKS: u32 = 20;

        self.block_animation_upload_bytes.clear();
        self.block_animation_uploads.clear();

        let now = std::time::Instant::now();
        let elapsed = now.saturating_duration_since(self.block_animation_last_tick);
        let due = (elapsed.as_nanos() / CLIENT_TICK.as_nanos()) as u32;
        if due == 0 {
            return;
        }
        let ticks = due.min(MAX_CATCH_UP_TICKS);
        self.block_animation_last_tick += CLIENT_TICK * ticks;
        if due > MAX_CATCH_UP_TICKS {
            self.block_animation_last_tick = now;
        }

        let Some(atlas) = self.block_atlas.as_mut() else {
            return;
        };
        for _ in 0..ticks {
            atlas.animate_tick_into(
                &mut self.block_animation_upload_bytes,
                &mut self.block_animation_uploads,
            );
        }
        if self.block_animation_upload_bytes.is_empty() {
            return;
        }

        upload_dynamic_buffer(
            &self.device,
            &mut self.allocator,
            &mut self.block_animation_buffers[frame],
            &mut self.block_animation_allocs[frame],
            &mut self.block_animation_capacities[frame],
            vk::BufferUsageFlags::TRANSFER_SRC,
            &self.block_animation_upload_bytes,
        );
    }

    pub(super) fn record_block_animation_uploads(&self, cb: vk::CommandBuffer, frame: usize) {
        if self.block_animation_uploads.is_empty() {
            return;
        }
        let Some(staging) = self.block_animation_buffers[frame] else {
            return;
        };
        let copies = self
            .block_animation_uploads
            .iter()
            .map(|upload| vk::BufferImageCopy {
                buffer_offset: upload.buffer_offset,
                image_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    layer_count: 1,
                    ..Default::default()
                },
                image_offset: vk::Offset3D {
                    x: upload.pixel_x as i32,
                    y: upload.pixel_y as i32,
                    z: 0,
                },
                image_extent: vk::Extent3D {
                    width: upload.width,
                    height: upload.height,
                    depth: 1,
                },
                ..Default::default()
            })
            .collect::<Vec<_>>();

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
                    image: self.texture_image,
                    subresource_range: super::color_subresource(),
                    ..Default::default()
                }],
            );
            self.device.cmd_copy_buffer_to_image(
                cb,
                staging,
                self.texture_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &copies,
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
                    image: self.texture_image,
                    subresource_range: super::color_subresource(),
                    ..Default::default()
                }],
            );
        }
    }

    // --- Entity texture atlas ---

    pub(super) fn create_entity_texture(
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
        atlas: &super::entity::atlas::EntityTextureAtlas,
    ) -> (
        vk::Image,
        vk::ImageView,
        gpu_allocator::vulkan::Allocation,
        vk::Sampler,
    ) {
        use super::entity::atlas::ENTITY_ATLAS_SIZE;
        create_rgba_texture(
            device,
            allocator,
            command_pool,
            queue,
            &atlas.pixels,
            ENTITY_ATLAS_SIZE,
            ENTITY_ATLAS_SIZE,
            "entity_texture",
        )
    }
}

/// Re-upload raw RGBA pixel data to an existing GPU image (resource pack reload).
pub fn reupload_gpu_image(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    image: vk::Image,
    w: u32,
    h: u32,
    pixels: &[u8],
) {
    upload_gpu_image_with_layout(
        device,
        allocator,
        command_pool,
        queue,
        image,
        0,
        0,
        w,
        h,
        pixels,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
}

/// Re-upload tightly packed RGBA pixels to a region of an existing GPU image.
pub fn reupload_gpu_image_region(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    image: vk::Image,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    pixels: &[u8],
) {
    upload_gpu_image_with_layout(
        device,
        allocator,
        command_pool,
        queue,
        image,
        x,
        y,
        w,
        h,
        pixels,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
}

pub fn upload_new_gpu_image(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    image: vk::Image,
    w: u32,
    h: u32,
    pixels: &[u8],
) {
    upload_gpu_image_with_layout(
        device,
        allocator,
        command_pool,
        queue,
        image,
        0,
        0,
        w,
        h,
        pixels,
        vk::ImageLayout::UNDEFINED,
    );
}

fn upload_gpu_image_with_layout(
    device: &ash::Device,
    allocator: &mut gpu_allocator::vulkan::Allocator,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    image: vk::Image,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    pixels: &[u8],
    old_layout: vk::ImageLayout,
) {
    assert!(w > 0 && h > 0, "GPU image upload extent must be non-zero");
    assert!(
        x <= i32::MAX as u32 && y <= i32::MAX as u32,
        "GPU image upload offset exceeds Vulkan's signed coordinate range"
    );
    let expected_len = (w as usize)
        .checked_mul(h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .expect("GPU image upload size overflow");
    assert_eq!(
        pixels.len(),
        expected_len,
        "GPU image upload requires tightly packed RGBA pixels"
    );
    let size = expected_len as u64;
    let staging = unsafe {
        device
            .create_buffer(
                &vk::BufferCreateInfo {
                    size,
                    usage: vk::BufferUsageFlags::TRANSFER_SRC,
                    ..Default::default()
                },
                None,
            )
            .unwrap()
    };
    let s_reqs = unsafe { device.get_buffer_memory_requirements(staging) };
    let s_alloc = allocator
        .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
            name: "rp-staging",
            requirements: s_reqs,
            location: gpu_allocator::MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
        })
        .unwrap();
    unsafe {
        device
            .bind_buffer_memory(staging, s_alloc.memory(), s_alloc.offset())
            .unwrap();
    }
    unsafe {
        let ptr = device
            .map_memory(
                s_alloc.memory(),
                s_alloc.offset(),
                size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), ptr as *mut u8, pixels.len());
        device.unmap_memory(s_alloc.memory());
    }
    let cb = unsafe {
        device
            .allocate_command_buffers(&vk::CommandBufferAllocateInfo {
                command_pool,
                level: vk::CommandBufferLevel::PRIMARY,
                command_buffer_count: 1,
                ..Default::default()
            })
            .unwrap()[0]
    };
    unsafe {
        device
            .begin_command_buffer(
                cb,
                &vk::CommandBufferBeginInfo {
                    flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                    ..Default::default()
                },
            )
            .unwrap();
        let (source_stage, source_access) = if old_layout == vk::ImageLayout::UNDEFINED {
            (
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::AccessFlags::empty(),
            )
        } else {
            (
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::SHADER_READ,
            )
        };
        device.cmd_pipeline_barrier(
            cb,
            source_stage,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[vk::ImageMemoryBarrier {
                src_access_mask: source_access,
                dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                old_layout,
                new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                image,
                subresource_range: color_subresource(),
                ..Default::default()
            }],
        );
        device.cmd_copy_buffer_to_image(
            cb,
            staging,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[vk::BufferImageCopy {
                image_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    layer_count: 1,
                    ..Default::default()
                },
                image_offset: vk::Offset3D {
                    x: x as i32,
                    y: y as i32,
                    z: 0,
                },
                image_extent: vk::Extent3D {
                    width: w,
                    height: h,
                    depth: 1,
                },
                ..Default::default()
            }],
        );
        device.cmd_pipeline_barrier(
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
                image,
                subresource_range: color_subresource(),
                ..Default::default()
            }],
        );
        device.end_command_buffer(cb).unwrap();
        let fence = device
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .unwrap();
        device
            .queue_submit(
                queue,
                &[vk::SubmitInfo {
                    command_buffer_count: 1,
                    p_command_buffers: &cb,
                    ..Default::default()
                }],
                fence,
            )
            .unwrap();
        device.wait_for_fences(&[fence], true, u64::MAX).unwrap();
        device.destroy_fence(fence, None);
        device.free_command_buffers(command_pool, &[cb]);
        device.destroy_buffer(staging, None);
    }
    allocator.free(s_alloc).unwrap();
}
