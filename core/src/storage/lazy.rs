use crate::storage::Storage;
use std::convert::Infallible;
use std::hash::Hash;

/// Type capable of deterministically evaluating values `V` for a subset of `K`.
pub trait Evaluator<K, V> {
    async fn evaluate(&self, key: &K) -> Option<V>;
}

impl<K, V, S: Storage<K, V, Error = Infallible>> Evaluator<K, V> for S {
    async fn evaluate(&self, key: &K) -> Option<V> {
        self.load(key).await.unwrap_or_else(|_| unreachable!())
    }
}

/// Thread-safe cache of lazily-evaluated values
#[derive(Debug, Default, Clone)]
pub struct LazyCache<K: Eq + Hash, V, S: Evaluator<K, V>> {
    // elsa::sync::FrozenMap is used in order to allow concurrent reads and
    // writes by disallowing mutation or deletion of existing entries
    items: elsa::sync::FrozenMap<K, Box<V>>,
    evaluator: S,
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

impl<K: Eq + Hash, V, S: Evaluator<K, V>> LazyCache<K, V, S>
where
    K: Send + Sync,
    V: Send + Sync,
    S: Send + Sync,
{
    /// Create a new map backed by `evaluator`
    pub fn new(evaluator: S) -> LazyCache<K, V, S> {
        LazyCache {
            items: elsa::sync::FrozenMap::default(),
            evaluator,
        }
    }

    /// Evaluate the value at `key`, using the cached value if available.
    /// Returns `None` if there is no matching value for `key`.
    pub async fn evaluate(&self, key: K) -> Option<&V> {
        if let Some(value) = self.items.get(&key) {
            return Some(value);
        }
        let value = self.evaluator.evaluate(&key).await?;
        Some(self.items.insert(key, Box::new(value)))
    }
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
        Ok(Some(value_ref))
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

    struct TestCache;

    struct TestStorage;

    static_assertions::assert_impl_all!(
        LazyCache<(), (), TestCache>: Send, Sync);
    static_assertions::assert_impl_all!(
        LazyStorage<(), (),TestStorage>: Send, Sync);

    impl Evaluator<(), ()> for TestCache {
        async fn evaluate(&self, key: &()) -> Option<()> {
            Some(())
        }
    }

    impl Storage<(), ()> for TestStorage {
        type Error = ();
        async fn load(&self, _key: &()) -> Result<Option<()>, Self::Error> {
            Ok(Some(()))
        }
        async fn store(&self, _key: &(), _value: &()) -> Result<(), Self::Error> {
            Ok(())
        }
    }
}
