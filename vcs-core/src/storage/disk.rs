use std::path::PathBuf;
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;
use crate::crypto::digest::CryptoDigest;

use super::{Storage, StorageError, StorageResult};

// DiskStorage struct
pub struct DiskStorage {
    pub base_path: PathBuf,
}

impl DiskStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }
}

// DiskStorage impl Storage
impl<K, V> Storage<K, V> for DiskStorage
where
    K: CryptoDigest,
    V: Serialize + DeserializeOwned,
{
    type Error = std::io::Error;

    fn load(&self, key: &K)
        -> impl Future<Output = StorageResult<V, Self::Error>>
    {
        let base_path = self.base_path.clone();

        async move {
            // make path from key
            let filename = hex::encode(key.bytes());
            let path = base_path.join(filename);

            // ensure file oki
            if !path.exists() {
                return Err(StorageError::MissingObject);
            }

            // get the data as bytes
            let bytes = std::fs::read(&path)
                .map_err(StorageError::InternalError)?;

            // deserialize
            let value = postcard::from_bytes::<V>(&bytes)
                .map_err(|_| {
                    StorageError::InternalError(
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "deserialization failed",
                        )
                    )
                })?;

            Ok(value)
        }
    }

    fn store(&self, key: &K, value: &V)
        -> impl Future<Output = Result<(), Self::Error>>
    {
        let base_path = self.base_path.clone();

        async move {
            // make path from key
            let filename = hex::encode(key.bytes());
            let path = base_path.join(filename);

            // serialize
            let bytes = postcard::to_allocvec(value)
                .map_err(|_| std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "serialization failed"
                ))?;

            // ensure directory oki
            std::fs::create_dir_all(&base_path)?;

            // save to disk
            std::fs::write(path, bytes)?;

            Ok(())
        }
    }

    fn delete(&self, key: &K)
        -> impl Future<Output = Result<(), Self::Error>>
    {
        let base_path = self.base_path.clone();

        async move {
            // make path from key
            let filename = hex::encode(key.bytes());
            let path = base_path.join(filename);

            // ensure file oki
            if !path.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "missing object",
                ));
            }

            // delete file
            std::fs::remove_file(path)?;

            Ok(())
        }
    }
}
