use crate::{
    errors::{HandleType, Result},
    mem::ImageViewFatHandle,
    RenderPlanHandle, RenderTargetHandle, VkTracerApp,
};
use ash::vk;

impl VkTracerApp {
    /// The first attachment must be the color attachment
    pub fn allocate_render_target(
        &mut self,
        render_plan: RenderPlanHandle,
        attachments: &[ImageViewFatHandle],
    ) -> Result<RenderTargetHandle> {
        let render_plan = storage_access!(
            self.render_plan_storage,
            render_plan,
            HandleType::RenderPlan
        );
        debug_assert_eq!(render_plan.attachments.len(), attachments.len());

        let attachment_views = attachments.iter().map(|a| a.view).collect::<Vec<_>>();

        let framebuffer = unsafe {
            self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::builder()
                    .render_pass(render_plan.render_pass)
                    .attachments(&attachment_views)
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

    pub fn recreate_render_target<const N: usize>(
        &mut self,
        render_plan: RenderPlanHandle,
        new_window_size: (u32, u32),
        render_target: RenderTargetHandle,
        attachments: [ImageViewFatHandle; N],
    ) -> Result<()> {
        let render_plan = storage_access!(
            self.render_plan_storage,
            render_plan,
            HandleType::RenderPlan
        );
        let render_target = storage_access_mut!(
            self.render_target_storage,
            render_target,
            HandleType::RenderTarget
        );

        unsafe {
            self.device
                .destroy_framebuffer(render_target.framebuffer, None);
        }

        let mut attachment_views = [vk::ImageView::null(); N];
        for (i, attachment) in attachments.iter().enumerate() {
            attachment_views[i] = attachment.view;
        }

        let framebuffer = unsafe {
            self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::builder()
                    .render_pass(render_plan.render_pass)
                    .attachments(&attachment_views)
                    .width(new_window_size.0)
                    .height(new_window_size.1)
                    .layers(1),
                None,
            )?
        };

        render_target.extent = vk::Extent2D::builder()
            .width(new_window_size.0)
            .height(new_window_size.1)
            .build();
        render_target.framebuffer = framebuffer;
        Ok(())
    }
}

pub(crate) struct RenderTarget {
    pub(crate) framebuffer: vk::Framebuffer,
    pub(crate) extent: vk::Extent2D,
}
