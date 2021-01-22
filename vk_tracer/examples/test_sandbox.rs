use vk_tracer::prelude::*;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Test Sandbox")
        .build(&event_loop)?;

    let renderer_creator = RendererCreator::builder()
        .pick_best_physical_device()
        .with_app_info(AppInfo {
            name: "Test Sandbox",
            version: (0, 0, 0),
        })
        .with_debug_utils(true)
        .with_validation_layer("VK_LAYER_KHRONOS_validation")
        .with_hardware_raytracing()
        .build_with_window(Some(&window), window.inner_size().into())?;

    let mesh = renderer_creator.create_mesh(
        &[
            VertexPosUv {
                pos: [1.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
            VertexPosUv {
                pos: [-1.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
            VertexPosUv {
                pos: [0.0, -1.0, 0.0],
                uv: [0.0, 0.0],
            },
        ],
        &[0, 1, 2],
    )?;

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
