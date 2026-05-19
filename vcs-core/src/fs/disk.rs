use crate::changeset::Changeset;
use crate::changeset::file::{File, FileChange, FileDiff, FileDiffRef, FileRef};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::fs::map_ops::{
    DashMapGuard, DashMapReadOnlyGuard, OuterJoinEntry, outer_join, remove_difference,
};
use crate::fs::path::{RepoPath, RepoPathComponent, RepoPathError};
use crate::fs::{
    FileSystem, FileSystemError, FileSystemReadError, FileSystemReadResult, FileSystemWriteError,
    FileSystemWriteResult, FileTree, update_create_file_change, update_delete_file_change,
    update_modify_file_change,
};
use crate::repo::PendingChanges;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::Storage;
use cfg_if::cfg_if;
use dashmap::{DashMap, ReadOnlyView};
use futures::future::try_join_all;
use std::ffi::{OsStr, OsString};
use std::fs::{FileType, Metadata};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::try_join;

pub struct DiskFileSystem {
    base_path: Box<Path>,
    ignored_root_entries: Box<[OsString]>,
    cache_times: DashMap<RepoPath, SystemTime>,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    IoError(std::io::Error),
    #[error("invalid repository path")]
    InvalidPath(RepoPathError),
    #[error("unsupported file type: {0:?}")]
    InvalidFileType(FileType),
}

impl DiskFileSystem {
    pub fn new(base_path: Box<Path>) -> Self {
        Self {
            base_path,
            ignored_root_entries: Box::new([]),
            cache_times: DashMap::new(),
        }
    }

