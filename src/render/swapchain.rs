//! Swapchain, depth buffer, framebuffer, and frame-sync management.
//!
//! `SwapchainManager` owns all resources that must be recreated when the
//! window is resized: the Vulkan swapchain itself, depth images, framebuffers,
//! and per-frame synchronisation primitives.

use ash::vk;
use gpu_allocator::vulkan::Allocator;

use super::{color_subresource, depth_subresource, MAX_FRAMES};

// ---------------------------------------------------------------------------
// SwapchainManager
// ---------------------------------------------------------------------------

pub(crate) struct SwapchainManager {
    // Surface
    pub(crate) surface_fn: ash::khr::surface::Instance,
    pub(crate) surface: vk::SurfaceKHR,

    // Swapchain
    pub(crate) swapchain_fn: ash::khr::swapchain::Device,
    pub(crate) swapchain: vk::SwapchainKHR,
    pub(crate) _swapchain_images: Vec<vk::Image>,
    pub(crate) swapchain_image_views: Vec<vk::ImageView>,
    pub(crate) _swapchain_format: vk::Format,
    pub(crate) swapchain_extent: vk::Extent2D,

    // Depth
    pub(crate) depth_images: Vec<vk::Image>,
    pub(crate) depth_image_views: Vec<vk::ImageView>,
    pub(crate) depth_format: vk::Format,
    pub(crate) depth_allocs: Vec<gpu_allocator::vulkan::Allocation>,

    // Framebuffers
    pub(crate) framebuffers: Vec<vk::Framebuffer>,

    // Sync
    pub(crate) image_available: Vec<vk::Semaphore>,
    pub(crate) render_finished: Vec<vk::Semaphore>,
    pub(crate) in_flight_fences: Vec<vk::Fence>,
    pub(crate) current_frame: usize,

    // Window state
    pub(crate) window_size: (u32, u32),
    pub(crate) needs_recreate: bool,
}

impl SwapchainManager {
    /// Build a new `SwapchainManager` from the freshly created Vulkan objects
    /// produced during `Renderer::new`.
    pub(crate) fn new(
        surface_fn: ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
        swapchain_fn: ash::khr::swapchain::Device,
        swapchain: vk::SwapchainKHR,
        swapchain_images: Vec<vk::Image>,
        swapchain_image_views: Vec<vk::ImageView>,
        swapchain_format: vk::Format,
        swapchain_extent: vk::Extent2D,
        depth_images: Vec<vk::Image>,
        depth_image_views: Vec<vk::ImageView>,
        depth_format: vk::Format,
        depth_allocs: Vec<gpu_allocator::vulkan::Allocation>,
        framebuffers: Vec<vk::Framebuffer>,
        image_available: Vec<vk::Semaphore>,
        render_finished: Vec<vk::Semaphore>,
        in_flight_fences: Vec<vk::Fence>,
        window_size: (u32, u32),
    ) -> Self {
        Self {
            surface_fn,
            surface,
            swapchain_fn,
            swapchain,
            _swapchain_images: swapchain_images,
            swapchain_image_views,
            _swapchain_format: swapchain_format,
            swapchain_extent,
            depth_images,
            depth_image_views,
            depth_format,
            depth_allocs,
            framebuffers,
            image_available,
            render_finished,
            in_flight_fences,
            current_frame: 0,
            window_size,
            needs_recreate: false,
        }
    }

    // --- Swapchain creation (static helper) ---

    pub(crate) fn create_swapchain(
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

    pub(crate) fn create_depth_buffer(
        device: &ash::Device,
        allocator: &mut Allocator,
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

    pub(crate) fn create_depth_buffers(
        device: &ash::Device,
        allocator: &mut Allocator,
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

    pub(crate) fn create_framebuffers(
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

    pub(crate) fn create_sync_objects(
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

    // --- Recreate ---

    pub(crate) fn recreate(
        &mut self,
        device: &ash::Device,
        allocator: &mut Allocator,
        physical_device: vk::PhysicalDevice,
        render_pass: vk::RenderPass,
    ) {
        let window_size = self.window_size;
        if window_size.0 == 0 || window_size.1 == 0 {
            return;
        }
        unsafe {
            device.device_wait_idle().ok();
        }

        unsafe {
            for fb in &self.framebuffers {
                device.destroy_framebuffer(*fb, None);
            }
            for view in self.depth_image_views.drain(..) {
                device.destroy_image_view(view, None);
            }
            for image in self.depth_images.drain(..) {
                device.destroy_image(image, None);
            }
            for allocation in self.depth_allocs.drain(..) {
                allocator.free(allocation).ok();
            }
            for v in &self.swapchain_image_views {
                device.destroy_image_view(*v, None);
            }
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
        }

        let (sc, images, views, format, extent) = Self::create_swapchain(
            &self.surface_fn,
            &self.swapchain_fn,
            device,
            physical_device,
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
            device,
            allocator,
            self.depth_format,
            extent,
            self.swapchain_image_views.len(),
        );
        self.depth_images = depth_images;
        self.depth_image_views = depth_image_views;
        self.depth_allocs = depth_allocs;

        self.framebuffers = Self::create_framebuffers(
            device,
            render_pass,
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

    pub(crate) fn notify_resize(&mut self, width: u32, height: u32) {
        self.window_size = (width, height);
        self.needs_recreate = true;
    }

    /// Advance to the next frame index (round-robin).
    pub(crate) fn advance_frame(&mut self) {
        self.current_frame = (self.current_frame + 1) % MAX_FRAMES;
    }

    // --- Acquire / present helpers ---

    /// Acquire the next swapchain image. Returns `Ok(image_index)` on success
    /// or `Err(vk::Result)` on out-of-date / suboptimal.
    pub(crate) fn acquire_next_image(
        &self,
        device: &ash::Device,
    ) -> Result<(u32, bool), vk::Result> {
        let frame = self.current_frame;
        unsafe {
            self.swapchain_fn.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available[frame],
                vk::Fence::null(),
            )
        }
    }

    /// Present the current frame to the swapchain.
    /// Returns `true` if the swapchain needs recreation.
    pub(crate) fn present(
        &self,
        queue: vk::Queue,
        image_index: u32,
        frame: usize,
    ) -> bool {
        let present_info = vk::PresentInfoKHR {
            wait_semaphore_count: 1,
            p_wait_semaphores: &self.render_finished[frame],
            swapchain_count: 1,
            p_swapchains: &self.swapchain,
            p_image_indices: &image_index,
            ..Default::default()
        };
        unsafe {
            match self.swapchain_fn.queue_present(queue, &present_info) {
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
                Err(e) => {
                    log::error!("failed to present swapchain image: {e:?}");
                    false
                }
                _ => false,
            }
        }
    }
}
