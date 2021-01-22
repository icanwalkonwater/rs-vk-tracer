use crate::{
    adapter::Adapter,
    debug_utils::VtDebugUtils,
    renderer_creator_builder::RendererCreatorBuilder,
};
use std::sync::Arc;
use crate::mesh::{Mesh, VertexPosUv, MeshStandard};
use crate::errors::Result;

pub struct RendererCreator {
    pub(crate) instance: ash::Instance,
    pub(crate) adapter: Adapter,
    pub(crate) device: ash::Device,
    pub(crate) debug_utils: Option<VtDebugUtils>,
    pub(crate) vma: Arc<vk_mem::Allocator>,
}

impl RendererCreator {
    pub fn builder() -> RendererCreatorBuilder {
        RendererCreatorBuilder::new()
    }

    pub fn create_mesh(&self, vertices: &[VertexPosUv], indices: &[u16]) -> Result<MeshStandard> {
        Mesh::new(self, vertices, indices)
    }
}
