use crate::command_recorder::QueueType;
use crate::new::errors::Result;
use crate::new::mem::allocator::RawBufferAllocation;
use crate::new::mem::buffers::{TypedBuffer, TypedBufferWithStaging};
use crate::new::{MeshHandle, VkTracerApp};
use ash::vk;

impl VkTracerApp {
    pub fn create_mesh_indexed<V: MeshVertex, I: MeshIndex>(
        &mut self,
        vertices: &[V],
        indices: &[I],
    ) -> Result<MeshHandle> {
        let mesh = Mesh::new(
            &self.device,
            &self.vma,
            *self.command_pools.get(&QueueType::Transfer).unwrap(),
            vertices,
            indices,
        )?;
        Ok(self.mesh_storage.insert(mesh))
    }
}

pub trait MeshVertex: Copy {}

#[derive(Copy, Clone, Debug)]
pub struct VertexXyzUv {
    pub xyz: [f32; 3],
    pub uv: [f32; 2],
}

impl MeshVertex for VertexXyzUv {}

pub trait MeshIndex: Copy {}

impl MeshIndex for u16 {}

pub struct Mesh {
    pub(crate) vertices: RawBufferAllocation,
    pub(crate) indices: RawBufferAllocation,
}

impl Mesh {
    fn new<V: MeshVertex, I: MeshIndex>(
        device: &ash::Device,
        vma: &vk_mem::Allocator,
        transfer_pool: (vk::Queue, vk::CommandPool),
        vertices: &[V],
        indices: &[I],
    ) -> Result<Self> {
        let vertices = {
            let mut staging = TypedBufferWithStaging::new(
                vma,
                TypedBuffer::new_vertex_buffer(vma, vertices.len())?,
            )?;
            staging.store(vma, vertices)?;
            staging.commit(device, transfer_pool)?
        };

        let indices = {
            let mut staging = TypedBufferWithStaging::new(
                vma,
                TypedBuffer::new_index_buffer(vma, indices.len())?,
            )?;
            staging.store(vma, indices)?;
            staging.commit(device, transfer_pool)?
        };

        Ok(Self {
            vertices: vertices.into_raw(),
            indices: indices.into_raw(),
        })
    }
}
