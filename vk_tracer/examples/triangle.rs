use vk_tracer::{
    prelude::*,
    shaderc::{OptimizationLevel, ShaderKind},
    utils::{FpsLimiter, ShaderCompiler},
    RenderPlanHandle, RenderTargetHandle, RendererHandle, SwapchainHandle,
};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    // Compile shaders
    let (vertex_shader, fragment_shader) = {
        let mut compiler = ShaderCompiler::new()?;
        compiler.set_optimization_level(OptimizationLevel::Performance);

        (
            compiler.compile_and_return_file(
                "vk_tracer/examples/shaders/triangle.vert.glsl".into(),
                ShaderKind::Vertex,
                "main",
            )?,
            compiler.compile_and_return_file(
                "vk_tracer/examples/shaders/triangle.frag.glsl".into(),
                ShaderKind::Fragment,
                "main",
            )?,
        )
    };

    // Create window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("API Mockup")
        .with_resizable(true)
        .build(&event_loop)?;

    // Create app
    let mut graphics = VkTracerApp::builder()
        .pick_best_physical_device()
        .with_app_info("API Mockup".into(), (1, 0, 0))
        .with_debug_utils()
        .build(Some((&window, window.inner_size().into())))?;

    // Create a swapchain
    let my_swapchain_handle = graphics.create_swapchain_with_surface()?;

    // Create a mesh (the triangle)
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

    // Create a color attachment for each image in the swapchain
    let my_swapchain_images_ref = graphics.get_images_from_swapchain(my_swapchain_handle)?;

    // Create a render plan (vulkan render pass), listing the attachments and subpasses of the render
    let my_render_plan_handle = graphics
        .new_render_plan()
        .add_subpass(
            SubpassBuilder::new().graphics().color_attachments([0]),
            Some(
                SubpassDependency::builder()
                    .src_subpass(SUBPASS_EXTERNAL)
                    .dst_subpass(0)
                    .src_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .src_access_mask(AccessFlags::empty())
                    .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .build(),
            ),
        )
        .add_color_attachment_present(my_swapchain_images_ref[0])?
        .build()?;

    // Allocate one render target (vulkan framebuffer) per swapchain image
    let mut my_render_targets_handles = Vec::with_capacity(my_swapchain_images_ref.len());
    for my_color_attachment_handle in my_swapchain_images_ref {
        my_render_targets_handles.push(
            graphics
                .allocate_render_target(my_render_plan_handle, &[my_color_attachment_handle])?,
        );
    }

    // Create a (forward) pipeline for our triangle
    let my_mesh_pipeline_handle = graphics.create_forward_pipeline(
        my_render_plan_handle,
        0,
        vertex_shader,
        fragment_shader,
        my_mesh_handle,
    )?;

    // Create a renderer for each render target
    let mut my_renderers_handles = Vec::with_capacity(my_render_targets_handles.len());
    for my_render_target_handle in my_render_targets_handles.iter().copied() {
        my_renderers_handles.push(
            graphics
                .new_renderer_from_plan(my_render_plan_handle, my_render_target_handle)
                .execute_pipeline(my_mesh_pipeline_handle.into())
                .build()?,
        );
    }

    // Limit fps to 60 for convenience
    let mut fps_limiter = FpsLimiter::new(60.0);

    event_loop.run(move |event, _, control| {
        // Run as fast as possible
        *control = ControlFlow::Poll;

        // Don't draw more that 60fps
        if fps_limiter.should_render() {
            fps_limiter.new_frame();

            // Acquire the next render target to draw to
            let (render_target_index, should_recreate_swapchain) = graphics
                .get_next_swapchain_render_target_index(my_swapchain_handle)
                .unwrap();

            // And draw then present through the swapchain
            let should_recreate_swapchain = graphics
                .render_and_present(
                    my_renderers_handles[render_target_index as usize],
                    my_swapchain_handle,
                    render_target_index,
                )
                .unwrap() || should_recreate_swapchain;

            if should_recreate_swapchain {
                recreate_swapchain(
                    &mut graphics,
                    window.inner_size().into(),
                    my_swapchain_handle,
                    my_render_plan_handle,
                    &my_render_targets_handles,
                    &my_renderers_handles,
                )
                    .unwrap();
            }
        }

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
                event: WindowEvent::Resized(new_size),
                ..
            } => {
                recreate_swapchain(
                    &mut graphics,
                    new_size.into(),
                    my_swapchain_handle,
                    my_render_plan_handle,
                    &my_render_targets_handles,
                    &my_renderers_handles,
                )
                .unwrap();
            }
            _ => (),
        }
    });
}

fn recreate_swapchain(
    graphics: &mut VkTracerApp,
    new_size: (u32, u32),
    swapchain: SwapchainHandle,
    render_plan: RenderPlanHandle,
    render_targets: &[RenderTargetHandle],
    renderers: &[RendererHandle],
) -> anyhow::Result<()> {
    // Recreate swapchain
    graphics.recreate_swapchain(swapchain, new_size)?;
    let swapchain_images = graphics.get_images_from_swapchain(swapchain)?;

    // Recreate render targets
    for (render_target, image) in render_targets.iter().zip(swapchain_images.into_iter()) {
        graphics.recreate_render_target(render_plan, new_size, *render_target, [image])?;
    }

    // Recreate renderers
    for (renderer, render_target) in renderers
        .iter()
        .copied()
        .zip(render_targets.iter().copied())
    {
        graphics.recreate_renderer(renderer, render_target)?;
    }
    Ok(())
}
