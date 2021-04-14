use vk_tracer::{new::prelude::*};
use winit::{event_loop::EventLoop, window::WindowBuilder};
use winit::platform::run_return::EventLoopExtRunReturn;
use winit::event_loop::ControlFlow;
use winit::event::{Event, WindowEvent, KeyboardInput, ElementState, VirtualKeyCode};
use std::fs::File;
use log::info;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("API Mockup")
        .with_resizable(true)
        .build(&event_loop)?;

    let mut graphics = VkTracerApp::builder()
        .pick_best_physical_device()
        .with_app_info("API Mockup".into(), (1, 0, 0))
        .with_debug_utils()
        .with_extensions(&[VkTracerExtensions::PipelineRaytracing])
        .build(Some((&window, window.inner_size().into())))?;

    info!("Graphics created");

    let my_swapchain_handle = graphics.create_swapchain_for_window(window.inner_size().into())?;

    info!("Swapchain created");

    let my_mesh_handle = graphics.create_mesh_indexed(
        &[
            VertexXyzUv {
                xyz: [1.0, 1.0, 0.0],
                uv: [1.0, 0.0],
            },
            VertexXyzUv {
                xyz: [-1.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
            VertexXyzUv {
                xyz: [0.0, -1.0, 0.0],
                uv: [0.5, 1.0],
            },
        ],
        &[0, 1, 2],
    )?;

    info!("Mesh created");

    // Create a color attachment for each image in the swapchain
    let my_swapchain_images_ref = graphics.get_images_from_swapchain(my_swapchain_handle)?;

    let my_render_plan_handle = graphics
        .new_render_plan(Some(SubpassDependency::builder()
            .src_subpass(ash::vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(ash::vk::AccessFlags::empty())
            .dst_stage_mask(ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .build())
        )
        .add_color_attachment_present(my_swapchain_images_ref[0])?
        .build()?;

    info!("Render plan created");

    let mut my_render_targets_handles = Vec::with_capacity(my_swapchain_images_ref.len());
    for my_color_attachment_handle in my_swapchain_images_ref {
        my_render_targets_handles.push(
            graphics.allocate_render_target(my_render_plan_handle, &[my_color_attachment_handle])?,
        );
    }

    info!("Render target created");

    let my_mesh_pipeline_handle = graphics.create_forward_pipeline(
        my_render_plan_handle,
        0,
        File::open("vk_tracer/examples/shaders/simple.vert.spv")?,
        File::open("vk_tracer/examples/shaders/simple.frag.spv")?,
        my_mesh_handle,
    )?;

    info!("Pipeline created");

    let mut my_renderer_handles = Vec::with_capacity(my_render_targets_handles.len());
    for my_render_target_handle in my_render_targets_handles {
        my_renderer_handles.push(
            graphics
                .new_renderer_from_plan(my_render_plan_handle, my_render_target_handle)
                .execute_pipeline(my_mesh_pipeline_handle.into())
                .build()?,
        );
    }

    info!("Renderer created");

    event_loop.run(move |event, _, control| {
        *control = ControlFlow::Poll;

        let (render_target_index, is_suboptimal) = graphics.get_next_swapchain_render_target_index(my_swapchain_handle).unwrap();
        graphics.render_and_present(
            my_renderer_handles[render_target_index as usize],
            my_swapchain_handle,
            render_target_index,
        ).unwrap();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control = ControlFlow::Exit,
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
            } => *control = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(_size),
                ..
            } => todo!(),
            _ => (),
        }
    });
}
