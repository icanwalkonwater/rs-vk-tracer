use crate::{
    errors::{HandleType, Result},
    mem::find_depth_format,
    present::Swapchain,
    render_graph2::{
        baking::BakedRenderGraph,
        builder::{
            RenderGraphImageFormat, RenderGraphLogicalTag, RenderGraphPassResourceBindPoint,
            RenderGraphResourcePersistence,
        },
    },
    SwapchainHandle, VkTracerApp,
};
use ash::vk;
use std::slice::from_ref;
use crate::render_graph2::baking::BakedRenderGraphResource;

impl VkTracerApp {
    fn translate_format(
        &self,
        swapchain: &Swapchain,
        format: RenderGraphImageFormat,
    ) -> vk::Format {
        use RenderGraphImageFormat::*;
        match format {
            BackbufferFormat => swapchain.create_info.image_format,
            ColorRgba8Unorm => vk::Format::R8G8B8A8_UNORM,
            ColorRgba16Sfloat => vk::Format::R16G16B16A16_SFLOAT,
            DepthStencilOptimal => find_depth_format(self),
        }
    }
}

impl RenderGraphResourcePersistence {
    #[inline]
    fn load_op(&self) -> vk::AttachmentLoadOp {
        match self {
            Self::Transient | Self::PreserveOutput => vk::AttachmentLoadOp::DONT_CARE,
            Self::PreserveInput | Self::PreserveAll => vk::AttachmentLoadOp::LOAD,
            Self::ClearInput | Self::ClearInputPreserveOutput => vk::AttachmentLoadOp::CLEAR,
        }
    }

    #[inline]
    fn store_op(&self) -> vk::AttachmentStoreOp {
        match self {
            Self::Transient | Self::PreserveInput | Self::ClearInput => {
                vk::AttachmentStoreOp::DONT_CARE
            }
            Self::PreserveOutput | Self::ClearInputPreserveOutput | Self::PreserveAll => {
                vk::AttachmentStoreOp::STORE
            }
        }
    }
}

