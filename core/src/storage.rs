use std::hash::Hash;

/// Trait representing an external storage such as a file system
pub trait Storage<K, V> {
    type Error;

    async fn load(&self, key: &K) -> Result<V, Self::Error>;

    async fn store(&self, key: &K, value: &V) -> Result<(), Self::Error>;
}

/// Trait representing the ability to enumerate keys known to a storage backend.
pub trait KeyIndex<K, V> {
    type Error;

    async fn keys(&self) -> Result<Vec<K>, Self::Error>;
}

/// Thread-safe append-only map with lazy loading from an external storage.
/// Guarantees persistent key-to-value mappings if `S` makes that guarantee.
#[derive(Debug, Default, Clone)]
pub struct LazyStorage<K: Eq + Hash, V, S: Storage<K, V>> {
    // elsa::sync::FrozenMap is used in order to allow concurrent reads and
    // writes by disallowing mutation or deletion of existing entries

    // tokio::sync::OnceCell instead of std::sync::OnceLock in order to
    // allow async initialization (tokio::sync::OnceCell::get_or_init)
    items: elsa::sync::FrozenMap<K, Box<tokio::sync::OnceCell<V>>>,
    storage: S,
}

impl<K: Eq + Hash + Send + Sync, V: Send + Sync, S: Storage<K, V> + Send + Sync>
    LazyStorage<K, V, S>
where
    S::Error: Send,
{
    /// Create a new map backed by `storage`
    pub fn new(storage: S) -> LazyStorage<K, V, S> {
        LazyStorage {
            items: elsa::sync::FrozenMap::default(),
            storage,
        }
    }

    /// Register `key` as a lazily evaluated entry. Does nothing if `key`
    /// already has an entry.
    pub fn register(&self, key: K) {
        self.items
            .insert(key, Box::new(tokio::sync::OnceCell::new()));
    }

    /// Get the value at `key` if it is loaded, or try to load it from storage
    pub async fn get(&self, key: &K) -> Option<Result<&V, S::Error>> {
        let value = self.items.get(key)?;
        Some(
            value
                .get_or_try_init(|| async { self.storage.load(key).await })
                .await,
        )
    }

    /// Attempt to insert `value` at `key`. Does nothing if `key` already
    /// has an entry. If `value` is inserted, also attempts to store `value`
    /// in storage, returning the error if there is one.
    ///
    /// # Concurrency
    /// Concurrent inserts on the same `key` may result in redundant stores.
    /// Concurrent inserts on different `key`s are safe.
    pub async fn insert(&self, key: K, value: V) -> Result<(), S::Error> {
        if self.items.get(&key).is_some() {
            return Ok(());
        }
        self.storage.store(&key, &value).await?;
        self.items
            .insert(key, Box::new(tokio::sync::OnceCell::from(value)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestStorage;

    impl Storage<(), ()> for TestStorage {
        type Error = ();
        async fn load(&self, _key: &()) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn store(&self, _key: &(), _value: &()) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn test_impl_send() {
        //! Test that futures returned from LazyStorage::get actually implement
        //! Send. This is required to spawn futures in tokio::spawn, for example.

        let storage = LazyStorage::new(TestStorage);

        fn require_send<T: Send + Sized>(_: T) {}

        // compile error if the future returned from storage.get doesn't
        // implement Send
        require_send(storage.get(&()));
    }
}
