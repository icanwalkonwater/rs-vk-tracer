use vk_tracer::renderer_creator::RendererCreator;
use vk_tracer::AppInfo;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().with_title("Test Sandbox").build(&event_loop)?;

    let _renderer_creator = RendererCreator::builder()
        .pick_best_physical_device()
        .with_app_info(AppInfo {
            name: "Test Sandbox",
            version: (0, 0, 0),
        })
        .with_validation_layer("VK_LAYER_KHRONOS_validation")
        .with_hardware_raytracing()
        .build_with_window(Some(&window), window.inner_size().into())?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
