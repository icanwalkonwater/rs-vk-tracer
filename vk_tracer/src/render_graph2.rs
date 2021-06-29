mod backing;
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

    #[test]
    fn test_1() {
        let mut graph_builder = RenderGraphBuilder::new();

        graph_builder.add_resource(
            "Albedo",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::BackbufferFormat,
        );

        graph_builder.add_resource(
            "Depth",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::DepthStencilOptimal,
        );

        graph_builder
            .new_pass("Pass Albedo")
            .uses(
                "Albedo",
                RenderGraphPassResourceBindPoint::ColorAttachment,
                true,
            )
            .uses(
                "Depth",
                RenderGraphPassResourceBindPoint::DepthAttachment,
                true,
            );

        graph_builder.set_back_buffer("Albedo");
    }
}
