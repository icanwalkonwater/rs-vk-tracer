use crate::allocator::RawBufferAllocation;
use crate::errors::Result;
use crate::renderer_creator::RendererCreator;
use std::sync::Arc;

pub struct TypedBuffer<D: Copy>(RawBufferAllocation, std::marker::PhantomData<D>);

impl<D: Copy> TypedBuffer<D> {
    /// # Safety
    /// The alignment of `D` and the size of the buffer isn't guarantied.
    pub(crate) unsafe fn from_raw(raw: RawBufferAllocation) -> Self {
        Self(raw, std::marker::PhantomData)
    }

    pub(crate) fn new_vertex_buffer(vma: &Arc<vk_mem::Allocator>, size: usize) -> Result<Self> {
        unsafe {
            Ok(TypedBuffer::from_raw(RawBufferAllocation::new_vertex_buffer(vma, size * std::mem::size_of::<D>())?))
        }
    }

    pub(crate) fn new_index_buffer(vma: &Arc<vk_mem::Allocator>, size: usize) -> Result<Self> {
        unsafe {
            Ok(TypedBuffer::from_raw(RawBufferAllocation::new_index_buffer(vma, size * std::mem::size_of::<D>())?))
        }
    }

    pub fn len(&self) -> usize {
        self.0.info.get_size() / std::mem::size_of::<D>()
    }

    pub fn store(&mut self, data: &[D]) -> Result<()> {
        // Don't copy too much
        let amount = data.len().min(self.len());
        unsafe { self.0.store(&data[..amount]) }
    }

    // TODO: copy_to

    pub fn as_raw(&self) -> &RawBufferAllocation {
        &self.0
    }
}

pub struct TypedBufferWithStaging<D: Copy> {
    staging: RawBufferAllocation,
    dst: TypedBuffer<D>,
}

impl<D: Copy> TypedBufferWithStaging<D> {
    pub(crate) fn new(creator: &RendererCreator, dst: TypedBuffer<D>) -> Result<Self> {
        let staging = RawBufferAllocation::new_staging_buffer(&creator.vma, dst.0.info.get_size())?;
        Ok(Self {
            staging,
            dst
        })
    }

    pub(crate) fn new_raw(creator: &RendererCreator, dst: RawBufferAllocation) -> Result<Self> {
        Self::new(creator, unsafe {TypedBuffer::from_raw(dst)})
    }

    pub fn store(&mut self, data: &[D]) -> Result<()> {
        let amount = data.len().min(self.dst.len());
        unsafe { self.staging.store(&data[..amount]) }
    }

    pub fn commit(mut self) -> Result<TypedBuffer<D>> {
        todo!("upload");
        Ok(self.dst)
    }
}
