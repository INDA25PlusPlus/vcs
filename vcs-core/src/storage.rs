pub mod cache;
pub mod in_memory_storage;
mod slotmap;

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
    async fn delete(&self, key: &K) -> Result<(), Self::Error>;
}

pub trait SingletonStorage<V>: Storage<(), V>
where
    V: Sync,
{
    async fn load_singleton(&self) -> StorageResult<V, Self::Error> {
        Storage::load(self, &()).await
    }

    async fn store_singleton(&self, value: &V) -> Result<(), Self::Error> {
        Storage::store(self, &(), value).await
    }

    async fn delete_singleton(&self) -> Result<(), Self::Error> {
        Storage::delete(self, &()).await
    }
}
