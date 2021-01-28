use crate::physical_device_selection::AdapterInfo;
use ash::vk;

const QUEUE_PRIORITIES_ONE: [f32; 1] = [1.0];

#[derive(Copy, Clone)]
pub(crate) struct QueueFamilyIndices {
    pub graphics: u32,
    pub transfer: u32,
}

impl From<&AdapterInfo> for QueueFamilyIndices {
    fn from(device: &AdapterInfo) -> Self {
        Self {
            graphics: device.graphics_queue.index as u32,
            transfer: device.transfer_queue.index as u32,
        }
    }
}

impl QueueFamilyIndices {
    pub fn into_queue_create_info(self) -> Vec<vk::DeviceQueueCreateInfo> {
        let mut queues_create_info = Vec::new();

        // Graphics queue
        queues_create_info.push(
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(self.graphics as u32)
                .queue_priorities(&QUEUE_PRIORITIES_ONE)
                .build(),
        );

        // Transfer queue
        if self.transfer != self.graphics {
            queues_create_info.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(self.transfer as u32)
                    .queue_priorities(&QUEUE_PRIORITIES_ONE)
                    .build(),
            );
        }

        queues_create_info
    }
}
