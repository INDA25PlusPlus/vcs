use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::diff::repo_diff::RepoDiff;
use crate::fs::file::{File, FileChange, FileDiff, FileDiffRef, FileRef};
use crate::fs::map_ops::{
    DashMapGuard, DashMapReadOnlyGuard, OuterJoinEntry, outer_join, remove_difference,
    replace_or_insert,
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
use std::fmt::Debug;
use std::ops::Deref;
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

impl FileSystem for MemoryFileSystem {
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

    async fn update_pending_changes<D, P, S>(
        &self,
        diff_policy: &P,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &mut PendingChanges<D>,
        head_changed: bool,
    ) -> FileSystemReadResult<(), Self::Error, S::RepoStorageError>
    where
        D: CryptoDigest + CryptoHash + Send + Eq,
        P: DiffPolicy,
        S: RepoStorage<D>,
    {
        let mut files = self.files.write().await;

        let PendingChanges(RepoDiff { changeset }) = pending_changes;
        let changeset_rw = DashMapGuard::new(changeset);

        {
            let files_ro = DashMapReadOnlyGuard::new(&mut files);

            // regardless of if head changed, remove all changes to files that don't exist neither on
            // head nor in the file system
            remove_difference!(changeset_rw, head.files, files_ro);

            let outer_join = outer_join(&head.files, files_ro.deref());

            let futures = outer_join.map(|(path, outer_join)| {
                update_change(
                    diff_policy,
                    storage,
                    changeset_rw.deref(),
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

    async fn apply_pending_changes<D, S>(
        &self,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &PendingChanges<D>,
        head_changed: bool,
    ) -> FileSystemWriteResult<(), Self::Error, S::RepoStorageError>
    where
        D: CryptoDigest + CryptoHash + Send + Eq,
        S: RepoStorage<D>,
    {
        let files = self.files.read().await;
        let PendingChanges(RepoDiff { changeset }) = pending_changes;

        // regardless of if head changed, delete all files that don't exist on head and are not
        // changed in pending changes
        remove_difference!(files.deref(), head.files, changeset);

        let outer_join = outer_join(&head.files, changeset);

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
    pending_changes: &DashMap<RepoPath, FileChange<D>>,
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

impl Default for MemoryFileSystem {
    fn default() -> Self {
        MemoryFileSystem::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_policy::NaiveDiff;
    use crate::fs::file::File;
    use crate::storage::memory::MemoryRepoStorage;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref DIFF_POLICY: NaiveDiff = NaiveDiff;
    }

    lazy_static! {
        static ref FILE_CONTENT_1: Box<[u8]> =
            vec![0x00, 0x01, 0x02, 0x03, 0x04,].into_boxed_slice();
        static ref FILE_CONTENT_2: Box<[u8]> =
            vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,].into_boxed_slice();
        static ref FILE_CONTENT_3: Box<[u8]> = vec![0xff, 0xfe, 0xfd,].into_boxed_slice();
    }

    lazy_static! {
        static ref FILE_1A: File = File {
            content: FILE_CONTENT_1.clone(),
            executable_status: false,
        };
        static ref FILE_1B: File = File {
            content: FILE_CONTENT_1.clone(),
            executable_status: true,
        };
        static ref FILE_2: File = File {
            content: FILE_CONTENT_2.clone(),
            executable_status: false,
        };
        static ref FILE_3: File = File {
            content: FILE_CONTENT_3.clone(),
            executable_status: false,
        };
    }

    lazy_static! {
        static ref FILE_1A_DIGEST: blake3::Hash = FILE_1A.to_digest();
        static ref FILE_1B_DIGEST: blake3::Hash = FILE_1B.to_digest();
        static ref FILE_2_DIGEST: blake3::Hash = FILE_2.to_digest();
        static ref FILE_3_DIGEST: blake3::Hash = FILE_3.to_digest();
    }

    lazy_static! {
        static ref FILE_DIFF_1_2: FileDiff = {
            let hunks = DIFF_POLICY.diff(&FILE_CONTENT_1, &FILE_CONTENT_2);
            FileDiff {
                hunks,
                executable_status: false,
            }
        };
        static ref FILE_DIFF_1_3: FileDiff = {
            let hunks = DIFF_POLICY.diff(&FILE_CONTENT_1, &FILE_CONTENT_3);
            FileDiff {
                hunks,
                executable_status: false,
            }
        };
    }

    lazy_static! {
        static ref FILE_DIFF_1_2_DIGEST: blake3::Hash = FILE_DIFF_1_2.to_digest();
        static ref FILE_DIFF_1_3_DIGEST: blake3::Hash = FILE_DIFF_1_3.to_digest();
    }

    async fn test_wrapper(
        f: impl AsyncFnOnce(
            &MemoryRepoStorage<blake3::Hash>,
            &MemoryFileSystem,
            &FileTree<blake3::Hash>,
        ),
    ) {
        async fn insert(
            storage: &impl RepoStorage<blake3::Hash>,
            fs: &MemoryFileSystem,
            head: &DashMap<RepoPath, FileRef<blake3::Hash>>,
            path: &str,
            file: &impl Deref<Target = File>,
        ) {
            let file_digest = file.to_digest();
            let path = RepoPath::try_from(path).unwrap();
            storage.store(&file_digest, file.deref()).await.unwrap();
            fs.files.read().await.insert(
                path.clone(),
                MemoryFileSystemEntry {
                    file: file.deref().clone(),
                    dirty: false,
                },
            );
            head.insert(path, file_digest);
        }
        let storage = MemoryRepoStorage::new();
        let fs = MemoryFileSystem::new();
        let head = DashMap::new();

        insert(&storage, &fs, &head, "1", &FILE_1A).await;
        insert(&storage, &fs, &head, "2", &FILE_2).await;

        let head = FileTree {
            files: head.into_read_only(),
        };
        f(&storage, &fs, &head).await;
    }

    mod update_pending {
        use super::*;

        struct ExpectedChangeset<'a> {
            path: &'a str,
            before: FileChange<blake3::Hash>,
            after: Option<FileChange<blake3::Hash>>,
        }

        async fn update_pending_test_wrapper<'a>(
            changesets: &[ExpectedChangeset<'a>],
            files: &[(&str, File, bool)],
            head_changed: bool,
        ) {
            test_wrapper(async |storage, fs, head| {
                let changeset_before = changesets.iter().map(|expected| {
                    (
                        RepoPath::try_from(expected.path).unwrap(),
                        expected.before.clone(),
                    )
                });
                let changeset_after = changesets.iter().map(|expected| {
                    (
                        RepoPath::try_from(expected.path).unwrap(),
                        expected.after.clone(),
                    )
                });
                let mut pending_changes = PendingChanges(RepoDiff {
                    changeset: changeset_before.collect::<DashMap<_, _>>().into_read_only(),
                });

                for (path, file, dirty) in files {
                    let path = RepoPath::try_from(*path).unwrap();
                    fs.files.read().await.insert(
                        path,
                        MemoryFileSystemEntry {
                            file: file.clone(),
                            dirty: *dirty,
                        },
                    );
                }

                fs.update_pending_changes(
                    DIFF_POLICY.deref(),
                    storage,
                    head,
                    &mut pending_changes,
                    head_changed,
                )
                .await
                .unwrap();

                for (path, expected_change) in changeset_after {
                    if let Some(expected_change) = expected_change {
                        let actual_change = pending_changes.0.changeset.get(&path).unwrap();
                        assert_eq!(actual_change, &expected_change);
                    } else {
                        assert!(!pending_changes.0.changeset.contains_key(&path));
                    }
                }
            })
            .await;
        }

        #[tokio::test]
        async fn no_changes() {
            // when head_changed = true, expect pending change to be None as head = file system
            update_pending_test_wrapper(&[], &[], true).await;
            update_pending_test_wrapper(
                &[ExpectedChangeset {
                    path: "1",
                    before: FileChange::Create(*FILE_1A_DIGEST),
                    after: None,
                }],
                &[],
                true,
            )
            .await;
            update_pending_test_wrapper(
                &[ExpectedChangeset {
                    path: "1",
                    before: FileChange::Modify(*FILE_DIFF_1_2_DIGEST),
                    after: None,
                }],
                &[],
                true,
            )
            .await;
            update_pending_test_wrapper(
                &[ExpectedChangeset {
                    path: "1",
                    before: FileChange::Delete,
                    after: None,
                }],
                &[],
                true,
            )
            .await;

            // when head_changed = false, expect pending change to not have been updated as
            // dirty = false
            update_pending_test_wrapper(&[], &[], false).await;
            update_pending_test_wrapper(
                &[ExpectedChangeset {
                    path: "1",
                    before: FileChange::Create(*FILE_1A_DIGEST),
                    after: Some(FileChange::Create(*FILE_1A_DIGEST)),
                }],
                &[],
                false,
            )
            .await;
            update_pending_test_wrapper(
                &[ExpectedChangeset {
                    path: "1",
                    before: FileChange::Modify(*FILE_DIFF_1_2_DIGEST),
                    after: Some(FileChange::Modify(*FILE_DIFF_1_2_DIGEST)),
                }],
                &[],
                false,
            )
            .await;
            update_pending_test_wrapper(
                &[ExpectedChangeset {
                    path: "1",
                    before: FileChange::Delete,
                    after: Some(FileChange::Delete),
                }],
                &[],
                false,
            )
            .await;
        }

        #[tokio::test]
        async fn change_executable_status() {}
    }
}
