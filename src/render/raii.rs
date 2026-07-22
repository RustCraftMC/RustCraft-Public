//! RAII wrappers for Vulkan resources.
//!
//! Each wrapper owns a Vulkan handle and invokes the matching `destroy_*`
//! on the device (or instance / extension entry point) when dropped.
//! `ash::Device` and the ash extension entry points
//! (`ash::khr::swapchain::Device`, `ash::khr::surface::Instance`,
//! `ash::ext::debug_utils::Instance`) are internally reference counted
//! (Arc-backed since ash 0.37), so cloning them is cheap and is safe to
//! store alongside the handle.
//!
//! These wrappers are consumed in two ways:
//! 1. `Renderer::drop` adopts the raw handles currently stored on the
//!    `Renderer` (via `from_handle`) and lets them drop, replacing the
//!    previous wall of manual `destroy_*` calls.
//! 2. Resource-creation helpers in `resources.rs` return these wrappers
//!    so freshly created handles are owned and freed automatically.
//!
//! Notes:
//! - `Buffer` and `Image` wrappers do NOT free the backing
//!   `gpu_allocator::vulkan::Allocation`; the caller must free it
//!   separately. The renderer's `Allocator` is `ManuallyDrop` and is not
//!   cheaply shareable, so RAII ownership of the allocation is left to
//!   the caller.

use ash::vk;

