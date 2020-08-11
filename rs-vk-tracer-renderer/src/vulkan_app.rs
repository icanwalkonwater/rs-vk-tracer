use crate::errors::Result;
use crate::extensions::vk_required_extensions_with_surface_raw;
use crate::{str_to_vk_string, AppInfo, ENGINE_NAME, ENGINE_VERSION, VULKAN_VERSION};
use ash;
use ash::version::{EntryV1_0, InstanceV1_0};
use ash::vk;
use winit::window::Window;

use crate::surface::SurfaceModule;
#[cfg(feature = "validation-layers")]
use crate::validation_layers::vk_validation_layers_raw;
use std::mem::ManuallyDrop;

pub struct VulkanApp {
    _entry: ash::Entry,
    instance: ash::Instance,

    surface: ManuallyDrop<SurfaceModule>,
}

// Creation
impl VulkanApp {
    pub fn new(app_info: AppInfo, window: &Window) -> Result<Self> {
        // Get entry and create instance
        let entry = ash::Entry::new()?;
        let instance = unsafe {
            let extension_names = vk_required_extensions_with_surface_raw(window)?;

            let application_info = vk::ApplicationInfo::builder()
                .application_name(str_to_vk_string(&app_info.name))
                .application_version(app_info.version)
                .engine_name(str_to_vk_string(ENGINE_NAME))
                .engine_version(*ENGINE_VERSION)
                .api_version(VULKAN_VERSION);

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&application_info)
                .enabled_extension_names(&extension_names);

            // Enable layers
            #[cfg(feature = "validation-layers")]
            let layers = vk_validation_layers_raw();
            #[cfg(feature = "validation-layers")]
            let create_info = create_info.enabled_layer_names(&layers);

            entry.create_instance(&create_info, None)
        }?;

        // Build surface
        let surface = SurfaceModule::new(&entry, &instance, window)?;

        Ok(Self {
            _entry: entry,
            instance,
            surface: ManuallyDrop::new(surface),
        })
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.surface);
            self.instance.destroy_instance(None)
        }
    }
}
