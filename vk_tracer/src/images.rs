use crate::buffers::VtBuffer;
use crate::command_recorder::{CopyBufferDescription, CopyImageDescription, VtTransferCommands};
use crate::device::VtDevice;
use crate::errors::Result;
use ash::vk;
use log::error;

pub type Format = vk::Format;
pub type ColorSpace = vk::ColorSpaceKHR;
pub type Extent3D = vk::Extent3D;
pub type Extent2D = vk::Extent2D;
pub type SampleCount = vk::SampleCountFlags;
pub type Tiling = vk::ImageTiling;
pub type ImageUsage = vk::ImageUsageFlags;
pub type ImageLayout = vk::ImageLayout;

/// Owned vulkan image.
pub struct VtImage<'a> {
    pub(crate) device: &'a VtDevice,
    pub(crate) vma: &'a vk_mem::Allocator,
    pub(crate) image: vk::Image,
    pub(crate) allocation: vk_mem::Allocation,
    pub(crate) info: vk_mem::AllocationInfo,
    pub(crate) extent: Extent3D,
}

impl<'ptr> VtImage<'ptr> {
    pub fn store<'data: 'ptr, D: 'data, B: Into<VtBuffer<'ptr, 'data, D>>>(
        &mut self,
        src: CopyBufferDescription<B>,
        dst: CopyImageDescription<'ptr>,
    ) -> Result<()> {
        let mut encoder = self.device.get_transient_transfer_encoder()?;
        encoder.copy_buffer_to_image(src, dst)?;
        encoder.finish()?.submit()?;

        Ok(())
    }
}

impl PartialEq for VtImage<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.image == other.image && self.info.get_offset() == other.info.get_offset()
    }
}

impl Drop for VtImage<'_> {
    fn drop(&mut self) {
        if let Err(err) = self.vma.destroy_image(self.image, &self.allocation) {
            error!("VMA Free error: {}", err);
        }
    }
}
