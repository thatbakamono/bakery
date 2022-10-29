mod config;
mod project;
mod tools;

pub(crate) use project::*;

use clap::{App, SubCommand};
use config::ToolchainConfiguration;
use std::{
    env,
    error::Error,
    fs::{self, File},
    io::Write,
};

fn main() -> Result<(), Box<dyn Error>> {
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
            .write(true)
            .open(&toolchain_configuration_path)?
            .write_all(toolchain_configuration_toml.as_bytes())?;
    }

    let toolchain_configuration_content = fs::read_to_string(&toolchain_configuration_path)?;
    let toolchain_configuration =
        toml::from_str::<ToolchainConfiguration>(&toolchain_configuration_content)?;

    let matches = App::new("ez")
        .version("0.1")
        .author("Bakamono")
        .about("Build system for C/C++")
        .subcommand(SubCommand::with_name("build"))
        .subcommand(SubCommand::with_name("run"))
        .get_matches();

    if matches.subcommand_matches("build").is_some() {
        Project::open(".")?.build(&toolchain_configuration)?;
    } else if matches.subcommand_matches("run").is_some() {
        Project::open(".")?.run(&toolchain_configuration)?;
    }

    Ok(())
}
