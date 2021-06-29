use crate::errors::Result;
use crate::render_graph2::builder::{
    FrozenRenderGraph, RenderGraphBuilder, RenderGraphLogicalTag, RenderGraphPassResourceBindPoint,
};
use ash::vk;
use indexmap::IndexSet;
use multimap::MultiMap;
use std::collections::{HashMap, VecDeque};

type PhysicalResourceIndex = usize;
type PhysicalPassIndex = usize;

#[derive(Debug)]
pub struct BakedRenderGraph<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> {
    resources: Vec<BakedRenderGraphResource>,
    passes: Vec<BakedRenderGraphPass<R, P>>,
}

#[derive(Debug)]
pub struct BakedRenderGraphPass<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> {
    tag: P,
    resources: MultiMap<RenderGraphPassResourceBindPoint, R>,
}

#[derive(Debug)]
pub enum BakedRenderGraphResource {
    Image { size: vk::Extent3D },
}

struct ImageResourceState<R: RenderGraphLogicalTag> {
    physical: PhysicalResourceIndex,
    // This can change throughout its life if it is aliased
    current_logical: R,

    current_layout: vk::ImageLayout,

    // Past information
    last_used_in_stage: vk::PipelineStageFlags2KHR,
    is_being_written: bool,
    // I don't care about your opinion
    is_being_readden: bool,

    // Future information (in order)
    future_actions: Vec<FutureResourceUsage>,
}

enum FutureResourceUsage {
    Read(PhysicalPassIndex),
    Write(PhysicalPassIndex),
    ReadWrite(PhysicalPassIndex),
}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> BakedRenderGraph<R, P> {
    pub fn bake(graph: &FrozenRenderGraph<R, P>) -> Result<Self> {
        let graph = &graph.0;

        let schedule = Self::schedule_passes(graph);
        let (resource, resource_mapping) = Self::create_physical_resources_and_mapping(graph);
    }

    fn schedule_passes(graph: &RenderGraphBuilder<R, P>) -> Vec<R> {
        let mut to_schedule_passes = vec![graph.get_back_buffer()];
        // The order is reversed, the first ones to be added will be scheduled last
        let mut schedule = IndexSet::new();

        while !to_schedule_passes.is_empty() {
            let pass_tag = to_schedule_passes.pop().unwrap();
            let pass = &graph.passes[&pass_tag];

            // If this pass already appears in the schedule, it means that another
            // depends on it but the one that scheduled is after and need this same pass.
            // So we need to remove the old one and reschedule it at the end.
            schedule.shift_remove(pass_tag);
            schedule.insert(pass_tag);

            // Now schedule the dependencies after this pass
            for (res_tag, res) in &pass.resources {
                if let Some(dependant_pass) = &graph.resources[res_tag].written_in_pass {
                    to_schedule_passes.push(dependant_pass);
                }
            }
        }

        schedule.iter().copied().collect()
    }

    fn create_physical_resources_and_mapping(
        graph: &RenderGraphBuilder<R, P>,
    ) -> (
        Vec<BakedRenderGraphResource>,
        HashMap<R, PhysicalResourceIndex>,
    ) {
        let mut resources = Vec::with_capacity(graph.resources.len());
        let mut mapping = HashMap::with_capacity(graph.resources.len());

        // TODO: resource aliasing
        for (tag, res) in graph.resources {
            resources.push(BakedRenderGraphResource {});
            mapping.insert(tag, resources.len() - 1);
        }

        (resources, mapping)
    }

    fn init_resource_state(
        graph: &RenderGraphBuilder<R, P>,
        mapping: HashMap<R, BakedRenderGraphResource>,
    ) -> Vec<ImageResourceState<R>> {
        todo!()
    }
}
