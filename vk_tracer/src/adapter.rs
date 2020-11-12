//! # Adapters
//! [VtAdapter]s are the equivalent of a Vulkan physical device but with all the information queryable
//! already cached inside.

use std::{ffi::CStr, os::raw::c_char};

use ash::vk;
use raw_window_handle::HasRawWindowHandle;

use crate::{
    device::VtDevice,
    errors::Result,
    instance::VtInstance,
    physical_device_selection::{pick_physical_device, VtAdapterInfo},
    surface::VtSurface,
};
use crate::images::{Format, ColorSpace};

pub struct VtAdapterRequirements {
    pub compatible_surface: Option<VtSurface>,
    pub instance_extensions: Vec<*const c_char>,
    pub required_extensions: Vec<&'static CStr>,
    pub optional_extensions: Vec<&'static CStr>,
    pub surface_formats: Vec<Format>,
    pub surface_color_spaces: Vec<ColorSpace>,
    pub present_modes: Vec<vk::PresentModeKHR>,
    pub validation_layers: Vec<&'static str>,
}

impl VtAdapterRequirements {
    pub fn default_from_window(
        surface: VtSurface,
        window: &impl HasRawWindowHandle,
    ) -> Result<Self> {
        Ok(Self {
            compatible_surface: Some(surface),
            instance_extensions: VtInstance::required_extensions_with_surface(window)?,
            ..Default::default()
        })
    }
}

impl Default for VtAdapterRequirements {
    fn default() -> Self {
        Self {
            compatible_surface: None,
            instance_extensions: VtInstance::required_extensions(),
            required_extensions: VtDevice::required_extensions(),
            optional_extensions: Vec::new(),
            surface_formats: vec![Format::R8G8B8A8_SRGB, Format::B8G8R8A8_SRGB],
            surface_color_spaces: vec![ColorSpace::SRGB_NONLINEAR],
            present_modes: vec![vk::PresentModeKHR::MAILBOX],
            validation_layers: Vec::new(),
        }
    }
}

/// A handle to a physical device
pub struct VtAdapter(
    pub vk::PhysicalDevice,
    pub(crate) VtAdapterInfo,
    pub(crate) VtAdapterRequirements,
);

impl VtInstance {
    pub fn request_adapter(&self, requirements: VtAdapterRequirements) -> Result<VtAdapter> {
        let adapter = pick_physical_device(&self.instance, &requirements)?;

        Ok(VtAdapter(adapter.info.handle, adapter, requirements))
    }
}
