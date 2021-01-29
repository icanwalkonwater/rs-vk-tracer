use crate::{
    errors::Result,
    present::{render_pass::RenderPass, swapchain::Swapchain},
};
use ash::{version::DeviceV1_0, vk};
use std::{slice::from_ref, sync::Arc};

pub struct MainRenderer {
    device: Arc<ash::Device>,
    pub(crate) commands: vk::CommandBuffer,
}

impl MainRenderer {
    pub(crate) fn new(
        device: &Arc<ash::Device>,
        graphics_queue: (vk::Queue, vk::CommandPool),
        swapchain: &Swapchain,
        render_pass: &RenderPass,
        swapchain_image_index: usize,
    ) -> Result<Self> {
        let command_buffer = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::builder()
                    .command_pool(graphics_queue.1)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0]
        };

        unsafe {
            device.begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())?;

            {
                let clear_value = vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.0, 0.0, 1.0, 1.0],
                    },
                };
                device.cmd_begin_render_pass(
                    command_buffer,
                    &vk::RenderPassBeginInfo::builder()
                        .render_pass(render_pass.handle)
                        .framebuffer(render_pass.framebuffers[swapchain_image_index])
                        .render_area(
                            vk::Rect2D::builder()
                                .extent(swapchain.extent)
                                .offset(vk::Offset2D::default())
                                .build(),
                        )
                        .clear_values(from_ref(&clear_value)),
                    vk::SubpassContents::SECONDARY_COMMAND_BUFFERS,
                );

                // TODO: render every other renderer

                device.cmd_end_render_pass(command_buffer);
            }

            device.end_command_buffer(command_buffer)?;
        }

        Ok(Self {
            device: Arc::clone(device),
            commands: command_buffer,
        })
    }
}
