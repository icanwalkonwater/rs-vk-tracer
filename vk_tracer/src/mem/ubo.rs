use crate::{
    command_recorder::QueueType,
    errors::{HandleType, Result},
    mem::{TypedBuffer, TypedBufferWithStaging},
    UboHandle, VkTracerApp,
};
use glsl_layout::Uniform;

impl VkTracerApp {
    pub fn create_ubo<U: Uniform, const N: usize>(&mut self, data: [U; N]) -> Result<UboHandle> {
        let mut staging = TypedBufferWithStaging::new(
            &self.vma,
            TypedBuffer::new_uniform_buffer(&self.vma, data.len())?,
        )?;

        staging.store(&self.vma, &data)?;
        let ubo = staging.commit(
            &self.vma,
            &self.device,
            *self.command_pools.get(&QueueType::Transfer).unwrap(),
        )?;

        Ok(self.ubo_storage.insert(ubo.into_raw()))
    }

    pub fn update_ubo<U: Uniform, const N: usize>(
        &mut self,
        handle: UboHandle,
        data: [U; N],
    ) -> Result<()> {
        let buffer = storage_access!(self.ubo_storage, handle, HandleType::Ubo);

        let mut staging = TypedBufferWithStaging::new_raw(&self.vma, buffer.clone())?;
        staging.store(&self.vma, &data)?;
        staging.commit(
            &self.vma,
            &self.device,
            *self.command_pools.get(&QueueType::Transfer).unwrap(),
        )?;
        Ok(())
    }
}
