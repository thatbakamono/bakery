use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) struct Ar {
    location: String,
}

impl Ar {
    pub(crate) fn new(location: String) -> Ar {
        Ar { location }
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
