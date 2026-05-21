use crate::changeset::file::FileChangeError;
use crate::diff::hunk::HunkCollectionError;
use crate::fs::{FileSystemReadError, FileSystemWriteError, FileTreeError};
use crate::storage::StorageError;

pub type RepoResult<T, E> = Result<T, RepoError<E>>;

#[derive(Debug, thiserror::Error)]
pub enum RepoError<E> {
    #[error("failed to find object in database")]
    MissingObject,
    #[error("no staged changes to commit")]
    NoStagedChanges,
    #[error("invalid file change")]
    InvalidFileChange,
    #[error("invalid file diff: {0}")]
    InvalidFileDiff(HunkCollectionError),
    #[error("invalid file tree: {0}")]
    InvalidFileTree(FileTreeError),
    #[error("internal storage error: '{0}'")]
    StorageError(E),
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshPendingChangesError<FE, SE> {
    #[error("{0}")]
    Repo(#[from] RepoError<SE>),
    #[error("invalid file tree at head: {0}")]
    InvalidHeadFileTree(#[from] FileTreeError),
    #[error("{0}")]
    FileSystem(#[from] FileSystemReadError<FE, SE>),
}

#[derive(Debug, thiserror::Error)]
pub enum CheckoutError<FE, SE> {
    #[error("{0}")]
    Repo(#[from] RepoError<SE>),
    #[error("{0}")]
    FileSystemWrite(#[from] FileSystemWriteError<FE, SE>),
}

#[derive(Debug, thiserror::Error)]
pub enum RestoreError<FE, SE> {
    #[error("{0}")]
    Repo(#[from] RepoError<SE>),
    #[error("{0}")]
    FileSystemWrite(#[from] FileSystemWriteError<FE, SE>),
}

impl<E> From<StorageError<E>> for RepoError<E> {
    fn from(value: StorageError<E>) -> Self {
        match value {
            StorageError::InternalError(err) => RepoError::StorageError(err),
            StorageError::MissingObject => RepoError::MissingObject,
        }
    }
}

impl<E> From<E> for RepoError<E> {
    fn from(value: E) -> Self {
        RepoError::StorageError(value)
    }
}

impl<E> From<FileChangeError<E>> for RepoError<E> {
    fn from(value: FileChangeError<E>) -> Self {
        match value {
            FileChangeError::StorageError(err) => RepoError::from(err),
            FileChangeError::InvalidFileDiff(err) => RepoError::InvalidFileDiff(err),
            FileChangeError::InvalidFileChange => RepoError::InvalidFileChange,
        }
    }
}
