use crate::{
    adapter::Adapter,
    command_recorder::QueueType,
    debug_utils::VtDebugUtils,
    errors::Result,
    mesh::{Mesh, MeshStandard, VertexPosUv},
    renderer_creator_builder::RendererCreatorBuilder,
};
use ash::{
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};
use std::{
    collections::HashMap,
    mem::ManuallyDrop,
    sync::{Arc, Mutex},
};

pub struct RendererCreator {
    pub(crate) instance: ash::Instance,
    pub(crate) adapter: Adapter,
    // TODO: replace with swapchain
    pub(crate) window_size: (f32, f32),
    pub(crate) device: ash::Device,
    pub(crate) debug_utils: ManuallyDrop<Option<VtDebugUtils>>,
    pub(crate) vma: Arc<Mutex<vk_mem::Allocator>>,
    // TODO: this is broken
    pub(crate) command_pools: HashMap<QueueType, Arc<Mutex<(vk::Queue, vk::CommandPool)>>>,
}

impl RendererCreator {
    pub fn builder() -> RendererCreatorBuilder {
        RendererCreatorBuilder::new()
    }

    pub fn create_mesh(&self, vertices: &[VertexPosUv], indices: &[u16]) -> Result<MeshStandard> {
        Mesh::new(self, vertices, indices)
    }
}

impl Drop for RendererCreator {
    fn drop(&mut self) {
        self.command_pools
            .iter()
            .for_each(|(_, queue_pool)| unsafe {
                let guard = queue_pool.lock().unwrap();
                self.device.destroy_command_pool(guard.1, None);
            });

        self.vma.lock().unwrap().destroy();

        unsafe {
            ManuallyDrop::drop(&mut self.debug_utils);
        }

        unsafe {
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
