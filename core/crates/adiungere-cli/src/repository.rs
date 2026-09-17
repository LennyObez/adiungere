//! Finding the documents the command line reads.
//!
//! The register and the roadmap live in the repository, so the command works from anywhere inside a checkout
//! by walking upwards until it finds both. It never guesses a path and never falls back to a partial answer:
//! either both documents are found together, or the caller is told that neither was.

use std::path::{Path, PathBuf};

/// Path of the evidence register, relative to the repository root.
pub const REGISTER: &str = "docs/evidence.md";

/// Path of the roadmap, relative to the repository root.
pub const ROADMAP: &str = "docs/roadmap.md";

/// Walks upwards from `start` and returns the first directory holding both documents.
///
/// Returns [`None`] when no ancestor holds them, which is what happens outside a checkout.
#[must_use]
pub fn locate_root(start: &Path) -> Option<PathBuf> {
    let mut candidate = Some(start);

    while let Some(directory) = candidate {
        if directory.join(REGISTER).is_file() && directory.join(ROADMAP).is_file() {
            return Some(directory.to_path_buf());
        }

        candidate = directory.parent();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{REGISTER, ROADMAP, locate_root};
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn a_directory_holding_both_documents_is_the_root() {
        // Arrange
        let temporary = Temporary::new("both-documents");
        fs::create_dir_all(temporary.join("docs")).unwrap();
        fs::write(temporary.join(REGISTER), "register").unwrap();
        fs::write(temporary.join(ROADMAP), "roadmap").unwrap();

        // Act
        let found = locate_root(temporary.path());

        // Assert
        assert_eq!(found.as_deref(), Some(temporary.path()));
    }

    #[test]
    fn the_search_climbs_out_of_a_subdirectory() {
        // Arrange
        let temporary = Temporary::new("from-below");
        let deep = temporary.join("core/crates/somewhere");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(temporary.join("docs")).unwrap();
        fs::write(temporary.join(REGISTER), "register").unwrap();
        fs::write(temporary.join(ROADMAP), "roadmap").unwrap();

        // Act
        let found = locate_root(&deep);

        // Assert
        assert_eq!(found.as_deref(), Some(temporary.path()));
    }

    #[test]
    fn one_document_without_the_other_is_not_a_root() {
        // A partial answer here would send every later step to a file that does not exist, and report the
        // absence as an empty register rather than as a missing repository.

        // Arrange
        let temporary = Temporary::new("register-only");
        fs::create_dir_all(temporary.join("docs")).unwrap();
        fs::write(temporary.join(REGISTER), "register").unwrap();

        // Act
        let found = locate_root(temporary.path());

        // Assert
        assert_eq!(found, None);
    }

    /// A directory that exists for one test and is removed when the test ends, whichever way it ends.
    struct Temporary(PathBuf);

    impl Temporary {
        fn new(name: &str) -> Self {
            let unique = format!(
                "adiungere-{name}-{}-{:?}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_nanos())
                    .unwrap_or_default()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn join(&self, relative: &str) -> PathBuf {
            self.0.join(relative)
        }
    }

    impl Drop for Temporary {
        fn drop(&mut self) {
            // A directory that cannot be removed is not a failed test; it is left for the operating system.
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
