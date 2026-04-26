pub mod index;
pub mod repo_storage;

use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::diff::file_diff::FileDiff;
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::path::RepoPath;
use crate::repo::index::Index;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::{Patch, Revision, RevisionHeader, RevisionId, RevisionMetadata};
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

    /// Generates a new repo diff from a series of patches, stores it to storage and returns its
    /// hash.
    pub async fn squash(&self, patches: &[Patch<D>]) -> RepoDiffRef<D> {
        todo!()
    }

    pub async fn index(&self, id: RevisionId<D>) -> StorageResult<&Index<D>, S::RepoStorageError> {
        todo!()
        // repo_result(<S as Storage<CommitId, Index<H>>>::load(self, &id).await)
    }

    pub async fn create_patch(&mut self, message: String) {
        todo!()
    }

    pub async fn stage(&self, repo_path: &RepoPath) -> RepoResult<(), S::RepoStorageError> {
        todo!()
    }

    pub async fn unstage(&mut self, repo_path: &RepoPath) {
        todo!()
    }

    pub async fn checkout(&mut self, commit_id: &RevisionId<D>) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
