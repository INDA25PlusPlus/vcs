use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::diff::repo_diff::RepoDiff;
use crate::fs::file::{File, FileChange, FileDiff, FileDiffRef, FileRef};
use crate::fs::map_ops::{
    DashMapReadOnlyGuard, OuterJoinEntry, outer_join, remove_difference, replace_or_insert,
};
use crate::fs::path::RepoPath;
use crate::fs::{
    FileSystem, FileSystemError, FileSystemReadError, FileSystemReadResult, FileSystemResult,
    FileSystemWriteError, FileSystemWriteResult, FileTree, update_create_file_change,
    update_delete_file_change, update_modify_file_change,
};
use crate::repo::PendingChanges;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::Storage;
use dashmap::DashMap;
use futures::future::try_join_all;
use std::convert::Infallible;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use tokio::sync::RwLock;

pub struct MemoryFileSystem {
    files: RwLock<DashMap<RepoPath, MemoryFileSystemEntry>>,
}

#[derive(Clone, Debug)]
struct MemoryFileSystemEntry {
    file: File,
    dirty: bool,
}

impl MemoryFileSystem {
    pub fn new() -> MemoryFileSystem {
        MemoryFileSystem {
            files: RwLock::new(DashMap::new()),
        }
    }
}

impl<D: CryptoDigest + CryptoHash + Send> FileSystem<D> for MemoryFileSystem
where
    D: Eq,
{
    type Error = Infallible;

    async fn read(&self, path: &RepoPath) -> FileSystemResult<File, Self::Error> {
        self.files
            .read()
            .await
            .get(path)
            .map(|entry| entry.file.clone())
            .ok_or(FileSystemError::MissingFile)
    }

    async fn write(&self, path: &RepoPath, file: &File) -> Result<(), Self::Error> {
        self.files.read().await.insert(
            path.clone(),
            MemoryFileSystemEntry {
                file: file.clone(),
                dirty: true,
            },
        );
        Ok(())
    }

    async fn delete(&self, path: &RepoPath) -> FileSystemResult<(), Self::Error> {
        self.files
            .read()
            .await
            .remove(path)
            .ok_or(FileSystemError::MissingFile)?;
        Ok(())
    }

    async fn update_pending_changes<P, S>(
        &self,
        diff_policy: &P,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &PendingChanges<D>,
        head_changed: bool,
    ) -> FileSystemReadResult<(), Self::Error, S::RepoStorageError>
    where
        P: DiffPolicy,
        S: RepoStorage<D>,
    {
        let mut files = self.files.write().await;
        {
            let files_read_only = DashMapReadOnlyGuard::new(files.deref_mut());

            // regardless of if head changed, remove all changes to files that don't exist neither on
            // head nor in the file system
            remove_difference!(pending_changes.0.changeset, head.files, files_read_only);

            let outer_join = outer_join(&head.files, files_read_only.deref());

            let futures = outer_join.map(|(path, outer_join)| {
                update_change(
                    diff_policy,
                    storage,
                    pending_changes,
                    head_changed,
                    path,
                    outer_join,
                )
            });
            try_join_all(futures).await?;
        }
        // set all non-dirty
        files.iter_mut().for_each(|mut entry| entry.dirty = false);
        Ok(())
    }

    async fn apply_pending_changes<S>(
        &self,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &mut PendingChanges<D>,
        head_changed: bool,
    ) -> FileSystemWriteResult<(), Self::Error, S::RepoStorageError>
    where
        S: RepoStorage<D>,
    {
        let files = self.files.read().await;
        let PendingChanges(RepoDiff { changeset }) = pending_changes;
        let changeset_read_only = DashMapReadOnlyGuard::new(changeset);

        // regardless of if head changed, delete all files that don't exist on head and are not
        // changed in pending changes
        remove_difference!(files.deref(), head.files, changeset_read_only);

        let outer_join = outer_join(&head.files, changeset_read_only.deref());

        let futures = outer_join.map(|(path, outer_join)| {
            apply_change(storage, files.deref(), head_changed, path, outer_join)
        });
        try_join_all(futures).await?;

        Ok(())
    }
}

