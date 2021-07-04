use crate::VkTracerApp;
use crate::errors::Result;
use crate::render_graph2::baking::BakedRenderGraph;
use crate::render_graph2::builder::RenderGraphLogicalTag;
use ash::vk;

pub struct RenderGraphAllocation<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> {

}

impl<R: RenderGraphLogicalTag, P: RenderGraphLogicalTag> RenderGraphAllocation<R, P> {
    pub(crate) fn new(graph: &BakedRenderGraph<R, P>) -> Result<Self> {
        Ok(())
    }

    fn build_render_passes(graph: &BakedRenderGraph<R, P>) {
        for (pass_physical, pass) in graph.passes.iter().enumerate() {
            pass.resources
                .iter_all()
                .filter(|(b, _)| b.is_attachment())
                .flat_map(|(_, res)| res.iter())
                .map(|(res_tag, persistence)| {

                })

            let attachment_description = vk::AttachmentDescription2 {
                s_type: Default::default(),
                p_next: (),
                flags: Default::default(),
                format: Default::default(),
                samples: Default::default(),
                load_op: Default::default(),
                store_op: Default::default(),
                stencil_load_op: Default::default(),
                stencil_store_op: Default::default(),
                initial_layout: Default::default(),
                final_layout: Default::default()
            }

            vk::RenderPassCreateInfo2::builder()
                .attachments()
                .subpasses()
                .dependencies()
        }
    }
}
