use crate::errors::{Result, VkTracerError};
use log::info;
use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

pub struct ShaderCompiler<'a> {
    compiler: shaderc::Compiler,
    options: Option<shaderc::CompileOptions<'a>>,
}

impl<'a> ShaderCompiler<'a> {
    pub fn new() -> Result<Self> {
        Ok(Self {
            compiler: shaderc::Compiler::new().ok_or(VkTracerError::ShaderCompilerError(
                "Can't create shaderc compiler !",
            ))?,
            options: None,
        })
    }

    pub fn set_options(mut self, options: shaderc::CompileOptions<'a>) -> ShaderCompiler<'a> {
        self.options = Some(options);
        self
    }

    pub fn compile(
        &mut self,
        filename: PathBuf,
        kind: shaderc::ShaderKind,
        entry_point: &str,
    ) -> Result<()> {
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
            self.options.as_ref(),
        )?;

        File::create(format!("{}.spv", filename.display()))?.write_all(compiled.as_binary_u8())?;

        info!(
            "Compiled {} ({:?} shader)",
            filename.file_name().unwrap().to_str().unwrap(),
            kind
        );
        Ok(())
    }
}
