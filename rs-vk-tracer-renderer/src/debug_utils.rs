use ash::{extensions::ext, vk};
use log::{log, Level};
use std::{borrow::Cow, ffi::CStr, os::raw::c_char};

use crate::errors::Result;

const LAYER_KHRONOS: &str = "VK_LAYER_KHRONOS_validation\0";

pub fn vk_validation_layers_raw() -> Vec<*const c_char> {
    vec![LAYER_KHRONOS.as_ptr() as *const c_char]
}

pub struct DebugUtilsModule {
    debug_utils_ext: ext::DebugUtils,
    debug_utils_messenger: vk::DebugUtilsMessengerEXT,
}

impl DebugUtilsModule {
    pub fn new(entry: &ash::Entry, instance: &ash::Instance) -> Result<Self> {
        let debug_utils_ext = ext::DebugUtils::new(entry, instance);
        let debug_utils_messenger = {
            let create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::all())
                .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
                .pfn_user_callback(Some(vulkan_debug_callback));

            unsafe { debug_utils_ext.create_debug_utils_messenger(&create_info, None) }
        }?;

        Ok(Self {
            debug_utils_ext,
            debug_utils_messenger,
        })
    }
}

impl Drop for DebugUtilsModule {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils_ext
                .destroy_debug_utils_messenger(self.debug_utils_messenger, None);
        }
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;

    let message_id_number: i32 = callback_data.message_id_number as i32;
    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };
    let message = if callback_data.p_message.is_null() {
        Cow::from("No message")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    log!(
        severity_to_level(message_severity),
        "[{:?}] {} [{} ({})]",
        message_type,
        message,
        message_id_name,
        message_id_number
    );

    vk::FALSE
}

fn severity_to_level(severity: vk::DebugUtilsMessageSeverityFlagsEXT) -> Level {
    match severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => Level::Error,
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => Level::Warn,
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => Level::Info,
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => Level::Trace,
        _ => Level::Trace,
    }
}
