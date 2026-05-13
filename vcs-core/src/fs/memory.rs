use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::fs::file::{File, FileChange, FileDiff, FileDiffRef, FileRef};
use crate::fs::map_ops::{
    DashMapReadOnlyGuard, OuterJoinEntry, outer_join, remove_difference, replace_or_insert,
};
use crate::fs::path::RepoPath;
use crate::fs::{
    FileSystem, FileSystemError, FileSystemReadError, FileSystemReadResult, FileSystemResult,
    FileSystemWriteError, FileSystemWriteResult, FileTree,
};
use crate::repo::PendingChanges;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::Storage;
use dashmap::DashMap;
use std::convert::Infallible;
use std::ops::{Deref, DerefMut};
use tokio::sync::RwLock;

pub struct MemoryFileSystem {
    files: RwLock<DashMap<RepoPath, MemoryFileSystemEntry>>,
}

struct MemoryFileSystemEntry {
    file: File,
    dirty: bool,
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

    async fn read_pending_changes<P, S>(
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
        let mut files_guard = self.files.write().await;
        let files = DashMapReadOnlyGuard::new(files_guard.deref_mut());
        let pending_changes = &pending_changes.0.file_diffs;

        // regardless of if head changed, remove all changes to files that don't exist neither on
        // head nor in the file system
        remove_difference!(pending_changes, head.files, files);

        for (path, join_entry) in outer_join(&head.files, files.deref()) {
            match join_entry {
                OuterJoinEntry::Left(_) => {
                    // file exists on head but not in file system
                    // => the file has been deleted
                    replace_or_insert(pending_changes, path, FileChange::Delete);
                }
                OuterJoinEntry::Right(MemoryFileSystemEntry { file, dirty }) => {
                    // file does not exist on head but does exist in file system
                    // => the file has been created

                    // only update if head has changed or the file has been changed
                    if head_changed || *dirty {
                        let file_digest = D::generate(file);
                        storage
                            .store(&file_digest, file)
                            .await
                            .map_err(FileSystemReadError::StoreError)?;
                        replace_or_insert(pending_changes, path, FileChange::Create(file_digest));
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
                        let on_head_file =
                            <S as Storage<FileRef<D>, File>>::load(storage, on_head_digest)
                                .await
                                .map_err(FileSystemReadError::LoadError)?;

                        if on_head_file == *fs_file {
                            pending_changes.remove(path);
                        } else {
                            let hunks = diff_policy.diff(&on_head_file.content, &fs_file.content);
                            let file_diff = FileDiff {
                                hunks,
                                executable_status: fs_file.executable_status,
                            };
                            let file_diff_digest = D::generate(&file_diff);
                            storage
                                .store(&file_diff_digest, &file_diff)
                                .await
                                .map_err(FileSystemReadError::StoreError)?;

                            replace_or_insert(
                                pending_changes,
                                path,
                                FileChange::Modify(file_diff_digest),
                            );
                        }
                    }
                }
            }
        }
        // set all non-dirty
        drop(files);
        files_guard
            .iter_mut()
            .for_each(|mut entry| entry.dirty = false);
        Ok(())
    }

    async fn write_pending_changes<S>(
        &self,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &mut PendingChanges<D>,
        head_changed: bool,
    ) -> FileSystemWriteResult<(), Self::Error, S::RepoStorageError>
    where
        S: RepoStorage<D>,
    {
        let mut files = self.files.write().await;
        let pending_changes = DashMapReadOnlyGuard::new(&mut pending_changes.0.file_diffs);

        // regardless of if head changed, delete all files that don't exist on head and are not
        // changed in pending changes
        remove_difference!(files.deref_mut(), head.files, pending_changes);

        for (path, join_entry) in outer_join(&head.files, pending_changes.deref()) {
            match join_entry {
                OuterJoinEntry::Left(on_head_digest) => {
                    let dirty = files.get(path).is_none_or(|entry| entry.dirty);
                    if head_changed || dirty {
                        if let Some(mut entry) = files.get_mut(path) {
                            let fs_digest = D::generate(&entry.file);
                            if *on_head_digest == fs_digest {
                                entry.dirty = false;
                                return Ok(());
                            }
                        }
                        let on_head_file =
                            <S as Storage<FileRef<D>, File>>::load(storage, on_head_digest)
                                .await
                                .map_err(FileSystemWriteError::LoadError)?;
                        replace_or_insert(
                            files.deref_mut(),
                            path,
                            MemoryFileSystemEntry {
                                file: on_head_file,
                                dirty: false,
                            },
                        );
                    }
                }
                OuterJoinEntry::Right(FileChange::Create(pending_file_digest)) => {
                    let dirty = files.get(path).is_none_or(|entry| entry.dirty);
                    if head_changed || dirty {
                        if let Some(mut entry) = files.get_mut(path) {
                            let fs_digest = D::generate(&entry.file);
                            if *pending_file_digest == fs_digest {
                                entry.dirty = false;
                                return Ok(());
                            }
                        }
                        let pending_file =
                            <S as Storage<FileRef<D>, File>>::load(storage, pending_file_digest)
                                .await
                                .map_err(FileSystemWriteError::LoadError)?;
                        replace_or_insert(
                            files.deref_mut(),
                            path,
                            MemoryFileSystemEntry {
                                file: pending_file,
                                dirty: false,
                            },
                        );
                    }
                }
                OuterJoinEntry::Right(_) => {
                    return Err(FileSystemWriteError::InvalidPendingChanges);
                }
                OuterJoinEntry::Both(
                    on_head_digest,
                    FileChange::Modify(pending_file_diff_digest),
                ) => {
                    let dirty = files.get(path).is_none_or(|entry| entry.dirty);
                    if head_changed || dirty {
                        let on_head_file =
                            <S as Storage<FileRef<D>, File>>::load(storage, on_head_digest)
                                .await
                                .map_err(FileSystemWriteError::LoadError)?;
                        let pending_file_diff = <S as Storage<FileDiffRef<D>, FileDiff>>::load(
                            storage,
                            pending_file_diff_digest,
                        )
                        .await
                        .map_err(FileSystemWriteError::LoadError)?;

                        let file_contents = pending_file_diff
                            .hunks
                            .apply(&on_head_file.content)
                            .map_err(FileSystemWriteError::HunkError)?;
                        let file = File {
                            content: file_contents,
                            executable_status: pending_file_diff.executable_status,
                        };
                        replace_or_insert(
                            files.deref_mut(),
                            path,
                            MemoryFileSystemEntry { file, dirty: false },
                        );
                    }
                }
                OuterJoinEntry::Both(_, FileChange::Delete) => {
                    files.remove(path);
                }
                OuterJoinEntry::Both(_, _) => {
                    return Err(FileSystemWriteError::InvalidPendingChanges);
                }
            }
        }
        Ok(())
    }
}
