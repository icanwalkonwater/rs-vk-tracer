use crate::render_graph::attachments::AttachmentInfo;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::fmt::Debug;

pub struct RenderGraph<Tag: Copy + Clone + Eq + PartialEq + Hash> {
    attachments: HashMap<Tag, RenderResourceAttachment<Tag>>,
    passes: Vec<RenderPass<Tag>>,
    back_buffer: Option<Tag>,
}

pub struct RenderPass<Tag: Copy + Clone + Eq + PartialEq + Hash> {
    tag: Tag,
    ty: RenderPassType,
    attachment_inputs: Vec<Tag>,
    color_outputs: Vec<Tag>,
    depth_stencil_input: Option<Tag>,
    depth_stencil_output: Option<Tag>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RenderPassType {
    Graphics,
    Compute,
}

pub(crate) struct RenderResourceAttachment<Tag: Copy + Clone + Eq + PartialEq + Hash> {
    info: AttachmentInfo,
    writes: HashSet<Tag>,
    reads: HashSet<Tag>,
}

impl<Tag: Copy + Clone + Eq + PartialEq + Hash> RenderGraph<Tag> {
    pub fn new() -> Self {
        Self {
            attachments: HashMap::new(),
            passes: Vec::new(),
            back_buffer: None,
        }
    }

    pub fn register_attachment(&mut self, tag: Tag, info: AttachmentInfo) {
        self.attachments.insert(tag, RenderResourceAttachment {
            info,
            writes: HashSet::new(),
            reads: HashSet::new(),
        });
    }

    pub fn new_pass(&mut self, tag: Tag, ty: RenderPassType) -> &mut RenderPass<Tag> {
        self.passes.push(RenderPass {
            tag,
            ty,
            attachment_inputs: Vec::new(),
            color_outputs: Vec::new(),
            depth_stencil_input: None,
            depth_stencil_output: None,
        });
        self.passes.last_mut().unwrap()
    }

    pub fn set_back_buffer(&mut self, tag: Tag) {
        self.back_buffer = Some(tag);
    }
}

impl<Tag: Debug> RenderGraph<Tag> {
    pub fn dump(self) {

    }
}

impl<Tag: Copy + Clone + Eq + PartialEq + Hash> RenderPass<Tag> {
    pub fn add_attachment_input(&mut self, tag: Tag) -> &mut Self {
        self.attachment_inputs.push(tag);
        self
    }

    pub fn add_color_output(&mut self, tag: Tag) -> &mut Self {
        self.color_outputs.push(tag);
        self
    }

    pub fn add_color_input_output(&mut self, tag_in: Tag, tag_out: Tag) -> &mut Self {
        self.attachment_inputs.push(tag_in);
        self.color_outputs.push(tag_out);
        self
    }

    pub fn set_depth_stencil_input(&mut self, tag: Tag) -> &mut Self {
        self.depth_stencil_input = Some(tag);
        self
    }

    pub fn set_depth_stencil_output(&mut self, tag: Tag) -> &mut Self {
        self.depth_stencil_output = Some(tag);
        self
    }
}
