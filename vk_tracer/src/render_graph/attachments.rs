use ash::vk;
use std::fmt::{Display, Formatter};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AttachmentSize {
    SwapchainRelative,
    Fixed(vk::Extent3D),
}

impl Display for AttachmentSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::SwapchainRelative => f.write_str("2D - Swapchain Sized")?,
            Self::Fixed(vk::Extent3D {width, height, depth }) => {
                if depth == 1 {
                    if height == 1 {
                        write!(f, "1D - {}", width)?;
                    } else {
                        write!(f, "2D - {}x{}", width, height)?;
                    }
                } else {
                    write!(f, "3D - {}x{}x{}", width, height, depth)?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AttachmentInfo {
    pub size: AttachmentSize,
    pub format: vk::Format,
    pub transient: bool,
}
