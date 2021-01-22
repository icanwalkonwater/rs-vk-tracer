use crate::{command_recorder::QueueType, errors::Result, renderer_creator::RendererCreator};
use ash::{version::DeviceV1_0, vk};
use std::{alloc::dealloc, sync::Arc};

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
        Self::new(
            vma,
            &BufferDescription {
                size: size as vk::DeviceSize,
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                location: vk_mem::MemoryUsage::GpuOnly,
            },
        )
    }

    pub(crate) fn new_index_buffer(vma: &Arc<vk_mem::Allocator>, size: usize) -> Result<Self> {
        Self::new(
            vma,
            &BufferDescription {
                size: size as vk::DeviceSize,
                usage: vk::BufferUsageFlags::INDEX_BUFFER,
                location: vk_mem::MemoryUsage::GpuOnly,
            },
        )
    }

    pub(crate) fn new_staging_buffer(vma: &Arc<vk_mem::Allocator>, size: usize) -> Result<Self> {
        Self::new(
            vma,
            &BufferDescription {
                size: size as vk::DeviceSize,
                usage: vk::BufferUsageFlags::TRANSFER_SRC,
                location: vk_mem::MemoryUsage::CpuOnly,
            },
        )
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
        use std::{ffi, mem};

        let (need_to_unmap, mapped_ptr) = self.ensure_mapped()?;

        let size = (mem::size_of::<D>() * data.len()) as vk::DeviceSize;
        let mut mapped_slice = ash::util::Align::new(
            mapped_ptr as *mut ffi::c_void,
            mem::align_of::<D>() as vk::DeviceSize,
            size,
        );

        mapped_slice.copy_from_slice(data);

        // Will be ignored if HOST_COHERENT
        self.vma
            .flush_allocation(&self.allocation, 0, size as usize)?;

        if need_to_unmap {
            self.vma.unmap_memory(&self.allocation)?;
        }

        Ok(())
    }

    pub unsafe fn copy_to(
        &self,
        creator: &RendererCreator,
        other: &mut RawBufferAllocation,
    ) -> Result<()> {
        assert!(self.info.get_size() <= other.info.get_size());

        let pool = creator
            .command_pools
            .get(&QueueType::Transfer)
            .unwrap()
            .lock()
            .expect("Poisoned");

        let buffer = creator.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::builder()
                .command_pool(*pool)
                .command_buffer_count(1)
                .level(vk::CommandBufferLevel::PRIMARY),
        )?[0];

        creator.device.begin_command_buffer(
            buffer,
            &vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        {
            let copy = vk::BufferCopy::builder()
                .size(self.info.get_size() as vk::DeviceSize)
                .src_offset(self.info.get_offset() as vk::DeviceSize)
                .dst_offset(other.info.get_offset() as vk::DeviceSize);

            creator
                .device
                .cmd_copy_buffer(buffer, self.buffer, other.buffer, &[copy.build()])
        }

        creator.device.end_command_buffer(buffer)?;

        Ok(())
    }
}

impl Drop for RawBufferAllocation {
    fn drop(&mut self) {
        self.vma
            .destroy_buffer(self.buffer, &self.allocation)
            .expect("Failed to free VMA buffer");
    }
}
