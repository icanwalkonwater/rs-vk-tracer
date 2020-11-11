//! Abstraction to easily manage descriptor sets.
//!
//! You need to query a [VtDescriptorSetManager] from a [VtDevice]
//! and it will automatically allocate the request descriptor sets.
//!
//! The [VtDescriptorSetManager] is thread-safe.
//!
//! You can then use this manager to retrieve your sets for binding.
//!
//! You can also batch-write to them but it will not statically check that
//! you are writing the correct thing in the correct binding.
//! 
//! TODO: Panic when ext-debug is enabled.

use crate::allocation::DeviceSize;
use crate::buffers::{VtRawBufferHandle};
use crate::errors::VtError;
use crate::{device::VtDevice, errors::Result};
use ash::version::DeviceV1_0;
use ash::vk;
use std::collections::HashMap;
use std::pin::Pin;
use std::hash::Hash;
use std::sync::{RwLock, RwLockReadGuard};

// Some aliases from ash
pub type DescriptorType = vk::DescriptorType;
pub type ShaderStage = vk::ShaderStageFlags;

/// Describe a vulkan descriptor set in a simplified way.
/// Used to create a [VtDescriptorSetManager].
#[derive(Clone)]
pub struct DescriptorSetDescription<K: Copy + Eq + Hash> {
    pub key: K,
    pub bindings: Vec<DescriptorSetBindingDescription>,
}

/// Describe a vulkan descriptor set binding in a simplified way.
/// Used withing a [DescriptorSetDescription].
#[derive(Clone)]
pub struct DescriptorSetBindingDescription {
    pub binding: u32,
    pub ty: DescriptorType,
    pub len: usize,
    pub stages: ShaderStage,
}

/// The role of this manager is to allocate, free, and write to descriptor sets.
///
/// It creates a pool with just enough space to create the sets described
/// in the constructor.
///
/// The sets are behind a [RwLock] to allow concurrent writes to them, effectively
/// rendering this struct thread-safe.
pub struct VtDescriptorSetManager<'a, K: Copy + Eq + Hash = &'static str> {
    device: &'a VtDevice,
    pool: vk::DescriptorPool,
    sets: HashMap<K, RwLock<VtDescriptorSet<'a>>>,
}

impl VtDevice {
    /// Create a [VtDescriptorSetManager] tailored for the set descriptions provided.
    pub fn create_descriptor_set_manager<K: Copy + Eq + Hash>(
        &self,
        sets_description: &[DescriptorSetDescription<K>],
    ) -> Result<VtDescriptorSetManager<K>> {
        VtDescriptorSetManager::create_and_allocate(self, sets_description)
    }
}

// Creation internals
impl<K: Copy + Eq + Hash> VtDescriptorSetManager<'_, K> {
    fn create_and_allocate<'a>(
        device: &'a VtDevice,
        sets: &[DescriptorSetDescription<K>],
    ) -> Result<VtDescriptorSetManager<'a, K>> {
        let sets_desc = sets.to_vec();
        let set_names = sets_desc
            .iter()
            .map(|desc| desc.key)
            .collect::<Vec<_>>();

        let sizes = Self::compute_sizes(&sets_desc);

        let pool = unsafe {
            device.handle.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::builder()
                    .max_sets(sets_desc.len() as u32)
                    .pool_sizes(&sizes),
                None,
            )?
        };

        let layouts = {
            let (layouts, _layout_storage) = Self::compute_layouts(&sets_desc);

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
            let sets = device.handle.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )?;

            set_names
                .into_iter()
                .zip(sets.into_iter())
                .zip(layouts.into_iter())
                .zip(sets_desc.into_iter())
                .map(|(((name, set), layout), desc)| {
                    // Binding types is a Vec with empty spaces where there a no bindings.
                    // That way we allow jumps in the binding indices and avoid a HashMap.
                    // But I hate the guy who uses binding=1510 with all my heart.
                    let binding_types = desc.bindings
                        .iter()
                        .fold(Vec::with_capacity(desc.bindings.len()), |mut acc, item| {
                            while acc.len() < (item.binding - 1) as usize {
                                acc.push(DescriptorType::default());
                            }
                            acc[item.binding as usize] = item.ty;
                            acc
                        });

                    (name, RwLock::new(VtDescriptorSet {
                        set,
                        layout,
                        binding_types,
                        #[cfg(feature = "ext-debug")]
                        _desc: desc.bindings,
                        _phantom: Default::default()
                    }))
                })
                .collect()
        };

        Ok(VtDescriptorSetManager { device, pool, sets })
    }

    fn compute_sizes(sets: &[DescriptorSetDescription<K>]) -> Vec<vk::DescriptorPoolSize> {
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
        sets: &[DescriptorSetDescription<K>],
    ) -> (
        Vec<vk::DescriptorSetLayoutCreateInfo>,
        Pin<Box<Vec<Vec<vk::DescriptorSetLayoutBinding>>>>,
    ) {
        let mut layouts = Vec::new();
        // The create info only contain a reference, so we need own them here.
        // Its pinned so the data will not be moved when we return it.
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

/// Represent a valid vulkan descriptor set.
pub struct VtDescriptorSet<'a> {
    set: vk::DescriptorSet,
    layout: vk::DescriptorSetLayout,
    binding_types: Vec<DescriptorType>,
    #[cfg(feature = "ext-debug")]
    _desc: Vec<DescriptorSetBindingDescription>,
    _phantom: std::marker::PhantomData<&'a ()>
}

