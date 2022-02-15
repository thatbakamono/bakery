use crate::config::{
    BuildConfiguration, CPPStandard, CStandard, Dependency, Distribution, EzConfiguration, Language,
};
use crate::tools::{Ar, GCC, GPP};
use glob::glob;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};
use thiserror::Error;

const BUILD_CONFIGURATION_FILE: &'static str = "ez.toml";
const EZ_BUILD_DIRECTORY: &'static str = ".ez/build";

const EXECUTABLE_EXTENSION: &'static str = if cfg!(target_os = "windows") {
    "exe"
} else if cfg!(target_os = "linux") {
    ""
} else {
    unreachable!()
};

const DYNAMIC_LIBRARY_EXTENSION: &'static str = if cfg!(target_os = "windows") {
    "dll"
} else if cfg!(target_os = "linux") {
    "so"
} else {
    unreachable!()
};

const STATIC_LIBRARY_EXTENSION: &'static str = if cfg!(target_os = "windows") {
    "lib"
} else if cfg!(target_os = "linux") {
    "a"
} else {
    unreachable!()
};

const OBJECT_FILE_EXTENSION: &'static str = "o";

pub(crate) struct Project {
    configuration: BuildConfiguration,
    base_path: PathBuf,
}

impl Project {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Project, ProjectOpenError> {
        let build_configuration_path = path.as_ref().join(BUILD_CONFIGURATION_FILE);

        let build_configuration_content = fs::read_to_string(build_configuration_path)
            .map_err(|_| ProjectOpenError::InvalidProjectPath)?;
        let build_configuration =
            toml::from_str::<BuildConfiguration>(&build_configuration_content)
                .map_err(|_| ProjectOpenError::InvalidBuildConfiguration)?;

        Ok(Project {
            configuration: build_configuration,
            base_path: PathBuf::from(path.as_ref()),
        })
    }

