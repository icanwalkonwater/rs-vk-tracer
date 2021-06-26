use crate::{errors::Result, render_graph::attachments::AttachmentInfo};
use log::error;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::{Debug, Display},
    hash::Hash,
    ops::DerefMut,
    rc::Rc,
};
use crate::render_graph::GraphTag;
use indexmap::IndexMap;
use crate::ash::vk;

pub struct RenderGraph<Tag: GraphTag> {
    pub(crate) resources: Rc<RefCell<IndexMap<Tag, RenderGraphResource<Tag>>>>,
    pub(crate) passes: HashMap<Tag, RenderPass<Tag>>,
    pub(crate) back_buffer: Option<Tag>,
}

pub struct RenderPass<Tag: GraphTag> {
    pub(crate) tag: Tag,
    pub(crate) resources: Rc<RefCell<IndexMap<Tag, RenderGraphResource<Tag>>>>,
    pub(crate) ty: RenderPassType,
    pub(crate) color_attachments: Vec<Tag>,
    pub(crate) input_attachments: Vec<Tag>,
    pub(crate) depth_stencil_output: Option<Tag>,
    pub(crate) image_inputs: Vec<Tag>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RenderPassType {
    Graphics,
    Compute,
}

pub(crate) struct RenderGraphResource<Tag: GraphTag> {
    pub(crate) info: AttachmentInfo,
    pub(crate) written_in_pass: Option<Tag>,
    pub(crate) read_in_passes: Vec<Tag>,
}

impl<Tag: GraphTag> RenderGraph<Tag> {
    pub fn new() -> Self {
        Self {
            resources: Rc::new(RefCell::new(IndexMap::new())),
            passes: HashMap::new(),
            back_buffer: None,
        }
    }

    pub fn register_attachment(&mut self, tag: Tag, info: AttachmentInfo) {
        // TODO: better error handling
        self.resources.borrow_mut().insert(
            tag,
            RenderGraphResource {
                info,
                written_in_pass: None,
                read_in_passes: Vec::new(),
            },
        );
    }

    pub fn new_pass(&mut self, tag: Tag, ty: RenderPassType) -> &mut RenderPass<Tag> {
        self.passes.insert(
            tag,
            RenderPass {
                tag,
                resources: self.resources.clone(),
                ty,
                color_attachments: Vec::new(),
                input_attachments: Vec::new(),
                depth_stencil_output: None,
                image_inputs: Vec::new(),
            },
        );

        self.passes.get_mut(&tag).unwrap()
    }

    pub fn set_back_buffer(&mut self, tag: Tag) {
        self.back_buffer = Some(tag);
    }
}

/*#[cfg(any(test, debug_assertions))]
impl<Tag: GraphTag> RenderGraph<Tag> {
    pub fn dump(&self) -> Result<()> {
        use std::{fs::File, io::Write};

        fn tag_to_graph_id<Tag: GraphTag>(tag: Tag) -> String {
            let mut formatted = format!("{}", tag);
            formatted = formatted.replace(&[' ', '\n', '\t', '\r'][..], "_");
            formatted
        }

        let mut out_file = File::create("./raw_render_graph.dot")?;
        writeln!(out_file, "digraph raw_render_graph {{")?;
        writeln!(out_file, " rankdir=LR;")?;

        // Write passes
        for (pass_tag, pass) in &self.passes {
            writeln!(
                out_file,
                " {} [shape=rectangle color=orange style=filled label=\"[{:?}]\\n{}\"]",
                tag_to_graph_id(*pass_tag),
                pass.ty,
                pass.tag
            )?;
        }

        // Write attachments
        for (tag, attachment) in self.resources.borrow().iter() {
            let tag_id = tag_to_graph_id(*tag);

            if self.back_buffer.is_some() && self.back_buffer.unwrap() == *tag {
                writeln!(
                    out_file,
                    " {} [shape=oval color=red style=filled label=\"[Backbuffer]\\n{}\\n{:?}\"]",
                    tag_id, tag, attachment.info.format
                )?;
            } else {
                writeln!(
                    out_file,
                    " {} [shape=oval label=\"{}\\n{:?}\"]",
                    tag_id, tag, attachment.info.format
                )?;
            }

            // Write attachment edges
            if let Some(source_pass) = &attachment.written_in_pass {
                writeln!(out_file, " {} -> {}", tag_to_graph_id(source_pass), tag_id)?;
            }
            for target_pass in &attachment.read_in_passes {
                writeln!(out_file, " {} -> {}", tag_id, tag_to_graph_id(target_pass))?;
            }
        }

        writeln!(out_file, "}}")?;
        Ok(())
    }
}*/

impl<Tag: GraphTag> RenderPass<Tag> {
    pub fn set_execute_callback(&mut self, commands: fn(ash::vk::CommandBuffer)) -> &mut Self {
        // TODO
        self
    }

    pub fn add_color_attachment(&mut self, tag: Tag) -> &mut Self {
        {
            let mut resources = self.resources.borrow_mut();
            let resource = resources.get_mut(&tag).unwrap();
            if let Some(other_tag) = resource.written_in_pass {
                // TODO: better error management
                panic!("RenderGraph: Can't write multiple time to the same logical attachment !");
            }
            resource.written_in_pass = Some(self.tag);
        }
        self.color_attachments.push(tag);
        self
    }

    pub fn add_input_attachment(&mut self, tag: Tag) -> &mut Self {
        {
            let mut resources = self.resources.borrow_mut();
            let resource = resources.get_mut(&tag).unwrap();
            resource.read_in_passes.push(self.tag);
        }
        self.input_attachments.push(tag);
        self
    }

    pub fn set_depth_stencil_output(&mut self, tag: Tag) -> &mut Self {
        {
            let mut resources = self.resources.borrow_mut();
            let resource = resources.get_mut(&tag).unwrap();
            resource.written_in_pass = Some(self.tag);
        }
        self.depth_stencil_output = Some(tag);
        self
    }

    pub fn add_image_input(&mut self, tag: Tag) -> &mut Self {
        {
            let mut resources = self.resources.borrow_mut();
            let resource = resources.get_mut(&tag).unwrap();
            resource.read_in_passes.push(self.tag);
        }
        self.image_inputs.push(tag);
        self
    }
}
