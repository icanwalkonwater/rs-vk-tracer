use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Fullscreen, Window, WindowBuilder};

use errors::Result;
use lazy_static::lazy_static;
use std::borrow::Cow;
use std::ffi::CStr;

pub mod errors;
mod extensions;
pub mod surface;
pub mod vulkan_app;

#[cfg(feature = "validation-layers")]
mod validation_layers;

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

pub unsafe fn str_to_vk_string(string: &str) -> &CStr {
    CStr::from_ptr(string.as_ptr() as *const std::os::raw::c_char)
}

pub unsafe fn vk_string_to_string<'a>(
    vk_string: *const std::os::raw::c_char,
) -> Cow<'a, str> {
    CStr::from_ptr(vk_string).to_string_lossy()
}
