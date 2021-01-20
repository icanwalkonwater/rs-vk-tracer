//! # Adapters
//! [VtAdapter]s are the equivalent of a Vulkan physical device but with all the information queryable
//! already cached inside.

use std::{ffi::CStr, os::raw::c_char};

use ash::vk;
use raw_window_handle::HasRawWindowHandle;

use crate::{
    errors::Result,
    extensions::{
        required_device_extensions, required_instance_extensions,
        required_instance_extensions_with_surface,
    },
    physical_device_selection::{AdapterInfo},
    surface::Surface,
};

pub struct AdapterRequirements {
    pub compatible_surface: Option<Surface>,
    pub instance_extensions: Vec<*const c_char>,
    pub required_extensions: Vec<&'static CStr>,
    pub optional_extensions: Vec<&'static CStr>,
    pub surface_formats: Vec<vk::Format>,
    pub surface_color_spaces: Vec<vk::ColorSpaceKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
    pub validation_layers: Vec<&'static str>,
}

impl AdapterRequirements {
    pub fn default_from_window(surface: Surface, window: &impl HasRawWindowHandle) -> Result<Self> {
        Ok(Self {
            compatible_surface: Some(surface),
            instance_extensions: required_instance_extensions_with_surface(window)?,
            ..Default::default()
        })
    }
}

impl Default for AdapterRequirements {
    fn default() -> Self {
        Self {
            compatible_surface: None,
            instance_extensions: required_instance_extensions(),
            required_extensions: required_device_extensions(),
            optional_extensions: Vec::new(),
            surface_formats: vec![vk::Format::R8G8B8A8_SRGB, vk::Format::B8G8R8A8_SRGB],
            surface_color_spaces: vec![vk::ColorSpaceKHR::SRGB_NONLINEAR],
            present_modes: vec![vk::PresentModeKHR::MAILBOX],
            validation_layers: Vec::new(),
        }
    }
}

/// A handle to a physical device
pub struct Adapter {
    pub handle: vk::PhysicalDevice,
    pub(crate) info: AdapterInfo,
    pub(crate) requirements: AdapterRequirements,
}

impl Adapter {
    pub(crate) fn new(
        handle: vk::PhysicalDevice,
        info: AdapterInfo,
        requirements: AdapterRequirements,
    ) -> Self {
        Self {
            handle,
            info,
            requirements,
        }
    }
}
