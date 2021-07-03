mod baking;
mod builder;
mod allocation;

#[derive(Copy, Clone, Debug)]
pub enum GraphValidationError {
    NoBackBuffer,
    InvalidBackBuffer,
    TagNotRegistered,
    ColorOrInputAttachmentDifferInSize,
    InputAndOutputDepthAttachmentsDiffer,
    LogicalResourceWrittenMoreThanOnce,
    ReadModifyWriteWrongBindPoint,
}

#[cfg(test)]
mod tests {
    use crate::ash::vk;
    use crate::render_graph2::builder::{RenderGraphBuilder, RenderGraphImageFormat, RenderGraphImageSize, RenderGraphPassResourceBindPoint, RenderGraphResource, RenderGraphResourcePersistence};
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
            RenderGraphResourcePersistence::ClearInputPreserveOutput,
        );

        graph_builder.add_resource(
            "Depth",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::DepthStencilOptimal,
            RenderGraphResourcePersistence::Transient,
        );

        graph_builder
            .new_pass("Pass Albedo")
            .uses("Albedo", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Depth", RenderGraphPassResourceBindPoint::DepthAttachment);

        graph_builder.set_back_buffer("Albedo");

        let mut valid_graph = graph_builder.finalize_and_validate().unwrap();
        let baked_graph = BakedRenderGraph::bake(&mut valid_graph).unwrap();
    }

    #[test]
    fn test_complex() {
        let mut graph_builder = RenderGraphBuilder::new();

        graph_builder.add_resource(
            "Albedo",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::BackbufferFormat,
            RenderGraphResourcePersistence::Transient,
        );
        graph_builder.add_resource(
            "Depth",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::DepthStencilOptimal,
            RenderGraphResourcePersistence::ClearInput,
        );
        graph_builder.add_resource(
            "Position",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::ColorRgba16Sfloat,
            RenderGraphResourcePersistence::Transient,
        );
        graph_builder.add_resource(
            "Normal",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::ColorRgba16Sfloat,
            RenderGraphResourcePersistence::Transient,
        );
        graph_builder.add_resource(
            "Color",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::BackbufferFormat,
            RenderGraphResourcePersistence::Transient,
        );
        graph_builder.add_resource(
            "Final",
            RenderGraphImageSize::BackbufferSized,
            RenderGraphImageFormat::BackbufferFormat,
            RenderGraphResourcePersistence::ClearInputPreserveOutput,
        );

        graph_builder
            .new_pass("Geometry pass")
            .uses("Albedo", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Position", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Normal", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Depth", RenderGraphPassResourceBindPoint::DepthAttachment);
        graph_builder
            .new_pass("Lighting pass")
            .uses("Color", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Albedo", RenderGraphPassResourceBindPoint::InputAttachment)
            .uses("Position", RenderGraphPassResourceBindPoint::InputAttachment)
            .uses("Normal", RenderGraphPassResourceBindPoint::InputAttachment)
            .uses("Depth", RenderGraphPassResourceBindPoint::InputAttachment)
            .allow_read_modify_write("Albedo", "Color");
        graph_builder
            .new_pass("Post process pass")
            .uses("Final", RenderGraphPassResourceBindPoint::ColorAttachment)
            .uses("Color", RenderGraphPassResourceBindPoint::InputAttachment)
            .allow_read_modify_write("Color", "Final");

        graph_builder.set_back_buffer("Final");

        let mut valid_graph = graph_builder.finalize_and_validate().unwrap();
        let baked_graph = BakedRenderGraph::bake(&mut valid_graph).unwrap();
    }
}
