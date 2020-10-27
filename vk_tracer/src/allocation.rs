use std::borrow::Cow;

use crate::device::VtDevice;
use crate::errors::Result;
use ash::vk;
use log::error;

pub type DeviceSize = vk::DeviceSize;
pub type BufferUsage = vk::BufferUsageFlags;

#[derive(Default)]
pub struct BufferDescription<'a> {
    pub label: Option<Cow<'a, str>>,
    pub size: DeviceSize,
    pub usage: BufferUsage,
}

impl VtDevice {
    pub fn create_buffer(&self, desc: &BufferDescription) -> Result<VtBuffer> {
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

        #[cfg(feature = "ext-debug")]
        if let Some(label) = &desc.label {
            self.name_object(vk::ObjectType::BUFFER, buffer, label)?;
        }

        Ok(VtBuffer {
            vma: &self.vma,
            buffer,
            allocation,
            info,
        })
    }

    #[inline]
    pub fn create_staging_buffer_for(&self, buffer: &VtBuffer) -> Result<VtBuffer> {
        self.create_staging_buffer(buffer.info.get_size() as DeviceSize)
    }

    pub fn create_staging_buffer(&self, size: DeviceSize) -> Result<VtBuffer> {
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

        Ok(VtBuffer {
            vma: &self.vma,
            buffer,
            allocation,
            info,
        })
    }
}

pub struct VtBuffer<'a> {
    vma: &'a vk_mem::Allocator,
    pub(crate) buffer: vk::Buffer,
    pub(crate) allocation: vk_mem::Allocation,
    pub(crate) info: vk_mem::AllocationInfo,
}

impl VtBuffer<'_> {
    pub fn store<D: Copy>(&self, data: &[D]) -> Result<()> {
        let (need_to_unmap, mapped_ptr) = if self.info.get_mapped_data().is_null() {
            (true, self.vma.map_memory(&self.allocation)?)
        } else {
            (false, self.info.get_mapped_data())
        };

        unsafe {
            use std::{ffi, mem};

            // Compute length of data
            let size = (mem::size_of::<D>() * data.len()) as DeviceSize;

            // Make sure to respect alignement requirements
            let mut mapped_slice = ash::util::Align::new(
                mapped_ptr as *mut ffi::c_void,
                mem::align_of::<D>() as DeviceSize,
                size,
            );

            // Copy the data
            mapped_slice.copy_from_slice(data);

            // Flush
            // Will be ignored of the memory is HOST_COHERANT
            self.vma
                .flush_allocation(&self.allocation, 0, size as usize)?;
        }

        if need_to_unmap {
            self.vma.unmap_memory(&self.allocation)?;
        }

        Ok(())
    }
}

impl PartialEq for VtBuffer<'_> {
    fn eq(&self, other: &Self) -> bool {
        // Don't check the content for obvious reasons
        self.buffer == other.buffer && self.info.get_offset() == other.info.get_offset()
    }
}

impl Drop for VtBuffer<'_> {
    fn drop(&mut self) {
        if let Err(err) = self.vma.destroy_buffer(self.buffer, &self.allocation) {
            error!("VMA Free Error: {}", err);
        }
    }
}

pub struct VtStagingBuffer<'a, D: Copy> {
    staging: VtBuffer<'a>,
    dest: VtBuffer<'a>,
    _marker: std::marker::PhantomData<D>,
}

impl<'a, D: Copy> VtStagingBuffer<'a, D> {
    pub fn store(&self, data: &[D]) -> Result<()> {
        self.staging.store(data)
    }

    pub fn upload(self) -> Result<VtBuffer<'a>> {
        Ok(self.dest)
    }
}
