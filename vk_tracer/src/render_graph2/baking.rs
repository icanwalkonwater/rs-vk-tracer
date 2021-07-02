use crate::errors::Result;
use crate::mem::find_depth_format;
use crate::render_graph2::builder::{
    FrozenRenderGraph, RenderGraphBuilder, RenderGraphImageFormat, RenderGraphImageSize,
    RenderGraphLogicalTag, RenderGraphPassResource, RenderGraphPassResourceBindPoint,
    RenderGraphResource,
};
use crate::VkTracerApp;
use ash::vk;
use indexmap::IndexSet;
use itertools::Itertools;
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
    Image {
        size: RenderGraphImageSize,
        format: RenderGraphImageFormat,
    },
}

#[derive(Debug)]
pub enum BakedRenderGraphPassBarrier {
    Image {
        src_stage: vk::PipelineStageFlags2KHR,
        src_access: vk::AccessFlags2KHR,
        dst_stage: vk::PipelineStageFlags2KHR,
        dst_access: vk::AccessFlags2KHR,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    },
}

#[derive(Debug)]
enum ResourceState<R: RenderGraphLogicalTag> {
    Image {
        physical: PhysicalResourceIndex,
        // This can change throughout its life if it is aliased
        current_logical: Option<R>,

        current_layout: vk::ImageLayout,

        // Past information
        last_stage: vk::PipelineStageFlags2KHR,
        last_access: vk::AccessFlags2KHR,
        write_pending: bool,
        // I don't care about your opinion
        read_pending: bool,

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
        #[cfg(feature = "debug_render_graph")]
        use std::io::Write;
        #[cfg(feature = "debug_render_graph")]
        let mut timeline_barriers = std::fs::File::create(
            option_env!("DEBUG_GRAPH_TIMELINE_BARRIER")
                .unwrap_or("./render_graph_timeline_barrier.mmd"),
        )
        .unwrap();
        #[cfg(feature = "debug_render_graph")]
        {
            writeln!(timeline_barriers, "gantt");
            writeln!(timeline_barriers, " title Baked Render Graph");
            writeln!(timeline_barriers, " dateFormat X");
            writeln!(timeline_barriers, " axisFormat %S");
        }
        #[cfg(feature = "debug_render_graph")]
        let mut current_time = 0;

        #[cfg(feature = "debug_render_graph")]
        let mut section_resources_layout = multimap::MultiMap::new();
        #[cfg(feature = "debug_render_graph")]
        let mut section_resources_name = multimap::MultiMap::new();
        #[cfg(feature = "debug_render_graph")]
        let mut section_passes = Vec::new();

        let graph = &graph.0;

        // Schedule passes by following the graph in reverse order
        let schedule = Self::schedule_passes(graph);
        let (resources, resource_mapping) =
            Self::create_physical_resources_and_mapping(graph, &schedule);
        let mut resource_states =
            Self::init_resource_state(graph, &schedule, &resources, &resource_mapping);

        // Now simulate the execution of this graph and add barriers when it is appropriate
        for (pass_physical, pass_tag) in schedule.into_iter().enumerate() {
            let pass = &graph.passes[&pass_tag];

            let mut barriers = Vec::new();

            #[cfg(feature = "debug_render_graph")]
            let mut debug_barriers = Vec::new();

            for (res_tag, res) in pass.resources.iter() {
                let res_state = &mut resource_states[resource_mapping[res_tag]];

                match res_state {
                    ResourceState::Image {
                        physical,
                        current_logical,
                        write_pending,
                        read_pending,
                        current_layout,
                        last_stage,
                        last_access,
                        ..
                    } => {
                        // Update tag right away
                        *current_logical = Some(*res_tag);

                        let need_barrier =
                            // Read-After-Write hazards
                            (res.bind_point.can_read() && *write_pending)
                                // Write-After-Read hazards
                                || (res.bind_point.can_write() && *read_pending)
                                // Just a layout transition
                                || (res.bind_point.optimal_layout() != *current_layout);

                        if need_barrier {
                            let barrier = BakedRenderGraphPassBarrier::Image {
                                src_stage: *last_stage,
                                src_access: *last_access,
                                dst_stage: res.bind_point.stages(),
                                dst_access: res.bind_point.accesses(),
                                old_layout: *current_layout,
                                new_layout: res.bind_point.optimal_layout(),
                            };

                            barriers.push(barrier);

                            #[cfg(feature = "debug_render_graph")]
                            {
                                let mut debug_barrier = format!("{}", current_logical.unwrap());

                                if res.bind_point.can_read() && *write_pending {
                                    debug_barrier.push_str(" [Invalidate]");
                                }
                                if res.bind_point.can_write() && *read_pending {
                                    debug_barrier.push_str(" [Write-After-Read prevent]");
                                }
                                if res.bind_point.optimal_layout() != *current_layout {
                                    debug_barrier.push_str(" [Layout Transition]");
                                }
                                debug_barriers.push(debug_barrier);
                            }

                            // Reflect the barrier on the state of the resource
                            *write_pending = false;
                            *read_pending = false;
                            // If the layout need to change, we will reach this line, so no need to
                            // specify it outside too
                            *current_layout = res.bind_point.optimal_layout();
                            *last_stage = vk::PipelineStageFlags2KHR::NONE;
                            *last_access = vk::AccessFlags2KHR::NONE;
                        }

                        // Advance the resource state
                        if res.bind_point.can_write() {
                            *write_pending = true;
                        }
                        if res.bind_point.can_read() {
                            *read_pending = true;
                        }
                        *last_stage |= res.bind_point.stages();
                        *last_access |= res.bind_point.accesses();
                    }
                }
            }

            #[cfg(feature = "debug_render_graph")]
            {
                for state in &resource_states {
                    match state {
                        ResourceState::Image {
                            physical,
                            current_logical,
                            current_layout,
                            ..
                        } => {
                            section_resources_layout
                                .insert(*physical, format!("Layout {:?}", *current_layout));
                            section_resources_name.insert(
                                *physical,
                                current_logical
                                    .map_or_else(|| String::from("None"), |r| format!("{}", r)),
                            );
                        }
                    }
                }

                for (i, barrier) in debug_barriers.iter().enumerate() {
                    section_passes.push(format!(
                        " {} :crit, barrier_{}_{}, {}, 1s",
                        barrier, pass_physical, i, current_time
                    ));
                }
                section_passes.push(format!(
                    " {} :pass_{}, {}, 10s",
                    pass_tag, pass_physical, current_time
                ));
                current_time += 10;
            }
        }

        #[cfg(feature = "debug_render_graph")]
        {
            use itertools::Itertools;
            for (res, layouts) in section_resources_layout.iter_all().sorted_by_key(|x| *x.0) {
                writeln!(timeline_barriers, " section Res {} names", res);
                {
                    let names = section_resources_name.get_vec(&res).unwrap();
                    let mut prev_name_time = 0;
                    let mut prev_name = &names[0];

                    for (i, name) in names
                        .iter()
                        .enumerate()
                        .skip(1)
                        .chain(std::iter::once((names.len(), &String::new())))
                    {
                        if prev_name != name {
                            if prev_name != "None" {
                                writeln!(
                                    timeline_barriers,
                                    " {} :done, name_{}_{}, {}, {}s",
                                    prev_name,
                                    res,
                                    prev_name_time,
                                    prev_name_time * 10,
                                    (i - prev_name_time) * 10
                                );
                            }
                            prev_name_time = i;
                            prev_name = name;
                        }
                    }
                }

                writeln!(timeline_barriers, " section Res {} layouts", res);
                {
                    let mut prev_layout_time = 0;
                    let mut prev_layout = &layouts[0];

                    for (i, layout) in layouts
                        .iter()
                        .enumerate()
                        .skip(1)
                        .chain(std::iter::once((layouts.len(), &String::new())))
                    {
                        if prev_layout != layout {
                            writeln!(
                                timeline_barriers,
                                " {} :active, layout_{}_{}, {}, {}s",
                                prev_layout,
                                res,
                                prev_layout_time,
                                prev_layout_time * 10,
                                (i - prev_layout_time) * 10
                            );
                            prev_layout_time = i;
                            prev_layout = layout;
                        }
                    }
                }

                writeln!(timeline_barriers, " section Sep{}", res);
                for i in 0..2 {
                    writeln!(timeline_barriers, " _ :separator_{}_{}, 0, 0s", res, i);
                }
            }

            writeln!(timeline_barriers, " section Passes");
            for pass in section_passes {
                writeln!(timeline_barriers, "{}", pass);
            }
        }

        todo!()
    }

