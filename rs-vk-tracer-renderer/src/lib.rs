use ::winit::{
    dpi::LogicalSize,
    event_loop::EventLoop,
    window::{Fullscreen, Window, WindowBuilder},
};

use errors::Result;
use lazy_static::lazy_static;
use std::{borrow::Cow, ffi::CStr};

pub mod errors;
mod extensions;
mod physical_device_selection;
pub mod surface;
pub mod vulkan_app;

// Re-exports ash
pub mod ash {
    pub use ::ash::*;
}

// Re-export winit
pub mod winit {
    pub use ::winit::*;
}

#[cfg(feature = "validation-layers")]
mod debug_utils;

pub const ENGINE_NAME: &str = "VK Tracer";

lazy_static! {
    pub static ref ENGINE_VERSION: u32 = ash::vk::make_version(
        env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
        env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
        env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
    );
}

pub const VULKAN_VERSION: u32 = ash::vk::make_version(1, 1, 0);

pub struct AppInfo {
    name: String,
    version: u32,
}

impl AppInfo {
    pub fn new(name: &str, major: u32, minor: u32, patch: u32) -> Self {
        AppInfo {
            name: name.into(),
            version: ash::vk::make_version(major, minor, patch),
        }
    }
}

pub fn create_window(
    app_info: &AppInfo,
    dimensions: LogicalSize<u32>,
    fullscreen: bool,
) -> Result<(EventLoop<()>, Window)> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(dimensions)
        .with_title(&app_info.name)
        .build(&event_loop)?;

    let monitor = window.current_monitor();

    if fullscreen {
        // Pick highest quality mode
        let video_mode = monitor.video_modes().max().expect("What the fuck");
        window.set_fullscreen(Some(Fullscreen::Exclusive(video_mode)))
    }

    Ok((event_loop, window))
}

pub fn str_to_vk_string(string: &str) -> &CStr {
    unsafe { CStr::from_ptr(string.as_ptr() as *const std::os::raw::c_char) }
}

pub fn vk_string_to_string<'a>(vk_string: *const std::os::raw::c_char) -> Cow<'a, str> {
    unsafe { CStr::from_ptr(vk_string).to_string_lossy() }
}
