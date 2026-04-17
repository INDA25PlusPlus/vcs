pub mod in_memory_storage;
pub mod cache;

use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

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

    async fn load(&self, key: &K) -> StorageResult<V, Self::Error>;
    async fn store(&self, key: &K, value: &V) -> Result<(), Self::Error>;
}
