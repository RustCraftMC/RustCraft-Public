//! Sky renderer — day/night sky gradient with sun/moon.
//!
//! Uses a fullscreen triangle with the sky fragment shader.
//! Sun, moon phases, and fixed stars are rendered in the fragment shader.

use super::{Renderer, SkyUniforms};
use ash::vk;

impl Renderer {
    /// Load/reload the custom sky texture from pixel data.  Called during
    /// resource-pack reload when `mcpatcher/sky/` content changes.
    pub fn reload_custom_sky(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        data: super::custom_sky::CustomSky,
    ) {
        // Destroy old image and its descriptor-bound view
        if self.custom_sky_texture_image != vk::Image::null() {
            unsafe {
                self.device
                    .destroy_image_view(self.custom_sky_texture_view, None);
                self.device
                    .destroy_image(self.custom_sky_texture_image, None);
            }
            if let Some(alloc) = self.custom_sky_texture_alloc.take() {
                self.allocator.free(alloc).ok();
            }
        }

        let reqs = vk::ImageCreateInfo {
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
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            ..Default::default()
        };
        let image = unsafe { self.device.create_image(&reqs, None) }.unwrap();
        let mem_reqs = unsafe { self.device.get_image_memory_requirements(image) };
        let alloc = self
            .allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "custom_sky",
                requirements: mem_reqs,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            self.device
                .bind_image_memory(image, alloc.memory(), alloc.offset())
                .unwrap();
        }
        let view = unsafe {
            self.device.create_image_view(
                &vk::ImageViewCreateInfo {
                    image,
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: vk::Format::R8G8B8A8_SRGB,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        super::resources::upload_new_gpu_image(
            &self.device,
            &mut self.allocator,
            self.command_pool,
            self.queue,
            image,
            width,
            height,
            pixels,
        );

        self.custom_sky_texture_image = image;
        self.custom_sky_texture_view = view;
        self.custom_sky_texture_alloc = Some(alloc);
        self.custom_sky_data = Some(data);

        // Re-write descriptor sets to bind the new image view
        for &set in &self.sky_descriptor_sets {
            let img_info = vk::DescriptorImageInfo {
                sampler: self.entity_texture_sampler,
                image_view: self.custom_sky_texture_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };
            unsafe {
                self.device.update_descriptor_sets(
                    &[vk::WriteDescriptorSet {
                        dst_set: set,
                        dst_binding: 3,
                        dst_array_element: 0,
                        descriptor_count: 1,
                        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        p_image_info: &img_info,
                        ..Default::default()
                    }],
                    &[],
                );
            }
        }
    }

    /// Load custom sky from resource packs via a resolver that already has packs
    /// mounted. Call after the resolver is built with `enabled_resource_packs`.
    pub fn load_custom_sky_from_packs(
        &mut self,
        resolver: &mut crate::assets::resolver::AssetResolver,
        dimension: i8,
    ) {
        if let Some(mut custom_sky) = super::custom_sky::CustomSky::load(resolver, dimension) {
            let pixels = custom_sky.layers.first().map(|l| l.pixels.clone());
            let (w, h) = custom_sky
                .layers
                .first()
                .map_or((1, 1), |l| (l.width, l.height));
            if let Some(ref pixels) = pixels {
                log::info!("loaded custom MCPatcher sky: dimensions={w}x{h}");
                self.reload_custom_sky(pixels, w, h, custom_sky);
            }
        }
    }
    /// Upload sky uniform data for the current frame.
    pub(super) fn update_sky_uniforms(
        &mut self,
        camera: &crate::client::player::Camera,
        zenith: [f32; 3],
        horizon: [f32; 3],
        sun_dir: [f32; 3],
        sun_brightness: f32,
        moon_phase: f32,
        viewport_width: f32,
        viewport_height: f32,
        _ambient: f32,
        daylight: f32,
    ) {
        let view = camera.sky_view_matrix();
        // Match the world pass exactly during sprint/use-item FOV transitions.
        let proj = camera.projection_matrix_at(camera.partial_tick);
        let inv_view_proj = (proj * view)
            .try_inverse()
            .unwrap_or_else(nalgebra::Matrix4::identity);

        let (custom_alpha, custom_rot) = if let Some(ref sky) = self.custom_sky_data {
            let time_f = self.state.day_time as f32;
            let alpha = sky
                .layers
                .iter()
                .map(|l| super::custom_sky::CustomSky::layer_alpha(l, time_f))
                .fold(0.0f32, |a, b| a + b - a * b);
            let rot = sky
                .layers
                .iter()
                .find(|l| l.rotate)
                .map(|l| (self.state.day_time as f32 * l.speed).to_radians())
                .unwrap_or(0.0);
            (alpha.clamp(0.0, 1.0), rot)
        } else {
            (0.0, 0.0)
        };

        let uniforms = SkyUniforms {
            zenith: [zenith[0], zenith[1], zenith[2], 1.0],
            horizon: [horizon[0], horizon[1], horizon[2], 1.0],
            sun_dir: [sun_dir[0], sun_dir[1], sun_dir[2], sun_brightness],
            fog_params: [viewport_width, moon_phase, viewport_height, daylight],
            custom_sky: [custom_alpha, custom_rot, 0.0, 0.0],
            inv_view_proj: inv_view_proj.into(),
        };

        let data = bytemuck::bytes_of(&uniforms);
        unsafe {
            let mapped = self
                .device
                .map_memory(
                    self.sky_uniform_alloc.memory(),
                    self.sky_uniform_alloc.offset(),
                    std::mem::size_of::<SkyUniforms>() as u64,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            std::ptr::copy_nonoverlapping(data.as_ptr(), mapped as *mut u8, data.len());
            self.device.unmap_memory(self.sky_uniform_alloc.memory());
        }
    }

    /// Draw the sky. Called before world rendering.
    pub(super) fn draw_panorama(&mut self, cb: vk::CommandBuffer, frame_index: usize) {
        let time = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            % 3600.0) as f32;
        let aspect = self.swapchain_extent.width as f32 / self.swapchain_extent.height as f32;
        let uniforms = [time, aspect];

        unsafe {
            let alloc = &self.panorama_uniform_alloc;
            if let Some(mapped) = alloc.mapped_ptr() {
                let memory = mapped.as_ptr() as *mut f32;
                std::ptr::copy_nonoverlapping(uniforms.as_ptr(), memory, 2);
            }

            self.device.cmd_bind_pipeline(
                cb,
                vk::PipelineBindPoint::GRAPHICS,
                self.panorama_pipeline,
            );
            self.device.cmd_bind_descriptor_sets(
                cb,
                vk::PipelineBindPoint::GRAPHICS,
                self.panorama_pipeline_layout,
                0,
                &[self.panorama_descriptor_sets[frame_index]],
                &[],
            );

            self.device.cmd_set_viewport(
                cb,
                0,
                &[vk::Viewport {
                    width: self.swapchain_extent.width as f32,
                    height: self.swapchain_extent.height as f32,
                    max_depth: 1.0,
                    ..Default::default()
                }],
            );
            self.device.cmd_set_scissor(
                cb,
                0,
                &[vk::Rect2D {
                    extent: self.swapchain_extent,
                    ..Default::default()
                }],
            );

            self.device.cmd_draw(cb, 3, 1, 0, 0);
        }
    }

    pub(super) fn draw_sky(&self, cmd: vk::CommandBuffer, frame: usize) {
        if self.sky_pipeline == vk::Pipeline::null() {
            return;
        }

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.swapchain_extent.width as f32,
            height: self.swapchain_extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissors = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: self.swapchain_extent,
        }];

        unsafe {
            self.device.cmd_set_viewport(cmd, 0, &[viewport]);
            self.device.cmd_set_scissor(cmd, 0, &scissors);

            // Bind sky pipeline
            self.device
                .cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.sky_pipeline);

            // Bind sky descriptor set (uniforms)
            if frame < self.sky_descriptor_sets.len() {
                self.device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.sky_pipeline_layout,
                    0,
                    &[self.sky_descriptor_sets[frame]],
                    &[],
                );
            }

