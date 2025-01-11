use crate::tools::{Ar, Gcc, Gpp};
use crate::{
    config::{
        self, BuildConfiguration, CConfiguration, CStandard, CppConfiguration, CppStandard,
        Distribution, GccConfiguration, GppConfiguration, Language, OptimizationLevel,
        ToolchainConfiguration,
    },
    PathExtension,
};
use blake3::Hash;
use glob::glob;
use lazy_static::lazy_static;
use memmap2::MmapOptions;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};
use thiserror::Error;

const BUILD_CONFIGURATION_FILE: &str = "bakery.toml";
const BAKERY_BUILD_DIRECTORY: &str = ".bakery/build";
const BAKERY_CACHE_DIRECTORY: &str = ".bakery/cache";
const BAKERY_HASHES_FILE: &str = ".bakery/cache/hashes.json";

const EXECUTABLE_EXTENSION: &str = if cfg!(target_os = "windows") {
    "exe"
} else if cfg!(target_os = "linux") {
    ""
} else {
    unreachable!()
};

const DYNAMIC_LIBRARY_EXTENSION: &str = if cfg!(target_os = "windows") {
    "dll"
} else if cfg!(target_os = "linux") {
    "so"
} else {
    unreachable!()
};

const STATIC_LIBRARY_EXTENSION: &str = if cfg!(target_os = "windows") {
    "lib"
} else if cfg!(target_os = "linux") {
    "a"
} else {
    unreachable!()
};

const OBJECT_FILE_EXTENSION: &str = "o";

lazy_static! {
    static ref NAME_REGEX: Regex = Regex::new("[a-zA-Z][a-zA-Z0-9]+").unwrap();
}

#[allow(dead_code)]
pub(crate) struct Project {
    pub(crate) base_path: PathBuf,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) language: Language,
    pub(crate) distribution: Distribution,
    pub(crate) sources: Vec<String>,
    pub(crate) includes: Vec<String>,
    pub(crate) dependencies: Vec<Dependency>,
    pub(crate) optimization: OptimizationLevel,
    pub(crate) enable_all_warnings: bool,
    pub(crate) treat_all_warnings_as_errors: bool,
    pub(crate) has_project_configuration_changed: bool,
    pub(crate) hashes: HashMap<String, Hash>,
    pub(crate) c: Option<CConfiguration>,
    pub(crate) cpp: Option<CppConfiguration>,
    pub(crate) gcc: Option<GccConfiguration>,
    pub(crate) gpp: Option<GppConfiguration>,
}

pub(crate) enum Dependency {
    System { name: String },
    Project(Box<Project>),
}

impl Project {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Project, ProjectOpenError> {
        let base_path = path.as_ref();

        let build_configuration_file_path = base_path.join(BUILD_CONFIGURATION_FILE);

        let (build_configuration_content, build_configuration_hash) = {
            let mut build_configuration_file = File::open(&build_configuration_file_path)
                .map_err(ProjectOpenError::InvalidProjectPath)?;
            let build_configuration_file_size = build_configuration_file
                .metadata()
                .map_err(ProjectOpenError::InvalidProjectPath)?
                .len() as usize;
            let mut build_configuration_binary_content = vec![0; build_configuration_file_size];

            build_configuration_file
                .read(&mut build_configuration_binary_content)
                .map_err(ProjectOpenError::InvalidProjectPath)?;

            let build_configuration_content =
                String::from_utf8(build_configuration_binary_content.clone()).map_err(|err| {
                    ProjectOpenError::InvalidProjectPath(io::Error::new(
                        io::ErrorKind::InvalidData,
                        err.to_string(),
                    ))
                })?;
            let build_configuration_hash = blake3::hash(&build_configuration_binary_content);

            (build_configuration_content, build_configuration_hash)
        };

        let build_configuration =
            toml::from_str::<BuildConfiguration>(&build_configuration_content).map_err(|err| {
                ProjectOpenError::InvalidBuildConfiguration(BuildConfigurationError::SyntaxError(
                    err.to_string(),
                ))
            })?;

