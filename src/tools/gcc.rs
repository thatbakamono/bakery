use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::config::{CStandard, Distribution, OptimizationLevel};

pub(crate) struct Gcc {
    location: String,
}

impl Gcc {
    pub(crate) fn new(location: String) -> Gcc {
        Gcc { location }
    }

    pub(crate) fn compile_source_file(
        &self,
        distribution: Distribution,
        standard: CStandard,
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

        command.arg("-xc");

        command.arg(format!(
            "-std={}",
            match standard {
                CStandard::EightyNine => "c89",
                CStandard::NinetyNine => "c99",
                CStandard::Eleven => "c11",
                CStandard::Seventeen => "c17",
                CStandard::Twenty => "c20",
                CStandard::TwentyThree => "c23",
            }
        ));

        command.arg(format!(
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
