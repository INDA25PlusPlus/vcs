pub mod in_memory_storage;

use std::hash::Hash;

/// Trait representing an external storage such as a file system
pub trait Storage<K, V> {
    type Error;

    /// returns `Ok(None)` when a key is not present. Storage backends should
    /// reserve `Err` for actual storage failures.
    async fn load(&self, key: &K) -> Result<Option<V>, Self::Error>;
    async fn store(&self, key: &K, value: &V) -> Result<(), Self::Error>;
}

/// Thread-safe append-only map with lazy loading from an external storage.
/// Guarantees persistent key-to-value mappings if `S` makes that guarantee.
#[derive(Debug, Default, Clone)]
pub struct LazyStorage<K: Eq + Hash, V, S: Storage<K, V>> {
    // elsa::sync::FrozenMap is used in order to allow concurrent reads and
    // writes by disallowing mutation or deletion of existing entries
    items: elsa::sync::FrozenMap<K, Box<V>>,
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

    /// Get the value at `key` if it is loaded, or try to load it from storage
    pub async fn get(&self, key: K) -> Result<Option<&V>, S::Error> {
        if let Some(value) = self.items.get(&key) {
            return Ok(Some(value));
        }
        let Some(value) = self.storage.load(&key).await? else {
            return Ok(None);
        };
        let value_ref = self.items.insert(key, Box::new(value));
        return Ok(Some(value_ref));
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
        self.items.insert(key, Box::new(value));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestStorage;

    impl Storage<(), ()> for TestStorage {
        type Error = ();
        async fn load(&self, _key: &()) -> Result<Option<()>, Self::Error> {
            Ok(Some(()))
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
        require_send(storage.get(()));
    }
}
