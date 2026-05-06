pub mod file;
pub mod map_ops;
mod memory;
pub mod path;

use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::diff::hunk_collection::HunkCollectionError;
use crate::diff::repo_diff::RepoDiff;
use crate::fs::file::{FileChange, FileRef};
use crate::repo::PendingChanges;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::StorageError;
use file::File;
use path::RepoPath;
use std::collections::HashMap;
use std::{future::Future, hash::Hash};
use thiserror::Error;

pub struct FileTree<D> {
    // todo lazy loading from aggregate repo diffs
    files: HashMap<RepoPath, FileRef<D>>,
}

pub type FileSystemResult<T, E> = Result<T, FileSystemError<E>>;

pub type FileSystemReadResult<T, E, SE> = Result<T, FileSystemReadError<E, SE>>;

pub type FileSystemWriteResult<T, E, SE> = Result<T, FileSystemWriteError<E, SE>>;

#[derive(Clone, Debug, Error)]
pub enum FileSystemError<E> {
    #[error("internal file system error: {0}")]
    InternalError(E),
    #[error("file does not exist")]
    MissingFile,
}

#[derive(Clone, Debug, Error)]
pub enum FileSystemReadError<FE, SE> {
    #[error("{0}")]
    FileSystemError(FileSystemError<FE>),
    #[error("storage error: {0}")]
    LoadError(StorageError<SE>),
    #[error("storage error: {0}")]
    StoreError(SE),
}

#[derive(Clone, Debug, Error)]
pub enum FileSystemWriteError<FE, SE> {
    #[error("{0}")]
    FileSystemError(FileSystemError<FE>),
    #[error("storage error: {0}")]
    LoadError(StorageError<SE>),
    #[error("storage error: {0}")]
    StoreError(SE),
    #[error("invalid pending changes")]
    InvalidPendingChanges,
    #[error("hunk error: {0}")]
    HunkError(HunkCollectionError),
}

pub trait FileSystem<D: CryptoDigest + CryptoHash + Send> {
    type Error;

    /// Read a [`File`] from the file system
    fn read(
        &self,
        path: &RepoPath,
    ) -> impl Future<Output = FileSystemResult<File, Self::Error>> + Send;

    /// Write a [`File`] to the file system
    fn write(
        &self,
        path: &RepoPath,
        file: &File,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Delete a [`File`] from the file system
    fn delete(
        &self,
        path: &RepoPath,
    ) -> impl Future<Output = FileSystemResult<(), Self::Error>> + Send;

    /// Update `pending_changes` to match the diff from `head` to the current file tree.
    /// (`pending_changes` = files - `head`)
    ///
    /// `head_changed`: Set to `true` if `head` may have changed since the last call to
    /// `read_pending_changes` or `write_pending_changes`. If `false`, the implementer may assume
    /// that `head` has not changed.
    fn read_pending_changes<P, S>(
        &self,
        diff_policy: &P,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &mut PendingChanges<D>,
        head_changed: bool,
    ) -> impl Future<Output = FileSystemReadResult<(), Self::Error, S::RepoStorageError>>
    where
        P: DiffPolicy,
        S: RepoStorage<D>;

    /// Update the file tree to match `pending_changes` applied to `head`.
    /// (files = `head` + `pending_changes`)
    ///
    /// `head_changed`: Set to `true` if `head` may have changed since the last call to
    /// `read_pending_changes` or `write_pending_changes`. If `false`, the implementer may assume
    /// that `head` has not changed.
    fn write_pending_changes<S>(
        &self,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &PendingChanges<D>,
        head_changed: bool,
    ) -> impl Future<Output = FileSystemWriteResult<(), Self::Error, S::RepoStorageError>>
    where
        S: RepoStorage<D>;
}

#[derive(Clone, Copy, Debug, Error)]
pub enum FileTreeError {
    #[error("invalid file change mode")]
    InvalidFileChangeMode,
}

impl<D: CryptoDigest + CryptoHash + Eq + Hash> TryFrom<RepoDiff<D>> for FileTree<D> {
    type Error = FileTreeError;

    fn try_from(value: RepoDiff<D>) -> Result<Self, Self::Error> {
        value
            .file_diffs
            .into_iter()
            .map(|(path, change)| match change {
                FileChange::Create(file) => Ok((path, file)),
                _ => Err(FileTreeError::InvalidFileChangeMode),
            })
            .collect::<Result<HashMap<_, _>, _>>()
            .map(|files| FileTree { files })
    }
}

impl<E> From<E> for FileSystemError<E> {
    fn from(value: E) -> Self {
        FileSystemError::InternalError(value)
    }
}

impl<FE, SE> From<FileSystemError<FE>> for FileSystemReadError<FE, SE> {
    fn from(value: FileSystemError<FE>) -> Self {
        FileSystemReadError::FileSystemError(value)
    }
}

impl<FE, SE> From<FileSystemError<FE>> for FileSystemWriteError<FE, SE> {
    fn from(value: FileSystemError<FE>) -> Self {
        FileSystemWriteError::FileSystemError(value)
    }
}
