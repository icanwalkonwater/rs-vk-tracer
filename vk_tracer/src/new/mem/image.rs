use ash::vk;
use crate::new::{VkTracerApp, SwapchainHandle};
use crate::new::errors::{Result, VkTracerError, HandleType};

pub struct ImageViewFatHandle {
    pub handle: vk::Image,
    pub view: vk::ImageView,
    pub format: vk::Format,
}

impl VkTracerApp {
    pub fn get_images_from_swapchain(&self, swapchain: SwapchainHandle) -> Result<Vec<ImageViewFatHandle>> {
        let swapchain = self.swapchain_storage.get(swapchain).ok_or(VkTracerError::InvalidHandle(HandleType::Swapchain))?;

        Ok(swapchain.images.iter()
            .copied()
            .zip(swapchain.image_views.iter().copied())
            .map(|(handle, view)| ImageViewFatHandle { handle, view, format: swapchain.create_info.image_format })
            .collect())
    }
}