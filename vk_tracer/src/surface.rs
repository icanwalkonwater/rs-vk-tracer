use ash::vk;
use raw_window_handle::HasRawWindowHandle;

use crate::{
    adapter::{Adapter, AdapterRequirements},
    errors::Result,
    instance::VtInstance,
    utils::clamp,
};

/// Choose the surface format.
pub(crate) fn choose_surface_format(
    formats: &[vk::SurfaceFormatKHR],
    format_properties: &[vk::FormatProperties],
    requirements: &AdapterRequirements,
) -> Option<vk::SurfaceFormatKHR> {
    formats
        .iter()
        .zip(format_properties.iter())
        .find(|(format, _)| {
            requirements.surface_formats.contains(&format.format)
                && requirements
                    .surface_color_spaces
                    .contains(&format.color_space)
            // TODO: Reenable for ray tracing
            // && properties
            //     .optimal_tiling_features
            //     .contains(vk::FormatFeatureFlags::STORAGE_IMAGE)
        })
        .map(|(format, _)| format)
        .copied()
}

/// Choose the present mode, will fallback to FIFO if the requirements can't be met.
pub(crate) fn choose_surface_present_mode(
    present_modes: &[vk::PresentModeKHR],
    requirements: &AdapterRequirements,
) -> vk::PresentModeKHR {
    for mode in present_modes {
        if requirements.present_modes.contains(mode) {
            return *mode;
        }
    }

    vk::PresentModeKHR::FIFO
}

#[derive(Clone)]
pub struct Surface {
    pub(crate) loader: ash::extensions::khr::Surface,
    pub(crate) handle: vk::SurfaceKHR,
    format: vk::Format,
    color_space: vk::ColorSpaceKHR,
    extent: vk::Extent2D,
}

impl Surface {
    pub fn create(
        instance: &VtInstance,
        window: &impl HasRawWindowHandle,
        window_size: (u32, u32),
    ) -> Result<Self> {
        let loader = ash::extensions::khr::Surface::new(&instance.entry, &instance.instance);
        let handle = unsafe {
            ash_window::create_surface(&instance.entry, &instance.instance, window, None)?
        };

        Ok(Self {
            loader,
            handle,
            format: vk::Format::UNDEFINED,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            extent: vk::Extent2D::builder()
                .width(window_size.0)
                .height(window_size.1)
                .build(),
        })
    }

    pub fn complete(&mut self, adapter: &VtAdapter) {
        let format = choose_surface_format(
            &adapter.1.info.surface_formats.as_ref().unwrap(),
            &adapter.1.info.surface_format_properties.as_ref().unwrap(),
            &adapter.2,
        )
        .unwrap();

        self.format = format.format;
        self.color_space = format.color_space;

        let min_extent = adapter
            .1
            .info
            .surface_capabilities
            .unwrap()
            .min_image_extent;
        let max_extent = adapter
            .1
            .info
            .surface_capabilities
            .unwrap()
            .max_image_extent;

        let corrected_width = clamp(self.extent.width, min_extent.width, max_extent.width);
        let corrected_height = clamp(self.extent.height, min_extent.height, max_extent.height);

        self.extent = vk::Extent2D::builder()
            .width(corrected_width)
            .height(corrected_height)
            .build();
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.loader.destroy_surface(self.handle, None);
        }
    }
}
