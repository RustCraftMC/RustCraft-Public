//! GUI Vulkan runtime.

use super::super::color_subresource;
use ash::{vk, Device};
use std::ffi::CString;

use super::super::{GUI_TEXTURE_COUNT, MAX_FRAMES};

const GUI_TEX_FONT: usize = 0;
const GUI_TEX_WIDGETS: usize = 1;
const GUI_TEX_BLOCKS: usize = 2;
const GUI_TEX_INVENTORY: usize = 3;
const GUI_TEX_GENERIC54: usize = 4;
const GUI_TEX_ITEMS: usize = 5;
const GUI_TEX_ICONS: usize = 6;
const GUI_TEX_CREATIVE: usize = 7;
const GUI_TEX_OPTIONS_BG: usize = 8;
const GUI_TEX_UNDERWATER: usize = 9;
// Untextured full-screen menu overlay; shares the font image (never sampled,
// fill_rect uses uv = -1) but needs its own per-frame buffer slot so it does
// not clobber the font batch uploaded later in the same command buffer.
const GUI_TEX_OVERLAY: usize = 10;

impl super::super::Renderer {
    // --- GUI initialization (called once after Renderer::new) ---

    pub fn init_gui(&mut self, resolver: &mut crate::assets::resolver::AssetResolver) {
        // Widget texture
        let (wi, wv, ws, wa, _) = Self::load_gui_texture_from_resolver(
            &self.device,
            &mut self.allocator,
            resolver,
            "minecraft/textures/gui/widgets.png",
            self.command_pool,
            self.queue,
        );
        self.gui_widget_image = wi;
        self.gui_widget_view = wv;
        self.gui_widget_sampler = ws;
        self.gui_widget_alloc = wa;

        let (ii, iv, is_, ia, inv_size) = Self::load_gui_texture_from_resolver(
            &self.device,
            &mut self.allocator,
            resolver,
            "minecraft/textures/gui/container/inventory.png",
            self.command_pool,
            self.queue,
        );
        self.gui_inventory_image = ii;
        self.gui_inventory_view = iv;
        self.gui_inventory_sampler = is_;
        self.gui_inventory_alloc = ia;
        self.gui_inventory_size = [inv_size.0, inv_size.1];

        let (gi, gv, gs, ga) = Self::load_container_gui_atlas(
            &self.device,
            &mut self.allocator,
            resolver,
            self.command_pool,
            self.queue,
        );
        self.gui_generic54_image = gi;
        self.gui_generic54_view = gv;
        self.gui_generic54_sampler = gs;
        self.gui_generic54_alloc = ga;

        let (iti, itv, its, ita) = Self::load_item_icon_atlas_from_resolver(
            &self.device,
            &mut self.allocator,
            resolver,
            self.command_pool,
            self.queue,
        );
        self.gui_items_image = iti;
        self.gui_items_view = itv;
        self.gui_items_sampler = its;
        self.gui_items_alloc = ita;

        // Held/generated items use the same 256x256 atlas and normalized UVs as
        // the inventory. Keep their world-pipeline descriptor bound to that
        // image instead of the unrelated 2048x2048 entity atlas.
        Self::write_descriptors(
            &self.device,
            &self.fp_item_descriptor_sets,
            &self.uniform_buffers,
            self.gui_items_view,
            self.gui_items_sampler,
        );

        // Icons texture (icons.png - hearts, hunger, armor, XP bar)
        let (icon_i, icon_v, icon_s, icon_a, _) = Self::load_gui_texture_from_resolver(
            &self.device,
            &mut self.allocator,
            resolver,
            "minecraft/textures/gui/icons.png",
            self.command_pool,
            self.queue,
        );
        self.gui_icons_image = icon_i;
        self.gui_icons_view = icon_v;
        self.gui_icons_sampler = icon_s;
        self.gui_icons_alloc = icon_a;

        // Creative inventory atlas (tabs + tab panel backgrounds)
        let (ci, cv, cs, ca) = Self::load_creative_gui_atlas(
            &self.device,
            &mut self.allocator,
            resolver,
            self.command_pool,
            self.queue,
        );
        self.gui_creative_image = ci;
        self.gui_creative_view = cv;
        self.gui_creative_sampler = cs;
        self.gui_creative_alloc = ca;

        let (obi, obv, obs, oba, _) = Self::load_gui_texture_from_resolver(
            &self.device,
            &mut self.allocator,
            resolver,
            "minecraft/textures/gui/options_background.png",
            self.command_pool,
            self.queue,
        );
        self.gui_options_bg_image = obi;
        self.gui_options_bg_view = obv;
        self.gui_options_bg_sampler = obs;
        self.gui_options_bg_alloc = oba;

        // Underwater overlay texture
        let (uwi, uwv, uws, uwa, _) = Self::load_gui_texture_from_resolver(
            &self.device,
            &mut self.allocator,
            resolver,
            "minecraft/textures/misc/underwater.png",
            self.command_pool,
            self.queue,
        );
        self.gui_underwater_image = uwi;
        self.gui_underwater_view = uwv;
        self.gui_underwater_sampler = uws;
        self.gui_underwater_alloc = uwa;

        // Font atlas (initially empty, populated lazily by fontdue)
        let (fi, fv, fs, fa) = Self::create_empty_texture(
            &self.device,
            &mut self.allocator,
            self.font.atlas_width,
            self.font.atlas_height,
        );
        self.gui_font_image = fi;
        self.gui_font_view = fv;
        self.gui_font_sampler = fs;
        self.gui_font_alloc = fa;

        // Pre-populate font atlas with ASCII + CJK characters at common sizes
        self.font.preload_ascii();

        // GUI uniform buffer
        let (ub, ua) = Self::create_uniform_buf(
            &self.device,
            &mut self.allocator,
            std::mem::size_of::<super::GuiUniforms>(),
        );
        self.gui_uniform_buffer = ub;
        self.gui_uniform_alloc = ua;

        // GUI pipeline
        let (pipeline, pipeline_layout, descriptor_layout) =
            Self::create_gui_pipeline(&self.device, self.render_pass);

        // GUI descriptors
        let (dpool, dsets) = Self::create_gui_descriptors(&self.device, descriptor_layout);
        self.gui_pipeline = pipeline;
        self.gui_pipeline_layout = pipeline_layout;
        self.gui_descriptor_layout = descriptor_layout;
        self.gui_descriptor_pool = dpool;
        self.gui_descriptor_sets = dsets;

        let (pi, pv, ps, pa) = Self::load_panorama_atlas(
            &self.device,
            &mut self.allocator,
            resolver,
            self.command_pool,
            self.queue,
        );
        self.panorama_image = pi;
        self.panorama_view = pv;
        self.panorama_sampler = ps;
        self.panorama_alloc = Some(pa);

        let (pubuf, palloc) = Self::create_uniform_buf(&self.device, &mut self.allocator, 8);
        self.panorama_uniform_buffer = pubuf;
        self.panorama_uniform_alloc = palloc;

        let (ppipe, pplayout, pdlayout) =
            Self::create_panorama_pipeline(&self.device, self.render_pass);
        let (pdpool, pdsets) = Self::create_panorama_descriptors(&self.device, pdlayout);

        self.panorama_pipeline = ppipe;
        self.panorama_pipeline_layout = pplayout;
        self.panorama_descriptor_pool = pdpool;
        self.panorama_descriptor_sets = pdsets;

        let panorama_buffer_info = vk::DescriptorBufferInfo {
            buffer: self.panorama_uniform_buffer,
            offset: 0,
            range: 8,
        };
        let panorama_image_info = vk::DescriptorImageInfo {
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            image_view: self.panorama_view,
            sampler: self.panorama_sampler,
        };
        for &set in &self.panorama_descriptor_sets {
            unsafe {
                self.device.update_descriptor_sets(
                    &[
                        vk::WriteDescriptorSet {
                            dst_set: set,
                            dst_binding: 0,
                            descriptor_count: 1,
                            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                            p_buffer_info: &panorama_buffer_info,
                            ..Default::default()
                        },
                        vk::WriteDescriptorSet {
                            dst_set: set,
                            dst_binding: 1,
                            descriptor_count: 1,
                            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            p_image_info: &panorama_image_info,
                            ..Default::default()
                        },
                    ],
                    &[],
                );
            }
        }
        unsafe {
            self.device.destroy_descriptor_set_layout(pdlayout, None);
        }

        // Write descriptors for every frame/GUI texture pair.
        self.write_gui_texture_descriptors();
    }

