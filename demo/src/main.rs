use log::debug;
use simplelog::{
    ConfigBuilder, LevelFilter, LevelPadding, SimpleLogger, TermLogger, TerminalMode, ThreadLogMode,
};
use vk_tracer::prelude::*;
// use winit::{event_loop::EventLoop, window::WindowBuilder};
use winit::window::Window;

const LOG_LEVEL: LevelFilter = LevelFilter::Debug;

fn setup_logger() {
    let config = ConfigBuilder::new()
        .set_time_to_local(true)
        .set_level_padding(LevelPadding::Right)
        // Thread
        .set_thread_mode(ThreadLogMode::Both)
        .set_thread_level(LevelFilter::Error)
        // Code path
        .set_location_level(LevelFilter::Trace)
        .set_target_level(LevelFilter::Trace)
        .build();

    TermLogger::init(LOG_LEVEL, config.clone(), TerminalMode::Mixed).unwrap_or_else(|_| {
        SimpleLogger::init(LOG_LEVEL, config).expect("Failed to setup a logger !");
    })
}

fn main() -> anyhow::Result<()> {
    setup_logger();

    /*let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;*/

    let instance = VtInstance::create(
        VtAppInfo {
            name: "Demo App",
            version: (1, 0, 0),
        },
        // &window,
        Option::<&Window>::None,
    )?;

    /*let surface = VtSurface::create(&instance, &window, {
        let size = window.inner_size();
        (size.width, size.height)
    })?;*/

    let adapter = instance.request_adapter(
        // ..VtAdapterRequirements::default_from_window(surface.clone(), &window)?
        VtAdapterRequirements {
            validation_layers: vec!["VK_LAYER_KHRONOS_validation"],
            required_extensions: Vec::new(),
            ..Default::default()
        },
    )?;

    let device = adapter.create_device(&instance)?;

    let data = [1, 2, 3, 4];
    let mut gpu_buffer = device.create_buffer_with_staging(&BufferDescription {
        size: std::mem::size_of_val(&data) as DeviceSize,
        usage: BufferUsage::STORAGE_BUFFER,
    })?;

    gpu_buffer.stage(&data)?;
    let gpu_buffer = gpu_buffer.upload()?;

    let mut cpu_buffer = device.create_staging_buffer_for(&gpu_buffer)?;
    {
        let mut recorder = device.get_transient_transfer_recorder()?;
        recorder.copy_buffer_to_buffer(&gpu_buffer, &mut cpu_buffer)?;
        recorder.submit()?;
    }

    let data = cpu_buffer.retrieve()?;
    debug!("Result data: {:?}", data);

    Ok(())
}
