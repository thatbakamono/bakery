mod gcc;

use std::path::{Path, PathBuf};

pub(crate) use gcc::*;

use crate::config::{CStandard, CppStandard, Distribution, OptimizationLevel};

pub trait Archiver {
    fn archive_object_files(
        &self,
        object_files: &[PathBuf],
        output_file: &Path,
    ) -> Result<(), String>;
}

pub struct CCompilationSettings<'a> {
    pub distribution: Distribution,
    pub standard: CStandard,
    pub optimization: OptimizationLevel,
    pub includes: &'a [String],
    pub enable_all_warnings: bool,
    pub treat_all_warnings_as_errors: bool,
    pub additional_pre_arguments: &'a [String],
    pub additional_post_arguments: &'a [String],
}

pub trait CCompiler: Send + Sync {
    fn compile_source_file(
        &self,
        source_file: &Path,
        output_file: &Path,
        settings: &CCompilationSettings<'_>,
    ) -> Result<(), String>;

    fn link_object_files(
        &self,
        object_files: &[PathBuf],
        output_file: &Path,
        settings: &LinkingSettings<'_>,
    ) -> Result<(), String>;
}

pub struct CppCompilationSettings<'a> {
    pub distribution: Distribution,
    pub standard: CppStandard,
    pub optimization: OptimizationLevel,
    pub includes: &'a [String],
    pub enable_all_warnings: bool,
    pub treat_all_warnings_as_errors: bool,
    pub additional_pre_arguments: &'a [String],
    pub additional_post_arguments: &'a [String],
}

pub trait CppCompiler: Send + Sync {
    fn compile_source_file(
        &self,
        source_file: &Path,
        output_file: &Path,
        settings: &CppCompilationSettings<'_>,
    ) -> Result<(), String>;

    fn link_object_files(
        &self,
        object_files: &[PathBuf],
        output_file: &Path,
        settings: &LinkingSettings<'_>,
    ) -> Result<(), String>;
}

pub struct LinkingSettings<'a> {
    pub distribution: Distribution,
    pub includes: &'a [String],
    pub libraries: &'a [String],
    pub library_search_paths: &'a [String],
}
