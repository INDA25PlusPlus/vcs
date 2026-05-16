use std::convert::Infallible;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use vcs_core::repo::RepoError;

type CoreError = RepoError<Infallible>;

#[derive(Debug)]
pub(crate) enum CliError {
    Core(CoreError),
    InvalidPath(PathBuf),
    KeyGeneration,
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Core(RepoError::MissingObject) => write!(f, "not a vcs repository"),
            CliError::Core(RepoError::StorageError(err)) => match *err {},
            CliError::InvalidPath(path) => {
                write!(f, "invalid repository path '{}'", path.display())
            }
            CliError::KeyGeneration => write!(f, "failed to create signing key"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<CoreError> for CliError {
    fn from(value: CoreError) -> Self {
        CliError::Core(value)
    }
}
