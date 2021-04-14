use ash::vk;
use crate::new::{VkTracerApp, ForwardPipelineHandle};
use crate::new::errors::Result;

pub(crate) mod forward;
pub(crate) mod renderer;

#[derive(Copy, Clone)]
pub enum RenderablePipelineHandle {
    Forward(ForwardPipelineHandle),
}

impl Into<RenderablePipelineHandle> for ForwardPipelineHandle {
    fn into(self) -> RenderablePipelineHandle {
        RenderablePipelineHandle::Forward(self)
    }
}

trait VkRecordable {
    /// Only record bind and draw commands, no begin or end !
    unsafe fn record_commands(&self, app: &VkTracerApp, commands: vk::CommandBuffer) -> Result<()>;
}
