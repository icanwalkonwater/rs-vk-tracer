mod adapter;
mod allocator;
mod buffers;
mod command_recorder;
mod mesh;
mod mesh_storage;
pub mod new;
mod present;
mod raytracing;
pub mod renderer_creator;
mod renderers;
mod setup;
mod utils;
pub use utils::dump_vma_stats;

pub const VULKAN_VERSION: u32 = ash::vk::make_version(1, 2, 0);
pub const VULKAN_VERSION_STR: &str = "1.2.0";

#[derive(Debug)]
pub struct AppInfo {
    pub version: (u32, u32, u32),
    pub name: &'static str,
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
        #[error("Instance error")]
        InstanceError(#[from] ash::InstanceError),
        #[error("VMA Error")]
        VmaError(#[from] vk_mem::Error),
        #[error("IO Error")]
        IoError(#[from] std::io::Error),
        #[error("Renderer creator error")]
        RendererCreatorError(#[from] RendererCreatorError),
        #[error("No suitable adaptor error")]
        NoSuitableAdapterError,
    }

    #[derive(Debug, Error)]
    pub enum RendererCreatorError {
        #[error("Missing app info")]
        MissingAppInfo,
        #[error("If you provide an adapter, you must provide a device and vice-versa")]
        AdapterDeviceRequired,
    }
}

pub mod prelude {
    pub use crate::{
        command_recorder::QueueType, mesh::*, renderer_creator::*,
        setup::renderer_creator_builder::*, AppInfo,
    };
}
