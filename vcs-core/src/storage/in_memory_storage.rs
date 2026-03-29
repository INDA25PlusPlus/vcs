use crate::storage::Storage;
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

    async fn load(&self, key: &K) -> Result<Option<V>, Self::Error> {
        Ok(self.map.get(key).map(|v| v.clone()))
    }

    async fn store(&self, key: &K, value: &V) -> Result<(), Self::Error> {
        self.map.insert(key.clone(), value.clone());
        Ok(())
    }
}
