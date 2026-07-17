//! Instance, device, surface, swapchain, depth buffer, sync objects, framebuffers.

use ash::vk;
use std::ffi::CString;

use super::{
    color_subresource, depth_subresource, BufferRangeAllocator, ChunkStorage, Renderer,
    CHUNK_INDEX_ARENA_BYTES, CHUNK_VERTEX_ARENA_BYTES, MAX_FRAMES,
};

/// Find and enable the Khronos validation layer if available.
fn find_validation_layer(entry: &ash::Entry) -> Option<CString> {
    let layer_name = CString::new("VK_LAYER_KHRONOS_validation").unwrap();
    unsafe {
        if let Ok(props) = entry.enumerate_instance_layer_properties() {
            for p in &props {
                let name = std::ffi::CStr::from_ptr(p.layer_name.as_ptr());
                if name == layer_name.as_c_str() {
                    return Some(layer_name);
                }
            }
        }
    }
    None
}

fn supports_vulkan_ray_tracing(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> bool {
    let Ok(extensions) =
        (unsafe { instance.enumerate_device_extension_properties(physical_device) })
    else {
        return false;
    };
    let has_extension = |required: &std::ffi::CStr| unsafe {
        extensions.iter().any(|extension| {
            std::ffi::CStr::from_ptr(extension.extension_name.as_ptr()) == required
        })
    };
    if !has_extension(ash::khr::deferred_host_operations::NAME)
        || !has_extension(ash::khr::acceleration_structure::NAME)
        || !has_extension(ash::khr::ray_tracing_pipeline::NAME)
    {
        return false;
    }

    let mut acceleration = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
    let mut ray_pipeline = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
    let mut buffer_address = vk::PhysicalDeviceBufferDeviceAddressFeatures::default();
    let mut features = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut buffer_address)
        .push_next(&mut acceleration)
        .push_next(&mut ray_pipeline);
    unsafe { instance.get_physical_device_features2(physical_device, &mut features) };
    buffer_address.buffer_device_address == vk::TRUE
        && acceleration.acceleration_structure == vk::TRUE
        && ray_pipeline.ray_tracing_pipeline == vk::TRUE
}

unsafe extern "system" fn debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    if !p_callback_data.is_null() {
        let data = &*p_callback_data;
        let msg = std::ffi::CStr::from_ptr(data.p_message);
        let message = msg.to_string_lossy();
        if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
            log::error!(target: "rustcraft::vulkan_validation", "types={message_types:?} {message}");
        } else if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
            log::warn!(target: "rustcraft::vulkan_validation", "types={message_types:?} {message}");
        } else {
            log::debug!(target: "rustcraft::vulkan_validation", "types={message_types:?} {message}");
        }
    }
    vk::FALSE
}

