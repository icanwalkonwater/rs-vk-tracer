use crate::{errors::Result, present::swapchain::Swapchain};
use ash::{version::DeviceV1_0, vk};
use std::{slice::from_ref, sync::Arc};

pub(crate) struct RenderPass {
    device: Arc<ash::Device>,
    pub(crate) render_pass: vk::RenderPass,
    pub(crate) framebuffers: Vec<vk::Framebuffer>,
}

impl RenderPass {
    pub(crate) fn new_default(device: &Arc<ash::Device>, swapchain: &Swapchain) -> Result<Self> {
        // Create render pass
        let render_pass = {
            let color_attachment = vk::AttachmentDescription::builder()
                .format(swapchain.surface.format)
                .samples(vk::SampleCountFlags::TYPE_1)
                // Clear before pass
                .load_op(vk::AttachmentLoadOp::CLEAR)
                // Store at the end of pass
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                // Can build safely
                .build();

            let color_attachment_ref = vk::AttachmentReference::builder()
                .attachment(0)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                // Can build safely
                .build();

            let subpass = vk::SubpassDescription::builder()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(from_ref(&color_attachment_ref));
            let subpasses = [subpass.build()];

            unsafe {
                device.create_render_pass(
                    &vk::RenderPassCreateInfo::builder()
                        .attachments(from_ref(&color_attachment))
                        .subpasses(&subpasses),
                    None,
                )?
            }
        };

        // Create associated framebuffers
        let framebuffers = {
            let mut info = vk::FramebufferCreateInfo::builder()
                .render_pass(render_pass)
                .width(swapchain.extent.width)
                .height(swapchain.extent.height)
                .layers(1);

            let mut framebuffers = Vec::with_capacity(swapchain.image_views.len());

            for image_view in swapchain.image_views.iter().copied() {
                let framebuffer = unsafe {
                    device.create_framebuffer(
                        &vk::FramebufferCreateInfo::builder()
                            .render_pass(render_pass)
                            .attachments(from_ref(&image_view))
                            .width(swapchain.extent.width)
                            .height(swapchain.extent.height)
                            .layers(1),
                        None,
                    )?
                };

                framebuffers.push(framebuffer);
            }

            framebuffers
        };

        Ok(Self {
            device: Arc::clone(device),
            render_pass,
            framebuffers,
        })
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        unsafe {
            for framebuffer in self.framebuffers.iter().copied() {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}
