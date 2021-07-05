use crate::{
    errors::{HandleType, Result, VkTracerError},
    SwapchainHandle, VkTracerApp,
};
use ash::vk;

#[derive(Copy, Clone)]
pub struct ImageViewFatHandle {
    pub(crate) handle: vk::Image,
    pub(crate) view: vk::ImageView,
    pub(crate) format: vk::Format,
    pub(crate) extent: vk::Extent2D,
}

impl VkTracerApp {
    pub fn get_images_from_swapchain(
        &self,
        swapchain: SwapchainHandle,
    ) -> Result<Vec<ImageViewFatHandle>> {
        let swapchain = storage_access!(self.swapchain_storage, swapchain, HandleType::Swapchain);

        Ok(swapchain
            .images
            .iter()
            .copied()
            .zip(swapchain.image_views.iter().copied())
            .map(|(handle, view)| ImageViewFatHandle {
                handle,
                view,
                format: swapchain.create_info.image_format,
                extent: swapchain.create_info.image_extent,
            })
            .collect())
    }

    pub fn create_depth_texture(
        &mut self,
        swapchain: SwapchainHandle,
    ) -> Result<ImageViewFatHandle> {
        let swapchain = storage_access!(self.swapchain_storage, swapchain, HandleType::Swapchain);

        let format = find_depth_format(self);

        let image = RawImageAllocation::new(
            &self.vma,
            &ImageDescription {
                ty: vk::ImageType::TYPE_2D,
                extent: vk::Extent3D::builder()
                    .width(swapchain.extent.width)
                    .height(swapchain.extent.height)
                    .depth(1)
                    .build(),
                format,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                array_layers: 1,
                mip_levels: 1,
            },
        )?;

        let mut aspect = vk::ImageAspectFlags::DEPTH;
        if has_stencil(format) {
            aspect |= vk::ImageAspectFlags::STENCIL;
        }
        let image_view = image.fullscreen_view(&self.device, aspect)?;

        Ok(ImageViewFatHandle {
            handle: image.handle,
            view: image_view,
            format: image.format,
            extent: vk::Extent2D::builder()
                .width(image.extent.width)
                .height(image.extent.height)
                .build(),
        })
    }
}

/// Needs to be kept in sync with [has_stencil].
#[inline]
pub(crate) fn find_depth_format(app: &VkTracerApp) -> vk::Format {
    find_supported_format(
        app,
        [
            vk::Format::D24_UNORM_S8_UINT,
            vk::Format::D32_SFLOAT_S8_UINT,
            vk::Format::D32_SFLOAT,
        ],
        vk::ImageTiling::OPTIMAL,
        vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
    ).expect("Yay new edge case for depth format found !")
}

/// Needs to be kept in sync with [find_depth_format].
#[inline]
fn has_stencil(format: vk::Format) -> bool {
    format == vk::Format::D32_SFLOAT_S8_UINT || format == vk::Format::D24_UNORM_S8_UINT
}

fn find_supported_format<const N: usize>(
    app: &VkTracerApp,
    candidates: [vk::Format; N],
    tiling: vk::ImageTiling,
    features: vk::FormatFeatureFlags,
) -> Result<vk::Format> {
    for format in candidates {
        let mut props = vk::FormatProperties2::default();
        unsafe {
            app.instance.get_physical_device_format_properties2(
                app.adapter.handle,
                format,
                &mut props,
            );
        }

        let available_features = match tiling {
            vk::ImageTiling::LINEAR => props.format_properties.linear_tiling_features,
            vk::ImageTiling::OPTIMAL => props.format_properties.optimal_tiling_features,
            _ => unreachable!(),
        };

        if (available_features & features) == features {
            return Ok(format);
        }
    }

    Err(VkTracerError::NoSuitableImageFormat)
}

pub struct ImageDescription {
    pub(crate) ty: vk::ImageType,
    pub(crate) extent: vk::Extent3D,
    pub(crate) format: vk::Format,
    pub(crate) usage: vk::ImageUsageFlags,

    pub(crate) array_layers: u32,
    pub(crate) mip_levels: u32,
}

#[derive(Clone)]
pub struct RawImageAllocation {
    pub(crate) handle: vk::Image,
    pub(crate) allocation: vk_mem::Allocation,
    pub(crate) info: vk_mem::AllocationInfo,

    pub(crate) ty: vk::ImageType,
    pub(crate) format: vk::Format,
    pub(crate) extent: vk::Extent3D,
}

impl RawImageAllocation {
    pub(crate) fn new(vma: &vk_mem::Allocator, desc: &ImageDescription) -> Result<Self> {
        let (image, allocation, info) = vma.create_image(
            &vk::ImageCreateInfo::builder()
                .image_type(desc.ty)
                .format(desc.format)
                .extent(desc.extent)
                .mip_levels(desc.mip_levels)
                .array_layers(desc.array_layers)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(desc.usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED),
            &vk_mem::AllocationCreateInfo {
                usage: vk_mem::MemoryUsage::GpuOnly,
                ..Default::default()
            },
        )?;

        Ok(Self {
            handle: image,
            allocation,
            info,
            ty: desc.ty,
            format: desc.format,
            extent: desc.extent,
        })
    }

    pub(crate) fn fullscreen_view(
        &self,
        device: &ash::Device,
        aspect: vk::ImageAspectFlags,
    ) -> Result<vk::ImageView> {
        let view_type = match self.ty {
            vk::ImageType::TYPE_1D => vk::ImageViewType::TYPE_1D,
            vk::ImageType::TYPE_2D => vk::ImageViewType::TYPE_2D,
            vk::ImageType::TYPE_3D => vk::ImageViewType::TYPE_3D,
            _ => todo!(),
        };

        Ok(unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo::builder()
                    .image(self.handle)
                    .view_type(view_type)
                    .format(self.format)
                    .components(
                        vk::ComponentMapping::builder()
                            .r(vk::ComponentSwizzle::IDENTITY)
                            .g(vk::ComponentSwizzle::IDENTITY)
                            .b(vk::ComponentSwizzle::IDENTITY)
                            .a(vk::ComponentSwizzle::IDENTITY)
                            .build(),
                    )
                    .subresource_range(
                        vk::ImageSubresourceRange::builder()
                            .aspect_mask(aspect)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1)
                            .build(),
                    ),
                None,
            )?
        })
    }
}
