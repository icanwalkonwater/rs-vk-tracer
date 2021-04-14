use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;
use vk_tracer::prelude::VertexPosUv;
use vk_tracer::new::prelude::*;

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
        .with_extensions(&[
            VkTracerExtensions::PipelineRaytracing,
        ])
        .build(Some((&window, window.inner_size().into())))?;

    let my_swapchain_handle = graphics.create_swapchain_for_window(window.inner_size().into())?;

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

    let my_render_plan_handle = graphics.new_render_plan()
        .add_color_attachment(&my_swapchain_images_ref[0])
        // .add_depth_stencil_attachment(...)
        .add_subpass(SubpassBuilder::new()
            .graphics()
            .color_attachments([0])
        )
        .build();

    let mut my_render_targets_handles = Vec::with_capacity(my_swapchain_images_ref.len());
    for my_color_attachment_handle in my_swapchain_images_ref {
        my_render_targets_handles.push(graphics.allocate_render_target(my_render_plan_handle, [my_color_attachment_handle])?);
    }

    let my_mesh_pipeline_handle = graphics.create_forward_pipeline(my_render_plan_handle, "simple.vert", "simple.frag");
    // Bind an additional mesh
    graphics.bind_to_forward_pipeline(my_mesh_pipeline_handle, &[my_mesh_handle]);
    // Can unbind too
    // graphics.unbind_from_forward_pipeline(my_mesh_pipeline_handle, &[my_mesh_handle]);

    let mut my_renderer_handles = Vec::with_capacity(my_render_targets_handles.len());
    for my_render_target_handle in my_render_targets_handles {
        my_renderer_handles.push(
            graphics.new_renderer_from_plan(my_render_plan_handle, my_render_target_handle)
                .execute_pipeline(my_mesh_pipeline_handle)
                // .next_subpass()
                // .execute_pipeline(...)
                .build()
        );
    }

    loop {
        let render_target_index = graphics.get_next_swapchain_render_target_index(my_swapchain_handle);
        graphics.render_and_present(my_renderer_handles[render_target_index], my_swapchain_handle, render_target_index);
        // graphics.render(my_renderer_handles[render_target_index]);
        // graphics.present_swapchain(my_swapchain_handle, render_target_index);
    }

    Ok(())
}