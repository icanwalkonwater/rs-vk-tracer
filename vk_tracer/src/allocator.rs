use ash::vk;
use std::sync::Arc;
use crate::renderer_creator::RendererCreator;
use crate::errors::Result;

pub struct BufferDescription {
    pub size: vk::DeviceSize,
    pub usage: vk::BufferUsageFlags,
    pub location: vk_mem::MemoryUsage,
}

pub struct RawBufferAllocation {
    pub(crate) vma: Arc<vk_mem::Allocator>,
    pub(crate) buffer: vk::Buffer,
    pub(crate) allocation: vk_mem::Allocation,
    pub(crate) info: vk_mem::AllocationInfo,
}

impl RawBufferAllocation {
    pub(crate) fn new_vertex_buffer(vma: &Arc<vk_mem::Allocator>, size: usize) -> Result<Self> {
        Self::new(vma, &BufferDescription {
            size: size as vk::DeviceSize,
            usage: vk::BufferUsageFlags::VERTEX_BUFFER,
            location: vk_mem::MemoryUsage::GpuOnly,
        })
    }

    pub(crate) fn new_index_buffer(vma: &Arc<vk_mem::Allocator>, size: usize) -> Result<Self> {
        Self::new(vma, &BufferDescription {
            size: size as vk::DeviceSize,
            usage: vk::BufferUsageFlags::INDEX_BUFFER,
            location: vk_mem::MemoryUsage::GpuOnly,
        })
    }

    pub(crate) fn new_staging_buffer(vma: &Arc<vk_mem::Allocator>, size: usize) -> Result<Self> {
        Self::new(vma, &BufferDescription {
            size: size as vk::DeviceSize,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            location: vk_mem::MemoryUsage::CpuOnly,
        })
    }

    pub(crate) fn new(vma: &Arc<vk_mem::Allocator>, desc: &BufferDescription) -> Result<Self> {
        let (buffer, allocation, info) = vma.create_buffer(
            &vk::BufferCreateInfo::builder()
                .size(desc.size)
                .usage(desc.usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            &vk_mem::AllocationCreateInfo {
                usage: desc.location,
                ..Default::default()
            },
        )?;

        Ok(RawBufferAllocation {
            vma: Arc::clone(vma),
            buffer,
            allocation,
            info,
        })
    }
}

impl RawBufferAllocation {
    pub(crate) fn ensure_mapped(&self) -> Result<(bool, *mut u8)> {
        if self.info.get_mapped_data().is_null() {
            Ok((true, self.vma.map_memory(&self.allocation)?))
        } else {
            Ok((false, self.info.get_mapped_data()))
        }
    }

    /// # Safety
    /// Will fail if the buffer isn't HOST_VISIBLE
    pub unsafe fn store<D: Copy>(&mut self, data: &[D]) -> Result<()> {
        use std::mem;
        use std::ffi;

        let (need_to_unmap, mapped_ptr) = self.ensure_mapped()?;

        let size = (mem::size_of::<D>() * data.len()) as vk::DeviceSize;
        let mut mapped_slice = ash::util::Align::new(
            mapped_ptr as *mut ffi::c_void,
            mem::align_of::<D>() as vk::DeviceSize,
            size
        );

        mapped_slice.copy_from_slice(data);

        // Will be ignored if HOST_COHERENT
        self.vma.flush_allocation(&self.allocation, 0, size as usize)?;

        if need_to_unmap {
            self.vma.unmap_memory(&self.allocation)?;
        }

        Ok(())
    }

    pub unsafe fn copy_to(&self, _creator: &RendererCreator, other: &mut RawBufferAllocation) -> Result<()> {
        assert!(self.info.get_size() <= other.info.get_size());
        todo!("Copy stuff")
    }
}

impl Drop for RawBufferAllocation {
    fn drop(&mut self) {
        self.vma.destroy_buffer(self.buffer, &self.allocation).expect("Failed to free VMA buffer");
    }
}
