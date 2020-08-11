use ash::extensions::khr;
use ash::vk;
use winit::window::Window;

pub struct SurfaceModule {
    pub surface_ext: khr::Surface,
    pub surface_khr: vk::SurfaceKHR,
}

use crate::errors::Result;

impl SurfaceModule {
    pub fn new(entry: &ash::Entry, instance: &ash::Instance, window: &Window) -> Result<Self> {
        let surface_ext = khr::Surface::new(entry, instance);
        let surface = unsafe { ash_window::create_surface(entry, instance, window, None)? };

        Ok(Self {
            surface_ext,
            surface_khr: surface,
        })
    }
}

impl Drop for SurfaceModule {
    fn drop(&mut self) {
        unsafe { self.surface_ext.destroy_surface(self.surface_khr, None) }
    }
}
