use crate::{
    command_recorder::QueueType,
    new::{
        adapter::Adapter,
        mesh::Mesh,
        render::{forward::ForwardPipeline, renderer::Renderer},
        render_plan::RenderPlan,
        render_target::RenderTarget,
        swapchain::Swapchain,
        surface::Surface,
    },
    setup::debug_utils::DebugUtils,
};
use ash::vk;
use slotmap::{new_key_type, SlotMap};
use std::collections::HashMap;
use ash::version::{DeviceV1_0, InstanceV1_0};
use std::slice::from_ref;

mod app_builder;
mod mem;
mod extensions;
mod adapter;
mod physical_device_selection;
mod queue_indices;
mod mesh;
mod render;
mod render_plan;
mod render_target;
mod swapchain;
mod surface;

pub const VULKAN_VERSION: u32 = ash::vk::make_version(1, 2, 0);
pub const VULKAN_VERSION_STR: &str = "1.2.0";

pub mod errors {
    use thiserror::Error;

    pub type Result<T> = std::result::Result<T, VkTracerError>;

    #[derive(Error, Debug)]
    pub enum VkTracerError {
        #[error("Vulkan error")]
        Vulkan(#[from] ash::vk::Result),
        #[error("Loading error")]
        LoadingError(#[from] ash::LoadingError),
        #[error("Instance error")]
        InstanceError(#[from] ash::InstanceError),
        #[error("VMA Error")]
        VmaError(#[from] vk_mem::Error),
        #[error("IO Error")]
        IoError(#[from] std::io::Error),
        #[error("No surface available")]
        NoSurfaceAvailable,
        #[error("No suitable adapter")]
        NoSuitableAdapterError,
        #[error("Invalid {0:?} handle")]
        InvalidHandle(HandleType),
    }

    #[derive(Debug)]
    pub enum HandleType {
        Mesh,
        Swapchain,
        RenderPlan,
        RenderTarget,
        ForwardPipeline,
        Renderer,
    }
}

pub mod prelude {
    pub use super::{
        app_builder::VkTracerExtensions,
        mesh::{MeshIndex, MeshVertex, VertexXyzUv},
        render_plan::SubpassBuilder,
        VkTracerApp,
    };
    pub use ash::vk::SubpassDependency2 as SubpassDependency;
}

new_key_type! {
    pub struct MeshHandle;
    pub struct SwapchainHandle;
    pub struct RenderPlanHandle;
    pub struct RenderTargetHandle;
    pub struct ForwardPipelineHandle;
    pub struct RendererHandle;
}

pub struct VkTracerApp {
    pub(crate) entry: ash::Entry,
    pub(crate) instance: ash::Instance,
    pub(crate) debug_utils: Option<DebugUtils>,
    pub(crate) surface: Option<Surface>,
    pub(crate) adapter: Adapter,
    pub(crate) device: ash::Device,
    pub(crate) vma: vk_mem::Allocator,
    pub(crate) command_pools: HashMap<QueueType, (vk::Queue, vk::CommandPool)>,

    pub(crate) mesh_storage: SlotMap<MeshHandle, Mesh>,
    pub(crate) swapchain_storage: SlotMap<SwapchainHandle, Swapchain>,
    pub(crate) render_plan_storage: SlotMap<RenderPlanHandle, RenderPlan>,
    pub(crate) render_target_storage: SlotMap<RenderTargetHandle, RenderTarget>,
    pub(crate) forward_pipeline_storage: SlotMap<ForwardPipelineHandle, ForwardPipeline>,
    pub(crate) renderer_storage: SlotMap<RendererHandle, Renderer>,
}

impl Drop for VkTracerApp {
    fn drop(&mut self) {
        let device = &self.device;
        let graphics_pool = self.command_pools.get(&QueueType::Graphics).unwrap();
        let transfer_pool = self.command_pools.get(&QueueType::Transfer).unwrap();

        unsafe {
            for (_, renderer) in &self.renderer_storage {
                device.destroy_fence(renderer.render_fence, None);
                device.free_command_buffers(graphics_pool.1, from_ref(&renderer.commands));
            }

            for (_, pipeline) in &self.forward_pipeline_storage {
                device.destroy_pipeline(pipeline.pipeline, None);
                device.destroy_pipeline_layout(pipeline.pipeline_layout, None);
            }

            for (_, render_target) in &self.render_target_storage {
                device.destroy_framebuffer(render_target.framebuffer, None);
            }

            for (_, render_plan) in &self.render_plan_storage {
                device.destroy_render_pass(render_plan.render_pass, None);
            }

            for (_, swapchain) in &self.swapchain_storage {
                device.destroy_semaphore(swapchain.image_available_semaphore, None);
                for view in &swapchain.image_views {
                    device.destroy_image_view(*view, None);
                }
                swapchain.loader.destroy_swapchain(swapchain.handle, None);
            }

            for (_, mesh) in self.mesh_storage.drain() {
                mesh.vertices.destroy(&self.vma).unwrap();
                mesh.indices.destroy(&self.vma).unwrap();
            }

            self.vma.destroy();

            device.destroy_command_pool(transfer_pool.1, None);
            device.destroy_command_pool(graphics_pool.1, None);

            if let Some(surface) = self.surface.as_ref() {
                surface.loader.destroy_surface(surface.handle, None);
            }

            device.destroy_device(None);
            if let Some(debug) = self.debug_utils.take() {
                debug.destroy();
            }
            self.instance.destroy_instance(None);
        }
    }
}
