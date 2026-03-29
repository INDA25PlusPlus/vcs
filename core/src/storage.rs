pub mod in_memory_storage;
pub mod lazy;

/// Trait representing an external storage such as a file system
pub trait Storage<K, V> {
    type Error;

    /// returns `Ok(None)` when a key is not present. Storage backends should
    /// reserve `Err` for actual storage failures.
    async fn load(&self, key: &K) -> Result<Option<V>, Self::Error>;
    async fn store(&self, key: &K, value: &V) -> Result<(), Self::Error>;
}