    pub fn with_ignored_root_entries<I, S>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.ignored_root_entries = entries
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self
    }

    fn ignores_root_entry(&self, repo_path: &RepoPath, entry_name: &OsStr) -> bool {
        repo_path.components().is_empty()
            && self
                .ignored_root_entries
                .iter()
                .any(|ignored| ignored.as_os_str() == entry_name)
    }

    fn is_dirty(metadata: Option<&Metadata>, cache_time: Option<&SystemTime>) -> Result<bool> {
        Ok(match (metadata, cache_time) {
            // file does not exist on file => dirty
            (None, _) => true,
            // file does exist on file but has not been cached => dirty
            (Some(_), None) => true,
            // file has been modified since last cache => dirty
            // file has not been modified since last cache => not dirty
            (Some(metadata), Some(cache_time)) => &metadata.modified()? > cache_time,
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

    async fn write_file_cached_path(
        cached_path: Option<&Path>,
        base_path: &Path,
        repo_path: &RepoPath,
        file: &File,
    ) -> Result<()> {
        match cached_path {
            None => {
                // file does not exist in cache
                let mut fs_path: PathBuf = base_path.into();
                repo_path
                    .append_to(&mut fs_path)
                    .map_err(Error::InvalidPath)?;
                DiskFileSystem::write_file(&fs_path, file).await?;
            }
            Some(fs_path) => {
                // file does exist in cache
                DiskFileSystem::write_file(fs_path, file).await?;
            }
        }
        Ok(())
    }

    async fn write_file(path: &Path, file: &File) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, &file.content).await?;

        cfg_if! {
            if #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = tokio::fs::metadata(path).await?.permissions();
                if file.executable_status {
                    perms.set_mode(perms.mode() | 0o111);
                } else {
                    perms.set_mode(perms.mode() & 0o666);
                }
                tokio::fs::set_permissions(path, perms).await?;
            }
        }
        Ok(())
    }

    async fn delete_file(path: &Path) -> Result<()> {
        Ok(tokio::fs::remove_file(path).await?)
    }

    async fn index_files_recurse(
        &self,
        fs_path: &Path,
        repo_path: &RepoPath,
    ) -> Result<Vec<(RepoPath, FileIndexEntry)>> {
        let mut dir_stream = tokio::fs::read_dir(&fs_path).await?;
        let mut entries = vec![];
        while let Some(entry) = dir_stream.next_entry().await? {
            entries.push(entry);
        }

        let indexes = entries.iter().map(|entry| async {
            let entry_name = entry.file_name();
            if self.ignores_root_entry(repo_path, &entry_name) {
                return Ok(vec![]);
            }

            let (entry_type, metadata) = try_join!(entry.file_type(), entry.metadata())?;

            let repo_path_component =
                RepoPathComponent::try_from(entry_name.as_os_str()).map_err(Error::InvalidPath)?;

            let fs_path = fs_path.join(entry_name);
            let repo_path = repo_path.join(repo_path_component);
            if entry_type.is_dir() {
                self.index_files_recurse(&fs_path, &repo_path).await
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
        let index: DashMap<_, _> = self
            .index_files_recurse(fs_path, &RepoPath::new())
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

        let PendingChanges(Changeset { changeset }) = pending_changes;
        let changeset_rw = DashMapGuard::new(changeset);

        let now = SystemTime::now();

        {
            let cache_times_ro = DashMapReadOnlyGuard::new(&mut self.cache_times);

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
        self.cache_times
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
        let index = self.index_files(&self.base_path).await.map_err(|err| {
            FileSystemWriteError::FileSystemError(FileSystemError::InternalError(err))
        })?;

        let PendingChanges(Changeset { changeset }) = pending_changes;

        // regardless of if head changed, delete all files that don't exist on head and are not
        // changed in pending changes
        let remove_difference_futures = index.iter().filter_map(|(path, index_entry)| {
            let should_delete = !head.files.contains_key(path) && !changeset.contains_key(path);
            should_delete.then_some(async {
                DiskFileSystem::delete_file(&index_entry.fs_path)
                    .await
                    .map_err(|err| {
                        FileSystemWriteError::FileSystemError(FileSystemError::InternalError(err))
                    })
            })
        });

        let outer_join = outer_join(&head.files, changeset);

        let now = SystemTime::now();
        let apply_futures = outer_join.map(|(path, outer_join)| async {
            let cache_time = self.cache_times.get(path).map(|cache_time| *cache_time);

            apply_change(
                &self.base_path,
                storage,
                index.get(path),
                cache_time,
                head_changed,
                path,
                outer_join,
            )
            .await?;

            self.cache_times.insert(path.clone(), now);
            Ok(())
        });

        // futures can be safely run concurrently as they operate on disjoint sets of files
        try_join!(
            try_join_all(remove_difference_futures),
            try_join_all(apply_futures)
        )?;

        Ok(())
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
            let dirty = DiskFileSystem::is_dirty(Some(metadata), cache_time).map_err(|err| {
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
            let dirty = DiskFileSystem::is_dirty(Some(metadata), cache_time).map_err(|err| {
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

async fn apply_change<D, S>(
    base_path: &Path,
    storage: &S,
    index_entry: Option<&FileIndexEntry>,
    cache_time: Option<SystemTime>,
    head_changed: bool,
    path: &RepoPath,
    join: OuterJoinEntry<&D, &FileChange<D>>,
) -> FileSystemWriteResult<(), Error, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send + Eq,
    S: RepoStorage<D>,
{
    let dirty = DiskFileSystem::is_dirty(
        index_entry.map(|entry| &entry.metadata),
        cache_time.as_ref(),
    )
    .map_err(|err| FileSystemWriteError::FileSystemError(FileSystemError::InternalError(err)))?;
    match join {
        OuterJoinEntry::Left(file_digest)
        | OuterJoinEntry::Right(FileChange::Create(file_digest)) => {
            // left: file exists on head, not changed in pending changes
            // right: file does not exist on head but is created in pending changes
            // both cases => insert file in file system
            if head_changed || dirty {
                insert_file(base_path, storage, index_entry, path, file_digest).await?;
            }
        }
        OuterJoinEntry::Both(on_head_digest, FileChange::Modify(pending_file_diff_digest)) => {
            if head_changed || dirty {
                modify_file(
                    base_path,
                    storage,
                    index_entry,
                    path,
                    on_head_digest,
                    pending_file_diff_digest,
                )
                .await?;
            }
        }
        OuterJoinEntry::Both(_, FileChange::Delete) => {
            // file exists on head and is deleted in pending changes
            // => delete file on file system if present
            if let Some(index_entry) = index_entry {
                DiskFileSystem::delete_file(&index_entry.fs_path)
                    .await
                    .map_err(|err| {
                        FileSystemWriteError::FileSystemError(FileSystemError::InternalError(err))
                    })?;
            }
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

async fn insert_file<D, S>(
    base_path: &Path,
    storage: &S,
    index_entry: Option<&FileIndexEntry>,
    path: &RepoPath,
    file_digest: &D,
) -> FileSystemWriteResult<(), Error, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send + Eq,
    S: RepoStorage<D>,
{
    let file = <S as Storage<FileRef<D>, File>>::load(storage, file_digest)
        .await
        .map_err(FileSystemWriteError::LoadError)?;

    DiskFileSystem::write_file_cached_path(
        index_entry.map(|entry| entry.fs_path.as_ref()),
        base_path,
        path,
        &file,
    )
    .await
    .map_err(|err| FileSystemWriteError::FileSystemError(FileSystemError::InternalError(err)))?;
    Ok(())
}

async fn modify_file<D, S>(
    base_path: &Path,
    storage: &S,
    index_entry: Option<&FileIndexEntry>,
    path: &RepoPath,
    file_before_digest: &FileRef<D>,
    file_diff_digest: &FileDiffRef<D>,
) -> FileSystemWriteResult<(), Error, S::RepoStorageError>
where
    D: CryptoDigest + CryptoHash + Send + Eq,
    S: RepoStorage<D>,
{
    let (file_before, file_diff) = try_join!(
        <S as Storage<FileRef<D>, File>>::load(storage, file_before_digest),
        <S as Storage<FileDiffRef<D>, FileDiff>>::load(storage, file_diff_digest)
    )
    .map_err(FileSystemWriteError::LoadError)?;

    let file_after_contents = file_diff
        .hunks
        .apply(&file_before.content)
        .map_err(FileSystemWriteError::HunkError)?;
    let file_after = File {
        content: file_after_contents,
        executable_status: file_diff.executable_status,
    };

    DiskFileSystem::write_file_cached_path(
        index_entry.map(|entry| entry.fs_path.as_ref()),
        base_path,
        path,
        &file_after,
    )
    .await
    .map_err(|err| FileSystemWriteError::FileSystemError(FileSystemError::InternalError(err)))?;
    Ok(())
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::IoError(value)
    }
}
