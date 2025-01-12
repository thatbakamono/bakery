mod config;
mod extensions;
mod project;
mod task;
mod tools;

pub(crate) use extensions::*;
pub(crate) use project::*;
pub(crate) use task::*;

use clap::Command;
use config::ToolchainConfiguration;
use eyre::Context;
use itertools::Itertools;
use std::{
    collections::{HashMap, VecDeque},
    env,
    fs::{self, File},
    io::Write,
    path,
};

pub const BUILD_CONFIGURATION_FILE: &str = "bakery.toml";
pub const BAKERY_BUILD_DIRECTORY: &str = ".bakery/build";
pub const BAKERY_CACHE_DIRECTORY: &str = ".bakery/cache";
pub const BAKERY_HASHES_FILE: &str = ".bakery/cache/hashes.json";

fn main() -> eyre::Result<()> {
    let toolchain_configuration = deserialize_toolchain_configuration()
        .context("Failed to deserialize toolchain configuration")?;

    let mut tasks: HashMap<&str, Box<dyn Task>> = HashMap::new();

    tasks.insert("build", Box::new(Build::new()));
    tasks.insert("run", Box::new(Run::new()));

    let matches = Command::new("bakery")
        .version("0.1")
        .author("Bakamono")
        .about("Build system for C/C++")
        .subcommand(Command::new("build"))
        .subcommand(Command::new("run"))
        .get_matches();

    match Project::open(".") {
        Ok(project) => {
            if let Some((subcommand, _parameters)) = matches.subcommand() {
                if tasks.contains_key(subcommand) {
                    let context = TaskContext {
                        project,
                        toolchain_configuration,
                    };

                    execute_task_and_its_dependencies(&mut tasks, subcommand, &context);
                }
            }
        }
        Err(error) => match error {
            ProjectOpenError::InvalidProjectPath(_error) => {
                eprintln!("There is no bakery.toml in the current directory")
            }
            ProjectOpenError::InvalidBuildConfiguration(build_configuration_error) => {
                match build_configuration_error {
                    BuildConfigurationError::SyntaxError(error) => {
                        eprintln!("Syntax error occured:");

                        for line in error.split(path::is_separator) {
                            eprintln!("{line}");
                        }
                    }
                    BuildConfigurationError::InvalidName => eprintln!("Project's name consists of invalid characters. Valid characters are: {NAME_PATTERN}"),
                    BuildConfigurationError::IncorrectWildcard(wildcard) => eprintln!("Incorrect wildcard: {wildcard}"),
                    BuildConfigurationError::IncorrectSource(source) => {
                        eprintln!("Incorrect source: {}", source);
                    }
                    BuildConfigurationError::IncorrectInclude(include) => {
                        eprintln!("Incorrect include: {}", include);
                    }
                    BuildConfigurationError::DependencyIsNotALibrary(dependency) => {
                        eprintln!("Dependency is not a library: {}", dependency);
                    }
                }
            }
        },
    }

    Ok(())
}

fn execute_task_and_its_dependencies(
    tasks: &mut HashMap<&str, Box<dyn Task>>,
    task_id: &str,
    context: &TaskContext,
) {
    let mut processing_stack = VecDeque::new();
    let mut result_stack = VecDeque::new();

    processing_stack.push_front(task_id);

    while let Some(current_task_id) = processing_stack.pop_front() {
        result_stack.push_front(current_task_id);

        for dependency in tasks.get(current_task_id).unwrap().dependencies() {
            processing_stack.push_front(dependency);
        }
    }

    for task_id in result_stack.into_iter().unique() {
        tasks.get_mut(task_id).unwrap().on_execute(context);
    }
}

fn deserialize_toolchain_configuration() -> eyre::Result<ToolchainConfiguration> {
    let toolchain_configuration_path = {
        let mut executable_path = env::current_exe()?;
        executable_path.pop();
        executable_path.push("config.toml");
        executable_path
    };

    if !toolchain_configuration_path.exists() {
        let toolchain_configuration = ToolchainConfiguration::default();
        let toolchain_configuration_toml = toml::to_string_pretty(&toolchain_configuration)?;

        File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&toolchain_configuration_path)?
            .write_all(toolchain_configuration_toml.as_bytes())?;
    }

    let toolchain_configuration_content = fs::read_to_string(&toolchain_configuration_path)?;
    let toolchain_configuration =
        toml::from_str::<ToolchainConfiguration>(&toolchain_configuration_content)?;

    Ok(toolchain_configuration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, path::PathBuf, rc::Rc};

    #[test]
    fn test_execute_dependencies() {
        struct Task1 {
            steps: Rc<RefCell<Vec<u32>>>,
        }

        impl Task for Task1 {
            fn id(&self) -> &'static str {
                "task1"
            }

            fn dependencies(&self) -> &[&'static str] {
                &["task2", "task3"]
            }

            fn on_execute(&mut self, _context: &TaskContext) {
                self.steps.borrow_mut().push(1);
            }
        }

        struct Task2 {
            steps: Rc<RefCell<Vec<u32>>>,
        }

        impl Task for Task2 {
            fn id(&self) -> &'static str {
                "task2"
            }

            fn dependencies(&self) -> &[&'static str] {
                &["task3"]
            }

            fn on_execute(&mut self, _context: &TaskContext) {
                self.steps.borrow_mut().push(2);
            }
        }

        struct Task3 {
            steps: Rc<RefCell<Vec<u32>>>,
        }

        impl Task for Task3 {
            fn id(&self) -> &'static str {
                "task3"
            }

            fn dependencies(&self) -> &[&'static str] {
                &["task4"]
            }

            fn on_execute(&mut self, _context: &TaskContext) {
                self.steps.borrow_mut().push(3);
            }
        }

        struct Task4 {
            steps: Rc<RefCell<Vec<u32>>>,
        }

        impl Task for Task4 {
            fn id(&self) -> &'static str {
                "task4"
            }

            fn dependencies(&self) -> &[&'static str] {
                &[]
            }

            fn on_execute(&mut self, _context: &TaskContext) {
                self.steps.borrow_mut().push(4);
            }
        }

        let mut tasks: HashMap<&str, Box<dyn Task>> = HashMap::new();

        let steps = Rc::new(RefCell::new(vec![]));

        tasks.insert(
            "task1",
            Box::new(Task1 {
                steps: Rc::clone(&steps),
            }),
        );
        tasks.insert(
            "task2",
            Box::new(Task2 {
                steps: Rc::clone(&steps),
            }),
        );
        tasks.insert(
            "task3",
            Box::new(Task3 {
                steps: Rc::clone(&steps),
            }),
        );
        tasks.insert(
            "task4",
            Box::new(Task4 {
                steps: Rc::clone(&steps),
            }),
        );

        let context = TaskContext {
            project: Project {
                base_path: PathBuf::new(),
                name: String::new(),
                description: None,
                author: None,
                language: config::Language::Cpp,
                distribution: config::Distribution::Executable,
                sources: vec![],
                includes: vec![],
                dependencies: vec![],
                optimization: config::OptimizationLevel::Zero,
                enable_all_warnings: false,
                treat_all_warnings_as_errors: false,
                has_project_configuration_changed: false,
                hashes: HashMap::new(),
                c: None,
                cpp: None,
                gcc: None,
                gpp: None,
            },
            toolchain_configuration: ToolchainConfiguration::default(),
        };

        execute_task_and_its_dependencies(&mut tasks, "task1", &context);

        assert_eq!(*steps.borrow(), vec![4, 3, 2, 1]);
    }
}
