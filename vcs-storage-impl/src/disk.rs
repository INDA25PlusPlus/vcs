use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::path::PathBuf;
// use vcs_core::revision::RevisionMetadata;

use vcs_core::crypto::digest::CryptoDigest;
use vcs_core::storage::{Storage, StorageError, StorageResult};

pub struct DiskStorage<K, V> {
    pub base_path: PathBuf,
    _phantom_data: PhantomData<(K, V)>,
}

impl<K, V> DiskStorage<K, V> {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            _phantom_data: PhantomData,
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

impl<K, V> Storage<K, V> for DiskStorage<K, V>
where
    K: CryptoDigest,
    V: Serialize + for<'de> Deserialize<'de> + DiskStorable,
{
    type Error = DiskStorageError;

    async fn load(&self, key: &K) -> StorageResult<V, Self::Error> {
        // make path from key
        let filename = hex::encode(key.bytes());

        let path = self
            .base_path
            .join(V::OBJECT_PATH)
            .join(filename);

        // ensure file exists
        if !path.exists() {
            return Err(StorageError::MissingObject);
        }

        // read bytes
        let bytes = std::fs::read(&path)
            .map_err(|e| {
                StorageError::InternalError(
                    DiskStorageError::Io(e)
                )
            })?;

        // deserialize
        let value = postcard::from_bytes::<V>(&bytes)
            .map_err(|_| {
                StorageError::InternalError(
                    DiskStorageError::Deserialization
                )
            })?;

        Ok(value)
    }

    async fn store(
        &self,
        key: &K,
        value: &V,
    ) -> Result<(), Self::Error> {
        // make path from key
        let filename = hex::encode(key.bytes());

        let dir = self.base_path.join(V::OBJECT_PATH);

        let path = dir.join(filename);

        // serialize
        let bytes = postcard::to_allocvec(value)
            .map_err(|_| DiskStorageError::Serialization)?;

        // ensure directory exists
        std::fs::create_dir_all(&dir)?;

        // write file
        std::fs::write(path, bytes)?;

        Ok(())
    }

    async fn delete(
        &self,
        key: &K,
    ) -> Result<(), Self::Error> {
        // make path from key
        let filename = hex::encode(key.bytes());

        let path = self
            .base_path
            .join(V::OBJECT_PATH)
            .join(filename);

        // ensure file exists
        if !path.exists() {
            return Err(
                DiskStorageError::Io(
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "missing object",
                    )
                )
            );
        }

        // delete file
        std::fs::remove_file(path)?;

        Ok(())
    }
}

pub trait DiskStorable {
    const OBJECT_PATH: &'static str;
}

// impl<D: CryptoDigest> DiskStorable for RevisionMetadata<D> {
//     const OBJECT_PATH: &'static str = "rev_meta";
// }
