pub mod cache;
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
