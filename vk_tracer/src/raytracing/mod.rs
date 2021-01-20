use ash::extensions::khr;
use std::ffi::CStr;

pub(crate) fn required_device_rt_extensions() -> [&'static CStr; 3] {
    [
        khr::AccelerationStructure::name(),
        khr::DeferredHostOperations::name(),
        khr::RayTracingPipeline::name(),
    ]
}
