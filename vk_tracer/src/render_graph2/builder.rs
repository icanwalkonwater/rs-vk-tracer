use super::GraphValidationError;
use crate::errors::{Result, VkTracerError};
use ash::vk;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use itertools::Itertools;

pub trait RenderGraphLogicalTag: Copy + Eq + Hash + Debug + Display + 'static {}

impl<T: Copy + Eq + Hash + Debug + Display + 'static> RenderGraphLogicalTag for T {}

/// Immutable version of a render graph, can only be created from a graph
/// that has been validated.
pub struct FrozenRenderGraph<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag>(
    pub(crate) RenderGraphBuilder<R, P>,
);

#[derive(Debug)]
pub struct RenderGraphBuilder<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> {
    pub(crate) back_buffer_tag: Option<R>,
    pub(crate) resources: HashMap<R, RenderGraphResource<P>>,
    pub(crate) passes: HashMap<P, RenderGraphBuilderPass<R>>,
}

#[derive(Debug)]
pub struct RenderGraphBuilderPass<R: RenderGraphLogicalTag> {
    // TODO: add callback to check if the pass should really be included
    pub(crate) resources: HashMap<R, RenderGraphPassResource>,
    pub(crate) read_modify_write_whitelist: Vec<(R, R)>,
}

#[derive(Debug)]
pub struct RenderGraphResource<P: RenderGraphLogicalTag> {
    pub(crate) size: RenderGraphImageSize,
    pub(crate) format: RenderGraphImageFormat,
    pub(crate) written_in_pass: Option<P>,
    // I don't care about your opinion
    pub(crate) readden_in_pass: Vec<P>,
    pub(crate) persistence: RenderGraphResourcePersistence,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum RenderGraphImageSize {
    BackbufferSized,
    /// To restrict the actual dimensions of the image, **set the unused dimensions to 0, NOT 1**.
    Fixed(vk::Extent3D),
}

// TODO: Add more formats
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum RenderGraphImageFormat {
    BackbufferFormat,
    ColorRgba8Unorm,
    ColorRgba16Sfloat,
    DepthStencilOptimal,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum RenderGraphResourcePersistence {
    Transient,
    PreserveInput,
    PreserveOutput,
    ClearInput,
    ClearInputPreserveOutput,
    PreserveAll,
}

#[derive(Debug)]
pub struct RenderGraphPassResource {
    pub(crate) bind_point: RenderGraphPassResourceBindPoint,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum RenderGraphPassResourceBindPoint {
    ColorAttachment,
    InputAttachment,
    DepthAttachment,
    Sampler,
    // Not for public use
    GeneralInputAndColorAttachment,
}

impl RenderGraphPassResourceBindPoint {
    #[inline]
    pub(crate) fn optimal_layout(&self) -> vk::ImageLayout {
        match self {
            Self::ColorAttachment => vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            Self::DepthAttachment => vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            Self::InputAttachment | Self::Sampler => vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            Self::GeneralInputAndColorAttachment => vk::ImageLayout::GENERAL,
        }
    }

    #[inline]
    pub(crate) fn stages(&self) -> vk::PipelineStageFlags2KHR {
        match self {
            Self::ColorAttachment => vk::PipelineStageFlags2KHR::COLOR_ATTACHMENT_OUTPUT,
            Self::DepthAttachment => {
                vk::PipelineStageFlags2KHR::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags2KHR::LATE_FRAGMENT_TESTS
            }
            Self::InputAttachment | Self::Sampler => vk::PipelineStageFlags2KHR::FRAGMENT_SHADER,
            Self::GeneralInputAndColorAttachment => vk::PipelineStageFlags2KHR::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags2KHR::FRAGMENT_SHADER,
        }
    }

    #[inline]
    pub(crate) fn accesses(&self) -> vk::AccessFlags2KHR {
        match self {
            Self::ColorAttachment => {
                vk::AccessFlags2KHR::COLOR_ATTACHMENT_READ
                    | vk::AccessFlags2KHR::COLOR_ATTACHMENT_WRITE
            }
            Self::InputAttachment => vk::AccessFlags2KHR::INPUT_ATTACHMENT_READ,
            Self::DepthAttachment => {
                vk::AccessFlags2KHR::DEPTH_STENCIL_ATTACHMENT_READ
                    | vk::AccessFlags2KHR::DEPTH_STENCIL_ATTACHMENT_WRITE
            }
            Self::Sampler => vk::AccessFlags2KHR::SHADER_READ,
            Self::GeneralInputAndColorAttachment => vk::AccessFlags2KHR::COLOR_ATTACHMENT_READ | vk::AccessFlags2KHR::COLOR_ATTACHMENT_WRITE | vk::AccessFlags2KHR::INPUT_ATTACHMENT_READ,
        }
    }

    #[inline]
    pub(crate) fn can_write(&self) -> bool {
        match self {
            Self::ColorAttachment | Self::DepthAttachment => true,
            Self::GeneralInputAndColorAttachment => true,
            _ => false,
        }
    }

    #[inline]
    pub(crate) fn can_read(&self) -> bool {
        match self {
            Self::DepthAttachment | Self::InputAttachment | Self::Sampler => true,
            Self::GeneralInputAndColorAttachment => true,
            _ => false,
        }
    }
}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> RenderGraphBuilder<R, P> {
    pub fn new() -> Self {
        Self {
            back_buffer_tag: None,
            resources: Default::default(),
            passes: Default::default(),
        }
    }

    pub fn add_resource(
        &mut self,
        tag: R,
        size: RenderGraphImageSize,
        format: RenderGraphImageFormat,
        persistence: RenderGraphResourcePersistence,
    ) -> R {
        // TODO: warn if already present
        self.resources.insert(
            tag,
            RenderGraphResource {
                size,
                format,
                written_in_pass: None,
                readden_in_pass: Vec::new(),
                persistence,
            },
        );
        tag
    }

    pub fn new_pass(&mut self, tag: P) -> &mut RenderGraphBuilderPass<R> {
        // TODO: warn if already present
        self.passes.insert(
            tag,
            RenderGraphBuilderPass {
                resources: Default::default(),
                read_modify_write_whitelist: Default::default(),
            },
        );
        self.passes.get_mut(&tag).unwrap()
    }

    pub fn set_back_buffer(&mut self, tag: R) {
        self.back_buffer_tag = Some(tag);
    }

    pub(crate) fn get_back_buffer(&self) -> R {
        self.back_buffer_tag.unwrap()
    }
}

impl<R: RenderGraphLogicalTag> RenderGraphBuilderPass<R> {
    pub fn uses(&mut self, tag: R, bind_point: RenderGraphPassResourceBindPoint) -> &mut Self {
        if bind_point == RenderGraphPassResourceBindPoint::GeneralInputAndColorAttachment {
            panic!("These enums are reserved for private use, instead use #uses_input_to_color()");
        }

        self.resources
            .insert(tag, RenderGraphPassResource { bind_point });
        self
    }

    pub fn allow_read_modify_write(&mut self, from: R, to: R) -> &mut Self {
        self.read_modify_write_whitelist.push((from, to));
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

        // Warn orphan resources
        // TODO: warn about orphan passes
        self.resources
            .iter()
            .filter(|(_, res)| res.written_in_pass.is_none() && res.readden_in_pass.is_empty())
            .for_each(|(tag, _)| {
                warn!("Resource {:?} is an orphan !", tag);
            });

        // Let every resource know where it will be written to and read from
        // Useful for later on
        for (&pass_tag, pass) in self.passes.iter() {
            for (res_tag, res) in pass.resources.iter() {

                // Bind points that write and maybe read
                // There can only be one per logical resource, otherwise we can't tell which to
                // schedule first
                if res.bind_point.can_write() {
                    if let Some(previously_written_in) = self
                        .resources
                        .get_mut(res_tag)
                        .ok_or(VkTracerError::InvalidRenderGraph(
                            GraphValidationError::TagNotRegistered,
                        ))?
                        .written_in_pass
                        .replace(pass_tag)
                    {
                        error!(
                            "Resource already written in pass {:?} !",
                            previously_written_in
                        );
                        return Err(VkTracerError::InvalidRenderGraph(
                            GraphValidationError::LogicalResourceWrittenMoreThanOnce,
                        ));
                    }
                }

                if res.bind_point.can_read() {
                    self.resources
                        .get_mut(res_tag)
                        .unwrap()
                        .readden_in_pass
                        .push(pass_tag);
                }
            }
        }

        if self.resources[&self.get_back_buffer()]
            .written_in_pass
            .is_none()
        {
            error!("The back buffer is never written to !");
            return Err(VkTracerError::InvalidRenderGraph(
                GraphValidationError::InvalidBackBuffer,
            ));
        }

        // Check color and input attachments are all the same size
        for (_, pass) in &self.passes {
            let _ = pass.resources.iter()
                .filter(|(_, res)| res.bind_point == RenderGraphPassResourceBindPoint::ColorAttachment || res.bind_point == RenderGraphPassResourceBindPoint::InputAttachment)
                .map(|(tag, _)| self.resources[tag].size)
                .try_fold(None, |acc, size| match acc {
                        None => Ok(Some(size)),
                        Some(last) if last == size => Ok(Some(size)),
                        _ => Err(VkTracerError::InvalidRenderGraph(GraphValidationError::ColorOrInputAttachmentDifferInSize)),
                    })?;
        }

        // Check RMW
        for (pass_tag, pass) in &self.passes {
            for (input, color) in &pass.read_modify_write_whitelist {
                if pass.resources.get(input).ok_or(VkTracerError::InvalidRenderGraph(
                    GraphValidationError::TagNotRegistered,
                ))?.bind_point != RenderGraphPassResourceBindPoint::InputAttachment {
                    error!("Read-Modify-Write error: Pass {}, attachment {} is not an input attachment !", pass_tag, input);
                    return Err(VkTracerError::InvalidRenderGraph(
                        GraphValidationError::ReadModifyWriteWrongBindPoint,
                    ));
                }

                if pass.resources.get(color).ok_or(VkTracerError::InvalidRenderGraph(
                    GraphValidationError::TagNotRegistered,
                ))?.bind_point != RenderGraphPassResourceBindPoint::ColorAttachment {
                    error!("Read-Modify-Write error: Pass {}, attachment {} is not a color attachment !", pass_tag, color);
                    return Err(VkTracerError::InvalidRenderGraph(
                        GraphValidationError::ReadModifyWriteWrongBindPoint,
                    ));
                }
            }
        }

        #[cfg(feature = "visualizer_render_graph")]
        {
            fn tag_to_graph_id<Tag: RenderGraphLogicalTag>(tag: Tag) -> String {
                let mut formatted = format!("{}", tag);
                formatted = formatted.replace(&[' ', '\n', '\t', '\r'][..], "_");
                formatted
            }

            use std::io::Write;
            let mut visualizer_nodes = std::fs::File::create(
                option_env!("VISUALIZER_GRAPH_OUT")
                    .unwrap_or("./render_graph_nodes.dot"),
            )
            .unwrap();

            writeln!(visualizer_nodes, "digraph raw_render_graph {{").unwrap();
            writeln!(visualizer_nodes, " rankdir=LR").unwrap();
            writeln!(visualizer_nodes, " labelloc=t").unwrap();
            writeln!(visualizer_nodes, " label=\"Raw Render Graph\"").unwrap();

            // Write resources
            for (res_tag, res) in self.resources.iter().map(|(t, r)| (*t, r)) {
                let res_id = tag_to_graph_id(res_tag);

                writeln!(
                    visualizer_nodes,
                    " {} [shape=oval label=\"[{}]\\nSize - {:?}\\lFormat - {:?}\\lPersistence - {:?}\\l\"]",
                    res_id, res_tag, res.size, res.format, res.persistence,
                ).unwrap();
            }

            // Write passes
            for (pass_tag, pass) in self.passes.iter().map(|(t, p)| (*t, p)) {
                let pass_id = tag_to_graph_id(pass_tag);
                writeln!(visualizer_nodes,
                " {} [shape=rectangle color=orange style=filled label=\"[{}]\\n{}\"]", pass_id, pass_tag, "Graphics").unwrap();

                // Write edges
                for (res_tag, res_local) in pass.resources.iter().map(|(t, r)| (*t, r)) {
                    use RenderGraphPassResourceBindPoint::*;

                    let res_id = tag_to_graph_id(res_tag);
                    match res_local.bind_point {
                        ColorAttachment | DepthAttachment => {
                            writeln!(visualizer_nodes," {} -> {}", pass_id, res_id).unwrap();
                        },
                        InputAttachment | Sampler => {
                            writeln!(visualizer_nodes," {} -> {}", res_id, pass_id).unwrap();
                        },
                        GeneralInputAndColorAttachment => unreachable!(),
                    }
                }
            }

            writeln!(visualizer_nodes, "}}").unwrap();
        }

        Ok(FrozenRenderGraph(self))
    }
}
