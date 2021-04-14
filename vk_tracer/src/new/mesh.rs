use crate::{
    command_recorder::QueueType,
    new::{
        errors::Result,
        mem::{
            allocator::RawBufferAllocation,
            buffers::{TypedBuffer, TypedBufferWithStaging},
        },
        MeshHandle, VkTracerApp,
    },
};
use ash::vk;
use field_offset::offset_of;
use lazy_static::lazy_static;
use std::any::TypeId;

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

pub trait MeshVertex: Copy + 'static {
    fn binding_description() -> &'static [vk::VertexInputBindingDescription];
    fn attribute_description() -> &'static [vk::VertexInputAttributeDescription];
}

lazy_static! {
    static ref VERTEX_XYZ_UV_BINDING_DESC: [vk::VertexInputBindingDescription; 1] =
        [vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(std::mem::size_of::<VertexXyzUv>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build(),];
    static ref VERTEX_XYZ_UV_ATTRIBUTE_DESC: [vk::VertexInputAttributeDescription; 2] = [
        vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(offset_of!(VertexXyzUv => xyz).get_byte_offset() as u32)
            .build(),
        vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(offset_of!(VertexXyzUv => uv).get_byte_offset() as u32)
            .build(),
    ];
}

#[derive(Copy, Clone, Debug)]
pub struct VertexXyzUv {
    pub xyz: [f32; 3],
    pub uv: [f32; 2],
}

impl MeshVertex for VertexXyzUv {
    fn binding_description() -> &'static [vk::VertexInputBindingDescription] {
        &*VERTEX_XYZ_UV_BINDING_DESC
    }

    fn attribute_description() -> &'static [vk::VertexInputAttributeDescription] {
        &*VERTEX_XYZ_UV_ATTRIBUTE_DESC
    }
}

pub trait MeshIndex: Copy + 'static {
    fn ty() -> vk::IndexType;
}

impl MeshIndex for u16 {
    fn ty() -> vk::IndexType {
        vk::IndexType::UINT16
    }
}

pub struct Mesh {
    pub(crate) vertices: RawBufferAllocation,
    pub(crate) vertex_desc: (
        TypeId,
        &'static [vk::VertexInputBindingDescription],
        &'static [vk::VertexInputAttributeDescription],
    ),
    pub(crate) indices: RawBufferAllocation,
    pub(crate) indices_len: u32,
    pub(crate) index_ty: (TypeId, vk::IndexType),
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
            staging.commit(vma, device, transfer_pool)?
        };

        let indices = {
            let mut staging = TypedBufferWithStaging::new(
                vma,
                TypedBuffer::new_index_buffer(vma, indices.len())?,
            )?;
            staging.store(vma, indices)?;
            staging.commit(vma, device, transfer_pool)?
        };

        let indices_len = indices.len() as u32;

        Ok(Self {
            vertices: vertices.into_raw(),
            vertex_desc: (
                TypeId::of::<V>(),
                V::binding_description(),
                V::attribute_description(),
            ),
            indices: indices.into_raw(),
            indices_len,
            index_ty: (TypeId::of::<I>(), I::ty()),
        })
    }
}
