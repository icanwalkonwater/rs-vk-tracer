use crate::renderer_creator::RendererCreator;
use ash::{version::DeviceV1_0, Device, vk};
use std::sync::Arc;
use std::fs::File;
use crate::mesh::{Mesh, Vertex, Index};
use crate::errors::Result;

pub struct ForwardRenderer {
    pub(crate) creator: Arc<RendererCreator>,
}

impl ForwardRenderer {
    pub(crate) fn new<V: Vertex, I: Index>(creator: &RendererCreator, vertex_shader: &mut File, fragment_shader: &mut File, mesh: Mesh<V, I>) -> Result<Self> {
        // Modules
        // <editor-fold>
        let vertex_module = unsafe {
            let spv = ash::util::read_spv(vertex_shader)?;
            creator.device.create_shader_module(
                &vk::ShaderModuleCreateInfo::builder()
                    .code(&spv),
                None,
            )?
        };

        let fragment_module = unsafe {
            let spv = ash::util::read_spv(fragment_shader)?;
            creator.device.create_shader_module(
                &vk::ShaderModuleCreateInfo::builder()
                    .code(&spv),
                None,
            )?
        };
        // </editor-fold>

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
            .depth_bias_enable(false);

        let msaa_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .min_sample_shading(1.0)
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false);

        let color_blend_info = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::all())
            .blend_enable(false);

        // TODO: that's wrong
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&[vk::Viewport::builder().build()])
            .scissors(&[vk::Rect2D::builder().build()]);

        // TODO: attachment
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(&[color_blend_info.build()]);

        let pipeline = unsafe {
            let pipelines = creator.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo::builder()
                    .vertex_input_state(&vertex_input_info)
                    .input_assembly_state(&input_assembly_info)
                    .rasterization_state(&raster_state_info)
                    .multisample_state(&msaa_info)
                    .color_blend_state(&color_blend_state)
                    .viewport_state(&viewport_state_info).build()],
                None,
            )?;
            pipelines[0]
        };

        Ok(())
    }
}
