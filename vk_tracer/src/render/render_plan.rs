use crate::{errors::Result, mem::ImageViewFatHandle, RenderPlanHandle, VkTracerApp};
use ash::vk::ClearColorValue;
use ash::{version::DeviceV1_2, vk};

impl VkTracerApp {
    pub fn new_render_plan(&mut self) -> RenderPlanBuilder {
        RenderPlanBuilder {
            app: self,
            clear_values: Vec::new(),
            attachments: Vec::new(),
            references: Vec::new(),
            dependencies: Vec::new(),
            subpasses: Vec::new(),
        }
    }
}

pub(crate) struct RenderPlan {
    pub(crate) render_pass: vk::RenderPass,
    // Data used to recreate the render pass when necessary
    pub(crate) clear_values: Vec<vk::ClearValue>,
    pub(crate) attachments: Vec<vk::AttachmentDescription2>,
    pub(crate) references: Vec<vk::AttachmentReference2>,
    pub(crate) subpasses: Vec<SubpassBuilder>,
}

pub struct RenderPlanBuilder<'app> {
    app: &'app mut VkTracerApp,
    clear_values: Vec<vk::ClearValue>,
    attachments: Vec<vk::AttachmentDescription2>,
    references: Vec<vk::AttachmentReference2>,
    dependencies: Vec<vk::SubpassDependency2>,
    subpasses: Vec<SubpassBuilder>,
}

impl RenderPlanBuilder<'_> {
    /// Add a color attachment that will be used for presentation.
    pub fn add_color_attachment_present(mut self, image: ImageViewFatHandle) -> Result<Self> {
        let description = vk::AttachmentDescription2::builder()
            .format(image.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
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
        self.clear_values.push(vk::ClearValue {
            color: ClearColorValue {
                float32: Default::default(),
            },
        });
        Ok(self)
    }

    pub fn add_depth_attachment(mut self, image: ImageViewFatHandle) -> Result<Self> {
        let description = vk::AttachmentDescription2::builder()
            .format(image.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let reference = vk::AttachmentReference2::builder()
            .attachment(self.attachments.len() as u32)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        self.attachments.push(description);
        self.references.push(reference);
        self.clear_values.push(vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        });
        Ok(self)
    }

    pub fn set_clear_color(mut self, index: usize, color: [f32; 4]) -> Self {
        self.clear_values[index] = vk::ClearValue {
            color: vk::ClearColorValue { float32: color },
        };
        self
    }

    pub fn set_clear_depth_stencil(mut self, index: usize, depth: f32, stencil: u32) -> Self {
        self.clear_values[index] = vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue { depth, stencil },
        };
        self
    }

    pub fn add_subpass(
        mut self,
        subpass: SubpassBuilder,
        dependency: Option<vk::SubpassDependency2>,
    ) -> Self {
        self.subpasses.push(subpass);
        if let Some(dependency) = dependency {
            self.dependencies.push(dependency);
        }
        self
    }

    pub fn build(self) -> Result<RenderPlanHandle> {
        let mut subpasses = Vec::with_capacity(self.subpasses.len());
        let mut subpasses_references = Vec::with_capacity(self.subpasses.len());

        for subpass in &self.subpasses {
            let color_attachments = subpass
                .color_attachments
                .iter()
                .copied()
                .map(|i| self.references[i])
                .collect::<Box<[_]>>();

            // Ok we can build because we know that the attachments will not move or drop
            let mut subpass_description = vk::SubpassDescription2::builder()
                .pipeline_bind_point(subpass.bind_point)
                .color_attachments(&color_attachments);

            if let Some(i) = subpass.depth_stencil_attachment {
                subpass_description = subpass_description.depth_stencil_attachment(&self.references[i]);

                subpasses_references.push(Vec::from([self.references[i]]).into_boxed_slice());
            }

            subpasses.push(subpass_description.build());

            subpasses_references.push(color_attachments);
        }

        let render_pass = unsafe {
            self.app.device.create_render_pass2(
                &vk::RenderPassCreateInfo2::builder()
                    .attachments(&self.attachments)
                    .dependencies(&self.dependencies)
                    .subpasses(&subpasses),
                None,
            )?
        };

        Ok(self.app.render_plan_storage.insert(RenderPlan {
            render_pass,
            clear_values: self.clear_values,
            attachments: self.attachments,
            references: self.references,
            subpasses: self.subpasses,
        }))
    }
}

pub struct SubpassBuilder {
    bind_point: vk::PipelineBindPoint,
    color_attachments: Box<[usize]>,
    depth_stencil_attachment: Option<usize>,
}

impl Default for SubpassBuilder {
    #[inline]
    fn default() -> Self {
        Self {
            bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachments: Box::default(),
            depth_stencil_attachment: None,
        }
    }
}

impl SubpassBuilder {
    pub fn new() -> Self {
        Self::default()
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

    pub fn depth_stencil_attachment(mut self, attachment: usize) -> Self {
        self.depth_stencil_attachment = Some(attachment);
        self
    }
}
