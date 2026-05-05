use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::fs::file::{File, FileChange, FileDiff};
use crate::fs::map_ops::{OuterJoinEntry, outer_join, remove_difference, replace_or_insert};
use crate::fs::path::RepoPath;
use crate::fs::{
    FileSystem, FileSystemError, FileSystemResult, FileSystemStorageError, FileSystemStorageResult,
    FileTree,
};
use crate::repo::PendingChanges;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::Storage;
use std::collections::HashMap;
use std::convert::Infallible;
use std::ops::Deref;
use tokio::sync::RwLock;

pub struct MemoryFileSystem {
    files: RwLock<HashMap<RepoPath, MemoryFileSystemEntry>>,
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
        self.files.write().await.insert(
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
            .write()
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
        pending_changes: &mut PendingChanges<D>,
        head_changed: bool,
    ) -> FileSystemStorageResult<(), Self::Error, S::RepoStorageError>
    where
        P: DiffPolicy,
        S: RepoStorage<D>,
    {
        let files = self.files.read().await;
        let pending_changes = &mut pending_changes.0.file_diffs;

        // regardless of if head changed, remove all changes to files that don't exist neither on
        // head or in the file system
        remove_difference!(pending_changes, files, head.files);

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
                            .map_err(FileSystemStorageError::StoreError)?;
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
                        let on_head_file: File =
                            <S as Storage<D, File>>::load(storage, on_head_digest)
                                .await
                                .map_err(FileSystemStorageError::LoadError)?;

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
                                .map_err(FileSystemStorageError::StoreError)?;

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
        Ok(())
    }

    async fn write_pending_changes<P, S>(
        &self,
        diff_policy: &P,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &PendingChanges<D>,
        head_changed: bool,
    ) -> FileSystemStorageResult<(), Self::Error, S::RepoStorageError>
    where
        P: DiffPolicy,
        S: RepoStorage<D>,
    {
        todo!()
    }
}
