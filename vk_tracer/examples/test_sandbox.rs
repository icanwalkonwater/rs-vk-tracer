use log::info;
use std::{
    fs::File,
    io::{Read, Write},
    ops::Add,
    slice::from_ref,
    sync::Arc,
    time::{Duration, Instant},
};
use vk_tracer::prelude::*;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Test Sandbox")
        .with_resizable(true)
        .build(&event_loop)?;

    let renderer_creator = RendererCreator::builder()
        .pick_best_physical_device()
        .with_app_info(AppInfo {
            name: "Test Sandbox",
            version: (0, 0, 0),
        })
        .with_debug_utils(true)
        //.with_hardware_raytracing()
        .build_with_window(&window, window.inner_size().into())?;
    let mut renderer_creator = renderer_creator.lock();

    let mesh = renderer_creator.create_mesh(
        &[
            VertexPosUv {
                pos: [1.0, 1.0, 0.0],
                uv: [1.0, 0.0],
            },
            VertexPosUv {
                pos: [-1.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
            VertexPosUv {
                pos: [0.0, -1.0, 0.0],
                uv: [0.5, 1.0],
            },
        ],
        &[0, 1, 2],
    )?;

    {
        let mut compiler = shaderc::Compiler::new().unwrap();
        File::create("vk_tracer/examples/shaders/simple.vert.spv")
            .unwrap()
            .write_all(
                compiler
                    .compile_into_spirv(
                        &{
                            let mut src = String::new();
                            File::open("vk_tracer/examples/shaders/simple.vert")
                                .unwrap()
                                .read_to_string(&mut src)
                                .unwrap();
                            src
                        },
                        shaderc::ShaderKind::Vertex,
                        "simple.vert",
                        "main",
                        None,
                    )
                    .unwrap()
                    .as_binary_u8(),
            )
            .unwrap();

        File::create("vk_tracer/examples/shaders/simple.frag.spv")
            .unwrap()
            .write_all(
                compiler
                    .compile_into_spirv(
                        &{
                            let mut src = String::new();
                            File::open("vk_tracer/examples/shaders/simple.frag")
                                .unwrap()
                                .read_to_string(&mut src)
                                .unwrap();
                            src
                        },
                        shaderc::ShaderKind::Fragment,
                        "frag.vert",
                        "main",
                        None,
                    )
                    .unwrap()
                    .as_binary_u8(),
            )
            .unwrap();
    }

    let forward_renderer = renderer_creator.new_forward_renderer(
        mesh,
        File::open("vk_tracer/examples/shaders/simple.vert.spv").unwrap(),
        File::open("vk_tracer/examples/shaders/simple.frag.spv").unwrap(),
    )?;

    let start = Instant::now();
    let mut last_fps_check = Instant::now();
    let mut frames = 0.0;

    event_loop.run_return(move |event, _, control_flow| {
        // Run as fast a we can
        //*control_flow = ControlFlow::Poll;
        *control_flow =
            ControlFlow::WaitUntil(Instant::now().add(Duration::from_millis(1000 / 60)));

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
            } => renderer_creator.resize(size.into()).unwrap(),
            Event::RedrawRequested(_) => {
                frames += 1.0;
                renderer_creator.draw(from_ref(&forward_renderer)).unwrap();
                window.request_redraw();
            }
            _ => (),
        }
    });

    Ok(())
}
