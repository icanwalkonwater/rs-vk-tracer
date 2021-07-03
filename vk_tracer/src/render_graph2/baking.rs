use crate::errors::Result;
use crate::mem::find_depth_format;
use crate::render_graph2::builder::{FrozenRenderGraph, RenderGraphBuilder, RenderGraphImageFormat, RenderGraphImageSize, RenderGraphLogicalTag, RenderGraphPassResource, RenderGraphPassResourceBindPoint, RenderGraphResource, RenderGraphResourcePersistence};
use crate::VkTracerApp;
use ash::vk;
use indexmap::IndexSet;
use itertools::Itertools;
use multimap::MultiMap;
use std::collections::{HashMap, VecDeque};
use std::ops::RangeBounds;

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
        current_logical: (Option<R>, Option<R>),

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
    pub fn bake(graph: &mut FrozenRenderGraph<R, P>) -> Result<Self> {
        #[cfg(feature = "visualizer_render_graph")]
        use std::io::Write;
        #[cfg(feature = "visualizer_render_graph")]
        let mut visualizer_timeline = std::fs::File::create(
            option_env!("VISUALIZER_TIMELINE_OUT")
                .unwrap_or("./render_graph_timeline.mmd"),
        )
        .unwrap();
        #[cfg(feature = "visualizer_render_graph")]
        {
            writeln!(visualizer_timeline, "gantt");
            writeln!(visualizer_timeline, " title Baked Render Graph");
            writeln!(visualizer_timeline, " dateFormat X");
            writeln!(visualizer_timeline, " axisFormat %S");
        }
        #[cfg(feature = "visualizer_render_graph")]
        let mut current_time = 0;

        #[cfg(feature = "visualizer_render_graph")]
        let mut visualizer_resources_layout = multimap::MultiMap::new();
        #[cfg(feature = "visualizer_render_graph")]
        let mut visualizer_resources_name = multimap::MultiMap::new();
        #[cfg(feature = "visualizer_render_graph")]
        let mut visualizer_passes = Vec::new();

        // Schedule passes by following the graph in reverse order
        let schedule = Self::schedule_passes(&graph.0);
        let (resources, resource_mapping, resource_mapping_inverse) =
            Self::create_physical_resources_and_mapping(&mut graph.0, &schedule);
        let mut resource_states =
            Self::init_resource_state(&graph.0, &schedule, &resources, &resource_mapping);

        // Now simulate the execution of this graph and add barriers when it is appropriate
        for (pass_physical, pass_tag) in schedule.into_iter().enumerate() {
            let pass = &graph.0.passes[&pass_tag];

            let mut barriers = Vec::new();

            #[cfg(feature = "visualizer_render_graph")]
            let mut visualizer_barriers = Vec::new();

            // Update each state
            for (res_physical, res_state) in resource_states.iter_mut().enumerate() {
                let res_tags = resource_mapping_inverse.get_vec(&res_physical).unwrap().iter().copied().filter(|r| pass.resources.contains_key(r)).collect_vec();

                // If resource is not used in this pass
                if res_tags.is_empty() {
                    continue;
                }
                debug_assert!(res_tags.len() <= 2);

                // When we have multiple aliases in the same pass, it means RMW and those bind points
                // share the same properties, so taking either is ok.
                let bind_point = pass.resources[&res_tags[0]].bind_point;

                match res_state {
                    ResourceState::Image {
                        current_logical,
                        write_pending,
                        read_pending,
                        current_layout,
                        last_stage,
                        last_access,
                        ..
                    } => {
                        *current_logical = (Some(res_tags[0]), None);
                        if res_tags.len() == 2 {
                            current_logical.1 = Some(res_tags[1]);
                        }

                        let need_barrier =
                            // Read-After-Write hazards
                            (bind_point.can_read() && *write_pending)
                                // Write-After-Read hazards
                                || (bind_point.can_write() && *read_pending)
                                // Just a layout transition
                                || (bind_point.optimal_layout() != *current_layout);

                        if need_barrier {
                            let barrier = BakedRenderGraphPassBarrier::Image {
                                src_stage: *last_stage,
                                src_access: *last_access,
                                dst_stage: bind_point.stages(),
                                dst_access: bind_point.accesses(),
                                old_layout: *current_layout,
                                new_layout: bind_point.optimal_layout(),
                            };

                            barriers.push(barrier);

                            #[cfg(feature = "visualizer_render_graph")]
                            {
                                let mut visualizer_barrier = if current_logical.1.is_some() {
                                    format!("[{}] {} / {}", res_physical, current_logical.0.unwrap(), current_logical.1.unwrap())
                                } else {
                                    format!("[{}] {}", res_physical, current_logical.0.unwrap())
                                };

                                if bind_point.can_read() && *write_pending {
                                    visualizer_barrier.push_str(" [Flush]");
                                }
                                if bind_point.can_write() && *read_pending {
                                    visualizer_barrier.push_str(" [Write-After-Read prevent]");
                                }
                                if bind_point.optimal_layout() != *current_layout {
                                    visualizer_barrier.push_str(" [Layout Transition]");
                                }
                                visualizer_barriers.push(visualizer_barrier);
                            }

                            // Reflect the barrier on the state of the resource
                            *write_pending = false;
                            *read_pending = false;
                            // If the layout need to change, we will reach this line, so no need to
                            // specify it outside too
                            *current_layout = bind_point.optimal_layout();
                            *last_stage = vk::PipelineStageFlags2KHR::NONE;
                            *last_access = vk::AccessFlags2KHR::NONE;
                        }

                        // Advance the resource state
                        if bind_point.can_write() {
                            *write_pending = true;
                        }
                        if bind_point.can_read() {
                            *read_pending = true;
                        }
                        *last_stage |= bind_point.stages();
                        *last_access |= bind_point.accesses();
                    }
                }
            }

            #[cfg(feature = "visualizer_render_graph")]
            {
                for state in &resource_states {
                    match state {
                        ResourceState::Image {
                            physical,
                            current_logical,
                            current_layout,
                            ..
                        } => {
                            visualizer_resources_layout
                                .insert(*physical, format!("Layout {:?}", *current_layout));
                            visualizer_resources_name.insert(
                                *physical,
                                format!("{}{}",
                                    current_logical.0
                                        .map_or_else(|| String::from("None"), |r| format!("{}", r)),
                                    current_logical.1
                                        .map_or_else(|| String::from(""), |r| format!(" / {}", r)),
                                ),
                            );
                        }
                    }
                }

                for (i, barrier) in visualizer_barriers.iter().enumerate() {
                    visualizer_passes.push(format!(
                        " {} :crit, barrier_{}_{}, {}, 1s",
                        barrier, pass_physical, i, current_time
                    ));
                }
                visualizer_passes.push(format!(
                    " {} :pass_{}, {}, 10s",
                    pass_tag, pass_physical, current_time
                ));
                current_time += 10;
            }
        }

        #[cfg(feature = "visualizer_render_graph")]
        {
            use itertools::Itertools;
            for (res, layouts) in visualizer_resources_layout.iter_all().sorted_by_key(|x| *x.0) {
                writeln!(visualizer_timeline, " section Res {} names", res);
                {
                    let names = visualizer_resources_name.get_vec(&res).unwrap();
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
                                    visualizer_timeline,
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

                writeln!(visualizer_timeline, " section Res {} layouts", res);
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
                                visualizer_timeline,
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

                writeln!(visualizer_timeline, " section Sep{}", res);
                for i in 0..2 {
                    writeln!(visualizer_timeline, " _ :separator_{}_{}, 0, 0s", res, i);
                }
            }

            writeln!(visualizer_timeline, " section Passes");
            for pass in visualizer_passes {
                writeln!(visualizer_timeline, "{}", pass);
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
        graph: &mut RenderGraphBuilder<R, P>,
        schedule: &[P],
    ) -> (
        Vec<BakedRenderGraphResource>,
        HashMap<R, PhysicalResourceIndex>,
        MultiMap<PhysicalResourceIndex, R>,
    ) {
        let mut resources = Vec::with_capacity(graph.resources.len());
        let mut mapping = HashMap::new();
        let mut mapping_inverse = MultiMap::new();

        // Compute logical to physical passes mapping for convenience
        let pass_mapping = schedule
            .iter()
            .copied()
            .enumerate()
            .map(|(i, p)| (p, i))
            .collect::<HashMap<_, _>>();

        // Put resources with the same size and format to the same bucket to simplify aliasing
        let mut potential_aliases = multimap::MultiMap::new();
        for (tag, res) in &graph.resources {
            if res.written_in_pass.is_some() {
                potential_aliases.insert((res.size, res.format), *tag);
            }
        }

        for ((size, format), alias_candidates) in potential_aliases.iter_all_mut() {
            // Easy case
            if alias_candidates.len() == 1 {
                resources.push(BakedRenderGraphResource::Image {
                    size: *size,
                    format: *format,
                });
                mapping.insert(alias_candidates[0], resources.len() - 1);
                mapping_inverse.insert(resources.len() - 1, alias_candidates[0]);
                continue;
            }

            // Compute lifetimes
            let lifetimes = alias_candidates
                .iter()
                .map(|tag| {
                    let res = &graph.resources[tag];
                    let (mut first_use, mut last_use) = res
                        .readden_in_pass
                        .iter()
                        .chain(res.written_in_pass.as_ref())
                        .map(|tag| pass_mapping[tag])
                        .minmax()
                        .into_option()
                        // We checked that there are no orphan resources
                        .unwrap();

                    // Extent this lifetime based on the persistence type of the resource
                    {
                        use RenderGraphResourcePersistence::*;
                        match res.persistence {
                            Transient | ClearInput => { /* no-op */ },
                            PreserveInput => { first_use = 0; },
                            PreserveOutput | ClearInputPreserveOutput => { last_use = usize::MAX; },
                            PreserveAll => {
                                first_use = 0;
                                last_use = usize::MAX
                            },
                        }
                    }

                    (*tag, first_use..=last_use)
                })
                // Sort for the algorithm to be stable
                .sorted_by_key(|(_, lifetime)| *lifetime.start())
                .collect::<Vec<_>>();


            // Put resources in buckets on no overlapping lifetimes
            
            let mut buckets = Vec::<Vec<usize>>::new();
            // Loop through every lifetime from the shortest to the longest
            'lifetime_for: for (res_physical, (res_tag, lifetime)) in lifetimes.iter().enumerate() {

                // First look for Read-Modify-Write
                for bucket in buckets.iter_mut() {
                    let last_physical = *bucket.last().unwrap();
                    let last_lifetime = &lifetimes[last_physical].1;

                    if lifetime.start() == last_lifetime.end() {
                        let pass = graph.passes.get_mut(&schedule[*lifetime.start()]).unwrap();
                        let last_tag = &lifetimes[last_physical].0;

                        // Is it in the right bind points ?
                        let input = &pass.resources[last_tag];
                        let color = &pass.resources[res_tag];
                        if input.bind_point != RenderGraphPassResourceBindPoint::InputAttachment || color.bind_point != RenderGraphPassResourceBindPoint::ColorAttachment {
                            continue;
                        }

                        // Is it even allowed ?
                        if !pass.read_modify_write_whitelist.contains(&(*last_tag, *res_tag)) {
                            continue;
                        }

                        // We need to change a few things to make it work
                        // Namely, it only works with the GENERAL image layout

                        let input = pass.resources.get_mut(last_tag).unwrap();
                        input.bind_point = RenderGraphPassResourceBindPoint::GeneralInputAndColorAttachment;
                        let color = pass.resources.get_mut(res_tag).unwrap();
                        color.bind_point = RenderGraphPassResourceBindPoint::GeneralInputAndColorAttachment;

                        // Finish
                        bucket.push(res_physical);
                        continue 'lifetime_for;
                    }
                }


                // Try a bucket the easy way now
                for bucket in buckets.iter_mut() {
                    let last_lifetime = &lifetimes[*bucket.last().unwrap()].1;

                    if !lifetime.contains(last_lifetime.end()) {
                        bucket.push(res_physical);
                        continue 'lifetime_for;
                    }
                }

                buckets.push(vec![res_physical]);
            }

            // Each bucket is a new physical resource
            for bucket in buckets {
                resources.push(BakedRenderGraphResource::Image {
                    size: *size,
                    format: *format,
                });
                let res_id =  resources.len() - 1;

                for alias_i in bucket {
                    let alias = lifetimes[alias_i].0;
                    mapping.insert(alias, res_id);
                    mapping_inverse.insert(res_id, alias);
                }
            }
        }

        (resources, mapping, mapping_inverse)
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
                current_logical: (None, None),
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
