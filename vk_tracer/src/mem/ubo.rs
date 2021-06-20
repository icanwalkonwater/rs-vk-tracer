use crate::{VkTracerApp, UboHandle};
use glsl_layout::Uniform;
use crate::mem::{TypedBufferWithStaging, TypedBuffer};
use crate::errors::Result;
use crate::command_recorder::QueueType;

impl VkTracerApp {
    pub fn create_ubo<U: Uniform, const N: usize>(&mut self, data: [U; N]) -> Result<UboHandle> {
        let mut staging = TypedBufferWithStaging::new(
            &self.vma,
            TypedBuffer::new_uniform_buffer(&self.vma, data.len())?,
        )?;

        staging.store(&self.vma, &data)?;
        let ubo = staging.commit(&self.vma, &self.device, *self.command_pools.get(&QueueType::Transfer).unwrap())?;

        Ok(self.ubo_storage.insert(ubo.into_raw()))
    }
}