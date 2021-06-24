mod attachments;
mod baking;
mod graph;

pub use attachments::*;
pub use baking::*;
pub use graph::*;
use std::fmt::{Debug, Display};
use std::hash::Hash;

pub trait GraphTag: Copy + Clone + Eq + PartialEq + Hash + Debug + Display + 'static {}

impl GraphTag for &'static str {}

#[cfg(test)]
mod tests {
    use super::*;
    use ash::vk;

    #[test]
    fn test_graph_simple() {
        let mut graph = RenderGraph::new();

        graph.register_attachment(
            "Swapchain",
            AttachmentInfo {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::R8G8B8_UNORM,
                transient: false,
            }
        );

        graph.register_attachment(
            "Depth",
            AttachmentInfo {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::D32_SFLOAT,
                transient: true,
            }
        );

        graph.new_pass("Forward", RenderPassType::Graphics)
            .add_color_attachment("Swapchain")
            .set_depth_stencil_output("Depth");

        graph.set_back_buffer("Swapchain");

        let baked_graph = BakedRenderGraph::bake(graph).unwrap();

        // Check attachments
        {
            // Swapchain
            assert_eq!(baked_graph.resources[0], BakedRenderResource {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::R8G8B8_UNORM,
            });
            // Depth
            assert_eq!(baked_graph.resources[1], BakedRenderResource {
                size: AttachmentSize::SwapchainRelative,
                format: vk::Format::D32_SFLOAT,
            });
        }

        // Check pass
        assert_eq!(baked_graph.passes[0], BakedRenderPass {
            ty: RenderPassType::Graphics,
            color_attachments: vec![0],
            input_attachments: vec![],
            depth_attachment: Some(1),
            image_inputs: vec![],
            barriers: [
                (0, BakedResourceBarrier {
                    src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                    dst_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    src_access: vk::AccessFlags::empty(),
                    dst_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    old_layout: vk::ImageLayout::UNDEFINED,
                    new_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                }),
                (1, BakedResourceBarrier {
                    src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                    dst_stage: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                    src_access: vk::AccessFlags::empty(),
                    dst_access: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    old_layout: vk::ImageLayout::UNDEFINED,
                    new_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                }),
            ].iter().cloned().collect(),
        })
    }

