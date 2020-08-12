use crate::errors::Result;
use std::{ffi::CStr, os::raw::c_char};

pub fn vk_required_device_extensions() -> Vec<&'static CStr> {
    vec![
        ash::extensions::khr::Swapchain::name(),
        #[cfg(feature = "raytracing-nv")]
        ash::extensions::nv::RayTracing::name(),
    ]
}

fn vk_required_instance_extensions() -> Vec<*const c_char> {
    vec![
        #[cfg(feature = "validation-layers")]
        ash::extensions::ext::DebugUtils::name().as_ptr(),
    ]
}

pub fn vk_required_instance_extensions_with_surface(
    window: &winit::window::Window,
) -> Result<Vec<*const c_char>> {
    let mut extensions = vk_required_instance_extensions();
    extensions.extend(
        ash_window::enumerate_required_extensions(window)?
            .iter()
            .map(|ext| ext.as_ptr()),
    );

    Ok(extensions)
}
