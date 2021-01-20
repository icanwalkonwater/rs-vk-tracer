use crate::errors::Result;
use ash::{extensions::ext, vk};
use log::{info, log, Level};
use std::{borrow::Cow, ffi::CStr};

pub(crate) struct VtDebugUtils {
    pub(crate) loader: ext::DebugUtils,
    messenger: vk::DebugUtilsMessengerEXT,
}

impl VtDebugUtils {
    pub(crate) fn new(entry: &ash::Entry, instance: &ash::Instance) -> Result<Self> {
        let loader = ext::DebugUtils::new(entry, instance);
        let messenger = unsafe {
            loader.create_debug_utils_messenger(
                &vk::DebugUtilsMessengerCreateInfoEXT::builder()
                    .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::all())
                    .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
                    .pfn_user_callback(Some(vulkan_debug_callback)),
                None,
            )?
        };

        info!("Debug utils setup !");

        Ok(Self { loader, messenger })
    }
}

impl Drop for VtDebugUtils {
    fn drop(&mut self) {
        unsafe {
            self.loader
                .destroy_debug_utils_messenger(self.messenger, None);
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
