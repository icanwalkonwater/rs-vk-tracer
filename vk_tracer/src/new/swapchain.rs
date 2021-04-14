use crate::{
    new::adapter::{Adapter, AdapterRequirements},
    new::{
        errors::{Result, VkTracerError},
        SwapchainHandle, VkTracerApp,
    },
    new::surface::Surface,
};
use ash::{version::DeviceV1_0, vk};
use crate::new::errors::HandleType;

impl VkTracerApp {
    pub fn create_swapchain_for_window(
        &mut self,
        window_size: (u32, u32),
    ) -> Result<SwapchainHandle> {
        let surface = self
            .surface
            .as_ref()
            .ok_or(VkTracerError::NoSurfaceAvailable)?;
        let swapchain = Swapchain::new(
            &self.instance,
            surface,
            &self.adapter,
            &self.device,
            window_size,
        )?;
        Ok(self.swapchain_storage.insert(swapchain))
    }

    pub fn get_next_swapchain_render_target_index(&self, swapchain: SwapchainHandle) -> Result<(u32, bool)> {
        let swapchain = self.swapchain_storage.get(swapchain).ok_or(VkTracerError::InvalidHandle(HandleType::Swapchain))?;
        swapchain.acquire_next_image()
    }
}

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

pub(crate) struct Swapchain {
    pub(crate) loader: ash::extensions::khr::Swapchain,
    pub(crate) create_info: vk::SwapchainCreateInfoKHR,
    pub(crate) handle: vk::SwapchainKHR,
    pub(crate) images: Vec<vk::Image>,
    pub(crate) image_views: Vec<vk::ImageView>,
    pub(crate) extent: vk::Extent2D,

    pub(crate) image_available_semaphore: vk::Semaphore,
}

impl Swapchain {
    pub(crate) fn new(
        instance: &ash::Instance,
        surface: &Surface,
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

        let extent = Self::create_clamped_extent(window_size, capabilities);

        let create_info = vk::SwapchainCreateInfoKHR::builder()
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
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());

        let swapchain = unsafe { loader.create_swapchain(&create_info, None)? };

        let images = unsafe { loader.get_swapchain_images(swapchain)? };
        let image_views = Self::create_image_views(device, &surface, &images)?;

        let image_available_semaphore = unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };

        Ok(Self {
            loader,
            create_info: create_info.build(),
            handle: swapchain,
            images,
            image_views,
            extent,
            image_available_semaphore,
        })
    }

    pub(crate) fn recreate(
        &mut self,
        device: &ash::Device,
        adapter: &Adapter,
        surface: &Surface,
        window_size: (u32, u32),
    ) -> Result<()> {
        unsafe {
            // Destroy previous swapchain images
            for image_view in self.image_views.iter().copied() {
                device.destroy_image_view(image_view, None);
            }
        }

        self.extent = Self::create_clamped_extent(
            window_size,
            adapter
                .info
                .physical_device_info
                .surface_capabilities
                .as_ref()
                .unwrap(),
        );
        self.create_info.image_extent = self.extent;
        self.create_info.old_swapchain = self.handle;

        self.handle = unsafe { self.loader.create_swapchain(&self.create_info, None)? };

        self.images = unsafe { self.loader.get_swapchain_images(self.handle)? };
        self.image_views = Self::create_image_views(device, surface, &self.images)?;

        Ok(())
    }

    pub(crate) fn acquire_next_image(&self) -> Result<(u32, bool)> {
        unsafe {
            Ok(self.loader.acquire_next_image(
                self.handle,
                u64::MAX,
                self.image_available_semaphore,
                vk::Fence::null(),
            )?)
        }
    }

    fn create_clamped_extent(
        window_size: (u32, u32),
        capabilities: &vk::SurfaceCapabilitiesKHR,
    ) -> vk::Extent2D {
        vk::Extent2D::builder()
            .width(window_size.0.clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            ))
            .height(window_size.1.clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            ))
            .build()
    }

    fn create_image_views(
        device: &ash::Device,
        surface: &Surface,
        images: &[vk::Image],
    ) -> Result<Vec<vk::ImageView>> {
        Ok(images
            .iter()
            .copied()
            .map(|image| unsafe {
                device.create_image_view(
                    &vk::ImageViewCreateInfo::builder()
                        .image(image)
                        .format(surface.format)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .components(
                            vk::ComponentMapping::builder()
                                .r(vk::ComponentSwizzle::IDENTITY)
                                .g(vk::ComponentSwizzle::IDENTITY)
                                .b(vk::ComponentSwizzle::IDENTITY)
                                .a(vk::ComponentSwizzle::IDENTITY)
                                .build(),
                        )
                        .subresource_range(
                            vk::ImageSubresourceRange::builder()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(0)
                                .layer_count(1)
                                .build(),
                        ),
                    None,
                )
            })
            .collect::<ash::prelude::VkResult<Vec<_>>>()?)
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        /*unsafe {
            for image_view in self.image_views.iter().copied() {
                self.device.destroy_image_view(image_view, None);
            }

            self.loader.destroy_swapchain(self.handle, None);
        }*/
    }
}
