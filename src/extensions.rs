use std::path::{Path, PathBuf};

pub trait PathExtension {
    fn relative_to(&self, other: &Self) -> Option<PathBuf>;
}

impl PathExtension for Path {
    fn relative_to(&self, other: &Self) -> Option<PathBuf> {
        let from = if self.is_relative() {
            self.canonicalize().ok()?
        } else {
            self.to_path_buf()
        };

        let to = if other.is_relative() {
            other.canonicalize().ok()?
        } else {
            other.to_path_buf()
        };

        // TODO: Support cases where from isn't to + something
        if !from.starts_with(&to) {
            return None;
        }

        Some(
            from.components()
                .skip(to.components().count())
                .collect::<PathBuf>(),
        )
    }
}
