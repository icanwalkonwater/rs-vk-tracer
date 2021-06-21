use ash::vk;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AttachmentSize {
    SwapchainRelative,
    Fixed(vk::Extent3D),
}

#[derive(Copy, Clone, Debug)]
pub struct AttachmentInfo {
    pub size: AttachmentSize,
    pub format: vk::Format,
    pub transient: bool,
}
