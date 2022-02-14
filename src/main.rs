mod config;
mod project;
mod tools;

pub(crate) use project::*;

use clap::{App, SubCommand};
use config::EzConfiguration;
use std::{
    env,
    error::Error,
    fs::{self, File},
    io::Write,
};

fn main() -> Result<(), Box<dyn Error>> {
    let ez_configuration_path = {
        let mut executable_path = env::current_exe()?;
        executable_path.pop();
        executable_path.push("config.toml");
        executable_path
    };

    if !ez_configuration_path.exists() {
        let ez_configuration = EzConfiguration::default();
        let ez_configuration_toml = toml::to_string_pretty(&ez_configuration)?;

        File::options()
            .create(true)
            .write(true)
            .open(&ez_configuration_path)?
            .write_all(ez_configuration_toml.as_bytes())?;
    }

    let ez_configuration_content = fs::read_to_string(&ez_configuration_path)?;
    let ez_configuration = toml::from_str::<EzConfiguration>(&ez_configuration_content)?;

    let matches = App::new("ez")
        .version("0.1")
        .author("Bakamono")
        .about("Build system for C/C++")
        .subcommand(SubCommand::with_name("build"))
        .subcommand(SubCommand::with_name("run"))
        .get_matches();

    if matches.subcommand_matches("build").is_some() {
        Project::open(".")?.build(&ez_configuration)?;
    } else if matches.subcommand_matches("run").is_some() {
        Project::open(".")?.run(&ez_configuration)?;
    }

    Ok(())
}
