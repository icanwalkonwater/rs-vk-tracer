use log::debug;
use simplelog::{
    ConfigBuilder, LevelFilter, LevelPadding, SimpleLogger, TermLogger, TerminalMode, ThreadLogMode,
};
use vk_tracer::{command_recorder::VtTransferCommands, prelude::*};
// use winit::{event_loop::EventLoop, window::WindowBuilder};
use vk_tracer::descriptor_sets::{
    DescriptorSetBindingDescription, DescriptorSetBindingWriteDescription,
    DescriptorSetDescription, DescriptorType, ShaderStage,
};
use winit::window::Window;

const LOG_LEVEL: LevelFilter = LevelFilter::Trace;

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

    copy_back_and_forth(&device)?;

    create_descriptor_set(&device)?;

    Ok(())
}

fn copy_back_and_forth(device: &VtDevice) -> anyhow::Result<()> {
    let data = [1, 2, 3, 4];
    debug!("Copy back and forth {:?}", &data);

    let mut gpu_buffer = device.create_buffer_with_staging(&BufferDescription {
        size: std::mem::size_of_val(&data) as DeviceSize,
        usage: BufferUsage::STORAGE_BUFFER,
    })?;

    gpu_buffer.stage(&data)?;
    gpu_buffer.upload()?;
    let gpu_buffer = gpu_buffer.into_dst();

    let mut cpu_buffer = device.create_staging_buffer_for(&gpu_buffer)?;

    let mut recorder = device.get_transient_transfer_encoder()?;

    recorder.copy_buffer_to_buffer(&gpu_buffer, &mut cpu_buffer)?;

    recorder.finish()?.submit()?;

    let data = cpu_buffer.retrieve()?;
    debug!("{:?}", data);

    Ok(())
}

fn create_descriptor_set(device: &VtDevice) -> anyhow::Result<()> {
    debug!("Create descriptor set 0");

    #[derive(Copy, Clone, Eq, PartialEq, Hash)]
    enum SetNames {
        Main,
    }

    let descriptor_set_manager =
        device.create_descriptor_set_manager(&[DescriptorSetDescription {
            key: SetNames::Main,
            bindings: vec![
                DescriptorSetBindingDescription {
                    binding: 0,
                    ty: DescriptorType::STORAGE_BUFFER,
                    len: 1,
                    stages: ShaderStage::COMPUTE,
                },
                DescriptorSetBindingDescription {
                    binding: 1,
                    ty: DescriptorType::UNIFORM_BUFFER,
                    len: 1,
                    stages: ShaderStage::COMPUTE,
                },
            ],
        }])?;

    let mut out_buffer = device.create_buffer::<u32>(&BufferDescription {
        size: (std::mem::size_of::<u32>() * 16) as DeviceSize,
        usage: BufferUsage::STORAGE_BUFFER,
    })?;

    let values = [1u32, 2];
    let mut values_buffer = device.create_buffer_with_staging(&BufferDescription {
        size: std::mem::size_of_val(&values) as DeviceSize,
        usage: BufferUsage::UNIFORM_BUFFER,
    })?;
    values_buffer.stage(&values)?;
    values_buffer.upload()?;
    let values_buffer = values_buffer.into_dst();

    descriptor_set_manager.write_set(
        SetNames::Main,
        &[
            DescriptorSetBindingWriteDescription::Buffer {
                binding: 0,
                buffer: out_buffer.raw_handle(),
            },
            DescriptorSetBindingWriteDescription::Buffer {
                binding: 1,
                buffer: values_buffer.raw_handle(),
            },
        ],
    );

    debug!("Everything went fine");
    Ok(())
}
