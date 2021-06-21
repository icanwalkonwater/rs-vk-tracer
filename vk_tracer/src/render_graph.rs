mod attachments;
mod graph;

pub use attachments::*;
pub use graph::*;

#[cfg(test)]
mod tests {
    use super::*;
    use ash::vk;

    #[test]
    fn yolo() {
        let mut graph = RenderGraph::new();

        graph.register_attachment("swapchain", AttachmentInfo {
            size: AttachmentSize::SwapchainRelative,
            format: vk::Format::R8G8B8_SRGB,
            transient: false,
        });

        graph.register_attachment("albedo", AttachmentInfo {
            size: AttachmentSize::SwapchainRelative,
            format: vk::Format::R8G8B8A8_SRGB,
            transient: false,
        });

        graph.register_attachment("normal", AttachmentInfo {
            size: AttachmentSize::SwapchainRelative,
            format: vk::Format::A2B10G10R10_UNORM_PACK32,
            transient: false,
        });

        graph.register_attachment("depth", AttachmentInfo {
            size: AttachmentSize::SwapchainRelative,
            format: vk::Format::D32_SFLOAT,
            transient: true,
        });

        graph.new_pass("g_buffer", RenderPassType::Graphics)
            .add_color_output("albedo")
            .add_color_output("normal")
            .set_depth_stencil_output("depth");

        graph.new_pass("lighting", RenderPassType::Graphics)
            .add_color_input_output("albedo", "swapchain")
            .add_attachment_input("normal")
            .set_depth_stencil_input("depth");

        graph.set_back_buffer("swapchain");

        graph.dump();
    }
}
