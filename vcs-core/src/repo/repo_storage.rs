use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::file_diff::{FileDiff, FileDiffRef};
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::repo::index::Index;
use crate::revision::{RevisionHeader, RevisionId, RevisionMetadata};
use crate::storage::in_memory_storage::InMemoryStorage;
use crate::storage::{Storage, StorageResult};
use std::convert::Infallible;
use std::error::Error;
use std::hash::Hash;

pub trait RepoStorage<D: CryptoDigest + CryptoHash>:
    Storage<(), RevisionId<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, RevisionHeader<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, RevisionMetadata<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, Index<D>, Error = Self::RepoStorageError>
    + Storage<RepoDiffRef<D>, RepoDiff<D>, Error = Self::RepoStorageError>
    + Storage<FileDiffRef<D>, FileDiff, Error = Self::RepoStorageError>
    + Send
    + Sync
where
    D: Send,
{
    type RepoStorageError: Error + Send;
}

macro_rules! in_memory_repo_storage {
    ($($field:ident: InMemoryStorage<$key:ty, $value:ty>,)*) => {
        pub struct InMemoryRepoStorage<D: CryptoDigest + CryptoHash + Eq + Hash + Clone> {
            $($field: InMemoryStorage<$key, $value>,)*
        }

        impl<D: CryptoDigest + CryptoHash + Eq + Hash + Clone> InMemoryRepoStorage<D> {
            pub fn new() -> InMemoryRepoStorage<D> {
                InMemoryRepoStorage {
                    $($field: InMemoryStorage::new(),)*
                }
            }
        }

        $(
        impl<D: CryptoDigest + CryptoHash + Eq + Hash + Clone> Storage<$key, $value>
            for InMemoryRepoStorage<D>
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
            for InMemoryRepoStorage<D>
        {
            type RepoStorageError = Infallible;
        }
    };
}

in_memory_repo_storage! {
    head: InMemoryStorage<(), RevisionId<D>>,
    revision_headers: InMemoryStorage<RevisionId<D>, RevisionHeader<D>>,
    revision_metadatas: InMemoryStorage<RevisionId<D>, RevisionMetadata<D>>,
    indexes: InMemoryStorage<RevisionId<D>, Index<D>>,
    repo_diffs: InMemoryStorage<RepoDiffRef<D>, RepoDiff<D>>,
    file_diffs: InMemoryStorage<FileDiffRef<D>, FileDiff>,
}
