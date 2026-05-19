use crate::changeset::Changeset;
use crate::changeset::file::{File, FileDiff};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::repo::repo_storage::RepoStorage;
use crate::repo::{Head, PendingChanges, StagedChanges};
use crate::revision::{RevisionHeader, RevisionMetadata};
use crate::storage::{Storage, StorageError, StorageResult};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
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
    #[error("invalid storage key")]
    InvalidKey,
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
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Err(StorageError::MissingObject);
            }
            Err(err) => return Err(StorageError::InternalError(DiskStorageError::Io(err))),
        };

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

    async fn dump(&self) -> Result<Vec<(K, V)>, Self::Error> {
        let mut dir = self.base_path.to_path_buf();
        dir.push(V::OBJECT_PATH);

        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(DiskStorageError::Io(err)),
        };

        let mut values = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                continue;
            }

            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                return Err(DiskStorageError::InvalidKey);
            };
            let Some(key) = K::from_file_name(file_name) else {
                return Err(DiskStorageError::InvalidKey);
            };

            let bytes = tokio::fs::read(entry.path()).await?;
            let value =
                postcard::from_bytes(&bytes).map_err(|_| DiskStorageError::Deserialization)?;
            values.push((key, value));
        }

        Ok(values)
    }
}

pub trait DiskStorageKey: Sized {
    fn to_file_name(&self) -> impl AsRef<Path>;
    fn from_file_name(file_name: &str) -> Option<Self>;
}

impl DiskStorageKey for blake3::Hash {
    fn to_file_name(&self) -> impl AsRef<Path> {
        hex::encode(self.bytes())
    }

    fn from_file_name(file_name: &str) -> Option<Self> {
        let mut bytes = [0; blake3::OUT_LEN];
        hex::decode_to_slice(file_name, &mut bytes).ok()?;
        Some(blake3::Hash::from_bytes(bytes))
    }
}

impl DiskStorageKey for () {
    fn to_file_name(&self) -> impl AsRef<Path> {
        "0"
    }

    fn from_file_name(file_name: &str) -> Option<Self> {
        (file_name == "0").then_some(())
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
impl_disk_storable! { impl Changeset<D> = "changeset"; }
impl_disk_storable! { impl File = "file_content"; }
impl_disk_storable! { impl FileDiff = "file_diff"; }
