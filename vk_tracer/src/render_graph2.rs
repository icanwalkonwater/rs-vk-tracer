mod baking;
mod builder;

#[derive(Copy, Clone, Debug)]
pub enum GraphValidationError {
    NoBackBuffer,
    InvalidBackBuffer,
    TagNotRegistered,
    ColorOrInputAttachmentDifferInSize,
    InputAndOutputDepthAttachmentsDiffer,
    LogicalResourceWrittenMoreThanOnce,
}

#[cfg(test)]
mod tests {
    use crate::ash::vk;
    use crate::render_graph2::builder::{
        RenderGraphBuilder, RenderGraphImageFormat, RenderGraphImageSize,
        RenderGraphPassResourceBindPoint, RenderGraphResource,
    };
    use crate::render_graph2::baking::BakedRenderGraph;

    #[test]
    fn test_1() {
        let mut graph_builder = RenderGraphBuilder::new();

        graph_builder.add_resource(
            "Albedo",
            RenderGraphImageSize::Fixed(vk::Extent3D {
                width: 1920,
                height: 1080,
                depth: 0,
            }),
            RenderGraphImageFormat::ColorRgba8Unorm,
        );

        graph_builder.add_resource(
            "Depth",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::DepthStencilOptimal,
        );

        graph_builder
            .new_pass("Pass Albedo")
            .uses("Albedo", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Depth", RenderGraphPassResourceBindPoint::DepthAttachment);

        graph_builder.set_back_buffer("Albedo");

        let valid_graph = graph_builder.finalize_and_validate().unwrap();
        let baked_graph = BakedRenderGraph::bake(&valid_graph).unwrap();
    }

    #[test]
    fn test_complex() {
        let mut graph_builder = RenderGraphBuilder::new();

        graph_builder.add_resource(
            "Albedo",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::BackbufferFormat,
        );
        graph_builder.add_resource(
            "Depth",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::BackbufferFormat,
        );
        graph_builder.add_resource(
            "Final",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::BackbufferFormat,
        );

        graph_builder
            .new_pass("Forward pass")
            .uses("Albedo", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Depth", RenderGraphPassResourceBindPoint::DepthAttachment);
        graph_builder
            .new_pass("Post process")
            .uses("Final", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Albedo", RenderGraphPassResourceBindPoint::Sampler);

        graph_builder.set_back_buffer("Final");

        let valid_graph = graph_builder.finalize_and_validate().unwrap();
        let baked_graph = BakedRenderGraph::bake(&valid_graph).unwrap();
    }
}
