use crate::{
    adapter::{Adapter, AdapterRequirements},
    command_recorder::QueueType,
    debug_utils::VtDebugUtils,
    errors::{RendererCreatorError, Result, VtError},
    extensions::{required_instance_extensions, required_instance_extensions_with_surface},
    mesh_storage::MeshStorage,
    physical_device_selection::pick_adapter,
    queue_indices::QueueFamilyIndices,
    renderer_creator::RendererCreator,
    surface::Surface,
    swapchain::Swapchain,
    utils::str_to_cstr,
    AppInfo, VULKAN_VERSION,
};
use ash::{
    version::{DeviceV1_0, EntryV1_0, InstanceV1_0},
    vk,
};
use parking_lot::Mutex;
use raw_window_handle::HasRawWindowHandle;
use std::{
    collections::HashMap, ffi::CStr, mem::ManuallyDrop, ops::Deref, os::raw::c_char, sync::Arc,
};

#[derive(Debug)]
pub enum PhysicalDeviceChoice {
    First,
    Best,
}

pub struct RendererCreatorBuilder {
    instance: Option<ash::Instance>,
    adapter: Option<Adapter>,
    device: Option<ash::Device>,
    app_info: Option<AppInfo>,
    install_debug_utils: bool,
    validation_layers: Vec<&'static str>,
    // Ignored
    physical_device_choice: PhysicalDeviceChoice,
    hw_raytracing: bool,
}

impl RendererCreatorBuilder {
    pub(crate) fn new() -> Self {
        Self {
            instance: None,
            adapter: None,
            device: None,
            app_info: None,
            install_debug_utils: false,
            validation_layers: Vec::new(),
            physical_device_choice: PhysicalDeviceChoice::First,
            hw_raytracing: false,
        }
    }

    pub fn with_app_info(mut self, app_info: AppInfo) -> Self {
        self.app_info = Some(app_info);
        self
    }

    pub fn with_instance(mut self, instance: ash::Instance) -> Self {
        self.instance = Some(instance);
        self
    }

    pub fn with_device(mut self, adapter: Adapter, device: ash::Device) -> Self {
        self.adapter = Some(adapter);
        self.device = Some(device);
        self
    }

    pub fn with_debug_utils(mut self, install_debug_utils: bool) -> Self {
        self.install_debug_utils = install_debug_utils;
        self
    }

    pub fn with_validation_layer(mut self, layer: &'static str) -> Self {
        self.validation_layers.push(layer);
        self
    }

    pub fn pick_best_physical_device(mut self) -> Self {
        self.physical_device_choice = PhysicalDeviceChoice::Best;
        self
    }

    pub fn with_hardware_raytracing(mut self) -> Self {
        self.hw_raytracing = true;
        self
    }

