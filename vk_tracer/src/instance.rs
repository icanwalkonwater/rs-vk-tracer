use std::mem::ManuallyDrop;

use raw_window_handle::HasRawWindowHandle;

#[cfg(feature = "ext-debug")]
use crate::debug_utils::VtDebugUtils;

use crate::{
    errors::Result,
    extensions::{vk_required_instance_extensions, vk_required_instance_extensions_with_surface},
    utils::str_to_cstr,
    VULKAN_VERSION,
};
use ash::{
    version::{EntryV1_0, InstanceV1_0},
    vk,
};

/// Information about the app to provide to vulkan
#[derive(Debug)]
pub struct VtAppInfo {
    pub name: &'static str,
    pub version: (u32, u32, u32),
}

/// Represent the entry point of the app.
/// Analog to the vulkan instance.
pub struct VtInstance {
    pub(crate) entry: ash::Entry,
    pub(crate) instance: ash::Instance,

    #[cfg(feature = "ext-debug")]
    pub(crate) debug_utils: ManuallyDrop<VtDebugUtils>,
}

impl VtInstance {
    pub fn create(app_info: VtAppInfo, window: Option<&impl HasRawWindowHandle>) -> Result<Self> {
        let entry = ash::Entry::new()?;

        let vk_app_info = vk::ApplicationInfo::builder()
            .application_name(str_to_cstr(app_info.name))
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

        let extension_names = if let Some(window) = window {
            vk_required_instance_extensions_with_surface(window)?
        } else {
            vk_required_instance_extensions()
        };

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&vk_app_info)
            .enabled_extension_names(&extension_names);

        let instance = unsafe { entry.create_instance(&create_info, None)? };

        #[cfg(feature = "ext-debug")]
        let debug_utils = VtDebugUtils::new(&entry, &instance)?;

        Ok(Self {
            entry,
            instance,
            #[cfg(feature = "ext-debug")]
            debug_utils: ManuallyDrop::new(debug_utils),
        })
    }
}

impl Drop for VtInstance {
    fn drop(&mut self) {
        unsafe {
            #[cfg(feature = "ext-debug")]
            ManuallyDrop::drop(&mut self.debug_utils);

            self.instance.destroy_instance(None);
        }
    }
}
