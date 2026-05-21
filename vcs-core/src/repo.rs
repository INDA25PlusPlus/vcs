pub mod repo_storage;

mod commit;
mod error;
mod history;
mod state;
#[cfg(test)]
mod tests;
mod worktree;

use crate::changeset::Changeset;
use crate::changeset::file::FileDiff;
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::{Revision, RevisionHeader, RevisionMetadata, RevisionRef};
use crate::storage::cache::{FrozenCache, MutableCache};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

pub use error::{CheckoutError, RefreshPendingChangesError, RepoError, RepoResult, RestoreError};
pub use state::{Head, PendingChanges, RepoStatus, StagedChanges};

pub struct Repo<D: CryptoDigest + CryptoHash, S>
where
    D: Hash + Eq + Send + Sync,
    S: RepoStorage<D>,
    S::RepoStorageError: Error + Send,
{
    head: MutableCache<(), Head<D>, S>,

    revision_headers: MutableCache<RevisionRef<D>, RevisionHeader<D>, S>,
    revision_metadatas: MutableCache<RevisionRef<D>, RevisionMetadata<D>, S>,

    pending_changes: MutableCache<RevisionRef<D>, PendingChanges<D>, S>,
    staged_changes: MutableCache<RevisionRef<D>, StagedChanges<D>, S>,

    changesets: FrozenCache<D, Changeset<D>, S>,
    file_diffs: FrozenCache<D, FileDiff, S>,

    storage: Arc<S>,
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
        let init_rev: Revision<D> = Revision::new_initial(sign_context);
        // UGLY: This duplicates the empty diff created by `Revision::new_initial`.
        // The two must hash identically so the stored diff matches the initial revision header.
        let init_changeset = Changeset::<D>::empty();
        let init_changeset_digest = init_rev.header().changeset.clone();
        let init_rev_digest: D = init_rev.to_digest();

        let (init_rev_header, init_rev_meta) = init_rev.into_parts();

        let head = MutableCache::new(storage.clone());
        let revision_headers = MutableCache::new(storage.clone());
        let revision_metadatas = MutableCache::new(storage.clone());
        let changesets = FrozenCache::new(storage.clone());

        let result: Result<_, S::RepoStorageError> = tokio::try_join!(
            head.set(&(), Head(init_rev_digest.clone())),
            revision_headers.set(&init_rev_digest, init_rev_header),
            revision_metadatas.set(&init_rev_digest, init_rev_meta),
            changesets.insert(&init_changeset_digest, init_changeset),
        );
        result?;

        Ok(Repo {
            head,
            revision_headers,
            revision_metadatas,
            pending_changes: MutableCache::new(storage.clone()),
            staged_changes: MutableCache::new(storage.clone()),
            changesets,
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
            changesets: FrozenCache::new(storage.clone()),
            file_diffs: FrozenCache::new(storage.clone()),
            storage,
        }
    }
}
