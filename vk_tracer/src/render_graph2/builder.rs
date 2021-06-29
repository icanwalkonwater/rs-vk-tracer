use super::GraphValidationError;
use crate::errors::{Result, VkTracerError};
use ash::vk;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::hash::Hash;

pub trait RenderGraphLogicalTag: Copy + Eq + Hash + Debug + Display + 'static {}

impl<T: Copy + Eq + Hash + Debug + Display + 'static> RenderGraphLogicalTag for T {}

/// Immutable version of a render graph, can only be created from a graph
/// that has been validated.
pub struct FrozenRenderGraph<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag>(
    pub(crate) RenderGraphBuilder<R, P>,
);

#[derive(Default, Debug)]
pub struct RenderGraphBuilder<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> {
    pub(crate) back_buffer_tag: Option<R>,
    pub(crate) resources: HashMap<R, RenderGraphResource<P>>,
    pub(crate) passes: HashMap<P, RenderGraphBuilderPass<R>>,
}

#[derive(Default, Debug)]
pub struct RenderGraphBuilderPass<R: RenderGraphLogicalTag> {
    // TODO: add callback to check if the pass should really be included
    pub(crate) resources: HashMap<R, RenderGraphPassResource>,
}

#[derive(Debug)]
pub struct RenderGraphResource<P: RenderGraphLogicalTag> {
    pub(crate) size: RenderGraphImageSize,
    pub(crate) format: RenderGraphImageFormat,
    pub(crate) written_in_pass: Option<P>,
    // I don't care about your opinion
    pub(crate) readden_in_pass: Vec<P>,
}

#[derive(Debug)]
pub enum RenderGraphImageSize {
    BackbufferSized,
    /// To restrict the actual dimensions of the image, **set the unused dimensions to 0, NOT 1**.
    Fixed(vk::Extent3D),
}

// TODO: Add more formats
pub enum RenderGraphImageFormat {
    BackbufferFormat,
    ColorRgba8Unorm,
    ColorRgba16Sfloat,
    DepthStencilOptimal,
}

#[derive(Debug)]
pub struct RenderGraphPassResource {
    pub(crate) bind_point: RenderGraphPassResourceBindPoint,
    pub(crate) used_in: vk::PipelineStageFlags2KHR,
    pub(crate) persistent: bool,
}

#[derive(Debug)]
pub enum RenderGraphPassResourceBindPoint {
    ColorAttachment,
    DepthAttachment,
    Sampler,
}

impl RenderGraphPassResourceBindPoint {
    #[inline]
    pub(crate) fn is_read_only(&self) -> bool {
        match self {
            Self::ColorAttachment | Self::DepthAttachment => false,
            _ => true,
        }
    }
}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> RenderGraphBuilder<R, P> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_resource(
        &mut self,
        tag: R,
        size: RenderGraphImageSize,
        format: RenderGraphImageFormat,
    ) -> R {
        // TODO: warn if already present
        self.resources.insert(
            tag,
            RenderGraphResource {
                size,
                format,
                written_in_pass: None,
                readden_in_pass: Vec::new(),
            },
        );
        tag
    }

    pub fn new_pass(&mut self, tag: P) -> &mut RenderGraphBuilderPass<R> {
        // TODO: warn if already present
        self.passes.insert(tag, Default::default());
        self.passes.get_mut(tag).unwrap()
    }

    pub fn set_back_buffer(&mut self, tag: P) -> &mut RenderGraphBuilderPass<R> {
        self.back_buffer_tag = Some(tag);
        self
    }

    pub(crate) fn get_back_buffer(&self) -> R {
        self.back_buffer_tag.unwrap()
    }
}

impl<R: RenderGraphLogicalTag> RenderGraphBuilderPass<R> {
    pub fn uses(
        &mut self,
        tag: R,
        bind_point: RenderGraphPassResourceBindPoint,
        persistent: bool,
    ) -> &mut Self {
        let used_in = match bind_point {
            RenderGraphPassResourceBindPoint::ColorAttachment => {
                vk::PipelineStageFlags2KHR::COLOR_ATTACHMENT_OUTPUT
            }
            RenderGraphPassResourceBindPoint::DepthAttachment => {
                vk::PipelineStageFlags2KHR::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags2KHR::LATE_FRAGMENT_TESTS
            }
            RenderGraphPassResourceBindPoint::Sampler => {
                vk::PipelineStageFlags2KHR::FRAGMENT_SHADER
            }
        };

        self.resources.insert(
            tag,
            RenderGraphPassResource {
                bind_point,
                used_in,
                persistent,
            },
        );
        self
    }
}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> RenderGraphBuilder<R, P> {
    /// Finalize the graph by filling in the gaps and see if everything makes sense.
    /// Also tries to be descriptive to what went wrong (if it goes wrong).
    pub(crate) fn finalize_and_validate(mut self) -> Result<FrozenRenderGraph<R, P>> {
        use log::{error, warn};

        // Check if the back buffer exist
        if let None = &self.back_buffer_tag {
            error!("The back buffer is missing !");
            return Err(VkTracerError::InvalidRenderGraph(
                GraphValidationError::NoBackBuffer,
            ));
        }

        // Check for obvious problems with the back buffer
        if self.resources[self.get_back_buffer()].format == RenderGraphImageFormat::BackbufferFormat
        {
            error!("The back buffer can't be in the same format that the back buffer.");
            return Err(VkTracerError::InvalidRenderGraph(
                GraphValidationError::InvalidBackBuffer,
            ));
        }

        // Let every resource know where it will be written to and read from
        // Useful for later on
        for (pass_tag, pass) in self.passes.iter() {
            for (res_tag, res) in pass.resources.iter() {
                match res.bind_point {
                    // Bind points that write and maybe read
                    // There can only be one per logical resource, otherwise we can't tell which to
                    // schedule first
                    RenderGraphPassResourceBindPoint::ColorAttachment
                    | RenderGraphPassResourceBindPoint::DepthAttachment => {
                        if let Some(previously_written_in) = self
                            .resources
                            .get_mut(res_tag)
                            .ok_or(GraphValidationError::TagNotRegistered)?
                            .written_in_pass
                            .replace(pass_tag)
                        {
                            error!(
                                "Resource already written in pass {} !",
                                previously_written_in
                            );
                            return Err(VkTracerError::InvalidRenderGraph(
                                GraphValidationError::LogicalResourceWrittenMoreThanOnce,
                            ));
                        }
                    }
                    // Bind points that only read
                    RenderGraphPassResourceBindPoint::Sampler => {
                        self.resources
                            .get_mut(res_tag)
                            .unwrap()
                            .readden_in_pass
                            .push(pass_tag);
                    }
                }
            }
        }

        Ok(FrozenRenderGraph(self))
    }
}