pub struct RenderGraphAllocation<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> {
    _phantom: std::marker::PhantomData<(R, P)>,
}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> RenderGraphAllocation<R, P> {
    pub(crate) fn new(
        app: &VkTracerApp,
        swapchain: SwapchainHandle,
        graph: &BakedRenderGraph<R, P>,
    ) -> Result<Self> {
        let swapchain = storage_access!(app.swapchain_storage, swapchain, HandleType::Swapchain);
        let vk_render_passes = Self::build_render_passes(app, swapchain, graph)?;

        graph.resources
        todo!()
    }

    fn build_render_passes(
        app: &VkTracerApp,
        swapchain: &Swapchain,
        graph: &BakedRenderGraph<R, P>,
    ) -> Result<Vec<vk::RenderPass>> {
        let mut passes = Vec::with_capacity(graph.passes.len());

        for (pass_physical, pass) in graph.passes.iter().enumerate() {
            let mut dependency_before = vk::MemoryBarrier2KHR::default();
            let mut dependency_after = vk::MemoryBarrier2KHR::default();

            let mut inputs = Vec::new();
            let mut colors = Vec::new();
            let mut depth = None;

            // Create attachments
            let attachments =
                pass.resources
                    .iter()
                    .filter(|res| res.bind_point().is_attachment())
                    .enumerate()
                    .map(|(attachment_index, res)| {
                        let timeline = &graph.resources_timelines[res.physical()];

                        // Update external dependencies
                        {
                            fn merge_barrier(
                                dst: &mut vk::MemoryBarrier2KHR,
                                src: &vk::MemoryBarrier2KHR,
                            ) {
                                dst.src_stage_mask |= src.src_stage_mask;
                                dst.src_access_mask |= src.src_access_mask;
                                dst.dst_stage_mask |= src.dst_stage_mask;
                                dst.dst_access_mask |= src.dst_access_mask;
                            }

                            let (before, after) = timeline.sync_around_pass(res.physical());
                            merge_barrier(&mut dependency_before, &before);
                            merge_barrier(&mut dependency_after, &after);
                        }

                        let description = vk::AttachmentDescription2::builder()
                            .flags(if res.bind_point().is_aliased() {
                                vk::AttachmentDescriptionFlags::MAY_ALIAS
                            } else {
                                vk::AttachmentDescriptionFlags::empty()
                            })
                            .format(app.translate_format(
                                swapchain,
                                graph.resources[res.physical()].format(),
                            ))
                            .samples(vk::SampleCountFlags::TYPE_1)
                            .load_op(res.persistence().load_op())
                            .store_op(res.persistence().store_op())
                            .stencil_load_op(if res.bind_point().is_depth() {
                                res.persistence().load_op()
                            } else {
                                vk::AttachmentLoadOp::DONT_CARE
                            })
                            .stencil_store_op(if res.bind_point().is_depth() {
                                res.persistence().store_op()
                            } else {
                                vk::AttachmentStoreOp::DONT_CARE
                            })
                            .initial_layout(timeline.layout_for_pass(pass_physical))
                            .final_layout(timeline.layout_after_pass(pass_physical))
                            .build();

                        let reference = vk::AttachmentReference2::builder()
                            .attachment(attachment_index as _)
                            .layout(description.initial_layout)
                            .aspect_mask(res.bind_point().aspect())
                            .build();

                        use RenderGraphPassResourceBindPoint::*;
                        match res.bind_point() {
                            InputAttachment | AliasedInputAttachment => inputs.push(reference),
                            ColorAttachment | AliasedColorAttachment => colors.push(reference),
                            DepthAttachment => depth = Some(reference),
                            _ => unreachable!(),
                        }

                        description
                    })
                    .collect::<Vec<_>>();

            let mut subpass = vk::SubpassDescription2::builder()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

            if !inputs.is_empty() {
                subpass = subpass.input_attachments(&inputs);
            }
            if !colors.is_empty() {
                subpass = subpass.color_attachments(&colors);
            }
            if depth.is_some() {
                subpass = subpass.depth_stencil_attachment(depth.as_ref().unwrap());
            }
            // TODO: resolve & preserve attachments

            // SAFETY: update lifetimes
            let mut dependency_before = dependency_before;
            let mut dependency_after = dependency_after;

            let render_pass = unsafe {
                app.device.create_render_pass2(
                    &vk::RenderPassCreateInfo2::builder()
                        .attachments(&attachments)
                        .subpasses(from_ref(&subpass))
                        .dependencies(&[
                            vk::SubpassDependency2::builder()
                                .src_subpass(vk::SUBPASS_EXTERNAL)
                                .dst_subpass(0)
                                .dependency_flags(vk::DependencyFlags::BY_REGION)
                                .push_next(&mut dependency_before)
                                .build(),
                            vk::SubpassDependency2::builder()
                                .src_subpass(0)
                                .dst_subpass(vk::SUBPASS_EXTERNAL)
                                .dependency_flags(vk::DependencyFlags::BY_REGION)
                                .push_next(&mut dependency_after)
                                .build(),
                        ]),
                    None,
                )?
            };

            passes.push(render_pass);
        }

        Ok(passes)
    }

    fn allocate_attachments(graph: &BakedRenderGraph<R, P>) {
        let size = graph.resources.iter().find(|res| match res { BakedRenderGraphResource::Image {  } })
    }

    fn build_framebuffers(app: &VkTracerApp, render_passes: &[vk::RenderPass]) {
        vk::FramebufferCreateInfo {
            s_type: Default::default(),
            p_next: (),
            flags: Default::default(),
            render_pass: Default::default(),
            attachment_count: 0,
            p_attachments: (),
            width: 0,
            height: 0,
            layers: 0
        }
    }
}
