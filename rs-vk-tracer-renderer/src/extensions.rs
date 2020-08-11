use crate::errors::Result;
use std::os::raw::c_char;

fn vk_required_extensions_raw() -> Vec<*const c_char> {
    vec![
        ash::extensions::khr::Swapchain::name().as_ptr(),
        #[cfg(feature = "validation-layers")]
        ash::extensions::ext::DebugUtils::name().as_ptr(),
    ]
}

pub fn vk_required_extensions_with_surface_raw(
    window: &winit::window::Window,
) -> Result<Vec<*const c_char>> {
    let mut extensions = vk_required_extensions_raw();
    extensions.extend(
        ash_window::enumerate_required_extensions(window)?
            .iter()
            .map(|ext| ext.as_ptr()),
    );

    Ok(extensions)
}
