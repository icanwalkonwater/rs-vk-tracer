use crate::{
    errors::Result,
    mesh::{Index, Mesh, Vertex},
    renderer_creator::RendererCreator,
};
use ash::{version::DeviceV1_0, vk};
use std::{fs::File, slice::from_ref, sync::Arc};

pub struct ForwardRenderer {
    pub(crate) creator: Arc<RendererCreator>,
    pub(crate) pipeline: vk::Pipeline,
}

impl ForwardRenderer {
    pub(crate) fn new<V: Vertex, I: Index>(
        creator: &Arc<RendererCreator>,
        vertex_shader: &mut File,
        fragment_shader: &mut File,
        mesh: Mesh<V, I>,
    ) -> Result<Self> {
        // Modules
        // <editor-fold>
        let vertex_module = unsafe {
            let spv = ash::util::read_spv(vertex_shader)?;
            creator
                .device
                .create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&spv), None)?
        };

        let fragment_module = unsafe {
            let spv = ash::util::read_spv(fragment_shader)?;
            creator
                .device
                .create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&spv), None)?
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

        let swapchain_extent = creator.swapchain.extent;
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

        let pipeline = unsafe {
            let create_info = vk::GraphicsPipelineCreateInfo::builder()
                .vertex_input_state(&vertex_input_info)
                .input_assembly_state(&input_assembly_info)
                .rasterization_state(&raster_state_info)
                .multisample_state(&msaa_info)
                .color_blend_state(&color_blend_state)
                .viewport_state(&viewport_state_info);

            let pipelines = creator
                .device
                .create_graphics_pipelines(vk::PipelineCache::null(), from_ref(&create_info), None)
                .map_err(|(_, err)| err)?;
            pipelines[0]
        };

        // Cleanup
        unsafe {
            creator.device.destroy_shader_module(vertex_module, None);
            creator.device.destroy_shader_module(fragment_module, None);
        }

        Ok(Self {
            creator: Arc::clone(creator),
            pipeline,
        })
    }
}
