use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoPath {
    components: Vec<RepoPathComponent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoPathComponent {
    inner: String,
}

impl Display for RepoPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl From<RepoPath> for PathBuf {
    fn from(value: RepoPath) -> Self {
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
