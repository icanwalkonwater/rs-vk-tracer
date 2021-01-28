use std::{collections::HashMap, mem::ManuallyDrop, sync::Arc};

use ash::{
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};
use log::debug;
use parking_lot::Mutex;

use crate::{
    adapter::Adapter,
    command_recorder::QueueType,
    errors::Result,
    mesh::{Mesh, MeshStandard, VertexPosUv},
    mesh_storage::{MeshId, StandardMeshStorage},
    present::{render_pass::RenderPass, swapchain::Swapchain},
    setup::{debug_utils::VtDebugUtils, renderer_creator_builder::RendererCreatorBuilder},
};

pub struct RendererCreator {
    pub(crate) entry: ash::Entry,
    pub(crate) instance: ash::Instance,
    pub(crate) debug_utils: ManuallyDrop<Option<VtDebugUtils>>,
    pub(crate) adapter: Adapter,
    pub(crate) device: Arc<ash::Device>,
    pub(crate) swapchain: ManuallyDrop<Swapchain>,
    pub(crate) render_pass: ManuallyDrop<RenderPass>,
    pub(crate) vma: Arc<Mutex<vk_mem::Allocator>>,
    pub(crate) command_pools: HashMap<QueueType, Arc<Mutex<(vk::Queue, vk::CommandPool)>>>,
    pub(crate) mesh_storage: ManuallyDrop<StandardMeshStorage>,
}

impl RendererCreator {
    pub fn builder() -> RendererCreatorBuilder {
        RendererCreatorBuilder::new()
    }

    pub fn resize(&mut self, window_size: (u32, u32)) -> Result<()> {
        self.adapter.update_surface_capabilities()?;
        self.swapchain.recreate(&self.adapter, window_size)?;
        debug!("Swapchain recreated to size {:?}", window_size);

        Ok(())
    }

    pub fn create_mesh(&mut self, vertices: &[VertexPosUv], indices: &[u16]) -> Result<MeshId> {
        let mesh = Mesh::new(self, vertices, indices)?;
        Ok(self.mesh_storage.register_mesh(mesh))
    }
}

impl Drop for RendererCreator {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.mesh_storage);
        }

        self.command_pools
            .iter()
            .for_each(|(_, queue_pool)| unsafe {
                let guard = queue_pool.lock();
                self.device.destroy_command_pool(guard.1, None);
            });

        self.vma.lock().destroy();

        unsafe {
            ManuallyDrop::drop(&mut self.render_pass);
            ManuallyDrop::drop(&mut self.swapchain);
        }

        unsafe {
            self.device.destroy_device(None);

            ManuallyDrop::drop(&mut self.debug_utils);
            self.instance.destroy_instance(None);
        }
    }
}
