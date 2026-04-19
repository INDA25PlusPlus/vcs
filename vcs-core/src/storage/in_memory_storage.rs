use crate::storage::{Storage, StorageError, StorageResult};
use std::convert::Infallible;
use std::hash::Hash;

#[derive(Debug, Default)]
pub struct InMemoryStorage<K: Eq + Hash + Clone, V: Clone> {
    map: dashmap::DashMap<K, V>,
}

impl<K: Eq + Hash + Clone, V: Clone> InMemoryStorage<K, V> {
    pub fn new() -> InMemoryStorage<K, V> {
        InMemoryStorage {
            map: dashmap::DashMap::new(),
        }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Storage<K, V> for InMemoryStorage<K, V> {
    type Error = Infallible;

    async fn load(&self, key: &K) -> StorageResult<V, Self::Error> {
        self.map
            .get(key)
            .map(|v| v.clone())
            .ok_or(StorageError::MissingObject)
    }

    async fn store(&self, key: &K, value: &V) -> Result<(), Self::Error> {
        self.map.insert(key.clone(), value.clone());
        Ok(())
    }

    async fn delete(&self, key: &K) -> Result<(), Self::Error> {
        self.map.remove(key);
        Ok(())
    }
}
