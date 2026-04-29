use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Relative path referring to a directory or (regular) file within a repository. Composed of zero
/// or more [`RepoPathComponent`]s. A path with zero components corresponds to the repository root
/// directory. Imposes some limitations to ensure compatibility with Linux paths:
/// - Combined length of path components plus path separators must be less than or equal to 4096.
///
/// Limitations for path components:
/// - Length must be between 1 and 255 bytes, inclusive.
/// - May not contain the null byte ('\0') or the slash symbol ('/').
/// - May not be equal to '.' or '..'.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoPath {
    components: Box<[RepoPathComponent]>,
}

/// Component of a [`RepoPath`] corresponding to a directory or file name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoPathComponent {
    inner: Box<[u8]>,
}

impl RepoPath {
    pub fn new() -> RepoPath {
        RepoPath {
            components: vec![].into_boxed_slice(),
        }
    }

    pub fn components(&self) -> &[RepoPathComponent] {
        &self.components
    }
}

impl Display for RepoPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl TryFrom<&RepoPath> for PathBuf {
    type Error = ();

    fn try_from(value: &RepoPath) -> Result<PathBuf, Self::Error> {
        todo!()
    }
}

impl TryFrom<&Path> for RepoPath {
    type Error = ();

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        todo!("error handling, e.g. fail if path is absolute or contains ..")
    }
}

impl FromStr for RepoPathComponent {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!("validate format")
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
