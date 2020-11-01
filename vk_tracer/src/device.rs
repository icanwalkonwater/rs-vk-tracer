use std::{ffi::CStr, os::raw::c_char};

use ash::{
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};

use crate::{
    adapter::VtAdapter,
    command_recorder::{QueueFamilyIndices, VtCommandPool},
    errors::Result,
    instance::VtInstance,
};

pub struct VtDevice {
    pub(crate) handle: ash::Device,
    pub(crate) command_pool: VtCommandPool,
    pub(crate) vma: vk_mem::Allocator,

    #[cfg(feature = "ext-debug")]
    pub(crate) debug_utils: ash::extensions::ext::DebugUtils,
}

impl VtAdapter {
    pub fn create_device(&self, instance: &VtInstance) -> Result<VtDevice> {
        let enable_extensions = {
            // Add required extensions
            let mut extensions = self
                .2
                .required_extensions
                .iter()
                .map(|ext| ext.as_ptr())
                .collect::<Vec<_>>();

            // Add optional extensions that are present in the info
            unsafe {
                self.2
                    .optional_extensions
                    .iter()
                    .filter(|&&ext| {
                        self.1
                            .info
                            .extensions
                            .iter()
                            .any(|other| CStr::from_ptr(other.extension_name.as_ptr()) == ext)
                    })
                    .for_each(|ext| {
                        extensions.push(ext.as_ptr());
                    })
            }

            extensions
        };

        // Build validation layers
        let enable_layers = self
            .2
            .validation_layers
            .iter()
            .map(|layer| layer.as_ptr() as *const c_char)
            .collect::<Vec<_>>();

        // Queues create infos
        let queues_create_info = QueueFamilyIndices::from(&self.1).into_queue_create_info();

        // Build the device
        let device = unsafe {
            instance.instance.create_device(
                self.0,
                &vk::DeviceCreateInfo::builder()
                    .enabled_extension_names(&enable_extensions)
                    .enabled_layer_names(&enable_layers)
                    .queue_create_infos(&queues_create_info),
                None,
            )?
        };

        // Create the command pool container
        let command_pool = VtCommandPool::new(&self.1, &device)?;

        // Create a handle to VMA
        let vma = vk_mem::Allocator::new(&vk_mem::AllocatorCreateInfo {
            physical_device: self.0,
            device: device.clone(),
            instance: instance.instance.clone(),
            ..Default::default()
        })?;

        Ok(VtDevice {
            handle: device,
            command_pool,
            vma,
            #[cfg(feature = "ext-debug")]
            debug_utils: instance.debug_utils.loader.clone(),
        })
    }
}

impl Drop for VtDevice {
    fn drop(&mut self) {
        unsafe {
            for queue in self.command_pool.queues.iter() {
                let pool = queue.pool.lock().expect("Poisoned");
                self.handle.destroy_command_pool(*pool, None);
            }

            self.vma.destroy();

            self.handle.destroy_device(None);
        }
    }
}
