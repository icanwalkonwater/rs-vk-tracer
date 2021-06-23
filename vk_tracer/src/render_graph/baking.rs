use crate::{
    errors::{Result, VkTracerError},
    render_graph::{AttachmentSize, RenderGraph, RenderGraphResource, RenderPass, RenderPassType},
};
use ash::vk;
use indexmap::IndexSet;
use multimap::MultiMap;
use std::{
    borrow::{Borrow, BorrowMut},
    cell::RefCell,
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
};

type LogicalAttachmentId = usize;
type PhysicalAttachmentId = usize;

#[derive(Default)]
pub struct BakedRenderGraph {
    resources: BakedRenderGraphResources,
    passes: Vec<BakedRenderPass>,
}

#[derive(Default)]
pub(crate) struct BakedRenderGraphResources {
    attachments: Vec<BakedImageResource>,
}

pub(crate) struct BakedImageResource {
    accesses: vk::AccessFlags,
    usages: vk::ImageUsageFlags,
}

#[derive(Default)]
pub(crate) struct BakedRenderPass {
    color_attachments: Vec<BakedRenderPassAttachment>,
    input_attachments: Vec<BakedRenderPassAttachment>,
    depth_stencil_input_attachment: Option<BakedRenderPassAttachment>,
    depth_stencil_output_attachment: Option<BakedRenderPassAttachment>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct BakedRenderPassAttachment {
    logical_id: usize,
    layout_needed: vk::ImageLayout,
}

#[derive(Copy, Clone, Debug)]
pub enum GraphValidationError {
    NoBackBuffer,
    InvalidBackBuffer,
    TagNotRegistered,
    ColorOrInputAttachmentDifferInSize,
    InputAndOutputDepthAttachmentsDiffer,
}

/*
impl BakedRenderGraph {
    pub(crate) fn new<Tag: Copy + Clone + Eq + PartialEq + Hash>(graph: RenderGraph<Tag>) -> Result<Self> {
        // Validate graph
        if let Some(error) = Self::validate_graph(&graph) {
            return Err(VkTracerError::InvalidRenderGraph(error));
        }

        // Traverse

        // Merge

        // Final

        todo!()
    }

    fn validate_graph<Tag: Copy + Clone + Eq + PartialEq + Hash>(graph: &RenderGraph<Tag>) -> Option<GraphValidationError> {
        // Check the backbuffer
        if let Some(backbuffer) = &graph.back_buffer {
            if !graph.resources.borrow().contains_key(backbuffer) {
                return Some(GraphValidationError::InvalidBackBuffer);
            }
        } else {
            return Some(GraphValidationError::NoBackBuffer);
        }

        // Check passes

        for pass in &graph.passes {
            use std::iter::{empty, once};

            // All attachments tags must exist
            let mut all_attachments_iter = pass
                .color_attachments
                .iter()
                .chain(pass.input_attachments.iter())
                .chain(pass.depth_stencil_output.iter());

            for attachment in all_attachments_iter {
                if !graph.resources.borrow().contains_key(attachment) {
                    return Some(GraphValidationError::TagNotRegistered);
                }
            }

            // All color/input attachments must have the same size
            if !pass.color_attachments.is_empty() && !pass.input_attachments.is_empty() {
                let target_size = if !pass.color_attachments.is_empty() {
                    graph.resources.borrow()[&pass.color_attachments[0]].info.size
                } else {
                    graph.resources.borrow()[&pass.input_attachments[0]].info.size
                };

                let all_same_size = pass
                    .color_attachments
                    .iter()
                    .chain(pass.input_attachments.iter())
                    .all(|tag| graph.resources.borrow()[tag].info.size == target_size);
                if !all_same_size {
                    return Some(GraphValidationError::ColorOrInputAttachmentDifferInSize);
                }
            }

            // Input & Output depth must be the same resource
            // if pass.depth_stencil_input != pass.depth_stencil_output {
            //     return Some(GraphValidationError::InputAndOutputDepthAttachmentsDiffer);
            // }
        }

        None
    }

    fn build_graph<Tag: Copy + Clone + Eq + PartialEq + Hash>(graph: &RenderGraph<Tag>) -> Self {
        // Assumes graph has been validated

        // Prepare global data
        let mut baked_graph = BakedRenderGraph::default();
        let mut resources = BakedRenderGraphResources::default();
        let tag_to_logical_resource = {
            let mut tag_to_logical_resource = HashMap::new();

            // Register attachments
            let raw_resources = graph.borrow();
            for (i, (tag, _)) in raw_resources.iter().enumerate() {
                tag_to_logical_resource.insert(tag, i);
                // Register to resources while we're at it
                resources.attachments.push(BakedImageResource {
                    accesses: vk::AccessFlags::empty(),
                    usages: vk::ImageUsageFlags::empty(),
                });
            }

            tag_to_logical_resource.shrink_to_fit();
            tag_to_logical_resource
        };

        // Populate passes
        for pass in &graph.passes {
            let mut baked_pass = BakedRenderPass::default();

            // Register each attachment

            // Color attachments (outputs)
            for color_attachment in &pass.color_attachments {
                let logical_id = tag_to_logical_resource[color_attachment];
                let resource = &mut resources.attachments[logical_id];
                resource.accesses |= vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
                resource.usages |= vk::ImageUsageFlags::COLOR_ATTACHMENT;

                baked_pass.color_attachments.push(BakedRenderPassAttachment {
                    logical_id,
                    layout_needed: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                });
            }

            // Input attachments
            for input_attachment in &pass.input_attachments {
                let logical_id = tag_to_logical_resource[input_attachment];
                let resource = &mut resources.attachments[logical_id];
                resource.accesses |= vk::AccessFlags::INPUT_ATTACHMENT_READ;
                resource.usages |= vk::ImageUsageFlags::INPUT_ATTACHMENT;

                baked_pass.input_attachments.push(BakedRenderPassAttachment {
                    logical_id,
                    layout_needed: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                });
            }

            // Depth attachment
            // If both input and output are present, we know that they are the same
            // {
            //     match (&pass.depth_stencil_input, &pass.depth_stencil_output) {
            //         (Some(depth_in), Some(_)) => {
            //             let logical_id = tag_to_logical_resource[depth_in];
            //             let resource = &mut resources.attachments[logical_id];
            //             resource.accesses |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
            //             resource.usages |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
            //
            //             let attachment = BakedRenderPassAttachment {
            //                 logical_id,
            //                 layout_needed: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            //             };
            //             baked_pass.depth_stencil_input_attachment = Some(attachment);
            //             baked_pass.depth_stencil_output_attachment = Some(attachment);
            //         },
            //         (Some(depth_in), None) => {
            //             let logical_id = tag_to_logical_resource[depth_in];
            //             let resource = &mut resources.attachments[logical_id];
            //             resource.accesses |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ;
            //             resource.usages |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
            //
            //             baked_pass.depth_stencil_input_attachment = Some(BakedRenderPassAttachment {
            //                 logical_id,
            //                 layout_needed: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            //             });
            //         },
            //         (None, Some(depth_out)) => {
            //             let logical_id = tag_to_logical_resource[depth_out];
            //             let resource = &mut resources.attachments[logical_id];
            //             resource.accesses |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ;
            //             resource.usages |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
            //
            //             baked_pass.depth_stencil_input_attachment = Some(BakedRenderPassAttachment {
            //                 logical_id,
            //                 layout_needed: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            //             });
            //         },
            //         (None, None) => { /* no-op */ }
            //     }
            // }

            // Now we have registered every attachment for this pass, let's look for barriers to insert
        }

        // {
        //     let mut passes_to_process = Vec::new();
        //     let mut last_writes = HashMap::new();
        //     let mut last_reads = HashMap::new();
        //
        //     {
        //         // Initiate with passes that writes the backbuffer
        //         let back_buffer = graph.back_buffer.unwrap();
        //         for pass in graph.passes {
        //             if pass.color_attachments.contains(back_buffer) {
        //                 passes_to_process.push(pass)
        //             }
        //         }
        //     }
        // }

        // {
        //     struct ResourceNode {}
        //     struct PassNode {}
        // }

        baked_graph
    }
}
*/

