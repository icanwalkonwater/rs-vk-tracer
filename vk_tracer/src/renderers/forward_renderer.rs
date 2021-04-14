use crate::{
    command_recorder::QueueType,
    errors::Result,
    mesh::{Index, Vertex},
    mesh_storage::MeshId,
    present::{render_pass::RenderPass, swapchain::Swapchain},
    renderer_creator::RendererCreator,
    utils::{str_to_cstr},
};
use ash::{version::DeviceV1_0, vk};
use std::{fs::File, slice::from_ref, sync::Arc};

pub struct ForwardRenderer {
    pub(crate) device: Arc<ash::Device>,
    pub(crate) layout: vk::PipelineLayout,
    pub(crate) pipeline: vk::Pipeline,
    pub(crate) mesh: MeshId,
}

impl ForwardRenderer {
    pub(crate) fn new<V: Vertex, I: Index>(
        device: &Arc<ash::Device>,
        swapchain: &Swapchain,
        render_pass: &RenderPass,
        vertex_shader: &mut File,
        fragment_shader: &mut File,
        mesh: MeshId,
    ) -> Result<Self> {
        // Modules
        // <editor-fold>
        let vertex_module = unsafe {
            let spv = ash::util::read_spv(vertex_shader)?;
            device.create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&spv), None)?
        };

        let fragment_module = unsafe {
            let spv = ash::util::read_spv(fragment_shader)?;
            device.create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&spv), None)?
        };
        // </editor-fold>

        let stage_vertex = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_module)
            .name(str_to_cstr("main\0"));

        let stage_fragment = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_module)
            .name(str_to_cstr("main\0"));

        let stages = [stage_vertex.build(), stage_fragment.build()];

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(V::binding_desc())
            .vertex_attribute_descriptions(V::attribute_desc());

        let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let raster_state_info = vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_bias_enable(false)
            .line_width(1.0);

        let msaa_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .min_sample_shading(1.0)
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false);

        let color_blend_info = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::all())
            .blend_enable(false);

        let swapchain_extent = swapchain.extent;
        let viewport = vk::Viewport::builder()
            .width(swapchain_extent.width as f32)
            .height(swapchain_extent.height as f32);
        let scissors = vk::Rect2D::builder()
            .extent(swapchain_extent)
            .offset(vk::Offset2D::default());
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(from_ref(&viewport))
            .scissors(from_ref(&scissors));

        // TODO: attachment
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(from_ref(&color_blend_info));

        let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&[]);

        let layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::builder()
                    .set_layouts(&[])
                    .push_constant_ranges(&[]),
                None,
            )?
        };

        let pipeline = unsafe {
            let create_info = vk::GraphicsPipelineCreateInfo::builder()
                .stages(&stages)
                .vertex_input_state(&vertex_input_info)
                .input_assembly_state(&input_assembly_info)
                .rasterization_state(&raster_state_info)
                .multisample_state(&msaa_info)
                .color_blend_state(&color_blend_state)
                .viewport_state(&viewport_state_info)
                .dynamic_state(&dynamic_state)
                .layout(layout)
                .render_pass(render_pass.handle)
                .subpass(0);

            let pipelines = device
                .create_graphics_pipelines(vk::PipelineCache::null(), from_ref(&create_info), None)
                .map_err(|(_, err)| err)?;
            pipelines[0]
        };

        // Cleanup
        unsafe {
            device.destroy_shader_module(vertex_module, None);
            device.destroy_shader_module(fragment_module, None);
        }

        Ok(Self {
            device: Arc::clone(device),
            layout,
            pipeline,
            mesh,
        })
    }

    pub(crate) fn draw(
        &self,
        creator: &RendererCreator,
        frame_index: u32,
    ) -> Result<vk::CommandBuffer> {
        let mesh = unsafe { creator.mesh_storage.get_mesh_unchecked(self.mesh) };
        let pool = creator
            .command_pools
            .get(&QueueType::Graphics)
            .unwrap()
            .lock();

        let buffer = unsafe {
            let buffer = creator.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::builder()
                    .level(vk::CommandBufferLevel::SECONDARY)
                    .command_buffer_count(1)
                    .command_pool(pool.1),
            )?[0];

            creator.device.begin_command_buffer(
                buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                    .inheritance_info(
                        &vk::CommandBufferInheritanceInfo::builder()
                            .render_pass(creator.render_pass.handle)
                            .subpass(0)
                            .framebuffer(creator.render_pass.framebuffers[frame_index as usize]),
                    ),
            );

            creator.device.cmd_bind_vertex_buffers(
                buffer,
                0,
                from_ref(&mesh.vertices.as_raw().buffer),
                from_ref(&(mesh.vertices.as_raw().info.get_offset() as vk::DeviceSize)),
            );

            creator.device.cmd_bind_index_buffer(
                buffer,
                mesh.indices.as_raw().buffer,
                mesh.indices.as_raw().info.get_offset() as vk::DeviceSize,
                vk::IndexType::UINT16, // TODO: replace with Index::ty() method
            );

            creator.device.cmd_bind_pipeline(
                buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            creator
                .device
                .cmd_draw_indexed(buffer, mesh.indices.len() as u32, 1, 0, 0, 0);

            creator.device.end_command_buffer(buffer)?;
            buffer
        };

        Ok(buffer)
    }
}

impl Drop for ForwardRenderer {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
        }
    }
}
