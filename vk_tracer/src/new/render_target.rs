use crate::new::{VkTracerApp, RenderTargetHandle, RenderPlanHandle};
use ash::vk;
use crate::new::errors::{Result, VkTracerError, HandleType};
use crate::new::mem::image::ImageViewFatHandle;
use ash::version::DeviceV1_0;

impl VkTracerApp {
    /// The first attachment must be the color attachment
    pub fn allocate_render_target<const N: usize>(&mut self, render_plan_handle: RenderPlanHandle, attachments: [ImageViewFatHandle; N]) -> Result<RenderTargetHandle> {
        let render_plan = self.render_plan_storage.get(render_plan_handle).ok_or(VkTracerError::InvalidHandle(HandleType::RenderPlan))?;
        debug_assert_eq!(render_plan.attachments.len(), N);

        let mut attachments_view = [vk::ImageView::null(); N];
        for (i, attachment) in attachments.iter().enumerate() {
            attachments_view[i] = attachment.view;
        }

        let framebuffer = unsafe {
            self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::builder()
                    .render_pass(render_plan.render_pass)
                    .attachments(&attachments_view)
                    .width(attachments[0].extent.width)
                    .height(attachments[0].extent.height)
                    .layers(1),
                None,
            )?
        };

        Ok(self.render_target_storage.insert(RenderTarget { framebuffer, extent: attachments[0].extent }))
    }
}

pub(crate) struct RenderTarget {
    pub(crate) framebuffer: vk::Framebuffer,
    pub(crate) extent: vk::Extent2D,
}
