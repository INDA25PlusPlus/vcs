use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use vcs_core::crypto::digest::CryptoDigest;
use vcs_core::revision::RevisionMetadata;
use vcs_core::storage::{Storage, StorageResult};

pub struct DiskStorage<K, V> {
    _phantom_data: PhantomData<(K, V)>,
    // todo
}

pub enum DiskStorageError {
    // todo
}

impl<K, V> Storage<K, V> for DiskStorage<K, V>
where
    K: CryptoDigest,
    V: Serialize + for<'de> Deserialize<'de> + DiskStorable,
{
    type Error = DiskStorageError;

    async fn load(&self, key: &K) -> StorageResult<V, Self::Error> {
        todo!()
    }

    async fn store(&self, key: &K, value: &V) -> Result<(), Self::Error> {
        todo!()
    }

    async fn delete(&self, key: &K) -> Result<(), Self::Error> {
        todo!()
    }
}

pub trait DiskStorable {
    const OBJECT_PATH: &'static str;
}

impl<D: CryptoDigest> DiskStorable for RevisionMetadata<D> {
    const OBJECT_PATH: &'static str = "rev_meta";
}

// impl...
