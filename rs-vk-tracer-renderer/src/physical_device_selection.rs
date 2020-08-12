use crate::{
    errors::{RendererError, Result},
    extensions::vk_required_device_extensions,
    surface::SurfaceModule,
    vk_string_to_string,
};
use ash::{version::InstanceV1_0, vk};
use log::{debug, error, info};
use std::{
    collections::{hash_map::RandomState, HashSet},
    ffi::CStr,
    iter::FromIterator,
};

#[derive(Debug)]
pub struct PhysicalDeviceInfo {
    physical_device: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
    extensions: Vec<vk::ExtensionProperties>,
    features: vk::PhysicalDeviceFeatures,
    queue_families: Vec<vk::QueueFamilyProperties>,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
}

#[derive(Debug, Clone)]
pub struct QueueFamilyInfo {
    index: usize,
    properties: vk::QueueFamilyProperties,
}

#[derive(Debug)]
pub struct PhysicalDeviceResult {
    info: PhysicalDeviceInfo,
    graphics_queue: QueueFamilyInfo,
    present_queue: QueueFamilyInfo,
    transfer_queue: QueueFamilyInfo,
    score: u32,
}

pub fn pick_physical_device(
    instance: &ash::Instance,
    surface: &SurfaceModule,
) -> Result<PhysicalDeviceResult> {
    let required_extensions = vk_required_device_extensions();
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

            PhysicalDeviceInfo {
                physical_device,
                properties,
                extensions,
                features,
                queue_families,
                memory_properties,
            }
        })
        .filter_map(|device_info| {
            if let Some(res) = process_physical_device(device_info, &required_extensions, surface) {
                info!("=> Device is eligible");
                Some(res)
            } else {
                info!("=> Device not suitable");
                None
            }
        })
        .max_by(|left, right| Ord::cmp(&left.score, &right.score));

    if let Some(res) = &best_device {
        info!(
            "Choosed physical device '{}'",
            vk_string_to_string(res.info.properties.device_name.as_ptr())
        )
    }

    best_device.ok_or(RendererError::NoSuitablePhysicalDevice)
}

fn process_physical_device(
    info: PhysicalDeviceInfo,
    required_extensions: &[&CStr],
    surface: &SurfaceModule,
) -> Option<PhysicalDeviceResult> {
    info!(
        "Processing physical device {:?}",
        vk_string_to_string(info.properties.device_name.as_ptr())
    );

    // *** Check extensions

    debug!("  Checking extensions...");
    let mut missing_extensions =
        HashSet::<_, RandomState>::from_iter(required_extensions.iter().cloned());
    for extension in info.extensions.iter() {
        let name = unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) };
        if missing_extensions.remove(name) {
            info!("  - {} [OK]", name.to_str().unwrap());
        }
    }

    if !missing_extensions.is_empty() {
        for missing in missing_extensions {
            debug!("  - {} [NOT FOUND]", missing.to_str().unwrap());
        }
        return None;
    }

    // *** Check queue families
    debug!("  Checking queue families...");

    // Graphics

    let graphics_queue = info
        .queue_families
        .iter()
        .enumerate()
        .find(|(_, queue)| queue.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .map(|(index, &properties)| QueueFamilyInfo { index, properties });

    if let None = graphics_queue {
        debug!("  - No graphics queue found !");
        return None;
    }
    let graphics_queue = graphics_queue.unwrap();
    debug!(
        "  - Graphics queue found (ID: {}) (x{}) [{:?}]",
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
            surface
                .surface_ext
                .get_physical_device_surface_support(
                    info.physical_device,
                    *index as u32,
                    surface.surface_khr,
                )
                .unwrap_or_else(|err| {
                    error!("  - Failed to get surface support because of {:?}", err);
                    false
                })
        })
        .map(|(index, &properties)| QueueFamilyInfo { index, properties });

    if let None = present_queue {
        debug!("  - No queue that support presentation have been found !");
        return None;
    }
    let present_queue = present_queue.unwrap();
    debug!(
        "  - Present queue found (ID: {}) (x{}) [{:?}]",
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
        .map(|(index, &properties)| QueueFamilyInfo { index, properties })
        // Fallback to using the graphics queue
        .unwrap_or(graphics_queue.clone());

    if transfer_queue.index == graphics_queue.index {
        debug!("  - Using the graphics queue for transfer operations");
    } else {
        debug!(
            "  - Using dedicated transfer queue (ID: {}) (x{}) [{:?}]",
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

    Some(PhysicalDeviceResult {
        info,
        graphics_queue,
        present_queue,
        transfer_queue,
        score,
    })
}