        if !NAME_REGEX.is_match(&build_configuration.project.name) {
            return Err(ProjectOpenError::InvalidBuildConfiguration(
                BuildConfigurationError::InvalidName,
            ));
        }

        let hashes = fs::read_to_string(Path::new(&base_path).join(BAKERY_HASHES_FILE))
            .map(|hashes_content| {
                serde_json::from_str::<HashMap<String, String>>(&hashes_content)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(key, value)| (key, Hash::from_hex(value).unwrap()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        let has_project_configuration_changed = hashes
            .get(BUILD_CONFIGURATION_FILE)
            .map(|hash| *hash != build_configuration_hash)
            .unwrap_or_default();

        let dependencies = build_configuration
            .project
            .dependencies
            .into_iter()
            .map(|dependency| match dependency {
                config::Dependency::System { name } => Ok(Dependency::System { name }),
                config::Dependency::Local { path } => Project::open(base_path.join(path))
                    .map(|project| Dependency::Project(Box::new(project))),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let sources = build_configuration
            .project
            .sources
            .into_iter()
            .flat_map(|source| {
                glob(&base_path.join(&source).to_string_lossy())
                    .map(|paths| {
                        paths
                            .into_iter()
                            .map(|path| {
                                path.map(|path| {
                                    path.relative_to(base_path)
                                        .unwrap()
                                        .to_string_lossy()
                                        .into_owned()
                                })
                                .map_err(|_| {
                                    ProjectOpenError::InvalidBuildConfiguration(
                                        BuildConfigurationError::IncorrectSource(source.clone()),
                                    )
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .map_err(|err| {
                        ProjectOpenError::InvalidBuildConfiguration(
                            BuildConfigurationError::IncorrectWildcard(String::from(err.msg)),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .map(|source| {
                let path = base_path.join(&source);

                if path.exists() && path.is_file() && path.is_relative() && !path.is_symlink() {
                    Ok(source)
                } else {
                    Err(ProjectOpenError::InvalidBuildConfiguration(
                        BuildConfigurationError::IncorrectSource(source),
                    ))
                }
            })
            .collect::<Result<Vec<String>, _>>()?;

        let includes = build_configuration
            .project
            .includes
            .clone()
            .into_iter()
            .map(|include| base_path.join(include).to_string_lossy().into_owned())
            .chain(
                dependencies
                    .iter()
                    .filter_map(|dependency| match dependency {
                        Dependency::Project(project) => Some(project),
                        _ => None,
                    })
                    .flat_map(|dependency_project| dependency_project.includes.clone().into_iter()),
            )
            .map(|include| {
                let path = Path::new(&include);

                if path.exists() && path.is_dir() && path.is_relative() && !path.is_symlink() {
                    Ok(include)
                } else {
                    Err(ProjectOpenError::InvalidBuildConfiguration(
                        BuildConfigurationError::IncorrectInclude(include),
                    ))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Project {
            base_path: PathBuf::from(path.as_ref()),
            name: build_configuration.project.name,
            description: build_configuration.project.description,
            author: build_configuration.project.author,
            language: build_configuration.project.language,
            distribution: build_configuration.project.distribution,
            sources,
            includes,
            dependencies,
            optimization: build_configuration.project.optimization,
            enable_all_warnings: build_configuration.project.enable_all_warnings,
            treat_all_warnings_as_errors: build_configuration.project.treat_all_warnings_as_errors,
            has_project_configuration_changed,
            hashes,
            c: build_configuration.c,
            cpp: build_configuration.cpp,
            gcc: build_configuration.gcc,
            gpp: build_configuration.gpp,
        })
    }

    pub(crate) fn run(
        &self,
        toolchain_configuration: &ToolchainConfiguration,
    ) -> Result<(), ProjectRunError> {
        if self.distribution != Distribution::Executable {
            return Err(ProjectRunError::CannotRunNonExecutable);
        }

        self.build(toolchain_configuration)
            .map_err(ProjectRunError::FailedToBuildProject)?;

        let absolute_executable_path = self
            .base_path
            .join(BAKERY_BUILD_DIRECTORY)
            .join(&self.name)
            .with_extension(EXECUTABLE_EXTENSION);

        let mut command = Command::new(&absolute_executable_path);

        println!("Running {}", self.name);

        command
            .status()
            .map_err(|_| ProjectRunError::BuiltExecutableIsMissing)?;

        Ok(())
    }

    pub(crate) fn build(
        &self,
        toolchain_configuration: &ToolchainConfiguration,
    ) -> Result<(), ProjectBuildError> {
        let gcc = toolchain_configuration
            .gcc_location
            .as_ref()
            .map(|gcc_location| Gcc::new(gcc_location.clone()))
            .ok_or(ProjectBuildError::ToolNotFound(Tool::CCompiler))?;
        let gpp = toolchain_configuration
            .gpp_location
            .as_ref()
            .map(|gpp_location| Gpp::new(gpp_location.clone()))
            .ok_or(ProjectBuildError::ToolNotFound(Tool::CppCompiler))?;
        let ar = toolchain_configuration
            .ar_location
            .as_ref()
            .map(|ar_location| Ar::new(ar_location.clone()))
            .ok_or(ProjectBuildError::ToolNotFound(Tool::Archiver))?;

        self.build_dependencies(toolchain_configuration)?;

        let sources_to_compile = self.get_sources_to_compile();

        if !sources_to_compile.is_empty() {
            println!("Building {}", self.name);

            self.create_directories()
                .map_err(ProjectBuildError::FailedToCreateBakeryDirectories)?;

            self.build_source_code(sources_to_compile, &gcc, &gpp, &ar)?;

            self.copy_artifacts_to_build_directory()
                .map_err(ProjectBuildError::FailedToCopyArtifacts)?;

            println!("Built {}", self.name);
        }

        Ok(())
    }

    fn build_dependencies(
        &self,
        toolchain_configuration: &ToolchainConfiguration,
    ) -> Result<(), ProjectBuildError> {
        let project_dependencies = self
            .dependencies
            .iter()
            .filter_map(|dependency| match dependency {
                Dependency::Project(project) => Some(project),
                _ => None,
            })
            .collect::<Vec<_>>();

        if !project_dependencies.is_empty() {
            println!("Building dependencies");

            for dependency_project in &project_dependencies {
                if dependency_project.distribution == Distribution::Executable {
                    return Err(ProjectBuildError::DependencyMustBeLibrary);
                }

                dependency_project
                    .build(toolchain_configuration)
                    .map_err(|err| ProjectBuildError::FailedToBuildDependency(Box::new(err)))?;
            }

            println!("Built dependencies");
        }

        Ok(())
    }

    fn create_directories(&self) -> Result<(), io::Error> {
        fs::create_dir_all(self.base_path.join(BAKERY_BUILD_DIRECTORY))?;
        fs::create_dir_all(self.base_path.join(BAKERY_CACHE_DIRECTORY))?;

        Ok(())
    }

    fn build_source_code(
        &self,
        sources_to_compile: Vec<&String>,
        gcc: &Gcc,
        gpp: &Gpp,
        ar: &Ar,
    ) -> Result<(), ProjectBuildError> {
        let mut current_hashes = HashMap::new();

        current_hashes.insert(
            String::from(BUILD_CONFIGURATION_FILE),
            hash_file(
                &File::open(self.base_path.join(BUILD_CONFIGURATION_FILE))
                    .map_err(ProjectBuildError::FailedToOpenFile)?,
            )
            .map_err(ProjectBuildError::FailedToOpenFile)?,
        );

        let (hashes, errors) = sources_to_compile
            .par_iter()
            .fold(
                || (HashMap::new(), Vec::new()),
                |(mut hashes, mut errors), source| {
                    println!("Compiling {}", source);

                    match self.build_source_file(source, gcc, gpp) {
                        Ok(_) => match File::open(self.base_path.join(source)) {
                            Ok(file) => match hash_file(&file) {
                                Ok(hash) => {
                                    hashes.insert((*source).clone(), hash);

                                    println!("Compiled {}", source);
                                }
                                Err(err) => errors.push(SourceFileBuildError::FailedToHash(err)),
                            },
                            Err(err) => errors.push(SourceFileBuildError::FailedToHash(err)),
                        },
                        Err(err) => errors.push(err),
                    }

                    (hashes, errors)
                },
            )
            .reduce(
                || (HashMap::new(), Vec::new()),
                |(mut hashes1, mut errors1), (hashes2, errors2)| {
                    hashes1.extend(hashes2);
                    errors1.extend(errors2);

                    (hashes1, errors1)
                },
            );

        if !errors.is_empty() {
            return Err(ProjectBuildError::CompilationError(errors));
        }

        current_hashes.extend(hashes);

        let hashes_content = serde_json::to_string_pretty(
            &current_hashes
                .into_iter()
                .map(|(key, value)| (key, Hash::to_string(&value)))
                .collect::<HashMap<_, _>>(),
        )
        .unwrap();

        fs::write(self.base_path.join(BAKERY_HASHES_FILE), &hashes_content)
            .map_err(ProjectBuildError::FailedToSaveHashes)?;

        let mut object_files = self
            .sources
            .iter()
            .map(|source| {
                self.base_path
                    .join(BAKERY_BUILD_DIRECTORY)
                    .join(PathBuf::from(source).file_name().unwrap())
                    .with_extension(OBJECT_FILE_EXTENSION)
            })
            .collect::<Vec<_>>();

        let project_dependencies = self
            .dependencies
            .iter()
            .filter_map(|dependency| match dependency {
                Dependency::Project(project) => Some(project),
                _ => None,
            })
            .collect::<Vec<_>>();

        for project_dependency in &project_dependencies {
            if project_dependency.distribution == Distribution::StaticLibrary {
                object_files.push(
                    Path::new(&project_dependency.base_path)
                        .join(BAKERY_BUILD_DIRECTORY)
                        .join(format!(
                            "{}.{}",
                            project_dependency.name, STATIC_LIBRARY_EXTENSION
                        )),
                );
            }
        }

        let absolute_output_file_path = self
            .base_path
            .join(BAKERY_BUILD_DIRECTORY)
            .join(&self.name)
            .with_extension(match self.distribution {
                Distribution::Executable => EXECUTABLE_EXTENSION,
                Distribution::DynamicLibrary => DYNAMIC_LIBRARY_EXTENSION,
                Distribution::StaticLibrary => STATIC_LIBRARY_EXTENSION,
            });

        match self.distribution {
            Distribution::Executable | Distribution::DynamicLibrary => {
                let libraries = self
                    .dependencies
                    .iter()
                    .filter_map(|dependency| match dependency {
                        Dependency::System { name } => Some(name.clone()),
                        Dependency::Project(project) => match project.distribution {
                            Distribution::DynamicLibrary => Some(project.name.clone()),
                            _ => None,
                        },
                    })
                    .collect::<Vec<_>>();

                let library_search_paths = project_dependencies
                    .iter()
                    .map(|project| {
                        Path::new(&project.base_path)
                            .join(BAKERY_BUILD_DIRECTORY)
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect::<Vec<_>>();

                match self.distribution {
                    Distribution::Executable => {
                        println!("Generating executable");

                        match self.language {
                            Language::C => {
                                gcc.link_object_files(
                                    self.distribution.clone(),
                                    &object_files,
                                    &absolute_output_file_path,
                                    &self.includes,
                                    &libraries,
                                    &library_search_paths,
                                )
                                .map_err(ProjectBuildError::LinkageError)?;
                            }
                            Language::Cpp => {
                                gpp.link_object_files(
                                    self.distribution.clone(),
                                    &object_files,
                                    &absolute_output_file_path,
                                    &self.includes,
                                    &libraries,
                                    &library_search_paths,
                                )
                                .map_err(ProjectBuildError::LinkageError)?;
                            }
                        }

                        println!("Generated executable");
                    }
                    Distribution::DynamicLibrary => {
                        println!("Generating dynamic library");

                        match self.language {
                            Language::C => {
                                gcc.link_object_files(
                                    self.distribution.clone(),
                                    &object_files,
                                    &absolute_output_file_path,
                                    &self.includes,
                                    &libraries,
                                    &library_search_paths,
                                )
                                .map_err(ProjectBuildError::LinkageError)?;
                            }
                            Language::Cpp => {
                                gpp.link_object_files(
                                    self.distribution.clone(),
                                    &object_files,
                                    &absolute_output_file_path,
                                    &self.includes,
                                    &libraries,
                                    &library_search_paths,
                                )
                                .map_err(ProjectBuildError::LinkageError)?;
                            }
                        }

                        println!("Generated dynamic library");
                    }
                    _ => unreachable!(),
                }
            }
            Distribution::StaticLibrary => {
                println!("Generating static library");

                ar.archive_object_files(&object_files, &absolute_output_file_path)
                    .map_err(ProjectBuildError::ArchivalError)?;

                println!("Generated static library");
            }
        }

        Ok(())
    }

    fn build_source_file(
        &self,
        source: &str,
        gcc: &Gcc,
        gpp: &Gpp,
    ) -> Result<(), SourceFileBuildError> {
        let absolute_source_file_path = self.base_path.join(source);

        let absolute_output_file_path = self
            .base_path
            .join(BAKERY_BUILD_DIRECTORY)
            .join(PathBuf::from(source).file_name().unwrap())
            .with_extension(OBJECT_FILE_EXTENSION);

        match self.language {
            Language::C => {
                let standard = self
                    .c
                    .as_ref()
                    .and_then(|c| c.standard.as_ref().cloned())
                    .unwrap_or_else(CStandard::latest);

                let (additional_pre_arguments, additional_post_arguments) = self
                    .gcc
                    .as_ref()
                    .map(|gcc| {
                        (
                            gcc.additional_pre_arguments.clone(),
                            gcc.additional_post_arguments.clone(),
                        )
                    })
                    .unwrap_or_default();

                gcc.compile_source_file(
                    self.distribution.clone(),
                    standard,
                    self.optimization.clone(),
                    &absolute_source_file_path,
                    &absolute_output_file_path,
                    &self.includes,
                    self.enable_all_warnings,
                    self.treat_all_warnings_as_errors,
                    &additional_pre_arguments,
                    &additional_post_arguments,
                )
                .map_err(SourceFileBuildError::FailedToCompile)?;
            }
            Language::Cpp => {
                let standard = self
                    .cpp
                    .as_ref()
                    .and_then(|cpp| cpp.standard.as_ref().cloned())
                    .unwrap_or_else(CppStandard::latest);

                let (additional_pre_arguments, additional_post_arguments) = self
                    .gpp
                    .as_ref()
                    .map(|gpp| {
                        (
                            gpp.additional_pre_arguments.clone(),
                            gpp.additional_post_arguments.clone(),
                        )
                    })
                    .unwrap_or_default();

                gpp.compile_source_file(
                    self.distribution.clone(),
                    standard,
                    self.optimization.clone(),
                    &absolute_source_file_path,
                    &absolute_output_file_path,
                    &self.includes,
                    self.enable_all_warnings,
                    self.treat_all_warnings_as_errors,
                    &additional_pre_arguments,
                    &additional_post_arguments,
                )
                .map_err(SourceFileBuildError::FailedToCompile)?;
            }
        }

        Ok(())
    }

    fn get_sources_to_compile(&self) -> Vec<&String> {
        if self.has_project_configuration_changed {
            self.sources.iter().collect::<Vec<_>>()
        } else {
            self.sources
                .par_iter()
                .filter(|source| {
                    self.hashes
                        .get(*source)
                        .map(|hash| {
                            let object_file_exists = fs::metadata(
                                self.base_path
                                    .join(BAKERY_BUILD_DIRECTORY)
                                    .join(PathBuf::from(source).file_name().unwrap())
                                    .with_extension(OBJECT_FILE_EXTENSION),
                            )
                            .map(|_| true)
                            .unwrap_or(false);

                            let source_file_changed = File::open(self.base_path.join(*source))
                                .map(|file| {
                                    let file_content =
                                        unsafe { MmapOptions::new().map(&file).unwrap() };

                                    *hash != blake3::hash(&file_content)
                                })
                                .unwrap_or(false);

                            !object_file_exists | source_file_changed
                        })
                        .unwrap_or(true)
                })
                .collect::<Vec<_>>()
        }
    }

    fn copy_artifacts_to_build_directory(&self) -> Result<(), io::Error> {
        for (index, artifact) in self.get_artifacts().into_iter().enumerate() {
            if index == 0 && self.distribution == Distribution::DynamicLibrary {
                continue;
            }

            fs::copy(
                &artifact,
                self.base_path
                    .join(BAKERY_BUILD_DIRECTORY)
                    .join(artifact.file_name().unwrap()),
            )?;
        }

        Ok(())
    }

    fn get_artifacts(&self) -> Vec<PathBuf> {
        let mut artifacts = Vec::new();

        if self.distribution == Distribution::DynamicLibrary {
            artifacts.push(
                self.base_path
                    .join(BAKERY_BUILD_DIRECTORY)
                    .join(format!("{}.{}", self.name, DYNAMIC_LIBRARY_EXTENSION)),
            );
        }

        for dependency in &self.dependencies {
            if let Dependency::Project(project) = dependency {
                artifacts.append(&mut project.get_artifacts());
            }
        }

        artifacts
    }
}

fn hash_file(file: &File) -> Result<Hash, io::Error> {
    let file_content = unsafe { MmapOptions::new().map(file)? };

    Ok(blake3::hash(&file_content))
}

#[derive(Error, Debug)]
pub(crate) enum ProjectOpenError {
    #[error("specificed path doesn't contain bakery.toml")]
    InvalidProjectPath(io::Error),
    #[error("the project's build configuration is invalid: {0:?}")]
    InvalidBuildConfiguration(BuildConfigurationError),
}

#[derive(Error, Debug)]
pub(crate) enum BuildConfigurationError {
    #[error("found a syntax error: {0}")]
    SyntaxError(String),
    #[error("the project's name contains invalid characters")]
    InvalidName,
    #[error("found an incorrect wildcard: {0}")]
    IncorrectWildcard(String),
    #[error("found an incorrect source: {0}")]
    IncorrectSource(String),
    #[error("found an incorrect include: {0}")]
    IncorrectInclude(String),
}

#[derive(Error, Debug)]
pub(crate) enum ProjectBuildError {
    #[error("failed to locate a {0:?}")]
    ToolNotFound(Tool),
    #[error("failed to create bakery directories: {0:?}")]
    FailedToCreateBakeryDirectories(io::Error),
    #[error("failed to copy artifacts: {0:?}")]
    FailedToCopyArtifacts(io::Error),
    #[error("failed to open a file: {0:?}")]
    FailedToOpenFile(io::Error),
    #[error("failed to save hashes: {0:?}")]
    FailedToSaveHashes(io::Error),
    #[error("dependency must be a static or dynamic library")]
    DependencyMustBeLibrary,
    #[error("failed to build a dependency: {0:?}")]
    FailedToBuildDependency(Box<ProjectBuildError>),
    #[error("failed to compile a project: {0:?}")]
    CompilationError(Vec<SourceFileBuildError>),
    #[error("failed to link a project: {0}")]
    LinkageError(String),
    #[error("failed to archive a project: {0}")]
    ArchivalError(String),
}

#[derive(Debug)]
pub(crate) enum Tool {
    CCompiler,
    CppCompiler,
    Archiver,
}

#[derive(Error, Debug)]
pub(crate) enum SourceFileBuildError {
    #[error("failed to compile a source file: {0}")]
    FailedToCompile(String),
    #[error("failed to hash a source file: {0:?}")]
    FailedToHash(io::Error),
}

#[derive(Error, Debug)]
pub(crate) enum ProjectRunError {
    #[error("failed to build a project: {0:?}")]
    FailedToBuildProject(ProjectBuildError),
    #[error("can't run a project whose distribution isn't set to 'executable'")]
    CannotRunNonExecutable,
    #[error("the project's executable is missing")]
    BuiltExecutableIsMissing,
}
