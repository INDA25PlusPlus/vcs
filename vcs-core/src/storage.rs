pub mod cache;
mod slotmap;

use std::path::PathBuf;
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;

pub type StorageResult<T, E> = Result<T, StorageError<E>>;

#[derive(thiserror::Error, Debug)]
pub enum StorageError<E> {
    #[error("internal storage error: {0}")]
    InternalError(E),
    #[error("entry does not exist")]
    MissingObject,
}

/// Trait representing an external storage such as a file system
pub trait Storage<K, V> {
    type Error;

    fn load(&self, key: &K) -> impl Future<Output = StorageResult<V, Self::Error>>;
    fn store(&self, key: &K, value: &V) -> impl Future<Output = Result<(), Self::Error>>;
    fn delete(&self, key: &K) -> impl Future<Output = Result<(), Self::Error>>;
}

pub trait SingletonStorage<V>: Storage<(), V>
where
    V: Sync,
{
    fn load_singleton(&self) -> impl Future<Output = StorageResult<V, Self::Error>> {
        async { Storage::load(self, &()).await }
    }
    fn store_singleton(&self, value: &V) -> impl Future<Output = Result<(), Self::Error>> {
        async { Storage::store(self, &(), value).await }
    }
    fn delete_singleton(&self) -> impl Future<Output = Result<(), Self::Error>> {
        async { Storage::delete(self, &()).await }
    }
}


// DiskStorage struct
pub struct DiskStorage {
    pub base_path: PathBuf,
}

// DiskStorage impl Storage
impl<K, V> Storage<K, V> for DiskStorage
where
    K: ToString,
    V: Serialize + DeserializeOwned,
{
    type Error = std::io::Error;

    fn load(&self, key: &K)
        -> impl Future<Output = StorageResult<V, Self::Error>>
    {
        let base_path = self.base_path.clone();

        async move {
            // make path from key
            let path = base_path.join(key.to_string());

            // ensure directory oki
            std::fs::create_dir_all(&base_path)
                .map_err(StorageError::InternalError)?;

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
            let path = base_path.join(key.to_string());

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
            let path = base_path.join(key.to_string());

            // ensure directory oki
            std::fs::create_dir_all(&base_path)?;

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
