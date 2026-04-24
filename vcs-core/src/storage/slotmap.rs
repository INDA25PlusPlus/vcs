use std::hash::Hash;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;

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

    /// Return a slot for `key`, inserting `init()` if absent.
    ///
    /// `init()` is synchronous and is called at most once for a given inserted
    /// slot. Async initialization should be done inside the slot value, for
    /// example with `tokio::sync::OnceCell`.
    pub fn get_or_insert_with(&self, key: &K, init: impl FnOnce() -> V) -> Arc<RwLock<V>>
    where
        K: Clone,
    {
        self.inner
            .entry(key.clone())
            .or_insert_with(|| Arc::new(RwLock::new(init())))
            .clone()
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
