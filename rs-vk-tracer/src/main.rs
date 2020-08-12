use simplelog::{
    ConfigBuilder, LevelFilter, LevelPadding, SimpleLogger, TermLogger, TerminalMode, ThreadLogMode,
};

use rs_vk_tracer_renderer::{
    create_window, vulkan_app::VulkanApp, winit::dpi::LogicalSize, AppInfo,
};

const LOG_LEVEL: LevelFilter = LevelFilter::Debug;

fn main() -> anyhow::Result<()> {
    setup_logger();

    let app_info = AppInfo::new("Test App", 1, 0, 0);
    let (event_loop, window) = create_window(&app_info, LogicalSize::new(400, 200), false)?;

    let app = VulkanApp::new(app_info, &window)?;

    Ok(())
}

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
