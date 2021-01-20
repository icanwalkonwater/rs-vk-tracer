use crate::{
    adapter::Adapter,
    debug_utils::VtDebugUtils,
    renderer_creator_builder::RendererCreatorBuilder,
};

pub struct RendererCreator {
    pub(crate) instance: ash::Instance,
    pub(crate) adapter: Adapter,
    pub(crate) device: ash::Device,
    pub(crate) debug_utils: Option<VtDebugUtils>,
}

impl RendererCreator {
    pub fn builder() -> RendererCreatorBuilder {
        RendererCreatorBuilder::new()
    }
}
