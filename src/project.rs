use crate::{
    config::{
        self, BuildConfiguration, CConfiguration, CppConfiguration, Distribution, GccConfiguration,
        GppConfiguration, Language, OptimizationLevel,
    },
    PathExtension, BAKERY_HASHES_FILE, BUILD_CONFIGURATION_FILE,
};
use blake3::Hash;
use glob::glob;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;

pub(crate) const NAME_PATTERN: &str = "[a-zA-Z][a-zA-Z0-9]+";

lazy_static! {
    static ref NAME_REGEX: Regex = Regex::new(NAME_PATTERN).unwrap();
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

        let hashes = Self::read_hashes(base_path);

        let has_project_configuration_changed = hashes
            .get(BUILD_CONFIGURATION_FILE)
            .map(|hash| *hash != build_configuration_hash)
            .unwrap_or_default();

        let dependencies = Self::resolve_dependencies(base_path, &build_configuration)?;
        let sources = Self::resolve_sources(base_path, &build_configuration)?;
        let includes = Self::resolve_includes(base_path, &build_configuration, &dependencies)?;

        for dependency in &dependencies {
            if let Dependency::Project(project) = dependency {
                if project.distribution == Distribution::Executable {
                    return Err(ProjectOpenError::InvalidBuildConfiguration(
                        BuildConfigurationError::DependencyIsNotALibrary(project.name.clone()),
                    ));
                }
            }
        }

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

    fn read_hashes(base_path: &Path) -> HashMap<String, Hash> {
        fs::read_to_string(base_path.join(BAKERY_HASHES_FILE))
            .map(|hashes_content| {
                serde_json::from_str::<HashMap<String, String>>(&hashes_content)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(key, value)| (key, Hash::from_hex(value).unwrap()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default()
    }

    fn resolve_dependencies(
        base_path: &Path,
        build_configuration: &BuildConfiguration,
    ) -> Result<Vec<Dependency>, ProjectOpenError> {
        build_configuration
            .project
            .dependencies
            .iter()
            .map(|dependency| match dependency {
                config::Dependency::System { name } => {
                    Ok(Dependency::System { name: name.clone() })
                }
                config::Dependency::Local { path } => Project::open(base_path.join(path))
                    .map(|project| Dependency::Project(Box::new(project))),
            })
            .collect::<Result<Vec<_>, _>>()
    }

    fn resolve_sources(
        base_path: &Path,
        build_configuration: &BuildConfiguration,
    ) -> Result<Vec<String>, ProjectOpenError> {
        build_configuration
            .project
            .sources
            .iter()
            .flat_map(|source| {
                glob(&base_path.join(source).to_string_lossy())
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
            .collect::<Result<Vec<String>, _>>()
    }

    fn resolve_includes(
        base_path: &Path,
        build_configuration: &BuildConfiguration,
        dependencies: &[Dependency],
    ) -> Result<Vec<String>, ProjectOpenError> {
        build_configuration
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
            .collect::<Result<Vec<_>, _>>()
    }
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
    #[error("dependency {0} is not a library")]
    DependencyIsNotALibrary(String),
}

#[derive(Error, Debug)]
pub(crate) enum ProjectBuildError {
    #[error("failed to create bakery directories: {0:?}")]
    FailedToCreateBakeryDirectories(io::Error),
    #[error("failed to open a file: {0:?}")]
    FailedToOpenFile(io::Error),
    #[error("failed to save hashes: {0:?}")]
    FailedToSaveHashes(io::Error),
    #[error("failed to compile a project: {0:?}")]
    CompilationError(Vec<SourceFileBuildError>),
    #[error("failed to link a project: {0}")]
    LinkageError(String),
    #[error("failed to archive a project: {0}")]
    ArchivalError(String),
}

#[derive(Error, Debug)]
pub(crate) enum SourceFileBuildError {
    #[error("failed to compile a source file: {0}")]
    FailedToCompile(String),
    #[error("failed to hash a source file: {0:?}")]
    FailedToHash(io::Error),
}