            // Bind vertex buffer and draw fullscreen triangle
            self.device
                .cmd_bind_vertex_buffers(cmd, 0, &[self.sky_vertex_buffer], &[0]);
            self.device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }
}

// ---------------------------------------------------------------------------
// Sky color calculation
// ---------------------------------------------------------------------------

/// GPU-facing sky/light values derived from Minecraft time.
#[derive(Clone, Copy, Debug)]
pub struct SkyEnvironment {
    pub clear_color: [f32; 4],
    pub fog_color: [f32; 4],
    pub light_dir: [f32; 4],
    pub fog_params: [f32; 4],
}

/// Sky gradient colors for different times of day.
pub struct SkyGradient;

impl SkyGradient {
    const DAY_TICKS: f32 = 24000.0;

    pub fn environment(day_time: i64, render_distance: u8, dimension: i8) -> SkyEnvironment {
        match dimension {
            -1 => Self::fixed_environment(
                [0.18, 0.035, 0.025, 1.0],
                [0.33, 0.055, 0.035, 1.0],
                [0.35, -0.65, 0.25],
                0.48,
                0.35,
                render_distance,
            ),
            1 => Self::fixed_environment(
                [0.012, 0.010, 0.030, 1.0],
                [0.035, 0.025, 0.060, 1.0],
                [0.20, -0.35, 0.85],
                0.30,
                0.10,
                render_distance,
            ),
            _ => Self::overworld_environment(day_time, render_distance),
        }
    }

