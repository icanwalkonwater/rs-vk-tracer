use crate::errors::Result;
use raw_window_handle::HasRawWindowHandle;
use std::{ffi::CStr, os::raw::c_char};

/// Get extensions required for the instance.
pub fn required_instance_extensions(with_debug_utils: bool) -> Vec<*const c_char> {
    if with_debug_utils {
        vec![ash::extensions::ext::DebugUtils::name().as_ptr()]
    } else {
        vec![]
    }
}

/// Get extensions required for the instance and to present to the given surface.
pub fn required_instance_extensions_with_surface(
    with_debug_utils: bool,
    handle: &dyn HasRawWindowHandle,
) -> Result<Vec<*const c_char>> {
    let mut extensions = required_instance_extensions(with_debug_utils);

    extensions.extend(
        ash_window::enumerate_required_extensions(handle)
            .expect("That's not supposed to happen, damn")
            .iter()
            .map(|ext| ext.as_ptr()),
    );

    Ok(extensions)
}

pub fn required_device_extensions() -> Vec<&'static CStr> {
    use ash::extensions::khr;
    // VK_KHR_create_renderpass2 promoted to vulkan 1.2
    vec![khr::Swapchain::name()]
}