    pub(crate) fn build(
        &self,
        ez_configuration: &EzConfiguration,
    ) -> Result<(), ProjectBuildError> {
        let gcc = GCC::locate(ez_configuration).ok_or(ProjectBuildError::CompilerNotFound)?;
        let gpp = GPP::locate(ez_configuration).ok_or(ProjectBuildError::CompilerNotFound)?;
        let ar = Ar::locate(ez_configuration).ok_or(ProjectBuildError::ArchiverNotFound)?;

        let project_dependencies = self
            .configuration
            .project
            .dependencies
            .iter()
            .filter_map(|dependency| match dependency {
                Dependency::Local { path } => {
                    let absolute_project_path = Path::new(&self.base_path).join(path);

                    Some(Project::open(absolute_project_path).unwrap())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let system_dependencies = self
            .configuration
            .project
            .dependencies
            .iter()
            .filter_map(|dependency| match dependency {
                Dependency::System { name } => Some(name.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        if !project_dependencies.is_empty() {
            println!("Building dependencies");

            for dependency_project in &project_dependencies {
                if dependency_project.configuration.project.distribution == Distribution::Executable
                {
                    return Err(ProjectBuildError::DependencyMustBeLibrary);
                }

                dependency_project
                    .build(ez_configuration)
                    .map_err(|err| ProjectBuildError::FailedToBuildDependency(Box::new(err)))?;
            }

            println!("Built dependencies");
        }

        let sources = self
            .configuration
            .project
            .sources
            .clone()
            .into_iter()
            .map(|source| {
                glob(&source)
                    .map(|paths| {
                        paths
                            .into_iter()
                            .map(|path| {
                                path.map(|path| path.to_string_lossy().into_owned())
                                    .map_err(
                                        |_| ProjectBuildError::IncorrectSource(source.clone()),
                                    )
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .map_err(|err| ProjectBuildError::IncorrectWildcard(String::from(err.msg)))
            })
            .flatten()
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .map(|source| {
                let path = PathBuf::from(&source);
    
                if path.exists() && path.is_file() && !path.is_symlink() {
                    Ok(source)
                } else {
                    Err(ProjectBuildError::IncorrectSource(source))
                }
            })
            .collect::<Result<Vec<String>, _>>()?;

        let includes = self
            .configuration
            .project
            .includes
            .clone()
            .into_iter()
            .map(|include| {
                Path::new(&self.base_path)
                    .join(include)
                    .to_string_lossy()
                    .into_owned()
            })
            .chain(project_dependencies.iter().flat_map(|dependency_project| {
                dependency_project
                    .configuration
                    .project
                    .includes
                    .clone()
                    .into_iter()
                    .map(|include| {
                        Path::new(&dependency_project.base_path)
                            .join(include)
                            .to_string_lossy()
                            .into_owned()
                    })
            }))
            .collect::<Vec<_>>();

        if !sources.is_empty() {
            println!("Building {}", self.configuration.project.name);

            fs::create_dir_all(Path::new(&self.base_path).join(EZ_BUILD_DIRECTORY))
                .map_err(|err| ProjectBuildError::IOError(err))?;

            let mut object_files = vec![];

            for source in &sources {
                println!("Compiling {}", source);

                let absolute_source_file_path = Path::new(&self.base_path).join(source);

                let absolute_output_file_path = Path::new(&self.base_path)
                    .join(EZ_BUILD_DIRECTORY)
                    .join(PathBuf::from(source).file_name().unwrap())
                    .with_extension(OBJECT_FILE_EXTENSION);

                object_files.push(absolute_output_file_path.clone());

                match self.configuration.project.language {
                    Language::C => {
                        let standard = self
                            .configuration
                            .c
                            .as_ref()
                            .and_then(|c| c.standard.as_ref().cloned())
                            .unwrap_or_else(CStandard::latest);

                        let (additional_pre_arguments, additional_post_arguments) = self
                            .configuration
                            .gcc
                            .as_ref()
                            .map(|gcc| {
                                (
                                    gcc.additional_pre_arguments.clone(),
                                    gcc.additional_post_arguments.clone(),
                                )
                            })
                            .unwrap_or_default();

                        if let Err(error_message) = gcc.compile_source_file(
                            self.configuration.project.distribution.clone(),
                            standard,
                            self.configuration.project.optimization.clone(),
                            &absolute_source_file_path,
                            &absolute_output_file_path,
                            &includes,
                            self.configuration.project.enable_all_warnings,
                            self.configuration.project.treat_all_warnings_as_errors,
                            &additional_pre_arguments,
                            &additional_post_arguments,
                        ) {
                            return Err(ProjectBuildError::CompilerError(error_message));
                        }
                    }
                    Language::CPP => {
                        let standard = self
                            .configuration
                            .cpp
                            .as_ref()
                            .and_then(|cpp| cpp.standard.as_ref().cloned())
                            .unwrap_or_else(CPPStandard::latest);

                        let (additional_pre_arguments, additional_post_arguments) = self
                            .configuration
                            .gpp
                            .as_ref()
                            .map(|gpp| {
                                (
                                    gpp.additional_pre_arguments.clone(),
                                    gpp.additional_post_arguments.clone(),
                                )
                            })
                            .unwrap_or_default();

                        if let Err(error_message) = gpp.compile_source_file(
                            self.configuration.project.distribution.clone(),
                            standard,
                            self.configuration.project.optimization.clone(),
                            &absolute_source_file_path,
                            &absolute_output_file_path,
                            &includes,
                            self.configuration.project.enable_all_warnings,
                            self.configuration.project.treat_all_warnings_as_errors,
                            &additional_pre_arguments,
                            &additional_post_arguments,
                        ) {
                            return Err(ProjectBuildError::CompilerError(error_message));
                        }
                    }
                }

                println!("Compiled {}", source);
            }

            for project_dependency in &project_dependencies {
                if project_dependency.configuration.project.distribution
                    == Distribution::StaticLibrary
                {
                    object_files.push(
                        Path::new(&project_dependency.base_path)
                            .join(EZ_BUILD_DIRECTORY)
                            .join(format!(
                                "{}.{}",
                                project_dependency.configuration.project.name,
                                STATIC_LIBRARY_EXTENSION
                            )),
                    );
                }
            }

            match self.configuration.project.distribution {
                Distribution::Executable => {
                    println!("Generating executable");

                    let absolute_output_file_path = Path::new(&self.base_path)
                        .join(EZ_BUILD_DIRECTORY)
                        .join(&self.configuration.project.name)
                        .with_extension(EXECUTABLE_EXTENSION);

                    let libraries = system_dependencies
                        .into_iter()
                        .chain(
                            project_dependencies
                                .iter()
                                .filter(|project| {
                                    project.configuration.project.distribution
                                        == Distribution::DynamicLibrary
                                })
                                .map(|project| project.configuration.project.name.clone()),
                        )
                        .collect::<Vec<_>>();

                    let library_search_paths = project_dependencies
                        .iter()
                        .map(|project| {
                            Path::new(&project.base_path)
                                .join(EZ_BUILD_DIRECTORY)
                                .to_string_lossy()
                                .into_owned()
                        })
                        .collect::<Vec<_>>();

                    match self.configuration.project.language {
                        Language::C => {
                            if let Err(error_message) = gcc.link_object_files(
                                self.configuration.project.distribution.clone(),
                                &object_files,
                                &absolute_output_file_path,
                                &includes,
                                &libraries,
                                &library_search_paths,
                            ) {
                                return Err(ProjectBuildError::LinkerError(error_message));
                            }
                        }
                        Language::CPP => {
                            if let Err(error_message) = gpp.link_object_files(
                                self.configuration.project.distribution.clone(),
                                &object_files,
                                &absolute_output_file_path,
                                &includes,
                                &libraries,
                                &library_search_paths,
                            ) {
                                return Err(ProjectBuildError::LinkerError(error_message));
                            }
                        }
                    }

                    println!("Generated executable");
                }
                Distribution::StaticLibrary => {
                    println!("Generating static library");

                    let absolute_output_file_path = Path::new(&self.base_path)
                        .join(EZ_BUILD_DIRECTORY)
                        .join(&self.configuration.project.name)
                        .with_extension(STATIC_LIBRARY_EXTENSION);

                    if let Err(error_message) =
                        ar.archive_object_files(&object_files, &absolute_output_file_path)
                    {
                        return Err(ProjectBuildError::ArchiverError(error_message));
                    }

                    println!("Generated static library");
                }
                Distribution::DynamicLibrary => {
                    println!("Generating dynamic library");

                    let absolute_output_file_path = Path::new(&self.base_path)
                        .join(EZ_BUILD_DIRECTORY)
                        .join(&self.configuration.project.name)
                        .with_extension(DYNAMIC_LIBRARY_EXTENSION);

                    let libraries = system_dependencies
                        .into_iter()
                        .chain(
                            project_dependencies
                                .iter()
                                .filter(|project| {
                                    project.configuration.project.distribution
                                        == Distribution::DynamicLibrary
                                })
                                .map(|project| project.configuration.project.name.clone()),
                        )
                        .collect::<Vec<_>>();

                    let library_search_paths = project_dependencies
                        .iter()
                        .map(|project| {
                            Path::new(&project.base_path)
                                .join(EZ_BUILD_DIRECTORY)
                                .to_string_lossy()
                                .into_owned()
                        })
                        .collect::<Vec<_>>();

                    match self.configuration.project.language {
                        Language::C => {
                            if let Err(error_message) = gcc.link_object_files(
                                self.configuration.project.distribution.clone(),
                                &object_files,
                                &absolute_output_file_path,
                                &includes,
                                &libraries,
                                &library_search_paths,
                            ) {
                                return Err(ProjectBuildError::LinkerError(error_message));
                            }
                        }
                        Language::CPP => {
                            if let Err(error_message) = gpp.link_object_files(
                                self.configuration.project.distribution.clone(),
                                &object_files,
                                &absolute_output_file_path,
                                &includes,
                                &libraries,
                                &library_search_paths,
                            ) {
                                return Err(ProjectBuildError::LinkerError(error_message));
                            }
                        }
                    }

                    println!("Generated dynamic library");
                }
            }

            for (index, artifact) in self.get_artifacts().into_iter().enumerate() {
                if index == 0
                    && self.configuration.project.distribution == Distribution::DynamicLibrary
                {
                    continue;
                }

                fs::copy(
                    &artifact,
                    self.base_path
                        .join(EZ_BUILD_DIRECTORY)
                        .join(artifact.file_name().unwrap()),
                )
                .map_err(|err| ProjectBuildError::IOError(err))?;
            }

            println!("Built {}", self.configuration.project.name);
        }

        Ok(())
    }

    pub(crate) fn run(&self, ez_configuration: &EzConfiguration) -> Result<(), ProjectRunError> {
        if self.configuration.project.distribution != Distribution::Executable {
            return Err(ProjectRunError::CannotRunNonExecutable);
        }

        self.build(ez_configuration)
            .map_err(|err| ProjectRunError::FailedToBuildProject(err))?;

        let absolute_executable_path = Path::new(&self.base_path)
            .join(EZ_BUILD_DIRECTORY)
            .join(&self.configuration.project.name)
            .with_extension(EXECUTABLE_EXTENSION);

        let mut command = Command::new(&absolute_executable_path);

        println!("Running {}", self.configuration.project.name);

        command.status().unwrap();

        Ok(())
    }

    fn get_artifacts(&self) -> Vec<PathBuf> {
        let mut artifacts = Vec::new();

        if self.configuration.project.distribution == Distribution::DynamicLibrary {
            artifacts.push(self.base_path.join(EZ_BUILD_DIRECTORY).join(format!(
                "{}.{}",
                self.configuration.project.name, DYNAMIC_LIBRARY_EXTENSION
            )));
        }

        for dependency in &self.configuration.project.dependencies {
            if let Dependency::Local { path } = dependency {
                let project = Project::open(path).unwrap();

                artifacts.append(&mut project.get_artifacts());
            }
        }

        artifacts
    }
}

#[derive(Error, Debug)]
pub(crate) enum ProjectOpenError {
    #[error("specificed path doesn't contain ez.toml")]
    InvalidProjectPath,
    #[error("ez.toml is invalid")]
    InvalidBuildConfiguration,
}

#[derive(Error, Debug)]
pub(crate) enum ProjectBuildError {
    #[error("failed to locate compiler")]
    CompilerNotFound,
    #[error("failed to locate archiver")]
    ArchiverNotFound,
    #[error("io error occurred: {0:?}")]
    IOError(io::Error),
    #[error("dependency must be static or dynamic library")]
    DependencyMustBeLibrary,
    #[error("failed to build dependency: {0:?}")]
    FailedToBuildDependency(Box<ProjectBuildError>),
    #[error("failed to compile project: {0}")]
    CompilerError(String),
    #[error("failed to link project: {0}")]
    LinkerError(String),
    #[error("failed to archive project: {0}")]
    ArchiverError(String),
    #[error("found incorrect wildcard: {0}")]
    IncorrectWildcard(String),
    #[error("found incorrect source: {0}")]
    IncorrectSource(String),
}

#[derive(Error, Debug)]
pub(crate) enum ProjectRunError {
    #[error("failed to build project: {0:?}")]
    FailedToBuildProject(ProjectBuildError),
    #[error("can't run project that isn't executable")]
    CannotRunNonExecutable,
}
