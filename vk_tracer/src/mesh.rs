use crate::{
    allocator::RawBufferAllocation,
    buffers::{TypedBuffer, TypedBufferWithStaging},
    errors::Result,
    renderer_creator::RendererCreator,
};
use ash::vk;
use lazy_static::lazy_static;
use field_offset::offset_of;
use ash::vk::{VertexInputBindingDescription, VertexInputAttributeDescription};

pub trait Vertex: Copy {
    fn binding_desc() -> &'static [vk::VertexInputBindingDescription];
    fn attribute_desc() -> &'static [vk::VertexInputAttributeDescription];
}
pub trait Index: Copy {}

#[derive(Clone, Copy)]
pub struct VertexPosUv {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
}

lazy_static! {
        static ref VERTEX_POS_UV_BINDING_DESC: [vk::VertexInputBindingDescription; 1] = [
            vk::VertexInputBindingDescription::builder()
                .binding(0)
                .stride(std::mem::size_of::<VertexPosUv>())
                .input_rate(vk::VertexInputRate::VERTEX)
                .build(),
        ];

        static ref VERTEX_POS_UV_ATTR_DESC: [vk::VertexInputAttributeDescription; 2] = [
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(VertexPosUv => pos).get_byte_offset())
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(offset_of!(VertexPosUv => uv).get_byte_offset())
                .build(),
        ];
    }

impl Vertex for VertexPosUv {
    fn binding_desc() -> &'static [VertexInputBindingDescription] {
        &*VERTEX_POS_UV_BINDING_DESC
    }

    fn attribute_desc() -> &'static [VertexInputAttributeDescription] {
        &*VERTEX_POS_UV_ATTR_DESC
    }
}

impl Index for u16 {}

pub struct Mesh<V: Vertex, I: Index> {
    vertices: TypedBuffer<V>,
    indices: TypedBuffer<I>,
}

pub type MeshStandard = Mesh<VertexPosUv, u16>;

impl<V: Vertex, I: Index> Mesh<V, I> {
    pub(crate) fn new(creator: &RendererCreator, vertices: &[V], indices: &[I]) -> Result<Self> {
        let vertices = {
            let mut staging = TypedBufferWithStaging::new(
                creator,
                TypedBuffer::new_vertex_buffer(&creator.vma, vertices.len())?,
            )?;
            staging.store(vertices)?;
            staging.commit()?
        };

        let indices = {
            let mut staging = TypedBufferWithStaging::new(
                creator,
                TypedBuffer::new_index_buffer(&creator.vma, indices.len())?,
            )?;
            staging.store(indices)?;
            staging.commit()?
        };

        Ok(Self { vertices, indices })
    }
}
