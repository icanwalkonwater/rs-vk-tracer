use crate::{
    command_recorder::QueueType,
    errors::{HandleType, Result},
    render::{RenderablePipelineHandle, VkRecordable},
    RenderPlanHandle, RenderTargetHandle, RendererHandle, VkTracerApp,
};
use ash::{
    version::{DeviceV1_0, DeviceV1_2},
    vk,
};

impl VkTracerApp {
    pub fn new_renderer_from_plan(
        &mut self,
        render_plan: RenderPlanHandle,
        render_target: RenderTargetHandle,
    ) -> RendererBuilder {
        RendererBuilder {
            app: self,
            render_plan,
            render_target,
            current_subpass: 0,
            pipelines_by_subpass: vec![Vec::with_capacity(1)],
            pipelines_amount: 0,
        }
    }

    pub fn recreate_renderer(
        &mut self,
        renderer: RendererHandle,
        render_target: RenderTargetHandle,
    ) -> Result<()> {
        // We do this like that because otherwise the builder can't borrow &mut self
        let (render_plan, pipelines_by_subpass, pipelines_amount) = {
            let renderer =
                storage_access_mut!(self.renderer_storage, renderer, HandleType::Renderer);

            // Destroy old
            unsafe {
                let pool = self.command_pools.get(&QueueType::Graphics).unwrap().1;
                self.device
                    .free_command_buffers(pool, &[renderer.main_commands]);
                self.device
                    .free_command_buffers(pool, &renderer.secondary_commands);
                self.device.destroy_fence(renderer.render_fence, None);
            }

            (
                renderer.render_plan,
                std::mem::take(&mut renderer.pipelines_by_subpass),
                renderer.pipelines_amount,
            )
        };

        let builder = RendererBuilder {
            app: self,
            render_plan,
            render_target,
            current_subpass: 0,
            pipelines_by_subpass,
            pipelines_amount,
        };
        let ((main_commands, secondary_commands), fence) = builder.inner_build()?;
        let pipelines_by_subpass = builder.pipelines_by_subpass;

        let renderer = storage_access_mut!(self.renderer_storage, renderer, HandleType::Renderer);
        renderer.pipelines_by_subpass = pipelines_by_subpass;
        renderer.main_commands = main_commands;
        renderer.secondary_commands = secondary_commands;
        renderer.render_fence = fence;

        Ok(())
    }
}

pub(crate) struct Renderer {
    pub(crate) main_commands: vk::CommandBuffer,
    secondary_commands: Box<[vk::CommandBuffer]>,
    pub(crate) render_fence: vk::Fence,

    // For recreation
    render_plan: RenderPlanHandle,
    pipelines_by_subpass: Vec<Vec<RenderablePipelineHandle>>,
    pipelines_amount: u32,
}

pub struct RendererBuilder<'app> {
    app: &'app mut VkTracerApp,
    render_plan: RenderPlanHandle,
    render_target: RenderTargetHandle,
    current_subpass: usize,
    pipelines_by_subpass: Vec<Vec<RenderablePipelineHandle>>,
    pipelines_amount: u32,
}

