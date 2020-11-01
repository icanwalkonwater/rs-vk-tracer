use crate::{device::VtDevice, errors::Result};
use ash::vk;
use log::error;
use std::{ffi, mem};

pub type DeviceSize = vk::DeviceSize;
pub type BufferUsage = vk::BufferUsageFlags;

#[derive(Default)]
pub struct BufferDescription {
    pub size: DeviceSize,
    pub usage: BufferUsage,
}

impl VtDevice {
    pub fn create_buffer<D>(&self, desc: &BufferDescription) -> Result<VtBuffer<D>> {
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

        Ok(VtBuffer {
            vma: &self.vma,
            buffer,
            allocation,
            info,
            _phantom: Default::default(),
        })
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
    pub fn create_staging_buffer_for<D>(&self, buffer: &VtBuffer<D>) -> Result<VtBuffer<D>> {
        self.create_staging_buffer(buffer.info.get_size() as DeviceSize)
    }

    pub fn create_staging_buffer<D>(&self, size: DeviceSize) -> Result<VtBuffer<D>> {
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
            _phantom: Default::default(),
        })
    }
}

pub struct VtBuffer<'a, D> {
    vma: &'a vk_mem::Allocator,
    pub(crate) buffer: vk::Buffer,
    pub(crate) allocation: vk_mem::Allocation,
    pub(crate) info: vk_mem::AllocationInfo,
    _phantom: std::marker::PhantomData<D>,
}

impl<D: Copy> VtBuffer<'_, D> {
    #[inline]
    fn ensure_mapped(&self) -> Result<(bool, *mut u8)> {
        if self.info.get_mapped_data().is_null() {
            Ok((true, self.vma.map_memory(&self.allocation)?))
        } else {
            Ok((false, self.info.get_mapped_data()))
        }
    }

    pub fn store(&mut self, data: &[D]) -> Result<()> {
        let (need_to_unmap, mapped_ptr) = self.ensure_mapped()?;

        unsafe {
            // Compute length of data
            let size = (mem::size_of::<D>() * data.len()) as DeviceSize;

            // Make sure to respect alignment requirements
            let mut mapped_slice = ash::util::Align::new(
                mapped_ptr as *mut ffi::c_void,
                mem::align_of::<D>() as DeviceSize,
                size,
            );

            // Copy the data
            mapped_slice.copy_from_slice(data);

            // Flush
            // Will be ignored of the memory is HOST_COHERENT
            self.vma
                .flush_allocation(&self.allocation, 0, size as usize)?;
        }

        if need_to_unmap {
            self.vma.unmap_memory(&self.allocation)?;
        }

        Ok(())
    }

    pub fn retrieve(&self) -> Result<Vec<D>> {
        let (need_to_unmap, mapped_ptr) = self.ensure_mapped()?;

        let data = unsafe {
            // Compute length of data
            let size = (self.info.get_size() / mem::size_of::<D>()) as DeviceSize;

            // Make sure to respect alignment requirements
            let mut mapped_slice = ash::util::Align::new(
                mapped_ptr as *mut ffi::c_void,
                mem::align_of::<D>() as DeviceSize,
                size,
            );

            // Copy the data
            mapped_slice.iter_mut().map(|a| *a).collect()
        };

        if need_to_unmap {
            self.vma.unmap_memory(&self.allocation)?;
        }

        Ok(data)
    }
}

impl<D> PartialEq for VtBuffer<'_, D> {
    fn eq(&self, other: &Self) -> bool {
        // Don't check the content for obvious reasons
        self.buffer == other.buffer && self.info.get_offset() == other.info.get_offset()
    }
}

impl<D> Drop for VtBuffer<'_, D> {
    fn drop(&mut self) {
        if let Err(err) = self.vma.destroy_buffer(self.buffer, &self.allocation) {
            error!("VMA Free Error: {}", err);
        }
    }
}

pub struct VtBufferAndStaging<'a, D> {
    device: &'a VtDevice,
    staging: VtBuffer<'a, D>,
    dst: VtBuffer<'a, D>,
}

impl<'a, D: Copy> VtBufferAndStaging<'a, D> {
    pub fn stage(&mut self, data: &[D]) -> Result<()> {
        self.staging.store(data)
    }

    pub fn upload(mut self) -> Result<VtBuffer<'a, D>> {
        let mut recorder = self.device.get_transient_transfer_recorder()?;

        recorder.copy_buffer_to_buffer(&self.staging, &mut self.dst)?;
        recorder.submit()?;

        Ok(self.dst)
    }
}
