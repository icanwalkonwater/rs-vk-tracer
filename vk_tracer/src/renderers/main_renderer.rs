use crate::{
    errors::Result,
    present::{render_pass::RenderPass, swapchain::Swapchain},
};
use ash::{version::DeviceV1_0, vk};
use std::{slice::from_ref, sync::Arc};

pub struct MainRenderer {
    device: Arc<ash::Device>,
    pool: vk::CommandPool,
    pub(crate) commands: vk::CommandBuffer,
}

impl MainRenderer {
    pub(crate) fn new(
        device: &Arc<ash::Device>,
        graphics_queue: &(vk::Queue, vk::CommandPool),
        swapchain: &Swapchain,
        render_pass: &RenderPass,
        swapchain_image_index: u32,
        pipelines: &[vk::CommandBuffer],
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
                        float32: [0.4, 0.3, 0.5, 1.0],
                    },
                };

                device.cmd_begin_render_pass(
                    command_buffer,
                    &vk::RenderPassBeginInfo::builder()
                        .render_pass(render_pass.handle)
                        .framebuffer(render_pass.framebuffers[swapchain_image_index as usize])
                        .render_area(
                            vk::Rect2D::builder()
                                .extent(swapchain.extent)
                                .offset(vk::Offset2D::default())
                                .build(),
                        )
                        .clear_values(from_ref(&clear_value)),
                    vk::SubpassContents::SECONDARY_COMMAND_BUFFERS,
                );

                // Execute other pipelines
                device.cmd_execute_commands(command_buffer, pipelines);

                device.cmd_end_render_pass(command_buffer);
            }

            device.end_command_buffer(command_buffer)?;
        }

        Ok(Self {
            device: Arc::clone(device),
            pool: graphics_queue.1,
            commands: command_buffer,
        })
    }
}

impl Drop for MainRenderer {
    fn drop(&mut self) {
        unsafe {
            self.device
                .free_command_buffers(self.pool, from_ref(&self.commands));
        }
    }
}
