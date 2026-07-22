//! GPU resource allocation — allocator, uniform buffers, buffer/image helpers.
//!
//! `ResourceManager` owns the Vulkan memory allocator and provides
//! buffer/image/sampler creation and destruction helpers. All GPU memory
//! allocation in the renderer goes through this subsystem.

use ash::vk;
use gpu_allocator::vulkan::Allocator;

use super::{Uniforms, MAX_FRAMES};

// ---------------------------------------------------------------------------
// ResourceManager
// ---------------------------------------------------------------------------

pub(crate) struct ResourceManager {
    // GPU memory allocator — ManuallyDrop to avoid double-destroy on exit
    // (the allocator's Drop calls vkDestroyDevice/vkDestroyInstance on
    // internal ash clones before our own Device/Instance drops).
    allocator: std::mem::ManuallyDrop<Allocator>,

    // Per-frame uniform buffers (shared across all render passes)
    uniform_buffers: Vec<vk::Buffer>,
    uniform_allocs: Vec<gpu_allocator::vulkan::Allocation>,
    uniform_mapped: Vec<*mut std::ffi::c_void>,
}

impl ResourceManager {
    /// Build a new `ResourceManager` from the freshly created allocator and
    /// uniform buffers produced during `Renderer::new`.
    pub(crate) fn new(
        allocator: Allocator,
        uniform_buffers: Vec<vk::Buffer>,
        uniform_allocs: Vec<gpu_allocator::vulkan::Allocation>,
        uniform_mapped: Vec<*mut std::ffi::c_void>,
    ) -> Self {
        Self {
            allocator: std::mem::ManuallyDrop::new(allocator),
            uniform_buffers,
            uniform_allocs,
            uniform_mapped,
        }
    }

    // --- Allocator accessors ---

    pub(crate) fn allocator(&self) -> &Allocator {
        &self.allocator
    }

    pub(crate) fn allocator_mut(&mut self) -> &mut Allocator {
        &mut self.allocator
    }

    /// Free a GPU allocation through the internal allocator.
    pub(crate) fn free(&mut self, allocation: gpu_allocator::vulkan::Allocation) {
        self.allocator.free(allocation).ok();
    }

    // --- Uniform buffer accessors ---

    pub(crate) fn uniform_buffers(&self) -> &[vk::Buffer] {
        &self.uniform_buffers
    }

    pub(crate) fn uniform_alloc(&self, frame: usize) -> &gpu_allocator::vulkan::Allocation {
        &self.uniform_allocs[frame]
    }

    // --- Uniform buffer creation (static helper) ---

    pub(crate) fn create_uniform_buffers(
        device: &ash::Device,
        allocator: &mut Allocator,
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

    // --- Drop helpers ---

    /// Destroy uniform buffers and free their allocations.
    /// Called during `Renderer::drop` before the device is destroyed.
    pub(crate) fn destroy_uniform_buffers(&mut self, device: &ash::Device) {
        for i in 0..self.uniform_buffers.len() {
            let _ = unsafe { device.destroy_buffer(self.uniform_buffers[i], None) };
        }
        for alloc in self.uniform_allocs.drain(..) {
            self.allocator.free(alloc).ok();
        }
        self.uniform_buffers.clear();
        self.uniform_mapped.clear();
    }

    /// Replace a uniform buffer handle with null and return the old handle
    /// (for RAII adoption during Drop).
    pub(crate) fn take_uniform_buffer(&mut self, index: usize) -> vk::Buffer {
        std::mem::replace(&mut self.uniform_buffers[index], vk::Buffer::null())
    }
}

// The allocator must NOT be dropped automatically — Renderer handles
// the destruction order explicitly. ManuallyDrop prevents the default
// Drop impl from running.
// No `impl Drop` here: the parent Renderer calls `destroy_uniform_buffers`
// and manually handles the allocator lifetime.
