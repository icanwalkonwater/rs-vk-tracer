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
    renderers::{forward_renderer::ForwardRenderer, main_renderer::MainRenderer},
    setup::{debug_utils::VtDebugUtils, renderer_creator_builder::RendererCreatorBuilder},
};
use ash::vk::Queue;
use std::{fs::File, slice::from_ref};

pub struct RendererCreator {
    pub(crate) entry: ash::Entry,
    pub(crate) instance: ash::Instance,
    pub(crate) debug_utils: ManuallyDrop<Option<VtDebugUtils>>,
    pub(crate) adapter: Adapter,
    pub(crate) device: Arc<ash::Device>,

    pub(crate) swapchain: ManuallyDrop<Swapchain>,
    pub(crate) swpachain_suboptimal: bool,
    pub(crate) render_pass: ManuallyDrop<RenderPass>,

    pub(crate) vma: Arc<Mutex<vk_mem::Allocator>>,
    pub(crate) command_pools: HashMap<QueueType, Arc<Mutex<(vk::Queue, vk::CommandPool)>>>,
    pub(crate) mesh_storage: ManuallyDrop<StandardMeshStorage>,

    pub(crate) render_fence: vk::Fence,
    pub(crate) render_semaphore: vk::Semaphore,
}

impl RendererCreator {
    pub fn builder() -> RendererCreatorBuilder {
        RendererCreatorBuilder::new()
    }

    pub fn resize(&mut self, window_size: (u32, u32)) -> Result<()> {
        // Wait for everything to settle
        unsafe {
            self.device.queue_wait_idle(
                self.command_pools
                    .get(&QueueType::Graphics)
                    .unwrap()
                    .lock()
                    .0,
            )?;
        }

        self.adapter.update_surface_capabilities()?;
        self.swapchain.recreate(&self.adapter, window_size)?;
        debug!("Swapchain recreated to size {:?}", window_size);
        self.render_pass.recreate_framebuffers(&self.swapchain)?;

        Ok(())
    }

    pub fn create_mesh(&mut self, vertices: &[VertexPosUv], indices: &[u16]) -> Result<MeshId> {
        let mesh = Mesh::new(self, vertices, indices)?;
        Ok(self.mesh_storage.register_mesh(mesh))
    }

    pub fn new_forward_renderer(
        &mut self,
        mesh: MeshId,
        vertex: impl Into<File>,
        fragment: impl Into<File>,
    ) -> Result<ForwardRenderer> {
        ForwardRenderer::new::<VertexPosUv, u16>(
            &self.device,
            &self.swapchain,
            &self.render_pass,
            &mut vertex.into(),
            &mut fragment.into(),
            mesh,
        )
    }

    pub fn draw(&mut self, pipelines: &[ForwardRenderer]) -> Result<()> {
        // If swapchain suboptimal, recreate it
        if self.swpachain_suboptimal {
            let size = (self.swapchain.extent.width, self.swapchain.extent.height);
            self.resize(size)?;
        }

        // Wait for previous frame to finish and reset fences.
        unsafe {
            self.device
                .wait_for_fences(from_ref(&self.render_fence), true, std::u64::MAX)?;
            self.device.reset_fences(from_ref(&self.render_fence))?;
        }

        let (swapchain_image_index, is_suboptimal) = self.swapchain.acquire_next_image()?;
        self.swpachain_suboptimal = is_suboptimal;

        let mut main_renderer = None;
        {
            let other_pipelines = pipelines
                .iter()
                .map(|renderer| renderer.draw(self, swapchain_image_index).unwrap())
                .collect::<Vec<_>>();

            let graphics_queue = self.command_pools.get(&QueueType::Graphics).unwrap().lock();
            main_renderer = Some(MainRenderer::new(
                &self.device,
                &*graphics_queue,
                &self.swapchain,
                &self.render_pass,
                swapchain_image_index,
                &other_pipelines,
            )?);
            let main_renderer = main_renderer.as_ref().unwrap();

            let submit_info = vk::SubmitInfo::builder()
                .wait_dst_stage_mask(from_ref(&vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT))
                .wait_semaphores(from_ref(&self.swapchain.present_semaphore))
                .signal_semaphores(from_ref(&self.render_semaphore))
                .command_buffers(from_ref(&main_renderer.commands));

            let present_info = vk::PresentInfoKHR::builder()
                .swapchains(from_ref(&self.swapchain.handle))
                .wait_semaphores(from_ref(&self.render_semaphore))
                .image_indices(from_ref(&swapchain_image_index));

            unsafe {
                // Submit frame rendering
                self.device.queue_submit(
                    graphics_queue.0,
                    from_ref(&submit_info),
                    self.render_fence,
                )?;
                // Present frame as soon as its rendered
                let is_suboptimal = self
                    .swapchain
                    .loader
                    .queue_present(graphics_queue.0, &present_info)?;
                self.swpachain_suboptimal = is_suboptimal;
            }
        }

        unsafe {
            self.device
                .wait_for_fences(from_ref(&self.render_fence), true, std::u64::MAX)?;
        }

        Ok(())
    }
}

impl Drop for RendererCreator {
    fn drop(&mut self) {
        // Wait for queues to settle
        self.command_pools.values().for_each(|item| unsafe {
            let queue = item.lock();
            self.device.queue_wait_idle(queue.0).unwrap();
        });

        unsafe {
            // Destroy sync objects
            self.device.destroy_semaphore(self.render_semaphore, None);
            self.device.destroy_fence(self.render_fence, None);

            // Drop meshes
            ManuallyDrop::drop(&mut self.mesh_storage);
        }

        // Drop command pools
        self.command_pools
            .iter()
            .for_each(|(_, queue_pool)| unsafe {
                let guard = queue_pool.lock();
                self.device.destroy_command_pool(guard.1, None);
            });

        // Drop VMA
        self.vma.lock().destroy();

        unsafe {
            // Drop render pass & framebuffer
            ManuallyDrop::drop(&mut self.render_pass);
            // Drop swapchain
            ManuallyDrop::drop(&mut self.swapchain);
        }

        // Finally, drop device, instance and debug utils
        unsafe {
            self.device.destroy_device(None);

            ManuallyDrop::drop(&mut self.debug_utils);
            self.instance.destroy_instance(None);
        }
    }
}
