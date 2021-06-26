use crate::render_graph::GraphTag;
use crate::{
    errors::{Result, VkTracerError},
    render_graph::{AttachmentSize, RenderGraph, RenderGraphResource, RenderPass, RenderPassType},
};
use ash::vk;
use indexmap::{IndexMap, IndexSet};
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

#[derive(Copy, Clone, Debug)]
pub enum GraphValidationError {
    NoBackBuffer,
    InvalidBackBuffer,
    TagNotRegistered,
    ColorOrInputAttachmentDifferInSize,
    InputAndOutputDepthAttachmentsDiffer,
}

pub struct BakedRenderGraph {
    pub(crate) passes: Box<[BakedRenderPass]>,
    pub(crate) resources: Box<[BakedRenderResource]>,
}

#[derive(Eq, PartialEq, Debug)]
pub(crate) struct BakedRenderPass {
    pub(crate) ty: RenderPassType,
    pub(crate) color_attachments: Vec<PhysicalAttachmentId>,
    pub(crate) input_attachments: Vec<PhysicalAttachmentId>,
    pub(crate) depth_attachment: Option<PhysicalAttachmentId>,
    pub(crate) image_inputs: Vec<PhysicalAttachmentId>,
    pub(crate) barriers: HashMap<PhysicalAttachmentId, BakedResourceBarrier>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub(crate) struct BakedRenderResource {
    pub(crate) is_backbuffer: bool,
    pub(crate) size: AttachmentSize,
    pub(crate) format: vk::Format,
    pub(crate) usages: vk::ImageUsageFlags,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) struct BakedResourceBarrier {
    pub(crate) src_stage: vk::PipelineStageFlags,
    pub(crate) dst_stage: vk::PipelineStageFlags,
    pub(crate) src_access: vk::AccessFlags,
    pub(crate) dst_access: vk::AccessFlags,
    pub(crate) old_layout: vk::ImageLayout,
    pub(crate) new_layout: vk::ImageLayout,
}

impl BakedResourceBarrier {
    fn new(resource: &SimulatedRenderResource) -> Self {
        Self {
            src_stage: resource.last_usage_stage,
            dst_stage: vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            src_access: resource.last_usage_access,
            dst_access: vk::AccessFlags::empty(),
            old_layout: resource.current_layout,
            new_layout: vk::ImageLayout::UNDEFINED,
        }
    }
    fn update_resource(&self, resource: &mut SimulatedRenderResource) {
        resource.has_been_used = true;
        resource.last_usage_stage = self.dst_stage;
        resource.last_usage_access = self.dst_access;
        resource.current_layout = self.new_layout;
    }
}

#[derive(Clone)]
struct SimulatedRenderResource {
    usages: vk::ImageUsageFlags,
    current_layout: vk::ImageLayout,
    last_usage_stage: vk::PipelineStageFlags,
    last_usage_access: vk::AccessFlags,
    has_been_used: bool,
    pending_flush: bool,
}

impl BakedRenderGraph {
    pub fn bake<Tag: GraphTag>(graph: RenderGraph<Tag>) -> Result<BakedRenderGraph> {
        let resources = RefCell::borrow(&graph.resources);
        let passes = &graph.passes;

        // Schedule passes
        let pass_schedule = {
            let mut pass_schedule = IndexSet::with_capacity(passes.len());

            fn walk_pass<Tag: GraphTag>(
                resources: &IndexMap<Tag, RenderGraphResource<Tag>>,
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

            pass_schedule
        };

        // Prepare physical resources
        let (baked_resources, logical_to_physical_resources) = {
            let logical_to_physical_resources = resources
                .keys()
                .copied()
                .enumerate()
                .map(|(physical, logical)| (logical, physical))
                .collect::<HashMap<_, _>>();

            let physical_resources_len = logical_to_physical_resources
                .values()
                .max()
                .map(|last_index| last_index + 1)
                .unwrap_or(0);
            let mut baked_resources = Vec::with_capacity(physical_resources_len);
            baked_resources.resize(
                physical_resources_len,
                BakedRenderResource {
                    is_backbuffer: false,
                    size: AttachmentSize::SwapchainRelative,
                    format: vk::Format::UNDEFINED,
                    usages: vk::ImageUsageFlags::empty(),
                },
            );

            for (tag, res) in resources.iter() {
                let baked = &mut baked_resources[logical_to_physical_resources[tag]];
                baked.size = res.info.size;
                baked.format = res.info.format;

                if *tag == graph.back_buffer.unwrap() {
                    baked.is_backbuffer = true;
                }
            }

            (baked_resources, logical_to_physical_resources)
        };

        // Simulate the schedule and add appropriate barriers
        let baked_passes = {
            let mut baked_passes = Vec::with_capacity(pass_schedule.len());

            // Prepare resources
            let mut simulated_resources = Vec::with_capacity(baked_resources.len());
            simulated_resources.resize(
                baked_resources.len(),
                SimulatedRenderResource {
                    usages: vk::ImageUsageFlags::empty(),
                    current_layout: vk::ImageLayout::UNDEFINED,
                    last_usage_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                    last_usage_access: vk::AccessFlags::empty(),
                    has_been_used: false,
                    pending_flush: false,
                },
            );

            // Simulate passes
            for pass_tag in pass_schedule.iter() {
                let pass: &RenderPass<Tag> = &passes[pass_tag];
                let mut baked_pass = BakedRenderPass {
                    ty: pass.ty,
                    color_attachments: Vec::with_capacity(pass.color_attachments.len()),
                    input_attachments: Vec::with_capacity(pass.input_attachments.len()),
                    depth_attachment: None,
                    image_inputs: Vec::with_capacity(pass.image_inputs.len()),
                    barriers: HashMap::new(),
                };

                // Treat attachments

                for color_tag in &pass.color_attachments {
                    let physical = logical_to_physical_resources[color_tag];
                    let color = &mut simulated_resources[physical];
                    let mut barrier = BakedResourceBarrier::new(color);

                    color.usages |= vk::ImageUsageFlags::COLOR_ATTACHMENT;

                    barrier.dst_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
                    barrier.dst_access = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
                    barrier.new_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

                    // Special case if the color attachment is the backbuffer, transition right away
                    if *color_tag == graph.back_buffer.unwrap() {
                        barrier.new_layout = vk::ImageLayout::PRESENT_SRC_KHR;
                    }

                    barrier.update_resource(color);
                    baked_pass.color_attachments.push(physical);
                    baked_pass.barriers.insert(physical, barrier);
                }

                for input in &pass.input_attachments {
                    let physical = logical_to_physical_resources[input];
                    let input = &mut simulated_resources[physical];
                    let mut barrier = BakedResourceBarrier::new(input);

                    input.usages |= vk::ImageUsageFlags::INPUT_ATTACHMENT;

                    barrier.dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
                    barrier.dst_access = vk::AccessFlags::INPUT_ATTACHMENT_READ;
                    barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

                    barrier.update_resource(input);
                    baked_pass.input_attachments.push(physical);
                    baked_pass.barriers.insert(physical, barrier);
                }

                if let Some(depth) = &pass.depth_stencil_output {
                    let physical = logical_to_physical_resources[depth];
                    let depth = &mut simulated_resources[physical];
                    let mut barrier = BakedResourceBarrier::new(depth);

                    depth.usages |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;

                    barrier.dst_stage = vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
                    barrier.dst_access = vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
                    barrier.new_layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;

                    barrier.update_resource(depth);
                    baked_pass.depth_attachment = Some(physical);
                    baked_pass.barriers.insert(physical, barrier);
                }

                for image in &pass.image_inputs {
                    let physical = logical_to_physical_resources[image];
                    let image = &mut simulated_resources[physical];
                    let mut barrier = BakedResourceBarrier::new(image);

                    image.usages |= vk::ImageUsageFlags::SAMPLED;

                    // Assume image inputs that just entered the graph are in their optimal layout
                    if !image.has_been_used && image.current_layout == vk::ImageLayout::UNDEFINED {
                        barrier.old_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                    }

                    barrier.dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
                    barrier.dst_access = vk::AccessFlags::SHADER_READ;
                    barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

                    barrier.update_resource(image);
                    baked_pass.image_inputs.push(physical);
                    baked_pass.barriers.insert(physical, barrier);
                }

                baked_passes.push(baked_pass);
            }

            baked_passes
        };

        // Dump
        #[cfg(test)]
        {
            use std::{fmt::Display, fs::File, io::Write};

            let mut physical_to_logical_resources = HashMap::new();
            for (logical, physical) in logical_to_physical_resources {
                physical_to_logical_resources.insert(physical, logical);
            }

            fn tag_to_graph_id<Tag: GraphTag>(tag: Tag) -> String {
                let mut formatted = format!("{}", tag);
                formatted = formatted.replace(&[' ', '\n', '\t', '\r'][..], "_");
                formatted
            }

            let mut out_file = File::create("./baked_render_graph.dot")?;
            writeln!(out_file, "digraph baked_render_graph {{")?;
            writeln!(out_file, " rankdir=LR;")?;

            // Write attachments
            for (tag_physical, resource) in baked_resources.iter().enumerate() {
                let tag = physical_to_logical_resources[&tag_physical];
                let tag_id = tag_to_graph_id(tag);
                writeln!(
                    out_file,
                    " {} [shape=oval label=\"{} - {}\\n{}\n{:?}\"]",
                    tag_id, tag_physical, tag, resource.size, resource.format,
                )?;
            }

            // Write passes
            for (tag_physical, pass) in baked_passes.iter().enumerate() {
                let tag = pass_schedule[tag_physical];
                let pass_id = format!("pass_{}", tag_physical);

                // Write name
                writeln!(
                    out_file,
                    " {} [shape=rectangle color=orange style=filled label=\"[{:?}]\\n{} - {}\"]",
                    pass_id, pass.ty, tag_physical, tag
                )?;

                // Write edges and sync operations
                for color in &pass.color_attachments {
                    let color_id = tag_to_graph_id(physical_to_logical_resources[color]);
                    let sync_id = format!("{}_{}", pass_id, color_id);

                    let barrier = format!("{:#?}\\l", pass.barriers[color])
                        .replace("\n", "\\l")
                        .replace("\"", "\\\"");
                    writeln!(
                        out_file,
                        " {} [shape=rectangle color=green style=filled label=\"{}\"]",
                        sync_id, barrier
                    )?;

                    writeln!(out_file, " {} -> {}", pass_id, sync_id)?;
                    writeln!(out_file, " {} -> {}", sync_id, color_id)?;
                }

                for input in &pass.input_attachments {
                    let input_id = tag_to_graph_id(physical_to_logical_resources[input]);
                    let sync_id = format!("{}_{}", pass_id, input_id);

                    let barrier = format!("{:#?}\\l", pass.barriers[input])
                        .replace("\n", "\\l")
                        .replace("\"", "\\\"");
                    writeln!(
                        out_file,
                        " {} [shape=rectangle color=green style=filled label=\"{}\"]",
                        sync_id, barrier
                    )?;

                    writeln!(out_file, " {} -> {}", input_id, sync_id)?;
                    writeln!(out_file, " {} -> {}", sync_id, pass_id)?;
                }

                if let Some(depth) = &pass.depth_attachment {
                    let depth_id = tag_to_graph_id(physical_to_logical_resources[depth]);
                    let sync_id = format!("{}_{}", pass_id, depth_id);

                    let barrier = format!("{:#?}\\l", pass.barriers[depth])
                        .replace("\n", "\\l")
                        .replace("\"", "\\\"");
                    writeln!(
                        out_file,
                        " {} [shape=rectangle color=green style=filled label=\"{}\"]",
                        sync_id, barrier
                    )?;

                    writeln!(out_file, " {} -> {}", pass_id, sync_id)?;
                    writeln!(out_file, " {} -> {}", sync_id, depth_id)?;
                }

                for image in &pass.image_inputs {
                    let image_id = tag_to_graph_id(physical_to_logical_resources[image]);
                    let sync_id = format!("{}_{}", pass_id, image_id);

                    let barrier = format!("{:#?}\\l", pass.barriers[image])
                        .replace("\n", "\\l")
                        .replace("\"", "\\\"");
                    writeln!(
                        out_file,
                        " {} [shape=rectangle color=green style=filled label=\"{}\"]",
                        sync_id, barrier
                    )?;

                    writeln!(out_file, " {} -> {}", image_id, sync_id)?;
                    writeln!(out_file, " {} -> {}", sync_id, pass_id)?;
                }
            }

            writeln!(out_file, "}}")?;
        }

        Ok(BakedRenderGraph {
            passes: baked_passes.into_boxed_slice(),
            resources: baked_resources.into_boxed_slice(),
        })
    }
}
