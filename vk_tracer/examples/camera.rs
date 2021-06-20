use log::debug;
use nalgebra_glm as glm;
use vk_tracer::{
    ash::vk::ShaderStageFlags,
    prelude::*,
    shaderc::{OptimizationLevel, ShaderKind},
    utils::{FpsLimiter, ShaderCompiler},
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
                "vk_tracer/examples/shaders/camera.vert.glsl".into(),
                ShaderKind::Vertex,
                "main",
            )?,
            compiler.compile_and_return_file(
                "vk_tracer/examples/shaders/camera.frag.glsl".into(),
                ShaderKind::Fragment,
                "main",
            )?,
        )
    };

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("API Mockup")
        .with_resizable(true)
        .build(&event_loop)?;

    let mut graphics = VkTracerApp::builder()
        .pick_best_physical_device()
        .with_app_info("API Mockup".into(), (1, 0, 0))
        .with_debug_utils()
        .build(Some((&window, window.inner_size().into())))?;

    let swapchain = graphics.create_swapchain_with_surface()?;
    let plane = graphics.create_mesh_indexed(
        &[
            VertexXyz([1.0, 1.0, -1.0]),
            VertexXyz([1.0, -1.0, -1.0]),
            VertexXyz([1.0, 1.0, 1.0]),
            VertexXyz([1.0, -1.0, 1.0]),
            VertexXyz([-1.0, 1.0, -1.0]),
            VertexXyz([-1.0, -1.0, -1.0]),
            VertexXyz([-1.0, 1.0, 1.0]),
            VertexXyz([-1.0, -1.0, 1.0]),
        ],
        &[
            4, 2, 0, 2, 7, 3, 6, 5, 7, 1, 7, 5, 0, 3, 1, 4, 1, 5, 4, 6, 2, 2, 6, 7, 6, 4, 5, 1, 3,
            7, 0, 2, 3, 4, 0, 1,
        ],
    )?;

    #[derive(Copy, Clone, Uniform)]
    struct CameraUbo {
        model: glsl_layout::mat4,
        view: glsl_layout::mat4,
        proj: glsl_layout::mat4,
    }

    fn get_camera_ubo(window: &winit::window::Window) -> CameraUbo {
        CameraUbo {
            model: glm::identity::<f32, 4>().into(),
            view: glm::look_at_rh(
                &glm::vec3(7.0, -7.0, 5.0),
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 1.0, 0.0),
            )
            .into(),
            proj: glm::perspective(
                window.inner_size().width as f32 / window.inner_size().height as f32,
                (45f32).to_radians(),
                0.1,
                100.0,
            )
            .into(),
        }
    }

    let plane_ubo = graphics.create_ubo([get_camera_ubo(&window).std140()])?;

    let swapchain_images = graphics.get_images_from_swapchain(swapchain)?;
    let render_plan = graphics
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
        .add_color_attachment_present(swapchain_images[0])?
        .build()?;

    let render_targets = swapchain_images
        .into_iter()
        .map(|image| graphics.allocate_render_target(render_plan, &[image]))
        .collect::<Result<Vec<_>>>()?;

    let descriptor_set = graphics
        .new_descriptor_sets()
        .new_set(DescriptorSetBuilder::new().ubo(0, ShaderStageFlags::VERTEX))
        .build()?[0];

    graphics.write_descriptor_set_ubo(descriptor_set, 0, plane_ubo)?;

    let pipeline = graphics.create_forward_pipeline(
        render_plan,
        0,
        &[descriptor_set],
        vertex_shader,
        fragment_shader,
        plane,
    )?;

    let renderers = render_targets
        .iter()
        .copied()
        .map(|render_target| {
            graphics
                .new_renderer_from_plan(render_plan, render_target)
                .clear_color([0.1, 0.1, 0.2, 1.0])
                .execute_pipeline(pipeline.into())
                .build()
        })
        .collect::<Result<Vec<_>>>()?;

    let mut fps_limiter = FpsLimiter::new(60.0);
    event_loop.run(move |event, _, control| {
        *control = ControlFlow::Poll;

        if fps_limiter.should_render() {
            fps_limiter.new_frame();

            let (render_target_index, should_recreate_swapchain) = graphics
                .get_next_swapchain_render_target_index(swapchain)
                .unwrap();

            let should_recreate_swapchain = graphics
                .render_and_present(
                    renderers[render_target_index as usize],
                    swapchain,
                    render_target_index,
                )
                .unwrap()
                || should_recreate_swapchain;

            if should_recreate_swapchain {
                recreate_swapchain(
                    &mut graphics,
                    window.inner_size().into(),
                    swapchain,
                    render_plan,
                    &render_targets,
                    &renderers,
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
                    swapchain,
                    render_plan,
                    &render_targets,
                    &renderers,
                )
                .unwrap();

                graphics
                    .update_ubo(plane_ubo, [get_camera_ubo(&window).std140()])
                    .unwrap();
            }
            _ => (),
        }
    })
}

fn recreate_swapchain(
    graphics: &mut VkTracerApp,
    new_size: (u32, u32),
    swapchain: SwapchainHandle,
    render_plan: RenderPlanHandle,
    render_targets: &[RenderTargetHandle],
    renderers: &[RendererHandle],
) -> anyhow::Result<()> {
    graphics.recreate_swapchain(swapchain, new_size)?;
    let swapchain_images = graphics.get_images_from_swapchain(swapchain)?;
    for (render_target, image) in render_targets.iter().zip(swapchain_images.into_iter()) {
        graphics.recreate_render_target(render_plan, new_size, *render_target, [image])?;
    }
    for (renderer, render_target) in renderers
        .iter()
        .copied()
        .zip(render_targets.iter().copied())
    {
        graphics.recreate_renderer(renderer, render_target)?;
    }
    Ok(())
}
