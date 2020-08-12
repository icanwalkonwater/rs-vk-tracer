use crate::{
    errors::Result, extensions::vk_required_instance_extensions_with_surface, str_to_vk_string,
    AppInfo, ENGINE_NAME, ENGINE_VERSION, VULKAN_VERSION,
};
use ash::{
    self,
    version::{EntryV1_0, InstanceV1_0},
    vk,
};
use winit::window::Window;

#[cfg(feature = "validation-layers")]
use crate::debug_utils::{vk_validation_layers_raw, DebugUtilsModule};
use crate::{physical_device_selection::pick_physical_device, surface::SurfaceModule};
use std::mem::ManuallyDrop;

pub struct VulkanApp {
    _entry: ash::Entry,
    instance: ash::Instance,

    #[cfg(feature = "validation-layers")]
    debug_utils: ManuallyDrop<DebugUtilsModule>,

    surface: ManuallyDrop<SurfaceModule>,
}

// Creation
impl VulkanApp {
    pub fn new(app_info: AppInfo, window: &Window) -> Result<Self> {
        // Get entry and create instance
        let entry = ash::Entry::new()?;
        let instance = unsafe {
            let extension_names = vk_required_instance_extensions_with_surface(window)?;

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

        // Debug utils
        #[cfg(feature = "validation-layers")]
        let debug_utils = DebugUtilsModule::new(&entry, &instance)?;

        // Build surface
        let surface = SurfaceModule::new(&entry, &instance, window)?;

        // Pick physical device
        let physical_device = pick_physical_device(&instance, &surface)?;

        Ok(Self {
            _entry: entry,
            instance,
            #[cfg(feature = "validation-layers")]
            debug_utils: ManuallyDrop::new(debug_utils),
            surface: ManuallyDrop::new(surface),
        })
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.surface);

            #[cfg(feature = "validation-layers")]
            ManuallyDrop::drop(&mut self.debug_utils);

            self.instance.destroy_instance(None)
        }
    }
}
