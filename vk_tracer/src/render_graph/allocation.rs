use ash::vk;

use crate::errors::{HandleType, Result};
use crate::mem::{ImageDescription, RawImageAllocation};
use crate::present::Swapchain;
use crate::render_graph::{AttachmentSize, BakedRenderGraph, RenderPassType};
use crate::{SwapchainHandle, VkTracerApp};
use crate::ash::version::DeviceV1_2;
use std::slice::from_ref;

pub struct RenderGraphResourceAllocation {}

impl BakedRenderGraph {
    pub fn allocate_graph(
        &self,
        app: &VkTracerApp,
        swapchain: SwapchainHandle,
    ) -> Result<RenderGraphResourceAllocation> {
        let swapchain = storage_access!(app.swapchain_storage, swapchain, HandleType::Swapchain);

        let images = self.allocate_images(app, swapchain)?;

        Ok(RenderGraphResourceAllocation {})
    }

    fn allocate_images(
        &self,
        app: &VkTracerApp,
        swapchain: &Swapchain,
    ) -> Result<Vec<(RawImageAllocation, vk::ImageView)>> {
        let vma = &app.vma;
        let mut images = Vec::with_capacity(self.resources.len());

        for resource in self.resources.iter() {
            if resource.is_backbuffer {
                continue;
            }

            let image = RawImageAllocation::new(
                vma,
                &ImageDescription {
                    ty: vk::ImageType::TYPE_2D,
                    extent: match resource.size {
                        AttachmentSize::Fixed(extent) => extent,
                        AttachmentSize::SwapchainRelative => vk::Extent3D {
                            width: swapchain.extent.width,
                            height: swapchain.extent.height,
                            depth: 1,
                        },
                    },
                    format: resource.format,
                    usage: resource.usages,
                    array_layers: 1,
                    mip_levels: 1,
                },
            )?;

            let view = image.fullscreen_view(&app.device, vk::ImageAspectFlags::COLOR)?;

            images.push((image, view));
        }

        Ok(images)
    }

    fn allocate_render_passes(
        &self,
        app: &VkTracerApp,
        images: Vec<(RawImageAllocation, vk::ImageView)>,
    ) -> Result<()> {
        let mut barriers = Vec::new();
        let mut render_passes = Vec::with_capacity(self.passes.len());

        for pass in self.passes.iter() {
            // Barriers
            barriers.extend(pass.image_inputs.iter().copied().map(|id| pass.barriers[&id]));

            // Attachments & their refs
            let mut attachments = Vec::new();

            attachments.extend(pass.color_attachments.iter().copied().map(|id| {
                let res = &self.resources[id];
                let barrier = &pass.barriers[&id];

                get_attachment_description(
                    res.format,
                    barrier.src_access == vk::AccessFlags::empty(),
                    true,
                    false,
                    false,
                    barrier.new_layout,
                    barrier.new_layout,
                )
            }));

            let input_attachments_offset = attachments.len();

            attachments.extend(pass.input_attachments.iter().copied().map(|id| {
                let res = &self.resources[id];
                let barrier = &pass.barriers[&id];

                get_attachment_description(
                    res.format,
                    true,
                    false,
                    false,
                    false,
                    barrier.new_layout,
                    barrier.new_layout,
                )
            }));

            let depth_attachment_offset = attachments.len();

            if let Some(id) = pass.depth_attachment {
                let res = &self.resources[id];
                let barrier = &pass.barriers[&id];

                attachments.push(get_attachment_description(
                    res.format,
                    false,
                    true,
                    true,
                    true,
                    barrier.new_layout,
                    barrier.new_layout,
                ));
            }

            let color_refs = attachments.iter()
                .enumerate()
                .take(pass.color_attachments.len())
                .map(|(i, desc)| vk::AttachmentReference2::builder()
                    .attachment(i as _)
                    .layout(desc.initial_layout)
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .build()
                )
                .collect::<Box<_>>();

            let input_refs = attachments.iter()
                .enumerate()
                .skip(input_attachments_offset)
                .take(pass.input_attachments.len())
                .map(|(i, desc)| vk::AttachmentReference2::builder()
                    .attachment(i as _)
                    .layout(desc.initial_layout)
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .build()
                )
                .collect::<Box<_>>();

            let depth_ref = pass.depth_attachment.map(|desc|
                {
                    let desc = &attachments[depth_attachment_offset];
                    vk::AttachmentReference2::builder()
                        .attachment(depth_attachment_offset as _)
                        .layout(desc.initial_layout)
                        .aspect_mask(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                        .build()
                }
            );

            let subpass = vk::SubpassDescription2::builder()
                .pipeline_bind_point(match pass.ty {
                    RenderPassType::Graphics => vk::PipelineBindPoint::GRAPHICS,
                    RenderPassType::Compute => vk::PipelineBindPoint::COMPUTE,
                })
                .color_attachments(&color_refs)
                .input_attachments(&input_refs);

            let subpass = if let Some(depth) = depth_ref {
                subpass.depth_stencil_attachment(&depth)
            } else {
                subpass
            };

            vk::SubpassDependency2::builder()


            unsafe {
                app.device
                    .create_render_pass2(
                        &vk::RenderPassCreateInfo2::builder()
                            .attachments(&attachments)
                            .subpasses(from_ref(&subpass.build()))
                        ,
                     None
                    );
            }
        }

        Ok(())
    }
}

fn get_attachment_description(
    format: vk::Format,
    load_or_clear: bool,
    store: bool,
    stencil_load: bool,
    stencil_store: bool,
    initial_layout: vk::ImageLayout,
    final_layout: vk::ImageLayout,
) -> vk::AttachmentDescription2KHR {
    vk::AttachmentDescription2KHR::builder()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(if load_or_clear {
            vk::AttachmentLoadOp::LOAD
        } else {
            vk::AttachmentLoadOp::CLEAR
        })
        .store_op(if store {
            vk::AttachmentStoreOp::STORE
        } else {
            vk::AttachmentStoreOp::DONT_CARE
        })
        .stencil_load_op(if stencil_load {
            vk::AttachmentLoadOp::LOAD
        } else {
            vk::AttachmentLoadOp::DONT_CARE
        })
        .stencil_store_op(if stencil_store {
            vk::AttachmentStoreOp::STORE
        } else {
            vk::AttachmentStoreOp::DONT_CARE
        })
        .initial_layout(initial_layout)
        .final_layout(final_layout)
        .build()
}
