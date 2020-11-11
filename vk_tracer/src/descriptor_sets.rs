use crate::allocation::DeviceSize;
use crate::buffers::VtBuffer;
use crate::errors::VtError;
use crate::{device::VtDevice, errors::Result};
use ash::version::DeviceV1_0;
use ash::vk;
use std::collections::HashMap;
use std::pin::Pin;

pub type DescriptorType = vk::DescriptorType;
pub type ShaderStage = vk::ShaderStageFlags;

#[derive(Clone)]
pub struct DescriptorSetDescription {
    pub set: u32,
    pub bindings: Vec<DescriptorSetBindingDescription>,
}

#[derive(Clone)]
pub struct DescriptorSetBindingDescription {
    pub binding: u32,
    pub ty: DescriptorType,
    pub len: usize,
    pub stages: ShaderStage,
}

pub struct VtDescriptorSetManager<'a> {
    device: &'a VtDevice,
    pool: vk::DescriptorPool,
    layouts: Vec<vk::DescriptorSetLayout>,
    sets: Vec<vk::DescriptorSet>,
}

impl VtDevice {
    /// Create a [VtDescriptorSetManager] tailored for the set descriptions provided.
    pub fn create_descriptor_set_manager(
        &self,
        sets_description: &[DescriptorSetDescription],
    ) -> Result<VtDescriptorSetManager> {
        VtDescriptorSetManager::create_and_allocate(self, sets_description)
    }
}

// Creation
impl VtDescriptorSetManager<'_> {
    fn create_and_allocate<'a>(
        device: &'a VtDevice,
        sets: &'_ [DescriptorSetDescription],
    ) -> Result<VtDescriptorSetManager<'a>> {
        let mut sets = sets.to_vec();
        sets.sort_by_key(|desc| desc.set);

        // Sanity check
        if sets.windows(2).any(|win| win[0].set != win[1].set - 1) {
            return Result::Err(VtError::MalformedDescriptorSetsDescription);
        }

        let sizes = Self::compute_sizes(&sets);

        let pool = unsafe {
            device.handle.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::builder()
                    .max_sets(sets.len() as u32)
                    .pool_sizes(&sizes),
                None,
            )?
        };

        let layouts = {
            let (layouts, _layout_storage) = Self::compute_layouts(&sets);

            layouts
                .into_iter()
                .map(|layout| unsafe {
                    device
                        .handle
                        .create_descriptor_set_layout(&layout, None)
                        .map_err(|e| VtError::Vulkan(e))
                })
                .collect::<Result<Vec<_>>>()?
        };

        let sets = unsafe {
            device.handle.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )?
        };

        Ok(VtDescriptorSetManager {
            device,
            pool,
            layouts,
            sets,
        })
    }

    fn compute_sizes(sets: &[DescriptorSetDescription]) -> Vec<vk::DescriptorPoolSize> {
        let mut sizes = HashMap::new();

        for set in sets {
            for binding in &set.bindings {
                sizes.entry(binding.ty).and_modify(|s| *s += 1).or_insert(1);
            }
        }

        sizes
            .into_iter()
            .map(|(ty, count)| {
                vk::DescriptorPoolSize::builder()
                    .ty(ty)
                    .descriptor_count(count)
                    .build()
            })
            .collect()
    }

    fn compute_layouts(
        sets: &[DescriptorSetDescription],
    ) -> (
        Vec<vk::DescriptorSetLayoutCreateInfo>,
        Pin<Box<Vec<Vec<vk::DescriptorSetLayoutBinding>>>>,
    ) {
        let mut layouts = Vec::new();
        let mut bindings_storage = Box::pin(Vec::new());

        for set in sets {
            let mut bindings = Vec::new();

            for binding in &set.bindings {
                bindings.push(
                    vk::DescriptorSetLayoutBinding::builder()
                        .binding(binding.binding)
                        .descriptor_type(binding.ty)
                        .descriptor_count(binding.len as u32)
                        .stage_flags(binding.stages)
                        .build(),
                );
            }

            bindings_storage.push(bindings);

            // WARNING: we build it to avoid lifetime things but only because we know its safe:
            // the array is pinned in memory and will be return to the caller so it will not be dropped rn.
            layouts.push(
                vk::DescriptorSetLayoutCreateInfo::builder()
                    .bindings(bindings_storage.last().unwrap())
                    .build(),
            );
        }

        (layouts, bindings_storage)
    }
}

#[derive(Clone)]
pub enum DescriptorSetBindingWriteDescription<'a, D> {
    Buffer {
        set: u32,
        binding: u32,
        ty: DescriptorType,
        buffer: VtBuffer<'a, 'a, D>,
    },
}

impl VtDescriptorSetManager<'_> {
    pub fn write<'a, D>(
        &mut self,
        writes: &[DescriptorSetBindingWriteDescription<'a, D>],
    ) {
        let mut buffer_info_storage = Vec::new();
        let mut buffer_writes = Vec::new();

        // Collect buffer info
        for write in writes {
            match write {
                DescriptorSetBindingWriteDescription::Buffer { buffer, .. } => {
                    let data = buffer.as_ref();
                    buffer_info_storage.push([vk::DescriptorBufferInfo::builder()
                        .buffer(data.buffer)
                        .offset(data.info.get_offset() as DeviceSize)
                        .range(data.info.get_size() as DeviceSize)
                        .build()]);

                    buffer_writes.push(write);
                }
            }
        }

        // Build writes
        let mut writes = Vec::new();

        // Build buffer writes
        for (i, &buffer_write) in buffer_writes.iter().enumerate() {
            match buffer_write {
                DescriptorSetBindingWriteDescription::Buffer {
                    set, binding, ty, ..
                } => {
                    writes.push(
                        vk::WriteDescriptorSet::builder()
                            .dst_set(self.sets[*set as usize])
                            .dst_binding(*binding)
                            .dst_array_element(0)
                            .descriptor_type(*ty)
                            .buffer_info(&buffer_info_storage[i]),
                    );
                },
                _ => unreachable!()
            }
        }

        // Finish writes and actually write
        let writes = writes.into_iter().map(|b| b.build()).collect::<Vec<_>>();
        unsafe {
            self.device.handle.update_descriptor_sets(&writes, &[]);
        }
    }
}

impl Drop for VtDescriptorSetManager<'_> {
    fn drop(&mut self) {
        unsafe {
            // Free sets
            self.device
                .handle
                .free_descriptor_sets(self.pool, &self.sets);
            // Destroy layouts
            for layout in &self.layouts {
                self.device
                    .handle
                    .destroy_descriptor_set_layout(*layout, None);
            }

            // Destroy pool
            self.device.handle.destroy_descriptor_pool(self.pool, None);
        }
    }
}
