pub mod repo_storage;

use crypto_hash_derive::CryptoHash;

use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::fs::file::FileDiff;
use crate::fs::map_ops::DashMapGuard;
use crate::fs::path::RepoPath;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::{Patch, Revision, RevisionHeader, RevisionId, RevisionMetadata};
use crate::storage::cache::MutableCache;
use crate::storage::{StorageError, cache::FrozenCache};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

#[derive(Clone, CryptoHash, Debug)]
pub struct Head<D: CryptoDigest + CryptoHash>(pub RevisionId<D>);

#[derive(Clone, CryptoHash, Debug)]
pub struct PendingChanges<D: CryptoDigest + CryptoHash>(pub RepoDiff<D>);

#[derive(Clone, CryptoHash, Debug)]
pub struct StagedChanges<D: CryptoDigest + CryptoHash>(pub RepoDiff<D>);

impl<D: CryptoDigest + CryptoHash> PendingChanges<D> {
    pub fn empty() -> PendingChanges<D> {
        PendingChanges(RepoDiff::empty())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<D: CryptoDigest + CryptoHash> StagedChanges<D> {
    pub fn empty() -> StagedChanges<D> {
        StagedChanges(RepoDiff::empty())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct RepoStatus<D: CryptoDigest + CryptoHash> {
    pub staged: StagedChanges<D>,
    pub pending: PendingChanges<D>,
}

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
        let init_rev: Revision<D> = Revision::new_initial(sign_context);
        // UGLY: This duplicates the empty diff created by `Revision::new_initial`.
        // The two must hash identically so the stored diff matches the initial revision header.
        let init_repo_diff = RepoDiff::<D>::empty();
        let init_repo_diff_ref = init_rev.header().repo_diff.clone();
        let init_rev_digest: D = init_rev.to_digest();

        let (init_rev_header, init_rev_meta) = init_rev.into_parts();

        let head = MutableCache::new(storage.clone());
        let revision_headers = MutableCache::new(storage.clone());
        let revision_metadatas = MutableCache::new(storage.clone());
        let repo_diffs = FrozenCache::new(storage.clone());

        let result: Result<_, S::RepoStorageError> = tokio::try_join!(
            head.set(&(), init_rev_digest.clone()),
            revision_headers.set(&init_rev_digest, init_rev_header),
            revision_metadatas.set(&init_rev_digest, init_rev_meta),
            repo_diffs.insert(&init_repo_diff_ref, init_repo_diff),
        );
        result?;

        Ok(Repo {
            head,
            revision_headers,
            revision_metadatas,
            pending_changes: MutableCache::new(storage.clone()),
            staged_changes: MutableCache::new(storage.clone()),
            repo_diffs,
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

    async fn pending_changes_at<R>(
        &self,
        head: &RevisionId<D>,
        f: impl AsyncFnOnce(&mut PendingChanges<D>) -> R,
    ) -> RepoResult<R, S::RepoStorageError> {
        Ok(self
            .pending_changes
            .get_mut_or_default(head, f, async |_key| PendingChanges(RepoDiff::empty()))
            .await?)
    }

    async fn staged_changes_at<R>(
        &self,
        head: &RevisionId<D>,
        f: impl AsyncFnOnce(&mut StagedChanges<D>) -> R,
    ) -> RepoResult<R, S::RepoStorageError> {
        Ok(self
            .staged_changes
            .get_mut_or_default(head, f, async |_key| StagedChanges(RepoDiff::empty()))
            .await?)
    }

    pub async fn status(&self) -> RepoResult<RepoStatus<D>, S::RepoStorageError> {
        let head = self.head().await?;
        let pending = self
            .pending_changes_at(&head, async |pending| pending.clone())
            .await?;
        let staged = self
            .staged_changes_at(&head, async |staged| staged.clone())
            .await?;
        Ok(RepoStatus { staged, pending })
    }

    pub async fn stage(&self, paths: &[RepoPath]) -> RepoResult<(), S::RepoStorageError> {
        let head = self.head().await?;

        let pending = self
            .pending_changes_at(&head, async |pending| pending.clone())
            .await?;

        self.staged_changes_at(&head, async |staged| {
            let staged = DashMapGuard::new(&mut staged.0.changeset);
            for path in paths {
                if let Some(change) = pending.0.changeset.get(path) {
                    staged.insert(path.clone(), change.clone());
                } else {
                    staged.remove(path);
                }
            }
        })
        .await?;
        Ok(())
    }

    pub async fn unstage(&self, paths: &[RepoPath]) -> RepoResult<(), S::RepoStorageError> {
        let head = self.head().await?;

        self.staged_changes_at(&head, async |staged: &mut StagedChanges<D>| {
            let staged = DashMapGuard::new(&mut staged.0.changeset);
            for path in paths {
                staged.remove(path);
            }
        })
        .await?;
        Ok(())
    }

    // pub async fn get_diff(
    //     &self,
    //     repo_diff_ref: RepoDiffRef<D>,
    // ) -> RepoResult<&RepoDiff<D>, S::RepoStorageError> {
    //     todo!()
    // }

    pub async fn insert_repo_diff(
        &self,
        repo_diff: RepoDiff<D>,
    ) -> RepoResult<RepoDiffRef<D>, S::RepoStorageError> {
        let repo_diff_ref = repo_diff.to_digest();
        self.repo_diffs.insert(&repo_diff_ref, repo_diff).await?;

        Ok(repo_diff_ref)
    }

    #[allow(clippy::diverging_sub_expression)]
    pub async fn create_revision(
        &self,
        parent: RevisionId<D>,
        patches: Box<[Patch<D>]>,
    ) -> RepoResult<Revision<D>, S::RepoStorageError> {
        let repo_diff = todo!("combine patch repo diffs");
        let repo_diff_ref = self.insert_repo_diff(repo_diff).await?;

        Ok(Revision::from_parts(parent, repo_diff_ref, patches))
    }

    pub async fn get_revision_header(
        &self,
        revision_id: &RevisionId<D>,
    ) -> RepoResult<RevisionHeader<D>, S::RepoStorageError> {
        let header = self
            .revision_headers
            .get(revision_id, async |header| header.clone())
            .await?;

        Ok(header)
    }

    pub async fn get_revision_metadata(
        &self,
        revision_id: &RevisionId<D>,
    ) -> RepoResult<RevisionMetadata<D>, S::RepoStorageError> {
        let metadata = self
            .revision_metadatas
            .get(revision_id, async |metadata| metadata.clone())
            .await?;

        Ok(metadata)
    }

    pub async fn insert_revision(
        &self,
        revision: Revision<D>,
    ) -> RepoResult<RevisionId<D>, S::RepoStorageError> {
        let header = revision.header();

        // Verify parent exists in storage
        self.revision_headers
            .get(&header.parent, async |_| ())
            .await?;

        // Verify repo diff exists in storage
        self.repo_diffs.get(&header.repo_diff).await?;

        // TODO: Check that `header.repo_diff` applies cleanly to `header.parent`.

        let revision_id = revision.to_digest();
        let (header, metadata) = revision.into_parts();

        // TODO: Make revision insertion atomic so storage cannot retain only one of these parts.
        // If one fails it can cause storage to be out of sync
        tokio::try_join!(
            self.revision_headers.set(&revision_id, header),
            self.revision_metadatas.set(&revision_id, metadata),
        )?;

        Ok(revision_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::file::FileChange;
    use crate::storage::memory::MemoryRepoStorage;
    use dashmap::DashMap;
    use std::sync::Arc;

    type Digest = blake3::Hash;
    type TestStorage = MemoryRepoStorage<Digest>;

    #[tokio::test]
    async fn stage_and_unstage_selected_path_round_trip() {
        let (repo, head) = repo_with_head().await;
        let foo = RepoPath::try_from("foo.txt").unwrap();
        let bar = RepoPath::try_from("bar.txt").unwrap();
        let foo_change = FileChange::Create(blake3::hash(b"foo"));
        let bar_change = FileChange::Delete;
        let pending = pending_changes([
            (foo.clone(), foo_change.clone()),
            (bar.clone(), bar_change.clone()),
        ]);

        repo.pending_changes_at(&head, async |entry| *entry = pending)
            .await
            .unwrap();
        let paths = [foo.clone()];
        repo.stage(&paths).await.unwrap();

        let staged = repo
            .staged_changes_at(&head, async |entry| entry.clone())
            .await
            .unwrap();
        assert_eq!(staged.0.changeset.len(), 1);
        assert_eq!(staged.0.changeset.get(&foo), Some(&foo_change));

        let pending = repo
            .pending_changes_at(&head, async |entry| entry.clone())
            .await
            .unwrap();
        assert_eq!(pending.0.changeset.len(), 2);
        assert_eq!(pending.0.changeset.get(&foo), Some(&foo_change));
        assert_eq!(pending.0.changeset.get(&bar), Some(&bar_change));

        let paths = [foo.clone()];
        repo.unstage(&paths).await.unwrap();
        let staged = repo
            .staged_changes_at(&head, async |entry| entry.clone())
            .await
            .unwrap();
        assert!(staged.is_empty());
    }

    async fn repo_with_head() -> (Repo<Digest, TestStorage>, Digest) {
        let repo = Repo::load(Arc::new(TestStorage::new())).await;
        let head = blake3::hash(b"head");
        repo.set_head(head).await.unwrap();
        (repo, head)
    }

    fn pending_changes(
        changes: impl IntoIterator<Item = (RepoPath, FileChange<Digest>)>,
    ) -> PendingChanges<Digest> {
        let changes: DashMap<_, _> = changes.into_iter().collect();
        PendingChanges(RepoDiff {
            changeset: changes.into_read_only(),
        })
    }
}
