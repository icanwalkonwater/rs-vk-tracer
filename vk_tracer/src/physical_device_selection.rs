use std::{
    collections::{hash_map::RandomState, HashSet},
    ffi::CStr,
};

use crate::{adapter::AdapterRequirements, errors::Result};
use ash::{version::InstanceV1_0, vk};
use log::{debug, error, info};

use crate::{
    errors::VtError, surface::choose_surface_format, utils::cstr_to_str, VULKAN_VERSION,
    VULKAN_VERSION_STR,
};

#[derive(Debug)]
pub(crate) struct PhysicalDeviceInfo {
    pub handle: vk::PhysicalDevice,
    pub properties: vk::PhysicalDeviceProperties,
    pub extensions: Vec<vk::ExtensionProperties>,
    pub features: vk::PhysicalDeviceFeatures,
    pub queue_families: Vec<vk::QueueFamilyProperties>,
    pub memory_properties: vk::PhysicalDeviceMemoryProperties,

    pub surface_capabilities: Option<vk::SurfaceCapabilitiesKHR>,
    pub surface_formats: Option<Vec<vk::SurfaceFormatKHR>>,
    pub surface_format_properties: Option<Vec<vk::FormatProperties>>,
    pub surface_present_modes: Option<Vec<vk::PresentModeKHR>>,
}

#[derive(Debug, Clone)]
pub(crate) struct QueueFamilyInfo {
    pub index: u32,
    pub properties: vk::QueueFamilyProperties,
}

#[derive(Debug)]
pub(crate) struct AdapterInfo {
    pub info: PhysicalDeviceInfo,
    pub graphics_queue: QueueFamilyInfo,
    pub present_queue: QueueFamilyInfo,
    pub transfer_queue: QueueFamilyInfo,
    pub score: u32,
}

pub(crate) fn pick_adapter(
    instance: &ash::Instance,
    requirements: &AdapterRequirements,
) -> Result<AdapterInfo> {
    let physical_devices = unsafe { instance.enumerate_physical_devices()? };

    let best_device = physical_devices
        .into_iter()
        .map(|physical_device| unsafe {
            let properties = instance.get_physical_device_properties(physical_device);
            let extensions = instance
                .enumerate_device_extension_properties(physical_device)
                .expect("Failed to enumerate device extensions");
            let features = instance.get_physical_device_features(physical_device);
            let queue_families =
                instance.get_physical_device_queue_family_properties(physical_device);
            let memory_properties = instance.get_physical_device_memory_properties(physical_device);

            let surface_capabilities = requirements.compatible_surface.as_ref().map(|surface| {
                surface
                    .loader
                    .get_physical_device_surface_capabilities(physical_device, surface.handle)
                    .expect("Failed to get surface capabilities")
            });

            let surface_formats = requirements.compatible_surface.as_ref().map(|surface| {
                surface
                    .loader
                    .get_physical_device_surface_formats(physical_device, surface.handle)
                    .expect("Faild to get surface formats")
            });
            let surface_format_properties = surface_formats.as_ref().map(|surface_formats| {
                surface_formats
                    .iter()
                    .map(|format| {
                        instance
                            .get_physical_device_format_properties(physical_device, format.format)
                    })
                    .collect::<Vec<_>>()
            });
            let surface_present_modes = requirements.compatible_surface.as_ref().map(|surface| {
                surface
                    .loader
                    .get_physical_device_surface_present_modes(physical_device, surface.handle)
                    .expect("Failed to get surface present modes")
            });

            PhysicalDeviceInfo {
                handle: physical_device,
                properties,
                extensions,
                features,
                queue_families,
                memory_properties,
                surface_capabilities,
                surface_formats,
                surface_format_properties,
                surface_present_modes,
            }
        })
        .filter_map(|device_info| {
            if let Some(res) = process_physical_device(device_info, requirements) {
                info!(" => Device is eligible");
                Some(res)
            } else {
                info!(" => Device not suitable");
                None
            }
        })
        .max_by(|left, right| Ord::cmp(&left.score, &right.score));

    if let Some(res) = &best_device {
        info!(
            "Choosed physical device '{}'",
            cstr_to_str(res.info.properties.device_name.as_ptr())
        )
    }

    best_device.ok_or(VtError::NoSuitableAdapter)
}

