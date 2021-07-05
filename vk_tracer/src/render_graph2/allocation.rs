use crate::{VkTracerApp, SwapchainHandle};
use crate::errors::{Result, HandleType};
use crate::render_graph2::baking::{BakedRenderGraph, BakedRenderGraphPassResource};
use crate::render_graph2::builder::{RenderGraphLogicalTag, RenderGraphImageFormat, RenderGraphResourcePersistence, RenderGraphPassResourceBindPoint};
use ash::vk;
use crate::present::Swapchain;
use crate::mem::find_depth_format;
use std::slice::from_ref;

impl VkTracerApp {
    fn translate_format(&self, swapchain: &Swapchain, format: RenderGraphImageFormat) -> vk::Format {
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
            Self::Transient | Self::PreserveInput | Self::ClearInput => vk::AttachmentStoreOp::DONT_CARE,
            Self::PreserveOutput | Self::ClearInputPreserveOutput | Self::PreserveAll => vk::AttachmentStoreOp::STORE,
        }
    }
}

pub struct RenderGraphAllocation<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> {
    _phantom: std::marker::PhantomData<(R, P)>,
}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> RenderGraphAllocation<R, P> {
    pub(crate) fn new(app: &VkTracerApp, swapchain: SwapchainHandle, graph: &BakedRenderGraph<R, P>) -> Result<Self> {
        let swapchain = storage_access!(app.swapchain_storage, swapchain, HandleType::Swapchain);
        todo!()
    }

    fn build_render_passes(app: &VkTracerApp, swapchain: &Swapchain, graph: &BakedRenderGraph<R, P>) -> Result<Vec<vk::RenderPass>> {
        let mut passes = Vec::with_capacity(graph.passes.len());

        for (pass_physical, pass) in graph.passes.iter().enumerate() {

            let mut dependency_before = vk::MemoryBarrier2KHR::default();
            let mut dependency_after = vk::MemoryBarrier2KHR::default();

            let mut inputs = Vec::new();
            let mut colors = Vec::new();
            let mut depth = None;

            // Create attachments
            let attachments = pass.resources.iter()
                .filter(|res| res.bind_point().is_attachment())
                .enumerate()
                .map(|(attachment_index, res)| {
                    let timeline = &graph.resources_timelines[res.physical()];

                    // Update external dependencies
                    {
                        let barrier = timeline.sync_before_pass(res.physical());
                        dependency_before.src_stage_mask |= barrier.src_stage_mask;
                        dependency_before.src_access_mask |= barrier.src_access_mask;
                        dependency_before.dst_stage_mask |= barrier.dst_stage_mask;
                        dependency_before.dst_access_mask |= barrier.dst_access_mask;

                        let barrier = timeline.sync_after_pass(res.physical());
                        dependency_after.src_stage_mask |= barrier.src_stage_mask;
                        dependency_after.src_access_mask |= barrier.src_access_mask;
                        dependency_after.dst_stage_mask |= barrier.dst_stage_mask;
                        dependency_after.dst_access_mask |= barrier.dst_access_mask;
                    }

                    let description = vk::AttachmentDescription2::builder()
                        .flags(if res.bind_point().is_aliased() { vk::AttachmentDescriptionFlags::MAY_ALIAS } else { vk::AttachmentDescriptionFlags::empty() })
                        .format(app.translate_format(swapchain, graph.resources[res.physical()].format()))
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .load_op(res.persistence().load_op())
                        .store_op(res.persistence().store_op())
                        .stencil_load_op(if res.bind_point().is_depth() { res.persistence().load_op() } else { vk::AttachmentLoadOp::DONT_CARE })
                        .stencil_store_op(if res.bind_point().is_depth() { res.persistence().store_op() } else { vk::AttachmentStoreOp::DONT_CARE })
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
                                .build()
                        ]),
                    None,
                )?
            };

            passes.push(render_pass);
        }

        Ok(passes)
    }
}
