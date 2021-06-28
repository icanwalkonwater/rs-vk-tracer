mod builder;

#[cfg(test)]
mod tests {
    use crate::render_graph2::builder::{RenderGraphBuilder, RenderGraphResource, RenderGraphImageSize, RenderGraphPassResourceBindPoint};
    use crate::ash::vk;

    #[test]
    fn test_1() {
        let mut graph_builder = RenderGraphBuilder::new();

        graph_builder.add_resource("Albedo", RenderGraphResource {
            size: RenderGraphImageSize::SwapchainSized,
            format: vk::Format::B8G8R8_SRGB,
        });

        graph_builder.new_pass("Pass Albedo")
            .uses("Albedo", RenderGraphPassResourceBindPoint::ColorAttachment, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .uses("Depth", RenderGraphPassResourceBindPoint::DepthAttachment, vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS);
    }
}