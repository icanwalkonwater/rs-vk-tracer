use crate::new::{
    errors::{HandleType, Result, VkTracerError},
    mem::image::ImageViewFatHandle,
    RenderPlanHandle, RenderTargetHandle, VkTracerApp,
};
use ash::{version::DeviceV1_0, vk};

impl VkTracerApp {
    /// The first attachment must be the color attachment
    pub fn allocate_render_target(
        &mut self,
        render_plan_handle: RenderPlanHandle,
        attachments: &[ImageViewFatHandle],
    ) -> Result<RenderTargetHandle> {
        let render_plan = self
            .render_plan_storage
            .get(render_plan_handle)
            .ok_or(VkTracerError::InvalidHandle(HandleType::RenderPlan))?;
        debug_assert_eq!(render_plan.attachments.len(), attachments.len());

        let attachments_view = attachments.iter()
            .map(|a| a.view)
            .collect::<Vec<_>>();

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

        Ok(self.render_target_storage.insert(RenderTarget {
            framebuffer,
            extent: attachments[0].extent,
        }))
    }
}

pub(crate) struct RenderTarget {
    pub(crate) framebuffer: vk::Framebuffer,
    pub(crate) extent: vk::Extent2D,
}
