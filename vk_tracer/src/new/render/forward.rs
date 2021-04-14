use crate::{
    new::{
        errors::{HandleType, Result, VkTracerError},
        mesh::Mesh,
        render::VkRecordable,
        render_plan::RenderPlan,
        ForwardPipelineHandle, MeshHandle, RenderPlanHandle, VkTracerApp,
    },
    utils::str_to_cstr,
};
use ash::{version::DeviceV1_0, vk, vk::CommandBuffer};
use std::{
    io::{Read, Seek},
    slice::from_ref,
};
use log::debug;

impl VkTracerApp {
    pub fn create_forward_pipeline(
        &mut self,
        render_plan: RenderPlanHandle,
        subpass: u32,
        vertex_shader: impl Read + Seek,
        fragment_shader: impl Read + Seek,
        mesh_handle: MeshHandle,
    ) -> Result<ForwardPipelineHandle> {
        let mesh = self
            .mesh_storage
            .get(mesh_handle)
            .ok_or(VkTracerError::InvalidHandle(HandleType::Mesh))?;
        let render_plan = self
            .render_plan_storage
            .get(render_plan)
            .ok_or(VkTracerError::InvalidHandle(HandleType::RenderPlan))?;
        let pipeline = ForwardPipeline::new(
            &self.device,
            render_plan,
            subpass,
            vertex_shader,
            fragment_shader,
            mesh_handle,
            mesh,
        )?;

        Ok(self.forward_pipeline_storage.insert(pipeline))
    }
}

pub(crate) struct ForwardPipeline {
    pub(crate) pipeline: vk::Pipeline,
    pub(crate) pipeline_layout: vk::PipelineLayout,
    pub(crate) mesh: MeshHandle,
}

impl ForwardPipeline {
    pub fn new(
        device: &ash::Device,
        render_plan: &RenderPlan,
        subpass: u32,
        mut vertex_shader: impl Read + Seek,
        mut fragment_shader: impl Read + Seek,
        mesh_handle: MeshHandle,
        mesh: &Mesh,
    ) -> Result<Self> {
        let vertex_module = unsafe {
            let spv = ash::util::read_spv(&mut vertex_shader)?;
            device.create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&spv), None)?
        };

        let fragment_module = unsafe {
            let spv = ash::util::read_spv(&mut fragment_shader)?;
            device.create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&spv), None)?
        };

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
            .vertex_binding_descriptions(mesh.vertex_desc.1)
            .vertex_attribute_descriptions(mesh.vertex_desc.2);

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

        // Dynamic state
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::default();

        // TODO: attachments
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(from_ref(&color_blend_info));

        let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);

        let pipeline_layout = unsafe {
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
                .layout(pipeline_layout)
                .render_pass(render_plan.render_pass)
                .subpass(subpass);

            let pipelines = device
                .create_graphics_pipelines(vk::PipelineCache::null(), from_ref(&create_info), None)
                .map_err(|(_, err)| err)?;
            pipelines[0]
        };

        Ok(Self {
            pipeline,
            pipeline_layout,
            mesh: mesh_handle,
        })
    }
}

impl VkRecordable for ForwardPipeline {
    unsafe fn record_commands(&self, app: &VkTracerApp, commands: CommandBuffer) -> Result<()> {
        let mesh = app
            .mesh_storage
            .get(self.mesh)
            .ok_or(VkTracerError::InvalidHandle(HandleType::Mesh))?;

        app.device.cmd_bind_vertex_buffers(
            commands,
            0,
            from_ref(&mesh.vertices.buffer),
            from_ref(&(mesh.vertices.info.get_offset() as vk::DeviceSize)),
        );

        app.device.cmd_bind_index_buffer(
            commands,
            mesh.indices.buffer,
            mesh.indices.info.get_offset() as vk::DeviceSize,
            mesh.index_ty.1,
        );

        app.device
            .cmd_bind_pipeline(commands, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

        app.device
            .cmd_draw_indexed(commands, mesh.indices_len, 1, 0, 0, 1);

        Ok(())
    }
}
