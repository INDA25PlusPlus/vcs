use std::hash::Hash;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

/// Concurrent key-to-slot map for values that need async per-key access.
///
/// The map itself only stores stable `Arc<RwLock<V>>` handles. Callers clone a
/// slot handle while `DashMap` briefly holds a shard lock, then await on the
/// slot lock independently.
#[derive(Debug)]
pub struct SlotMap<K: Eq + Hash, V> {
    inner: DashMap<K, Arc<RwLock<V>>>,
}

impl<K: Eq + Hash, V> Default for SlotMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash, V> SlotMap<K, V> {
    /// Create an empty map.
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    /// Return the slot for `key`, if present.
    pub fn slot(&self, key: &K) -> Option<Arc<RwLock<V>>> {
        self.inner.get(key).map(|entry| Arc::clone(entry.value()))
    }

    /// Return a slot for `key`, inserting `init()` if absent.
    ///
    /// `init()` is synchronous and is called at most once for a given inserted
    /// slot. Async initialization should be done inside the slot value, for
    /// example with `tokio::sync::OnceCell`.
    pub fn slot_or_insert_with(&self, key: &K, init: impl FnOnce() -> V) -> Arc<RwLock<V>>
    where
        K: Clone,
    {
        self.inner
            .entry(key.clone())
            .or_insert_with(|| Arc::new(RwLock::new(init())))
            .clone()
    }

    /// Read-lock the slot for `key`, if present.
    pub async fn read(&self, key: &K) -> Option<OwnedRwLockReadGuard<V>> {
        let slot = self.slot(key)?;
        Some(slot.read_owned().await)
    }

    /// Write-lock the slot for `key`, if present.
    pub async fn write(&self, key: &K) -> Option<OwnedRwLockWriteGuard<V>> {
        let slot = self.slot(key)?;
        Some(slot.write_owned().await)
    }

    /// Read-lock the slot for `key`, inserting `init()` if absent.
    pub async fn read_or_insert_with(
        &self,
        key: &K,
        init: impl FnOnce() -> V,
    ) -> OwnedRwLockReadGuard<V>
    where
        K: Clone,
    {
        let slot = self.slot_or_insert_with(key, init);
        slot.read_owned().await
    }

    /// Write-lock the slot for `key`, inserting `init()` if absent.
    pub async fn write_or_insert_with(
        &self,
        key: &K,
        init: impl FnOnce() -> V,
    ) -> OwnedRwLockWriteGuard<V>
    where
        K: Clone,
    {
        let slot = self.slot_or_insert_with(key, init);
        slot.write_owned().await
    }

    /// Retain only slots satisfying `predicate`.
    pub fn retain(&self, mut predicate: impl FnMut(&K, &Arc<RwLock<V>>) -> bool) {
        self.inner.retain(|k, v| predicate(k, v));
    }

    /// Return the number of slots in the map.
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}