/// Defines a simple RAII wrapper around a device-owned Vulkan handle.
macro_rules! define_device_raii {
    (
        $(#[$meta:meta])*
        $name:ident, $handle_ty:ty, $destroy_method:ident
    ) => {
        $(#[$meta])*
        pub struct $name {
            handle: $handle_ty,
            device: ash::Device,
        }

        impl $name {
            /// Adopt an existing raw handle. The wrapper takes ownership
            /// and will destroy it on drop. Passing a null handle is
            /// allowed and is a no-op on drop.
            pub fn from_handle(device: ash::Device, handle: $handle_ty) -> Self {
                Self { handle, device }
            }

            /// Borrow the underlying raw handle.
            pub fn handle(&self) -> $handle_ty {
                self.handle
            }

            /// Take ownership of the inner handle without destroying it.
            /// Useful when migrating a field to RAII incrementally.
            pub fn into_handle(mut self) -> $handle_ty {
                let h = std::mem::replace(&mut self.handle, <$handle_ty>::null());
                // Drop runs with a null handle so the Vulkan object is retained
                // while the owned `device` clone is still cleaned up.
                h
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                if self.handle != <$handle_ty>::null() {
                    unsafe { self.device.$destroy_method(self.handle, None); }
                }
            }
        }
    };
}

define_device_raii!(
    /// RAII wrapper for `vk::ImageView`. Drops with `destroy_image_view`.
    ImageView,
    vk::ImageView,
    destroy_image_view
);

define_device_raii!(
    /// RAII wrapper for `vk::Sampler`. Drops with `destroy_sampler`.
    Sampler,
    vk::Sampler,
    destroy_sampler
);

define_device_raii!(
    /// RAII wrapper for `vk::Pipeline`. Drops with `destroy_pipeline`.
    Pipeline,
    vk::Pipeline,
    destroy_pipeline
);

define_device_raii!(
    /// RAII wrapper for `vk::PipelineLayout`. Drops with `destroy_pipeline_layout`.
    PipelineLayout,
    vk::PipelineLayout,
    destroy_pipeline_layout
);

define_device_raii!(
    /// RAII wrapper for `vk::DescriptorPool`. Drops with `destroy_descriptor_pool`.
    DescriptorPool,
    vk::DescriptorPool,
    destroy_descriptor_pool
);

define_device_raii!(
    /// RAII wrapper for `vk::DescriptorSetLayout`. Drops with `destroy_descriptor_set_layout`.
    DescriptorSetLayout,
    vk::DescriptorSetLayout,
    destroy_descriptor_set_layout
);

define_device_raii!(
    /// RAII wrapper for `vk::RenderPass`. Drops with `destroy_render_pass`.
    RenderPass,
    vk::RenderPass,
    destroy_render_pass
);

define_device_raii!(
    /// RAII wrapper for `vk::Framebuffer`. Drops with `destroy_framebuffer`.
    Framebuffer,
    vk::Framebuffer,
    destroy_framebuffer
);

define_device_raii!(
    /// RAII wrapper for `vk::CommandPool`. Drops with `destroy_command_pool`.
    CommandPool,
    vk::CommandPool,
    destroy_command_pool
);

define_device_raii!(
    /// RAII wrapper for `vk::Semaphore`. Drops with `destroy_semaphore`.
    Semaphore,
    vk::Semaphore,
    destroy_semaphore
);

define_device_raii!(
    /// RAII wrapper for `vk::Fence`. Drops with `destroy_fence`.
    Fence,
    vk::Fence,
    destroy_fence
);

define_device_raii!(
    /// RAII wrapper for `vk::Buffer`. Drops with `destroy_buffer`.
    ///
    /// NOTE: This wrapper does NOT free the backing
    /// `gpu_allocator::vulkan::Allocation`; the caller must free it
    /// separately.
    Buffer,
    vk::Buffer,
    destroy_buffer
);

define_device_raii!(
    /// RAII wrapper for `vk::Image`. Drops with `destroy_image`.
    ///
    /// NOTE: This wrapper does NOT free the backing
    /// `gpu_allocator::vulkan::Allocation`; the caller must free it
    /// separately.
    Image,
    vk::Image,
    destroy_image
);

// ---------------------------------------------------------------------------
// Handle wrappers that are not destroyed via `ash::Device`
// ---------------------------------------------------------------------------

/// RAII wrapper for `vk::SwapchainKHR`.
/// Drops with `swapchain_fn.destroy_swapchain`.
pub struct Swapchain {
    handle: vk::SwapchainKHR,
    swapchain_fn: ash::khr::swapchain::Device,
}

impl Swapchain {
    /// Adopt an existing raw swapchain handle. The wrapper takes ownership
    /// and will destroy it on drop. A null handle is a no-op on drop.
    pub fn from_handle(
        swapchain_fn: ash::khr::swapchain::Device,
        handle: vk::SwapchainKHR,
    ) -> Self {
        Self {
            handle,
            swapchain_fn,
        }
    }

    /// Borrow the underlying raw handle.
    pub fn handle(&self) -> vk::SwapchainKHR {
        self.handle
    }

    /// Take ownership of the inner handle without destroying it.
    pub fn into_handle(mut self) -> vk::SwapchainKHR {
        let h = std::mem::replace(&mut self.handle, vk::SwapchainKHR::null());
        h
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        if self.handle != vk::SwapchainKHR::null() {
            unsafe {
                self.swapchain_fn.destroy_swapchain(self.handle, None);
            }
        }
    }
}

/// RAII wrapper for `vk::SurfaceKHR`.
/// Drops with `surface_fn.destroy_surface`.
pub struct Surface {
    handle: vk::SurfaceKHR,
    surface_fn: ash::khr::surface::Instance,
}

impl Surface {
    /// Adopt an existing raw surface handle. The wrapper takes ownership
    /// and will destroy it on drop. A null handle is a no-op on drop.
    pub fn from_handle(
        surface_fn: ash::khr::surface::Instance,
        handle: vk::SurfaceKHR,
    ) -> Self {
        Self {
            handle,
            surface_fn,
        }
    }

    /// Borrow the underlying raw handle.
    pub fn handle(&self) -> vk::SurfaceKHR {
        self.handle
    }

    /// Take ownership of the inner handle without destroying it.
    pub fn into_handle(mut self) -> vk::SurfaceKHR {
        let h = std::mem::replace(&mut self.handle, vk::SurfaceKHR::null());
        h
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.handle != vk::SurfaceKHR::null() {
            unsafe {
                self.surface_fn.destroy_surface(self.handle, None);
            }
        }
    }
}

/// RAII wrapper for `vk::DebugUtilsMessengerEXT`.
/// Drops with `debug_utils.destroy_debug_utils_messenger`.
pub struct DebugMessenger {
    handle: vk::DebugUtilsMessengerEXT,
    debug_utils: ash::ext::debug_utils::Instance,
}

impl DebugMessenger {
    /// Adopt an existing raw debug messenger handle. The wrapper takes
    /// ownership and will destroy it on drop. A null handle is a no-op
    /// on drop.
    pub fn from_handle(
        debug_utils: ash::ext::debug_utils::Instance,
        handle: vk::DebugUtilsMessengerEXT,
    ) -> Self {
        Self {
            handle,
            debug_utils,
        }
    }

    /// Borrow the underlying raw handle.
    pub fn handle(&self) -> vk::DebugUtilsMessengerEXT {
        self.handle
    }

    /// Take ownership of the inner handle without destroying it.
    pub fn into_handle(mut self) -> vk::DebugUtilsMessengerEXT {
        let h = std::mem::replace(&mut self.handle, vk::DebugUtilsMessengerEXT::null());
        h
    }
}

impl Drop for DebugMessenger {
    fn drop(&mut self) {
        if self.handle != vk::DebugUtilsMessengerEXT::null() {
            unsafe {
                self.debug_utils
                    .destroy_debug_utils_messenger(self.handle, None);
            }
        }
    }
}
