mod attachments;
mod baking;
mod graph;

pub use attachments::*;
pub use baking::*;
pub use graph::*;

#[cfg(test)]
mod tests {
    use super::*;
    use ash::vk;

    #[test]
    fn yolo() {
        let mut graph = RenderGraph::new();

        graph.register_attachment(
            "Albedo",
            AttachmentInfo {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::R8G8B8A8_UNORM,
                transient: false,
            },
        );

        graph.register_attachment(
            "Position",
            AttachmentInfo {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::R16G16B16A16_SFLOAT,
                transient: false,
            },
        );

        graph.register_attachment(
            "Normal",
            AttachmentInfo {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::R16G16B16A16_SFLOAT,
                transient: false,
            },
        );

        graph.register_attachment(
            "Depth",
            AttachmentInfo {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::D32_SFLOAT,
                transient: true,
            },
        );

        graph.register_attachment(
            "ShadowMap",
            AttachmentInfo {
                size: AttachmentSize::Fixed(vk::Extent3D {
                    width: 1024,
                    height: 1024,
                    depth: 1,
                }),
                format: vk::Format::D32_SFLOAT,
                transient: false,
            },
        );

        graph.register_attachment(
            "Shaded",
            AttachmentInfo {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::R8G8B8_SRGB,
                transient: false,
            },
        );

        graph.register_attachment(
            "ToneMap",
            AttachmentInfo {
                size: AttachmentSize::Fixed(vk::Extent3D {
                    width: 256,
                    height: 256,
                    depth: 1,
                }),
                format: vk::Format::R8G8B8_UNORM,
                transient: false,
            },
        );

        graph.register_attachment(
            "Swapchain",
            AttachmentInfo {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::R8G8B8_SRGB,
                transient: false,
            },
        );

        graph
            .new_pass("GBuffer", RenderPassType::Graphics)
            .add_color_attachment("Albedo")
            .add_color_attachment("Position")
            .add_color_attachment("Normal")
            .set_depth_stencil_output("Depth");

        graph
            .new_pass("Shadow Pass", RenderPassType::Graphics)
            .set_depth_stencil_output("ShadowMap");

        graph
            .new_pass("Lighting", RenderPassType::Graphics)
            .add_color_attachment("Shaded")
            .add_input_attachment("Albedo")
            .add_input_attachment("Position")
            .add_input_attachment("Normal")
            .add_input_attachment("Depth")
            .add_image_input("ShadowMap");

        graph
            .new_pass("Post Processing", RenderPassType::Graphics)
            .add_color_attachment("Swapchain")
            .add_input_attachment("Shaded")
            .add_image_input("ToneMap");

        graph.set_back_buffer("Swapchain");

        graph.dump().unwrap();
        bake(graph).unwrap();
    }
}
