use crate::{
    command_recorder::QueueType,
    errors::Result,
    present::Surface,
    setup::{
        debug_utils::DebugUtils,
        extensions::{required_instance_extensions, required_instance_extensions_with_surface},
        pick_adapter, Adapter, AdapterRequirements, QueueFamilyIndices,
    },
    utils::str_to_cstr,
    VkTracerApp, VULKAN_VERSION,
};
use ash::{
    version::{DeviceV1_0, EntryV1_0, InstanceV1_0},
    vk,
};
use log::debug;
use raw_window_handle::HasRawWindowHandle;
use slotmap::SlotMap;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::{CStr, CString},
};

#[derive(Copy, Clone, Debug)]
enum PhysicalDevicePreference {
    Best,
}

#[derive(Hash, Eq, PartialEq, Copy, Clone, Debug)]
pub enum VkTracerExtensions {
    PipelineRaytracing,
}

pub struct VkTracerAppBuilder {
    physical_device_preference: PhysicalDevicePreference,
    app_name: Cow<'static, str>,
    version: (u32, u32, u32),
    debug_utils: bool,
    extensions: HashSet<VkTracerExtensions>,
}

impl VkTracerApp {
    pub fn builder() -> VkTracerAppBuilder {
        VkTracerAppBuilder {
            physical_device_preference: PhysicalDevicePreference::Best,
            app_name: Cow::Borrowed("Unnamed"),
            version: (0, 0, 1),
            debug_utils: false,
            extensions: HashSet::new(),
        }
    }
}

impl VkTracerAppBuilder {
    pub fn pick_best_physical_device(mut self) -> Self {
        self.physical_device_preference = PhysicalDevicePreference::Best;
        self
    }

    pub fn with_app_info(mut self, app_name: Cow<'static, str>, version: (u32, u32, u32)) -> Self {
        self.app_name = app_name;
        self.version = version;
        self
    }

    pub fn with_debug_utils(mut self) -> Self {
        self.debug_utils = true;
        self
    }

    pub fn with_extensions(mut self, extensions: &[VkTracerExtensions]) -> Self {
        self.extensions.extend(extensions.iter());
        self
    }

