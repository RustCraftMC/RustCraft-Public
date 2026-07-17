//! Render pass, graphics pipeline, descriptor set layout/pool/sets.

use ash::vk;
use std::ffi::CString;

use super::{Uniforms, MAX_FRAMES};
use crate::world::mesh::Vertex;

impl super::Renderer {
    // --- Render pass ---

    pub(super) fn create_render_pass(
        device: &ash::Device,
        color_format: vk::Format,
        depth_format: vk::Format,
    ) -> vk::RenderPass {
        let attachments = [
            vk::AttachmentDescription {
                format: color_format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            vk::AttachmentDescription {
                format: depth_format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::DONT_CARE,
                stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
        ];
        let color_ref = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        let depth_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };
        let subpasses = [vk::SubpassDescription {
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachment_count: 1,
            p_color_attachments: color_ref.as_ptr(),
            p_depth_stencil_attachment: &depth_ref,
            ..Default::default()
        }];
        let deps = [vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            ..Default::default()
        }];
        unsafe {
            device.create_render_pass(
                &vk::RenderPassCreateInfo {
                    attachment_count: 2,
                    p_attachments: attachments.as_ptr(),
                    subpass_count: 1,
                    p_subpasses: subpasses.as_ptr(),
                    dependency_count: 1,
                    p_dependencies: deps.as_ptr(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap()
    }

    // --- Pipeline ---

    pub(super) fn create_pipeline(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        depth_write: bool,
        alpha_blend: bool,
        cull_mode: vk::CullModeFlags,
        shaders: &super::shader_pack::ShaderPackShaders,
    ) -> (vk::Pipeline, vk::PipelineLayout, vk::DescriptorSetLayout) {
        // Descriptor layout
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
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

        // Pipeline layout with push constants
        let pc_range = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: 12, // vec3 chunk offset
        };
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo {
                    set_layout_count: 1,
                    p_set_layouts: &descriptor_layout,
                    push_constant_range_count: 1,
                    p_push_constant_ranges: &pc_range,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        // Shaders
        let vs_spv = shaders.stage(
            super::shader_pack::ShaderStage::WorldVertex,
            include_bytes!("../shaders/basic.vert.spv"),
        );
        let fs_spv = shaders.stage(
            super::shader_pack::ShaderStage::WorldFragment,
            include_bytes!("../shaders/basic.frag.spv"),
        );
        let vs_module = unsafe {
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
        let fs_module = unsafe {
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

        let entry_name = CString::new("main").unwrap();
        let stages = [
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::VERTEX,
                module: vs_module,
                p_name: entry_name.as_ptr(),
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::FRAGMENT,
                module: fs_module,
                p_name: entry_name.as_ptr(),
                ..Default::default()
            },
        ];

        // Vertex input: block geometry, tint selector, vanilla lightmap channels, and AO.
        let vert_bindings = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: Vertex::STRIDE,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vert_attrs = [
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 12,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 2,
                format: vk::Format::R32G32_SFLOAT,
                offset: 24,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 3,
                format: vk::Format::R32_SFLOAT,
                offset: 32,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 4,
                format: vk::Format::R32_SFLOAT,
                offset: 36,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 5,
                format: vk::Format::R32_SFLOAT,
                offset: 40,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 6,
                format: vk::Format::R32_SFLOAT,
                offset: 44,
            },
        ];

        let viewports = [vk::Viewport {
            width: 1.0,
            height: 1.0,
            max_depth: 1.0,
            ..Default::default()
        }];
        let scissors = [vk::Rect2D::default()];
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let color_blend_att = [vk::PipelineColorBlendAttachmentState {
            blend_enable: if alpha_blend { vk::TRUE } else { vk::FALSE },
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::RGBA,
        }];

        let info = vk::GraphicsPipelineCreateInfo {
            stage_count: 2,
            p_stages: stages.as_ptr(),
            p_vertex_input_state: &vk::PipelineVertexInputStateCreateInfo {
                vertex_binding_description_count: 1,
                p_vertex_binding_descriptions: vert_bindings.as_ptr(),
                vertex_attribute_description_count: vert_attrs.len() as u32,
                p_vertex_attribute_descriptions: vert_attrs.as_ptr(),
                ..Default::default()
            },
            p_input_assembly_state: &vk::PipelineInputAssemblyStateCreateInfo {
                topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                ..Default::default()
            },
            p_viewport_state: &vk::PipelineViewportStateCreateInfo {
                viewport_count: 1,
                p_viewports: viewports.as_ptr(),
                scissor_count: 1,
                p_scissors: scissors.as_ptr(),
                ..Default::default()
            },
            p_rasterization_state: &vk::PipelineRasterizationStateCreateInfo {
                polygon_mode: vk::PolygonMode::FILL,
                cull_mode,
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                ..Default::default()
            },
            p_multisample_state: &vk::PipelineMultisampleStateCreateInfo {
                rasterization_samples: vk::SampleCountFlags::TYPE_1,
                ..Default::default()
            },
            p_depth_stencil_state: &vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: vk::TRUE,
                depth_write_enable: if depth_write { vk::TRUE } else { vk::FALSE },
                depth_compare_op: vk::CompareOp::LESS,
                ..Default::default()
            },
            p_color_blend_state: &vk::PipelineColorBlendStateCreateInfo {
                attachment_count: 1,
                p_attachments: color_blend_att.as_ptr(),
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
        };

        let pipeline =
            unsafe { device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None) }
                .unwrap()[0];

        unsafe {
            device.destroy_shader_module(vs_module, None);
            device.destroy_shader_module(fs_module, None);
        }

        (pipeline, pipeline_layout, descriptor_layout)
    }

    // --- Entity pipeline ---

    pub(super) fn create_entity_pipeline(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        descriptor_layout: vk::DescriptorSetLayout,
        depth_write: bool,
        shaders: &super::shader_pack::ShaderPackShaders,
    ) -> (vk::Pipeline, vk::PipelineLayout) {
        // Reuse the same descriptor layout (uniforms + texture sampler)

        // Pipeline layout — no push constants for entities (world-space baked into vertices)
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

        // Entity shaders
        let vs_spv = shaders.stage(
            super::shader_pack::ShaderStage::EntityVertex,
            include_bytes!("../shaders/entity.vert.spv"),
        );
        let fs_spv = shaders.stage(
            super::shader_pack::ShaderStage::EntityFragment,
            include_bytes!("../shaders/entity.frag.spv"),
        );
        let vs_module = unsafe {
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
        let fs_module = unsafe {
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

        let entry_name = CString::new("main").unwrap();
        let stages = [
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::VERTEX,
                module: vs_module,
                p_name: entry_name.as_ptr(),
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::FRAGMENT,
                module: fs_module,
                p_name: entry_name.as_ptr(),
                ..Default::default()
            },
        ];

        // EntityVertex: pos(3f) + normal(3f) + uv(2f) + color(4f) = 48 bytes
        use super::entity::mesh::EntityVertex;
        let vert_binding = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<EntityVertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vert_attrs = [
            vk::VertexInputAttributeDescription {
                // position: vec3
                binding: 0,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                // normal: vec3
                binding: 0,
                location: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 12,
            },
            vk::VertexInputAttributeDescription {
                // uv: vec2
                binding: 0,
                location: 2,
                format: vk::Format::R32G32_SFLOAT,
                offset: 24,
            },
            vk::VertexInputAttributeDescription {
                // color: vec4
                binding: 0,
                location: 3,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: 32,
            },
        ];

        let viewports = [vk::Viewport {
            width: 1.0,
            height: 1.0,
            max_depth: 1.0,
            ..Default::default()
        }];
        let scissors = [vk::Rect2D::default()];
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let color_blend_att = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::RGBA,
        }];

        let info = vk::GraphicsPipelineCreateInfo {
            stage_count: 2,
            p_stages: stages.as_ptr(),
            p_vertex_input_state: &vk::PipelineVertexInputStateCreateInfo {
                vertex_binding_description_count: 1,
                p_vertex_binding_descriptions: vert_binding.as_ptr(),
                vertex_attribute_description_count: 4,
                p_vertex_attribute_descriptions: vert_attrs.as_ptr(),
                ..Default::default()
            },
            p_input_assembly_state: &vk::PipelineInputAssemblyStateCreateInfo {
                topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                ..Default::default()
            },
            p_viewport_state: &vk::PipelineViewportStateCreateInfo {
                viewport_count: 1,
                p_viewports: viewports.as_ptr(),
                scissor_count: 1,
                p_scissors: scissors.as_ptr(),
                ..Default::default()
            },
            p_rasterization_state: &vk::PipelineRasterizationStateCreateInfo {
                polygon_mode: vk::PolygonMode::FILL,
                cull_mode: vk::CullModeFlags::NONE, // disabled for wireframe visibility
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                ..Default::default()
            },
            p_multisample_state: &vk::PipelineMultisampleStateCreateInfo {
                rasterization_samples: vk::SampleCountFlags::TYPE_1,
                ..Default::default()
            },
            p_depth_stencil_state: &vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: vk::TRUE,
                depth_write_enable: if depth_write { vk::TRUE } else { vk::FALSE },
                depth_compare_op: vk::CompareOp::LESS,
                ..Default::default()
            },
            p_color_blend_state: &vk::PipelineColorBlendStateCreateInfo {
                attachment_count: 1,
                p_attachments: color_blend_att.as_ptr(),
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
        };

        let pipeline =
            unsafe { device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None) }
                .unwrap()[0];

        unsafe {
            device.destroy_shader_module(vs_module, None);
            device.destroy_shader_module(fs_module, None);
        }

        (pipeline, pipeline_layout)
    }

    pub(super) fn create_nametag_pipeline(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        descriptor_layout: vk::DescriptorSetLayout,
        shaders: &super::shader_pack::ShaderPackShaders,
    ) -> (vk::Pipeline, vk::PipelineLayout) {
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

        let vs_spv = shaders.stage(
            super::shader_pack::ShaderStage::EntityVertex,
            include_bytes!("../shaders/entity.vert.spv"),
        );
        let fs_spv = shaders.stage(
            super::shader_pack::ShaderStage::EntityFragment,
            include_bytes!("../shaders/entity.frag.spv"),
        );
        let vs_module = unsafe {
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
        let fs_module = unsafe {
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

        let entry_name = CString::new("main").unwrap();
        let stages = [
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::VERTEX,
                module: vs_module,
                p_name: entry_name.as_ptr(),
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::FRAGMENT,
                module: fs_module,
                p_name: entry_name.as_ptr(),
                ..Default::default()
            },
        ];

        use super::entity::mesh::EntityVertex;
        let vert_binding = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<EntityVertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vert_attrs = [
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 12,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 2,
                format: vk::Format::R32G32_SFLOAT,
                offset: 24,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 3,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: 32,
            },
        ];

        let viewports = [vk::Viewport {
            width: 1.0,
            height: 1.0,
            max_depth: 1.0,
            ..Default::default()
        }];
        let scissors = [vk::Rect2D::default()];
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let color_blend_att = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::RGBA,
        }];