type RendererData = ((vk::CommandBuffer, Box<[vk::CommandBuffer]>), vk::Fence);
impl RendererBuilder<'_> {
    pub fn execute_pipeline(mut self, pipeline: RenderablePipelineHandle) -> Self {
        self.pipelines_by_subpass[self.current_subpass].push(pipeline);
        self.pipelines_amount += 1;
        self
    }

    pub fn next_subpass(mut self) -> Self {
        self.pipelines_by_subpass.push(Vec::with_capacity(1));
        self.current_subpass += 1;
        self
    }

    fn inner_build(&self) -> Result<RendererData> {
        let render_plan = storage_access!(
            self.app.render_plan_storage,
            self.render_plan,
            HandleType::RenderPlan
        );
        let render_target = storage_access!(
            self.app.render_target_storage,
            self.render_target,
            HandleType::RenderTarget
        );

        let device = &self.app.device;
        let pool = self.app.command_pools.get(&QueueType::Graphics).unwrap();

        let commands = unsafe {
            // Record secondary command buffers

            let mut secondary_commands_by_subpass = {
                // Allocate all the command buffer necessary for all subpasses
                let mut command_pool = device.allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::builder()
                        .command_pool(pool.1)
                        .level(vk::CommandBufferLevel::SECONDARY)
                        .command_buffer_count(self.pipelines_amount as u32),
                )?;

                let mut commands_by_subpass = Vec::with_capacity(self.pipelines_by_subpass.len());

                // Iterate through each subpass and record a command buffer at a time
                for (i, subpass) in self.pipelines_by_subpass.iter().enumerate() {
                    let mut subpass_commands = Vec::with_capacity(subpass.len());

                    for pipeline in subpass.iter().copied() {
                        // Take a command buffer from the stash
                        let commands = command_pool.pop().unwrap();

                        device.begin_command_buffer(
                            commands,
                            &vk::CommandBufferBeginInfo::builder()
                                .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                                .inheritance_info(
                                    &vk::CommandBufferInheritanceInfo::builder()
                                        .render_pass(render_plan.render_pass)
                                        .subpass(i as u32)
                                        .framebuffer(render_target.framebuffer),
                                ),
                        )?;

                        match pipeline {
                            RenderablePipelineHandle::Forward(handle) => {
                                let pipeline = storage_access!(
                                    self.app.forward_pipeline_storage,
                                    handle,
                                    HandleType::ForwardPipeline
                                );
                                pipeline.record_commands(
                                    self.app,
                                    render_target.extent,
                                    commands,
                                )?;
                            }
                        }

                        device.end_command_buffer(commands)?;
                        subpass_commands.push(commands);
                    }
                    commands_by_subpass.push(subpass_commands);
                }
                commands_by_subpass
            };

            // Record top level command buffer

            let top_level_commands = device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::builder()
                    .command_pool(pool.1)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0];

            device
                .begin_command_buffer(top_level_commands, &vk::CommandBufferBeginInfo::default())?;

            device.cmd_begin_render_pass2(
                top_level_commands,
                &vk::RenderPassBeginInfo::builder()
                    .render_pass(render_plan.render_pass)
                    .framebuffer(render_target.framebuffer)
                    .render_area(
                        vk::Rect2D::builder()
                            .offset(vk::Offset2D::default())
                            .extent(render_target.extent)
                            .build(),
                    )
                    .clear_values(&render_plan.clear_values),
                &vk::SubpassBeginInfo::builder()
                    .contents(vk::SubpassContents::SECONDARY_COMMAND_BUFFERS),
            );

            let mut secondary_commands = Vec::with_capacity(self.pipelines_amount as usize);
            loop {
                let subpass_commands = secondary_commands_by_subpass.pop().unwrap();
                device.cmd_execute_commands(top_level_commands, &subpass_commands);
                secondary_commands.extend(subpass_commands);

                if secondary_commands_by_subpass.is_empty() {
                    break;
                }

                device.cmd_next_subpass2(
                    top_level_commands,
                    &vk::SubpassBeginInfo::builder()
                        .contents(vk::SubpassContents::SECONDARY_COMMAND_BUFFERS),
                    &vk::SubpassEndInfo::default(),
                );
            }

            device.cmd_end_render_pass2(top_level_commands, &vk::SubpassEndInfo::default());

            device.end_command_buffer(top_level_commands)?;
            (top_level_commands, secondary_commands.into_boxed_slice())
        };

        // Create the fence already signaled because otherwise we will block infinitely when rendering for the first time
        let render_fence = unsafe {
            device.create_fence(
                &vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )?
        };

        Ok((commands, render_fence))
    }

    pub fn build(self) -> Result<RendererHandle> {
        let (commands, render_fence) = self.inner_build()?;

        Ok(self.app.renderer_storage.insert(Renderer {
            main_commands: commands.0,
            secondary_commands: commands.1,
            render_fence,
            render_plan: self.render_plan,
            pipelines_by_subpass: self.pipelines_by_subpass,
            pipelines_amount: self.pipelines_amount,
        }))
    }
}
