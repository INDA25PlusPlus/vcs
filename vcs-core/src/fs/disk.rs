use crate::changeset::Changeset;
use crate::changeset::file::{File, FileChange, FileRef};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::fs::map_ops::{
    DashMapGuard, DashMapReadOnlyGuard, OuterJoinEntry, outer_join, remove_difference,
};
use crate::fs::path::{RepoPath, RepoPathComponent, RepoPathError};
use crate::fs::{
    FileSystem, FileSystemError, FileSystemReadError, FileSystemReadResult, FileSystemWriteResult,
    FileTree, update_create_file_change, update_delete_file_change, update_modify_file_change,
};
use crate::repo::PendingChanges;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::Storage;
use cfg_if::cfg_if;
use dashmap::{DashMap, ReadOnlyView};
use futures::future::try_join_all;
use std::fs::{FileType, Metadata};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::sync::Mutex;
use tokio::try_join;

pub const IGNORED_PATH: &str = ".vcs";

pub struct DiskFileSystem {
    base_path: Box<Path>,
    cache_times: Mutex<DashMap<RepoPath, SystemTime>>,
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum Error {
    IoError(std::io::Error),
    InvalidPath(RepoPathError),
    InvalidFileType(FileType),
}

impl DiskFileSystem {
    fn is_dirty(metadata: &Metadata, cache_time: Option<&SystemTime>) -> Result<bool> {
        Ok(match cache_time {
            None => true,
            Some(cache_time) => &metadata.modified()? > cache_time,
        })
    }

    fn new_executable_status(cached_executable_status: Option<bool>, metadata: &Metadata) -> bool {
        cfg_if! {
            if #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                const S_IXUSR: u32 = 0o100;

                // ignore cached executable status, always fetch from file's metadata
                let _ = cached_executable_status;

                let mode = metadata.permissions().mode();
                mode & S_IXUSR > 0
            } else {
                // leave executable status unchanged, or default to non-executable
                let _ = metadata;
                cached_executable_status.unwrap_or(false)
            }
        }
    }

    async fn read_file(path: &Path, metadata: &Metadata) -> Result<File> {
        let content = tokio::fs::read(path).await?.into_boxed_slice();
        Ok(File {
            content,
            executable_status: DiskFileSystem::new_executable_status(None, metadata),
        })
    }

    async fn read_metadata(path: &Path) -> Result<Metadata> {
        Ok(tokio::fs::metadata(path).await?)
    }

    async fn index_files_recurse(
        fs_path: &Path,
        repo_path: &RepoPath,
    ) -> Result<Vec<(RepoPath, FileIndexEntry)>> {
        let mut dir_stream = tokio::fs::read_dir(&fs_path).await?;
        let mut entries = vec![];
        while let Some(entry) = dir_stream.next_entry().await? {
            entries.push(entry);
        }

        let indexes = entries.iter().map(|entry| async {
            let (entry_type, metadata) = try_join!(entry.file_type(), entry.metadata())?;

            let entry_name = entry.file_name();
            let repo_path_component =
                RepoPathComponent::try_from(entry_name.as_os_str()).map_err(Error::InvalidPath)?;

            let fs_path = fs_path.join(entry_name);
            let repo_path = repo_path.join(repo_path_component);
            if entry_type.is_dir() {
                DiskFileSystem::index_files_recurse(&fs_path, &repo_path).await
            } else if entry_type.is_file() {
                Ok(vec![(repo_path, FileIndexEntry { fs_path, metadata })])
            } else {
                // file is symlink or some other unsupported file
                Err(Error::InvalidFileType(entry_type))
            }
        });
        let indexes = try_join_all(indexes).await;
        indexes.map(|indexes| indexes.into_iter().flatten().collect())
    }

    async fn index_files(&self, fs_path: &Path) -> Result<ReadOnlyView<RepoPath, FileIndexEntry>> {
        let index: DashMap<_, _> = DiskFileSystem::index_files_recurse(fs_path, &RepoPath::new())
            .await?
            .into_iter()
            .collect();
        Ok(index.into_read_only())
    }
}