        let info = vk::GraphicsPipelineCreateInfo {
            stage_count: 2,
            p_stages: stages.as_ptr(),
            p_vertex_input_state: &vk::PipelineVertexInputStateCreateInfo {
                vertex_binding_description_count: 1,
                p_vertex_binding_descriptions: vert_binding.as_ptr(),
                vertex_attribute_description_count: 4,
                p_vertex_attribute_descriptions: vert_attrs.as_ptr(),
                ..Default::default()
            },
            p_input_assembly_state: &vk::PipelineInputAssemblyStateCreateInfo {
                topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                ..Default::default()
            },
            p_viewport_state: &vk::PipelineViewportStateCreateInfo {
                viewport_count: 1,
                p_viewports: viewports.as_ptr(),
                scissor_count: 1,
                p_scissors: scissors.as_ptr(),
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
                p_attachments: color_blend_att.as_ptr(),
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
        };

        let pipeline =
            unsafe { device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None) }
                .unwrap()[0];

        unsafe {
            device.destroy_shader_module(vs_module, None);
            device.destroy_shader_module(fs_module, None);
        }

        (pipeline, pipeline_layout)
    }

    // --- Descriptors ---

    pub(super) fn create_descriptors(
        device: &ash::Device,
        layout: vk::DescriptorSetLayout,
        _uniform_buffers: &[vk::Buffer],
    ) -> (vk::DescriptorPool, Vec<vk::DescriptorSet>) {
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: (MAX_FRAMES * 4) as u32,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: (MAX_FRAMES * 4) as u32,
            },
        ];
        let pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo {
                    pool_size_count: 2,
                    p_pool_sizes: pool_sizes.as_ptr(),
                    max_sets: (MAX_FRAMES * 4) as u32,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let layouts = vec![layout; MAX_FRAMES];
        let sets = unsafe {
            device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo {
                descriptor_pool: pool,
                descriptor_set_count: MAX_FRAMES as u32,
                p_set_layouts: layouts.as_ptr(),
                ..Default::default()
            })
        }
        .unwrap();
        (pool, sets)
    }

    pub(super) fn write_descriptors(
        device: &ash::Device,
        sets: &[vk::DescriptorSet],
        uniform_buffers: &[vk::Buffer],
        tex_view: vk::ImageView,
        sampler: vk::Sampler,
    ) {
        for i in 0..MAX_FRAMES {
            let buf_info = vk::DescriptorBufferInfo {
                buffer: uniform_buffers[i],
                offset: 0,
                range: std::mem::size_of::<Uniforms>() as u64,
            };
            let img_info = vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: tex_view,
                sampler,
            };
            unsafe {
                device.update_descriptor_sets(
                    &[
                        vk::WriteDescriptorSet {
                            dst_set: sets[i],
                            dst_binding: 0,
                            descriptor_count: 1,
                            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                            p_buffer_info: &buf_info,
                            ..Default::default()
                        },
                        vk::WriteDescriptorSet {
                            dst_set: sets[i],
                            dst_binding: 1,
                            descriptor_count: 1,
                            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            p_image_info: &img_info,
                            ..Default::default()
                        },
                    ],
                    &[],
                );
            }
        }
    }

    // --- Sky pipeline ---

    pub(super) fn create_sky_pipeline(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        shaders: &super::shader_pack::ShaderPackShaders,
    ) -> (vk::Pipeline, vk::PipelineLayout, vk::DescriptorSetLayout) {
        // Descriptor layout: binding 0 = uniform buffer, binding 1 = sun texture,
        // binding 2 = moon texture, binding 3 = custom sky texture
        let bindings = [
            vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: 1,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: 2,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: 3,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ];
        let descriptor_layout = unsafe {
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo {
                    binding_count: 4,
                    p_bindings: bindings.as_ptr(),
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();

        // Pipeline layout — no push constants, uniforms via descriptor
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

        // Sky shaders
        let vs_spv = shaders.stage(
            super::shader_pack::ShaderStage::SkyVertex,
            include_bytes!("../shaders/sky.vert.spv"),
        );
        let fs_spv = shaders.stage(
            super::shader_pack::ShaderStage::SkyFragment,
            include_bytes!("../shaders/sky.frag.spv"),
        );
        let vs_module = unsafe {
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
        let fs_module = unsafe {
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

        let entry_name = CString::new("main").unwrap();
        let stages = [
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::VERTEX,
                module: vs_module,
                p_name: entry_name.as_ptr(),
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::FRAGMENT,
                module: fs_module,
                p_name: entry_name.as_ptr(),
                ..Default::default()
            },
        ];

        // Vertex input: just vec2 position
        let vert_binding = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: 8, // 2 * f32
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vert_attrs = [vk::VertexInputAttributeDescription {
            binding: 0,
            location: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        }];

        let vertex_input = vk::PipelineVertexInputStateCreateInfo {
            vertex_binding_description_count: vert_binding.len() as u32,
            p_vertex_binding_descriptions: vert_binding.as_ptr(),
            vertex_attribute_description_count: 1,
            p_vertex_attribute_descriptions: vert_attrs.as_ptr(),
            ..Default::default()
        };

        let viewports = [vk::Viewport {
            width: 1.0,
            height: 1.0,
            max_depth: 1.0,
            ..Default::default()
        }];
        let scissors = [vk::Rect2D::default()];
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        // No blending, no depth test/write — sky is always behind everything
        let color_blend_att = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::FALSE,
            color_write_mask: vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A,
            ..Default::default()
        }];

        let pipeline = unsafe {
            device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo {
                    stage_count: 2,
                    p_stages: stages.as_ptr(),
                    p_vertex_input_state: &vertex_input,
                    p_input_assembly_state: &vk::PipelineInputAssemblyStateCreateInfo {
                        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                        ..Default::default()
                    },
                    p_viewport_state: &vk::PipelineViewportStateCreateInfo {
                        viewport_count: 1,
                        p_viewports: viewports.as_ptr(),
                        scissor_count: 1,
                        p_scissors: scissors.as_ptr(),
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
                        p_attachments: color_blend_att.as_ptr(),
                        ..Default::default()
                    },
                    p_dynamic_state: &vk::PipelineDynamicStateCreateInfo {
                        dynamic_state_count: dynamic_states.len() as u32,
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

        // Cleanup shader modules (keep descriptor layout alive for descriptor sets)
        unsafe {
            device.destroy_shader_module(vs_module, None);
            device.destroy_shader_module(fs_module, None);
        }

        (pipeline, pipeline_layout, descriptor_layout)
    }
}
