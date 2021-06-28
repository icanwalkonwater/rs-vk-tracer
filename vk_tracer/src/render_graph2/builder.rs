use ash::vk;
use std::hash::Hash;
use std::fmt::{Debug, Display};
use std::collections::HashMap;

pub trait RenderGraphLogicalTag: Copy + Eq + Hash + Debug + Display + 'static {}

impl<T: Copy + Eq + Hash + Debug + Display + 'static> RenderGraphLogicalTag for T {}

#[derive(Default, Debug)]
pub struct RenderGraphBuilder<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> {
    pub(crate) resources: HashMap<R, RenderGraphResource>,
    pub(crate) passes: HashMap<P, RenderGraphBuilderPass<R>>,
}

#[derive(Default, Debug)]
pub struct RenderGraphBuilderPass<R: RenderGraphLogicalTag> {
    // TODO: add callback to check if the pass should really be included
    pub(crate) resources: HashMap<R, RenderGraphPassResource>,
}

#[derive(Debug)]
pub struct RenderGraphResource {
    pub(crate) size: RenderGraphImageSize,
    // TODO: use abstract format like `DepthOptimal` or `SameAsSwapchain`
    pub(crate) format: vk::Format,
}

#[derive(Debug)]
pub enum RenderGraphImageSize {
    SwapchainSized,
    /// To restrict the actual dimensions of the image, **set the unused dimensions to 0, NOT 1**.
    Fixed(vk::Extent3D),
}

#[derive(Debug)]
pub struct RenderGraphPassResource {
    pub(crate) bind_point: RenderGraphPassResourceBindPoint,
    pub(crate) used_in: vk::PipelineStageFlags2KHR,
}

#[derive(Debug)]
pub enum RenderGraphPassResourceBindPoint {
    ColorAttachment,
    DepthAttachment,
    Sampler,
}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> RenderGraphBuilder<R, P> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_resource(&mut self, tag: R, resource: RenderGraphResource) -> R {
        // TODO: warn if already present
        self.resources.insert(tag, resource);
        tag
    }

    pub fn new_pass(&mut self, tag: P) -> &mut RenderGraphBuilderPass<R> {
        // TODO: warn if already present
        self.passes.insert(tag, Default::default());
        self.passes.get_mut(tag).unwrap()
    }
}

impl<R: RenderGraphLogicalTag> RenderGraphBuilderPass<R> {
    pub fn uses(&mut self, tag: R, bind_point: RenderGraphPassResourceBindPoint, used_in: vk::PipelineStageFlags) -> &mut Self {
        self.resources.insert(tag, RenderGraphPassResource {
            bind_point,
            used_in,
        });
        self
    }
}