    pub fn build<W: HasRawWindowHandle>(
        self,
        window: Option<(&W, (u32, u32))>,
    ) -> Result<VkTracerApp> {
        let entry = unsafe { ash::Entry::new()? };
        debug!("Entry created");

        let instance = {
            // Convert app info
            let app_name = CString::new(self.app_name.as_bytes()).unwrap();

            let vk_app_info = vk::ApplicationInfo::builder()
                .application_name(&app_name)
                .application_version({
                    let (major, minor, patch) = self.version;
                    vk::make_version(major, minor, patch)
                })
                .engine_name(str_to_cstr("VK Tracer\0"))
                .engine_version({
                    let major = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
                    let minor = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
                    let patch = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
                    vk::make_version(major, minor, patch)
                })
                .api_version(VULKAN_VERSION);

            // Gather extensions, window is optional
            let vk_extensions = if let Some((window, _)) = window {
                required_instance_extensions_with_surface(self.debug_utils, window).unwrap()
            } else {
                required_instance_extensions(self.debug_utils)
            };

            // Create instance
            let info = vk::InstanceCreateInfo::builder()
                .application_info(&vk_app_info)
                .enabled_extension_names(&vk_extensions);

            unsafe { entry.create_instance(&info, None)? }
        };
        debug!("Instance created");

        let debug_utils = if self.debug_utils {
            Some(DebugUtils::new(&entry, &instance).unwrap())
        } else {
            None
        };

        let mut surface = if let Some((window, size)) = window.as_ref() {
            Some(Surface::create(&entry, &instance, *window, *size).unwrap())
        } else {
            None
        };

        let (adapter, device) = {
            // Build adapter requirements
            let adapter_requirements = {
                let mut requirements = if let (Some((window, _)), Some(surface)) =
                    (window.as_ref(), surface.as_ref())
                {
                    AdapterRequirements::default_from_window(surface, *window).unwrap()
                } else {
                    AdapterRequirements::default()
                };

                requirements
                    .required_extensions
                    .extend(vk_tracer_extensions_to_vk_extensions(
                        self.extensions.iter(),
                    ));
                requirements
            };

            // Query adapter
            let adapter_info = pick_adapter(&instance, &adapter_requirements).unwrap();
            let adapter = Adapter::new(
                adapter_info.physical_device_info.handle,
                adapter_info,
                adapter_requirements,
            );

            debug!("Created adapter");

            // Create device
            let device = {
                let enable_extensions = adapter
                    .requirements
                    .required_extensions
                    .iter()
                    .map(|ext| ext.as_ptr())
                    .collect::<Vec<_>>();

                // Queues create info
                let queues_create_info =
                    QueueFamilyIndices::from(&adapter.info).into_queue_create_info();

                unsafe {
                    instance.create_device(
                        adapter.handle,
                        &vk::DeviceCreateInfo::builder()
                            .enabled_extension_names(&enable_extensions)
                            .queue_create_infos(&queues_create_info),
                        None,
                    )?
                }
            };
            debug!("Created device");

            (adapter, device)
        };

        if let Some(surface) = surface.as_mut() {
            surface.complete(&adapter);
            debug!("Surface complete");
        }

        let vma = vk_mem::Allocator::new(&vk_mem::AllocatorCreateInfo {
            physical_device: adapter.handle,
            device: device.clone(),
            instance: instance.clone(),
            flags: vk_mem::AllocatorCreateFlags::NONE,
            preferred_large_heap_block_size: 0,
            frame_in_use_count: 0,
            heap_size_limits: None,
        })?;

        debug!("VMA allocator created");

        let command_pools = {
            // Pool creation macro
            let pool_creator = |queue_index: u32, flags: vk::CommandPoolCreateFlags| unsafe {
                let queue = device.get_device_queue(queue_index, 0);
                let pool = device.create_command_pool(
                    &vk::CommandPoolCreateInfo::builder()
                        .flags(flags)
                        .queue_family_index(queue_index),
                    None,
                )?;
                Result::Ok((queue, pool))
            };

            let (graphics_pool, transfer_pool) =
                if adapter.info.graphics_queue.index == adapter.info.transfer_queue.index {
                    let pool = pool_creator(
                        adapter.info.graphics_queue.index,
                        vk::CommandPoolCreateFlags::empty(),
                    )?;
                    (pool, pool)
                } else {
                    let graphics_pool = pool_creator(
                        adapter.info.graphics_queue.index,
                        vk::CommandPoolCreateFlags::empty(),
                    )?;
                    let transfer_pool = pool_creator(
                        adapter.info.transfer_queue.index,
                        vk::CommandPoolCreateFlags::TRANSIENT,
                    )?;
                    (graphics_pool, transfer_pool)
                };

            let mut command_pools = HashMap::with_capacity(2);
            command_pools.insert(QueueType::Graphics, graphics_pool);
            command_pools.insert(QueueType::Transfer, transfer_pool);
            command_pools
        };

        debug!("Command pools created");

        Ok(VkTracerApp {
            entry,
            instance,
            debug_utils,
            surface,
            adapter,
            device,
            vma,
            command_pools,
            mesh_storage: SlotMap::with_key(),
            swapchain_storage: SlotMap::with_key(),
            render_plan_storage: SlotMap::with_key(),
            render_target_storage: SlotMap::with_key(),
            forward_pipeline_storage: SlotMap::with_key(),
            renderer_storage: SlotMap::with_key(),
        })
    }
}

fn vk_tracer_extensions_to_vk_extensions<'a>(
    extensions: impl Iterator<Item = &'a VkTracerExtensions>,
) -> impl Iterator<Item = &'static CStr> {
    use ash::extensions::khr;

    let mut res = HashSet::new();

    for extension in extensions {
        match extension {
            VkTracerExtensions::PipelineRaytracing => {
                // VK_KHR_spirv_1_4 promoted to vulkan 1.2
                // VK_EXT_descriptor_indexing promoted to vulkan 1.2
                // VK_KHR_buffer_device_address promoted to vulkan 1.2
                res.insert(khr::DeferredHostOperations::name());
                res.insert(khr::AccelerationStructure::name());
                res.insert(khr::RayTracingPipeline::name());
            }
        }
    }

    res.into_iter()
}
