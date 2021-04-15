use crate::{errors::Result, mem::RawBufferAllocation};
use ash::vk;

pub struct TypedBuffer<D: Copy>(RawBufferAllocation, std::marker::PhantomData<D>);

impl<D: Copy> TypedBuffer<D> {
    /// # Safety
    /// The alignment of `D` and the size of the buffer isn't guarantied.
    pub(crate) unsafe fn from_raw(raw: RawBufferAllocation) -> Self {
        Self(raw, std::marker::PhantomData)
    }

    pub(crate) fn new_vertex_buffer(vma: &vk_mem::Allocator, size: usize) -> Result<Self> {
        unsafe {
            Ok(TypedBuffer::from_raw(
                RawBufferAllocation::new_vertex_buffer(vma, size * std::mem::size_of::<D>())?,
            ))
        }
    }

    pub(crate) fn new_index_buffer(vma: &vk_mem::Allocator, size: usize) -> Result<Self> {
        unsafe {
            Ok(TypedBuffer::from_raw(
                RawBufferAllocation::new_index_buffer(vma, size * std::mem::size_of::<D>())?,
            ))
        }
    }

    pub fn len(&self) -> usize {
        self.0.info.get_size() / std::mem::size_of::<D>()
    }

    pub fn store(&mut self, vma: &vk_mem::Allocator, data: &[D]) -> Result<()> {
        // Don't copy too much
        let amount = data.len().min(self.len());
        unsafe { self.0.store(vma, &data[..amount]) }
    }

    // TODO: copy_to

    pub fn as_raw(&self) -> &RawBufferAllocation {
        &self.0
    }

    pub fn as_raw_mut(&mut self) -> &mut RawBufferAllocation {
        &mut self.0
    }

    pub fn into_raw(self) -> RawBufferAllocation {
        self.0
    }
}

pub struct TypedBufferWithStaging<D: Copy> {
    staging: RawBufferAllocation,
    dst: TypedBuffer<D>,
}

impl<D: Copy> TypedBufferWithStaging<D> {
    pub(crate) fn new(
        vma: &vk_mem::Allocator,
        dst: TypedBuffer<D>,
    ) -> Result<TypedBufferWithStaging<D>> {
        let staging = RawBufferAllocation::new_staging_buffer(vma, dst.0.info.get_size())?;
        Ok(TypedBufferWithStaging { staging, dst })
    }

    pub(crate) fn new_raw(
        vma: &vk_mem::Allocator,
        dst: RawBufferAllocation,
    ) -> Result<TypedBufferWithStaging<D>> {
        TypedBufferWithStaging::new(vma, unsafe { TypedBuffer::from_raw(dst) })
    }

    pub fn store(&mut self, vma: &vk_mem::Allocator, data: &[D]) -> Result<()> {
        let amount = data.len().min(self.dst.len());
        unsafe { self.staging.store(vma, &data[..amount]) }
    }

    pub fn commit(
        mut self,
        vma: &vk_mem::Allocator,
        device: &ash::Device,
        pool: (vk::Queue, vk::CommandPool),
    ) -> Result<TypedBuffer<D>> {
        unsafe {
            self.staging.copy_to(device, pool, self.dst.as_raw_mut())?;
        }

        self.staging.destroy(vma)?;
        Ok(self.dst)
    }
}
