use ash::vk;
use crate::allocator::RawBufferAllocation;
use crate::renderer_creator::RendererCreator;
use crate::errors::Result;
use crate::buffers::{TypedBuffer, TypedBufferWithStaging};

pub trait Vertex: Copy {}
pub trait Index: Copy {}

#[derive(Clone, Copy)]
pub struct VertexPosUv {
    pos: [f32; 3],
    uv: [f32; 2],
}

impl Vertex for VertexPosUv {}

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
                TypedBuffer::new_vertex_buffer(&creator.vma, vertices.len())?
            )?;
            staging.store(vertices)?;
            staging.commit()?
        };

        let indices = {
            let mut staging = TypedBufferWithStaging::new(
                creator,
                TypedBuffer::new_index_buffer(&creator.vma, indices.len())?
            )?;
            staging.store(indices)?;
            staging.commit()?
        };

        Ok(Self {
            vertices, indices
        })
    }
}
