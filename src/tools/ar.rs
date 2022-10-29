use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::config::ToolchainConfiguration;

pub(crate) struct Ar {
    location: String,
}

impl Ar {
    fn new(location: String) -> Ar {
        Ar { location }
    }

    pub(crate) fn locate(toolchain_configuration: &ToolchainConfiguration) -> Option<Ar> {
        if let Some(ref ar_location) = toolchain_configuration.ar_location {
            Some(Ar::new(ar_location.clone()))
        } else if cfg!(target_os = "windows") {
            Some(Ar::new(
                which::which("ar.exe").ok()?.to_string_lossy().into_owned(),
            ))
        } else if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Some(Ar::new(
                which::which("ar").ok()?.to_string_lossy().into_owned(),
            ))
        } else {
            None
        }
    }

    pub(crate) fn archive_object_files(
        &self,
        object_files: &Vec<PathBuf>,
        output_file: &impl AsRef<Path>,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location);

        command.arg("rcs");
        command.arg(output_file.as_ref());

        for object_file in object_files {
            command.arg(object_file);
        }

        let output = command.output().unwrap();

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}
