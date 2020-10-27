use ash::{version::DeviceV1_0, vk};
use log::error;

use crate::{device::VtDevice, errors::Result, physical_device_selection::VtAdapterInfo};

const QUEUE_PRIORITIES_ONE: [f32; 1] = [1.0];

// Queue indices
// <editor-fold>

#[derive(Copy, Clone)]
pub(crate) struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
    pub transfer: u32,
}

impl From<&VtAdapterInfo> for QueueFamilyIndices {
    fn from(device: &VtAdapterInfo) -> Self {
        Self {
            graphics: device.graphics_queue.index as u32,
            present: device.present_queue.index as u32,
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

        // Present queue
        if self.present != self.graphics {
            queues_create_info.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(self.present as u32)
                    .queue_priorities(&QUEUE_PRIORITIES_ONE)
                    .build(),
            );
        }

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

#[derive(Copy, Clone)]
pub(crate) struct QueuePool {
    pub queue: vk::Queue,
    pub pool: vk::CommandPool,
}

pub(crate) struct VtCommandPool {
    pub graphics: QueuePool,
    pub present: QueuePool,
    pub transfer: QueuePool,
}

impl VtCommandPool {
    pub fn new(adapter: &VtAdapterInfo, device: &ash::Device) -> Result<Self> {
        unsafe {
            Ok(Self {
                graphics: QueuePool {
                    queue: device.get_device_queue(adapter.graphics_queue.index, 0),
                    pool: device.create_command_pool(
                        &vk::CommandPoolCreateInfo::builder()
                            .queue_family_index(adapter.graphics_queue.index),
                        None,
                    )?,
                },
                present: QueuePool {
                    queue: device.get_device_queue(adapter.present_queue.index, 0),
                    pool: device.create_command_pool(
                        &vk::CommandPoolCreateInfo::builder()
                            .queue_family_index(adapter.present_queue.index),
                        None,
                    )?,
                },
                transfer: QueuePool {
                    queue: device.get_device_queue(adapter.transfer_queue.index, 0),
                    pool: device.create_command_pool(
                        &vk::CommandPoolCreateInfo::builder()
                            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
                            .queue_family_index(adapter.transfer_queue.index),
                        None,
                    )?,
                },
            })
        }
    }

    pub(crate) fn allocate_command_buffers(
        &self,
        device: &VtDevice,
        count: u32,
        pool: vk::CommandPool,
    ) -> Result<Vec<vk::CommandBuffer>> {
        let command_buffers = unsafe {
            device.handle.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::builder()
                    .command_pool(pool)
                    .command_buffer_count(count)
                    .level(vk::CommandBufferLevel::PRIMARY),
            )?
        };

        #[cfg(feature = "ext-debug")]
        for command_buffer in command_buffers.iter().copied() {
            device.name_object(
                vk::ObjectType::COMMAND_BUFFER,
                command_buffer,
                "Batched command buffer",
            )?
        }

        Ok(command_buffers)
    }
}

impl VtDevice {
    pub(crate) fn get_transient_transfer_recorder(&self) -> Result<()> {
        let command_buffer =
            self.command_pool
                .allocate_command_buffers(self, 1, self.command_pool.transfer.pool)?[0];

        unsafe {
            self.handle.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }

        Ok(())
    }
}

pub struct VtTransferRecorder<'a> {
    device: ash::Device,
    buffer: vk::CommandBuffer,
    queue: vk::Queue,
    has_been_submited: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl VtTransferRecorder<'_> {
    pub fn copy_buffer_to_buffer() {
        todo!("When buffer creation")
    }

    pub fn submit(&mut self) -> Result<()> {
        unsafe {
            self.device.end_command_buffer(self.buffer)?;
            let fence = self
                .device
                .create_fence(&vk::FenceCreateInfo::builder(), None)?;

            let buffers = [self.buffer];
            let submit_info = [vk::SubmitInfo::builder().command_buffers(&buffers).build()];

            self.device.queue_submit(self.queue, &submit_info, fence)?;
        }

        self.has_been_submited = true;

        Ok(())
    }
}

impl Drop for VtTransferRecorder<'_> {
    fn drop(&mut self) {
        if !self.has_been_submited {
            if cfg!(not(debug_assertions)) {
                error!("A command recorder was never submitted !");
            } else if !std::thread::panicking() {
                panic!("You forgot to submit me sempai !");
            }
        }
    }
}
