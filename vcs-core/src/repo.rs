pub mod repo_storage;

use crypto_hash_derive::CryptoHash;

use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::diff::file_diff::FileDiff;
use crate::diff::repo_diff::RepoDiff;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::{Revision, RevisionHeader, RevisionId, RevisionMetadata};
use crate::storage::cache::MutableCache;
use crate::storage::{StorageError, cache::FrozenCache};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

#[derive(Clone, CryptoHash, Debug)]
pub struct PendingChanges<D: CryptoDigest>(pub RepoDiff<D>);

#[derive(Clone, CryptoHash, Debug)]
pub struct StagedChanges<D: CryptoDigest>(pub RepoDiff<D>);

pub struct Repo<D: CryptoDigest + CryptoHash, S>
where
    D: Hash + Eq + Send + Sync,
    S: RepoStorage<D>,
    S::RepoStorageError: Error + Send,
{
    head: MutableCache<(), RevisionId<D>, S>,

    revision_headers: MutableCache<RevisionId<D>, RevisionHeader<D>, S>,
    revision_metadatas: MutableCache<RevisionId<D>, RevisionMetadata<D>, S>,

    pending_changes: MutableCache<RevisionId<D>, PendingChanges<D>, S>,
    staged_changes: MutableCache<RevisionId<D>, StagedChanges<D>, S>,

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

impl<E> From<StorageError<E>> for RepoError<E> {
    fn from(value: StorageError<E>) -> Self {
        match value {
            StorageError::InternalError(err) => RepoError::StorageError(err),
            StorageError::MissingObject => RepoError::MissingObject,
        }
    }
}

impl<E> From<E> for RepoError<E> {
    fn from(value: E) -> Self {
        RepoError::StorageError(value)
    }
}

impl<D: CryptoDigest + CryptoHash, S> Repo<D, S>
where
    D: Hash + Eq + Clone + Send + Sync,
    S: RepoStorage<D> + Send + Sync,
    S::RepoStorageError: Error + Send,
{
    pub async fn init(
        storage: Arc<S>,
        sign_context: SignContext<'_>,
    ) -> RepoResult<Repo<D, S>, S::RepoStorageError> {
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
        result?;

        Ok(Repo {
            head,
            revision_headers,
            revision_metadatas,
            pending_changes: MutableCache::new(storage.clone()),
            staged_changes: MutableCache::new(storage.clone()),
            repo_diffs: FrozenCache::new(storage.clone()),
            file_diffs: FrozenCache::new(storage.clone()),
            storage,
        })
    }

    pub async fn load(storage: Arc<S>) -> Repo<D, S> {
        Repo {
            head: MutableCache::new(storage.clone()),
            revision_headers: MutableCache::new(storage.clone()),
            revision_metadatas: MutableCache::new(storage.clone()),
            pending_changes: MutableCache::new(storage.clone()),
            staged_changes: MutableCache::new(storage.clone()),
            repo_diffs: FrozenCache::new(storage.clone()),
            file_diffs: FrozenCache::new(storage.clone()),
            storage,
        }
    }

    pub async fn head(&self) -> RepoResult<RevisionId<D>, S::RepoStorageError> {
        let head = self.head.get(&(), async |v| v.clone()).await?;

        Ok(head)
    }

    pub async fn set_head(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        self.head.set(&(), revision_id).await?;

        Ok(())
    }

    pub async fn pending_changes_at(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<PendingChanges<D>, S::RepoStorageError> {
        let pending_changes = self
            .pending_changes
            .get(&revision_id, async |changes| changes.clone())
            .await?;

        Ok(pending_changes)
    }

    pub async fn set_pending_changes_at(
        &self,
        revision_id: RevisionId<D>,
        diff: RepoDiff<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        self.pending_changes
            .set(&revision_id, PendingChanges(diff))
            .await?;

        Ok(())
    }

    pub async fn staged_changes_at(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<StagedChanges<D>, S::RepoStorageError> {
        let staged_changes = self
            .staged_changes
            .get(&revision_id, async |changes| changes.clone())
            .await?;

        Ok(staged_changes)
    }

    pub async fn set_staged_changes_at(
        &self,
        revision_id: RevisionId<D>,
        diff: RepoDiff<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        self.staged_changes
            .set(&revision_id, StagedChanges(diff))
            .await?;

        Ok(())
    }

    // pub async fn get_diff(
    //     &self,
    //     repo_diff_ref: RepoDiffRef<D>,
    // ) -> RepoResult<&RepoDiff<D>, S::RepoStorageError> {
    //     todo!()
    // }

    pub async fn get_revision_header(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<RevisionHeader<D>, S::RepoStorageError> {
        let header = self
            .revision_headers
            .get(&revision_id, async |header| header.clone())
            .await?;

        Ok(header)
    }

    pub async fn get_revision_metadata(
        &self,
        revision_id: RevisionId<D>,
    ) -> RepoResult<RevisionMetadata<D>, S::RepoStorageError> {
        let metadata = self
            .revision_metadatas
            .get(&revision_id, async |metadata| metadata.clone())
            .await?;

        Ok(metadata)
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
