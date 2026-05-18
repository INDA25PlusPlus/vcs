use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::fs::file::{File, FileDiff, FileDiffRef, FileRef};
use crate::repo::repo_storage::RepoStorage;
use crate::repo::{PendingChanges, StagedChanges};
use crate::revision::{RevisionHeader, RevisionMetadata, RevisionRef};
use crate::storage::{SingletonStorage, Storage, StorageError, StorageResult};
use std::convert::Infallible;
use std::hash::Hash;

#[derive(Debug, Default)]
pub struct MemoryStorage<K: Eq + Hash + Clone, V: Clone> {
    map: dashmap::DashMap<K, V>,
}

impl<K: Eq + Hash + Clone, V: Clone> MemoryStorage<K, V> {
    pub fn new() -> MemoryStorage<K, V> {
        MemoryStorage {
            map: dashmap::DashMap::new(),
        }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Storage<K, V> for MemoryStorage<K, V> {
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

impl<V: Clone + Sync> SingletonStorage<V> for MemoryStorage<(), V> {}

macro_rules! memory_repo_storage {
    ($($field:ident: MemoryStorage<$key:ty, $value:ty>,)*) => {
        pub struct MemoryRepoStorage<D: CryptoDigest + CryptoHash + Eq + Hash + Clone> {
            $($field: MemoryStorage<$key, $value>,)*
        }

        impl<D: CryptoDigest + CryptoHash + Eq + Hash + Clone> MemoryRepoStorage<D> {
            pub fn new() -> MemoryRepoStorage<D> {
                MemoryRepoStorage {
                    $($field: MemoryStorage::new(),)*
                }
            }
        }

        impl<D: CryptoDigest + CryptoHash + Eq + Hash + Clone> Default for MemoryRepoStorage<D> {
            fn default() -> MemoryRepoStorage<D> {
                MemoryRepoStorage::new()
            }
        }

        $(
        impl<D: CryptoDigest + CryptoHash + Eq + Hash + Clone> Storage<$key, $value>
            for MemoryRepoStorage<D>
        {
            type Error = Infallible;

            async fn load(&self, key: &$key) -> StorageResult<$value, Self::Error> {
                self.$field.load(key).await
            }

            async fn store(&self, key: &$key, value: &$value) -> Result<(), Self::Error> {
                self.$field.store(key, value).await
            }

            async fn delete(&self, key: &$key) -> Result<(), Self::Error> {
                self.$field.delete(key).await
            }
        }
        )*

        impl<D: CryptoDigest + CryptoHash + Eq + Hash + Clone + Send + Sync> RepoStorage<D>
            for MemoryRepoStorage<D>
        {
            type RepoStorageError = Infallible;
        }
    };
}

memory_repo_storage! {
    head: MemoryStorage<(), RevisionRef<D>>,
    revision_headers: MemoryStorage<RevisionRef<D>, RevisionHeader<D>>,
    revision_metadatas: MemoryStorage<RevisionRef<D>, RevisionMetadata<D>>,
    pending_changes: MemoryStorage<RevisionRef<D>, PendingChanges<D>>,
    staged_changes: MemoryStorage<RevisionRef<D>, StagedChanges<D>>,
    repo_diffs: MemoryStorage<RepoDiffRef<D>, RepoDiff<D>>,
    files: MemoryStorage<FileRef<D>, File>,
    file_diffs: MemoryStorage<FileDiffRef<D>, FileDiff>,
}
