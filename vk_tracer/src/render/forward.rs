use std::{
    io::{Read, Seek},
    slice::from_ref,
};

use ash::{version::DeviceV1_0, vk, vk::CommandBuffer};

use crate::{
    errors::{HandleType, Result},
    mesh::Mesh,
    render::{RenderPlan, VkRecordable},
    utils::str_to_cstr,
    DescriptorSetHandle, ForwardPipelineHandle, MeshHandle, RenderPlanHandle, VkTracerApp,
};

impl VkTracerApp {
    pub fn create_forward_pipeline(
        &mut self,
        render_plan: RenderPlanHandle,
        subpass: u32,
        descriptor_sets_handles: &[DescriptorSetHandle],
        vertex_shader: impl Read + Seek,
        fragment_shader: impl Read + Seek,
        mesh_handle: MeshHandle,
    ) -> Result<ForwardPipelineHandle> {
        let mesh = storage_access!(self.mesh_storage, mesh_handle, HandleType::Mesh);
        let render_plan = storage_access!(
            self.render_plan_storage,
            render_plan,
            HandleType::RenderPlan
        );

        let mut descriptor_layouts = Vec::with_capacity(descriptor_sets_handles.len());
        let mut descriptor_sets = Vec::with_capacity(descriptor_sets_handles.len());
        for handle in descriptor_sets_handles.iter().copied() {
            let set = storage_access!(
                self.descriptor_set_storage,
                handle,
                HandleType::DescriptorSet
            );
            descriptor_layouts.push(set.layout);
            descriptor_sets.push(set.handle);
        }

        let pipeline = ForwardPipeline::new(
            &self.device,
            render_plan,
            subpass,
            &descriptor_layouts,
            descriptor_sets.into_boxed_slice(),
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
    pub(crate) descriptor_sets: Box<[vk::DescriptorSet]>,
    pub(crate) mesh: MeshHandle,
}

impl ForwardPipeline {
    pub fn new(
        device: &ash::Device,
        render_plan: &RenderPlan,
        subpass: u32,
        descriptor_layouts: &[vk::DescriptorSetLayout],
        descriptor_sets: Box<[vk::DescriptorSet]>,
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
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
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
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
            .viewport_count(1)
            .scissor_count(1);

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
                    .set_layouts(descriptor_layouts)
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

        unsafe {
            device.destroy_shader_module(vertex_module, None);
            device.destroy_shader_module(fragment_module, None);
        }

        Ok(Self {
            pipeline,
            pipeline_layout,
            descriptor_sets,
            mesh: mesh_handle,
        })
    }
}

impl VkRecordable for ForwardPipeline {
    unsafe fn record_commands(
        &self,
        app: &VkTracerApp,
        viewport: vk::Extent2D,
        commands: CommandBuffer,
    ) -> Result<()> {
        let mesh = storage_access!(app.mesh_storage, self.mesh, HandleType::Mesh);

        app.device.cmd_bind_vertex_buffers(
            commands,
            0,
            from_ref(&mesh.vertices.buffer),
            &[0],
            //from_ref(&(mesh.vertices.info.get_offset() as vk::DeviceSize)),
        );

        app.device.cmd_bind_index_buffer(
            commands,
            mesh.indices.buffer,
            0,
            // mesh.indices.info.get_offset() as vk::DeviceSize,
            mesh.index_ty.1,
        );

        if !self.descriptor_sets.is_empty() {
            app.device.cmd_bind_descriptor_sets(
                commands,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &self.descriptor_sets,
                &[],
            );
        }

        app.device
            .cmd_bind_pipeline(commands, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

        app.device.cmd_set_viewport(
            commands,
            0,
            from_ref(
                &vk::Viewport::builder()
                    .height(viewport.height as f32)
                    .width(viewport.width as f32)
                    .x(0.0)
                    .y(0.0)
                    .min_depth(0.0)
                    .max_depth(1.0),
            ),
        );

        app.device.cmd_set_scissor(
            commands,
            0,
            from_ref(
                &vk::Rect2D::builder()
                    .extent(viewport)
                    .offset(vk::Offset2D::default()),
            ),
        );

        app.device
            .cmd_draw_indexed(commands, mesh.indices_len, 1, 0, 0, 1);

        Ok(())
    }
}
