use crate::adapter::{Adapter, AdapterRequirements};
use crate::errors::Result;
use crate::renderer_creator::RendererCreator;
use crate::surface::Surface;
use crate::utils::clamp;
use ash::vk;

/// Choose the present mode, will fallback to FIFO if the requirements can't be met.
pub(crate) fn choose_swapchain_present_mode(
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

pub struct Swapchain {
    loader: ash::extensions::khr::Swapchain,
    pub(crate) surface: Surface,
    pub(crate) swapchain: vk::SwapchainKHR,
    pub(crate) extent: vk::Extent2D,
}

impl Swapchain {
    pub(crate) fn new(
        instance: &ash::Instance,
        surface: Surface,
        adapter: &Adapter,
        device: &ash::Device,
        window_size: (u32, u32),
    ) -> Result<Self> {
        let capabilities = adapter
            .info
            .physical_device_info
            .surface_capabilities
            .as_ref()
            .unwrap();
        let loader = ash::extensions::khr::Swapchain::new(instance, device);

        let mut image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count {
            image_count = capabilities.max_image_count
        }

        let extent = vk::Extent2D::builder()
            .width(clamp(
                window_size.0,
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            ))
            .height(clamp(
                window_size.1,
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            ))
            .build();

        let swapchain = unsafe {
            loader.create_swapchain(
                &vk::SwapchainCreateInfoKHR::builder()
                    .surface(surface.handle)
                    .min_image_count(image_count)
                    .image_format(surface.format)
                    .image_color_space(surface.color_space)
                    .image_extent(extent)
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .queue_family_indices(&[])
                    .pre_transform(capabilities.current_transform)
                    .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                    .present_mode(choose_swapchain_present_mode(
                        adapter
                            .info
                            .physical_device_info
                            .surface_present_modes
                            .as_ref()
                            .unwrap(),
                        &adapter.requirements,
                    ))
                    .clipped(true),
                None,
            )?
        };

        Ok(Self {
            loader,
            surface,
            swapchain,
            extent,
        })
    }
}