    fn overworld_environment(day_time: i64, render_distance: u8) -> SkyEnvironment {
        let ticks = day_time as f32;
        let day = Self::daylight_factor(ticks);
        let dusk = Self::sunset_factor(ticks);
        let ambient = lerp(0.18, 0.46, day) + dusk * 0.04;
        let fog = Self::fog_color(ticks);
        let clear = Self::clear_color(ticks);
        let far = (render_distance.max(2) as f32 * 16.0).max(48.0);

        SkyEnvironment {
            clear_color: clear,
            fog_color: fog,
            light_dir: with_w(Self::sun_direction(ticks), ambient),
            fog_params: [far * 0.72, far, ambient, day],
        }
    }

    fn fixed_environment(
        clear_color: [f32; 4],
        fog_color: [f32; 4],
        light_dir: [f32; 3],
        ambient: f32,
        daylight: f32,
        render_distance: u8,
    ) -> SkyEnvironment {
        let far = (render_distance.max(2) as f32 * 16.0).max(48.0);
        SkyEnvironment {
            clear_color,
            fog_color,
            light_dir: with_w(normalize(light_dir), ambient),
            fog_params: [far * 0.45, far * 0.82, ambient, daylight],
        }
    }

    /// MC 1.8.9 matching celestial angle (used by sky rendering).
    /// Returns 0 at dawn, ~0.25 at noon, ~0.5 at dusk, ~0.75 at midnight
    /// after the vanilla easing curve.
    pub fn celestial_angle(time_of_day: f32) -> f32 {
        let t = (time_of_day % Self::DAY_TICKS) / Self::DAY_TICKS;
        let mut f = t - 0.25;
        if f < 0.0 {
            f += 1.0;
        }
        if f > 1.0 {
            f -= 1.0;
        }
        let original = f;
        let eased = 1.0 - ((f * std::f32::consts::PI).cos() + 1.0) * 0.5;
        original + (eased - original) / 3.0
    }

    /// Daylight brightness factor used by the sky gradient.
    /// 1.0 at noon and 0.0 at midnight with a smooth cosine transition.
    pub fn daylight_factor(time_of_day: f32) -> f32 {
        (Self::celestial_angle(time_of_day)
            .mul_add(std::f32::consts::TAU, 0.0)
            .cos()
            * 2.0
            + 0.5)
            .clamp(0.0, 1.0)
    }

