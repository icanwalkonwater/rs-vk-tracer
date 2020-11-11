//! # Buffers
//!
//! You can manipulate two kinds of buffers: CPU buffers ([VtCpuBuffer]) that are writable from the CPU
//! and GPU buffers ([VtGpuBuffer]) that aren't.
//!
//! This module also defines references types, namely [VtBuffer] and [VtBufferMut] that uses [From]
//! as well as [AsRef] and [AsMut] respectively to allow functions that don't care about the type
//! of buffer to use these reference types with something like `impl Into<VtBuffer<...>>`.

use std::{ffi, mem};

use ash::vk;
use log::error;

use crate::{
    allocation::DeviceSize, command_recorder::VtTransferCommands, device::VtDevice, errors::Result,
};

/// Represents a Vulkan buffer not accessible by the CPU.
pub struct VtGpuBuffer<'data, D: 'data>(pub(crate) VtBufferData<'data, D>);

/// Represents a Vulkan buffer accessible from the CPU.
pub struct VtCpuBuffer<'data, D: 'data>(pub(crate) VtBufferData<'data, D>);

/// Wrapper containing a reference to a buffer, doesn't care which type of buffer.
///
/// Implements [From] for [VtCpuBuffer] and [VtGpuBuffer] so you can write `impl Into<VtBuffer<...>>`.
///
/// Side note on the lifetimes:
/// * `'ptr` refers to the lifetime of the reference.
/// * `'data` refers to the lifetime of the buffer itself, it must be larger than `'ptr`.
#[derive(Copy, Clone)]
pub enum VtBuffer<'ptr, 'data: 'ptr, D: 'data> {
    Gpu(&'ptr VtGpuBuffer<'data, D>),
    Cpu(&'ptr VtCpuBuffer<'data, D>),
}

impl<'ptr, 'data: 'ptr, D: 'data> AsRef<VtBufferData<'data, D>> for VtBuffer<'ptr, 'data, D> {
    #[inline]
    fn as_ref(&self) -> &VtBufferData<'data, D> {
        match self {
            Self::Gpu(VtGpuBuffer(data)) | Self::Cpu(VtCpuBuffer(data)) => data,
        }
    }
}

/// Basically the same thing as [VtBuffer] but with a mutable reference.
pub enum VtBufferMut<'ptr, 'data: 'ptr, D: 'data> {
    Gpu(&'ptr mut VtGpuBuffer<'data, D>),
    Cpu(&'ptr mut VtCpuBuffer<'data, D>),
}

impl<'ptr, 'data: 'ptr, D: 'data> AsMut<VtBufferData<'data, D>> for VtBufferMut<'ptr, 'data, D> {
    #[inline]
    fn as_mut(&mut self) -> &mut VtBufferData<'data, D> {
        match self {
            Self::Gpu(VtGpuBuffer(data)) | Self::Cpu(VtCpuBuffer(data)) => data,
        }
    }
}

macro_rules! impl_from_buffer {
    ($buff_ty:ident, $ty:ident) => {
        impl<'ptr, 'data: 'ptr, D: 'data> From<&'ptr $buff_ty<'data, D>>
            for $crate::buffers::VtBuffer<'ptr, 'data, D>
        {
            #[inline]
            fn from(buff: &'ptr $buff_ty<'data, D>) -> Self {
                $crate::buffers::VtBuffer::$ty(buff)
            }
        }
    };
    (mut $buff_ty:ident, $ty:ident) => {
        impl<'ptr, 'data: 'ptr, D: 'data> From<&'ptr mut $buff_ty<'data, D>>
            for $crate::buffers::VtBufferMut<'ptr, 'data, D>
        {
            #[inline]
            fn from(buff: &'ptr mut $buff_ty<'data, D>) -> Self {
                $crate::buffers::VtBufferMut::$ty(buff)
            }
        }
    };
}

impl_from_buffer!(VtCpuBuffer, Cpu);
impl_from_buffer!(VtGpuBuffer, Gpu);
impl_from_buffer!(mut VtCpuBuffer, Cpu);
impl_from_buffer!(mut VtGpuBuffer, Gpu);

/// Inner data of every Vulkan buffer.
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

    /// Put some data in this buffer, it must be [Copy].
    /// Will map and unmap itself as needed.
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

    /// Copy data from the buffer into a [Vec].
    /// Will map and unmap itself as needed.
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

/// Wrapper around a Cpu buffer (used as a staging buffer) and Gpu buffer
/// to conveniently upload data .
pub struct VtBufferAndStaging<'a, D: 'a> {
    pub(crate) device: &'a VtDevice,
    pub(crate) staging: VtCpuBuffer<'a, D>,
    pub(crate) dst: VtGpuBuffer<'a, D>,
}

impl<'a, D: Copy> VtBufferAndStaging<'a, D> {
    /// Store data in the staging buffer, same as [VtCpuBuffer][store].
    /// Usually followed by [upload].
    pub fn stage(&mut self, data: &[D]) -> Result<()> {
        self.staging.store(data)
    }

    /// Retrieve data from the staging buffer, same as [VtCpuBuffer][retrieve].
    /// Usually done after a [download].
    pub fn retrieve(&self) -> Result<Vec<D>> {
        self.staging.retrieve()
    }
}

impl<'a, D> VtBufferAndStaging<'a, D> {
    /// Copy the data from the staging buffer into the Gpu buffer.
    /// Usually done after a call to [stage].
    /// This operation will overwrite the destination buffer.
    /// This operation is blocking.
    pub fn upload(&mut self) -> Result<()> {
        let mut recorder = self.device.get_transient_transfer_encoder()?;
        recorder.copy_buffer_to_buffer(&self.staging, &mut self.dst)?;
        recorder.finish()?.submit()?;

        Ok(())
    }

    /// Copy the data from the Gpu buffer into the staging buffer.
    /// This operation will overwrite the staging buffer.
    /// This operation is blocking.
    pub fn download(&mut self) -> Result<()> {
        let mut recorder = self.device.get_transient_transfer_encoder()?;
        recorder.copy_buffer_to_buffer(&self.dst, &mut self.staging)?;
        recorder.finish()?.submit()?;

        Ok(())
    }

    /// Discard the staging buffer and keep only the Gpu buffer.
    pub fn into_dst(self) -> VtGpuBuffer<'a, D> {
        self.dst
    }
}
