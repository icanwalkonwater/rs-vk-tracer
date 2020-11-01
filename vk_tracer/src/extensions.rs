use crate::{device::VtDevice, errors::Result, instance::VtInstance};
use raw_window_handle::HasRawWindowHandle;
use std::{ffi::CStr, os::raw::c_char};

impl VtInstance {
    /// Get extensions required for the instance.
    pub fn required_extensions() -> Vec<*const c_char> {
        if cfg!(feature = "ext-debug") {
            vec![ash::extensions::ext::DebugUtils::name().as_ptr()]
        } else {
            vec![]
        }
    }

    /// Get extensions required for the instance and to present to the given surface.
    pub fn required_extensions_with_surface(
        handle: &dyn HasRawWindowHandle,
    ) -> Result<Vec<*const c_char>> {
        let mut extensions = Self::required_extensions();

        extensions.extend(
            ash_window::enumerate_required_extensions(handle)?
                .iter()
                .map(|ext| ext.as_ptr()),
        );

        Ok(extensions)
    }
}

impl VtDevice {
    pub fn required_extensions() -> Vec<&'static CStr> {
        vec![ash::extensions::khr::Swapchain::name()]
    }
}
