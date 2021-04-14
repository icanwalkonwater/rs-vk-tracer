use crate::setup::debug_utils::DebugUtils;
use crate::adapter::Adapter;
use std::collections::HashMap;
use crate::command_recorder::QueueType;
use ash::vk;
use slotmap::{SlotMap, new_key_type};
use crate::new::mesh::Mesh;
use crate::present::surface::Surface;
use crate::new::swapchain::Swapchain;
use crate::new::render_plan::RenderPlan;
use crate::new::render_target::RenderTarget;

mod app_builder;
mod mesh;
mod swapchain;
mod mem;
mod render_plan;
mod render_target;

pub const VULKAN_VERSION: u32 = ash::vk::make_version(1, 2, 0);
pub const VULKAN_VERSION_STR: &str = "1.2.0";

new_key_type! {
    pub struct MeshHandle;
    pub struct SwapchainHandle;
    pub struct RenderPlanHandle;
    pub struct RenderTargetHandle;
}

pub struct VkTracerApp {
    pub(crate) entry: ash::Entry,
    pub(crate) instance: ash::Instance,
    pub(crate) debug_utils: Option<DebugUtils>,
    pub(crate) surface: Option<Surface>,
    pub(crate) adapter: Adapter,
    pub(crate) device: ash::Device,
    pub(crate) vma: vk_mem::Allocator,
    pub(crate) command_pools: HashMap<QueueType, (vk::Queue, vk::CommandPool)>,

    pub(crate) mesh_storage: SlotMap<MeshHandle, Mesh>,
    pub(crate) swapchain_storage: SlotMap<SwapchainHandle, Swapchain>,
    pub(crate) render_plan_storage: SlotMap<RenderPlanHandle, RenderPlan>,
    pub(crate) render_target_storage: SlotMap<RenderTargetHandle, RenderTarget>,
}

pub mod errors {
    use thiserror::Error;

    pub type Result<T> = std::result::Result<T, VkTracerError>;

    #[derive(Error, Debug)]
    pub enum VkTracerError {
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
        #[error("No surface available")]
        NoSurfaceAvailable,
        #[error("Invalid {0:?} handle")]
        InvalidHandle(HandleType),
    }

    #[derive(Debug)]
    pub enum HandleType {
        Mesh, Swapchain, RenderPlan, RenderTarget
    }
}

pub mod prelude {
    pub use super::VkTracerApp;
    pub use super::app_builder::VkTracerExtensions;
    pub use super::mesh::{MeshVertex, MeshIndex, VertexXyzUv};
    pub use super::render_plan::SubpassBuilder;
}