    fn schedule_passes(graph: &RenderGraphBuilder<R, P>) -> Vec<P> {
        let mut to_schedule_passes = vec![graph.resources[&graph.get_back_buffer()]
            .written_in_pass
            .unwrap()];
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
                    if dependant_pass != pass_tag {
                        to_schedule_passes.push(dependant_pass);
                    }
                }
            }
        }

        // Reverse so the first one to be scheduled come first
        schedule.iter().copied().rev().collect()
    }

    fn create_physical_resources_and_mapping(
        graph: &RenderGraphBuilder<R, P>,
        schedule: &[P],
    ) -> (
        Vec<BakedRenderGraphResource>,
        HashMap<R, PhysicalResourceIndex>,
    ) {
        let mut resources = Vec::with_capacity(graph.resources.len());
        let mut mapping = HashMap::with_capacity(graph.resources.len());

        // Compute logical to physical passes mapping for convenience
        let pass_mapping = schedule
            .iter()
            .copied()
            .enumerate()
            .map(|(i, p)| (p, i))
            .collect::<HashMap<_, _>>();

        // Put resources with the same size and format to the same bucket
        let mut potential_aliases = multimap::MultiMap::new();
        for (tag, res) in &graph.resources {
            potential_aliases.insert((res.size, res.format), *tag);
        }

        for ((size, format), aliases) in potential_aliases.iter_all() {
            // Easy case
            if aliases.len() == 1 {
                resources.push(BakedRenderGraphResource::Image {
                    size: res.size,
                    format: res.format,
                });
                mapping.insert(tag, resources.len() - 1);
                continue;
            }

            // Compute lifetimes
            let lifetimes = aliases
                .iter()
                .map(|tag| {
                    let res = &graph.resources[tag];
                    let (first_use, last_use) = res
                        .readden_in_pass
                        .iter()
                        .chain(res.written_in_pass)
                        .map(|tag| pass_mapping[tag])
                        .minmax()
                        .into_option()
                        // We checked that there are no orphan resources
                        .unwrap();

                    first_use..=last_use
                })
                .collect::<Vec<_>>();

            todo!()
        }

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
            let aliases = mappings
                .iter()
                .filter(|(_, id)| res_physical == **id)
                .map(|(tag, _)| *tag)
                .collect::<Box<[_]>>();

            let (future_actions, initial_layout) =
                Self::predict_future_of_resource(graph, schedule, &aliases);
            debug_assert!(!future_actions.is_empty());

            // Assume that the resource isn't being used before
            // Assume that the resource is in its optimal layout
            resource_states.push(ResourceState::Image {
                physical: res_physical,
                current_logical: None,
                current_layout: initial_layout,
                last_stage: vk::PipelineStageFlags2KHR::NONE,
                last_access: vk::AccessFlags2KHR::NONE,
                write_pending: false,
                read_pending: false,
                future_actions,
            })
        }

        resource_states
    }

    fn predict_future_of_resource(
        graph: &RenderGraphBuilder<R, P>,
        schedule: &[P],
        resource_aliases: &[R],
    ) -> (Vec<FutureResourceUsage>, vk::ImageLayout) {
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
