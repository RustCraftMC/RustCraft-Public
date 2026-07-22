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

        // Create debug messenger if validation layer is active.
        // The debug_utils loader and the messenger handle are stored on the
        // Renderer so the messenger can be explicitly destroyed in Drop instead
        // of being leaked via mem::forget.
        let debug_utils;
        let debug_messenger;
        if validation_layer.is_some() {
            let du = ash::ext::debug_utils::Instance::new(&entry, &instance);
            debug_messenger = unsafe {
                du.create_debug_utils_messenger(
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
                .unwrap()
            };
            debug_utils = Some(du);
        } else {
            debug_utils = None;
            debug_messenger = vk::DebugUtilsMessengerEXT::null();
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
            super::swapchain::SwapchainManager::create_swapchain(
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
        let (depth_images, depth_image_views, depth_allocs) =
            super::swapchain::SwapchainManager::create_depth_buffers(
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
        let framebuffers = super::swapchain::SwapchainManager::create_framebuffers(
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
            super::swapchain::SwapchainManager::create_sync_objects(&device);

        // Uniforms
        let (uniform_buffers, uniform_allocs, uniform_mapped) =
            super::resource_manager::ResourceManager::create_uniform_buffers(&device, &mut allocator);

        // Descriptors
        let (descriptor_pool, descriptor_sets) =
            Self::create_descriptors(&device, descriptor_layout, &uniform_buffers);

        // Texture atlas
        let (
            texture_image_raii,
            texture_view_raii,
            texture_alloc,
            texture_sampler_raii,
            block_atlas,
        ) = Self::create_texture_atlas(&device, &mut allocator, command_pool, queue, resolver);
        let texture_image = texture_image_raii.into_handle();
        let texture_image_view = texture_view_raii.into_handle();
        let texture_sampler = texture_sampler_raii.into_handle();

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
            entity_texture_image_raii,
            entity_texture_view_raii,
            entity_texture_alloc,
            entity_texture_sampler_raii,
        ) = Self::create_entity_texture(
            &device,
            &mut allocator,
            command_pool,
            queue,
            &entity_atlas,
        );
        let entity_texture_image = entity_texture_image_raii.into_handle();
        let entity_texture_view = entity_texture_view_raii.into_handle();
        let entity_texture_sampler = entity_texture_sampler_raii.into_handle();

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
            debug_utils,
            debug_messenger,
            device,
            _physical_device: physical_device,
            queue,
            swapchain: super::swapchain::SwapchainManager::new(
                surface_fn,
                surface,
                swapchain_fn,
                swapchain,
                swapchain_images,
                swapchain_views,
                swapchain_format,
                swapchain_extent,
                depth_images,
                depth_image_views,
                depth_format,
                depth_allocs,
                framebuffers,
                image_available,
                render_finished,
                in_flight_fences,
                window_size,
            ),
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
            command_buffers,
            command_pool,
            resources: super::resource_manager::ResourceManager::new(
                allocator,
                uniform_buffers,
                uniform_allocs,
                uniform_mapped,
            ),
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
                s.settings.set_gui_scale(3);
                s.settings.set_render_distance(8);
                s.settings.set_smooth_lighting(true);
                s.settings.set_particles_label("All".to_string());
                s.settings.set_particles_enabled(true);
                s.settings.set_master_volume(1.0);
                s.settings.set_music_volume(1.0);
                s.settings.set_blocks_volume(1.0);
                s.settings.set_hostile_volume(1.0);
                s.settings.set_friendly_volume(1.0);
                s.settings.set_players_volume(1.0);
                s.settings.set_ambient_volume(1.0);
                s.settings.set_weather_volume(1.0);
                s.settings.set_ui_volume(1.0);
                s.settings.set_fov(70.0);
                s.settings.set_max_framerate(crate::client::config::UNLIMITED_FRAMERATE);
                s.settings.set_clouds(true);
                s.settings.set_entity_shadows(true);
                s.settings.set_view_bobbing(true);
                s.settings.set_difficulty(2);
                s.settings.set_skin_parts(0xFF);
                s.settings.set_language_code("zh_CN".to_string());
                s.settings.set_language_name("中文（简体）".to_string());
                s.frame_profile.set_sky_brightness_cached(1.0);
                s.settings.set_local_skin_size([64, 64]);
                s.settings.set_local_skin_face([[255, 255, 255, 255]; 64]);
                s.settings.set_local_skin_preview(crate::assets::skin::PlayerSkin::default_steve().preview_pixels());
                s.settings.set_local_skin(crate::assets::skin::PlayerSkin::default_steve());
                s.inventory.set_inventory_window_type("minecraft:container".to_string());
                s.inventory.set_inventory_window_title("Inventory".to_string());
                s.server_list.set_ray_tracing_available(render_capabilities.ray_tracing);
                s.server_list.set_fsr3_available(render_capabilities.fsr3);
                s.server_list.set_shader_pack_status(shader_pack
                                            .error
                                            .clone()
                                            .unwrap_or_else(|| match &shader_pack.active_name {
                                                Some(name) => format!("Active: {name}"),
                                                None => "Shaders: Off".to_string(),
                                            }));
                s
            },
            particles: Vec::new(),
            particle_list: Vec::new(),
            hand_equip_progress: 1.0,
            hand_animation_last_update: std::time::Instant::now(),
            first_frame_done: std::cell::Cell::new(false),
        }
    }

    // --- Swapchain ---

    pub fn update_skin_gpu(&mut self) {
        self.local_skin_upload_pending = true;
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            // Ignore errors during shutdown — device may already be lost
            let _ = self.device.device_wait_idle();
        }

        // Adopt raw handles into RAII wrappers so their Drop impls invoke
        // the matching `destroy_*`. This replaces the previous wall of
        // manual `destroy_*` calls. Allocations are freed separately
        // because the gpu_allocator is `ManuallyDrop` and not shareable.

        let device = self.device.clone();

        // Destroy the debug messenger explicitly while the instance is
        // still alive. Previously this was leaked via std::mem::forget,
        // which triggered validation-layer errors on shutdown.
        if let Some(debug_utils) = self.debug_utils.take() {
            let _ = super::raii::DebugMessenger::from_handle(
                debug_utils,
                std::mem::replace(
                    &mut self.debug_messenger,
                    vk::DebugUtilsMessengerEXT::null(),
                ),
            );
        }

        // Per-frame sync objects (owned by SwapchainManager), uniform buffers and per-frame staging buffers.
        for i in 0..MAX_FRAMES {
            let _ = super::raii::Semaphore::from_handle(
                device.clone(),
                std::mem::replace(&mut self.swapchain.image_available[i], vk::Semaphore::null()),
            );
            let _ = super::raii::Semaphore::from_handle(
                device.clone(),
                std::mem::replace(&mut self.swapchain.render_finished[i], vk::Semaphore::null()),
            );
            let _ = super::raii::Fence::from_handle(
                device.clone(),
                std::mem::replace(&mut self.swapchain.in_flight_fences[i], vk::Fence::null()),
            );
            let _ = super::raii::Buffer::from_handle(
                device.clone(),
                self.resources.take_uniform_buffer(i),
            );
            if let Some(b) = self.block_animation_buffers[i].take() {
                let _ = super::raii::Buffer::from_handle(device.clone(), b);
            }
            if let Some(b) = self.entity_skin_upload_buffers[i].take() {
                let _ = super::raii::Buffer::from_handle(device.clone(), b);
            }
            if let Some(b) = self.local_skin_upload_buffers[i].take() {
                let _ = super::raii::Buffer::from_handle(device.clone(), b);
            }
            if let Some(b) = self.chunk_indirect_buffers[i].take() {
                let _ = super::raii::Buffer::from_handle(device.clone(), b);
            }
            if let Some(b) = self.chunk_upload_buffers[i].take() {
                let _ = super::raii::Buffer::from_handle(device.clone(), b);
            }
            if let Some(a) = self.block_animation_allocs[i].take() {
                self.resources.free(a);
            }
            if let Some(a) = self.entity_skin_upload_allocs[i].take() {
                self.resources.free(a);
            }
            if let Some(a) = self.local_skin_upload_allocs[i].take() {
                self.resources.free(a);
            }
            if let Some(a) = self.chunk_indirect_allocs[i].take() {
                self.resources.free(a);
            }
            if let Some(a) = self.chunk_upload_allocs[i].take() {
                self.resources.free(a);
            }
        }
        self.resources.destroy_uniform_buffers(&device);

        // Chunk draw command dedicated buffers
        for cmd in self.draw_cmds.drain(..) {
            if let ChunkStorage::Dedicated {
                vertex_buffer,
                index_buffer,
                vertex_alloc,
                index_alloc,
            } = cmd.storage
            {
                let _ = super::raii::Buffer::from_handle(device.clone(), vertex_buffer);
                let _ = super::raii::Buffer::from_handle(device.clone(), index_buffer);
                self.resources.free(vertex_alloc);
                self.resources.free(index_alloc);
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
                let _ = super::raii::Buffer::from_handle(device.clone(), vertex_buffer);
                let _ = super::raii::Buffer::from_handle(device.clone(), index_buffer);
                self.resources.free(vertex_alloc);
                self.resources.free(index_alloc);
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
                    let _ = super::raii::Buffer::from_handle(device.clone(), vertex_buffer);
                    let _ = super::raii::Buffer::from_handle(device.clone(), index_buffer);
                    self.resources.free(vertex_alloc);
                    self.resources.free(index_alloc);
                }
            }
        }

        // Chunk arena buffers
        let _ = super::raii::Buffer::from_handle(
            device.clone(),
            std::mem::replace(&mut self.chunk_vertex_buffer, vk::Buffer::null()),
        );
        let _ = super::raii::Buffer::from_handle(
            device.clone(),
            std::mem::replace(&mut self.chunk_index_buffer, vk::Buffer::null()),
        );
        self.resources
            .free(std::mem::take(&mut self.chunk_vertex_alloc));
        self.resources
            .free(std::mem::take(&mut self.chunk_index_alloc));

        // GUI buffers
        for slot in self.gui_buffers.drain(..) {
            if let Some(b) = slot.vertex_buffer {
                let _ = super::raii::Buffer::from_handle(device.clone(), b);
            }
            if let Some(b) = slot.index_buffer {
                let _ = super::raii::Buffer::from_handle(device.clone(), b);
            }
            if let Some(a) = slot.vertex_alloc {
                self.resources.free(a);
            }
            if let Some(a) = slot.index_alloc {
                self.resources.free(a);
            }
        }

        // Block selection buffers
        if let Some(b) = self.block_vertex_buffer.take() {
            let _ = super::raii::Buffer::from_handle(device.clone(), b);
        }
        if let Some(b) = self.block_index_buffer.take() {
            let _ = super::raii::Buffer::from_handle(device.clone(), b);
        }
        if let Some(a) = self.block_vertex_alloc.take() {
            self.resources.free(a);
        }
        if let Some(a) = self.block_index_alloc.take() {
            self.resources.free(a);
        }

        // GUI pipeline and descriptors
        let _ = super::raii::Pipeline::from_handle(
            device.clone(),
            std::mem::replace(&mut self.gui_pipeline, vk::Pipeline::null()),
        );
        let _ = super::raii::PipelineLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.gui_pipeline_layout, vk::PipelineLayout::null()),
        );
        let _ = super::raii::DescriptorPool::from_handle(
            device.clone(),
            std::mem::replace(&mut self.gui_descriptor_pool, vk::DescriptorPool::null()),
        );
        let _ = super::raii::DescriptorSetLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.gui_descriptor_layout, vk::DescriptorSetLayout::null()),
        );
        let _ = super::raii::Buffer::from_handle(
            device.clone(),
            std::mem::replace(&mut self.gui_uniform_buffer, vk::Buffer::null()),
        );
        self.resources
            .free(std::mem::take(&mut self.gui_uniform_alloc));

        // GUI textures: image + view + sampler + alloc.
        macro_rules! drop_gui_texture {
            ($image:expr, $view:expr, $sampler:expr, $alloc:expr) => {{
                let img = std::mem::replace(&mut $image, vk::Image::null());
                if img != vk::Image::null() {
                    let _ = super::raii::Image::from_handle(device.clone(), img);
                    let _ = super::raii::ImageView::from_handle(
                        device.clone(),
                        std::mem::replace(&mut $view, vk::ImageView::null()),
                    );
                    let _ = super::raii::Sampler::from_handle(
                        device.clone(),
                        std::mem::replace(&mut $sampler, vk::Sampler::null()),
                    );
                    self.resources.free(std::mem::take(&mut $alloc));
                }
            }};
        }
        drop_gui_texture!(
            self.gui_widget_image,
            self.gui_widget_view,
            self.gui_widget_sampler,
            self.gui_widget_alloc
        );
        drop_gui_texture!(
            self.gui_font_image,
            self.gui_font_view,
            self.gui_font_sampler,
            self.gui_font_alloc
        );
        drop_gui_texture!(
            self.gui_inventory_image,
            self.gui_inventory_view,
            self.gui_inventory_sampler,
            self.gui_inventory_alloc
        );
        drop_gui_texture!(
            self.gui_generic54_image,
            self.gui_generic54_view,
            self.gui_generic54_sampler,
            self.gui_generic54_alloc
        );
        drop_gui_texture!(
            self.gui_items_image,
            self.gui_items_view,
            self.gui_items_sampler,
            self.gui_items_alloc
        );
        drop_gui_texture!(
            self.gui_creative_image,
            self.gui_creative_view,
            self.gui_creative_sampler,
            self.gui_creative_alloc
        );
        drop_gui_texture!(
            self.gui_options_bg_image,
            self.gui_options_bg_view,
            self.gui_options_bg_sampler,
            self.gui_options_bg_alloc
        );
        drop_gui_texture!(
            self.gui_underwater_image,
            self.gui_underwater_view,
            self.gui_underwater_sampler,
            self.gui_underwater_alloc
        );

        // Panorama
        let _ = super::raii::Pipeline::from_handle(
            device.clone(),
            std::mem::replace(&mut self.panorama_pipeline, vk::Pipeline::null()),
        );
        let _ = super::raii::PipelineLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.panorama_pipeline_layout, vk::PipelineLayout::null()),
        );
        let _ = super::raii::DescriptorPool::from_handle(
            device.clone(),
            std::mem::replace(&mut self.panorama_descriptor_pool, vk::DescriptorPool::null()),
        );
        let _ = super::raii::Buffer::from_handle(
            device.clone(),
            std::mem::replace(&mut self.panorama_uniform_buffer, vk::Buffer::null()),
        );
        self.resources
            .free(std::mem::take(&mut self.panorama_uniform_alloc));
        {
            let img = std::mem::replace(&mut self.panorama_image, vk::Image::null());
            if img != vk::Image::null() {
                let _ = super::raii::Image::from_handle(device.clone(), img);
                let _ = super::raii::ImageView::from_handle(
                    device.clone(),
                    std::mem::replace(&mut self.panorama_view, vk::ImageView::null()),
                );
                let _ = super::raii::Sampler::from_handle(
                    device.clone(),
                    std::mem::replace(&mut self.panorama_sampler, vk::Sampler::null()),
                );
                if let Some(a) = self.panorama_alloc.take() {
                    self.resources.free(a);
                }
            }
        }

        // Particle mesh buffers
        if let Some(b) = self.particle_vertex_buffer.take() {
            let _ = super::raii::Buffer::from_handle(device.clone(), b);
        }
        if let Some(b) = self.particle_index_buffer.take() {
            let _ = super::raii::Buffer::from_handle(device.clone(), b);
        }
        if let Some(a) = self.particle_vertex_alloc.take() {
            self.resources.free(a);
        }
        if let Some(a) = self.particle_index_alloc.take() {
            self.resources.free(a);
        }

        // Nametag mesh buffers
        if let Some(b) = self.nametag_vertex_buffer.take() {
            let _ = super::raii::Buffer::from_handle(device.clone(), b);
        }
        if let Some(b) = self.nametag_index_buffer.take() {
            let _ = super::raii::Buffer::from_handle(device.clone(), b);
        }
        if let Some(a) = self.nametag_vertex_alloc.take() {
            self.resources.free(a);
        }
        if let Some(a) = self.nametag_index_alloc.take() {
            self.resources.free(a);
        }
        let _ = super::raii::Pipeline::from_handle(
            device.clone(),
            std::mem::replace(&mut self.nametag_pipeline, vk::Pipeline::null()),
        );
        let _ = super::raii::PipelineLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.nametag_pipeline_layout, vk::PipelineLayout::null()),
        );

        // First-person hand buffers (vertex/index buffer + allocation pairs).
        macro_rules! drop_buffer_alloc {
            ($buf:expr, $alloc:expr) => {{
                if let Some(b) = $buf.take() {
                    let _ = super::raii::Buffer::from_handle(device.clone(), b);
                }
                if let Some(a) = $alloc.take() {
                    self.resources.free(a);
                }
            }};
        }
        drop_buffer_alloc!(self.fp_arm_vertex_buffer, self.fp_arm_vertex_alloc);
        drop_buffer_alloc!(self.fp_arm_index_buffer, self.fp_arm_index_alloc);
        drop_buffer_alloc!(self.fp_block_op_vertex_buffer, self.fp_block_op_vertex_alloc);
        drop_buffer_alloc!(self.fp_block_op_index_buffer, self.fp_block_op_index_alloc);
        drop_buffer_alloc!(self.fp_block_tr_vertex_buffer, self.fp_block_tr_vertex_alloc);
        drop_buffer_alloc!(self.fp_block_tr_index_buffer, self.fp_block_tr_index_alloc);

        // World texture
        if let Some(a) = self.texture_alloc.take() {
            self.resources.free(a);
        }
        let _ = super::raii::DescriptorPool::from_handle(
            device.clone(),
            std::mem::replace(&mut self.descriptor_pool, vk::DescriptorPool::null()),
        );
        let _ = super::raii::DescriptorSetLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.descriptor_layout, vk::DescriptorSetLayout::null()),
        );
        let _ = super::raii::ImageView::from_handle(
            device.clone(),
            std::mem::replace(&mut self.texture_image_view, vk::ImageView::null()),
        );
        let _ = super::raii::Sampler::from_handle(
            device.clone(),
            std::mem::replace(&mut self.texture_sampler, vk::Sampler::null()),
        );
        let _ = super::raii::Image::from_handle(
            device.clone(),
            std::mem::replace(&mut self.texture_image, vk::Image::null()),
        );

        // Depth buffer (owned by SwapchainManager)
        for view in self.swapchain.depth_image_views.drain(..) {
            let _ = super::raii::ImageView::from_handle(device.clone(), view);
        }
        for image in self.swapchain.depth_images.drain(..) {
            let _ = super::raii::Image::from_handle(device.clone(), image);
        }
        for alloc in self.swapchain.depth_allocs.drain(..) {
            self.resources.free(alloc);
        }

        // Pipelines
        let _ = super::raii::Pipeline::from_handle(
            device.clone(),
            std::mem::replace(&mut self.pipeline, vk::Pipeline::null()),
        );
        let _ = super::raii::Pipeline::from_handle(
            device.clone(),
            std::mem::replace(&mut self.transparent_pipeline, vk::Pipeline::null()),
        );
        let _ = super::raii::PipelineLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.pipeline_layout, vk::PipelineLayout::null()),
        );

        // Sky pipeline
        let _ = super::raii::Pipeline::from_handle(
            device.clone(),
            std::mem::replace(&mut self.sky_pipeline, vk::Pipeline::null()),
        );
        let _ = super::raii::PipelineLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.sky_pipeline_layout, vk::PipelineLayout::null()),
        );
        let _ = super::raii::DescriptorPool::from_handle(
            device.clone(),
            std::mem::replace(&mut self.sky_descriptor_pool, vk::DescriptorPool::null()),
        );
        let _ = super::raii::Buffer::from_handle(
            device.clone(),
            std::mem::replace(&mut self.sky_uniform_buffer, vk::Buffer::null()),
        );
        self.resources
            .free(std::mem::take(&mut self.sky_uniform_alloc));
        let _ = super::raii::Buffer::from_handle(
            device.clone(),
            std::mem::replace(&mut self.sky_vertex_buffer, vk::Buffer::null()),
        );
        self.resources
            .free(std::mem::take(&mut self.sky_vertex_alloc));

        // Entity / particle pipelines
        let _ = super::raii::Pipeline::from_handle(
            device.clone(),
            std::mem::replace(&mut self.entity_pipeline, vk::Pipeline::null()),
        );
        let _ = super::raii::PipelineLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.entity_pipeline_layout, vk::PipelineLayout::null()),
        );
        let _ = super::raii::Pipeline::from_handle(
            device.clone(),
            std::mem::replace(&mut self.particle_pipeline, vk::Pipeline::null()),
        );
        let _ = super::raii::PipelineLayout::from_handle(
            device.clone(),
            std::mem::replace(&mut self.particle_pipeline_layout, vk::PipelineLayout::null()),
        );
        let _ = super::raii::DescriptorPool::from_handle(
            device.clone(),
            std::mem::replace(&mut self.entity_descriptor_pool, vk::DescriptorPool::null()),
        );

        // Entity GPU meshes
        for (_, mut mesh) in self.entity_gpu_meshes.drain() {
            super::destroy_entity_gpu_mesh(&self.device, self.resources.allocator_mut(), &mut mesh);
        }

        // Entity vertex/index buffers
        if let Some(b) = self.entity_vertex_buffer.take() {
            let _ = super::raii::Buffer::from_handle(device.clone(), b);
        }
        if let Some(b) = self.entity_index_buffer.take() {
            let _ = super::raii::Buffer::from_handle(device.clone(), b);
        }
        if let Some(a) = self.entity_vertex_alloc.take() {
            self.resources.free(a);
        }
        if let Some(a) = self.entity_index_alloc.take() {
            self.resources.free(a);
        }
        drop_buffer_alloc!(
            self.entity_held_block_vertex_buffer,
            self.entity_held_block_vertex_alloc
        );
        drop_buffer_alloc!(
            self.entity_held_block_index_buffer,
            self.entity_held_block_index_alloc
        );
        drop_buffer_alloc!(
            self.entity_held_item_vertex_buffer,
            self.entity_held_item_vertex_alloc
        );
        drop_buffer_alloc!(
            self.entity_held_item_index_buffer,
            self.entity_held_item_index_alloc
        );

        // Entity texture
        {
            let img = std::mem::replace(&mut self.entity_texture_image, vk::Image::null());
            if img != vk::Image::null() {
                let _ = super::raii::Image::from_handle(device.clone(), img);
                let _ = super::raii::ImageView::from_handle(
                    device.clone(),
                    std::mem::replace(&mut self.entity_texture_view, vk::ImageView::null()),
                );
                let _ = super::raii::Sampler::from_handle(
                    device.clone(),
                    std::mem::replace(&mut self.entity_texture_sampler, vk::Sampler::null()),
                );
                if let Some(a) = self.entity_texture_alloc.take() {
                    self.resources.free(a);
                }
            }
        }

        // Skin / sun / moon / custom sky textures (image + view + alloc, no sampler).
        macro_rules! drop_image_texture {
            ($image:expr, $view:expr, $alloc:expr) => {{
                let img = std::mem::replace(&mut $image, vk::Image::null());
                if img != vk::Image::null() {
                    let _ = super::raii::Image::from_handle(device.clone(), img);
                    let _ = super::raii::ImageView::from_handle(
                        device.clone(),
                        std::mem::replace(&mut $view, vk::ImageView::null()),
                    );
                    if let Some(a) = $alloc.take() {
                        self.resources.free(a);
                    }
                }
            }};
        }
        drop_image_texture!(
            self.skin_texture_image,
            self.skin_texture_view,
            self.skin_texture_alloc
        );
        drop_image_texture!(
            self.sun_texture_image,
            self.sun_texture_view,
            self.sun_texture_alloc
        );
        drop_image_texture!(
            self.moon_texture_image,
            self.moon_texture_view,
            self.moon_texture_alloc
        );
        drop_image_texture!(
            self.custom_sky_texture_image,
            self.custom_sky_texture_view,
            self.custom_sky_texture_alloc
        );

        // Render pass, framebuffers, command pool, swapchain, surface
        let _ = super::raii::RenderPass::from_handle(
            device.clone(),
            std::mem::replace(&mut self.render_pass, vk::RenderPass::null()),
        );
        for fb in self.swapchain.framebuffers.drain(..) {
            let _ = super::raii::Framebuffer::from_handle(device.clone(), fb);
        }
        for v in self.swapchain.swapchain_image_views.drain(..) {
            let _ = super::raii::ImageView::from_handle(device.clone(), v);
        }
        let _ = super::raii::CommandPool::from_handle(
            device.clone(),
            std::mem::replace(&mut self.command_pool, vk::CommandPool::null()),
        );
        let _ = super::raii::Swapchain::from_handle(
            self.swapchain.swapchain_fn.clone(),
            std::mem::replace(&mut self.swapchain.swapchain, vk::SwapchainKHR::null()),
        );
        // Destroy surface while instance is still alive (vkDestroySurfaceKHR needs a valid instance).
        let _ = super::raii::Surface::from_handle(
            self.swapchain.surface_fn.clone(),
            std::mem::replace(&mut self.swapchain.surface, vk::SurfaceKHR::null()),
        );
        // NOTE: device and instance are NOT manually destroyed here —
        // ash::Device and ash::Instance drop impls will call vkDestroyDevice
        // and vkDestroyInstance respectively. Letting ash own this avoids a
        // double-destroy that can crash drivers with 0xc0000005.
    }
}
