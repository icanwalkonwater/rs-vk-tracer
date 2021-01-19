use std::borrow::Cow;

mod render_creator;
mod renderers;
mod utils;
mod extensions;
mod debug_utils;
mod adapter;
mod physical_device_selection;
mod surface;

pub const VULKAN_VERSION: u32 = ash::vk::make_version(1, 2, 0);
pub const VULKAN_VERSION_STR: &str = "1.2.0";

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
        LoadingError(#[from] ash::LoadingError)
    }
}
