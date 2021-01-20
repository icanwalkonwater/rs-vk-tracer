use crate::{
    adapter::Adapter,
    debug_utils::VtDebugUtils,
    errors::Result,
    extensions::{required_instance_extensions, required_instance_extensions_with_surface},
    renderer_creator_builder::RendererCreatorBuilder,
    utils::str_to_cstr,
    AppInfo, VULKAN_VERSION,
};
use ash::{version::EntryV1_0, vk};
use raw_window_handle::HasRawWindowHandle;
use std::borrow::Cow;

pub struct RendererCreator {
    instance: ash::Instance,
    adapter: Adapter,
    device: ash::Device,
    debug_utils: VtDebugUtils,
}

impl RendererCreator {
    pub fn builder() -> RendererCreatorBuilder {
        RendererCreatorBuilder::new()
    }
}
