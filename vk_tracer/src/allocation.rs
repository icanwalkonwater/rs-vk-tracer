//! # Allocation
//! Extension of the [VtDevice] that allows the allocation of various buffers.
//! These methods are abstractions on top of VMA.

use crate::images::{Format, ImageLayout};
use crate::images::{Extent2D, Extent3D, ImageUsage, SampleCount, Tiling};
use crate::{
    buffers::{VtBufferAndStaging, VtBufferData, VtCpuBuffer, VtGpuBuffer},
    device::VtDevice,
    errors::Result,
};
use ash::vk;

pub type DeviceSize = vk::DeviceSize;
pub type BufferUsage = vk::BufferUsageFlags;

// *** Buffers ***

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

// *** Images ***

pub enum ImageDimension {
    Dim1(u32),
    Dim2(Extent2D),
    Dim3(Extent3D),
}

pub struct ImageDescription {
    size: ImageDimension,
    format: Format,
    mip_levels: Option<u32>,
    array_layers: Option<u32>,
    samples: SampleCount,
    tiling: Tiling,
    usage: ImageUsage,
    layout: Option<ImageLayout>,
}

impl VtDevice {
    pub fn create_image(&self, desc: &ImageDescription) -> Result<()> {
        let (image_type, extent) = match desc.size {
            ImageDimension::Dim1(width) => (
                vk::ImageType::TYPE_1D,
                Extent3D {
                    width,
                    height: 1,
                    depth: 1,
                },
            ),
            ImageDimension::Dim2(size) => (
                vk::ImageType::TYPE_2D,
                Extent3D {
                    width: size.width,
                    height: size.height,
                    depth: 1,
                },
            ),
            ImageDimension::Dim3(extent) => (vk::ImageType::TYPE_3D, extent),
        };

        let (image, allocation, info) = self.vma.create_image(
            &vk::ImageCreateInfo::builder()
                .image_type(image_type)
                .extent(extent)
                .format(desc.format)
                .mip_levels(desc.mip_levels.unwrap_or(1))
                .array_layers(desc.array_layers.unwrap_or(1))
                .samples(desc.samples)
                .tiling(desc.tiling)
                .usage(desc.usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(desc.layout.unwrap_or(ImageLayout::GENERAL)),
            &vk_mem::AllocationCreateInfo {
                usage: vk_mem::MemoryUsage::GpuOnly,
                ..Default::default()
            },
        )?;

        Ok(())
    }
}
