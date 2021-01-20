use std::borrow::Cow;

mod adapter;
mod debug_utils;
mod extensions;
mod physical_device_selection;
mod queue_indices;
mod raytracing;
mod renderer_creator;
mod renderer_creator_builder;
mod renderers;
mod surface;
mod utils;

pub const VULKAN_VERSION: u32 = ash::vk::make_version(1, 2, 0);
pub const VULKAN_VERSION_STR: &str = "1.2.0";

#[derive(Debug)]
pub struct AppInfo {
    pub version: (u32, u32, u32),
    pub name: Cow<'static, String>,
}

pub mod errors {
    use thiserror::Error;

    pub type Result<T> = std::result::Result<T, VtError>;

    #[derive(Debug, Error)]
    pub enum VtError {
        #[error("Vulkan error")]
        Vulkan(#[from] ash::vk::Result),
        #[error("Loading error")]
        LoadingError(#[from] ash::LoadingError),
    }
}
