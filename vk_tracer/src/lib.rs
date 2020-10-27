pub mod errors;
mod utils;

pub mod adapter;
#[cfg(feature = "ext-debug")]
mod debug_utils;
pub mod device;
pub mod extensions;
pub mod instance;
mod physical_device_selection;
pub mod surface;
pub mod allocation;
pub mod command_recorder;

pub mod raw_window_handle {
    pub use raw_window_handle::*;
}

pub mod prelude {
    use crate::{adapter, device, errors, instance, surface, allocation};

    pub use adapter::*;
    pub use device::*;
    pub use errors::*;
    pub use instance::*;
    pub use raw_window_handle::HasRawWindowHandle;
    pub use surface::*;
    pub use allocation::*;
}

pub(crate) const VULKAN_VERSION: u32 = ash::vk::make_version(1, 2, 0);
pub(crate) const VULKAN_VERSION_STR: &str = "1.2.0";
