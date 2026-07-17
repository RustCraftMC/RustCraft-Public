//! Vulkan upscaler integration boundary.
//!
//! FSR 3 needs color, depth, motion vectors, exposure, jitter and reactive
//! masks. Keeping that contract explicit prevents a backend from silently
//! falling back to a spatial scaler while claiming frame-generation support.

use ash::vk;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpscaleQuality {
    NativeAa,
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

#[derive(Clone, Copy, Debug)]
pub struct UpscaleFrame<'a> {
    pub command_buffer: vk::CommandBuffer,
    pub color: vk::ImageView,
    pub depth: vk::ImageView,
    pub motion_vectors: vk::ImageView,
    pub output: vk::ImageView,
    pub render_size: vk::Extent2D,
    pub output_size: vk::Extent2D,
    pub jitter: [f32; 2],
    pub camera_near: f32,
    pub camera_far: f32,
    pub frame_time_seconds: f32,
    pub reset_history: bool,
    pub reactive_mask: Option<vk::ImageView>,
    pub exposure: Option<vk::ImageView>,
    pub _lifetime: std::marker::PhantomData<&'a ()>,
}

pub trait VulkanUpscaler {
    fn name(&self) -> &'static str;
    fn resize(
        &mut self,
        render_size: vk::Extent2D,
        output_size: vk::Extent2D,
    ) -> Result<(), String>;
    fn dispatch(&mut self, frame: UpscaleFrame<'_>) -> Result<(), String>;
}

#[derive(Clone, Debug)]
pub struct Fsr3Status {
    pub available: bool,
    pub reason: String,
}

impl Fsr3Status {
    pub fn detect() -> Self {
        Self {
            available: false,
            reason: "FidelityFX FSR 3 SDK is not linked in this build".to_string(),
        }
    }
}