fn process_physical_device(
    info: PhysicalDeviceInfo,
    requirements: &AdapterRequirements,
) -> Option<AdapterInfo> {
    info!(
        "Processing physical device {:?}",
        cstr_to_str(info.properties.device_name.as_ptr())
    );

    // *** Check vulkan version (I think its useless but whatever

    {
        debug!(" Checking Vulkan version...");

        let device_version = info.properties.api_version;
        let major = vk::version_major(device_version);
        let minor = vk::version_minor(device_version);
        let patch = vk::version_patch(device_version);
        let device_version_str = format!("{}.{}.{}", major, minor, patch);

        if device_version >= VULKAN_VERSION {
            debug!("  Detected Vulkan {} [OK]", device_version_str);
        } else {
            error!(
                "  Vulkan {} required but only version {} found [FATAL]",
                VULKAN_VERSION_STR, device_version_str
            );
            return None;
        }
    }

    // *** Check extensions

    {
        use std::iter::FromIterator;
        debug!(" Checking extensions...");
        let mut missing_extensions =
            HashSet::<_, RandomState>::from_iter(requirements.required_extensions.iter().cloned());
        for extension in info.extensions.iter() {
            let name = unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) };
            if missing_extensions.remove(name) {
                debug!(" - {} [OK]", name.to_str().unwrap());
            }
        }

        if !missing_extensions.is_empty() {
            for missing in missing_extensions {
                debug!(" - {} [NOT FOUND]", missing.to_str().unwrap());
            }
            return None;
        }
    }

    // *** Check swapchain formats
    debug!(" Checking swapchain formats...");

    if let (Some(surface_formats), Some(surface_format_properties)) =
        (&info.surface_formats, &info.surface_format_properties)
    {
        debug!("  Available formats:");
        for format in surface_formats.iter() {
            debug!(
                "  - Format {:?} / Color space {:?}",
                format.format, format.color_space
            );
        }

        if let Some(format) =
            choose_surface_format(&surface_formats, &surface_format_properties, requirements)
        {
            debug!(" - Format {:?} [OK]", format.format);
            debug!(" - Color space {:?} [OK]", format.color_space);
        } else {
            debug!(" - Can't find the required color space and format !");
            return None;
        }
    } else {
        debug!("  No surface provided, skipping.")
    }

    // *** Check queue families
    debug!(" Checking queue families...");

    // Graphics

    let graphics_queue = info
        .queue_families
        .iter()
        .enumerate()
        .find(|(_, queue)| queue.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .map(|(index, &properties)| QueueFamilyInfo {
            index: index as u32,
            properties,
        });

    if let None = graphics_queue {
        debug!(" - No graphics queue found !");
        return None;
    }
    let graphics_queue = graphics_queue.unwrap();
    debug!(
        " - Graphics queue found (ID: {}) (x{}) [{:?}]",
        graphics_queue.index,
        graphics_queue.properties.queue_count,
        graphics_queue.properties.queue_flags,
    );

    // Present

    // TODO: try to find a dedicated queue
    let present_queue = info
        .queue_families
        .iter()
        .enumerate()
        .find(|(index, _)| unsafe {
            requirements
                .compatible_surface
                .as_ref()
                .map_or(true, |surface| {
                    surface
                        .loader
                        .get_physical_device_surface_support(
                            info.handle,
                            *index as u32,
                            surface.handle,
                        )
                        .unwrap_or_else(|err| {
                            error!(" - Failed to get surface support because of {:?}", err);
                            false
                        })
                })
        })
        .map(|(index, &properties)| QueueFamilyInfo {
            index: index as u32,
            properties,
        });

    if let None = present_queue {
        debug!(" - No queue that support presentation have been found !");
        return None;
    }
    let present_queue = present_queue.unwrap();
    debug!(
        " - Present queue found (ID: {}) (x{}) [{:?}]",
        present_queue.index,
        present_queue.properties.queue_count,
        present_queue.properties.queue_flags,
    );

    // Transfer

    let transfer_queue = info
        .queue_families
        .iter()
        .enumerate()
        // Try to find a queue exclusively for transfers
        .find(|(_, queue)| {
            queue.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && !queue.queue_flags.contains(vk::QueueFlags::GRAPHICS)
        })
        .map(|(index, &properties)| QueueFamilyInfo {
            index: index as u32,
            properties,
        })
        // Fallback to using the graphics queue
        .unwrap_or(graphics_queue.clone());

    if transfer_queue.index == graphics_queue.index {
        debug!(" - Using the graphics queue for transfer operations");
    } else {
        debug!(
            " - Using dedicated transfer queue (ID: {}) (x{}) [{:?}]",
            transfer_queue.index,
            transfer_queue.properties.queue_count,
            transfer_queue.properties.queue_flags
        );
    }

    // Score additional properties

    let mut score = 0u32;
    // Prefer dedicated hardware
    if info.properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
        score += 1000;
    }

    // Count device memory
    let physical_device_memory_size = info
        .memory_properties
        .memory_heaps
        .iter()
        .take(info.memory_properties.memory_heap_count as usize)
        .filter(|heap| heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
        .map(|heap| heap.size)
        .sum::<vk::DeviceSize>();
    // Count gigabytes of memory
    score += (physical_device_memory_size / 1_073_741_800) as u32;

    debug!(" Additional score of {}", score);

    Some(AdapterInfo {
        info,
        graphics_queue,
        present_queue,
        transfer_queue,
        score,
    })
}
