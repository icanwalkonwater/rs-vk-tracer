//! # Allocation
//! Extension of the [VtDevice] that allows the allocation of various buffers.
//! These methods are abstractions on top of VMA.

use crate::{
    buffers::{VtBufferAndStaging, VtBufferData, VtCpuBuffer, VtGpuBuffer},
    device::VtDevice,
    errors::Result,
};
use ash::vk;

pub type DeviceSize = vk::DeviceSize;
pub type BufferUsage = vk::BufferUsageFlags;

#[derive(Default)]
pub struct BufferDescription {
    pub size: DeviceSize,
    pub usage: BufferUsage,
}

impl VtDevice {
    pub fn create_buffer<D>(&self, desc: &BufferDescription) -> Result<VtGpuBuffer<D>> {
        let (buffer, allocation, info) = self.vma.create_buffer(
            &vk::BufferCreateInfo::builder()
                .size(desc.size)
                .usage(desc.usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            &vk_mem::AllocationCreateInfo {
                usage: vk_mem::MemoryUsage::GpuOnly,
                ..Default::default()
            },
        )?;

        Ok(VtGpuBuffer(VtBufferData {
            vma: &self.vma,
            buffer,
            allocation,
            info,
            _phantom: Default::default(),
        }))
    }

    pub fn create_buffer_with_staging<D: Copy>(
        &self,
        desc: &BufferDescription,
    ) -> Result<VtBufferAndStaging<D>> {
        let dst = self.create_buffer(desc)?;
        let staging = self.create_staging_buffer_for(&dst)?;

        Ok(VtBufferAndStaging {
            device: self,
            staging,
            dst,
        })
    }

    #[inline]
    pub fn create_staging_buffer_for<D>(&self, buffer: &VtGpuBuffer<D>) -> Result<VtCpuBuffer<D>> {
        self.create_staging_buffer(buffer.0.info.get_size() as DeviceSize)
    }

    pub fn create_staging_buffer<D>(&self, size: DeviceSize) -> Result<VtCpuBuffer<D>> {
        let (buffer, allocation, info) = self.vma.create_buffer(
            &vk::BufferCreateInfo::builder()
                .size(size)
                .usage(BufferUsage::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            &vk_mem::AllocationCreateInfo {
                usage: vk_mem::MemoryUsage::CpuOnly,
                flags: vk_mem::AllocationCreateFlags::MAPPED,
                ..Default::default()
            },
        )?;

        Ok(VtCpuBuffer(VtBufferData {
            vma: &self.vma,
            buffer,
            allocation,
            info,
            _phantom: Default::default(),
        }))
    }
}
