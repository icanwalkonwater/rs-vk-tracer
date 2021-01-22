use crate::renderer_creator::RendererCreator;
use std::sync::Arc;
use ash::Device;
use ash::version::DeviceV1_0;

pub struct ForwardRenderer {
    pub(crate) creator: Arc<RendererCreator>,
}

impl ForwardRenderer {
    pub(crate) fn new(device: &Device) {
        // device.create_graphics_pipelines()
    }
}
