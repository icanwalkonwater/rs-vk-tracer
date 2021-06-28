use crate::{
    command_recorder::QueueType,
    mesh::Mesh,
    render::{ForwardPipeline, Renderer},
    setup::DebugUtils,
};
use ash::{
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};
use present::{Surface, Swapchain};
use render::{RenderPlan, RenderTarget};
use setup::Adapter;
use slotmap::{new_key_type, SlotMap};
use std::{collections::HashMap, slice::from_ref};

#[macro_use]
macro_rules! storage_access {
    ($storage:expr, $handle:expr, $ty:expr) => {
        if cfg!(all(feature = "no_storage_checks", not(debug_assertions))) {
            #[allow(unused_unsafe)]
            unsafe {
                $storage.get_unchecked($handle)
            }
        } else {
            $storage
                .get($handle)
                .ok_or(crate::errors::VkTracerError::InvalidHandle($ty))?
        }
    };
}

#[macro_use]
macro_rules! storage_access_mut {
    ($storage:expr, $handle:expr, $ty:expr) => {
        if cfg!(all(feature = "no_storage_checks", not(debug_assertions))) {
            unsafe { $storage.get_unchecked_mut($handle) }
        } else {
            $storage
                .get_mut($handle)
                .ok_or(crate::errors::VkTracerError::InvalidHandle($ty))?
        }
    };
}

pub mod command_recorder;
pub mod mem;
pub mod mesh;
pub mod present;
pub mod render;
// #[cfg(feature = "render_graph")]
// pub mod render_graph;
#[cfg(feature = "render_graph")]
pub mod render_graph2;
pub mod setup;
pub mod utils;

use crate::mem::{DescriptorPool, DescriptorSet, RawBufferAllocation};
#[cfg(feature = "shaderc")]
pub use ::shaderc;
pub use ash;
pub use glsl_layout;
#[cfg(feature = "math")]
pub use nalgebra_glm as glm;

pub const VULKAN_VERSION: u32 = ash::vk::API_VERSION_1_2;
pub const VULKAN_VERSION_STR: &str = "1.2.0";

pub mod errors {
    #[cfg(feature = "render_graph")]
    use crate::render_graph::GraphValidationError;
    use thiserror::Error;

    pub type Result<T> = std::result::Result<T, VkTracerError>;

