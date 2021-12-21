mod commands;
mod config;

use std::error::Error;
use clap::{App, SubCommand};

fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("ez")
        .version("0.1")
        .author("Bakamono")
        .about("Build system for C/C++")
        .subcommand(SubCommand::with_name("build"))
        .get_matches();

    if let Some(_) = matches.subcommand_matches("build") {
        commands::build()?;
    }

    Ok(())
}
