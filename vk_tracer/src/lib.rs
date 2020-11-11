pub mod errors;
mod utils;

pub mod adapter;
pub mod allocation;
pub mod buffers;
pub mod command_recorder;
#[cfg(feature = "ext-debug")]
mod debug_utils;
pub mod descriptor_sets;
pub mod device;
pub mod extensions;
pub mod instance;
mod physical_device_selection;
pub mod surface;

pub mod raw_window_handle {
    pub use raw_window_handle::*;
}

pub mod prelude {
    use crate::{adapter, allocation, buffers, device, errors, instance, surface};

    pub use adapter::*;
    pub use allocation::*;
    pub use buffers::*;
    pub use device::*;
    pub use errors::*;
    pub use instance::*;
    pub use raw_window_handle::HasRawWindowHandle;
    pub use surface::*;
}

pub(crate) const VULKAN_VERSION: u32 = ash::vk::make_version(1, 2, 0);
pub(crate) const VULKAN_VERSION_STR: &str = "1.2.0";