pub fn bake<Tag: Copy + Clone + Eq + PartialEq + Hash + Debug + Display>(
    graph: RenderGraph<Tag>,
) -> Result<()> {
    let resources = RefCell::borrow(&graph.resources);
    let passes = &graph.passes;

    // Schedule passes
    let pass_schedule = {
        let mut pass_schedule = IndexSet::with_capacity(passes.len());

        fn walk_pass<Tag: Copy + Clone + Eq + PartialEq + Hash + Debug + Display>(
            resources: &HashMap<Tag, RenderGraphResource<Tag>>,
            passes: &HashMap<Tag, RenderPass<Tag>>,
            scheduled_passes: &mut IndexSet<Tag>,
            pass: &RenderPass<Tag>,
        ) {
            // Schedule the dependencies of this pass
            for tag in &pass.input_attachments {
                if let Some(dependency) = &resources[tag].written_in_pass {
                    walk_pass(resources, passes, scheduled_passes, &passes[dependency]);
                }
            }
            for tag in &pass.image_inputs {
                if let Some(dependency) = &resources[tag].written_in_pass {
                    walk_pass(resources, passes, scheduled_passes, &passes[dependency]);
                }
            }

            scheduled_passes.insert(pass.tag);
        }

        // Get the pass that writes to the back buffer
        let last_pass = &passes[&resources[&graph.back_buffer.unwrap()]
            .written_in_pass
            .unwrap()];
        walk_pass(&resources, passes, &mut pass_schedule, last_pass);
        println!("{:?}", &pass_schedule);

        pass_schedule
    };

    struct BakedRenderResource<Tag: Copy + Clone + Eq + PartialEq + Hash + Debug + Display> {
        tag: Tag,
        size: AttachmentSize,
        format: vk::Format,
    }

    struct BakedRenderPass<Tag: Copy + Clone + Eq + PartialEq + Hash + Debug + Display> {
        tag: Tag,
        ty: RenderPassType,
        color_attachments: Vec<Tag>,
        input_attachments: Vec<Tag>,
        depth_attachment: Option<Tag>,
        image_inputs: Vec<Tag>,
        before_operations: MultiMap<Tag, SyncOperation>,
        after_operations: MultiMap<Tag, SyncOperation>,
    }

    #[derive(Debug)]
    enum SyncOperation {
        LayoutTransition(vk::ImageLayout, vk::ImageLayout),
        Invalidate(vk::PipelineStageFlags, vk::AccessFlags),
        Flush(vk::PipelineStageFlags, vk::AccessFlags),
    }

    struct SimulatedRenderResource<Tag: Copy + Clone + Eq + PartialEq + Hash + Debug + Display> {
        tag: Tag,
        current_layout: vk::ImageLayout,
        last_usage_stage: vk::PipelineStageFlags,
        pending_invalidate: bool,
        pending_flush: bool,
    }

    // Simulate the schedule and add appropriate barriers
    let (baked_resources, baked_passes) = {
        let mut baked_passes = Vec::with_capacity(pass_schedule.len());
        let baked_resources = resources
            .iter()
            .map(|(tag, res)| {
                (
                    *tag,
                    BakedRenderResource {
                        tag: *tag,
                        size: res.info.size,
                        format: res.info.format,
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let simulated_resources = resources
            .iter()
            .map(|(tag, _)| {
                (
                    *tag,
                    SimulatedRenderResource {
                        tag,
                        current_layout: vk::ImageLayout::UNDEFINED,
                        last_usage_stage: vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        pending_invalidate: true,
                        pending_flush: false,
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        // Simulate passes
        for pass_tag in pass_schedule.iter() {
            let pass: &RenderPass<Tag> = &passes[pass_tag];
            let mut baked_pass = BakedRenderPass {
                tag: pass.tag,
                ty: pass.ty,
                color_attachments: Vec::with_capacity(pass.color_attachments.len()),
                input_attachments: Vec::with_capacity(pass.input_attachments.len()),
                depth_attachment: None,
                image_inputs: Vec::with_capacity(pass.image_inputs.len()),
                before_operations: MultiMap::new(),
                after_operations: MultiMap::new(),
            };

            // Treat color attachments
            for color in &pass.color_attachments {
                let color = &simulated_resources[color];

                baked_pass.color_attachments.push(*color.tag);

                // Check if transition needed
                if color.current_layout != vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL {
                    baked_pass.before_operations.insert(
                        *color.tag,
                        SyncOperation::LayoutTransition(
                            color.current_layout,
                            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        ),
                    );
                }
                // Flush afterwards
                baked_pass.after_operations.insert(
                    *color.tag,
                    SyncOperation::Flush(
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    ),
                );
            }

            for input in &pass.input_attachments {
                let input = &simulated_resources[input];

                baked_pass.input_attachments.push(*input.tag);

                // Invalidate before
                baked_pass.before_operations.insert(
                    *input.tag,
                    SyncOperation::Invalidate(
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::AccessFlags::INPUT_ATTACHMENT_READ,
                    ),
                );
                // Transition if necessary
                if input.current_layout != vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL {
                    baked_pass.before_operations.insert(
                        *input.tag,
                        SyncOperation::LayoutTransition(
                            input.current_layout,
                            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        ),
                    );
                }
            }

            if let Some(depth) = &pass.depth_stencil_output {
                let depth = &simulated_resources[depth];

                baked_pass.depth_attachment = Some(*depth.tag);

                // Transition if necessary
                if depth.current_layout != vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL {
                    baked_pass.before_operations.insert(
                        *depth.tag,
                        SyncOperation::LayoutTransition(
                            depth.current_layout,
                            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                        ),
                    );
                }
            }

            for image in &pass.image_inputs {
                let resource = &resources[image];
                let image = &simulated_resources[image];

                baked_pass.image_inputs.push(*image.tag);

                // If it was written before, invalidate
                if let Some(_) = resource.written_in_pass {
                    // TODO: handle images that are read outside of fragment shader
                    baked_pass.before_operations.insert(
                        *image.tag,
                        SyncOperation::Invalidate(
                            vk::PipelineStageFlags::FRAGMENT_SHADER,
                            vk::AccessFlags::SHADER_READ,
                        ),
                    );
                }
                // Transition if necessary
                if image.current_layout != vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL {
                    baked_pass.before_operations.insert(
                        *image.tag,
                        SyncOperation::LayoutTransition(
                            image.current_layout,
                            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        ),
                    );
                }
            }

            baked_passes.push(baked_pass);
        }

        (baked_resources, baked_passes)
    };

    // Dump
    {
        use std::{fmt::Display, fs::File, io::Write};

        fn tag_to_graph_id<Tag: Display>(tag: Tag) -> String {
            let mut formatted = format!("{}", tag);
            formatted = formatted.replace(&[' ', '\n', '\t', '\r'][..], "_");
            formatted
        }

        let mut out_file = File::create("./baked_render_graph.dot")?;
        writeln!(out_file, "digraph baked_render_graph {{")?;
        writeln!(out_file, " rankdir=LR;")?;

        // Write attachments
        for (tag, resource) in &baked_resources {
            let tag_id = tag_to_graph_id(tag);
            writeln!(
                out_file,
                " {} [shape=oval label=\"{}\\n{:?}\"]",
                tag_id, tag, resource.format,
            )?;
        }

        // Write passes
        for pass in &baked_passes {
            let pass_id = tag_to_graph_id(pass.tag);

            // Write name
            writeln!(
                out_file,
                " {} [shape=rectangle color=orange style=filled label=\"[{:?}]\\n{}\"]",
                pass_id, pass.ty, pass.tag
            )?;

            // Write edges and sync operations
            for color in &pass.color_attachments {
                let color_id = tag_to_graph_id(color);

                writeln!(out_file, " {} -> {}", pass_id, color_id)?;
            }

            for input in &pass.input_attachments {
                let input_id = tag_to_graph_id(input);

                writeln!(out_file, " {} -> {}", input_id, pass_id)?;
            }

            if let Some(depth) = &pass.depth_attachment {
                let depth_id = tag_to_graph_id(depth);

                writeln!(out_file, " {} -> {}", pass_id, depth_id)?;
            }

            for image in &pass.image_inputs {
                let image_id = tag_to_graph_id(image);

                writeln!(out_file, " {} -> {}", image_id, pass_id)?;
            }
        }

        writeln!(out_file, "}}")?;
    }

    Ok(())
}
