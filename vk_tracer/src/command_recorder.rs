use ash::{version::DeviceV1_0, vk};
use log::error;

use crate::{
    allocation::{DeviceSize, VtBuffer},
    device::VtDevice,
    errors::{Result, VtError},
    physical_device_selection::VtAdapterInfo,
};
use std::sync::{Mutex, MutexGuard};

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
    pub(crate) queues: Vec<Mutex<QueuePool>>,
    graphics: usize,
    present: usize,
    transfer: usize,
}

impl VtCommandPool {
    pub fn new(adapter: &VtAdapterInfo, device: &ash::Device) -> Result<Self> {
        unsafe {
            let mut queues = Vec::new();

            // Push graphics queue
            queues.push(Mutex::new(QueuePool {
                queue: device.get_device_queue(adapter.graphics_queue.index, 0),
                pool: device.create_command_pool(
                    &vk::CommandPoolCreateInfo::builder()
                        .queue_family_index(adapter.graphics_queue.index),
                    None,
                )?,
            }));

            let graphics = 0;

            // Add present queue if different from the graphics one
            let present = if adapter.present_queue.index != adapter.graphics_queue.index {
                queues.push(Mutex::new(QueuePool {
                    queue: device.get_device_queue(adapter.present_queue.index, 0),
                    pool: device.create_command_pool(
                        &vk::CommandPoolCreateInfo::builder()
                            .queue_family_index(adapter.present_queue.index),
                        None,
                    )?,
                }));

                queues.len() - 1
            } else {
                graphics
            };

            // Add transfer queue if different from the other ones
            let transfer = if adapter.transfer_queue.index == adapter.graphics_queue.index {
                graphics
            } else if adapter.transfer_queue.index == adapter.present_queue.index {
                present
            } else {
                queues.push(Mutex::new(QueuePool {
                    queue: device.get_device_queue(adapter.transfer_queue.index, 0),
                    pool: device.create_command_pool(
                        &vk::CommandPoolCreateInfo::builder()
                            .queue_family_index(adapter.transfer_queue.index),
                        None,
                    )?,
                }));

                queues.len() - 1
            };

            Ok(Self {
                queues,
                graphics,
                present,
                transfer,
            })
        }
    }

    pub(crate) fn acquire_graphics(&self) -> MutexGuard<'_, QueuePool> {
        self.queues[self.graphics]
            .lock()
            .expect("Poisoined mutex !")
    }

    pub(crate) fn acquire_present(&self) -> MutexGuard<'_, QueuePool> {
        self.queues[self.present].lock().expect("Poisoined mutex !")
    }

    pub(crate) fn acquire_transfer(&self) -> MutexGuard<'_, QueuePool> {
        self.queues[self.transfer]
            .lock()
            .expect("Poisoined mutex !")
    }

    pub(crate) fn allocate_command_buffers(
        &self,
        device: &VtDevice,
        count: u32,
        pool: &QueuePool,
    ) -> Result<Vec<vk::CommandBuffer>> {
        let command_buffers = unsafe {
            device.handle.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::builder()
                    .command_pool(pool.pool)
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
    pub fn get_transient_transfer_recorder(&self) -> Result<VtTransferRecorder> {
        let queue_pool = self.command_pool.acquire_transfer();

        let command_buffer = self
            .command_pool
            .allocate_command_buffers(self, 1, &queue_pool)?[0];

        unsafe {
            self.handle.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }

        Ok(VtTransferRecorder {
            device: &self,
            pool: queue_pool,
            buffer: command_buffer,
            has_been_submitted: false,
            _marker: Default::default(),
        })
    }
}

pub struct VtTransferRecorder<'a> {
    device: &'a VtDevice,
    pool: MutexGuard<'a, QueuePool>,
    buffer: vk::CommandBuffer,
    has_been_submitted: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl VtTransferRecorder<'_> {
    pub fn copy_buffer_to_buffer<D>(
        &mut self,
        src: &VtBuffer<D>,
        dst: &mut VtBuffer<D>,
    ) -> Result<()> {
        if self.has_been_submitted {
            return Err(VtError::CommandBufferAlreadySubmitted);
        }

        unsafe {
            let region = [vk::BufferCopy::builder()
                .src_offset(src.info.get_offset() as DeviceSize)
                .dst_offset(dst.info.get_offset() as DeviceSize)
                .size(src.info.get_size() as DeviceSize)
                .build()];

            self.device
                .handle
                .cmd_copy_buffer(self.buffer, src.buffer, dst.buffer, &region);
        }

        Ok(())
    }

    pub fn submit(&mut self) -> Result<()> {
        unsafe {
            self.device.handle.end_command_buffer(self.buffer)?;
            let fence = self
                .device
                .handle
                .create_fence(&vk::FenceCreateInfo::builder(), None)?;

            let buffers = [self.buffer];
            let submit_info = [vk::SubmitInfo::builder().command_buffers(&buffers).build()];

            self.device
                .handle
                .queue_submit(self.pool.queue, &submit_info, fence)?;

            let fences = [fence];
            self.device
                .handle
                .wait_for_fences(&fences, true, std::u64::MAX)?;
        }

        self.has_been_submitted = true;

        // Destroy buffer
        unsafe {
            let buffers = [self.buffer];
            self.device
                .handle
                .free_command_buffers(self.pool.pool, &buffers);
        }

        Ok(())
    }
}

impl Drop for VtTransferRecorder<'_> {
    fn drop(&mut self) {
        if !self.has_been_submitted {
            if cfg!(not(debug_assertions)) {
                error!("A command recorder was never submitted !");
            } else if !std::thread::panicking() {
                panic!("You forgot to submit me sempai !");
            }
        }
    }
}
