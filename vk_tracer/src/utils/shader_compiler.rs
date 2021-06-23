use crate::{
    errors::{Result, VkTracerError},
    shaderc::Compiler,
};
use log::{info, warn};
use shaderc::{CompileOptions, EnvVersion, OptimizationLevel, TargetEnv};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};

pub struct ShaderCompiler<'a> {
    compiler: Compiler,
    options: CompileOptions<'a>,
}

impl<'a> ShaderCompiler<'a> {
    #[inline]
    pub fn new() -> Result<Self> {
        let compiler = Compiler::new().ok_or(VkTracerError::ShaderCompilerError(
            "Can't create shaderc compiler !",
        ))?;
        let mut options = CompileOptions::new().ok_or(VkTracerError::ShaderCompilerError(
            "Failed to create compile options !",
        ))?;
        options.set_target_env(TargetEnv::Vulkan, EnvVersion::Vulkan1_2 as _);
        options.set_generate_debug_info();
        options.set_warnings_as_errors();

        Ok(Self { compiler, options })
    }

    #[inline(always)]
    pub fn set_optimization_level(&mut self, level: OptimizationLevel) {
        self.options.set_optimization_level(level);
    }

    #[inline]
    pub fn edit_options(&mut self, edit: impl FnOnce(&mut CompileOptions)) {
        edit(&mut self.options);
    }

    #[inline]
    pub fn compile_and_return_file(
        &mut self,
        mut filename: PathBuf,
        kind: shaderc::ShaderKind,
        entry_point: &str,
    ) -> Result<File> {
        let src = {
            let mut src = String::new();
            File::open(&filename)?.read_to_string(&mut src)?;
            src
        };

        let compiled = self.compiler.compile_into_spirv(
            &src,
            kind,
            filename.file_name().unwrap().to_str().unwrap(),
            entry_point,
            Some(&self.options),
        )?;

        info!(
            "Compiled {} as {:?} shader with {} warnings",
            filename.file_name().unwrap().to_str().unwrap(),
            kind,
            compiled.get_num_warnings(),
        );

        if compiled.get_num_warnings() > 0 {
            warn!("{}", compiled.get_warning_messages());
        }

        filename.set_extension("spv");

        let mut dst = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(filename)?;

        dst.write_all(compiled.as_binary_u8())?;

        Ok(dst)
    }

    #[inline(always)]
    pub fn compile(
        &mut self,
        filename: PathBuf,
        kind: shaderc::ShaderKind,
        entry_point: &str,
    ) -> Result<()> {
        self.compile_and_return_file(filename, kind, entry_point)
            .map(|_| ())
    }
}
