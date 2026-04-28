pub mod index;
pub mod repo_storage;

use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::diff::file_diff::FileDiff;
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::path::RepoPath;
use crate::repo::index::Index;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::{Revision, RevisionHeader, RevisionId, RevisionMetadata};
use crate::storage::cache::MutableCache;
use crate::storage::{StorageError, StorageResult, cache::FrozenCache};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

pub struct Repository<D: CryptoDigest + CryptoHash, S>
where
    D: Hash + Eq + Send + Sync,
    S: RepoStorage<D>,
    S::RepoStorageError: Error + Send,
{
    head: MutableCache<(), RevisionId<D>, S>,

    revision_headers: MutableCache<RevisionId<D>, RevisionHeader<D>, S>,
    revision_metadatas: MutableCache<RevisionId<D>, RevisionMetadata<D>, S>,

    indexes: MutableCache<RevisionId<D>, Index<D>, S>,

    repo_diffs: FrozenCache<D, RepoDiff<D>, S>,
    file_diffs: FrozenCache<D, FileDiff, S>,

    storage: Arc<S>,
}

pub type RepoResult<T, E> = Result<T, RepoError<E>>;

#[derive(Debug, thiserror::Error)]
pub enum RepoError<E> {
    #[error("failed to find object in database")]
    MissingObject,
    #[error("internal storage error: '{0}'")]
    StorageError(E),
}

fn storage_expect<T, E>(result: StorageResult<T, E>) -> RepoResult<T, E> {
    result.map_err(|err| match err {
        StorageError::InternalError(err) => RepoError::StorageError(err),
        StorageError::MissingObject => RepoError::MissingObject,
    })
}

impl<D: CryptoDigest + CryptoHash, S> Repository<D, S>
where
    D: Hash + Eq + Clone + Send + Sync,
    S: RepoStorage<D> + Send + Sync,
    S::RepoStorageError: Error + Send,
{
    pub async fn init(
        storage: Arc<S>,
        sign_context: SignContext<'_>,
    ) -> RepoResult<Repository<D, S>, S::RepoStorageError> {
        // todo: store repo diff
        let init_rev = Revision::new_initial(sign_context);
        let init_rev_digest: D = init_rev.to_digest();

        let (init_rev_header, init_rev_meta) = init_rev.into_parts();

        let head = MutableCache::new(storage.clone());
        let revision_headers = MutableCache::new(storage.clone());
        let revision_metadatas = MutableCache::new(storage.clone());

        let result: Result<_, S::RepoStorageError> = tokio::try_join!(
            head.set(&(), init_rev_digest.clone()),
            revision_headers.set(&init_rev_digest, init_rev_header),
            revision_metadatas.set(&init_rev_digest, init_rev_meta),
        );
        result.map_err(RepoError::StorageError)?;

        Ok(Repository {
            head,
            revision_headers,
            revision_metadatas,
            indexes: MutableCache::new(storage.clone()),
            repo_diffs: FrozenCache::new(storage.clone()),
            file_diffs: FrozenCache::new(storage.clone()),
            storage,
        })
    }

    pub async fn load(storage: Arc<S>) -> Repository<D, S> {
        Repository {
            head: MutableCache::new(storage.clone()),
            revision_headers: MutableCache::new(storage.clone()),
            revision_metadatas: MutableCache::new(storage.clone()),
            indexes: MutableCache::new(storage.clone()),
            repo_diffs: FrozenCache::new(storage.clone()),
            file_diffs: FrozenCache::new(storage.clone()),
            storage,
        }
    }

    pub async fn head(&self) -> RepoResult<RevisionId<D>, S::RepoStorageError> {
        storage_expect(self.head.get(&(), async |v| v.clone()).await)
    }

    pub async fn set_head(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        self.head
            .set(&(), revision_id)
            .await
            .map_err(RepoError::StorageError)
    }

    pub async fn load_working_tree(
        &self,
        revision_id: RevisionId<D>,
        path: RepoPath,
    ) -> RepoResult<(), S::RepoStorageError> {
        todo!("load from disk to working tree at `rev`")
    }

    pub async fn store_working_tree(
        &self,
        revision_id: RevisionId<D>,
        path: RepoPath,
    ) -> RepoResult<(), S::RepoStorageError> {
        todo!("store working tree at `rev` to disk")
    }

    pub async fn get_working_tree(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<RepoDiff<D>, S::RepoStorageError> {
        todo!("get diff from Head at `rev` to working tree at `rev`")
    }

    pub async fn apply_working_tree(
        &self,
        revision_id: RevisionId<D>,
        diff: RepoDiff<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        todo!("apply diff to working tree at `rev`")
    }

    pub async fn restore_working_tree(
        &self,
        revision_id: RevisionId<D>,
        path: RepoPath,
    ) -> RepoResult<(), S::RepoStorageError> {
        todo!(
            "restore working tree at `rev` to head at `rev`, applying only to `path` and subpaths"
        )
    }

    pub async fn get_index(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<RepoDiff<D>, S::RepoStorageError> {
        todo!("get diff from Head at `rev` to index at `rev`")
    }

    pub async fn apply_index(
        &self,
        revision_id: RevisionId<D>,
        diff: RepoDiff<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        todo!("apply diff to index at `rev`")
    }

    pub async fn restore_index(
        &self,
        revision_id: RevisionId<D>,
        path: RepoPath,
    ) -> RepoResult<(), S::RepoStorageError> {
        todo!("restore index at `rev` to head at `rev`, applying only to `path` and subpaths")
    }

    pub async fn get_diff(
        &self,
        repo_diff_ref: RepoDiffRef<D>,
    ) -> RepoResult<&RepoDiff<D>, S::RepoStorageError> {
        todo!()
    }

    pub async fn insert_diff(&self, repo_diff: RepoDiff<D>) -> RepoResult<(), S::RepoStorageError> {
        todo!()
    }

    pub async fn get_revision_header(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<RevisionHeader<D>, S::RepoStorageError> {
        // clone rev header
        todo!()
    }

    pub async fn get_revision_metadata<R>(
        &self,
        revision_id: RevisionId<D>,
        f: impl AsyncFnOnce(&RevisionMetadata<D>) -> R,
    ) -> RepoResult<R, S::RepoStorageError> {
        todo!()
    }

    pub async fn insert_revision(
        &self,
        revision: Revision<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        // check that parent exists
        // if `revision` is committed, check that parent is committed
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
