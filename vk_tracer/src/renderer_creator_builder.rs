use crate::{
    adapter::{Adapter, AdapterRequirements},
    debug_utils::VtDebugUtils,
    errors::Result,
    extensions::{required_instance_extensions, required_instance_extensions_with_surface},
    physical_device_selection::pick_adapter,
    queue_indices::QueueFamilyIndices,
    renderer_creator::RendererCreator,
    surface::Surface,
    utils::str_to_cstr,
    AppInfo, VULKAN_VERSION,
};
use ash::{
    version::{EntryV1_0, InstanceV1_0},
    vk,
};
use raw_window_handle::HasRawWindowHandle;
use std::{borrow::Cow, ffi::CStr, os::raw::c_char};

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

    pub fn build(self) -> Result<Self> {
        self.build_with_window(None, (0, 0))
    }

    pub fn build_with_window(
        self,
        window: Option<&impl HasRawWindowHandle>,
        window_size: (u32, u32),
    ) -> Result<Self> {
        let entry = ash::Entry::new()?;

        // Create instance
        // <editor-fold>
        let instance = if let Some(instance) = self.instance {
            instance
        } else {
            // Convert app info
            let vk_app_info = vk::ApplicationInfo::builder()
                .application_name(str_to_cstr(app_info.name.as_str()))
                .application_version({
                    let (major, minor, patch) = app_info.version;
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
                required_instance_extensions_with_surface(window)?
            } else {
                required_instance_extensions()
            };

            // Create instance
            let info = vk::InstanceCreateInfo::builder()
                .application_info(&vk_app_info)
                .enabled_extension_names(&extensions);

            unsafe { entry.create_instance(&info, None)? }
        };
        // </editor-fold>

        let debug_utils = if install_debug_utils {
            Some(VtDebugUtils::new(&entry, &instance)?)
        } else {
            None
        };

        // Create surface
        let surface = if let Some(window) = window {
            Some(Surface::create(insstance, window, window_size)?)
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
                    AdapterRequirements::default_from_window(surface.unwrap(), window)?
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
            let adapter =
                Adapter::new(adapter_info.info.handle, adapter_info, adapter_requirements);

            // Create device
            let device = {
                let enable_extensions = {
                    // Add required extensions
                    let mut extensions = adapter_requirements
                        .required_extensions
                        .iter()
                        .map(|ext| ext.as_ptr())
                        .collect::<Vec<_>>();

                    // Add optional extensions that are present in the info
                    unsafe {
                        adapter_requirements
                            .optional_extensions
                            .iter()
                            .filter(|&&ext| {
                                adapter.info.extensions.iter().any(|other| {
                                    CStr::from_ptr(other.extension_name.as_ptr()) == ext
                                })
                            })
                            .for_each(|ext| {
                                extensions.push(ext.as_ptr());
                            })
                    }

                    extensions
                };

                // Build validation layers
                let enable_layers = adapter_requirements
                    .validation_layers
                    .iter()
                    .map(|layer| layer.as_ptr() as *const c_char)
                    .collect::<Vec<_>>();

                // Queues create info
                let queues_create_info =
                    QueueFamilyIndices::from(&adapter_info).into_queue_create_info();

                unsafe {
                    instance.create_device(
                        adapter.info.handle,
                        &vk::DeviceCreateInfo::builder()
                            .enabled_extension_names(&enable_extensions)
                            .enabled_layer_names(&enable_layers)
                            .queue_create_infos(&queues_create_info),
                        None,
                    )?
                }

                // TODO command pool
                // TODO allocator
            };
        };
        // </editor-fold>

        Ok(())
    }
}
