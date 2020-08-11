use thiserror::Error;

pub type Result<T> = std::result::Result<T, RendererError>;

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("Vulkan error when loading entry point")]
    VulkanLoadEntry(#[from] ash::LoadingError),
    #[error("Vulkan error when creating the Instance")]
    VulkanInstanceCreation(#[from] ash::InstanceError),
    #[error("Vulkan generic error")]
    VulkanGeneric(#[from] ash::vk::Result),
    #[error("Winit window creation failure")]
    Winit(#[from] winit::error::OsError)
}
