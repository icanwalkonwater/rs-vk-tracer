use crate::VkTracerApp;
use std::{borrow::Cow, ffi::CStr, fs::File, io::Write};

#[cfg(feature = "shaderc")]
mod shader_compiler;
#[cfg(feature = "shaderc")]
pub use shader_compiler::*;

#[cfg(feature = "fps_limiter")]
mod fps_limiter;
#[cfg(feature = "fps_limiter")]
pub use fps_limiter::*;

#[cfg(feature = "camera")]
mod camera;
#[cfg(feature = "camera")]
pub use camera::*;

#[cfg(feature = "model_loader")]
mod model_loader;
#[cfg(feature = "model_loader")]
pub use model_loader::*;

/// Converts a rust string to a CStr in a kinda safe manner.
/// Can produce strange thing if the input string isn't valid ASCII.
pub(crate) fn str_to_cstr(s: &str) -> &CStr {
    unsafe { CStr::from_ptr(s.as_ptr() as *const std::os::raw::c_char) }
}

/// Convert a raw string pointer to a rust string with the assumption that it contains only ASCII symbols.
pub(crate) fn cstr_to_str<'a>(ptr: *const std::os::raw::c_char) -> Cow<'a, str> {
    unsafe { CStr::from_ptr(ptr).to_string_lossy() }
}

pub fn dump_vma_stats(app: &VkTracerApp) {
    let stats = app.vma.build_stats_string(true).unwrap();
    let mut f = File::create("vma_stats.json").unwrap();
    f.write_all(stats.as_bytes()).unwrap();
}
