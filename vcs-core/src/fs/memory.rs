use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::fs::file::File;
use crate::fs::path::RepoPath;
use crate::fs::{FileSystem, FileSystemError, FileSystemResult, FileTree};
use crate::repo::PendingChanges;
use crate::storage::Storage;
use std::collections::HashMap;
use std::convert::Infallible;
use tokio::sync::RwLock;

pub struct MemoryFileStorage {
    files: RwLock<HashMap<RepoPath, MemoryFileStorageEntry>>,
}

struct MemoryFileStorageEntry {
    file: File,
    dirty: bool,
}

impl<D: CryptoDigest + CryptoHash> FileSystem<D> for MemoryFileStorage {
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
            MemoryFileStorageEntry {
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

    async fn read_pending_changes<S: Storage<D, File>>(
        &self,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &mut PendingChanges<D>,
        head_changed: bool,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    async fn write_pending_changes<S: Storage<D, File>>(
        &self,
        storage: &S,
        head: &FileTree<D>,
        pending_changes: &PendingChanges<D>,
        head_changed: bool,
    ) -> Result<(), Self::Error> {
        todo!()
    }
}
