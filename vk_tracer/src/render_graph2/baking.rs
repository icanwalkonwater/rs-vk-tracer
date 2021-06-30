use crate::errors::Result;
use crate::render_graph2::builder::{FrozenRenderGraph, RenderGraphBuilder, RenderGraphLogicalTag, RenderGraphPassResourceBindPoint, RenderGraphImageSize, RenderGraphImageFormat, RenderGraphResource, RenderGraphPassResource};
use ash::vk;
use indexmap::IndexSet;
use multimap::MultiMap;
use std::collections::{HashMap, VecDeque};
use crate::mem::find_depth_format;
use crate::VkTracerApp;

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
    Image {
        size: RenderGraphImageSize,
        format: RenderGraphImageFormat,
    },
}

enum ResourceState<R: RenderGraphLogicalTag> {
    Image {
        physical: PhysicalResourceIndex,
        // This can change throughout its life if it is aliased
        current_logical: Option<R>,

        current_layout: vk::ImageLayout,

        // Past information
        last_used_in_stage: vk::PipelineStageFlags2KHR,
        is_being_written: bool,
        // I don't care about your opinion
        is_being_readden: bool,

        // Future information (in order)
        future_actions: Vec<FutureResourceUsage>,
    },
}

#[derive(Debug)]
enum FutureResourceUsage {
    Read(PhysicalPassIndex),
    Write(PhysicalPassIndex),
    ReadWrite(PhysicalPassIndex),
}

impl FutureResourceUsage {
    #[inline]
    fn pass(&self) -> PhysicalPassIndex {
        match *self {
            Self::Read(i) | Self::Write(i) | Self::ReadWrite(i) => i,
        }
    }
}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> BakedRenderGraph<R, P> {
    pub fn bake(graph: &FrozenRenderGraph<R, P>) -> Result<Self> {
        let graph = &graph.0;

        let schedule = Self::schedule_passes(graph);
        let (resources, resource_mapping) = Self::create_physical_resources_and_mapping(graph);
        let resource_states = Self::init_resource_state(graph, &schedule, &resources, &resource_mapping);

        todo!()
    }

    fn schedule_passes(graph: &RenderGraphBuilder<R, P>) -> Vec<P> {
        let mut to_schedule_passes = vec![graph.resources[&graph.get_back_buffer()].written_in_pass.unwrap()];
        // The order is reversed, the first ones to be added will be scheduled last
        let mut schedule = IndexSet::new();

        while !to_schedule_passes.is_empty() {
            let pass_tag = to_schedule_passes.pop().unwrap();
            let pass = &graph.passes[&pass_tag];

            // If this pass already appears in the schedule, it means that another
            // depends on it but the one that scheduled is after and need this same pass.
            // So we need to remove the old one and reschedule it at the end.
            schedule.shift_remove(&pass_tag);
            schedule.insert(pass_tag);

            // Now schedule the dependencies after this pass
            for (res_tag, res) in &pass.resources {
                if let Some(dependant_pass) = graph.resources[res_tag].written_in_pass {
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
        for (&tag, res) in &graph.resources {
            resources.push(BakedRenderGraphResource::Image {
                size: res.size,
                format: res.format,
            });
            mapping.insert(tag, resources.len() - 1);
        }

        (resources, mapping)
    }

    fn init_resource_state(
        graph: &RenderGraphBuilder<R, P>,
        schedule: &[P],
        resources: &[BakedRenderGraphResource],
        mappings: &HashMap<R, PhysicalResourceIndex>,
    ) -> Vec<ResourceState<R>> {
        let mut resource_states = Vec::with_capacity(resources.len());

        for (res_physical, res) in resources.iter().enumerate() {
            let aliases = mappings.iter().filter(|(_, id)| res_physical == **id).map(|(tag, _)| *tag).collect::<Box<[_]>>();

            let (future_actions, initial_layout) = Self::predict_future_of_resource(graph, schedule, &aliases);
            debug_assert!(!future_actions.is_empty());

            // Assume that the resource isn't being used before
            // Assume that the resource is in its optimal layout
            resource_states.push(ResourceState::Image {
                physical: res_physical,
                current_logical: None,
                current_layout: initial_layout,
                last_used_in_stage: vk::PipelineStageFlags2KHR::NONE,
                is_being_written: false,
                is_being_readden: false,
                future_actions,
            })
        }

        resource_states
    }

    fn predict_future_of_resource(graph: &RenderGraphBuilder<R, P>, schedule: &[P], resource_aliases: &[R]) -> (Vec<FutureResourceUsage>, vk::ImageLayout) {
        let mut usages = Vec::new();

        let find_alias_macro = |resources: &HashMap<R, RenderGraphPassResource>| {
            for alias in resource_aliases {
                if resources.contains_key(alias) {
                    return Some(alias);
                }
            }
            None
        };

        let mut initial_layout = None;

        // Walk pass schedule and when this resource is used, record how it will be used
        for (pass_physical, pass_logical) in schedule.iter().enumerate() {
            let pass = &graph.passes[pass_logical];
            // Search pass for one of the aliases
            // (There can only be 0 or 1 of the aliases in the pass because otherwise it wouldn't be aliased)
            if let Some(alias) = find_alias_macro(&pass.resources) {
                let bind_point = pass.resources[alias].bind_point;

                usages.push(match (bind_point.can_read(), bind_point.can_write()) {
                    (true, true) => FutureResourceUsage::ReadWrite(pass_physical),
                    (false, true) => FutureResourceUsage::Write(pass_physical),
                    (true, false) => FutureResourceUsage::Read(pass_physical),
                    _ => unreachable!(),
                });

                // If this pass is the first one where the resource will be used, the initial layout
                // of the resource must be the optimal one for this pass
                if let None = initial_layout {
                    initial_layout = Some(bind_point.optimal_layout());
                }
            }
        }

        (usages, initial_layout.unwrap())
    }
}
