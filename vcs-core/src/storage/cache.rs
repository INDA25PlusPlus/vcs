use crate::storage::{Storage, StorageResult};
use dashmap::DashMap;
use elsa::sync::FrozenMap;
use std::borrow::Borrow;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::{OnceCell, RwLock};

/// Type that can be converted into an owned value.
///
/// Has blanket implementations for
///
/// `T -> T` (move)
///
/// `&T -> T` (clone)
///
/// Users should generally prefer to use &T when T: Clone.
pub trait IntoOwned<R>: Borrow<R> {
    fn into_owned(self) -> R;
}

impl<T> IntoOwned<T> for T {
    fn into_owned(self) -> T {
        self
    }
}

impl<T: Clone> IntoOwned<T> for &T {
    fn into_owned(self) -> T {
        self.clone()
    }
}

/// Thread-safe append-only map with lazy loading from an external storage.
/// Guarantees persistent key-to-value mappings if `S` makes that guarantee.
#[derive(Debug, Default, Clone)]
pub struct FrozenCache<K: Eq + Hash, V, S: Storage<K, V>> {
    // FrozenMap is used in order to allow concurrent reads and
    // writes by disallowing mutation or deletion of existing entries
    items: FrozenMap<K, Box<V>>,
    storage: Arc<S>,
}

impl<K: Eq + Hash + Send + Sync, V: Send + Sync, S: Storage<K, V> + Send + Sync>
    FrozenCache<K, V, S>
where
    S::Error: Send,
{
    /// Create a new map backed by `storage`
    pub fn new(storage: Arc<S>) -> FrozenCache<K, V, S> {
        FrozenCache {
            items: FrozenMap::default(),
            storage,
        }
    }

    /// Get the value at `key` if it is loaded, or try to load it from storage
    pub async fn get<Q: IntoOwned<K>>(&self, key: Q) -> StorageResult<&V, S::Error> {
        if let Some(value) = self.items.get(key.borrow()) {
            return Ok(value);
        }
        let value = self.storage.load(key.borrow()).await?;
        let value_ref = self.items.insert(key.into_owned(), Box::new(value));
        Ok(value_ref)
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

/// Thread-safe mutable map with lazy loading and async storing to an external storage.
pub struct MutableCache<K: Eq + Hash, V, S: Storage<K, V>> {
    items: DashMap<K, Arc<RwLock<OnceCell<V>>>>,
    storage: Arc<S>,
}

impl<K: Eq + Hash, V, S: Storage<K, V>> MutableCache<K, V, S> {
    /// Create a new map backed by `storage`
    pub fn new(storage: Arc<S>) -> MutableCache<K, V, S> {
        MutableCache {
            items: DashMap::new(),
            storage,
        }
    }
}

impl<K: Eq + Hash, V, S: Storage<K, V>> MutableCache<K, V, S>
where
    K: Clone,
{
    /// Get the value at `key` if it is loaded, or try to load it from storage. Access to the value
    /// is provided through `f`.
    pub async fn get<R>(
        &self,
        key: &K,
        f: impl AsyncFnOnce(&V) -> R,
    ) -> StorageResult<R, S::Error> {
        let entry = self.get_or_create_entry(key);
        let guard = entry.read().await;

        // LOGICAL RACE: if another thread attempts to init the cell here, get_or_try_init ensures
        // that the current thread will wait and then retrieve the initialized value
        let value = guard
            .get_or_try_init(async || {
                let value = self.storage.load(key).await?;
                Ok(value)
            })
            .await?;

        Ok(f(value).await)
        // drop guard
    }

    /// Update the value at `key` and try to store the value in storage. Concurrent calls to this
    /// method are guaranteed to peform the stores atomically.
    ///
    /// **Locking behavior:** Will deadlock if called from a closure passed into `get`.
    pub async fn update(&self, key: &K, value: V) -> Result<(), S::Error> {
        let entry = self.get_or_create_entry(key);
        let mut guard = entry.write().await;

        // guard ensures no concurrent stores from this cache, which is required for atomicity in
        // this function
        self.storage.store(key, &value).await?;

        *guard = OnceCell::from(value);
        Ok(())
        // drop guard
    }

    fn get_or_create_entry(&self, key: &K) -> Arc<RwLock<OnceCell<V>>> {
        if let Some(entry) = self.items.get(key) {
            // need to clone Arc in order to drop reference into DashMap (which could otherwise
            // cause locking issues)
            entry.value().clone()
        } else {
            // only clone key if initial check shows that entry does not exist
            // LOGICAL RACE: if entry is created by another thread here, we clone unnecessarily but
            // don't lose any correctness
            self.items
                .entry(key.clone())
                .or_insert_with(|| Arc::new(RwLock::new(OnceCell::new())))
                .clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestStorage;

    impl Storage<(), ()> for TestStorage {
        type Error = ();

        async fn load(&self, _key: &()) -> StorageResult<(), Self::Error> {
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

        let storage = FrozenCache::new(Arc::new(TestStorage));

        fn require_send<T: Send + Sized>(_: T) {}

        // compile error if the future returned from storage.get doesn't
        // implement Send
        require_send(storage.get(()));
    }
}
