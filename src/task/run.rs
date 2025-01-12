use super::{Task, TaskContext};
use crate::{config::Distribution, BAKERY_BUILD_DIRECTORY};
use std::process::Command;

const EXECUTABLE_EXTENSION: &str = if cfg!(target_os = "windows") {
    "exe"
} else if cfg!(target_os = "linux") {
    ""
} else {
    unreachable!()
};

pub struct Run {}

impl Run {
    pub fn new() -> Self {
        Self {}
    }
}

impl Task for Run {
    fn id(&self) -> &'static str {
        "run"
    }

    fn dependencies(&self) -> &[&'static str] {
        &["build"]
    }

    fn on_execute(&mut self, context: &TaskContext) {
        let project = &context.project;

        if project.distribution != Distribution::Executable {
            eprintln!("Skipping run task because the project is not an executable");
        }

        let absolute_executable_path = project
            .base_path
            .join(BAKERY_BUILD_DIRECTORY)
            .join(&project.name)
            .with_extension(EXECUTABLE_EXTENSION);

        let mut command = Command::new(&absolute_executable_path);

        println!("Running {}", project.name);

        if let Err(error) = command.status() {
            eprintln!("Failed to run the executable: {}", error);
        }
    }
}
