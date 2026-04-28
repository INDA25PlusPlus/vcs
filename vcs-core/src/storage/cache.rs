use super::slotmap::SlotMap;
use crate::storage::{Storage, StorageError, StorageResult};
use elsa::sync::FrozenMap;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Thread-safe append-only map with lazy loading from an external storage.
/// Guarantees persistent key-to-value mappings if `S` makes that guarantee.
#[derive(Debug, Default, Clone)]
pub struct FrozenCache<K: Eq + Hash, V, S: Storage<K, V>> {
    // FrozenMap is used in order to allow concurrent reads and
    // writes by disallowing mutation or deletion of existing entries
    items: FrozenMap<K, Box<OnceCell<V>>>,
    storage: Arc<S>,
}

impl<K: Eq + Hash + Send + Sync, V: Send + Sync, S: Storage<K, V> + Send + Sync>
    FrozenCache<K, V, S>
where
    K: Clone,
    S::Error: Send,
{
    /// Create a new map backed by `storage`
    pub fn new(storage: Arc<S>) -> FrozenCache<K, V, S> {
        FrozenCache {
            items: FrozenMap::new(),
            storage,
        }
    }

    /// Get the value at `key` if it is loaded, or try to load it from storage
    pub async fn get(&self, key: &K) -> StorageResult<&V, S::Error> {
        let entry = self.get_or_create_entry(key);
        entry
            .get_or_try_init(async || self.storage.load(key).await)
            .await
    }

    /// Attempt to insert `value` at `key`, returning any storage errors. Returns the inserted value
    /// or the old value if the entry already exists.
    pub async fn insert(&self, key: &K, value: V) -> Result<&V, S::Error> {
        let entry = self.get_or_create_entry(key);
        // LOGICAL RACE: if another thread attempts to init the cell here, get_or_try_init ensures
        // that the current thread will wait and then retrieve the initialized value
        entry
            .get_or_try_init(async || {
                self.storage.store(key, &value).await?;
                Ok(value)
            })
            .await
    }

    fn get_or_create_entry(&self, key: &K) -> &OnceCell<V> {
        if let Some(entry) = self.items.get(key) {
            entry
        } else {
            // only clone key if initial check shows that entry does not exist
            // LOGICAL RACE: if entry is created by another thread here, we clone unnecessarily but
            // don't lose any correctness
            self.items.insert(key.clone(), Box::new(OnceCell::new()))
        }
    }
}

/// Thread-safe mutable map with lazy loading and async storing to an external storage.
pub struct MutableCache<K: Eq + Hash, V, S: Storage<K, V>> {
    items: SlotMap<K, MutableCacheEntry<V>>,
    storage: Arc<S>,
}

enum MutableCacheEntry<V> {
    Value(OnceCell<V>),
    Tombstone,
}

impl<K: Eq + Hash, V, S: Storage<K, V>> MutableCache<K, V, S> {
    /// Create a new map backed by `storage`
    pub fn new(storage: Arc<S>) -> MutableCache<K, V, S> {
        MutableCache {
            items: SlotMap::new(),
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
        let slot_guard = self
            .items
            .read_or_insert_with(key, || MutableCacheEntry::Value(OnceCell::new()))
            .await;

        match slot_guard.deref() {
            MutableCacheEntry::Value(cell) => {
                // get_or_try_init ensures that the current thread will wait for other threads
                // before retrieving the initialized value
                let value = cell
                    .get_or_try_init(async || {
                        let value = self.storage.load(key).await?;
                        Ok(value)
                    })
                    .await?;

                Ok(f(value).await)
            }
            MutableCacheEntry::Tombstone => Err(StorageError::MissingObject),
        }
        // drop slot_guard
    }

    /// Set the value at `key` only if able to successfully store the value in storage.
    /// Concurrent calls to this method are guaranteed to perform the stores atomically.
    ///
    /// **Locking behavior:** Will deadlock if called from a closure passed into `get`.
    pub async fn set(&self, key: &K, value: V) -> Result<(), S::Error> {
        let mut slot_guard = self
            .items
            .write_or_insert_with(key, || MutableCacheEntry::Value(OnceCell::new()))
            .await;

        // guard ensures no concurrent stores from this cache, which is required for atomicity in
        // this function
        self.storage.store(key, &value).await?;

        *slot_guard = MutableCacheEntry::Value(OnceCell::from(value));
        Ok(())
        // drop slot_guard
    }

    /// Update the value at `key` with `f`, only if able to successfully store the updated value in
    /// storage. Concurrent calls to this method are guaranteed to perform the load, update, store,
    /// and cache replacement atomically.
    ///
    /// **Locking behavior:** Will deadlock if called from a closure passed into `get`.
    pub async fn update(
        &self,
        key: &K,
        f: impl AsyncFnOnce(&V) -> V,
    ) -> StorageResult<(), S::Error> {
        let mut slot_guard = self
            .items
            .write_or_insert_with(key, || MutableCacheEntry::Value(OnceCell::new()))
            .await;

        let updated_value = match slot_guard.deref() {
            MutableCacheEntry::Value(cell) => {
                // get_or_try_init ensures that the current thread will wait for other threads
                // before retrieving the initialized value
                let value = cell
                    .get_or_try_init(async || {
                        let value = self.storage.load(key).await?;
                        Ok(value)
                    })
                    .await?;

                f(value).await
            }
            MutableCacheEntry::Tombstone => return Err(StorageError::MissingObject),
        };

        self.storage
            .store(key, &updated_value)
            .await
            .map_err(StorageError::InternalError)?;

        *slot_guard = MutableCacheEntry::Value(OnceCell::from(updated_value));
        Ok(())
        // drop slot_guard
    }

    /// Remove the entry at `key` only if able to successfully remove the value from storage.
    /// Concurrent calls to this method with `get`, `set`, or `update` are guaranteed to leave the
    /// cache and storage in a consistent state.
    ///
    /// **Locking behavior:** Will deadlock if called from a closure passed into `get`.
    pub async fn remove(&self, key: &K) -> Result<(), S::Error> {
        let mut slot_guard = self
            .items
            .write_or_insert_with(key, || MutableCacheEntry::Value(OnceCell::new()))
            .await;

        // guard ensures no concurrent stores or deletions, which is required for atomicity.
        self.storage.delete(key).await?;
        *slot_guard = MutableCacheEntry::Tombstone;
        Ok(())
        // drop slot_guard
    }

    /// Run garbage collection on cached entries that have been removed. Not necessary for normal
    /// operation but will improve memory usage after removal of many entries.
    pub async fn cleanup(&self) {
        self.items.retain(|_, entry| {
            if Arc::strong_count(entry) != 1 {
                // Another task already has this slot handle. Keep it so that
                // task cannot update a slot that has been detached from the map.
                return true;
            }

            let Ok(slot_guard) = entry.try_read() else {
                // With no external slot handles, no other task should be able to hold this lock.
                debug_assert!(false, "Expected read guard");
                return true;
            };
            matches!(slot_guard.deref(), MutableCacheEntry::Value(..))
            // drop guard
        });
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

        async fn delete(&self, _key: &()) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn test_impl_send() {
        //! Test that futures returned from FrozenCache::get actually implement
        //! Send. This is required to spawn futures in tokio::spawn, for example.

        let storage = FrozenCache::new(Arc::new(TestStorage));

        fn require_send<T: Send + Sized>(_: T) {}

        // compile error if the future returned from storage.get doesn't
        // implement Send
        require_send(storage.get(&()));
    }
}
