use crate::config::{
    BuildConfiguration, CPPStandard, CStandard, EzConfiguration, Language, OptimizationLevel,
};
use eyre::eyre;
use std::error::Error;
use std::fs;
use std::process::Command;

pub(crate) fn build(ez_configuration: &EzConfiguration) -> Result<(), Box<dyn Error>> {
    let build_content = fs::read_to_string("ez.toml")?;
    let build = toml::from_str::<BuildConfiguration>(&build_content)?;

    if let Some(ref sources) = build.project.sources {
        let compiler_location = match build.project.language {
            Language::C => {
                locate_gcc(ez_configuration).ok_or_else(|| eyre!("Failed to locate GCC"))?
            }
            Language::CPP => {
                locate_gpp(ez_configuration).ok_or_else(|| eyre!("Failed to locate G++"))?
            }
        };

        if !sources.is_empty() {
            for source in sources {
                println!("Compiling {} in {}", source, build.project.name);

                let mut command = Command::new(&compiler_location);

                match build.project.language {
                    Language::C => {
                        if let Some(ref gcc) = build.gcc {
                            if let Some(ref additional_pre_arguments) = gcc.additional_pre_arguments
                            {
                                for additional_pre_argument in additional_pre_arguments {
                                    command.arg(additional_pre_argument);
                                }
                            }
                        }
                    }
                    Language::CPP => {
                        if let Some(ref gpp) = build.gpp {
                            if let Some(ref additional_pre_arguments) = gpp.additional_pre_arguments
                            {
                                for additional_pre_argument in additional_pre_arguments {
                                    command.arg(additional_pre_argument);
                                }
                            }
                        }
                    }
                }

                command.arg("-c");

                command.arg(&format!(
                    "-x{}",
                    match build.project.language {
                        Language::C => "c",
                        Language::CPP => "c++",
                    }
                ));

                match build.project.language {
                    Language::C => {
                        if let Some(ref c) = build.c {
                            if let Some(ref standard) = c.standard {
                                command.arg(&format!(
                                    "-std={}",
                                    match standard {
                                        CStandard::EightyNine => "c89",
                                        CStandard::NinetyNine => "c99",
                                        CStandard::Eleven => "c11",
                                        CStandard::Seventeen => "c17",
                                    }
                                ));
                            }
                        }
                    }
                    Language::CPP => {
                        if let Some(ref cpp) = build.cpp {
                            if let Some(ref standard) = cpp.standard {
                                command.arg(&format!(
                                    "-std={}",
                                    match standard {
                                        CPPStandard::NinetyEight => "c++98",
                                        CPPStandard::Three => "c++3",
                                        CPPStandard::Eleven => "c++11",
                                        CPPStandard::Fourteen => "c++14",
                                        CPPStandard::Seventeen => "c++17",
                                        CPPStandard::Twenty => "c++20",
                                    }
                                ));
                            }
                        }
                    }
                }

                if let Some(ref optimization) = build.project.optimization {
                    command.arg(&format!(
                        "-O{}",
                        match optimization {
                            OptimizationLevel::Zero => "0",
                            OptimizationLevel::One => "1",
                            OptimizationLevel::Two => "2",
                            OptimizationLevel::Three => "3",
                            OptimizationLevel::Four => "fast",
                            OptimizationLevel::Size => "s",
                            OptimizationLevel::Debug => "g",
                        }
                    ));
                }

                if build.project.enable_all_warnings.unwrap_or(false) {
                    command.arg("-Wall");
                    command.arg("-Wpedantic");
                }

                if build.project.treat_all_warnings_as_errors.unwrap_or(false) {
                    command.arg("-Werror");
                }

                command.arg(source);

                if let Some(ref includes) = build.project.includes {
                    for include in includes {
                        command.arg(&format!("-I{}", include));
                    }
                }

                match build.project.language {
                    Language::C => {
                        if let Some(ref gcc) = build.gcc {
                            if let Some(ref additional_post_arguments) =
                                gcc.additional_post_arguments
                            {
                                for additional_post_argument in additional_post_arguments {
                                    command.arg(additional_post_argument);
                                }
                            }
                        }
                    }
                    Language::CPP => {
                        if let Some(ref gpp) = build.gpp {
                            if let Some(ref additional_post_arguments) =
                                gpp.additional_post_arguments
                            {
                                for additional_post_argument in additional_post_arguments {
                                    command.arg(additional_post_argument);
                                }
                            }
                        }
                    }
                }

                let output = command.output()?;

                if output.status.success() {
                    print!("{}", String::from_utf8_lossy(&output.stderr));
                } else {
                    println!("Failed to compile {} in {}", source, build.project.name);
                    print!("{}", String::from_utf8_lossy(&output.stderr));

                    return Ok(());
                }
            }

            println!("Linking {}", build.project.name);

            let output = Command::new(&compiler_location)
                .arg(&sources.join(" "))
                .arg(&format!("-o{}", build.project.name))
                .output()?;

            if !output.status.success() {
                println!("Failed to link {}", build.project.name);
                print!("{}", String::from_utf8_lossy(&output.stderr));
            }
        }
    }

    Ok(())
}

fn locate_gcc(ez_configuration: &EzConfiguration) -> Option<String> {
    if let Some(ref gcc_location) = ez_configuration.gcc_location {
        Some(gcc_location.clone())
    } else {
        if cfg!(target_os = "windows") {
            Some(which::which("gcc.exe").ok()?.to_string_lossy().into_owned())
        } else if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Some(which::which("gcc").ok()?.to_string_lossy().into_owned())
        } else {
            None
        }
    }
}

fn locate_gpp(ez_configuration: &EzConfiguration) -> Option<String> {
    if let Some(ref gpp_location) = ez_configuration.gpp_location {
        Some(gpp_location.clone())
    } else {
        if cfg!(target_os = "windows") {
            Some(which::which("g++.exe").ok()?.to_string_lossy().into_owned())
        } else if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Some(which::which("g++").ok()?.to_string_lossy().into_owned())
        } else {
            None
        }
    }
}
