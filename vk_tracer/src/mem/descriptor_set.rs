use crate::ash::version::DeviceV1_0;
use crate::errors::HandleType;
use crate::errors::Result;
use crate::{DescriptorSetHandle, VkTracerApp, UboHandle};
use ash::vk;
use std::collections::HashMap;
use std::slice::from_ref;

impl VkTracerApp {
    pub fn new_descriptor_sets(&mut self) -> DescriptorPoolBuilder {
        DescriptorPoolBuilder {
            app: self,
            sets: Vec::with_capacity(1),
            sizes: HashMap::new(),
        }
    }

    pub fn write_descriptor_set_ubo(
        &mut self,
        set: DescriptorSetHandle,
        binding: u32,
        ubo: UboHandle,
    ) -> Result<()> {
        let buffer = storage_access!(self.ubo_storage, ubo, HandleType::Ubo);
        unsafe {
            self.device.update_descriptor_sets(
                from_ref(
                    &vk::WriteDescriptorSet::builder()
                        .dst_set(
                            storage_access!(
                                self.descriptor_set_storage,
                                set,
                                HandleType::DescriptorSet
                            )
                            .handle,
                        )
                        .dst_binding(binding)
                        .dst_array_element(0)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(from_ref(&buffer.get_descriptor_buffer_info())),
                ),
                &[],
            )
        }
        Ok(())
    }
}

pub(crate) struct DescriptorPool {
    pub(crate) handle: vk::DescriptorPool,
    pub(crate) sets: Box<[vk::DescriptorSet]>,
}

pub(crate) struct DescriptorSet {
    pub(crate) handle: vk::DescriptorSet,
    pub(crate) layout: vk::DescriptorSetLayout,
}

pub struct DescriptorPoolBuilder<'app> {
    app: &'app mut VkTracerApp,
    sets: Vec<DescriptorSetBuilder>,
    sizes: HashMap<vk::DescriptorType, vk::DescriptorPoolSize>,
}

pub struct DescriptorSetBuilder {
    bindings: Vec<vk::DescriptorSetLayoutBinding>,
}

impl DescriptorPoolBuilder<'_> {
    pub fn new_set(mut self, set: DescriptorSetBuilder) -> Self {
        for binding in set.bindings.iter() {
            self.sizes
                .entry(binding.descriptor_type)
                .or_insert_with(|| vk::DescriptorPoolSize::builder().ty(binding.descriptor_type).build())
                .descriptor_count += 1;
        }
        self.sets.push(set);
        self
    }

    pub fn build(self) -> Result<Box<[DescriptorSetHandle]>> {
        let device = &self.app.device;

        let sizes = self
            .sizes
            .values()
            .map(|size| *size)
            .collect::<Vec<_>>();

        // Allocate pool
        let pool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::builder()
                    .max_sets(self.sets.len() as u32)
                    .pool_sizes(&sizes),
                None,
            )?
        };

        // Allocate set layouts
        let set_layouts = {
            let mut layouts = Vec::with_capacity(self.sets.len());
            for set in &self.sets {
                layouts.push(unsafe {
                    device.create_descriptor_set_layout(
                        &vk::DescriptorSetLayoutCreateInfo::builder().bindings(&set.bindings),
                        None,
                    )?
                });
            }
            layouts
        };

        // Allocate sets
        let sets = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(pool)
                    .set_layouts(&set_layouts),
            )?
        };

        // Register all that
        let set_handles = sets
            .iter()
            .zip(set_layouts)
            .map(|(set, layout)| {
                self.app.descriptor_set_storage.insert(DescriptorSet {
                    handle: *set,
                    layout,
                })
            })
            .collect::<Box<_>>();

        self.app.descriptor_pool_storage.insert(DescriptorPool {
            handle: pool,
            sets: sets.into_boxed_slice(),
        });

        Ok(set_handles)
    }
}

impl DescriptorSetBuilder {
    pub fn new() -> Self {
        Self {
            bindings: Default::default(),
        }
    }

    pub fn raw_binding(
        mut self,
        ty: vk::DescriptorType,
        binding: u32,
        len: u32,
        stage_flags: vk::ShaderStageFlags,
    ) -> Self {
        self.bindings.push(
            vk::DescriptorSetLayoutBinding::builder()
                .descriptor_type(ty)
                .binding(binding)
                .descriptor_count(len)
                .stage_flags(stage_flags)
                .build(),
        );
        self
    }

    #[inline]
    pub fn ubo(self, binding: u32, stage_flags: vk::ShaderStageFlags) -> Self {
        self.raw_binding(vk::DescriptorType::UNIFORM_BUFFER, binding, 1, stage_flags)
    }

    #[inline]
    pub fn sampler(self, binding: u32, stage_flags: vk::ShaderStageFlags) -> Self {
        self.raw_binding(vk::DescriptorType::SAMPLER, binding, 1, stage_flags)
    }
}
