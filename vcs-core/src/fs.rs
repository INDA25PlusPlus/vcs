pub mod disk;
pub mod map_ops;
pub mod memory;
pub mod path;

use crate::changeset::Changeset;
use crate::changeset::file::File;
use crate::changeset::file::{FileChange, FileDiff, FileRef};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::diff::hunk::{HunkCollection, HunkCollectionError};
use crate::fs::map_ops::replace_or_insert;
use crate::repo::PendingChanges;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::StorageError;
use dashmap::{DashMap, ReadOnlyView};
use path::RepoPath;
use std::{future::Future, hash::Hash};
use thiserror::Error;

pub struct FileTree<D> {
    // todo lazy loading from aggregate repo diffs
    files: ReadOnlyView<RepoPath, FileRef<D>>,
}

impl<D> FileTree<D> {
    /// This method may be removed if this struct is refactored in the future
    pub fn read_only_view(&self) -> &ReadOnlyView<RepoPath, FileRef<D>> {
        &self.files
    }
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

pub trait FileSystem {
    type Error;

    /// Update `pending_changes` to match the diff from `head` to the current file tree.
    /// (`pending_changes` = files - `head`)
    ///
    /// `head_changed`: Set to `true` if `head` may have changed since the last call to
    /// `read_pending_changes` or `write_pending_changes`. If `false`, the implementer may assume
    /// that `head` has not changed.
    fn update_pending_changes<D, P, S>(
        &mut self,
        diff_policy: &P,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &mut PendingChanges<D>,
        head_changed: bool,
    ) -> impl Future<Output = FileSystemReadResult<(), Self::Error, S::RepoStorageError>>
    where
        D: CryptoDigest + CryptoHash + Send + Eq,
        P: DiffPolicy,
        S: RepoStorage<D>;

    /// Update the file tree to match `pending_changes` applied to `head`.
    /// (files = `head` + `pending_changes`)
    ///
    /// `head_changed`: Set to `true` if `head` may have changed since the last call to
    /// `read_pending_changes` or `write_pending_changes`. If `false`, the implementer may assume
    /// that `head` has not changed.
    fn apply_pending_changes<D, S>(
        &mut self,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &PendingChanges<D>,
        head_changed: bool,
    ) -> impl Future<Output = FileSystemWriteResult<(), Self::Error, S::RepoStorageError>>
    where
        D: CryptoDigest + CryptoHash + Send + Eq,
        S: RepoStorage<D>;
}

pub async fn update_create_file_change<D, E, S>(
    storage: &S,
    pending_changes: &DashMap<RepoPath, FileChange<D>>,
    path: &RepoPath,
    file: &File,
) -> FileSystemReadResult<(), E, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send,
    S: RepoStorage<D>,
{
    let file_digest = file.to_digest();
    storage
        .store(&file_digest, file)
        .await
        .map_err(FileSystemReadError::StoreError)?;
    replace_or_insert(pending_changes, path, FileChange::Create(file_digest));
    Ok(())
}

pub async fn update_modify_file_change<D, E, P, S>(
    diff_policy: &P,
    storage: &S,
    pending_changes: &DashMap<RepoPath, FileChange<D>>,
    path: &RepoPath,
    file_before: &File,
    file_after: &File,
) -> FileSystemReadResult<(), E, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send,
    P: DiffPolicy,
    S: RepoStorage<D>,
{
    if file_before == file_after {
        pending_changes.remove(path);
    } else {
        let hunks = if file_before.content == file_after.content {
            HunkCollection::default()
        } else {
            diff_policy.diff(&file_before.content, &file_after.content)
        };
        let file_diff = FileDiff {
            hunks,
            executable_status: file_after.executable_status,
        };
        let file_diff_digest = file_diff.to_digest();
        storage
            .store(&file_diff_digest, &file_diff)
            .await
            .map_err(FileSystemReadError::StoreError)?;
        replace_or_insert(pending_changes, path, FileChange::Modify(file_diff_digest));
    }
    Ok(())
}

pub async fn update_delete_file_change<D, E, S>(
    storage: &S,
    pending_changes: &DashMap<RepoPath, FileChange<D>>,
    path: &RepoPath,
) -> FileSystemReadResult<(), E, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send,
    S: RepoStorage<D>,
{
    replace_or_insert(pending_changes, path, FileChange::Delete);
    // todo decrease ref count
    let _ = storage;
    Ok(())
}

#[derive(Clone, Copy, Debug, Error)]
pub enum FileTreeError {
    #[error("invalid file change mode")]
    InvalidFileChangeMode,
}

impl<D: CryptoDigest + CryptoHash + Eq + Hash> TryFrom<Changeset<D>> for FileTree<D> {
    type Error = FileTreeError;

    fn try_from(value: Changeset<D>) -> Result<Self, Self::Error> {
        value
            .changeset
            .into_inner()
            .into_iter()
            .map(|(path, change)| match change {
                FileChange::Create(file) => Ok((path, file)),
                _ => Err(FileTreeError::InvalidFileChangeMode),
            })
            .collect::<Result<DashMap<_, _>, _>>()
            .map(FileTree::from_files)
    }
}

impl<D> FileTree<D> {
    pub fn empty() -> FileTree<D> {
        FileTree::from_files(DashMap::new())
    }

    pub fn from_files(files: DashMap<RepoPath, FileRef<D>>) -> FileTree<D> {
        FileTree {
            files: files.into_read_only(),
        }
    }

    pub fn files(&self) -> &ReadOnlyView<RepoPath, FileRef<D>> {
        &self.files
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
