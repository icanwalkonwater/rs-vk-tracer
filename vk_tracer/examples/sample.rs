/// Sample file to help me design the API.
/// VERY subject to change.

fn main() {
    let forward_renderer = RendererCreator::builder()
        .auto_create_instance()
        .auto_create_device()
        .with_best_physical_device()
        .build_forward_renderer();

    let mesh = forward_renderer.create_mesh(
        &[Vertex(1, 1, 0), Vertex(-1, 1, 0), Vertex(1, -1, 0), Vertex(-1, -1, 0)],
        &[0, 1, 2, 1, 2, 3]
    );

    let camera = Camera::new_perspective(16f32/9f32, 60f32, 0.1f32, 100f32);

    let shader = forward_renderer.load_raster_shaders(RasterShaders::builder()
        .vertex("shaders/simple.vert.spv")
        .fragment("shaders/simple.frag.spv")
        .build()
    );

    let forward_pipeline = forward_renderer.create_forward_pipeline(mesh, shader);

    forward_renderer.draw(&camera, &[&forward_pipeline]);
}