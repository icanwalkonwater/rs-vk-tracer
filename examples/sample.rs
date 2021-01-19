/// Sample file to help me design the API.
/// VERY subject to change.

fn main() {
    let render_manager = VkRenderManager::builder()
        .pick_best_device()
        .enable_forward_renderer()
        .build();

    let mesh = render_manager.create_mesh(
        &[
            Vertex(1, 1, 0), Vertex(-1, 1, 0), Vertex(1, -1, 0), Vertex(-1, -1, 0),
        ],
        &[0, 1, 2, 1, 2, 3]
    );

    let camera = Camera::new_perspective(16f32/9f32, 60f32, 0.1f32, 100f32);

    let shader = render_manager.load_raster_shaders(RasterShaders::builder()
        .vertex("shaders/simple.vert.spv")
        .fragment("shaders/simple.frag.spv")
        .build()
    );

    let forward_pipeline = render_manager.create_forward_pipeline(mesh, shader);

    render_manager.draw_forward(&[forward_pipeline]);
}