    pub fn build_with_window(
        self,
        window: Option<&impl HasRawWindowHandle>,
        window_size: (u32, u32),
    ) -> Result<Arc<Mutex<RendererCreator>>> {
        // Checks
        if let None = self.app_info {
            return Err(VtError::RendererCreatorError(
                RendererCreatorError::MissingAppInfo,
            ));
        } else if self.adapter.is_some() ^ self.device.is_some() {
            return Err(VtError::RendererCreatorError(
                RendererCreatorError::AdapterDeviceRequired,
            ));
        }

        let entry = ash::Entry::new()?;

        // Create instance
        // <editor-fold>
        let instance = if let Some(instance) = self.instance {
            instance
        } else {
            // Convert app info
            let vk_app_info = vk::ApplicationInfo::builder()
                .application_name(str_to_cstr(self.app_info.as_ref().unwrap().name))
                .application_version({
                    let (major, minor, patch) = self.app_info.as_ref().unwrap().version;
                    vk::make_version(major, minor, patch)
                })
                .engine_name(str_to_cstr("VK Tracer"))
                .engine_version({
                    let major = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
                    let minor = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
                    let patch = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
                    vk::make_version(major, minor, patch)
                })
                .api_version(VULKAN_VERSION);

            // Gather extensions, window is optional
            let extensions = if let Some(window) = window {
                required_instance_extensions_with_surface(self.install_debug_utils, window)?
            } else {
                required_instance_extensions(self.install_debug_utils)
            };

            // Create instance
            let info = vk::InstanceCreateInfo::builder()
                .application_info(&vk_app_info)
                .enabled_extension_names(&extensions);

            unsafe { entry.create_instance(&info, None)? }
        };
        // </editor-fold>

        let debug_utils = if self.install_debug_utils {
            Some(VtDebugUtils::new(&entry, &instance)?)
        } else {
            None
        };

        // Create surface
        let surface = if let Some(window) = window {
            Some(Surface::create(&entry, &instance, window, window_size)?)
        } else {
            None
        };

        // Create adapter & device
        // <editor-fold>
        let (adapter, device) = if let (Some(adapter), Some(device)) = (self.adapter, self.device) {
            (adapter, device)
        } else {
            // Build adapter requirements
            let adapter_requirements = {
                let mut requirements = if let Some(window) = window {
                    AdapterRequirements::default_from_window(surface.as_ref().unwrap(), window)?
                } else {
                    AdapterRequirements::default()
                };

                requirements
                    .validation_layers
                    .extend(self.validation_layers.iter());

                if self.hw_raytracing {
                    requirements
                        .required_extensions
                        .extend_from_slice(&crate::raytracing::required_device_rt_extensions())
                }

                // Note: add other optional extensions here

                requirements
            };

            // Query adapter
            let adapter_info = pick_adapter(&instance, &adapter_requirements)?;
            let adapter = Adapter::new(
                adapter_info.physical_device_info.handle,
                adapter_info,
                adapter_requirements,
            );

            // Create device
            let device = {
                let enable_extensions =
                    {
                        // Add required extensions
                        let mut extensions = adapter
                            .requirements
                            .required_extensions
                            .iter()
                            .map(|ext| ext.as_ptr())
                            .collect::<Vec<_>>();

                        // Add optional extensions that are present in the info
                        unsafe {
                            adapter
                                .requirements
                                .optional_extensions
                                .iter()
                                .filter(|&&ext| {
                                    adapter.info.physical_device_info.extensions.iter().any(
                                        |other| {
                                            CStr::from_ptr(other.extension_name.as_ptr()) == ext
                                        },
                                    )
                                })
                                .for_each(|ext| {
                                    extensions.push(ext.as_ptr());
                                })
                        }

                        extensions
                    };

                // Build validation layers
                let enable_layers = adapter
                    .requirements
                    .validation_layers
                    .iter()
                    .map(|layer| layer.as_ptr() as *const c_char)
                    .collect::<Vec<_>>();

                // Queues create info
                let queues_create_info =
                    QueueFamilyIndices::from(&adapter.info).into_queue_create_info();

                unsafe {
                    instance.create_device(
                        adapter.handle,
                        &vk::DeviceCreateInfo::builder()
                            .enabled_extension_names(&enable_extensions)
                            .enabled_layer_names(&enable_layers)
                            .queue_create_infos(&queues_create_info),
                        None,
                    )?
                }
            };

            (adapter, device)
        };
        // </editor-fold>

        let device = Arc::new(device);

        // Swapchain
        let swapchain = if let Some(mut surface) = surface {
            surface.complete(&adapter);
            Some(Swapchain::new(
                &instance,
                surface,
                &adapter,
                &device,
                window_size,
            )?)
        } else {
            None
        };

        // Allocator
        let vma = vk_mem::Allocator::new(&vk_mem::AllocatorCreateInfo {
            physical_device: adapter.handle,
            device: (*device).clone(),
            instance: instance.clone(),
            flags: vk_mem::AllocatorCreateFlags::NONE,
            preferred_large_heap_block_size: 0,
            frame_in_use_count: 0,
            heap_size_limits: None,
        })?;

        // Command pools
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
                Result::Ok(Arc::new(Mutex::new((queue, pool))))
            };

            let (graphics_pool, transfer_pool) =
                if adapter.info.graphics_queue.index == adapter.info.transfer_queue.index {
                    let pool = pool_creator(
                        adapter.info.graphics_queue.index,
                        vk::CommandPoolCreateFlags::empty(),
                    )?;
                    (Arc::clone(&pool), pool)
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

        Ok(Arc::new(Mutex::new(RendererCreator {
            entry,
            instance,
            adapter,
            swapchain: ManuallyDrop::new(swapchain),
            device,
            debug_utils: ManuallyDrop::new(debug_utils),
            vma: Arc::new(Mutex::new(vma)),
            command_pools,
            mesh_storage: ManuallyDrop::new(MeshStorage::new()),
        })))
    }
}
