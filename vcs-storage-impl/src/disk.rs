use serde::{Deserialize, Serialize};
use std::path::Path;

use vcs_core::crypto::digest::{CryptoDigest, CryptoHash};
use vcs_core::diff::repo_diff::{RepoDiff, RepoDiffRef};
use vcs_core::fs::file::{FileDiff, FileDiffRef};
use vcs_core::repo::{
    PendingChanges,
    StagedChanges,
};
use vcs_core::revision::{
    RevisionHeader,
    RevisionId,
    RevisionMetadata,
};
use vcs_core::storage::{Storage, StorageError, StorageResult};
use std::error::Error;

pub struct DiskStorage {
    pub base_path: Box<Path>,
}

impl DiskStorage {
    pub fn new(base_path: Box<Path>) -> Self {
        Self {
            base_path,
        }
    }
}

#[derive(Debug)]
pub enum DiskStorageError {
    Io(std::io::Error),
    Serialization,
    Deserialization,
}

impl From<std::io::Error> for DiskStorageError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl<K, V> Storage<K, V> for DiskStorage
where
    K: CryptoDigest,
    V: Serialize + for<'de> Deserialize<'de> + DiskStorable,
{
    type Error = DiskStorageError;

    async fn load(&self, key: &K) -> StorageResult<V, Self::Error> {
        // make path from key
        let filename = hex::encode(key.bytes());

        let mut path = self.base_path.to_path_buf();
        path.push(V::OBJECT_PATH);
        path.push(filename);

        // read bytes
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| StorageError::InternalError(DiskStorageError::Io(e)))?;

        // deserialize
        let value = postcard::from_bytes::<V>(&bytes)
            .map_err(|_| StorageError::InternalError(DiskStorageError::Deserialization))?;

        Ok(value)
    }

    async fn store(&self, key: &K, value: &V) -> Result<(), Self::Error> {
        // make path from key
        let filename = hex::encode(key.bytes());

        let mut dir = self.base_path.to_path_buf();
        dir.push(V::OBJECT_PATH);

        let mut path = dir.clone();
        path.push(filename);

        // serialize
        let bytes = postcard::to_allocvec(value).map_err(|_| DiskStorageError::Serialization)?;

        // ensure directory exists
        tokio::fs::create_dir_all(&dir).await?;

        // write file
        tokio::fs::write(path, bytes).await?;

        Ok(())
    }

    async fn delete(&self, key: &K) -> Result<(), Self::Error> {
        // make path from key
        let filename = hex::encode(key.bytes());

        let mut path = self.base_path.to_path_buf();
        path.push(V::OBJECT_PATH);
        path.push(filename);

        // delete file
        tokio::fs::remove_file(path).await?;

        Ok(())
    }
}

pub trait DiskStorable {
    const OBJECT_PATH: &'static str;
}

pub trait RepoStorage<D: CryptoDigest + CryptoHash>:
    Storage<(), RevisionId<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, RevisionHeader<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, RevisionMetadata<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, PendingChanges<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, StagedChanges<D>, Error = Self::RepoStorageError>
    + Storage<RepoDiffRef<D>, RepoDiff<D>, Error = Self::RepoStorageError>
    + Storage<FileDiffRef<D>, FileDiff, Error = Self::RepoStorageError>
    + Send
    + Sync
where
    D: Send,
{
    type RepoStorageError: Error + Send;
}

impl<D: CryptoDigest + CryptoHash> DiskStorable for RevisionMetadata<D> {
    const OBJECT_PATH: &'static str = "revision_metadata";
}
