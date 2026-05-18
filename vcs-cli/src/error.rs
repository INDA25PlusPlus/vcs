use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use vcs_core::repo::RepoError;

#[derive(Debug)]
pub(crate) enum CliError {
    MissingObject,
    NoStagedChanges,
    StorageError(String),
    InvalidPath(PathBuf),
    KeyGeneration,
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CliError::MissingObject => write!(f, "not a vcs repository"),
            CliError::NoStagedChanges => write!(f, "no staged changes to commit"),
            CliError::StorageError(err) => write!(f, "{err}"),
            CliError::InvalidPath(path) => {
                write!(f, "invalid repository path '{}'", path.display())
            }
            CliError::KeyGeneration => write!(f, "failed to create signing key"),
        }
    }
}

impl std::error::Error for CliError {}

impl<E: Display> From<RepoError<E>> for CliError {
    fn from(value: RepoError<E>) -> Self {
        match value {
            RepoError::MissingObject => CliError::MissingObject,
            RepoError::NoStagedChanges => CliError::NoStagedChanges,
            RepoError::StorageError(err) => CliError::StorageError(err.to_string()),
        }
    }
}