    pub fn sun_brightness(time_of_day: f32, rain: f32, thunder: f32) -> f32 {
        let angle = Self::celestial_angle(time_of_day) * std::f32::consts::TAU;
        let mut brightness = 1.0 - (1.0 - (angle.cos() * 2.0 + 0.2)).clamp(0.0, 1.0);
        brightness *= 1.0 - rain.clamp(0.0, 1.0) * 5.0 / 16.0;
        brightness *= 1.0 - thunder.clamp(0.0, 1.0) * 5.0 / 16.0;
        brightness
    }

    /// Sunset glow factor (0-1), peaks at dusk/dawn.
    pub fn sunset_factor(time_of_day: f32) -> f32 {
        let t = (time_of_day % Self::DAY_TICKS) / Self::DAY_TICKS;
        let angle = (t - 0.25) * std::f32::consts::TAU;
        let horizon = 1.0 - angle.cos().abs();
        let daylight = Self::daylight_factor(time_of_day);
        (horizon.clamp(0.0, 1.0).powi(5) * (1.0 - (daylight - 0.5).abs() * 1.4)).clamp(0.0, 1.0)
    }

    pub fn clear_color(time_of_day: f32) -> [f32; 4] {
        let top = Self::zenith_color(time_of_day);
        let horizon = Self::horizon_color(time_of_day);
        [
            lerp(horizon[0], top[0], 0.42),
            lerp(horizon[1], top[1], 0.42),
            lerp(horizon[2], top[2], 0.42),
            1.0,
        ]
    }

    pub fn zenith_color(time_of_day: f32) -> [f32; 3] {
        let day = Self::daylight_factor(time_of_day);
        [0.29 * day, 0.53 * day, 0.88 * day]
    }

    pub fn horizon_color(time_of_day: f32) -> [f32; 3] {
        let day = Self::daylight_factor(time_of_day);
        let brightness = day * 0.94 + 0.06;
        [0.69 * brightness, 0.77 * brightness, 0.89 * day + 0.11]
    }

    pub fn fog_color(time_of_day: f32) -> [f32; 4] {
        let h = Self::horizon_color(time_of_day);
        [h[0], h[1], h[2], 1.0]
    }

    pub fn sun_color(time_of_day: f32) -> [f32; 4] {
        let direction = Self::sun_direction(time_of_day);
        let elevation = -direction[1];
        if elevation < -0.1 {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            let brightness = elevation.clamp(0.0, 1.0);
            let sf = (1.0 - elevation).powi(2).min(1.0);
            [1.0 - sf * 0.2, 1.0 - sf * 0.5, 1.0 - sf * 0.7, brightness]
        }
    }

    pub fn sun_direction(time_of_day: f32) -> [f32; 3] {
        let angle = Self::celestial_angle(time_of_day) * std::f32::consts::TAU;
        [angle.sin(), -angle.cos(), 0.0]
    }

    pub fn moon_phase(day_time: i64) -> u8 {
        ((day_time / 24000).rem_euclid(8)) as u8
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= f32::EPSILON {
        [0.0, -1.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}
fn with_w(v: [f32; 3], w: f32) -> [f32; 4] {
    [v[0], v[1], v[2], w]
}

#[cfg(test)]
mod tests {
    use super::SkyGradient;

    #[test]
    fn daylight_matches_vanilla_cycle() {
        assert!((SkyGradient::daylight_factor(6000.0) - 1.0).abs() < 0.0001);
        assert!(SkyGradient::daylight_factor(18000.0) < 0.0001);
    }

    #[test]
    fn moon_phase_wraps_over_eight_days() {
        assert_eq!(SkyGradient::moon_phase(0), 0);
        assert_eq!(SkyGradient::moon_phase(7 * 24000), 7);
        assert_eq!(SkyGradient::moon_phase(8 * 24000), 0);
    }

    #[test]
    fn sunrise_glow_is_limited_to_the_horizon() {
        assert!(SkyGradient::sunset_factor(0.0) > 0.0);
        assert_eq!(SkyGradient::sunset_factor(6000.0), 0.0);
        assert_eq!(SkyGradient::sunset_factor(18000.0), 0.0);
    }
}
