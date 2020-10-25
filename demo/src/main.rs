use simplelog::{
    ConfigBuilder, LevelFilter, LevelPadding, SimpleLogger, TermLogger, TerminalMode, ThreadLogMode,
};
use vk_tracer::prelude::*;
use winit::{event_loop::EventLoop, window::WindowBuilder};

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

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

    let instance = VtInstance::create(
        VtAppInfo {
            name: "Demo App",
            version: (1, 0, 0),
        },
        &window,
    )?;

    let surface = VtSurface::create(&instance, &window, {
        let size = window.inner_size();
        (size.width, size.height)
    })?;

    let adapter = instance.request_adapter(
        &surface,
        VtAdapterRequirements::default_from_window(&window)?,
    )?;

    let device = adapter.create_device(&instance)?;

    Ok(())
}
