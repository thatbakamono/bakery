use super::{Project, Task, TaskContext, ToolchainConfiguration};
use crate::{
    config::{CStandard, CppStandard, Distribution, Language},
    tools::{
        Archiver, CCompilationSettings, CCompiler, CppCompilationSettings, CppCompiler,
        GccFlavorArchiver, GccFlavorCCompiler, GccFlavorCppCompiler, LinkingSettings,
    },
    Dependency, ProjectBuildError, SourceFileBuildError, BAKERY_BUILD_DIRECTORY,
    BAKERY_CACHE_DIRECTORY, BAKERY_HASHES_FILE, BUILD_CONFIGURATION_FILE,
};
use blake3::Hash;
use memmap2::MmapOptions;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    collections::HashMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

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

pub struct Build {}

impl Build {
    pub fn new() -> Self {
        Self {}
    }

    fn create_c_compiler(
        &self,
        toolchain_configuration: &ToolchainConfiguration,
    ) -> Option<Box<dyn CCompiler>> {
        toolchain_configuration
            .gcc_location
            .as_ref()
            .map(|gcc_location| {
                let c_compiler: Box<dyn CCompiler> =
                    Box::new(GccFlavorCCompiler::new(gcc_location.clone()));

                c_compiler
            })
    }

    fn create_cpp_compiler(
        &self,
        toolchain_configuration: &ToolchainConfiguration,
    ) -> Option<Box<dyn CppCompiler>> {
        toolchain_configuration
            .gpp_location
            .as_ref()
            .map(|gpp_location| {
                let cpp_compiler: Box<dyn CppCompiler> =
                    Box::new(GccFlavorCppCompiler::new(gpp_location.clone()));

                cpp_compiler
            })
    }

    fn create_archiver(
        &self,
        toolchain_configuration: &ToolchainConfiguration,
    ) -> Option<Box<dyn Archiver>> {
        toolchain_configuration
            .ar_location
            .as_ref()
            .map(|ar_location| {
                let archiver: Box<dyn Archiver> =
                    Box::new(GccFlavorArchiver::new(ar_location.clone()));

                archiver
            })
    }

    fn get_c_standard(&self, project: &Project) -> CStandard {
        project
            .c
            .as_ref()
            .and_then(|c| c.standard.as_ref().cloned())
            .unwrap_or_else(CStandard::latest)
    }

    fn get_cpp_standard(&self, project: &Project) -> CppStandard {
        project
            .cpp
            .as_ref()
            .and_then(|cpp| cpp.standard.as_ref().cloned())
            .unwrap_or_else(CppStandard::latest)
    }