    #[test]
    fn test_graph_complex_deferred_shadow_post() {
        let mut graph = RenderGraph::new();

        // Add attachments
        {
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
        }

        // Add passes
        {
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
        }

        graph.set_back_buffer("Swapchain");

        let baked_graph = BakedRenderGraph::bake(graph).unwrap();

        // Check generated resources
        {
            assert_eq!(baked_graph.resources.len(), 8);

            // Albedo
            assert_eq!(
                baked_graph.resources[0],
                BakedRenderResource {
                    size: AttachmentSize::SwapchainRelative,
                    format: vk::Format::R8G8B8A8_UNORM,
                }
            );
            // Position
            assert_eq!(
                baked_graph.resources[1],
                BakedRenderResource {
                    size: AttachmentSize::SwapchainRelative,
                    format: vk::Format::R16G16B16A16_SFLOAT,
                }
            );
            // Normal
            assert_eq!(
                baked_graph.resources[2],
                BakedRenderResource {
                    size: AttachmentSize::SwapchainRelative,
                    format: vk::Format::R16G16B16A16_SFLOAT,
                }
            );
            // Depth
            assert_eq!(
                baked_graph.resources[3],
                BakedRenderResource {
                    size: AttachmentSize::SwapchainRelative,
                    format: vk::Format::D32_SFLOAT,
                }
            );
            // Shadow Map
            assert_eq!(
                baked_graph.resources[4],
                BakedRenderResource {
                    size: AttachmentSize::Fixed(vk::Extent3D {
                        width: 1024,
                        height: 1024,
                        depth: 1,
                    }),
                    format: vk::Format::D32_SFLOAT,
                }
            );
            // Shaded
            assert_eq!(
                baked_graph.resources[5],
                BakedRenderResource {
                    size: AttachmentSize::SwapchainRelative,
                    format: vk::Format::R8G8B8_SRGB,
                }
            );
            // ToneMap
            assert_eq!(
                baked_graph.resources[6],
                BakedRenderResource {
                    size: AttachmentSize::Fixed(vk::Extent3D {
                        width: 256,
                        height: 256,
                        depth: 1,
                    }),
                    format: vk::Format::R8G8B8_UNORM,
                }
            );
            // Swapchain
            assert_eq!(
                baked_graph.resources[7],
                BakedRenderResource {
                    size: AttachmentSize::SwapchainRelative,
                    format: vk::Format::R8G8B8_SRGB,
                }
            );
        }

        // Check generated passes
        {
            assert_eq!(baked_graph.passes.len(), 4);

            // Graphics
            let color_first_pass_barrier = BakedResourceBarrier {
                src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                dst_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                src_access: vk::AccessFlags::empty(),
                dst_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                old_layout: vk::ImageLayout::UNDEFINED,
                new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            };
            assert_eq!(baked_graph.passes[0], BakedRenderPass {
                ty: RenderPassType::Graphics,
                color_attachments: vec![0, 1, 2],
                input_attachments: vec![],
                depth_attachment: Some(3),
                image_inputs: vec![],
                barriers: [
                    (0, color_first_pass_barrier),
                    (1, color_first_pass_barrier),
                    (2, color_first_pass_barrier),
                    (3, BakedResourceBarrier {
                        src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                        dst_stage: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        src_access: vk::AccessFlags::empty(),
                        dst_access: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        old_layout: vk::ImageLayout::UNDEFINED,
                        new_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    }),
                ].iter().cloned().collect(),
            });

            // Shadow pass
            assert_eq!(baked_graph.passes[1], BakedRenderPass {
                ty: RenderPassType::Graphics,
                color_attachments: vec![],
                input_attachments: vec![],
                depth_attachment: Some(4),
                image_inputs: vec![],
                barriers: [
                    (4, BakedResourceBarrier {
                        src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                        dst_stage: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        src_access: vk::AccessFlags::empty(),
                        dst_access: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        old_layout: vk::ImageLayout::UNDEFINED,
                        new_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    }),
                ].iter().cloned().collect(),
            });

            // Lighting pass
            let lighting_color_input_barrier = BakedResourceBarrier {
                src_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                dst_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                src_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_access: vk::AccessFlags::INPUT_ATTACHMENT_READ,
                old_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };
            assert_eq!(baked_graph.passes[2], BakedRenderPass {
                ty: RenderPassType::Graphics,
                color_attachments: vec![5],
                input_attachments: vec![0,1,2,3],
                depth_attachment: None,
                image_inputs: vec![4],
                barriers: [
                    (5, BakedResourceBarrier {
                        src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                        dst_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        src_access: vk::AccessFlags::empty(),
                        dst_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        old_layout: vk::ImageLayout::UNDEFINED,
                        new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    }),
                    (0, lighting_color_input_barrier),
                    (1, lighting_color_input_barrier),
                    (2, lighting_color_input_barrier),
                    (3, BakedResourceBarrier {
                        src_stage: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        dst_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                        src_access: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        dst_access: vk::AccessFlags::INPUT_ATTACHMENT_READ,
                        old_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                        new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    }),
                    (4, BakedResourceBarrier {
                        src_stage: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        dst_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                        src_access: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        dst_access: vk::AccessFlags::SHADER_READ,
                        old_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                        new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    }),
                ].iter().cloned().collect(),
            });

            // Post processing
            assert_eq!(baked_graph.passes[3], BakedRenderPass {
                ty: RenderPassType::Graphics,
                color_attachments: vec![7],
                input_attachments: vec![5],
                depth_attachment: None,
                image_inputs: vec![6],
                barriers: [
                    (7, BakedResourceBarrier {
                        src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                        dst_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        src_access: vk::AccessFlags::empty(),
                        dst_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        old_layout: vk::ImageLayout::UNDEFINED,
                        new_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                    }),
                    (5, BakedResourceBarrier {
                        src_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        dst_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                        src_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        dst_access: vk::AccessFlags::INPUT_ATTACHMENT_READ,
                        old_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    }),
                    (6, BakedResourceBarrier {
                        src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                        dst_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                        src_access: vk::AccessFlags::empty(),
                        dst_access: vk::AccessFlags::SHADER_READ,
                        old_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    }),
                ].iter().cloned().collect(),
            })
        }
    }
}
