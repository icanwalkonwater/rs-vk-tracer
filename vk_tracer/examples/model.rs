use nalgebra_glm as glm;
use vk_tracer::{
    ash::vk::ShaderStageFlags,
    prelude::*,
    shaderc::{OptimizationLevel, ShaderKind},
    utils::{Camera, FpsLimiter, ShaderCompiler},
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
                "vk_tracer/examples/shaders/model.vert.glsl".into(),
                ShaderKind::Vertex,
                "main",
            )?,
            compiler.compile_and_return_file(
                "vk_tracer/examples/shaders/model.frag.glsl".into(),
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
    //
    // let (gltf, buffers, textures) = gltf::import("vk_tracer/examples/models/cube.gltf")?;
    //
    // for mesh in gltf.meshes() {
    //     debug!("Mesh {}:", mesh.index());
    //     for primitive in mesh.primitives() {
    //         debug!(" - Primitive {}:", primitive.index());
    //         for (sem, accessor) in primitive.attributes() {
    //             debug!("  - Attribute: {:?}", sem);
    //             debug!("    Type: {:?}", accessor.data_type());
    //             debug!("    Count: {}, Size: {}, Offset: {}", accessor.count(), accessor.size(), accessor.offset());
    //         }
    //
    //         debug!("  - Vertex positions:");
    //         let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
    //         reader.read_positions().unwrap()
    //             .for_each(|pos| debug!("   - {:?}", pos));
    //
    //         debug!("  - Indices:");
    //         let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
    //         reader.read_indices().unwrap().into_u32().for_each(|index| debug!("    {}", index));
    //     }
    // }

    let swapchain = graphics.create_swapchain_with_surface()?;
    let suzanne =
        graphics.load_first_mesh::<VertexXyzUvNorm>("vk_tracer/examples/models/suzanne.glb")?;

    #[derive(Copy, Clone, Uniform)]
    struct CameraUbo {
        mvp: glsl_layout::mat4,
        light_position: glsl_layout::vec3,
    }

    let mut camera = Camera::new_perspective(glm::vec3(5.0, 4.0, 4.0), glm::zero(), 1.0, 70.0);
    camera.aspect_auto(window.inner_size().into());

    fn get_camera_ubo(camera: &Camera) -> CameraUbo {
        CameraUbo {
            mvp: camera.compute_mvp(&glm::identity()).into(),
            light_position: glm::vec3(4.0, 1.0, 6.0).into(),
        }
    }

    let camera_ubo = graphics.create_ubo([get_camera_ubo(&camera).std140()])?;

    let swapchain_images = graphics.get_images_from_swapchain(swapchain)?;
    let depth_image = graphics.create_depth_texture(swapchain)?;

    let render_plan = graphics
        .new_render_plan()
        .add_subpass(
            SubpassBuilder::new()
                .graphics()
                .color_attachments([0])
                .depth_stencil_attachment(1),
            Some(
                SubpassDependency::builder()
                    .src_subpass(SUBPASS_EXTERNAL)
                    .dst_subpass(0)
                    .src_stage_mask(
                        PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    )
                    .src_access_mask(AccessFlags::empty())
                    .dst_stage_mask(
                        PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    )
                    .dst_access_mask(
                        AccessFlags::COLOR_ATTACHMENT_WRITE
                            | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .build(),
            ),
        )
        .add_color_attachment_present(swapchain_images[0])?
        .set_clear_color(0, [0.1, 0.1, 0.2, 1.0])
        .add_depth_attachment(depth_image)?
        .set_clear_depth_stencil(1, 1.0, 0)
        .build()?;

    let render_targets = swapchain_images
        .into_iter()
        .map(|image| graphics.allocate_render_target(render_plan, &[image, depth_image]))
        .collect::<Result<Vec<_>>>()?;

    let descriptor_set = graphics
        .new_descriptor_sets()
        .new_set(
            DescriptorSetBuilder::new()
                .ubo(0, ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT),
        )
        .build()?[0];

    graphics.write_descriptor_set_ubo(descriptor_set, 0, camera_ubo)?;

    let pipeline = graphics.create_forward_pipeline(
        render_plan,
        0,
        &[descriptor_set],
        vertex_shader,
        fragment_shader,
        suzanne,
    )?;

    let renderers = render_targets
        .iter()
        .copied()
        .map(|render_target| {
            graphics
                .new_renderer_from_plan(render_plan, render_target)
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

                camera.aspect_auto(window.inner_size().into());
                graphics
                    .update_ubo(camera_ubo, [get_camera_ubo(&camera).std140()])
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
