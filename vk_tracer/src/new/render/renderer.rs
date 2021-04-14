use crate::{
    command_recorder::QueueType,
    new::{
        errors::{HandleType, Result, VkTracerError},
        render::{RenderablePipelineHandle, VkRecordable},
        RenderPlanHandle, RenderTargetHandle, RendererHandle, VkTracerApp,
    },
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
            clear_color: vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 0.0],
                },
            },
            current_subpass: 0,
            pipelines_by_subpass: vec![Vec::with_capacity(1)],
            pipelines_amount: 0,
        }
    }
}

pub(crate) struct Renderer {
    pub(crate) commands: vk::CommandBuffer,
    pub(crate) render_fence: vk::Fence,
}

pub struct RendererBuilder<'app> {
    app: &'app mut VkTracerApp,
    render_plan: RenderPlanHandle,
    render_target: RenderTargetHandle,
    clear_color: vk::ClearValue,
    current_subpass: usize,
    pipelines_by_subpass: Vec<Vec<RenderablePipelineHandle>>,
    pipelines_amount: u32,
}

impl RendererBuilder<'_> {
    pub fn clear_color(mut self, color: [f32; 4]) -> Self {
        self.clear_color = vk::ClearValue {
            color: vk::ClearColorValue { float32: color },
        };
        self
    }

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

    pub fn build(self) -> Result<RendererHandle> {
        let render_plan = self
            .app
            .render_plan_storage
            .get(self.render_plan)
            .ok_or(VkTracerError::InvalidHandle(HandleType::RenderPlan))?;
        let render_target = self
            .app
            .render_target_storage
            .get(self.render_target)
            .ok_or(VkTracerError::InvalidHandle(HandleType::RenderTarget))?;

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
                                let pipeline =
                                    self.app.forward_pipeline_storage.get(handle).ok_or(
                                        VkTracerError::InvalidHandle(HandleType::ForwardPipeline),
                                    )?;
                                pipeline.record_commands(self.app, commands)?;
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

            let clear_values = std::iter::repeat(self.clear_color)
                .take(render_plan.attachments.len())
                .collect::<Vec<_>>();
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
                    .clear_values(&clear_values),
                &vk::SubpassBeginInfo::builder()
                    .contents(vk::SubpassContents::SECONDARY_COMMAND_BUFFERS),
            );

            loop {
                let subpass_commands = secondary_commands_by_subpass.pop().unwrap();
                device.cmd_execute_commands(top_level_commands, &subpass_commands);

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
            top_level_commands
        };

        // Create the fence already signaled because otherwise we will block infinitely when rendering for the first time
        let render_fence = unsafe { device.create_fence(&vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED), None)? };

        Ok(self.app.renderer_storage.insert(Renderer { commands, render_fence }))
    }
}
