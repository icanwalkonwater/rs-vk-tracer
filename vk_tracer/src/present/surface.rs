use ash::vk;
use raw_window_handle::HasRawWindowHandle;

use crate::{
    errors::Result,
    setup::{Adapter, AdapterRequirements},
};
use ash::extensions::khr;

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

#[derive(Clone)]
pub struct Surface {
    pub(crate) loader: ash::extensions::khr::Surface,
    pub(crate) handle: vk::SurfaceKHR,
    pub(crate) format: vk::Format,
    pub(crate) color_space: vk::ColorSpaceKHR,
    pub(crate) extent: vk::Extent2D,
}

impl Surface {
    pub fn create(
        entry: &ash::Entry,
        instance: &ash::Instance,
        window: &impl HasRawWindowHandle,
        window_size: (u32, u32),
    ) -> Result<Self> {
        let loader = khr::Surface::new(entry, instance);
        let handle = unsafe { ash_window::create_surface(entry, instance, window, None)? };

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

    pub fn complete(&mut self, adapter: &Adapter) {
        let format = choose_surface_format(
            &adapter
                .info
                .physical_device_info
                .surface_formats
                .as_ref()
                .unwrap(),
            &adapter
                .info
                .physical_device_info
                .surface_format_properties
                .as_ref()
                .unwrap(),
            &adapter.requirements,
        )
        .unwrap();

        self.format = format.format;
        self.color_space = format.color_space;

        let min_extent = adapter
            .info
            .physical_device_info
            .surface_capabilities
            .unwrap()
            .min_image_extent;
        let max_extent = adapter
            .info
            .physical_device_info
            .surface_capabilities
            .unwrap()
            .max_image_extent;

        let corrected_width = self.extent.width.clamp(min_extent.width, max_extent.width);
        let corrected_height = self
            .extent
            .height
            .clamp(min_extent.height, max_extent.height);

        self.extent = vk::Extent2D::builder()
            .width(corrected_width)
            .height(corrected_height)
            .build();
    }
}
