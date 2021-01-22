use std::{borrow::Cow, ffi::CStr, ops::Deref};

/// Converts a rust string to a CStr in a kinda safe manner.
/// Can produce strange thing if the input string isn't valid ASCII.
pub(crate) fn str_to_cstr(s: &str) -> &CStr {
    unsafe { CStr::from_ptr(s.as_ptr() as *const std::os::raw::c_char) }
}

/// Convert a raw string pointer to a rust string with the assumption that it contains only ASCII symbols.
pub(crate) fn cstr_to_str<'a>(ptr: *const std::os::raw::c_char) -> Cow<'a, str> {
    unsafe { CStr::from_ptr(ptr).to_string_lossy() }
}

pub fn clamp(value: u32, min: u32, max: u32) -> u32 {
    value.min(max).max(min)
}
