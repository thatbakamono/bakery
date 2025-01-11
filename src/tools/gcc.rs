use super::{Archiver, CCompiler, CppCompiler};
use crate::config::{CStandard, CppStandard, Distribution, OptimizationLevel};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) struct GccFlavorArchiver {
    location: String,
}

impl GccFlavorArchiver {
    pub(crate) fn new(location: String) -> GccFlavorArchiver {
        GccFlavorArchiver { location }
    }
}

impl Archiver for GccFlavorArchiver {
    fn archive_object_files(
        &self,
        object_files: &[PathBuf],
        output_file: &Path,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location);

        command.arg("rcs");
        command.arg(output_file);

        for object_file in object_files {
            command.arg(object_file);
        }

        let output = command.output().unwrap();

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}

pub(crate) struct GccFlavorCCompiler {
    location: String,
}

impl GccFlavorCCompiler {
    pub(crate) fn new(location: String) -> GccFlavorCCompiler {
        GccFlavorCCompiler { location }
    }
}

impl CCompiler for GccFlavorCCompiler {
    fn compile_source_file(
        &self,
        source_file: &Path,
        output_file: &Path,
        settings: &super::CCompilationSettings<'_>,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location);

        for additional_pre_argument in settings.additional_pre_arguments {
            command.arg(additional_pre_argument);
        }

        command.arg("-c");

        if settings.distribution == Distribution::DynamicLibrary {
            command.arg("-fPIC");
        }

        command.arg("-xc");

        command.arg(format!(
            "-std={}",
            match settings.standard {
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
            match settings.optimization {
                OptimizationLevel::Zero => "0",
                OptimizationLevel::One => "1",
                OptimizationLevel::Two => "2",
                OptimizationLevel::Three => "3",
                OptimizationLevel::Four => "fast",
                OptimizationLevel::Size => "s",
                OptimizationLevel::Debug => "g",
            }
        ));

        if settings.enable_all_warnings {
            command.arg("-Wall");
            command.arg("-Wpedantic");
        }

        if settings.treat_all_warnings_as_errors {
            command.arg("-Werror");
        }

        command.arg(source_file);

        command.arg(format!("-o{}", output_file.display()));

        for include in settings.includes {
            command.arg(format!("-I{}", include));
        }

        for additional_post_argument in settings.additional_post_arguments {
            command.arg(additional_post_argument);
        }

        let output = command.output().unwrap();

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    fn link_object_files(
        &self,
        object_files: &[PathBuf],
        output_file: &Path,
        settings: &super::LinkingSettings<'_>,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location);

        if settings.distribution == Distribution::DynamicLibrary {
            command.arg("-shared");
        }

        for object_file in object_files {
            command.arg(object_file);
        }

        command.arg(format!("-o{}", output_file.display()));

        for include in settings.includes {
            command.arg(format!("-I{}", include));
        }

        for library_search_path in settings.library_search_paths {
            command.arg(format!("-L{}", library_search_path));
        }

        for library in settings.libraries {
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

pub(crate) struct GccFlavorCppCompiler {
    location: String,
}

impl GccFlavorCppCompiler {
    pub(crate) fn new(location: String) -> GccFlavorCppCompiler {
        GccFlavorCppCompiler { location }
    }
}

impl CppCompiler for GccFlavorCppCompiler {
    fn compile_source_file(
        &self,
        source_file: &Path,
        output_file: &Path,
        settings: &super::CppCompilationSettings<'_>,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location);

        for additional_pre_argument in settings.additional_pre_arguments {
            command.arg(additional_pre_argument);
        }

        command.arg("-c");

        if settings.distribution == Distribution::DynamicLibrary {
            command.arg("-fPIC");
        }

        command.arg("-xc++");

        command.arg(format!(
            "-std={}",
            match settings.standard {
                CppStandard::NinetyEight => "c++98",
                CppStandard::Three => "c++3",
                CppStandard::Eleven => "c++11",
                CppStandard::Fourteen => "c++14",
                CppStandard::Seventeen => "c++17",
                CppStandard::Twenty => "c++20",
                CppStandard::TwentyThree => "c++23",
                CppStandard::TwentySix => "c++26",
            }
        ));

        command.arg(format!(
            "-O{}",
            match settings.optimization {
                OptimizationLevel::Zero => "0",
                OptimizationLevel::One => "1",
                OptimizationLevel::Two => "2",
                OptimizationLevel::Three => "3",
                OptimizationLevel::Four => "fast",
                OptimizationLevel::Size => "s",
                OptimizationLevel::Debug => "g",
            }
        ));

        if settings.enable_all_warnings {
            command.arg("-Wall");
            command.arg("-Wpedantic");
        }

        if settings.treat_all_warnings_as_errors {
            command.arg("-Werror");
        }

        command.arg(source_file);

        command.arg(format!("-o{}", output_file.display()));

        for include in settings.includes {
            command.arg(format!("-I{}", include));
        }

        for additional_post_argument in settings.additional_post_arguments {
            command.arg(additional_post_argument);
        }

        let output = command.output().unwrap();

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    fn link_object_files(
        &self,
        object_files: &[PathBuf],
        output_file: &Path,
        settings: &super::LinkingSettings<'_>,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location);

        if settings.distribution == Distribution::DynamicLibrary {
            command.arg("-shared");
        }

        for object_file in object_files {
            command.arg(object_file);
        }

        command.arg(format!("-o{}", output_file.display()));

        for include in settings.includes {
            command.arg(format!("-I{}", include));
        }

        for library_search_path in settings.library_search_paths {
            command.arg(format!("-L{}", library_search_path));
        }

        for library in settings.libraries {
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
