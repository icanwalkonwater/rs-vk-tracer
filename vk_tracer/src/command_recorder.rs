use ash::{version::DeviceV1_0, vk};
use log::warn;

use crate::{
    allocation::DeviceSize,
    buffers::{VtBuffer, VtBufferMut},
    device::VtDevice,
    errors::Result,
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

pub(crate) struct QueuePool {
    pub queue: Mutex<vk::Queue>,
    pub pool: Mutex<vk::CommandPool>,
}

pub(crate) struct VtCommandPool {
    pub(crate) queues: Vec<QueuePool>,
    graphics: usize,
    present: usize,
    transfer: usize,
}

impl VtCommandPool {
    pub fn new(adapter: &VtAdapterInfo, device: &ash::Device) -> Result<Self> {
        unsafe {
            let mut queues = Vec::new();

            // Push graphics queue
            queues.push(QueuePool {
                queue: Mutex::new(device.get_device_queue(adapter.graphics_queue.index, 0)),
                pool: Mutex::new(
                    device.create_command_pool(
                        &vk::CommandPoolCreateInfo::builder()
                            .queue_family_index(adapter.graphics_queue.index),
                        None,
                    )?,
                ),
            });

            let graphics = 0;

            // Add present queue if different from the graphics one
            let present = if adapter.present_queue.index != adapter.graphics_queue.index {
                queues.push(QueuePool {
                    queue: Mutex::new(device.get_device_queue(adapter.present_queue.index, 0)),
                    pool: Mutex::new(
                        device.create_command_pool(
                            &vk::CommandPoolCreateInfo::builder()
                                .queue_family_index(adapter.present_queue.index),
                            None,
                        )?,
                    ),
                });

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
                queues.push(QueuePool {
                    queue: Mutex::new(device.get_device_queue(adapter.transfer_queue.index, 0)),
                    pool: Mutex::new(
                        device.create_command_pool(
                            &vk::CommandPoolCreateInfo::builder()
                                .queue_family_index(adapter.transfer_queue.index),
                            None,
                        )?,
                    ),
                });

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

    pub(crate) fn graphics(&self) -> &QueuePool {
        &self.queues[self.graphics]
    }

    pub(crate) fn present(&self) -> &QueuePool {
        &self.queues[self.present]
    }

    pub(crate) fn transfer(&self) -> &QueuePool {
        &self.queues[self.transfer]
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
    pub fn get_transient_transfer_recorder(&self) -> Result<VtTransferRecorder> {
        let queue_pool = self.command_pool.transfer();
        let pool = queue_pool.pool.lock().expect("Poisoned");

        let command_buffer = self.command_pool.allocate_command_buffers(self, 1, *pool)?[0];

        unsafe {
            self.handle.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }

        Ok(VtTransferRecorder {
            device: &self,
            queue_pool,
            _pool: pool,
            buffer: command_buffer,
            has_been_ended: false,
            _marker: Default::default(),
        })
    }
}

pub trait VtTransferCommands<'a>: Sized {
    fn copy_buffer_to_buffer<'b, D: 'b>(
        &mut self,
        src: impl Into<VtBuffer<'b, 'a, D>>,
        dst: impl Into<VtBufferMut<'b, 'a, D>>,
    ) -> Result<()>
    where
        'b: 'a;
}

pub struct VtTransferRecorder<'a> {
    device: &'a VtDevice,
    queue_pool: &'a QueuePool,
    _pool: MutexGuard<'a, vk::CommandPool>,
    buffer: vk::CommandBuffer,
    #[cfg(feature = "ext-debug")]
    has_been_ended: bool,
    // Mark !Sync + !Send
    _marker: std::marker::PhantomData<std::cell::UnsafeCell<()>>,
}

impl<'a> VtTransferCommands<'a> for VtTransferRecorder<'a> {
    fn copy_buffer_to_buffer<'b, D: 'b>(
        &mut self,
        src: impl Into<VtBuffer<'b, 'a, D>>,
        dst: impl Into<VtBufferMut<'b, 'a, D>>,
    ) -> Result<()>
    where
        'b: 'a, {
        let src = src.into();
        let src = src.data();

        let mut dst = dst.into();
        let dst = dst.data_mut();

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
}

impl<'a> VtTransferRecorder<'a> {
    pub fn finish(mut self) -> Result<VtRecorderFinished<'a>> {
        unsafe {
            self.device.handle.end_command_buffer(self.buffer)?;
        }

        self.has_been_ended = true;

        Ok(VtRecorderFinished {
            device: self.device,
            pool: self.queue_pool,
            buffer: self.buffer,
        })
    }
}

#[cfg(feature = "ext-debug")]
impl Drop for VtTransferRecorder<'_> {
    fn drop(&mut self) {
        if !self.has_been_ended {
            warn!("Recorded was never ended :");
        }
    }
}

pub struct VtRecorderFinished<'a> {
    device: &'a VtDevice,
    pool: &'a QueuePool,
    buffer: vk::CommandBuffer,
}

impl VtRecorderFinished<'_> {
    pub fn submit(&mut self) -> Result<()> {
        unsafe {
            let fence = self
                .device
                .handle
                .create_fence(&vk::FenceCreateInfo::builder(), None)?;

            let buffers = [self.buffer];
            let submit_info = [vk::SubmitInfo::builder().command_buffers(&buffers).build()];

            let queue = self
                .device
                .command_pool
                .transfer()
                .queue
                .lock()
                .expect("Poisoned mutex");

            self.device
                .handle
                .queue_submit(*queue, &submit_info, fence)?;

            let fences = [fence];
            self.device
                .handle
                .wait_for_fences(&fences, true, std::u64::MAX)?;
        }

        Ok(())
    }
}

impl Drop for VtRecorderFinished<'_> {
    fn drop(&mut self) {
        // Destroy buffer
        unsafe {
            let buffers = [self.buffer];

            // Doesn't return a Result for some reason, why not
            self.device
                .handle
                .free_command_buffers(*self.pool.pool.lock().expect("Poisoned"), &buffers);
        }
    }
}