struct FileIndexEntry {
    fs_path: PathBuf,
    metadata: Metadata,
}

impl FileSystem for DiskFileSystem {
    type Error = Error;

    async fn update_pending_changes<D, P, S>(
        &mut self,
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
        let index = self.index_files(&self.base_path).await.map_err(|err| {
            FileSystemReadError::FileSystemError(FileSystemError::InternalError(err))
        })?;

        let mut cache_times = self.cache_times.lock().await;

        let PendingChanges(Changeset { changeset }) = pending_changes;
        let changeset_rw = DashMapGuard::new(changeset);

        let now = SystemTime::now();

        {
            let cache_times_ro = DashMapReadOnlyGuard::new(&mut cache_times);

            // regardless of if head changed, remove all changes to files that don't exist neither on
            // head nor in the file system
            remove_difference(changeset_rw.deref(), head.read_only_view(), &index);

            let outer_join = outer_join(head.read_only_view(), &index);

            let futures = outer_join.map(|(path, outer_join)| {
                update_change(
                    diff_policy,
                    storage,
                    changeset_rw.deref(),
                    head_changed,
                    path,
                    outer_join,
                    cache_times_ro.get(path),
                )
            });
            try_join_all(futures).await?;
        }
        // set all non-dirty
        cache_times
            .iter_mut()
            .for_each(|mut cache_time| *cache_time = now);
        Ok(())
    }

    async fn apply_pending_changes<D, S>(
        &mut self,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &PendingChanges<D>,
        head_changed: bool,
    ) -> FileSystemWriteResult<(), Self::Error, S::RepoStorageError>
    where
        D: CryptoDigest + CryptoHash + Send + Eq,
        S: RepoStorage<D>,
    {
        todo!()
    }
}

async fn update_change<D, P, S>(
    diff_policy: &P,
    storage: &S,
    pending_changes: &DashMap<RepoPath, FileChange<D>>,
    head_changed: bool,
    path: &RepoPath,
    join: OuterJoinEntry<&FileRef<D>, &FileIndexEntry>,
    cache_time: Option<&SystemTime>,
) -> FileSystemReadResult<(), Error, S::RepoStorageError>
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
        OuterJoinEntry::Right(FileIndexEntry { fs_path, metadata }) => {
            // file does not exist on head but does exist in file system
            // => the file has been created

            // only update if head has changed or the file has been changed
            let dirty = DiskFileSystem::is_dirty(metadata, cache_time).map_err(|err| {
                FileSystemReadError::FileSystemError(FileSystemError::InternalError(err))
            })?;
            if head_changed || dirty {
                let file = DiskFileSystem::read_file(fs_path, metadata)
                    .await
                    .map_err(|err| {
                        FileSystemReadError::FileSystemError(FileSystemError::InternalError(err))
                    })?;
                update_create_file_change(storage, pending_changes, path, &file).await?;
            }
        }
        OuterJoinEntry::Both(on_head_digest, FileIndexEntry { fs_path, metadata }) => {
            // file exists both on head and in file system
            // => the file may have been modified

            // only update if head has changed or the file has been changed
            let dirty = DiskFileSystem::is_dirty(metadata, cache_time).map_err(|err| {
                FileSystemReadError::FileSystemError(FileSystemError::InternalError(err))
            })?;
            if head_changed || dirty {
                let on_head_file = <S as Storage<FileRef<D>, File>>::load(storage, on_head_digest)
                    .await
                    .map_err(FileSystemReadError::LoadError)?;
                let file = DiskFileSystem::read_file(fs_path, metadata)
                    .await
                    .map_err(|err| {
                        FileSystemReadError::FileSystemError(FileSystemError::InternalError(err))
                    })?;
                update_modify_file_change(
                    diff_policy,
                    storage,
                    pending_changes,
                    path,
                    &on_head_file,
                    &file,
                )
                .await?;
            }
        }
    }
    Ok(())
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::IoError(value)
    }
}
