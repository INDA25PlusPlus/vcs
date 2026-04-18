use crate::storage::{Storage, StorageResult};
use dashmap::DashMap;
use elsa::sync::FrozenMap;
use std::borrow::Borrow;
use std::hash::Hash;
use std::ops::Deref;
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
    items: DashMap<K, Arc<OnceCell<RwLock<V>>>>,
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
    /// Get the value at `key` if it is loaded, or try to load it from storage. Access is provided
    /// through `f`.
    pub async fn get<R>(&self, key: K, f: impl AsyncFnOnce(&V) -> R) -> StorageResult<R, S::Error> {
        // Unfortunately there is no way around cloning the key, as it is used for both initializing
        // the dashmap entry and loading it from storage. This necessarily has to happen after
        // initializing the entry in order to avoid having to wait for storage at every read.

        // get an existing entry or insert a new OnceCell
        let entry = self.get_or_create_entry(key.clone());

        // CONCURRENCY: if another thread inits the entry here, we just retrieve that newly
        // initialized value instead of initializing it ourselves.

        // insert value into entry if it doesn't already exist
        let lock = self.read_or_init_entry(&key, &entry).await?;

        let guard = lock.read().await;
        Ok(f(&guard).await)
        // drop guard
    }

    /// Update the value at `key` and try to store the value in storage.
    ///
    /// **Locking behavior:** May deadlock if called from a closure passed into `get`.
    pub async fn update(&self, key: K, value: V) -> StorageResult<(), S::Error> {
        // Unfortunately there is no way around cloning the key, as it is used for both initializing
        // the dashmap entry and loading it from storage. This necessarily has to happen after
        // initializing the entry in order to avoid having to wait for storage at every read.

        // get an existing entry or insert a new OnceCell
        let entry = self.get_or_create_entry(key.clone());

        // CONCURRENCY: if another thread inits the entry here, we just retrieve that newly
        // initialized value instead of initializing it ourselves.

        // insert value into entry if it doesn't already exist
        let lock = self.read_or_init_entry(&key, &entry).await?;

        // LOCKING: May wait for a read lock only if this method is called when holding a read lock,
        // that is, inside the closure passed to `get`.
        let mut guard = lock.write().await;
        *guard = value;

        Ok(())
        // drop guard
    }

    fn get_or_create_entry(&self, key: K) -> Arc<OnceCell<RwLock<V>>> {
        let entry = self.items.entry(key);
        // clone the Arc stored in the dashmap
        entry.or_insert_with(|| Arc::new(OnceCell::new())).clone()
    }

    async fn read_or_init_entry<'entry>(
        &self,
        key: &K,
        entry: &'entry OnceCell<RwLock<V>>,
    ) -> StorageResult<&'entry RwLock<V>, S::Error> {
        Ok(entry
            .get_or_try_init(async || {
                let value = self.storage.load(key).await?;
                Ok(RwLock::new(value))
            })
            .await?)
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
