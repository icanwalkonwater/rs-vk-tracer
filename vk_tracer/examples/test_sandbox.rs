use std::sync::Arc;
use vk_tracer::prelude::*;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Test Sandbox")
        .with_resizable(true)
        .build(&event_loop)?;

    let mut renderer_creator = RendererCreator::builder()
        .pick_best_physical_device()
        .with_app_info(AppInfo {
            name: "Test Sandbox",
            version: (0, 0, 0),
        })
        .with_debug_utils(true)
        //.with_hardware_raytracing()
        .build_with_window(Some(&window), window.inner_size().into())?;

    let mesh = renderer_creator.lock().create_mesh(
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
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => renderer_creator.lock().resize(size.into()).unwrap(),
            _ => (),
        }
    });
}
