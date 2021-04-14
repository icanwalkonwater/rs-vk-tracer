use crate::new::{errors::Result, ForwardPipelineHandle, VkTracerApp, RendererHandle, SwapchainHandle};
use ash::vk;
use ash::version::DeviceV1_0;
use std::slice::from_ref;
use crate::new::errors::{VkTracerError, HandleType};
use crate::command_recorder::QueueType;

pub(crate) mod forward;
pub(crate) mod renderer;

#[derive(Copy, Clone)]
pub enum RenderablePipelineHandle {
    Forward(ForwardPipelineHandle),
}

impl Into<RenderablePipelineHandle> for ForwardPipelineHandle {
    fn into(self) -> RenderablePipelineHandle {
        RenderablePipelineHandle::Forward(self)
    }
}

trait VkRecordable {
    /// Only record bind and draw commands, no begin or end !
    unsafe fn record_commands(&self, app: &VkTracerApp, commands: vk::CommandBuffer) -> Result<()>;
}

impl VkTracerApp {
    pub fn render_and_present(&mut self, renderer: RendererHandle, swapchain: SwapchainHandle, render_target_index: u32) -> Result<()> {
        let renderer = self.renderer_storage.get(renderer).ok_or(VkTracerError::InvalidHandle(HandleType::Renderer))?;
        let swapchain = self.swapchain_storage.get(swapchain).ok_or(VkTracerError::InvalidHandle(HandleType::Swapchain))?;

        let render_semaphore = unsafe { self.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };

        // Reset render fence
        unsafe {
            // Should return immediately but its a precaution
            self.device.wait_for_fences(from_ref(&renderer.render_fence), true, u64::MAX)?;
            self.device.reset_fences(from_ref(&renderer.render_fence))?;
        }

        let submit_info = vk::SubmitInfo::builder()
            .wait_dst_stage_mask(from_ref(&vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT))
            .wait_semaphores(from_ref(&swapchain.image_available_semaphore))
            .signal_semaphores(from_ref(&render_semaphore))
            .command_buffers(from_ref(&renderer.commands));

        let present_info = vk::PresentInfoKHR::builder()
            .swapchains(from_ref(&swapchain.handle))
            .wait_semaphores(from_ref(&render_semaphore))
            .image_indices(from_ref(&render_target_index));

        let graphics_queue = self.command_pools.get(&QueueType::Graphics).unwrap().0;
        unsafe {
            // Launch render
            self.device.queue_submit(
                graphics_queue,
                from_ref(&submit_info),
                renderer.render_fence,
            )?;

            let is_suboptimal = swapchain.loader.queue_present(graphics_queue, &present_info)?;
        }

        unsafe {
            // Wait for the end of the render
            self.device.wait_for_fences(from_ref(&renderer.render_fence), true, u64::MAX)?;

            // Now we can free the semaphore
            self.device.destroy_semaphore(render_semaphore, None);
        }

        Ok(())
    }
}
