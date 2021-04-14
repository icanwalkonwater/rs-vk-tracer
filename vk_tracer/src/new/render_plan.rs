use ash::vk;
use ash::version::DeviceV1_2;
use crate::new::{VkTracerApp, RenderPlanHandle};
use crate::new::errors::{Result};
use crate::new::mem::image::ImageViewFatHandle;

impl VkTracerApp {
    pub fn new_render_plan(&mut self) -> RenderPlanBuilder {
        RenderPlanBuilder {
            app: self,
            attachments: Vec::new(),
            references: Vec::new(),
            subpasses: Vec::new(),
        }
    }
}

pub(crate) struct RenderPlan {
    pub(crate) render_pass: vk::RenderPass,
    // Data used to recreate the render pass when necessary
    pub(crate) attachments: Vec<vk::AttachmentDescription2>,
    pub(crate) references: Vec<vk::AttachmentReference2>,
    pub(crate) subpasses: Vec<SubpassBuilder>,
}

pub struct RenderPlanBuilder<'app> {
    app: &'app mut VkTracerApp,
    attachments: Vec<vk::AttachmentDescription2>,
    references: Vec<vk::AttachmentReference2>,
    subpasses: Vec<SubpassBuilder>,
}

impl RenderPlanBuilder<'_> {
    /// Add a color attachment that will be used for presentation.
    pub fn add_color_attachment_present(mut self, image: ImageViewFatHandle) -> Result<Self> {
        let description = vk::AttachmentDescription2::builder()
            .format(image.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();

        let reference = vk::AttachmentReference2::builder()
            .attachment(self.attachments.len() as u32)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();

        self.attachments.push(description);
        self.references.push(reference);
        Ok(self)
    }

    pub fn add_subpass(mut self, subpass: SubpassBuilder) -> Self {
        self.subpasses.push(subpass);
        self
    }

    pub fn build(self) -> Result<RenderPlanHandle> {
        let mut subpasses = Vec::with_capacity(self.subpasses.len());
        let mut subpasses_references = Vec::with_capacity(self.subpasses.len());

        for subpass in &self.subpasses {
            subpasses_references.push(subpass.color_attachments.iter()
                .copied()
                .map(|i| self.references[i])
                .collect::<Box<[_]>>()
            );

            // Ok we can build because we know that the attachments will not move or drop
            subpasses.push(vk::SubpassDescription2::builder()
                .pipeline_bind_point(subpass.bind_point)
                .color_attachments(subpasses_references.last().unwrap())
                .build()
            );
        }

        let render_pass = unsafe {
            self.app.device.create_render_pass2(
                &vk::RenderPassCreateInfo2::builder()
                    .attachments(&self.attachments)
                    .subpasses(&subpasses),
                None,
            )?
        };

        Ok(self.app.render_plan_storage.insert(RenderPlan {
            render_pass,
            attachments: self.attachments,
            references: self.references,
            subpasses: self.subpasses,
        }))
    }
}

pub struct SubpassBuilder {
    bind_point: vk::PipelineBindPoint,
    color_attachments: Box<[usize]>,
}

impl SubpassBuilder {
    pub fn new() -> Self {
        Self {
            bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachments: Box::default(),
        }
    }

    pub fn graphics(mut self) -> Self {
        self.bind_point = vk::PipelineBindPoint::GRAPHICS;
        self
    }

    pub fn compute(mut self) -> Self {
        self.bind_point = vk::PipelineBindPoint::COMPUTE;
        self
    }

    pub fn color_attachments<const N: usize>(mut self, attachments: [usize; N]) -> Self {
        self.color_attachments = Vec::from(attachments).into_boxed_slice();
        self
    }
}
