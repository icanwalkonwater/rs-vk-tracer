use std::borrow::Cow;
use crate::{AppInfo, VULKAN_VERSION};
use raw_window_handle::HasRawWindowHandle;
use ash::vk;
use crate::utils::str_to_cstr;
use crate::extensions::{required_instance_extensions_with_surface, required_instance_extensions};
use crate::errors::Result;
use ash::version::EntryV1_0;
use crate::debug_utils::VtDebugUtils;

pub struct RendererCreator<'instance, 'device> {
    instance: Option<Cow<'instance, ash::Instance>>,
    device: Option<Cow<'device, ash::Device>>,
    debug_utils: Option<VtDebugUtils>,
}

impl<'i, 'd> RendererCreator<'i, 'd> {
    pub fn builder() -> Self {
        Self {
            instance: None,
            device: None,
            debug_utils: None,
        }
    }

    pub fn with_instance<'instance>(mut self, instance: &'instance ash::Instance) -> RendererCreator<'instance, 'd> {
        self.instance = Some(Cow::Borrowed(instance));
        self
    }

    pub fn auto_create_instance(mut self, app_info: AppInfo, window: Option<&impl HasRawWindowHandle>, install_debug_utils: bool) -> Result<RendererCreator<'static, 'd>> {
        let entry = ash::Entry::new()?;

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
        let instance = {
            let info = vk::InstanceCreateInfo::builder()
                .application_info(&vk_app_info)
                .enabled_extension_names(&extensions);

            unsafe {
                entry.create_instance(&info, None)?
            }
        };

        if install_debug_utils {
            self.debug_utils = Some(VtDebugUtils::new(&entry, &instance)?);
        }

        self.instance = Some(Cow::Owned(instance));
        Ok(self)
    }

    pub fn with_device<'device>(mut self, device: &'device ash::Device) -> RendererCreator<'i, 'device> {
        self.device = Some(Cow::Borrowed(device));
        self
    }

    pub fn auto_create_device(mut self, )
}