    fn collect_sources_to_compile(&self, project: &Project) -> Vec<String> {
        if project.has_project_configuration_changed {
            project.sources.to_vec()
        } else {
            project
                .sources
                .par_iter()
                .cloned()
                .filter(|source| {
                    project
                        .hashes
                        .get(source)
                        .map(|hash| {
                            let object_file_exists = fs::metadata(
                                project
                                    .base_path
                                    .join(BAKERY_BUILD_DIRECTORY)
                                    .join(PathBuf::from(source).file_name().unwrap())
                                    .with_extension(OBJECT_FILE_EXTENSION),
                            )
                            .map(|_| true)
                            .unwrap_or(false);

                            let source_file_changed = File::open(project.base_path.join(source))
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

    fn collect_project_dependencies<'a>(&self, project: &'a Project) -> Vec<&'a Project> {
        project
            .dependencies
            .iter()
            .filter_map(|dependency| match dependency {
                Dependency::Project(project) => Some(project.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>()
    }

    fn collect_libraries(&self, project: &Project) -> Vec<String> {
        project
            .dependencies
            .iter()
            .filter_map(|dependency| match dependency {
                Dependency::System { name } => Some(name.clone()),
                Dependency::Project(project) => match project.distribution {
                    Distribution::DynamicLibrary => Some(project.name.clone()),
                    _ => None,
                },
            })
            .collect::<Vec<_>>()
    }

    fn collect_library_search_paths(&self, project_dependencies: &[&Project]) -> Vec<String> {
        project_dependencies
            .iter()
            .map(|project| {
                Path::new(&project.base_path)
                    .join(BAKERY_BUILD_DIRECTORY)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>()
    }

    fn collect_object_files(
        &self,
        project: &Project,
        project_dependencies: &[&Project],
    ) -> Vec<PathBuf> {
        let mut object_files = project
            .sources
            .iter()
            .map(|source| {
                project
                    .base_path
                    .join(BAKERY_BUILD_DIRECTORY)
                    .join(PathBuf::from(source).file_name().unwrap())
                    .with_extension(OBJECT_FILE_EXTENSION)
            })
            .collect::<Vec<_>>();

        for project_dependency in project_dependencies {
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

        object_files
    }

    fn collect_artifacts(&self, project: &Project) -> Vec<PathBuf> {
        let mut artifacts = Vec::new();

        if project.distribution == Distribution::DynamicLibrary {
            artifacts.push(
                project
                    .base_path
                    .join(BAKERY_BUILD_DIRECTORY)
                    .join(format!("{}.{}", project.name, DYNAMIC_LIBRARY_EXTENSION)),
            );
        }

        for dependency in &project.dependencies {
            if let Dependency::Project(subproject) = dependency {
                artifacts.append(&mut self.collect_artifacts(subproject));
            }
        }

        artifacts
    }

    fn serialize_hashes(&self, hashes: HashMap<String, Hash>) -> String {
        serde_json::to_string_pretty(
            &hashes
                .into_iter()
                .map(|(key, value)| (key, Hash::to_string(&value)))
                .collect::<HashMap<_, _>>(),
        )
        .unwrap()
    }

    fn create_directories(&self, project: &Project) -> Result<(), io::Error> {
        fs::create_dir_all(project.base_path.join(BAKERY_BUILD_DIRECTORY))?;
        fs::create_dir_all(project.base_path.join(BAKERY_CACHE_DIRECTORY))?;

        Ok(())
    }

    fn build_dependencies(
        &self,
        project: &Project,
        c_compiler: &dyn CCompiler,
        cpp_compiler: &dyn CppCompiler,
        archiver: &dyn Archiver,
    ) -> Result<(), ProjectBuildError> {
        for dependency in &project.dependencies {
            if let Dependency::Project(subproject) = dependency {
                self.build_dependencies(subproject, c_compiler, cpp_compiler, archiver)?;

                let sources = self.collect_sources_to_compile(subproject);

                self.build(subproject, sources, c_compiler, cpp_compiler, archiver)?;
            }
        }

        Ok(())
    }

    fn build(
        &self,
        project: &Project,
        sources: Vec<String>,
        c_compiler: &dyn CCompiler,
        cpp_compiler: &dyn CppCompiler,
        archiver: &dyn Archiver,
    ) -> Result<(), ProjectBuildError> {
        println!("Building {}", project.name);

        if let Err(err) = self.create_directories(project) {
            return Err(ProjectBuildError::FailedToCreateBakeryDirectories(err));
        }

        static EMPTY: Vec<String> = vec![];

        let mut current_hashes = HashMap::new();

        current_hashes.insert(
            String::from(BUILD_CONFIGURATION_FILE),
            hash_file(
                &File::open(project.base_path.join(BUILD_CONFIGURATION_FILE))
                    .map_err(ProjectBuildError::FailedToOpenFile)?,
            )
            .map_err(ProjectBuildError::FailedToOpenFile)?,
        );

        let c_standard = self.get_c_standard(project);
        let (c_additional_pre_arguments, c_additional_post_arguments) = project
            .gcc
            .as_ref()
            .map(|gcc| {
                (
                    &gcc.additional_pre_arguments,
                    &gcc.additional_post_arguments,
                )
            })
            .unwrap_or_else(|| (&EMPTY, &EMPTY));
        let c_compilation_settings = CCompilationSettings {
            distribution: project.distribution.clone(),
            standard: c_standard,
            optimization: project.optimization.clone(),
            includes: &project.includes,
            enable_all_warnings: project.enable_all_warnings,
            treat_all_warnings_as_errors: project.treat_all_warnings_as_errors,
            additional_pre_arguments: c_additional_pre_arguments,
            additional_post_arguments: c_additional_post_arguments,
        };

        let cpp_standard = self.get_cpp_standard(project);
        let (cpp_additional_pre_arguments, cpp_additional_post_arguments) = project
            .gpp
            .as_ref()
            .map(|gpp| {
                (
                    &gpp.additional_pre_arguments,
                    &gpp.additional_post_arguments,
                )
            })
            .unwrap_or_else(|| (&EMPTY, &EMPTY));
        let cpp_compilation_settings = CppCompilationSettings {
            distribution: project.distribution.clone(),
            standard: cpp_standard,
            optimization: project.optimization.clone(),
            includes: &project.includes,
            enable_all_warnings: project.enable_all_warnings,
            treat_all_warnings_as_errors: project.treat_all_warnings_as_errors,
            additional_pre_arguments: cpp_additional_pre_arguments,
            additional_post_arguments: cpp_additional_post_arguments,
        };

        let (hashes, errors) = sources
            .par_iter()
            .fold(
                || (HashMap::new(), Vec::new()),
                |(mut hashes, mut errors), source| {
                    println!("Compiling {}", source);

                    match self.compile_source_file(
                        project,
                        source,
                        c_compiler,
                        &c_compilation_settings,
                        cpp_compiler,
                        &cpp_compilation_settings,
                    ) {
                        Ok(_) => match File::open(project.base_path.join(source)) {
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

        let hashes_content = self.serialize_hashes(current_hashes);

        fs::write(project.base_path.join(BAKERY_HASHES_FILE), &hashes_content)
            .map_err(ProjectBuildError::FailedToSaveHashes)?;

        let project_dependencies = self.collect_project_dependencies(project);
        let object_files = self.collect_object_files(project, &project_dependencies);

        let absolute_output_file_path = project
            .base_path
            .join(BAKERY_BUILD_DIRECTORY)
            .join(&project.name)
            .with_extension(match project.distribution {
                Distribution::Executable => EXECUTABLE_EXTENSION,
                Distribution::DynamicLibrary => DYNAMIC_LIBRARY_EXTENSION,
                Distribution::StaticLibrary => STATIC_LIBRARY_EXTENSION,
            });

        match project.distribution {
            Distribution::Executable | Distribution::DynamicLibrary => {
                let libraries = self.collect_libraries(project);
                let library_search_paths = self.collect_library_search_paths(&project_dependencies);
                let linking_setttings = LinkingSettings {
                    distribution: project.distribution.clone(),
                    includes: &project.includes,
                    libraries: &libraries,
                    library_search_paths: &library_search_paths,
                };

                match project.distribution {
                    Distribution::Executable => {
                        println!("Generating executable");

                        match project.language {
                            Language::C => {
                                c_compiler
                                    .link_object_files(
                                        &object_files,
                                        &absolute_output_file_path,
                                        &linking_setttings,
                                    )
                                    .map_err(ProjectBuildError::LinkageError)?;
                            }
                            Language::Cpp => {
                                cpp_compiler
                                    .link_object_files(
                                        &object_files,
                                        &absolute_output_file_path,
                                        &linking_setttings,
                                    )
                                    .map_err(ProjectBuildError::LinkageError)?;
                            }
                        }

                        println!("Generated executable");
                    }
                    Distribution::DynamicLibrary => {
                        println!("Generating dynamic library");

                        match project.language {
                            Language::C => {
                                c_compiler
                                    .link_object_files(
                                        &object_files,
                                        &absolute_output_file_path,
                                        &linking_setttings,
                                    )
                                    .map_err(ProjectBuildError::LinkageError)?;
                            }
                            Language::Cpp => {
                                cpp_compiler
                                    .link_object_files(
                                        &object_files,
                                        &absolute_output_file_path,
                                        &linking_setttings,
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

                archiver
                    .archive_object_files(&object_files, &absolute_output_file_path)
                    .map_err(ProjectBuildError::ArchivalError)?;

                println!("Generated static library");
            }
        }

        println!("Built {}", project.name);

        Ok(())
    }

    fn compile_source_file(
        &self,
        project: &Project,
        source: &str,
        c_compiler: &dyn CCompiler,
        c_compilation_settings: &CCompilationSettings,
        cpp_compiler: &dyn CppCompiler,
        cpp_compilation_settings: &CppCompilationSettings,
    ) -> Result<(), SourceFileBuildError> {
        let absolute_source_file_path = project.base_path.join(source);
        let absolute_output_file_path = project
            .base_path
            .join(BAKERY_BUILD_DIRECTORY)
            .join(PathBuf::from(source).file_name().unwrap())
            .with_extension(OBJECT_FILE_EXTENSION);

        match project.language {
            Language::C => {
                c_compiler
                    .compile_source_file(
                        &absolute_source_file_path,
                        &absolute_output_file_path,
                        c_compilation_settings,
                    )
                    .map_err(SourceFileBuildError::FailedToCompile)?;
            }
            Language::Cpp => {
                cpp_compiler
                    .compile_source_file(
                        &absolute_source_file_path,
                        &absolute_output_file_path,
                        cpp_compilation_settings,
                    )
                    .map_err(SourceFileBuildError::FailedToCompile)?;
            }
        }

        Ok(())
    }

    fn copy_artifacts_to_build_directory(&self, project: &Project) -> Result<(), io::Error> {
        for (index, artifact) in self.collect_artifacts(project).into_iter().enumerate() {
            if index == 0 && project.distribution == Distribution::DynamicLibrary {
                continue;
            }

            fs::copy(
                &artifact,
                project
                    .base_path
                    .join(BAKERY_BUILD_DIRECTORY)
                    .join(artifact.file_name().unwrap()),
            )?;
        }

        Ok(())
    }
}

impl Task for Build {
    fn id(&self) -> &'static str {
        "build"
    }

    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    fn on_execute(&mut self, context: &TaskContext) {
        let project = &context.project;
        let toolchain_configuration = &context.toolchain_configuration;

        let c_compiler = match self.create_c_compiler(toolchain_configuration) {
            Some(c_compiler) => c_compiler,
            None => {
                eprintln!("C compiler not found");

                return;
            }
        };
        let cpp_compiler = match self.create_cpp_compiler(toolchain_configuration) {
            Some(cpp_compiler) => cpp_compiler,
            None => {
                eprintln!("C++ compiler not found");

                return;
            }
        };
        let archiver = match self.create_archiver(toolchain_configuration) {
            Some(archiver) => archiver,
            None => {
                eprintln!("Archiver not found");

                return;
            }
        };

        let sources = self.collect_sources_to_compile(project);

        if sources.is_empty() {
            println!("Nothing to build");

            return;
        }

        if let Err(err) = self.create_directories(project) {
            eprintln!("Failed to create directories: {}", err);

            return;
        }

        if !project.dependencies.is_empty() {
            println!("Building dependencies");

            match self.build_dependencies(
                project,
                c_compiler.as_ref(),
                cpp_compiler.as_ref(),
                archiver.as_ref(),
            ) {
                Ok(_) => {
                    println!("Built dependencies");
                }
                Err(err) => {
                    eprintln!("Failed to build dependencies: {}", err);

                    return;
                }
            }
        }

        match self.build(
            project,
            sources,
            c_compiler.as_ref(),
            cpp_compiler.as_ref(),
            archiver.as_ref(),
        ) {
            Ok(_) => {
                if let Err(err) = self.copy_artifacts_to_build_directory(project) {
                    eprintln!("Failed to copy artifacts to build directory: {}", err);
                }
            }
            Err(err) => eprintln!("{}", err),
        }
    }
}

fn hash_file(file: &File) -> Result<Hash, io::Error> {
    let file_content = unsafe { MmapOptions::new().map(file)? };

    Ok(blake3::hash(&file_content))
}
