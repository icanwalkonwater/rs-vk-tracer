use crate::{
    adapter::Adapter,
    command_recorder::QueueType,
    debug_utils::VtDebugUtils,
    errors::Result,
    mesh::{Mesh, MeshStandard, VertexPosUv},
    renderer_creator_builder::RendererCreatorBuilder,
};
use ash::vk;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub struct RendererCreator {
    pub(crate) instance: ash::Instance,
    pub(crate) adapter: Adapter,
    pub(crate) device: ash::Device,
    pub(crate) debug_utils: Option<VtDebugUtils>,
    pub(crate) vma: Arc<vk_mem::Allocator>,
    pub(crate) command_pools: HashMap<QueueType, Arc<Mutex<vk::CommandPool>>>,
}

impl RendererCreator {
    pub fn builder() -> RendererCreatorBuilder {
        RendererCreatorBuilder::new()
    }

    pub fn create_mesh(&self, vertices: &[VertexPosUv], indices: &[u16]) -> Result<MeshStandard> {
        Mesh::new(self, vertices, indices)
    }
}