    /// (Re)bind every GUI texture to its descriptor sets. Called once at init
    /// and again whenever a resource-pack reload replaces the GPU images.
    pub(in crate::render) fn write_gui_texture_descriptors(&self) {
        for i in 0..MAX_FRAMES {
            let buf_info = [vk::DescriptorBufferInfo {
                buffer: self.gui_uniform_buffer,
                offset: 0,
                range: std::mem::size_of::<super::GuiUniforms>() as u64,
            }];
            let font_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_font_view,
                sampler: self.gui_font_sampler,
            }];
            let widget_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_widget_view,
                sampler: self.gui_widget_sampler,
            }];
            let block_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.texture_image_view,
                sampler: self.texture_sampler,
            }];
            let inventory_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_inventory_view,
                sampler: self.gui_inventory_sampler,
            }];
            let generic54_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_generic54_view,
                sampler: self.gui_generic54_sampler,
            }];
            let items_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_items_view,
                sampler: self.gui_items_sampler,
            }];
            let icons_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_icons_view,
                sampler: self.gui_icons_sampler,
            }];
            let creative_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_creative_view,
                sampler: self.gui_creative_sampler,
            }];
            let options_bg_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_options_bg_view,
                sampler: self.gui_options_bg_sampler,
            }];
            let underwater_info = [vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.gui_underwater_view,
                sampler: self.gui_underwater_sampler,
            }];
            unsafe {
                for (tex, image_info) in [
                    (GUI_TEX_FONT, font_info.as_ptr()),
                    (GUI_TEX_WIDGETS, widget_info.as_ptr()),
                    (GUI_TEX_BLOCKS, block_info.as_ptr()),
                    (GUI_TEX_INVENTORY, inventory_info.as_ptr()),
                    (GUI_TEX_GENERIC54, generic54_info.as_ptr()),
                    (GUI_TEX_ITEMS, items_info.as_ptr()),
                    (GUI_TEX_ICONS, icons_info.as_ptr()),
                    (GUI_TEX_CREATIVE, creative_info.as_ptr()),
                    (GUI_TEX_OPTIONS_BG, options_bg_info.as_ptr()),
                    (GUI_TEX_UNDERWATER, underwater_info.as_ptr()),
                    (GUI_TEX_OVERLAY, font_info.as_ptr()),
                ] {
                    self.device.update_descriptor_sets(
                        &[
                            vk::WriteDescriptorSet {
                                dst_set: self.gui_descriptor_sets[tex * MAX_FRAMES + i],
                                dst_binding: 0,
                                descriptor_count: 1,
                                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                                p_buffer_info: buf_info.as_ptr(),
                                ..Default::default()
                            },
                            vk::WriteDescriptorSet {
                                dst_set: self.gui_descriptor_sets[tex * MAX_FRAMES + i],
                                dst_binding: 1,
                                descriptor_count: 1,
                                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                                p_image_info: image_info,
                                ..Default::default()
                            },
                        ],
                        &[],
                    );
                }
            }
        }
    }

    pub(in crate::render) fn refresh_gui_block_texture_descriptors(&self) {
        if self.gui_descriptor_sets.len() < GUI_TEXTURE_COUNT * MAX_FRAMES {
            return;
        }
        let image_info = [vk::DescriptorImageInfo {
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            image_view: self.texture_image_view,
            sampler: self.texture_sampler,
        }];
        for frame in 0..MAX_FRAMES {
            unsafe {
                self.device.update_descriptor_sets(
                    &[vk::WriteDescriptorSet {
                        dst_set: self.gui_descriptor_sets[GUI_TEX_BLOCKS * MAX_FRAMES + frame],
                        dst_binding: 1,
                        descriptor_count: 1,
                        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        p_image_info: image_info.as_ptr(),
                        ..Default::default()
                    }],
                    &[],
                );
            }
        }
    }

    fn create_gui_pipeline(
        device: &Device,
        render_pass: vk::RenderPass,
    ) -> (vk::Pipeline, vk::PipelineLayout, vk::DescriptorSetLayout) {
        // Descriptor layout: uniform + font tex
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: 1,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ];
        let descriptor_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo {
                    binding_count: 2,
                    p_bindings: bindings.as_ptr(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo {
                    set_layout_count: 1,
                    p_set_layouts: &descriptor_layout,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        let vs_spv = super::super::spirv_words(include_bytes!("../../shaders/gui.vert.spv"));
        let fs_spv = super::super::spirv_words(include_bytes!("../../shaders/gui.frag.spv"));
        let entry = CString::new("main").unwrap();
        let vs = unsafe {
            device.create_shader_module(
                &vk::ShaderModuleCreateInfo {
                    code_size: vs_spv.len() * std::mem::size_of::<u32>(),
                    p_code: vs_spv.as_ptr(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let fs = unsafe {
            device.create_shader_module(
                &vk::ShaderModuleCreateInfo {
                    code_size: fs_spv.len() * std::mem::size_of::<u32>(),
                    p_code: fs_spv.as_ptr(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let stages = [
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::VERTEX,
                module: vs,
                p_name: entry.as_ptr(),
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::FRAGMENT,
                module: fs,
                p_name: entry.as_ptr(),
                ..Default::default()
            },
        ];

        use super::GuiVertex;
        let vert_binding = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: GuiVertex::STRIDE,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vert_attrs = [
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32_SFLOAT,
                offset: 8,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 2,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: 16,
            },
        ];

        let blend = [vk::PipelineColorBlendAttachmentState {
            color_write_mask: vk::ColorComponentFlags::RGBA,
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            ..Default::default()
        }];

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let pipeline = unsafe {
            device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo {
                    stage_count: 2,
                    p_stages: stages.as_ptr(),
                    p_vertex_input_state: &vk::PipelineVertexInputStateCreateInfo {
                        vertex_binding_description_count: 1,
                        p_vertex_binding_descriptions: vert_binding.as_ptr(),
                        vertex_attribute_description_count: 3,
                        p_vertex_attribute_descriptions: vert_attrs.as_ptr(),
                        ..Default::default()
                    },
                    p_input_assembly_state: &vk::PipelineInputAssemblyStateCreateInfo {
                        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                        ..Default::default()
                    },
                    p_viewport_state: &vk::PipelineViewportStateCreateInfo {
                        viewport_count: 1,
                        scissor_count: 1,
                        ..Default::default()
                    },
                    p_rasterization_state: &vk::PipelineRasterizationStateCreateInfo {
                        polygon_mode: vk::PolygonMode::FILL,
                        line_width: 1.0,
                        ..Default::default()
                    },
                    p_multisample_state: &vk::PipelineMultisampleStateCreateInfo {
                        rasterization_samples: vk::SampleCountFlags::TYPE_1,
                        ..Default::default()
                    },
                    p_depth_stencil_state: &vk::PipelineDepthStencilStateCreateInfo {
                        depth_test_enable: vk::FALSE,
                        depth_write_enable: vk::FALSE,
                        ..Default::default()
                    },
                    p_color_blend_state: &vk::PipelineColorBlendStateCreateInfo {
                        attachment_count: 1,
                        p_attachments: blend.as_ptr(),
                        ..Default::default()
                    },
                    p_dynamic_state: &vk::PipelineDynamicStateCreateInfo {
                        dynamic_state_count: 2,
                        p_dynamic_states: dynamic_states.as_ptr(),
                        ..Default::default()
                    },
                    layout: pipeline_layout,
                    render_pass,
                    subpass: 0,
                    ..Default::default()
                }],
                None,
            )
        }
        .unwrap()[0];

        unsafe {
            device.destroy_shader_module(vs, None);
            device.destroy_shader_module(fs, None);
        }
        (pipeline, pipeline_layout, descriptor_layout)
    }

    fn create_panorama_pipeline(
        device: &Device,
        render_pass: vk::RenderPass,
    ) -> (vk::Pipeline, vk::PipelineLayout, vk::DescriptorSetLayout) {
        let descriptor_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo {
                    binding_count: 2,
                    p_bindings: [
                        vk::DescriptorSetLayoutBinding {
                            binding: 0,
                            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                            descriptor_count: 1,
                            stage_flags: vk::ShaderStageFlags::FRAGMENT,
                            ..Default::default()
                        },
                        vk::DescriptorSetLayoutBinding {
                            binding: 1,
                            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            descriptor_count: 1,
                            stage_flags: vk::ShaderStageFlags::FRAGMENT,
                            ..Default::default()
                        },
                    ]
                    .as_ptr(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo {
                    set_layout_count: 1,
                    p_set_layouts: &descriptor_layout,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        let vs_bytes = include_bytes!("../../shaders/panorama.vert.spv");
        let fs_bytes = include_bytes!("../../shaders/panorama.frag.spv");

        let mut vs_u32 = vec![0u32; vs_bytes.len() / 4];
        unsafe {
            std::ptr::copy_nonoverlapping(
                vs_bytes.as_ptr(),
                vs_u32.as_mut_ptr() as *mut u8,
                vs_bytes.len(),
            );
        }
        let mut fs_u32 = vec![0u32; fs_bytes.len() / 4];
        unsafe {
            std::ptr::copy_nonoverlapping(
                fs_bytes.as_ptr(),
                fs_u32.as_mut_ptr() as *mut u8,
                fs_bytes.len(),
            );
        }

        let vs = unsafe {
            device
                .create_shader_module(
                    &vk::ShaderModuleCreateInfo {
                        code_size: vs_bytes.len(),
                        p_code: vs_u32.as_ptr(),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap()
        };
        let fs = unsafe {
            device
                .create_shader_module(
                    &vk::ShaderModuleCreateInfo {
                        code_size: fs_bytes.len(),
                        p_code: fs_u32.as_ptr(),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap()
        };

        let entry_point = CString::new("main").unwrap();
        let stages = [
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::VERTEX,
                module: vs,
                p_name: entry_point.as_ptr(),
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::FRAGMENT,
                module: fs,
                p_name: entry_point.as_ptr(),
                ..Default::default()
            },
        ];

        let blend = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::FALSE,
            color_write_mask: vk::ColorComponentFlags::RGBA,
            ..Default::default()
        }];

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let pipeline = unsafe {
            device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo {
                    stage_count: 2,
                    p_stages: stages.as_ptr(),
                    p_vertex_input_state: &vk::PipelineVertexInputStateCreateInfo::default(),
                    p_input_assembly_state: &vk::PipelineInputAssemblyStateCreateInfo {
                        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                        ..Default::default()
                    },
                    p_viewport_state: &vk::PipelineViewportStateCreateInfo {
                        viewport_count: 1,
                        scissor_count: 1,
                        ..Default::default()
                    },
                    p_rasterization_state: &vk::PipelineRasterizationStateCreateInfo {
                        polygon_mode: vk::PolygonMode::FILL,
                        cull_mode: vk::CullModeFlags::NONE,
                        front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                        line_width: 1.0,
                        ..Default::default()
                    },
                    p_multisample_state: &vk::PipelineMultisampleStateCreateInfo {
                        rasterization_samples: vk::SampleCountFlags::TYPE_1,
                        ..Default::default()
                    },
                    p_depth_stencil_state: &vk::PipelineDepthStencilStateCreateInfo {
                        depth_test_enable: vk::FALSE,
                        depth_write_enable: vk::FALSE,
                        ..Default::default()
                    },
                    p_color_blend_state: &vk::PipelineColorBlendStateCreateInfo {
                        attachment_count: 1,
                        p_attachments: blend.as_ptr(),
                        ..Default::default()
                    },
                    p_dynamic_state: &vk::PipelineDynamicStateCreateInfo {
                        dynamic_state_count: 2,
                        p_dynamic_states: dynamic_states.as_ptr(),
                        ..Default::default()
                    },
                    layout: pipeline_layout,
                    render_pass,
                    subpass: 0,
                    ..Default::default()
                }],
                None,
            )
        }
        .unwrap()[0];

        unsafe {
            device.destroy_shader_module(vs, None);
            device.destroy_shader_module(fs, None);
        }
        (pipeline, pipeline_layout, descriptor_layout)
    }

    fn create_panorama_descriptors(
        device: &Device,
        layout: vk::DescriptorSetLayout,
    ) -> (vk::DescriptorPool, Vec<vk::DescriptorSet>) {
        let num_sets = crate::render::MAX_FRAMES as u32;
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: num_sets,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: num_sets,
            },
        ];
        let pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo {
                    pool_size_count: 2,
                    p_pool_sizes: pool_sizes.as_ptr(),
                    max_sets: num_sets,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let layouts = vec![layout; num_sets as usize];
        let sets = unsafe {
            device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo {
                descriptor_pool: pool,
                descriptor_set_count: num_sets,
                p_set_layouts: layouts.as_ptr(),
                ..Default::default()
            })
        }
        .unwrap();
        (pool, sets)
    }

    fn create_gui_descriptors(
        device: &Device,
        layout: vk::DescriptorSetLayout,
    ) -> (vk::DescriptorPool, Vec<vk::DescriptorSet>) {
        let num_sets = MAX_FRAMES as u32 * GUI_TEXTURE_COUNT as u32;
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: num_sets,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: num_sets,
            },
        ];
        let pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo {
                    pool_size_count: 2,
                    p_pool_sizes: pool_sizes.as_ptr(),
                    max_sets: num_sets,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let layouts = vec![layout; num_sets as usize];
        let sets = unsafe {
            device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo {
                descriptor_pool: pool,
                descriptor_set_count: num_sets,
                p_set_layouts: layouts.as_ptr(),
                ..Default::default()
            })
        }
        .unwrap();
        (pool, sets)
    }

    fn load_texture_file(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        path: &str,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
    ) -> (
        vk::Image,
        vk::ImageView,
        vk::Sampler,
        gpu_allocator::vulkan::Allocation,
    ) {
        let img = image::open(path)
            .expect(&format!("Failed to load {}", path))
            .to_rgba8();
        let (w, h) = img.dimensions();
        let pixels = img.into_raw();
        Self::upload_gui_texture(device, allocator, &pixels, w, h, command_pool, queue)
    }

    /// Build a grid atlas out of vanilla 256x256-canvas GUI textures.
    ///
    /// Resource packs ship these canvases at any power-of-two multiple of 256
    /// (512, 1024, ...). The cell size adapts to the highest-resolution source
    /// and every source is resized to exactly one cell, so callers can keep
    /// addressing the atlas with layout-normalized fractions
    /// (`(cell_index * 256 + src) / (256 * cols)`) regardless of the actual
    /// pixel resolution — nothing is ever truncated.
    fn build_gui_grid_atlas(
        resolver: &mut crate::assets::resolver::AssetResolver,
        resource_paths: &[&str],
        cols: u32,
        rows: u32,
    ) -> image::RgbaImage {
        assert!(
            resource_paths.len() as u32 <= cols * rows,
            "GUI grid atlas overflow: {} textures in a {cols}x{rows} grid",
            resource_paths.len()
        );
        let images: Vec<image::RgbaImage> = resource_paths
            .iter()
            .map(|resource_path| {
                let bytes = resolver
                    .read_bytes(resource_path)
                    .unwrap_or_else(|| panic!("Failed to load GUI texture: {resource_path}"));
                image::load_from_memory(&bytes)
                    .unwrap_or_else(|_| panic!("Failed to decode GUI texture: {resource_path}"))
                    .to_rgba8()
            })
            .collect();
        let scale = images
            .iter()
            .map(|img| img.width().max(img.height()).div_ceil(256).max(1))
            .max()
            .unwrap_or(1);
        let cell = 256 * scale;
        let mut atlas = image::RgbaImage::new(cell * cols, cell * rows);
        for (idx, img) in images.into_iter().enumerate() {
            let img = if img.width() != cell || img.height() != cell {
                image::imageops::resize(&img, cell, cell, image::imageops::FilterType::Nearest)
            } else {
                img
            };
            let x0 = idx as u32 % cols * cell;
            let y0 = idx as u32 / cols * cell;
            image::imageops::replace(&mut atlas, &img, x0 as i64, y0 as i64);
        }
        atlas
    }

    fn load_container_gui_atlas(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        resolver: &mut crate::assets::resolver::AssetResolver,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
    ) -> (
        vk::Image,
        vk::ImageView,
        vk::Sampler,
        gpu_allocator::vulkan::Allocation,
    ) {
        let atlas = Self::build_gui_grid_atlas(
            resolver,
            &[
                "minecraft/textures/gui/container/generic_54.png",
                "minecraft/textures/gui/container/crafting_table.png",
                "minecraft/textures/gui/container/furnace.png",
                "minecraft/textures/gui/container/dispenser.png",
                "minecraft/textures/gui/container/hopper.png",
                "minecraft/textures/gui/container/brewing_stand.png",
                "minecraft/textures/gui/container/enchanting_table.png",
                "minecraft/textures/gui/container/anvil.png",
                "minecraft/textures/gui/container/beacon.png",
                "minecraft/textures/gui/container/villager.png",
                "minecraft/textures/gui/container/horse.png",
            ],
            4,
            3,
        );
        let (w, h) = atlas.dimensions();
        let pixels = atlas.into_raw();
        let (image, view, nearest_sampler, allocation) =
            Self::upload_gui_texture(device, allocator, &pixels, w, h, command_pool, queue);
        unsafe {
            device.destroy_sampler(nearest_sampler, None);
        }
        let sampler = unsafe {
            device.create_sampler(
                &vk::SamplerCreateInfo {
                    mag_filter: vk::Filter::LINEAR,
                    min_filter: vk::Filter::LINEAR,
                    address_mode_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                    address_mode_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        (image, view, sampler, allocation)
    }

    fn load_creative_gui_atlas(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        resolver: &mut crate::assets::resolver::AssetResolver,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
    ) -> (
        vk::Image,
        vk::ImageView,
        vk::Sampler,
        gpu_allocator::vulkan::Allocation,
    ) {
        let atlas = Self::build_gui_grid_atlas(
            resolver,
            &[
                "minecraft/textures/gui/container/creative_inventory/tabs.png",
                "minecraft/textures/gui/container/creative_inventory/tab_items.png",
                "minecraft/textures/gui/container/creative_inventory/tab_inventory.png",
                "minecraft/textures/gui/container/creative_inventory/tab_item_search.png",
            ],
            2,
            2,
        );
        let (w, h) = atlas.dimensions();
        let pixels = atlas.into_raw();
        Self::upload_gui_texture(device, allocator, &pixels, w, h, command_pool, queue)
    }

    fn load_panorama_atlas(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        resolver: &mut crate::assets::resolver::AssetResolver,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
    ) -> (
        vk::Image,
        vk::ImageView,
        vk::Sampler,
        gpu_allocator::vulkan::Allocation,
    ) {
        let mut atlas = image::RgbaImage::new(256 * 6, 256);
        for i in 0..6 {
            let path = format!("minecraft/textures/gui/title/background/panorama_{}.png", i);
            if let Some(bytes) = resolver.read_bytes(&path) {
                if let Ok(img) = image::load_from_memory(&bytes) {
                    let img = img.to_rgba8();
                    let copy_w = img.width().min(256);
                    let copy_h = img.height().min(256);
                    for y in 0..copy_h {
                        for x in 0..copy_w {
                            atlas.put_pixel(i * 256 + x, y, *img.get_pixel(x, y));
                        }
                    }
                }
            }
        }
        let pixels = atlas.into_raw();
        let (image, view, nearest_sampler, allocation) = Self::upload_gui_texture(
            device,
            allocator,
            &pixels,
            256 * 6,
            256,
            command_pool,
            queue,
        );
        unsafe {
            device.destroy_sampler(nearest_sampler, None);
        }
        let sampler = unsafe {
            device.create_sampler(
                &vk::SamplerCreateInfo {
                    mag_filter: vk::Filter::LINEAR,
                    min_filter: vk::Filter::LINEAR,
                    address_mode_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                    address_mode_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        (image, view, sampler, allocation)
    }

    fn load_item_icon_atlas_from_resolver(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        resolver: &mut crate::assets::resolver::AssetResolver,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
    ) -> (
        vk::Image,
        vk::ImageView,
        vk::Sampler,
        gpu_allocator::vulkan::Allocation,
    ) {
        use crate::render::item_icons::ITEM_ATLAS_H;
        use crate::render::item_icons::ITEM_ATLAS_W;
        let pixels = crate::render::item_icons::build_item_icon_atlas(resolver);
        Self::upload_gui_texture(
            device,
            allocator,
            &pixels,
            ITEM_ATLAS_W,
            ITEM_ATLAS_H,
            command_pool,
            queue,
        )
    }

    fn load_gui_texture_from_resolver(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        resolver: &mut crate::assets::resolver::AssetResolver,
        resource_path: &str,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
    ) -> (
        vk::Image,
        vk::ImageView,
        vk::Sampler,
        gpu_allocator::vulkan::Allocation,
        (u32, u32),
    ) {
        let bytes = resolver
            .read_bytes(resource_path)
            .unwrap_or_else(|| panic!("Failed to load GUI texture: {resource_path}"));
        let img = image::load_from_memory(&bytes)
            .unwrap_or_else(|_| panic!("Failed to decode GUI texture: {resource_path}"))
            .to_rgba8();
        let (w, h) = img.dimensions();
        let pixels = img.into_raw();
        let (image, view, sampler, allocation) =
            Self::upload_gui_texture(device, allocator, &pixels, w, h, command_pool, queue);
        (image, view, sampler, allocation, (w, h))
    }

    fn destroy_gui_texture_objects(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        image: vk::Image,
        view: vk::ImageView,
        sampler: vk::Sampler,
        alloc: gpu_allocator::vulkan::Allocation,
    ) {
        if image == vk::Image::null() {
            return;
        }
        unsafe {
            device.destroy_image_view(view, None);
            device.destroy_sampler(sampler, None);
            device.destroy_image(image, None);
        }
        allocator.free(alloc).ok();
    }

    fn replace_gui_texture_slot(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        image: &mut vk::Image,
        view: &mut vk::ImageView,
        sampler: &mut vk::Sampler,
        alloc: &mut gpu_allocator::vulkan::Allocation,
        new: (
            vk::Image,
            vk::ImageView,
            vk::Sampler,
            gpu_allocator::vulkan::Allocation,
        ),
    ) {
        Self::destroy_gui_texture_objects(
            device,
            allocator,
            *image,
            *view,
            *sampler,
            std::mem::take(alloc),
        );
        *image = new.0;
        *view = new.1;
        *sampler = new.2;
        *alloc = new.3;
    }

    /// Rebuild every resource-pack-dependent GUI texture at its actual
    /// resolution and rebind the descriptor sets.
    ///
    /// Images are destroyed and recreated instead of updated in place: pack
    /// resolutions differ, and writing new pixels into an image with a stale
    /// extent corrupts sampling (all draw code addresses these textures with
    /// layout-normalized UVs, so only the extent must match the pixel data).
    ///
    /// The caller must ensure the GPU is idle (schedule_resource_reload does
    /// `device_wait_idle`).
    pub(in crate::render) fn reload_gui_textures(
        &mut self,
        resolver: &mut crate::assets::resolver::AssetResolver,
    ) {
        if self.gui_pipeline == vk::Pipeline::null() {
            return;
        }
        let single_textures: [(
            &str,
            &mut vk::Image,
            &mut vk::ImageView,
            &mut vk::Sampler,
            &mut gpu_allocator::vulkan::Allocation,
        ); 5] = [
            (
                "minecraft/textures/gui/widgets.png",
                &mut self.gui_widget_image,
                &mut self.gui_widget_view,
                &mut self.gui_widget_sampler,
                &mut self.gui_widget_alloc,
            ),
            (
                "minecraft/textures/gui/container/inventory.png",
                &mut self.gui_inventory_image,
                &mut self.gui_inventory_view,
                &mut self.gui_inventory_sampler,
                &mut self.gui_inventory_alloc,
            ),
            (
                "minecraft/textures/gui/icons.png",
                &mut self.gui_icons_image,
                &mut self.gui_icons_view,
                &mut self.gui_icons_sampler,
                &mut self.gui_icons_alloc,
            ),
            (
                "minecraft/textures/gui/options_background.png",
                &mut self.gui_options_bg_image,
                &mut self.gui_options_bg_view,
                &mut self.gui_options_bg_sampler,
                &mut self.gui_options_bg_alloc,
            ),
            (
                "minecraft/textures/misc/underwater.png",
                &mut self.gui_underwater_image,
                &mut self.gui_underwater_view,
                &mut self.gui_underwater_sampler,
                &mut self.gui_underwater_alloc,
            ),
        ];
        for (path, image, view, sampler, alloc) in single_textures {
            let new = Self::load_gui_texture_from_resolver(
                &self.device,
                &mut self.allocator,
                resolver,
                path,
                self.command_pool,
                self.queue,
            );
            let (new_img, new_view, new_samp, new_alloc, _tex_size) = new;
            Self::replace_gui_texture_slot(
                &self.device,
                &mut self.allocator,
                image,
                view,
                sampler,
                alloc,
                (new_img, new_view, new_samp, new_alloc),
            );
        }

        let new = Self::load_container_gui_atlas(
            &self.device,
            &mut self.allocator,
            resolver,
            self.command_pool,
            self.queue,
        );
        Self::replace_gui_texture_slot(
            &self.device,
            &mut self.allocator,
            &mut self.gui_generic54_image,
            &mut self.gui_generic54_view,
            &mut self.gui_generic54_sampler,
            &mut self.gui_generic54_alloc,
            new,
        );

        let new = Self::load_creative_gui_atlas(
            &self.device,
            &mut self.allocator,
            resolver,
            self.command_pool,
            self.queue,
        );
        Self::replace_gui_texture_slot(
            &self.device,
            &mut self.allocator,
            &mut self.gui_creative_image,
            &mut self.gui_creative_view,
            &mut self.gui_creative_sampler,
            &mut self.gui_creative_alloc,
            new,
        );

        // The old image views are gone; every descriptor set must be rebound.
        self.write_gui_texture_descriptors();
        log::info!("GUI textures reloaded from resource pack");
    }

    fn create_empty_texture(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        w: u32,
        h: u32,
    ) -> (
        vk::Image,
        vk::ImageView,
        vk::Sampler,
        gpu_allocator::vulkan::Allocation,
    ) {
        let image = unsafe {
            device.create_image(
                &vk::ImageCreateInfo {
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
                },
                None,
            )
        }
        .unwrap();
        let reqs = unsafe { device.get_image_memory_requirements(image) };
        let alloc = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "font_atlas",
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
                    address_mode_u: vk::SamplerAddressMode::REPEAT,
                    address_mode_v: vk::SamplerAddressMode::REPEAT,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        (image, view, sampler, alloc)
    }

    fn upload_gui_texture(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        pixels: &[u8],
        w: u32,
        h: u32,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
    ) -> (
        vk::Image,
        vk::ImageView,
        vk::Sampler,
        gpu_allocator::vulkan::Allocation,
    ) {
        let size = (w * h * 4) as u64;
        let image = unsafe {
            device.create_image(
                &vk::ImageCreateInfo {
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
                },
                None,
            )
        }
        .unwrap();
        let reqs = unsafe { device.get_image_memory_requirements(image) };
        let alloc = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "gui_tex",
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

        // Staging buffer
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
        let s_reqs = unsafe { device.get_buffer_memory_requirements(staging) };
        let s_alloc = allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "stg",
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

        // One-shot copy
        let cb = unsafe {
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
                    cb,
                    &vk::CommandBufferBeginInfo {
                        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                        ..Default::default()
                    },
                )
                .unwrap();
            device.cmd_pipeline_barrier(
                cb,
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
        }
        unsafe {
            device.destroy_buffer(staging, None);
        }
        allocator.free(s_alloc).unwrap();

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
                    address_mode_u: vk::SamplerAddressMode::REPEAT,
                    address_mode_v: vk::SamplerAddressMode::REPEAT,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        (image, view, sampler, alloc)
    }

    fn create_uniform_buf(
        device: &Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        size: usize,
    ) -> (vk::Buffer, gpu_allocator::vulkan::Allocation) {
        let buf = unsafe {
            device.create_buffer(
                &vk::BufferCreateInfo {
                    size: size as u64,
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
                name: "gui_ub",
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
        (buf, alloc)
    }

    // --- Draw GUI overlay (called within render pass) ---

    pub(crate) fn update_gui_uniforms(&mut self) {
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        if sw == self.cached_gui_vp_w && sh == self.cached_gui_vp_h {
            return;
        }
        self.cached_gui_vp_w = sw;
        self.cached_gui_vp_h = sh;
        let alloc = &self.gui_uniform_alloc;
        let udata = [sw, sh];
        unsafe {
            let ptr = self
                .device
                .map_memory(
                    alloc.memory(),
                    alloc.offset(),
                    std::mem::size_of::<super::GuiUniforms>() as u64,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            std::ptr::copy_nonoverlapping(udata.as_ptr() as *const u8, ptr as *mut u8, 8);
            self.device.unmap_memory(alloc.memory());
        }
    }

    fn upload_gui_buffers(
        &mut self,
        frame: usize,
        ds_offset: usize,
        content_generation: u64,
        vertex_bytes: &[u8],
        index_bytes: &[u8],
    ) -> (vk::Buffer, vk::Buffer) {
        let layer = ds_offset / MAX_FRAMES;
        debug_assert!(layer < GUI_TEXTURE_COUNT);
        let slot_index = frame * GUI_TEXTURE_COUNT + layer;
        if self.gui_buffers[slot_index].content_generation == Some(content_generation) {
            return (
                self.gui_buffers[slot_index]
                    .vertex_buffer
                    .expect("cached GUI vertex buffer must exist"),
                self.gui_buffers[slot_index]
                    .index_buffer
                    .expect("cached GUI index buffer must exist"),
            );
        }
        let vertex_size = vertex_bytes.len() as u64;
        let index_size = index_bytes.len() as u64;
        let hash_bytes = |bytes: &[u8]| {
            use std::hash::Hasher;
            let mut hasher = fnv::FnvHasher::default();
            hasher.write_usize(bytes.len());
            hasher.write(bytes);
            hasher.finish()
        };
        let vertex_hash = hash_bytes(vertex_bytes);
        let index_hash = hash_bytes(index_bytes);

        if self.gui_buffers[slot_index].vertex_capacity < vertex_size {
            let (buffer, alloc) = self.create_device_buffer(
                vertex_size,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vertex_bytes,
            );
            let old_buffer = self.gui_buffers[slot_index].vertex_buffer.replace(buffer);
            let old_alloc = self.gui_buffers[slot_index].vertex_alloc.replace(alloc);
            self.gui_buffers[slot_index].vertex_capacity = vertex_size;
            self.gui_buffers[slot_index].vertex_hash = vertex_hash;
            unsafe {
                if let Some(buffer) = old_buffer {
                    self.device.destroy_buffer(buffer, None);
                }
            }
            if let Some(alloc) = old_alloc {
                self.allocator.free(alloc).ok();
            }
        } else if vertex_size > 0 && self.gui_buffers[slot_index].vertex_hash != vertex_hash {
            let alloc = self.gui_buffers[slot_index]
                .vertex_alloc
                .as_ref()
                .expect("GUI vertex buffer allocation must exist");
            unsafe {
                let ptr = self
                    .device
                    .map_memory(
                        alloc.memory(),
                        alloc.offset(),
                        vertex_size,
                        vk::MemoryMapFlags::empty(),
                    )
                    .unwrap();
                std::ptr::copy_nonoverlapping(
                    vertex_bytes.as_ptr(),
                    ptr as *mut u8,
                    vertex_bytes.len(),
                );
                self.device.unmap_memory(alloc.memory());
            }
            self.gui_buffers[slot_index].vertex_hash = vertex_hash;
        }

        if self.gui_buffers[slot_index].index_capacity < index_size {
            let (buffer, alloc) = self.create_device_buffer(
                index_size,
                vk::BufferUsageFlags::INDEX_BUFFER,
                index_bytes,
            );
            let old_buffer = self.gui_buffers[slot_index].index_buffer.replace(buffer);
            let old_alloc = self.gui_buffers[slot_index].index_alloc.replace(alloc);
            self.gui_buffers[slot_index].index_capacity = index_size;
            self.gui_buffers[slot_index].index_hash = index_hash;
            unsafe {
                if let Some(buffer) = old_buffer {
                    self.device.destroy_buffer(buffer, None);
                }
            }
            if let Some(alloc) = old_alloc {
                self.allocator.free(alloc).ok();
            }
        } else if index_size > 0 && self.gui_buffers[slot_index].index_hash != index_hash {
            let alloc = self.gui_buffers[slot_index]
                .index_alloc
                .as_ref()
                .expect("GUI index buffer allocation must exist");
            unsafe {
                let ptr = self
                    .device
                    .map_memory(
                        alloc.memory(),
                        alloc.offset(),
                        index_size,
                        vk::MemoryMapFlags::empty(),
                    )
                    .unwrap();
                std::ptr::copy_nonoverlapping(
                    index_bytes.as_ptr(),
                    ptr as *mut u8,
                    index_bytes.len(),
                );
                self.device.unmap_memory(alloc.memory());
            }
            self.gui_buffers[slot_index].index_hash = index_hash;
        }

        self.gui_buffers[slot_index].content_generation = Some(content_generation);
        (
            self.gui_buffers[slot_index]
                .vertex_buffer
                .expect("GUI vertex buffer must exist after upload"),
            self.gui_buffers[slot_index]
                .index_buffer
                .expect("GUI index buffer must exist after upload"),
        )
    }

    /// Draw GUI with specified descriptor set. `ds_offset`: 0 = font, MAX_FRAMES = widget.
    pub fn draw_gui_ex(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
        ds_offset: usize,
    ) {
        if builder.vertices.is_empty() || self.gui_pipeline == vk::Pipeline::null() {
            return;
        }

        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let (vertex_buffer, index_buffer) = self.upload_gui_buffers(
            frame,
            ds_offset,
            builder.content_generation(),
            bytemuck::cast_slice(&builder.vertices),
            bytemuck::cast_slice(&builder.indices),
        );

        // Bind GUI pipeline and draw
        unsafe {
            self.device
                .cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.gui_pipeline);
            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: sw,
                height: sh,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            self.device.cmd_set_viewport(cmd, 0, &[viewport]);
            self.device.cmd_set_scissor(
                cmd,
                0,
                &[vk::Rect2D {
                    extent: self.swapchain_extent,
                    ..Default::default()
                }],
            );
            self.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.gui_pipeline_layout,
                0,
                &[self.gui_descriptor_sets[ds_offset + frame]],
                &[],
            );

            self.device
                .cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer], &[0]);
            self.device
                .cmd_bind_index_buffer(cmd, index_buffer, 0, vk::IndexType::UINT32);

            let index_count = builder.indices.len() as u32;
            if index_count > 0 {
                self.device.cmd_draw_indexed(cmd, index_count, 1, 0, 0, 0);
            }
        }
    }

    /// Draw GUI with font texture (convenience).
    pub fn draw_gui(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_FONT * MAX_FRAMES);
    }

    /// Draw the untextured full-screen overlay batch (own buffer slot).
    pub fn draw_gui_overlay(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_OVERLAY * MAX_FRAMES);
    }

    /// Draw GUI with widget texture (convenience).
    pub fn draw_gui_widget(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_WIDGETS * MAX_FRAMES);
    }

    /// Draw GUI with the world block atlas (for inventory item previews).
    pub fn draw_gui_blocks(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_BLOCKS * MAX_FRAMES);
    }

    pub fn draw_gui_inventory(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_INVENTORY * MAX_FRAMES);
    }

    pub fn draw_gui_generic54(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_GENERIC54 * MAX_FRAMES);
    }

    pub fn draw_gui_items(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_ITEMS * MAX_FRAMES);
    }

    /// Draw GUI with icons texture (hearts, hunger, armor, XP bar).
    pub fn draw_gui_icons(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_ICONS * MAX_FRAMES);
    }

    /// Draw GUI with creative inventory atlas (tabs + panel backgrounds).
    pub fn draw_gui_creative(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_CREATIVE * MAX_FRAMES);
    }

    pub fn draw_gui_options_background(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
    ) {
        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_OPTIONS_BG * MAX_FRAMES);
    }

    /// Draw the underwater overlay texture (full-screen quad with UV scroll).
    pub fn draw_gui_underwater(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        builder: &mut super::GuiVertexBuilder,
        yaw: f32,
        pitch: f32,
    ) {
        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;

        // Vanilla MC 1.8.9: underwater.png tiled 4x4, UV offset by yaw/pitch
        let uv_offset_x = -yaw / 64.0;
        let uv_offset_y = pitch / 64.0;

        // Brightness tint (vanilla uses entity brightness * 0.5 alpha)
        let brightness = 0.6;
        let alpha = 0.5;

        // Full-screen quad with tiled UV (0..4)
        builder.add_quad(
            0.0,
            0.0,
            sw,
            sh,
            uv_offset_x,
            uv_offset_y,
            4.0,
            4.0,
            [brightness, brightness, brightness, alpha],
        );

        self.draw_gui_ex(cmd, frame, builder, GUI_TEX_UNDERWATER * MAX_FRAMES);
    }

    /// Upload font atlas to GPU if dirty (new glyphs were added).
    pub(crate) fn upload_font_atlas(&mut self) {
        if !self.font.atlas_dirty {
            return;
        }
        let w = self.font.atlas_width;
        let h = self.font.atlas_height;
        let size = (w * h * 4) as u64;
        let staging = unsafe {
            self.device.create_buffer(
                &vk::BufferCreateInfo {
                    size,
                    usage: vk::BufferUsageFlags::TRANSFER_SRC,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let s_reqs = unsafe { self.device.get_buffer_memory_requirements(staging) };
        let s_alloc = self
            .allocator
            .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "font_stg",
                requirements: s_reqs,
                location: gpu_allocator::MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .unwrap();
        unsafe {
            self.device
                .bind_buffer_memory(staging, s_alloc.memory(), s_alloc.offset())
                .unwrap();
            let ptr = self
                .device
                .map_memory(
                    s_alloc.memory(),
                    s_alloc.offset(),
                    size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            std::ptr::copy_nonoverlapping(
                self.font.atlas_pixels.as_ptr(),
                ptr as *mut u8,
                self.font.atlas_pixels.len(),
            );
            self.device.unmap_memory(s_alloc.memory());
        }
        let cb = unsafe {
            self.device
                .allocate_command_buffers(&vk::CommandBufferAllocateInfo {
                    command_pool: self.command_pool,
                    level: vk::CommandBufferLevel::PRIMARY,
                    command_buffer_count: 1,
                    ..Default::default()
                })
        }
        .unwrap()[0];
        unsafe {
            self.device
                .begin_command_buffer(
                    cb,
                    &vk::CommandBufferBeginInfo {
                        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                        ..Default::default()
                    },
                )
                .unwrap();
            self.device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    old_layout: if self.gui_font_uploaded {
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
                    } else {
                        vk::ImageLayout::UNDEFINED
                    },
                    new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    image: self.gui_font_image,
                    subresource_range: color_subresource(),
                    ..Default::default()
                }],
            );
            self.device.cmd_copy_buffer_to_image(
                cb,
                staging,
                self.gui_font_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy {
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        layer_count: 1,
                        ..Default::default()
                    },
                    image_extent: vk::Extent3D {
                        width: w,
                        height: h,
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
                    image: self.gui_font_image,
                    subresource_range: color_subresource(),
                    ..Default::default()
                }],
            );
            self.device.end_command_buffer(cb).unwrap();
            let fence = self
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .unwrap();
            self.device
                .queue_submit(
                    self.queue,
                    &[vk::SubmitInfo {
                        command_buffer_count: 1,
                        p_command_buffers: &cb,
                        ..Default::default()
                    }],
                    fence,
                )
                .unwrap();
            self.device
                .wait_for_fences(&[fence], true, u64::MAX)
                .unwrap();
            self.device.destroy_fence(fence, None);
            self.device.free_command_buffers(self.command_pool, &[cb]);
        }
        unsafe {
            self.device.destroy_buffer(staging, None);
        }
        self.allocator.free(s_alloc).unwrap();
        self.font.atlas_dirty = false;
        self.gui_font_uploaded = true;
    }

    /// Test if a screen-space point hits any button from the last frame.
    pub fn gui_hit_test(&self, mx: f32, my: f32) -> Option<u32> {
        for btn in self.last_button_hits.iter().rev() {
            if mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h {
                return Some(btn.id);
            }
        }
        None
    }

    pub fn gui_hit_rect(&self, id: u32) -> Option<&super::ButtonHit> {
        self.last_button_hits.iter().rev().find(|hit| hit.id == id)
    }

    /// Return the nearest interactive rectangle when the point is close enough
    /// to it.  Controller virtual cursors use this to settle on the centre of
    /// menu buttons and inventory slots rather than requiring pixel-perfect
    /// analog-stick movement.
    pub fn gui_nearest_hit(&self, mx: f32, my: f32, max_distance: f32) -> Option<super::ButtonHit> {
        self.last_button_hits
            .iter()
            .copied()
            .filter_map(|hit| {
                let nearest_x = mx.clamp(hit.x, hit.x + hit.w);
                let nearest_y = my.clamp(hit.y, hit.y + hit.h);
                let dx = mx - nearest_x;
                let dy = my - nearest_y;
                let distance_squared = dx * dx + dy * dy;
                (distance_squared <= max_distance * max_distance).then_some((distance_squared, hit))
            })
            .min_by(|(left, _), (right, _)| left.total_cmp(right))
            .map(|(_, hit)| hit)
    }

    /// Find the next interactive rectangle in a screen-space direction.  This
    /// is intentionally based on rectangle centres so keyboard/controller
    /// navigation works for both menus and inventory slots.
    pub fn gui_directional_hit(
        &self,
        mx: f32,
        my: f32,
        direction: [f32; 2],
    ) -> Option<super::ButtonHit> {
        let [dx, dy] = direction;
        self.last_button_hits
            .iter()
            .copied()
            .filter_map(|hit| {
                let cx = hit.x + hit.w * 0.5;
                let cy = hit.y + hit.h * 0.5;
                let offset_x = cx - mx;
                let offset_y = cy - my;
                let forward = offset_x * dx + offset_y * dy;
                if forward <= 0.5 {
                    return None;
                }
                let sideways = (offset_x * dy - offset_y * dx).abs();
                // Prefer the closest control in the requested direction, while
                // keeping a neighbouring row/column ahead of a diagonal one.
                Some((forward + sideways * 0.35, hit))
            })
            .min_by(|(left, _), (right, _)| left.total_cmp(right))
            .map(|(_, hit)| hit)
    }

    pub fn set_gui_mouse_pos(&mut self, x: f32, y: f32) {
        self.gui_mouse_pos = [x, y];
    }

    pub fn set_particles(
        &mut self,
        particles: &[crate::client::particles::Particle],
        generation: u64,
    ) {
        if self.particle_generation == generation && self.particle_list.len() == particles.len() {
            return;
        }
        self.particle_generation = generation;
        self.particle_list.clear();
        self.particle_list.extend_from_slice(particles);
        self.state.particle_count = particles.len();
        // Old 2D sprite path — clear the sprite vec since we now render in 3D.
        self.particles.clear();
    }
}