async fn update_change<D, E, P, S>(
    diff_policy: &P,
    storage: &S,
    pending_changes: &PendingChanges<D>,
    head_changed: bool,
    path: &RepoPath,
    join: OuterJoinEntry<&D, &MemoryFileSystemEntry>,
) -> FileSystemReadResult<(), E, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send,
    P: DiffPolicy,
    S: RepoStorage<D>,
{
    match join {
        OuterJoinEntry::Left(_) => {
            // file exists on head but not in file system
            // => the file has been deleted
            update_delete_file_change(storage, pending_changes, path).await?;
        }
        OuterJoinEntry::Right(MemoryFileSystemEntry { file, dirty }) => {
            // file does not exist on head but does exist in file system
            // => the file has been created

            // only update if head has changed or the file has been changed
            if head_changed || *dirty {
                update_create_file_change(storage, pending_changes, path, file).await?;
            }
        }
        OuterJoinEntry::Both(
            on_head_digest,
            MemoryFileSystemEntry {
                file: fs_file,
                dirty,
            },
        ) => {
            // file exists both on head and in file system
            // => the file may have been modified

            // only update if head has changed or the file has been changed
            if head_changed || *dirty {
                let on_head_file = <S as Storage<FileRef<D>, File>>::load(storage, on_head_digest)
                    .await
                    .map_err(FileSystemReadError::LoadError)?;
                update_modify_file_change(
                    diff_policy,
                    storage,
                    pending_changes,
                    path,
                    &on_head_file,
                    fs_file,
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn apply_change<D, E, S>(
    storage: &S,
    files: &DashMap<RepoPath, MemoryFileSystemEntry>,
    head_changed: bool,
    path: &RepoPath,
    join: OuterJoinEntry<&D, &FileChange<D>>,
) -> FileSystemWriteResult<(), E, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send + Eq,
    S: RepoStorage<D>,
{
    match join {
        OuterJoinEntry::Left(file_digest)
        | OuterJoinEntry::Right(FileChange::Create(file_digest)) => {
            // left: file exists on head, not changed in pending changes
            // right: file does not exist on head but is created in pending changes
            // both cases => insert file in file system
            let dirty = files.get(path).is_none_or(|entry| entry.dirty);
            if head_changed || dirty {
                insert_file(storage, files, path, file_digest).await?;
            }
        }
        OuterJoinEntry::Both(on_head_digest, FileChange::Modify(pending_file_diff_digest)) => {
            let dirty = files.get(path).is_none_or(|entry| entry.dirty);
            if head_changed || dirty {
                modify_file(
                    storage,
                    files,
                    path,
                    on_head_digest,
                    pending_file_diff_digest,
                )
                .await?;
            }
        }
        OuterJoinEntry::Both(_, FileChange::Delete) => {
            // file exists on head and is deleted in pending changes
            // => delete file on file system
            files.remove(path);
        }
        OuterJoinEntry::Right(_) | OuterJoinEntry::Both(_, FileChange::Create(_)) => {
            // right: file does not exist on head but is modified or deleted in pending changes
            // left and right: file exists on head and is created in pending changes
            // both cases => invalid
            return Err(FileSystemWriteError::InvalidPendingChanges);
        }
    }
    Ok(())
}

async fn insert_file<D, E, S>(
    storage: &S,
    files: &DashMap<RepoPath, MemoryFileSystemEntry>,
    path: &RepoPath,
    file_digest: &D,
) -> FileSystemWriteResult<(), E, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send + Eq,
    S: RepoStorage<D>,
{
    // if file already exists and has same hash as the file to be inserted, skip the operation
    // altogether
    if let Some(mut entry) = files.get_mut(path) {
        let current_file_digest = entry.file.to_digest();
        if *file_digest == current_file_digest {
            entry.dirty = false;
            return Ok(());
        }
    }

    let file = <S as Storage<FileRef<D>, File>>::load(storage, file_digest)
        .await
        .map_err(FileSystemWriteError::LoadError)?;

    replace_or_insert(files, path, MemoryFileSystemEntry { file, dirty: false });
    Ok(())
}

async fn modify_file<D, E, S>(
    storage: &S,
    files: &DashMap<RepoPath, MemoryFileSystemEntry>,
    path: &RepoPath,
    file_before_digest: &FileRef<D>,
    file_diff_digest: &FileDiffRef<D>,
) -> FileSystemWriteResult<(), E, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send + Eq,
    S: RepoStorage<D>,
{
    let file_before = <S as Storage<FileRef<D>, File>>::load(storage, file_before_digest)
        .await
        .map_err(FileSystemWriteError::LoadError)?;
    let file_diff = <S as Storage<FileDiffRef<D>, FileDiff>>::load(storage, file_diff_digest)
        .await
        .map_err(FileSystemWriteError::LoadError)?;

    let file_after_contents = file_diff
        .hunks
        .apply(&file_before.content)
        .map_err(FileSystemWriteError::HunkError)?;
    let file_after = File {
        content: file_after_contents,
        executable_status: file_diff.executable_status,
    };

    replace_or_insert(
        files,
        path,
        MemoryFileSystemEntry {
            file: file_after,
            dirty: false,
        },
    );
    Ok(())
}

impl Clone for MemoryFileSystem {
    fn clone(&self) -> Self {
        let files = self.files.blocking_read();
        MemoryFileSystem {
            files: RwLock::new(files.deref().clone()),
        }
    }
}

impl Default for MemoryFileSystem {
    fn default() -> Self {
        MemoryFileSystem::new()
    }
}

impl Debug for MemoryFileSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let files = self.files.blocking_read();
        f.debug_struct("MemoryFileSystem")
            .field("files", files.deref())
            .finish()
    }
}