    #[derive(Error, Debug)]
    pub enum VkTracerError {
        #[cfg(feature = "shaderc")]
        #[error("Shader compiler error: {0}")]
        ShaderCompilerError(&'static str),
        #[cfg(feature = "shaderc")]
        #[error("Shaderc error: {0}")]
        ShaderCError(#[from] shaderc::Error),
        #[error("Vulkan error: {0}")]
        Vulkan(#[from] ash::vk::Result),
        #[error("Loading error: {0}")]
        LoadingError(#[from] ash::LoadingError),
        #[error("Instance error: {0}")]
        InstanceError(#[from] ash::InstanceError),
        #[error("VMA Error: {0}")]
        VmaError(#[from] vk_mem::Error),
        #[error("IO Error: {0}")]
        IoError(#[from] std::io::Error),
        #[error("No surface available")]
        NoSurfaceAvailable,
        #[error("No suitable adapter")]
        NoSuitableAdapterError,
        #[error("No suitable format can be found")]
        NoSuitableImageFormat,
        #[error("Invalid {0:?} handle")]
        InvalidHandle(HandleType),
        #[cfg(feature = "gltf")]
        #[error("Gltf error: {0}")]
        GltfError(#[from] gltf::Error),
        #[error("Can't bake render graph because of: {0:?}")]
        InvalidRenderGraph(GraphValidationError),
    }

    #[derive(Debug)]
    pub enum HandleType {
        // Higher level objects
        Mesh,
        Ubo,

        Swapchain,
        RenderPlan,
        RenderTarget,
        ForwardPipeline,
        Renderer,
        DescriptorPool,
        DescriptorSet,
    }
}

pub mod prelude {
    #[cfg(feature = "math")]
    pub use crate::mesh::{VertexXyz, VertexXyzUv, VertexXyzUvNorm};
    pub use crate::{
        errors::Result, glsl_layout::Uniform, mem::DescriptorSetBuilder, mesh::MeshIndex,
        render::SubpassBuilder, setup::VkTracerExtensions, ForwardPipelineHandle, MeshHandle,
        RenderPlanHandle, RenderTargetHandle, RendererHandle, SwapchainHandle, VkTracerApp,
    };
    pub use ash::vk::{
        AccessFlags, PipelineStageFlags, SubpassDependency2 as SubpassDependency, SUBPASS_EXTERNAL,
    };
}

new_key_type! {
    // Higher level objects
    pub struct MeshHandle;
    pub struct UboHandle;

    pub struct SwapchainHandle;
    pub struct RenderPlanHandle;
    pub struct RenderTargetHandle;
    pub struct ForwardPipelineHandle;
    pub struct RendererHandle;
    pub struct DescriptorPoolHandle;
    pub struct DescriptorSetHandle;
}

pub struct VkTracerApp {
    pub(crate) entry: ash::Entry,
    pub(crate) instance: ash::Instance,
    pub(crate) debug_utils: Option<DebugUtils>,
    pub(crate) surface: Option<Surface>,
    pub(crate) adapter: Adapter,
    pub(crate) device: ash::Device,
    pub(crate) synchronization2: ash::extensions::khr::Synchronization2,
    pub(crate) vma: vk_mem::Allocator,
    pub(crate) command_pools: HashMap<QueueType, (vk::Queue, vk::CommandPool)>,

    // Higher level objects
    pub(crate) mesh_storage: SlotMap<MeshHandle, Mesh>,
    pub(crate) ubo_storage: SlotMap<UboHandle, RawBufferAllocation>,

    pub(crate) swapchain_storage: SlotMap<SwapchainHandle, Swapchain>,
    pub(crate) render_plan_storage: SlotMap<RenderPlanHandle, RenderPlan>,
    pub(crate) render_target_storage: SlotMap<RenderTargetHandle, RenderTarget>,
    pub(crate) forward_pipeline_storage: SlotMap<ForwardPipelineHandle, ForwardPipeline>,
    pub(crate) renderer_storage: SlotMap<RendererHandle, Renderer>,
    pub(crate) descriptor_pool_storage: SlotMap<DescriptorPoolHandle, DescriptorPool>,
    pub(crate) descriptor_set_storage: SlotMap<DescriptorSetHandle, DescriptorSet>,
}

impl Drop for VkTracerApp {
    fn drop(&mut self) {
        let device = &self.device;
        let graphics_pool = self.command_pools.get(&QueueType::Graphics).unwrap();
        let transfer_pool = self.command_pools.get(&QueueType::Transfer).unwrap();

        unsafe {
            for (_, pool) in &self.descriptor_pool_storage {
                device.destroy_descriptor_pool(pool.handle, None);
            }

            for (_, set) in &self.descriptor_set_storage {
                device.destroy_descriptor_set_layout(set.layout, None);
            }

            for (_, renderer) in &self.renderer_storage {
                device.destroy_fence(renderer.render_fence, None);
                device.free_command_buffers(graphics_pool.1, from_ref(&renderer.main_commands));
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

            for (_, ubo) in self.ubo_storage.drain() {
                ubo.destroy(&self.vma).unwrap();
            }

            for (_, mesh) in self.mesh_storage.drain() {
                mesh.vertices.destroy(&self.vma).unwrap();
                mesh.indices.destroy(&self.vma).unwrap();
            }

            // synchronization2 has literally nothing to be dropped

            self.vma.destroy();

            device.destroy_command_pool(transfer_pool.1, None);
            if transfer_pool.1 != graphics_pool.1 {
                device.destroy_command_pool(graphics_pool.1, None);
            }

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
