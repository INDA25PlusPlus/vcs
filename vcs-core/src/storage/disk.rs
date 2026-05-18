use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::repo_diff::RepoDiff;
use crate::fs::file::{File, FileDiff};
use crate::repo::repo_storage::RepoStorage;
use crate::repo::{Head, PendingChanges, StagedChanges};
use crate::revision::{RevisionHeader, RevisionMetadata};
use crate::storage::{Storage, StorageError, StorageResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

pub struct DiskStorage {
    pub base_path: Box<Path>,
}

impl DiskStorage {
    pub fn new(base_path: Box<Path>) -> Self {
        Self { base_path }
    }
}

#[derive(Debug, Error)]
pub enum DiskStorageError {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("serialization error")]
    Serialization,
    #[error("deserialization error")]
    Deserialization,
}

impl From<std::io::Error> for DiskStorageError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl<K, V> Storage<K, V> for DiskStorage
where
    K: DiskStorageKey,
    V: Serialize + for<'de> Deserialize<'de> + DiskStorable,
{
    type Error = DiskStorageError;

    async fn load(&self, key: &K) -> StorageResult<V, Self::Error> {
        // make path from key
        let filename = key.to_file_name();

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
        let filename = key.to_file_name();

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
        let filename = key.to_file_name();

        let mut path = self.base_path.to_path_buf();
        path.push(V::OBJECT_PATH);
        path.push(filename);

        // delete file
        tokio::fs::remove_file(path).await?;

        Ok(())
    }
}

pub trait DiskStorageKey {
    fn to_file_name(&self) -> impl AsRef<Path>;
}

impl DiskStorageKey for blake3::Hash {
    fn to_file_name(&self) -> impl AsRef<Path> {
        hex::encode(self.bytes())
    }
}

impl DiskStorageKey for () {
    fn to_file_name(&self) -> impl AsRef<Path> {
        "0"
    }
}

pub trait DiskStorable {
    const OBJECT_PATH: &'static str;
}

impl<D: CryptoDigest + CryptoHash> RepoStorage<D> for DiskStorage
where
    D: DiskStorageKey + Serialize + for<'de> Deserialize<'de> + Send,
{
    type RepoStorageError = DiskStorageError;
}

macro_rules! impl_disk_storable {
    { impl $ty:ident = $path:expr; } => {
        impl DiskStorable for $ty {
            const OBJECT_PATH: &'static str = $path;
        }
    };
    { impl $ty:ident<D> = $path:expr; } => {
        impl<D: CryptoDigest + CryptoHash> DiskStorable for $ty<D> {
            const OBJECT_PATH: &'static str = $path;
        }
    };
}

impl_disk_storable! { impl Head<D> = "head"; }
impl_disk_storable! { impl RevisionHeader<D> = "rev_header"; }
impl_disk_storable! { impl RevisionMetadata<D> = "rev_meta"; }
impl_disk_storable! { impl PendingChanges<D> = "pending"; }
impl_disk_storable! { impl StagedChanges<D> = "staged"; }
impl_disk_storable! { impl RepoDiff<D> = "repo_diff"; }
impl_disk_storable! { impl File = "file_content"; }
impl_disk_storable! { impl FileDiff = "file_diff"; }