impl Renderer {
    pub fn new(
        window: &winit::window::Window,
        resolver: &mut crate::assets::resolver::AssetResolver,
        selected_shader_pack: Option<&str>,
    ) -> Self {
        use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

        let init_started = std::time::Instant::now();
        let win_size = window.inner_size();
        let window_size = (win_size.width, win_size.height);
        log::info!(
            "initialising Vulkan renderer: window={}x{}, requested_api=1.3",
            window_size.0,
            window_size.1
        );
        let entry = ash::Entry::linked();

        // Instance — enable required surface extensions + validation layer if available
        let app_name = CString::new("RustCraft").unwrap();
        let app_info = vk::ApplicationInfo {
            p_application_name: app_name.as_ptr(),
            api_version: vk::API_VERSION_1_3,
            ..Default::default()
        };
        let mut surface_extensions: Vec<*const i8> =
            ash_window::enumerate_required_extensions(window.display_handle().unwrap().as_raw())
                .unwrap()
                .to_vec();

        // Validation layer disabled for performance (was causing 10x FPS drop)
        // Uncomment to re-enable for debugging:
        // let validation_layer = find_validation_layer(&entry);
        let validation_layer: Option<CString> = None;
        let debug_utils_ext = CString::new("VK_EXT_debug_utils").unwrap();
        if validation_layer.is_some() {
            surface_extensions.push(debug_utils_ext.as_ptr());
            log::info!("Vulkan validation layer enabled");
        } else {
            log::debug!("Vulkan validation layer disabled");
        }
        log::debug!(
            "Vulkan instance extensions requested: {}",
            surface_extensions.len()
        );
        let enabled_layers: Vec<*const std::ffi::c_char> =
            validation_layer.iter().map(|l| l.as_ptr()).collect();
        let layer_count = enabled_layers.len() as u32;
        let layer_ptr = if layer_count > 0 {
            enabled_layers.as_ptr()
        } else {
            std::ptr::null()
        };

        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo {
                    p_application_info: &app_info,
                    enabled_extension_count: surface_extensions.len() as u32,
                    pp_enabled_extension_names: surface_extensions.as_ptr(),
                    enabled_layer_count: layer_count,
                    pp_enabled_layer_names: layer_ptr,
                    ..Default::default()
                },
                None,
            )
        }
        .expect("Failed to create instance");

        // Create debug messenger if validation layer is active
        if validation_layer.is_some() {
            let debug_utils = ash::ext::debug_utils::Instance::new(&entry, &instance);
            unsafe {
                let _messenger = debug_utils
                    .create_debug_utils_messenger(
                        &vk::DebugUtilsMessengerCreateInfoEXT {
                            message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                            message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                            pfn_user_callback: Some(debug_callback),
                            ..Default::default()
                        },
                        None,
                    )
                    .unwrap();
                // Intentionally keep the debug loader alive for program lifetime.
                let _ = _messenger;
                std::mem::forget(debug_utils);
            }
        }

        // Surface
        let surface = unsafe {
            let dh = window.display_handle().unwrap().as_raw();
            let wh = window.window_handle().unwrap().as_raw();
            ash_window::create_surface(&entry, &instance, dh, wh, None)
        }
        .expect("Failed to create surface");
        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);

        // Physical device
        let (physical_device, queue_family) = unsafe {
            instance
                .enumerate_physical_devices()
                .expect("enumerate_physical_devices")
                .into_iter()
                .filter_map(|pd| {
                    let queue_family = instance
                        .get_physical_device_queue_family_properties(pd)
                        .iter()
                        .enumerate()
                        .find(|(i, q)| {
                            q.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                && surface_fn
                                    .get_physical_device_surface_support(pd, *i as u32, surface)
                                    .unwrap_or(false)
                        })
                        .map(|(i, _)| i as u32)?;
                    let features = instance.get_physical_device_features(pd);
                    if features.sampler_anisotropy != vk::TRUE {
                        return None;
                    }
                    let properties = instance.get_physical_device_properties(pd);
                    let device_score = match properties.device_type {
                        vk::PhysicalDeviceType::DISCRETE_GPU => 4u64,
                        vk::PhysicalDeviceType::INTEGRATED_GPU => 3,
                        vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
                        vk::PhysicalDeviceType::CPU => 1,
                        _ => 0,
                    } << 60;
                    let limits_score = properties.limits.max_image_dimension2_d as u64;
                    Some((pd, queue_family, device_score | limits_score))
                })
                .max_by_key(|(_, _, score)| *score)
                .map(|(pd, queue_family, _)| (pd, queue_family))
                .expect("No suitable GPU")
        };

        let props = unsafe { instance.get_physical_device_properties(physical_device) };
        let name =
            unsafe { std::ffi::CStr::from_ptr(props.device_name.as_ptr()) }.to_string_lossy();
        let memory = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let local_memory_bytes = memory.memory_heaps[..memory.memory_heap_count as usize]
            .iter()
            .filter(|heap| heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
            .map(|heap| heap.size)
            .sum::<u64>();
        log::info!(
            "selected GPU: name='{}', type={:?}, vendor=0x{:04X}, device=0x{:04X}, api={}.{}.{}, driver=0x{:08X}, local_memory={} MiB, queue_family={}",
            name,
            props.device_type,
            props.vendor_id,
            props.device_id,
            vk::api_version_major(props.api_version),
            vk::api_version_minor(props.api_version),
            vk::api_version_patch(props.api_version),
            props.driver_version,
            local_memory_bytes / (1024 * 1024),
            queue_family
        );

        // Device
        let queue_prio = [1.0f32];
        let queue_info = vk::DeviceQueueCreateInfo {
            queue_family_index: queue_family,
            queue_count: 1,
            p_queue_priorities: queue_prio.as_ptr(),
            ..Default::default()
        };
        let ray_tracing_available = supports_vulkan_ray_tracing(&instance, physical_device);
        let mut extension_names = vec![ash::khr::swapchain::NAME.as_ptr()];
        if ray_tracing_available {
            extension_names.extend([
                ash::khr::deferred_host_operations::NAME.as_ptr(),
                ash::khr::acceleration_structure::NAME.as_ptr(),
                ash::khr::ray_tracing_pipeline::NAME.as_ptr(),
            ]);
        }
        let supported_features = unsafe { instance.get_physical_device_features(physical_device) };
        let mut features = vk::PhysicalDeviceFeatures::default();
        features.sampler_anisotropy = vk::TRUE;
        features.multi_draw_indirect = supported_features.multi_draw_indirect;
        let multi_draw_indirect = features.multi_draw_indirect == vk::TRUE;
        let mut buffer_device_address_features = vk::PhysicalDeviceBufferDeviceAddressFeatures {
            buffer_device_address: if ray_tracing_available {
                vk::TRUE
            } else {
                vk::FALSE
            },
            ..Default::default()
        };
        let mut acceleration_structure_features =
            vk::PhysicalDeviceAccelerationStructureFeaturesKHR {
                acceleration_structure: if ray_tracing_available {
                    vk::TRUE
                } else {
                    vk::FALSE
                },
                ..Default::default()
            };
        let mut ray_tracing_pipeline_features = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR {
            ray_tracing_pipeline: if ray_tracing_available {
                vk::TRUE
            } else {
                vk::FALSE
            },
            ..Default::default()
        };
        let mut device_info = vk::DeviceCreateInfo {
            queue_create_info_count: 1,
            p_queue_create_infos: &queue_info,
            enabled_extension_count: extension_names.len() as u32,
            pp_enabled_extension_names: extension_names.as_ptr(),
            p_enabled_features: &features,
            ..Default::default()
        };
        if ray_tracing_available {
            device_info = device_info
                .push_next(&mut buffer_device_address_features)
                .push_next(&mut acceleration_structure_features)
                .push_next(&mut ray_tracing_pipeline_features);
        }
        let device = unsafe { instance.create_device(physical_device, &device_info, None) }
            .expect("Failed to create device");
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let fsr3_status = super::upscaler::Fsr3Status::detect();
        let render_capabilities = super::shader_pack::RenderCapabilities {
            ray_tracing: ray_tracing_available,
            fsr3: fsr3_status.available,
        };
        log::info!(
            "render capabilities: vulkan_ray_tracing={}, fsr3={} ({})",
            render_capabilities.ray_tracing,
            render_capabilities.fsr3,
            fsr3_status.reason
        );
        let shader_pack = super::shader_pack::ShaderPackShaders::load_selected(
            selected_shader_pack,
            render_capabilities,
        );

        // Allocator
        let mut allocator =
            gpu_allocator::vulkan::Allocator::new(&gpu_allocator::vulkan::AllocatorCreateDesc {
                instance: instance.clone(),
                device: device.clone(),
                physical_device,
                debug_settings: Default::default(),
                buffer_device_address: ray_tracing_available,
                allocation_sizes: Default::default(),
            })
            .expect("Failed to create allocator");

        // Swapchain
        let swapchain_fn = ash::khr::swapchain::Device::new(&instance, &device);
        let (swapchain, swapchain_images, swapchain_views, swapchain_format, swapchain_extent) =
            Self::create_swapchain(
                &surface_fn,
                &swapchain_fn,
                &device,
                physical_device,
                surface,
                window_size,
                vk::SwapchainKHR::null(),
            );

        // Depth
        let depth_format = vk::Format::D32_SFLOAT;
        let (depth_images, depth_image_views, depth_allocs) = Self::create_depth_buffers(
            &device,
            &mut allocator,
            depth_format,
            swapchain_extent,
            swapchain_views.len(),
        );

        // Render pass & pipeline
        let render_pass = Self::create_render_pass(&device, swapchain_format, depth_format);
        let (pipeline, pipeline_layout, descriptor_layout) = Self::create_pipeline(
            &device,
            render_pass,
            true,
            false,
            vk::CullModeFlags::BACK,
            &shader_pack,
        );

        // Allow translucent geometry (water, glass, leaves) to blend without
        // occluding what was already drawn behind them.
        let (transparent_pipeline, _, _) = Self::create_pipeline(
            &device,
            render_pass,
            false,
            true,
            vk::CullModeFlags::NONE,
            &shader_pack,
        );

        // Entity pipeline (reuses same descriptor layout)
        let (entity_pipeline, entity_pipeline_layout) = Self::create_entity_pipeline(
            &device,
            render_pass,
            descriptor_layout,
            true,
            &shader_pack,
        );
        let (particle_pipeline, particle_pipeline_layout) = Self::create_entity_pipeline(
            &device,
            render_pass,
            descriptor_layout,
            false,
            &shader_pack,
        );
        let (nametag_pipeline, nametag_pipeline_layout) =
            Self::create_nametag_pipeline(&device, render_pass, descriptor_layout, &shader_pack);

        // Sky pipeline
        let (sky_pipeline, sky_pipeline_layout, sky_desc_layout) =
            Self::create_sky_pipeline(&device, render_pass, &shader_pack);

        // Command pool
        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo {
                    queue_family_index: queue_family,
                    flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        // Framebuffers
        let framebuffers = Self::create_framebuffers(
            &device,
            render_pass,
            &swapchain_views,
            &depth_image_views,
            swapchain_extent,
        );

        // Command buffers
        let command_buffers = unsafe {
            device.allocate_command_buffers(&vk::CommandBufferAllocateInfo {
                command_pool,
                level: vk::CommandBufferLevel::PRIMARY,
                command_buffer_count: MAX_FRAMES as u32,
                ..Default::default()
            })
        }
        .unwrap();

        // Sync
        let (image_available, render_finished, in_flight_fences) =
            Self::create_sync_objects(&device);

        // Uniforms
        let (uniform_buffers, uniform_allocs, uniform_mapped) =
            Self::create_uniform_buffers(&device, &mut allocator);

        // Descriptors
        let (descriptor_pool, descriptor_sets) =
            Self::create_descriptors(&device, descriptor_layout, &uniform_buffers);

        // Texture atlas
        let (texture_image, texture_image_view, texture_alloc, texture_sampler, block_atlas) =
            Self::create_texture_atlas(&device, &mut allocator, command_pool, queue, resolver);

        Self::write_descriptors(
            &device,
            &descriptor_sets,
            &uniform_buffers,
            texture_image_view,
            texture_sampler,
        );

        // Entity texture atlas
        let entity_atlas = super::entity::atlas::EntityTextureAtlas::load_with_resolver(resolver);
        super::item_icons::precompute_item_meshes_with_resolver(resolver);
        let (
            entity_texture_image,
            entity_texture_view,
            entity_texture_alloc,
            entity_texture_sampler,
        ) = Self::create_entity_texture(
            &device,
            &mut allocator,
            command_pool,
            queue,
            &entity_atlas,
        );

        // Entity descriptor pool and sets (same layout as world, different texture)
        let (entity_descriptor_pool, entity_descriptor_sets) =
            Self::create_descriptors(&device, descriptor_layout, &uniform_buffers);

        Self::write_descriptors(
            &device,
            &entity_descriptor_sets,
            &uniform_buffers,
            entity_texture_view,
            entity_texture_sampler,
        );

        // Item descriptor sets (for first-person held items using the item/entity atlas)
        let (fp_item_descriptor_pool, fp_item_descriptor_sets) =
            Self::create_descriptors(&device, descriptor_layout, &uniform_buffers);
        Self::write_descriptors(
            &device,
            &fp_item_descriptor_sets,
            &uniform_buffers,
            entity_texture_view,
            entity_texture_sampler,
        );

        // Skin texture (for first person arm)
        let skin_reqs = vk::ImageCreateInfo {
            image_type: vk::ImageType::TYPE_2D,
            format: vk::Format::R8G8B8A8_SRGB,
            extent: vk::Extent3D {
                width: 64,
                height: 64,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            ..Default::default()
        };
        let skin_texture_image = unsafe { device.create_image(&skin_reqs, None) }.unwrap();
        let s_mem_reqs = unsafe { device.get_image_memory_requirements(skin_texture_image) };
        let skin_texture_alloc = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "skin_texture",
                requirements: s_mem_reqs,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            device
                .bind_image_memory(
                    skin_texture_image,
                    skin_texture_alloc.memory(),
                    skin_texture_alloc.offset(),
                )
                .unwrap();
        }
        let skin_texture_view = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo {
                    image: skin_texture_image,
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: vk::Format::R8G8B8A8_SRGB,
                    subresource_range: color_subresource(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        let default_skin = crate::assets::skin::PlayerSkin::default_steve();
        super::resources::upload_new_gpu_image(
            &device,
            &mut allocator,
            command_pool,
            queue,
            skin_texture_image,
            64,
            64,
            &default_skin.pixels,
        );

        let (_, skin_descriptor_sets) =
            Self::create_descriptors(&device, descriptor_layout, &uniform_buffers);
        Self::write_descriptors(
            &device,
            &skin_descriptor_sets,
            &uniform_buffers,
            skin_texture_view,
            entity_texture_sampler,
        );

        // Sun and moon textures for sky rendering
        let mut load_sky_texture =
            |path: &str| -> (vk::Image, vk::ImageView, gpu_allocator::vulkan::Allocation) {
                let img = image::open(path)
                    .expect(&format!("Failed to load {}", path))
                    .to_rgba8();
                let (w, h) = img.dimensions();
                let reqs = vk::ImageCreateInfo {
                    image_type: vk::ImageType::TYPE_2D,
                    format: vk::Format::R8G8B8A8_SRGB,
                    extent: vk::Extent3D {
                        width: w,
                        height: h,
                        depth: 1,
                    },
                    mip_levels: 1,
                    array_layers: 1,
                    samples: vk::SampleCountFlags::TYPE_1,
                    usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
                    ..Default::default()
                };
                let image = unsafe { device.create_image(&reqs, None) }.unwrap();
                let mem_reqs = unsafe { device.get_image_memory_requirements(image) };
                let alloc = allocator
                    .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                        name: path,
                        requirements: mem_reqs,
                        location: gpu_allocator::MemoryLocation::GpuOnly,
                        linear: false,
                        allocation_scheme:
                            gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                    })
                    .unwrap();
                unsafe {
                    device
                        .bind_image_memory(image, alloc.memory(), alloc.offset())
                        .unwrap();
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
                super::resources::upload_new_gpu_image(
                    &device,
                    &mut allocator,
                    command_pool,
                    queue,
                    image,
                    w,
                    h,
                    &img,
                );
                (image, view, alloc)
            };

        let sun_texture_path = "assets/minecraft/textures/environment/sun.png";
        let (sun_texture_image, sun_texture_view, sun_texture_alloc) =
            load_sky_texture(sun_texture_path);
        let moon_texture_path = "assets/minecraft/textures/environment/moon_phases.png";
        let (moon_texture_image, moon_texture_view, moon_texture_alloc) =
            load_sky_texture(moon_texture_path);

        // Custom sky texture — 1×1 magenta placeholder until (if) a custom sky pack is loaded
        let custom_sky_pixels = vec![255u8, 0, 255, 255];
        let custom_sky_reqs = vk::ImageCreateInfo {
            image_type: vk::ImageType::TYPE_2D,
            format: vk::Format::R8G8B8A8_SRGB,
            extent: vk::Extent3D {
                width: 1,
                height: 1,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            ..Default::default()
        };
        let custom_sky_texture_image =
            unsafe { device.create_image(&custom_sky_reqs, None) }.unwrap();
        let custom_mem_reqs =
            unsafe { device.get_image_memory_requirements(custom_sky_texture_image) };
        let custom_sky_texture_alloc = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "custom_sky",
                requirements: custom_mem_reqs,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            device
                .bind_image_memory(
                    custom_sky_texture_image,
                    custom_sky_texture_alloc.memory(),
                    custom_sky_texture_alloc.offset(),
                )
                .unwrap();
        }
        let custom_sky_texture_view = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo {
                    image: custom_sky_texture_image,
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: vk::Format::R8G8B8A8_SRGB,
                    subresource_range: color_subresource(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        super::resources::upload_new_gpu_image(
            &device,
            &mut allocator,
            command_pool,
            queue,
            custom_sky_texture_image,
            1,
            1,
            &custom_sky_pixels,
        );

        // Custom sky texture placeholder will be replaced by load_custom_sky() called from app.rs

        // Sky uniform buffer + descriptor pool + sets
        let sky_uniform_size = std::mem::size_of::<super::SkyUniforms>() as u64;
        let (sky_uniform_buffer, sky_uniform_alloc) = Self::create_buffer_standalone(
            &device,
            &mut allocator,
            sky_uniform_size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            &[0u8; 64], // placeholder
        );
        // Sky descriptor pool
        let sky_pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: MAX_FRAMES as u32,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: MAX_FRAMES as u32 * 3,
            },
        ];
        let sky_desc_pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo {
                    max_sets: MAX_FRAMES as u32,
                    pool_size_count: 2,
                    p_pool_sizes: sky_pool_sizes.as_ptr(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        // Allocate sky descriptor sets (using sky_desc_layout from create_sky_pipeline)
        let sky_desc_sets: Vec<_> = (0..MAX_FRAMES)
            .map(|_| {
                unsafe {
                    device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo {
                        s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
                        p_next: std::ptr::null(),
                        _marker: std::marker::PhantomData,
                        descriptor_pool: sky_desc_pool,
                        descriptor_set_count: 1,
                        p_set_layouts: &sky_desc_layout,
                    })
                }
                .unwrap()[0]
            })
            .collect();
        // Write sky descriptors (uniform + sun texture + moon texture)
        for (i, &set) in sky_desc_sets.iter().enumerate() {
            let buf_info = vk::DescriptorBufferInfo {
                buffer: sky_uniform_buffer,
                offset: 0,
                range: sky_uniform_size,
            };
            let sun_img_info = vk::DescriptorImageInfo {
                sampler: entity_texture_sampler,
                image_view: sun_texture_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };
            let moon_img_info = vk::DescriptorImageInfo {
                sampler: entity_texture_sampler,
                image_view: moon_texture_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };
            let custom_sky_img_info = vk::DescriptorImageInfo {
                sampler: entity_texture_sampler,
                image_view: custom_sky_texture_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };
            unsafe {
                device.update_descriptor_sets(
                    &[
                        vk::WriteDescriptorSet {
                            dst_set: set,
                            dst_binding: 0,
                            dst_array_element: 0,
                            descriptor_count: 1,
                            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                            p_buffer_info: &buf_info,
                            ..Default::default()
                        },
                        vk::WriteDescriptorSet {
                            dst_set: set,
                            dst_binding: 1,
                            dst_array_element: 0,
                            descriptor_count: 1,
                            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            p_image_info: &sun_img_info,
                            ..Default::default()
                        },
                        vk::WriteDescriptorSet {
                            dst_set: set,
                            dst_binding: 2,
                            dst_array_element: 0,
                            descriptor_count: 1,
                            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            p_image_info: &moon_img_info,
                            ..Default::default()
                        },
                        vk::WriteDescriptorSet {
                            dst_set: set,
                            dst_binding: 3,
                            dst_array_element: 0,
                            descriptor_count: 1,
                            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            p_image_info: &custom_sky_img_info,
                            ..Default::default()
                        },
                    ],
                    &[],
                );
            }
        }

        // Sky vertex buffer (fullscreen triangle)
        let sky_verts: [[f32; 2]; 3] = [[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]];
        let (sky_vertex_buffer, sky_vertex_alloc) = Self::create_buffer_standalone(
            &device,
            &mut allocator,
            std::mem::size_of_val(&sky_verts) as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            bytemuck::bytes_of(&sky_verts),
        );
        let (chunk_vertex_buffer, chunk_vertex_alloc) = Self::create_empty_gpu_buffer_standalone(
            &device,
            &mut allocator,
            CHUNK_VERTEX_ARENA_BYTES,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            "chunk_vertex_arena",
        );
        let (chunk_index_buffer, chunk_index_alloc) = Self::create_empty_gpu_buffer_standalone(
            &device,
            &mut allocator,
            CHUNK_INDEX_ARENA_BYTES,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            "chunk_index_arena",
        );

        log::info!(
            "Vulkan renderer ready in {:.2} ms: swapchain={}x{} {:?}, images={}, frames_in_flight={}, multi_draw_indirect={}, chunk_vertex_arena={} MiB, chunk_index_arena={} MiB",
            init_started.elapsed().as_secs_f64() * 1000.0,
            swapchain_extent.width,
            swapchain_extent.height,
            swapchain_format,
            swapchain_images.len(),
            MAX_FRAMES,
            multi_draw_indirect,
            CHUNK_VERTEX_ARENA_BYTES / (1024 * 1024),
            CHUNK_INDEX_ARENA_BYTES / (1024 * 1024)
        );

        Renderer {
            _entry: entry,
            instance,
            device,
            _physical_device: physical_device,
            queue,
            surface_fn,
            surface,
            swapchain_fn,
            swapchain,
            _swapchain_images: swapchain_images,
            swapchain_image_views: swapchain_views,
            _swapchain_format: swapchain_format,
            swapchain_extent,
            render_pass,
            pipeline,
            pipeline_layout,
            transparent_pipeline,
            descriptor_layout,
            descriptor_pool,
            descriptor_sets,
            panorama_pipeline: vk::Pipeline::null(),
            panorama_pipeline_layout: vk::PipelineLayout::null(),
            panorama_descriptor_pool: vk::DescriptorPool::null(),
            panorama_descriptor_sets: vec![],
            panorama_image: vk::Image::null(),
            panorama_view: vk::ImageView::null(),
            panorama_alloc: None,
            panorama_sampler: vk::Sampler::null(),
            panorama_uniform_buffer: vk::Buffer::null(),
            panorama_uniform_alloc: gpu_allocator::vulkan::Allocation::default(),
            entity_pipeline,
            entity_pipeline_layout,
            particle_pipeline,
            particle_pipeline_layout,
            sky_pipeline,
            sky_pipeline_layout,
            sky_uniform_buffer,
            sky_uniform_alloc,
            sky_descriptor_pool: sky_desc_pool,
            sky_descriptor_sets: sky_desc_sets,
            sky_vertex_buffer,
            sky_vertex_alloc,
            entity_vertex_buffer: None,
            entity_index_buffer: None,
            entity_vertex_alloc: None,
            entity_index_alloc: None,
            entity_index_count: 0,
            entity_vertex_capacity: 0,
            entity_index_capacity: 0,
            entity_held_block_vertex_buffer: None,
            entity_held_block_index_buffer: None,
            entity_held_block_vertex_alloc: None,
            entity_held_block_index_alloc: None,
            entity_held_block_index_count: 0,
            entity_held_block_vertex_capacity: 0,
            entity_held_block_index_capacity: 0,
            entity_held_item_vertex_buffer: None,
            entity_held_item_index_buffer: None,
            entity_held_item_vertex_alloc: None,
            entity_held_item_index_alloc: None,
            entity_held_item_index_count: 0,
            entity_held_item_vertex_capacity: 0,
            entity_held_item_index_capacity: 0,
            entity_mesh_vertices: Vec::new(),
            entity_mesh_indices: Vec::new(),
            entity_held_block_vertices: Vec::new(),
            entity_held_block_indices: Vec::new(),
            entity_held_item_vertices: Vec::new(),
            entity_held_item_indices: Vec::new(),
            block_vertex_buffer: None,
            block_index_buffer: None,
            block_vertex_alloc: None,
            block_index_alloc: None,
            block_index_count: 0,
            block_vertex_capacity: 0,
            block_index_capacity: 0,
            block_dirty: true,
            pending_block_atlas: None,
            particle_vertex_buffer: None,
            particle_index_buffer: None,
            particle_vertex_alloc: None,
            particle_index_alloc: None,
            particle_index_count: 0,
            particle_vertex_capacity: 0,
            particle_index_capacity: 0,
            particle_generation: 0,
            particle_mesh_hash: 0,
            nametag_pipeline,
            nametag_pipeline_layout,
            nametag_vertex_buffer: None,
            nametag_index_buffer: None,
            nametag_vertex_alloc: None,
            nametag_index_alloc: None,
            nametag_index_count: 0,
            nametag_vertex_capacity: 0,
            nametag_index_capacity: 0,
            nametag_text_hash: 0,
            fp_arm_vertex_buffer: None,
            fp_arm_index_buffer: None,
            fp_arm_vertex_alloc: None,
            fp_arm_index_alloc: None,
            fp_arm_index_count: 0,
            fp_arm_vertex_capacity: 0,
            fp_arm_index_capacity: 0,
            fp_block_op_vertex_buffer: None,
            fp_block_op_index_buffer: None,
            fp_block_op_vertex_alloc: None,
            fp_block_op_index_alloc: None,
            fp_block_op_index_count: 0,
            fp_block_op_vertex_capacity: 0,
            fp_block_op_index_capacity: 0,
            fp_block_tr_vertex_buffer: None,
            fp_block_tr_index_buffer: None,
            fp_block_tr_vertex_alloc: None,
            fp_block_tr_index_alloc: None,
            fp_block_tr_index_count: 0,
            fp_block_tr_vertex_capacity: 0,
            fp_block_tr_index_capacity: 0,
            local_mesh_hash: None,
            fp_block_uses_item_atlas: false,
            entity_texture_image,
            entity_texture_view,
            entity_texture_sampler,
            entity_texture_alloc: Some(entity_texture_alloc),
            entity_descriptor_pool,
            entity_descriptor_sets,
            entity_atlas: Some(entity_atlas),
            entity_skin_upload_buffers: (0..MAX_FRAMES).map(|_| None).collect(),
            entity_skin_upload_allocs: (0..MAX_FRAMES).map(|_| None).collect(),
            entity_skin_upload_capacities: vec![0; MAX_FRAMES],
            entity_skin_upload_pending: false,
            entity_atlas_full_upload_pending: false,
            block_atlas: Some(block_atlas),
            block_animation_buffers: (0..MAX_FRAMES).map(|_| None).collect(),
            block_animation_allocs: (0..MAX_FRAMES).map(|_| None).collect(),
            block_animation_capacities: vec![0; MAX_FRAMES],
            block_animation_upload_bytes: Vec::new(),
            block_animation_uploads: Vec::new(),
            block_animation_last_tick: std::time::Instant::now(),
            skin_texture_image,
            skin_texture_view,
            skin_texture_alloc: Some(skin_texture_alloc),
            skin_descriptor_sets,
            local_skin_upload_buffers: (0..MAX_FRAMES).map(|_| None).collect(),
            local_skin_upload_allocs: (0..MAX_FRAMES).map(|_| None).collect(),
            local_skin_upload_capacities: vec![0; MAX_FRAMES],
            local_skin_upload_pending: false,
            cape_pixels: None,
            cape_upload_pending: false,
            sun_texture_image,
            sun_texture_view,
            sun_texture_alloc: Some(sun_texture_alloc),
            moon_texture_image,
            moon_texture_view,
            moon_texture_alloc: Some(moon_texture_alloc),
            custom_sky_texture_image,
            custom_sky_texture_view,
            custom_sky_texture_alloc: Some(custom_sky_texture_alloc),
            custom_sky_data: None,
            framebuffers,
            command_buffers,
            command_pool,
            image_available,
            render_finished,
            in_flight_fences,
            current_frame: 0,
            depth_images,
            depth_image_views,
            depth_format,
            depth_allocs,
            uniform_buffers,
            uniform_allocs,
            uniform_mapped,
            texture_image,
            texture_image_view,
            texture_sampler,
            texture_alloc: Some(texture_alloc),
            fp_item_descriptor_pool,
            fp_item_descriptor_sets,
            draw_cmds: Vec::new(),
            draw_cmd_indices: fnv::FnvHashMap::default(),
            visible_chunk_indices: Vec::new(),
            transparent_draw_indices: Vec::new(),
            retired_draw_cmds: (0..MAX_FRAMES).map(|_| Vec::new()).collect(),
            pending_retired_draw_cmds: Vec::new(),
            chunk_vertex_buffer,
            chunk_vertex_alloc,
            chunk_vertex_ranges: BufferRangeAllocator::new(
                (CHUNK_VERTEX_ARENA_BYTES / crate::world::mesh::Vertex::STRIDE as u64) as u32,
            ),
            chunk_index_buffer,
            chunk_index_alloc,
            chunk_index_ranges: BufferRangeAllocator::new(
                (CHUNK_INDEX_ARENA_BYTES / std::mem::size_of::<u32>() as u64) as u32,
            ),
            chunk_upload_buffers: (0..MAX_FRAMES).map(|_| None).collect(),
            chunk_upload_allocs: (0..MAX_FRAMES).map(|_| None).collect(),
            chunk_upload_capacities: vec![0; MAX_FRAMES],
            chunk_upload_bytes: Vec::new(),
            chunk_vertex_upload_copies: Vec::new(),
            chunk_index_upload_copies: Vec::new(),
            chunk_indirect_buffers: (0..MAX_FRAMES).map(|_| None).collect(),
            chunk_indirect_allocs: (0..MAX_FRAMES).map(|_| None).collect(),
            chunk_indirect_capacities: vec![0; MAX_FRAMES],
            chunk_indirect_commands: Vec::new(),
            chunk_opaque_indirect_count: 0,
            chunk_transparent_indirect_offset: 0,
            chunk_transparent_indirect_count: 0,
            multi_draw_indirect,
            last_gui_build: std::time::Instant::now() - std::time::Duration::from_secs(1),
            entity_state_hash: 0,
            sign_atlas_hash: 0,
            entity_model_cache: std::collections::HashMap::new(),
            entity_mesh_cache: fnv::FnvHashMap::default(),
            entity_gpu_meshes: fnv::FnvHashMap::default(),
            visible_entity_ids: Vec::new(),
            stale_entity_ids: Vec::new(),
            entity_frame_generation: 0,
            synced_player_skin_content_hash: None,
            player_skin_layout_hash: 0,
            player_skin_atlas_generation: 0,
            entity_atlas_generation: 0,
            allocator: std::mem::ManuallyDrop::new(allocator),
            // GUI placeholders (initialized by init_gui())
            player_preview_cache: Default::default(),
            gui_pipeline: vk::Pipeline::null(),
            gui_pipeline_layout: vk::PipelineLayout::null(),
            gui_descriptor_layout: vk::DescriptorSetLayout::null(),
            gui_descriptor_pool: vk::DescriptorPool::null(),
            gui_descriptor_sets: vec![
                vk::DescriptorSet::null();
                MAX_FRAMES * super::GUI_TEXTURE_COUNT
            ],
            gui_widget_image: vk::Image::null(),
            gui_widget_view: vk::ImageView::null(),
            gui_widget_sampler: vk::Sampler::null(),
            gui_widget_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_font_image: vk::Image::null(),
            gui_font_view: vk::ImageView::null(),
            gui_font_sampler: vk::Sampler::null(),
            gui_font_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_inventory_image: vk::Image::null(),
            gui_inventory_view: vk::ImageView::null(),
            gui_inventory_size: [256, 256],
            gui_inventory_sampler: vk::Sampler::null(),
            gui_inventory_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_generic54_image: vk::Image::null(),
            gui_generic54_view: vk::ImageView::null(),
            gui_generic54_sampler: vk::Sampler::null(),
            gui_generic54_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_items_image: vk::Image::null(),
            gui_items_view: vk::ImageView::null(),
            gui_items_sampler: vk::Sampler::null(),
            gui_items_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_icons_image: vk::Image::null(),
            gui_icons_view: vk::ImageView::null(),
            gui_icons_sampler: vk::Sampler::null(),
            gui_icons_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_creative_image: vk::Image::null(),
            gui_creative_view: vk::ImageView::null(),
            gui_creative_sampler: vk::Sampler::null(),
            gui_creative_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_options_bg_image: vk::Image::null(),
            gui_options_bg_view: vk::ImageView::null(),
            gui_options_bg_sampler: vk::Sampler::null(),
            gui_options_bg_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_underwater_image: vk::Image::null(),
            gui_underwater_view: vk::ImageView::null(),
            gui_underwater_sampler: vk::Sampler::null(),
            gui_underwater_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_buffers: (0..MAX_FRAMES * super::GUI_TEXTURE_COUNT)
                .map(|_| Default::default())
                .collect(),
            gui_builder_cache: Some((
                super::gui::GuiVertexBuilder::with_capacity(64),
                super::gui::GuiVertexBuilder::with_capacity(128),
                super::gui::GuiVertexBuilder::with_capacity(256),
                super::gui::GuiVertexBuilder::with_capacity(512),
                super::gui::GuiVertexBuilder::with_capacity(512),
                super::gui::GuiVertexBuilder::with_capacity(2048),
                super::gui::GuiVertexBuilder::with_capacity(512),
                super::gui::GuiVertexBuilder::with_capacity(512),
                super::gui::GuiVertexBuilder::with_capacity(256),
                super::gui::GuiVertexBuilder::with_capacity(512),
            )),
            gui_uniform_buffer: vk::Buffer::null(),
            gui_uniform_alloc: gpu_allocator::vulkan::Allocation::default(),
            gui_font_uploaded: false,
            cached_gui_vp_w: 0.0,
            cached_gui_vp_h: 0.0,
            font: crate::ui::font::FontRenderer::new(),
            last_button_hits: Vec::new(),
            gui_mouse_pos: [-1.0, -1.0],
            current_camera: None,
            state: {
                let mut s = super::state::GameRenderState::default();
                s.gui_scale = 3;
                s.render_distance = 8;
                s.smooth_lighting = true;
                s.particles_label = "All".to_string();
                s.particles_enabled = true;
                s.master_volume = 1.0;
                s.music_volume = 1.0;
                s.blocks_volume = 1.0;
                s.hostile_volume = 1.0;
                s.friendly_volume = 1.0;
                s.players_volume = 1.0;
                s.ambient_volume = 1.0;
                s.weather_volume = 1.0;
                s.ui_volume = 1.0;
                s.fov = 70.0;
                s.max_framerate = crate::client::config::UNLIMITED_FRAMERATE;
                s.clouds = true;
                s.entity_shadows = true;
                s.view_bobbing = true;
                s.difficulty = 2;
                s.skin_parts = 0xFF;
                s.language_code = "zh_CN".to_string();
                s.language_name = "中文（简体）".to_string();
                s.sky_brightness_cached = 1.0;
                s.local_skin_size = [64, 64];
                s.local_skin_face = [[255, 255, 255, 255]; 64];
                s.local_skin_preview =
                    crate::assets::skin::PlayerSkin::default_steve().preview_pixels();
                s.local_skin = crate::assets::skin::PlayerSkin::default_steve();
                s.inventory_window_type = "minecraft:container".to_string();
                s.inventory_window_title = "Inventory".to_string();
                s.ray_tracing_available = render_capabilities.ray_tracing;
                s.fsr3_available = render_capabilities.fsr3;
                s.shader_pack_status =
                    shader_pack
                        .error
                        .clone()
                        .unwrap_or_else(|| match &shader_pack.active_name {
                            Some(name) => format!("Active: {name}"),
                            None => "Shaders: Off".to_string(),
                        });
                s
            },
            particles: Vec::new(),
            particle_list: Vec::new(),
            hand_equip_progress: 1.0,
            hand_animation_last_update: std::time::Instant::now(),
            window_size,
            needs_recreate: false,
            first_frame_done: std::cell::Cell::new(false),
        }
    }

    // --- Swapchain ---

    pub fn update_skin_gpu(&mut self) {
        self.local_skin_upload_pending = true;
    }

    pub(super) fn create_swapchain(
        surface_fn: &ash::khr::surface::Instance,
        swapchain_fn: &ash::khr::swapchain::Device,
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        window_size: (u32, u32),
        old_swapchain: vk::SwapchainKHR,
    ) -> (
        vk::SwapchainKHR,
        Vec<vk::Image>,
        Vec<vk::ImageView>,
        vk::Format,
        vk::Extent2D,
    ) {
        let caps = unsafe {
            surface_fn.get_physical_device_surface_capabilities(physical_device, surface)
        }
        .unwrap();
        let formats =
            unsafe { surface_fn.get_physical_device_surface_formats(physical_device, surface) }
                .unwrap();

        let format = formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_SRGB)
            .map(|f| f.format)
            .unwrap_or(formats[0].format);

        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: window_size
                    .0
                    .clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: window_size
                    .1
                    .clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };
        let mut image_count = caps.min_image_count.max(MAX_FRAMES as u32);
        if caps.max_image_count != 0 {
            image_count = image_count.min(caps.max_image_count);
        }

        let swapchain = unsafe {
            swapchain_fn.create_swapchain(
                &vk::SwapchainCreateInfoKHR {
                    surface,
                    min_image_count: image_count,
                    image_format: format,
                    image_color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
                    image_extent: extent,
                    image_array_layers: 1,
                    image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
                    pre_transform: caps.current_transform,
                    composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
                    present_mode: vk::PresentModeKHR::IMMEDIATE,
                    clipped: vk::TRUE,
                    old_swapchain,
                    ..Default::default()
                },
                None,
            )
        }
        .expect("create_swapchain");

        let images = unsafe { swapchain_fn.get_swapchain_images(swapchain).unwrap() };
        let views: Vec<_> = images
            .iter()
            .map(|&img| {
                unsafe {
                    device.create_image_view(
                        &vk::ImageViewCreateInfo {
                            image: img,
                            view_type: vk::ImageViewType::TYPE_2D,
                            format,
                            subresource_range: color_subresource(),
                            ..Default::default()
                        },
                        None,
                    )
                }
                .unwrap()
            })
            .collect();

        (swapchain, images, views, format, extent)
    }

    // --- Depth buffer ---

    pub(super) fn create_depth_buffer(
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        format: vk::Format,
        extent: vk::Extent2D,
    ) -> (vk::Image, vk::ImageView, gpu_allocator::vulkan::Allocation) {
        let image = unsafe {
            device.create_image(
                &vk::ImageCreateInfo {
                    image_type: vk::ImageType::TYPE_2D,
                    format,
                    extent: vk::Extent3D {
                        width: extent.width,
                        height: extent.height,
                        depth: 1,
                    },
                    mip_levels: 1,
                    array_layers: 1,
                    samples: vk::SampleCountFlags::TYPE_1,
                    usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let reqs = unsafe { device.get_image_memory_requirements(image) };
        let alloc = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "depth",
                requirements: reqs,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            device
                .bind_image_memory(image, alloc.memory(), alloc.offset())
                .unwrap()
        };
        let view = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo {
                    image,
                    view_type: vk::ImageViewType::TYPE_2D,
                    format,
                    subresource_range: depth_subresource(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        (image, view, alloc)
    }

    pub(super) fn create_depth_buffers(
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        format: vk::Format,
        extent: vk::Extent2D,
        count: usize,
    ) -> (
        Vec<vk::Image>,
        Vec<vk::ImageView>,
        Vec<gpu_allocator::vulkan::Allocation>,
    ) {
        let mut images = Vec::with_capacity(count);
        let mut views = Vec::with_capacity(count);
        let mut allocations = Vec::with_capacity(count);
        for _ in 0..count {
            let (image, view, allocation) =
                Self::create_depth_buffer(device, allocator, format, extent);
            images.push(image);
            views.push(view);
            allocations.push(allocation);
        }
        (images, views, allocations)
    }

    // --- Framebuffers ---

    pub(super) fn create_framebuffers(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        views: &[vk::ImageView],
        depth_views: &[vk::ImageView],
        extent: vk::Extent2D,
    ) -> Vec<vk::Framebuffer> {
        assert_eq!(views.len(), depth_views.len());
        views
            .iter()
            .zip(depth_views)
            .map(|(&view, &depth_view)| {
                let att = [view, depth_view];
                unsafe {
                    device.create_framebuffer(
                        &vk::FramebufferCreateInfo {
                            render_pass,
                            attachment_count: 2,
                            p_attachments: att.as_ptr(),
                            width: extent.width,
                            height: extent.height,
                            layers: 1,
                            ..Default::default()
                        },
                        None,
                    )
                }
                .unwrap()
            })
            .collect()
    }

    // --- Sync ---

    pub(super) fn create_sync_objects(
        device: &ash::Device,
    ) -> (Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
        let sem_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo {
            flags: vk::FenceCreateFlags::SIGNALED,
            ..Default::default()
        };
        let mut a = Vec::with_capacity(MAX_FRAMES);
        let mut b = Vec::with_capacity(MAX_FRAMES);
        let mut c = Vec::with_capacity(MAX_FRAMES);
        for _ in 0..MAX_FRAMES {
            a.push(unsafe { device.create_semaphore(&sem_info, None) }.unwrap());
            b.push(unsafe { device.create_semaphore(&sem_info, None) }.unwrap());
            c.push(unsafe { device.create_fence(&fence_info, None) }.unwrap());
        }
        (a, b, c)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            // Ignore errors during shutdown — device may already be lost
            let _ = self.device.device_wait_idle();

            for i in 0..MAX_FRAMES {
                self.device.destroy_semaphore(self.image_available[i], None);
                self.device.destroy_semaphore(self.render_finished[i], None);
                self.device.destroy_fence(self.in_flight_fences[i], None);
                self.device.destroy_buffer(self.uniform_buffers[i], None);
                if let Some(buffer) = self.block_animation_buffers[i].take() {
                    self.device.destroy_buffer(buffer, None);
                }
                if let Some(allocation) = self.block_animation_allocs[i].take() {
                    self.allocator.free(allocation).ok();
                }
                if let Some(buffer) = self.entity_skin_upload_buffers[i].take() {
                    self.device.destroy_buffer(buffer, None);
                }
                if let Some(allocation) = self.entity_skin_upload_allocs[i].take() {
                    self.allocator.free(allocation).ok();
                }
                if let Some(buffer) = self.local_skin_upload_buffers[i].take() {
                    self.device.destroy_buffer(buffer, None);
                }
                if let Some(allocation) = self.local_skin_upload_allocs[i].take() {
                    self.allocator.free(allocation).ok();
                }
                if let Some(buffer) = self.chunk_indirect_buffers[i].take() {
                    self.device.destroy_buffer(buffer, None);
                }
                if let Some(allocation) = self.chunk_indirect_allocs[i].take() {
                    self.allocator.free(allocation).ok();
                }
                if let Some(buffer) = self.chunk_upload_buffers[i].take() {
                    self.device.destroy_buffer(buffer, None);
                }
                if let Some(allocation) = self.chunk_upload_allocs[i].take() {
                    self.allocator.free(allocation).ok();
                }
            }

            for cmd in self.draw_cmds.drain(..) {
                if let ChunkStorage::Dedicated {
                    vertex_buffer,
                    index_buffer,
                    vertex_alloc,
                    index_alloc,
                } = cmd.storage
                {
                    self.device.destroy_buffer(vertex_buffer, None);
                    self.device.destroy_buffer(index_buffer, None);
                    self.allocator.free(vertex_alloc).ok();
                    self.allocator.free(index_alloc).ok();
                }
            }
            for cmd in self.pending_retired_draw_cmds.drain(..) {
                if let ChunkStorage::Dedicated {
                    vertex_buffer,
                    index_buffer,
                    vertex_alloc,
                    index_alloc,
                } = cmd.storage
                {
                    self.device.destroy_buffer(vertex_buffer, None);
                    self.device.destroy_buffer(index_buffer, None);
                    self.allocator.free(vertex_alloc).ok();
                    self.allocator.free(index_alloc).ok();
                }
            }
            for commands in &mut self.retired_draw_cmds {
                for cmd in commands.drain(..) {
                    if let ChunkStorage::Dedicated {
                        vertex_buffer,
                        index_buffer,
                        vertex_alloc,
                        index_alloc,
                    } = cmd.storage
                    {
                        self.device.destroy_buffer(vertex_buffer, None);
                        self.device.destroy_buffer(index_buffer, None);
                        self.allocator.free(vertex_alloc).ok();
                        self.allocator.free(index_alloc).ok();
                    }
                }
            }
            self.device.destroy_buffer(self.chunk_vertex_buffer, None);
            self.device.destroy_buffer(self.chunk_index_buffer, None);
            self.allocator
                .free(std::mem::take(&mut self.chunk_vertex_alloc))
                .ok();
            self.allocator
                .free(std::mem::take(&mut self.chunk_index_alloc))
                .ok();
            for alloc in self.uniform_allocs.drain(..) {
                self.allocator.free(alloc).ok();
            }
            for slot in self.gui_buffers.drain(..) {
                if let Some(buffer) = slot.vertex_buffer {
                    self.device.destroy_buffer(buffer, None);
                }
                if let Some(buffer) = slot.index_buffer {
                    self.device.destroy_buffer(buffer, None);
                }
                if let Some(alloc) = slot.vertex_alloc {
                    self.allocator.free(alloc).ok();
                }
                if let Some(alloc) = slot.index_alloc {
                    self.allocator.free(alloc).ok();
                }
            }
            // Block selection buffers
            if let Some(buffer) = self.block_vertex_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(buffer) = self.block_index_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(alloc) = self.block_vertex_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(alloc) = self.block_index_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if self.gui_pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.gui_pipeline, None);
                self.device
                    .destroy_pipeline_layout(self.gui_pipeline_layout, None);
            }
            if self.gui_descriptor_pool != vk::DescriptorPool::null() {
                self.device
                    .destroy_descriptor_pool(self.gui_descriptor_pool, None);
            }
            if self.gui_descriptor_layout != vk::DescriptorSetLayout::null() {
                self.device
                    .destroy_descriptor_set_layout(self.gui_descriptor_layout, None);
            }
            if self.gui_uniform_buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.gui_uniform_buffer, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_uniform_alloc))
                    .ok();
            }
            if self.gui_widget_image != vk::Image::null() {
                self.device.destroy_image_view(self.gui_widget_view, None);
                self.device.destroy_sampler(self.gui_widget_sampler, None);
                self.device.destroy_image(self.gui_widget_image, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_widget_alloc))
                    .ok();
            }
            if self.gui_font_image != vk::Image::null() {
                self.device.destroy_image_view(self.gui_font_view, None);
                self.device.destroy_sampler(self.gui_font_sampler, None);
                self.device.destroy_image(self.gui_font_image, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_font_alloc))
                    .ok();
            }
            if self.gui_inventory_image != vk::Image::null() {
                self.device
                    .destroy_image_view(self.gui_inventory_view, None);
                self.device
                    .destroy_sampler(self.gui_inventory_sampler, None);
                self.device.destroy_image(self.gui_inventory_image, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_inventory_alloc))
                    .ok();
            }
            if self.gui_generic54_image != vk::Image::null() {
                self.device
                    .destroy_image_view(self.gui_generic54_view, None);
                self.device
                    .destroy_sampler(self.gui_generic54_sampler, None);
                self.device.destroy_image(self.gui_generic54_image, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_generic54_alloc))
                    .ok();
            }
            if self.gui_items_image != vk::Image::null() {
                self.device.destroy_image_view(self.gui_items_view, None);
                self.device.destroy_sampler(self.gui_items_sampler, None);
                self.device.destroy_image(self.gui_items_image, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_items_alloc))
                    .ok();
            }
            if self.gui_creative_image != vk::Image::null() {
                self.device.destroy_image_view(self.gui_creative_view, None);
                self.device.destroy_sampler(self.gui_creative_sampler, None);
                self.device.destroy_image(self.gui_creative_image, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_creative_alloc))
                    .ok();
            }
            if self.gui_options_bg_image != vk::Image::null() {
                self.device
                    .destroy_image_view(self.gui_options_bg_view, None);
                self.device
                    .destroy_sampler(self.gui_options_bg_sampler, None);
                self.device.destroy_image(self.gui_options_bg_image, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_options_bg_alloc))
                    .ok();
            }
            if self.gui_underwater_image != vk::Image::null() {
                self.device
                    .destroy_image_view(self.gui_underwater_view, None);
                self.device
                    .destroy_sampler(self.gui_underwater_sampler, None);
                self.device.destroy_image(self.gui_underwater_image, None);
                self.allocator
                    .free(std::mem::take(&mut self.gui_underwater_alloc))
                    .ok();
            }
            if self.panorama_pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.panorama_pipeline, None);
                self.device
                    .destroy_pipeline_layout(self.panorama_pipeline_layout, None);
            }
            if self.panorama_descriptor_pool != vk::DescriptorPool::null() {
                self.device
                    .destroy_descriptor_pool(self.panorama_descriptor_pool, None);
            }
            if self.panorama_uniform_buffer != vk::Buffer::null() {
                self.device
                    .destroy_buffer(self.panorama_uniform_buffer, None);
                self.allocator
                    .free(std::mem::take(&mut self.panorama_uniform_alloc))
                    .ok();
            }
            if self.panorama_image != vk::Image::null() {
                self.device.destroy_image_view(self.panorama_view, None);
                self.device.destroy_sampler(self.panorama_sampler, None);
                self.device.destroy_image(self.panorama_image, None);
                if let Some(alloc) = self.panorama_alloc.take() {
                    self.allocator.free(alloc).ok();
                }
            }
            // Particle mesh buffers
            if let Some(buf) = self.particle_vertex_buffer.take() {
                self.device.destroy_buffer(buf, None);
            }
            if let Some(buffer) = self.particle_index_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(alloc) = self.particle_vertex_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(alloc) = self.particle_index_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            // Nametag mesh buffers
            if let Some(buf) = self.nametag_vertex_buffer.take() {
                self.device.destroy_buffer(buf, None);
            }
            if let Some(buffer) = self.nametag_index_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(alloc) = self.nametag_vertex_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(alloc) = self.nametag_index_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if self.nametag_pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.nametag_pipeline, None);
                self.device
                    .destroy_pipeline_layout(self.nametag_pipeline_layout, None);
            }
            // First person hand buffers
            if let Some(buffer) = self.fp_arm_vertex_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(buffer) = self.fp_arm_index_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(alloc) = self.fp_arm_vertex_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(alloc) = self.fp_arm_index_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(buffer) = self.fp_block_op_vertex_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(buffer) = self.fp_block_op_index_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(alloc) = self.fp_block_op_vertex_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(alloc) = self.fp_block_op_index_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(buffer) = self.fp_block_tr_vertex_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(buffer) = self.fp_block_tr_index_buffer.take() {
                self.device.destroy_buffer(buffer, None);
            }
            if let Some(alloc) = self.fp_block_tr_vertex_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(alloc) = self.fp_block_tr_index_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(a) = self.texture_alloc.take() {
                self.allocator.free(a).ok();
            }
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_layout, None);

            self.device
                .destroy_image_view(self.texture_image_view, None);
            self.device.destroy_sampler(self.texture_sampler, None);
            self.device.destroy_image(self.texture_image, None);

            for view in self.depth_image_views.drain(..) {
                self.device.destroy_image_view(view, None);
            }
            for image in self.depth_images.drain(..) {
                self.device.destroy_image(image, None);
            }
            for allocation in self.depth_allocs.drain(..) {
                self.allocator.free(allocation).ok();
            }

            self.device.destroy_pipeline(self.pipeline, None);
            if self.transparent_pipeline != vk::Pipeline::null() {
                self.device
                    .destroy_pipeline(self.transparent_pipeline, None);
            }
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            // Sky pipeline
            if self.sky_pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.sky_pipeline, None);
                self.device
                    .destroy_pipeline_layout(self.sky_pipeline_layout, None);
            }
            if self.sky_descriptor_pool != vk::DescriptorPool::null() {
                self.device
                    .destroy_descriptor_pool(self.sky_descriptor_pool, None);
            }
            if self.sky_uniform_buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.sky_uniform_buffer, None);
                self.allocator
                    .free(std::mem::take(&mut self.sky_uniform_alloc))
                    .ok();
            }
            if self.sky_vertex_buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.sky_vertex_buffer, None);
                self.allocator
                    .free(std::mem::take(&mut self.sky_vertex_alloc))
                    .ok();
            }
            // Entity pipeline resources
            if self.entity_pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.entity_pipeline, None);
                self.device
                    .destroy_pipeline_layout(self.entity_pipeline_layout, None);
            }
            if self.particle_pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.particle_pipeline, None);
                self.device
                    .destroy_pipeline_layout(self.particle_pipeline_layout, None);
            }
            if self.entity_descriptor_pool != vk::DescriptorPool::null() {
                self.device
                    .destroy_descriptor_pool(self.entity_descriptor_pool, None);
            }
            for (_, mut mesh) in self.entity_gpu_meshes.drain() {
                super::destroy_entity_gpu_mesh(&self.device, &mut self.allocator, &mut mesh);
            }
            if let Some(buf) = self.entity_vertex_buffer.take() {
                self.device.destroy_buffer(buf, None);
            }
            if let Some(buf) = self.entity_index_buffer.take() {
                self.device.destroy_buffer(buf, None);
            }
            if let Some(alloc) = self.entity_vertex_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            if let Some(alloc) = self.entity_index_alloc.take() {
                self.allocator.free(alloc).ok();
            }
            for buffer in [
                self.entity_held_block_vertex_buffer.take(),
                self.entity_held_block_index_buffer.take(),
                self.entity_held_item_vertex_buffer.take(),
                self.entity_held_item_index_buffer.take(),
            ]
            .into_iter()
            .flatten()
            {
                self.device.destroy_buffer(buffer, None);
            }
            for alloc in [
                self.entity_held_block_vertex_alloc.take(),
                self.entity_held_block_index_alloc.take(),
                self.entity_held_item_vertex_alloc.take(),
                self.entity_held_item_index_alloc.take(),
            ]
            .into_iter()
            .flatten()
            {
                self.allocator.free(alloc).ok();
            }
            if self.entity_texture_image != vk::Image::null() {
                self.device
                    .destroy_image_view(self.entity_texture_view, None);
                self.device
                    .destroy_sampler(self.entity_texture_sampler, None);
                self.device.destroy_image(self.entity_texture_image, None);
                if let Some(alloc) = self.entity_texture_alloc.take() {
                    self.allocator.free(alloc).ok();
                }
            }
            if self.skin_texture_image != vk::Image::null() {
                self.device.destroy_image(self.skin_texture_image, None);
                self.device.destroy_image_view(self.skin_texture_view, None);
                if let Some(alloc) = self.skin_texture_alloc.take() {
                    self.allocator.free(alloc).ok();
                }
            }
            if self.sun_texture_image != vk::Image::null() {
                self.device.destroy_image(self.sun_texture_image, None);
                self.device.destroy_image_view(self.sun_texture_view, None);
                if let Some(alloc) = self.sun_texture_alloc.take() {
                    self.allocator.free(alloc).ok();
                }
            }
            if self.moon_texture_image != vk::Image::null() {
                self.device.destroy_image(self.moon_texture_image, None);
                self.device.destroy_image_view(self.moon_texture_view, None);
                if let Some(alloc) = self.moon_texture_alloc.take() {
                    self.allocator.free(alloc).ok();
                }
            }
            if self.custom_sky_texture_image != vk::Image::null() {
                self.device
                    .destroy_image(self.custom_sky_texture_image, None);
                self.device
                    .destroy_image_view(self.custom_sky_texture_view, None);
                if let Some(alloc) = self.custom_sky_texture_alloc.take() {
                    self.allocator.free(alloc).ok();
                }
            }
            self.device.destroy_render_pass(self.render_pass, None);

            for fb in &self.framebuffers {
                self.device.destroy_framebuffer(*fb, None);
            }
            for v in &self.swapchain_image_views {
                self.device.destroy_image_view(*v, None);
            }
            self.device.destroy_command_pool(self.command_pool, None);
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
            // Destroy surface while instance is still alive (vkDestroySurfaceKHR needs a valid instance).
            self.surface_fn.destroy_surface(self.surface, None);
            // NOTE: device and instance are NOT manually destroyed here —
            // ash::Device and ash::Instance drop impls will call vkDestroyDevice
            // and vkDestroyInstance respectively. Letting ash own this avoids a
            // double-destroy that can crash drivers with 0xc0000005.
        }
    }
}
