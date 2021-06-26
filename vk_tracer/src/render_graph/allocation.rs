use ash::vk;

use crate::errors::{HandleType, Result};
use crate::mem::{ImageDescription, RawImageAllocation};
use crate::present::Swapchain;
use crate::render_graph::{AttachmentSize, BakedRenderGraph, RenderPassType};
use crate::{SwapchainHandle, VkTracerApp};
use crate::ash::version::DeviceV1_2;

pub struct RenderGraphResourceAllocation {}

impl BakedRenderGraph {
    pub fn allocate_graph(
        &self,
        app: &VkTracerApp,
        swapchain: SwapchainHandle,
    ) -> Result<RenderGraphResourceAllocation> {
        let swapchain = storage_access!(app.swapchain_storage, swapchain, HandleType::Swapchain);

        let images = self.allocate_images(app, swapchain)?;

        Ok(())
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
        let render_passes = Vec::with_capacity(self.passes.len());

        for pass in self.passes.iter() {
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

            if let Some(id) = pass.depth_attachment {
                let res = &self.resources[id];
                let barrier = &pass.barriers[&id];

                attachments.push(get_attachment_description(
                    res.format
                    false,
                    true,
                    true,
                    true,
                    barrier.new_layout,
                    barrier.new_layout,
                ));
            }

            attachments.extend(pass.image_inputs.iter().copied().map(|id| {
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

            let subpass = vk::SubpassDescription2::builder()
                .pipeline_bind_point(match pass.ty {
                    RenderPassType::Graphics => vk::PipelineBindPoint::GRAPHICS,
                    RenderPassType::Compute => vk::PipelineBindPoint::COMPUTE,
                })
                .color_attachments(todo!());

            unsafe {
                app.device
                    .create_render_pass2(&vk::RenderPassCreateInfo2::builder().attachments(&attachments), None);
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
