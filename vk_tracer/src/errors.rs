use thiserror::Error;

pub type Result<T> = std::result::Result<T, VtError>;

#[derive(Debug, Error)]
pub enum VtError {
    #[error("Vulkan error")]
    Vulkan(#[from] ash::vk::Result),
    #[error("Failed to load vulkan")]
    VulkanLoading(#[from] ash::LoadingError),
    #[error("Instance error")]
    Instance(#[from] ash::InstanceError),
    #[error("No suitable adapter found")]
    NoSuitableAdapter,
    #[error("VMA error")]
    Vma(#[from] vk_mem::Error),
}