/// Represent a write operation to a descriptor set binding.
///
/// # Warning
/// No static checks will be performed to see if it is of the correct type.
/// The user is responsible for giving the correct type.
#[derive(Clone)]
pub enum DescriptorSetBindingWriteDescription<'a> {
    Buffer {
        binding: u32,
        buffer: VtRawBufferHandle<'a>,
    },
}

impl<'a, K: Copy + Eq + Hash> VtDescriptorSetManager<'a, K> {
    /// Retrieve an immutable reference to a [VtDescriptorSet].
    ///
    /// Since an [RwLock] is used, this method is thread-safe and will block
    /// if a write is currently in progress.
    ///
    /// Be sure to not keep those references around for too long or else you will prevent
    /// any pending write.
    pub fn retrieve_set<'b>(&'a self, name: &'b K) -> RwLockReadGuard<'a, VtDescriptorSet<'a>> {
        self.sets[name].read().expect("Poisoned descriptor set")
    }

    /// Write to a descriptor set binding.
    ///
    /// This will acquire the lock for this set for the full duration of this function.
    pub fn write_set(&self, set: K, writes_desc: &[DescriptorSetBindingWriteDescription]) {
        // Acquire set
        let set = self.sets[&set].write().expect("Poisoned descriptor set");

        let mut buffer_info_storage = Vec::new();
        let mut buffer_writes = Vec::new();

        // Collect buffer info
        // We can't do everything at once because we need to store a ref to the buffer_info_storage
        // AND mutate it at the next iteration so it doesn't work.
        for write in writes_desc {
            match write {
                DescriptorSetBindingWriteDescription::Buffer { buffer, .. } => {
                    buffer_info_storage.push([
                        vk::DescriptorBufferInfo::builder()
                            .buffer(buffer.buffer)
                            .offset(buffer.info.get_offset() as DeviceSize)
                            .range(buffer.info.get_size() as DeviceSize)
                            .build()
                    ]);

                    buffer_writes.push(write.clone());
                }
            }
        }

        // To store the writes
        let mut writes = Vec::new();

        // Append buffer writes
        for (i, buffer_write) in buffer_writes.into_iter().enumerate() {
            match buffer_write {
                DescriptorSetBindingWriteDescription::Buffer { binding, .. } => {
                    writes.push(
                        vk::WriteDescriptorSet::builder()
                            .dst_set(set.set)
                            .dst_binding(binding)
                            .dst_array_element(0)
                            .descriptor_type(set.binding_types[binding as usize])
                            .buffer_info(&buffer_info_storage[i])
                    );
                }
                _ => unreachable!(),
            }
        }

        // Commit writes
        let writes = writes.into_iter().map(|b| b.build()).collect::<Vec<_>>();
        unsafe {
            self.device.handle.update_descriptor_sets(&writes, &[]);
        }
    }
}

impl<K: Copy + Eq + Hash> Drop for VtDescriptorSetManager<'_, K> {
    fn drop(&mut self) {
        unsafe {
            // Acquire and free sets
            let sets_lock = self.sets.values().map(|set| set.write().unwrap()).collect::<Vec<_>>();
            let sets = sets_lock.iter().map(|s| s.set).collect::<Vec<_>>();
            self.device
                .handle
                .free_descriptor_sets(self.pool, &sets);

            // Destroy layouts
            for layout in sets_lock.iter().map(|s| s.layout) {
                self.device
                    .handle
                    .destroy_descriptor_set_layout(layout, None);
            }

            // Destroy pool
            self.device.handle.destroy_descriptor_pool(self.pool, None);
        }
    }
}
