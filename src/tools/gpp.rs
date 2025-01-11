use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::config::{CPPStandard, Distribution, OptimizationLevel, ToolchainConfiguration};

pub(crate) struct GPP {
    location: String,
}

impl GPP {
    fn new(location: String) -> GPP {
        GPP { location }
    }

    pub(crate) fn locate(toolchain_configuration: &ToolchainConfiguration) -> Option<GPP> {
        if let Some(ref gpp_location) = toolchain_configuration.gpp_location {
            Some(GPP::new(gpp_location.clone()))
        } else if cfg!(target_os = "windows") {
            Some(GPP::new(
                which::which("g++.exe").ok()?.to_string_lossy().into_owned(),
            ))
        } else if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Some(GPP::new(
                which::which("g++").ok()?.to_string_lossy().into_owned(),
            ))
        } else {
            None
        }
    }

    pub(crate) fn compile_source_file(
        &self,
        distribution: Distribution,
        standard: CPPStandard,
        optimization: OptimizationLevel,
        source_file: &impl AsRef<Path>,
        output_file: &impl AsRef<Path>,
        includes: &Vec<String>,
        enable_all_warnings: bool,
        treat_all_warnings_as_errors: bool,
        additional_pre_arguments: &Vec<String>,
        additional_post_arguments: &Vec<String>,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location);

        for additional_pre_argument in additional_pre_arguments {
            command.arg(additional_pre_argument);
        }

        command.arg("-c");

        if distribution == Distribution::DynamicLibrary {
            command.arg("-fPIC");
        }

        command.arg("-xc++");

        command.arg(&format!(
            "-std={}",
            match standard {
                CPPStandard::NinetyEight => "c++98",
                CPPStandard::Three => "c++3",
                CPPStandard::Eleven => "c++11",
                CPPStandard::Fourteen => "c++14",
                CPPStandard::Seventeen => "c++17",
                CPPStandard::Twenty => "c++20",
                CPPStandard::TwentyThree => "c++23",
                CPPStandard::TwentySix => "c++26",
            }
        ));

        command.arg(&format!(
            "-O{}",
            match optimization {
                OptimizationLevel::Zero => "0",
                OptimizationLevel::One => "1",
                OptimizationLevel::Two => "2",
                OptimizationLevel::Three => "3",
                OptimizationLevel::Four => "fast",
                OptimizationLevel::Size => "s",
                OptimizationLevel::Debug => "g",
            }
        ));

        if enable_all_warnings {
            command.arg("-Wall");
            command.arg("-Wpedantic");
        }

        if treat_all_warnings_as_errors {
            command.arg("-Werror");
        }

        command.arg(source_file.as_ref());

        command.arg(format!("-o{}", output_file.as_ref().display()));

        for include in includes {
            command.arg(format!("-I{}", include));
        }

        for additional_post_argument in additional_post_arguments {
            command.arg(additional_post_argument);
        }

        let output = command.output().unwrap();

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    pub(crate) fn link_object_files(
        &self,
        distribution: Distribution,
        object_files: &Vec<PathBuf>,
        output_file: &impl AsRef<Path>,
        includes: &Vec<String>,
        libraries: &Vec<String>,
        library_search_paths: &Vec<String>,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location);

        if distribution == Distribution::DynamicLibrary {
            command.arg("-shared");
        }

        for object_file in object_files {
            command.arg(object_file);
        }

        command.arg(format!("-o{}", output_file.as_ref().display()));

        for include in includes {
            command.arg(format!("-I{}", include));
        }

        for library_search_path in library_search_paths {
            command.arg(format!("-L{}", library_search_path));
        }

        for library in libraries {
            command.arg(format!("-l{}", library));
        }

        let output = command.output().unwrap();

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}
