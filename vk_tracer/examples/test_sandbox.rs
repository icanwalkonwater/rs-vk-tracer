use log::info;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
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
        .build_with_window(&window, window.inner_size().into())?;

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

    let start = Instant::now();
    let mut last_fps_check = Instant::now();
    let mut frames = 0.0;

    event_loop.run(move |event, _, control_flow| {
        // Run as fast a we can
        *control_flow = ControlFlow::Poll;

        if last_fps_check.elapsed().as_millis() >= 1000 {
            last_fps_check = Instant::now();
            info!("FPS: {}", frames / start.elapsed().as_secs_f64());
        }

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
