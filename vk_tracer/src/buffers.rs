//! # Buffers
//! You can manipulate two kinds of buffers: CPU buffers ([VtCpuBuffer]) that are writable from the CPU
//! and GPU buffers ([VtGpuBuffer]) that aren't. This module also defines two wrapper types that allows

use std::{ffi, mem};

use ash::vk;
use log::error;

use crate::{
    allocation::DeviceSize, command_recorder::VtTransferCommands, device::VtDevice, errors::Result,
};

pub struct VtGpuBuffer<'a, D: 'a>(pub(crate) VtBufferData<'a, D>);

pub struct VtCpuBuffer<'a, D: 'a>(pub(crate) VtBufferData<'a, D>);

#[derive(Copy, Clone)]
pub enum VtBuffer<'a: 'b, 'b, D: 'a> {
    Gpu(&'b VtGpuBuffer<'a, D>),
    Cpu(&'b VtCpuBuffer<'a, D>),
}

impl<'a: 'b, 'b, D: 'a> From<&'b VtCpuBuffer<'a, D>> for VtBuffer<'a, 'b, D> {
    fn from(buff: &'b VtCpuBuffer<'a, D>) -> Self {
        VtBuffer::Cpu(buff)
    }
}

impl<'a: 'b, 'b, D: 'a> From<&'b VtGpuBuffer<'a, D>> for VtBuffer<'a, 'b, D> {
    fn from(buff: &'b VtGpuBuffer<'a, D>) -> Self {
        VtBuffer::Gpu(buff)
    }
}

pub enum VtBufferMut<'a: 'b, 'b, D: 'a> {
    Gpu(&'b mut VtGpuBuffer<'a, D>),
    Cpu(&'b mut VtCpuBuffer<'a, D>),
}

impl<'a: 'b, 'b, D: 'a> From<&'b mut VtCpuBuffer<'a, D>> for VtBufferMut<'a, 'b, D> {
    fn from(buff: &'b mut VtCpuBuffer<'a, D>) -> Self {
        VtBufferMut::Cpu(buff)
    }
}

impl<'a: 'b, 'b, D: 'a> From<&'b mut VtGpuBuffer<'a, D>> for VtBufferMut<'a, 'b, D> {
    fn from(buff: &'b mut VtGpuBuffer<'a, D>) -> Self {
        VtBufferMut::Gpu(buff)
    }
}

impl<'a: 'b, 'b, D: 'a> VtBuffer<'a, 'b, D> {
    pub(crate) fn data(&self) -> &VtBufferData<'a, D> {
        match self {
            Self::Gpu(VtGpuBuffer(data)) | Self::Cpu(VtCpuBuffer(data)) => data,
        }
    }
}

impl<'a: 'b, 'b, D: 'a> VtBufferMut<'a, 'b, D> {
    pub(crate) fn data_mut(&mut self) -> &mut VtBufferData<'a, D> {
        match self {
            Self::Gpu(VtGpuBuffer(data)) | Self::Cpu(VtCpuBuffer(data)) => data,
        }
    }
}

pub(crate) struct VtBufferData<'a, D: 'a> {
    pub vma: &'a vk_mem::Allocator,
    pub buffer: vk::Buffer,
    pub allocation: vk_mem::Allocation,
    pub info: vk_mem::AllocationInfo,
    pub _phantom: std::marker::PhantomData<D>,
}

impl<D: Copy> VtCpuBuffer<'_, D> {
    #[inline]
    fn ensure_mapped(&self) -> Result<(bool, *mut u8)> {
        if self.0.info.get_mapped_data().is_null() {
            Ok((true, self.0.vma.map_memory(&self.0.allocation)?))
        } else {
            Ok((false, self.0.info.get_mapped_data()))
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
            self.0
                .vma
                .flush_allocation(&self.0.allocation, 0, size as usize)?;
        }

        // Self explanatory
        if need_to_unmap {
            self.0.vma.unmap_memory(&self.0.allocation)?;
        }

        Ok(())
    }

    pub fn retrieve(&self) -> Result<Vec<D>> {
        let (need_to_unmap, mapped_ptr) = self.ensure_mapped()?;

        let data = unsafe {
            // Make sure to respect alignment requirements
            let mut mapped_slice = ash::util::Align::new(
                mapped_ptr as *mut ffi::c_void,
                mem::align_of::<D>() as DeviceSize,
                self.0.info.get_size() as DeviceSize,
            );

            // Copy the data
            mapped_slice.iter_mut().map(|a| *a).collect()
        };

        if need_to_unmap {
            self.0.vma.unmap_memory(&self.0.allocation)?;
        }

        Ok(data)
    }
}

impl<D> PartialEq for VtBufferData<'_, D> {
    fn eq(&self, other: &Self) -> bool {
        // Don't check the content for obvious reasons
        self.buffer == other.buffer && self.info.get_offset() == other.info.get_offset()
    }
}

impl<D> Drop for VtBufferData<'_, D> {
    fn drop(&mut self) {
        if let Err(err) = self.vma.destroy_buffer(self.buffer, &self.allocation) {
            error!("VMA Free Error: {}", err);
        }
    }
}

pub struct VtBufferAndStaging<'a, D: 'a> {
    pub(crate) device: &'a VtDevice,
    pub staging: VtCpuBuffer<'a, D>,
    pub dst: VtGpuBuffer<'a, D>,
}

impl<'a, D: Copy> VtBufferAndStaging<'a, D> {
    pub fn stage(&mut self, data: &[D]) -> Result<()> {
        self.staging.store(data)
    }

    pub fn upload(mut self) -> Result<VtGpuBuffer<'a, D>> {
        let mut recorder = self.device.get_transient_transfer_recorder()?;
        recorder.copy_buffer_to_buffer(&self.staging, &mut self.dst)?;
        recorder.finish()?.submit()?;

        Ok(self.dst)
    }
}

impl<'a, D> VtBufferAndStaging<'a, D> {
    pub fn into_inner(self) -> VtGpuBuffer<'a, D> {
        self.dst
    }
}
