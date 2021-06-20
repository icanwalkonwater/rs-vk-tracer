use crate::{
    errors::{HandleType, Result},
    SwapchainHandle, VkTracerApp,
};
use ash::vk;

#[derive(Copy, Clone)]
pub struct ImageViewFatHandle {
    pub(crate) handle: vk::Image,
    pub(crate) view: vk::ImageView,
    pub(crate) format: vk::Format,
    pub(crate) extent: vk::Extent2D,
}

impl VkTracerApp {
    pub fn get_images_from_swapchain(
        &self,
        swapchain: SwapchainHandle,
    ) -> Result<Vec<ImageViewFatHandle>> {
        let swapchain = storage_access!(self.swapchain_storage, swapchain, HandleType::Swapchain);

        Ok(swapchain
            .images
            .iter()
            .copied()
            .zip(swapchain.image_views.iter().copied())
            .map(|(handle, view)| ImageViewFatHandle {
                handle,
                view,
                format: swapchain.create_info.image_format,
                extent: swapchain.create_info.image_extent,
            })
            .collect())
    }
}