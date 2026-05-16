pub mod repo_storage;

use crypto_hash_derive::CryptoHash;

use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::fs::file::{FileChange, FileDiff};
use crate::fs::path::RepoPath;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::timestamp::Timestamp;
use crate::revision::{Patch, Revision, RevisionHeader, RevisionId, RevisionMetadata};
use crate::storage::cache::MutableCache;
use crate::storage::{StorageError, cache::FrozenCache};
use std::collections::BTreeMap;
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

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

    pub fn changes(&self) -> BTreeMap<RepoPath, FileChange<D>>
    where
        D: Clone,
    {
        self.0.changes()
    }

    fn get(&self, path: &RepoPath) -> Option<FileChange<D>>
    where
        D: Clone,
    {
        self.0.get(path)
    }

    fn remove(&self, path: &RepoPath) {
        self.0.remove(path);
    }
}

impl<D: CryptoDigest + CryptoHash> StagedChanges<D> {
    pub fn empty() -> StagedChanges<D> {
        StagedChanges(RepoDiff::empty())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn changes(&self) -> BTreeMap<RepoPath, FileChange<D>>
    where
        D: Clone,
    {
        self.0.changes()
    }

    fn set(&self, path: RepoPath, change: FileChange<D>) {
        self.0.set(path, change);
    }

    fn remove(&self, path: &RepoPath) {
        self.0.remove(path);
    }

    fn repo_diff(self) -> RepoDiff<D> {
        self.0
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
    #[error("no staged changes to commit")]
    NoStagedChanges,
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

    pub async fn pending_changes_at(
        &self,
        revision_id: &RevisionId<D>,
    ) -> RepoResult<PendingChanges<D>, S::RepoStorageError> {
        let pending_changes = self
            .pending_changes
            .get(revision_id, async |changes| changes.clone())
            .await?;

        Ok(pending_changes)
    }

    pub async fn set_pending_changes_at(
        &self,
        revision_id: &RevisionId<D>,
        changes: PendingChanges<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        self.pending_changes.set(revision_id, changes).await?;

        Ok(())
    }

    pub async fn staged_changes_at(
        &self,
        revision_id: &RevisionId<D>,
    ) -> RepoResult<StagedChanges<D>, S::RepoStorageError> {
        let staged_changes = self
            .staged_changes
            .get(revision_id, async |changes| changes.clone())
            .await?;

        Ok(staged_changes)
    }

    pub async fn set_staged_changes_at(
        &self,
        revision_id: &RevisionId<D>,
        changes: StagedChanges<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        self.staged_changes.set(revision_id, changes).await?;

        Ok(())
    }

    pub async fn status(&self) -> RepoResult<RepoStatus<D>, S::RepoStorageError> {
        let head = self.head().await?;
        let pending = match self.pending_changes_at(&head).await {
            Ok(changes) => changes,
            Err(RepoError::MissingObject) => PendingChanges::empty(),
            Err(err) => return Err(err),
        };
        let staged = match self.staged_changes_at(&head).await {
            Ok(changes) => changes,
            Err(RepoError::MissingObject) => StagedChanges::empty(),
            Err(err) => return Err(err),
        };

        Ok(RepoStatus { staged, pending })
    }

    pub async fn stage(&self, paths: &[RepoPath]) -> RepoResult<(), S::RepoStorageError> {
        let head = self.head().await?;
        let pending = match self.pending_changes_at(&head).await {
            Ok(changes) => changes,
            Err(RepoError::MissingObject) => PendingChanges::empty(),
            Err(err) => return Err(err),
        };
        let staged = match self.staged_changes_at(&head).await {
            Ok(changes) => changes,
            Err(RepoError::MissingObject) => StagedChanges::empty(),
            Err(err) => return Err(err),
        };

        for path in paths {
            if let Some(change) = pending.get(path) {
                staged.set(path.clone(), change);
            }
        }

        self.set_staged_changes_at(&head, staged).await
    }

    pub async fn unstage(&self, paths: &[RepoPath]) -> RepoResult<(), S::RepoStorageError> {
        let head = self.head().await?;
        let staged = match self.staged_changes_at(&head).await {
            Ok(changes) => changes,
            Err(RepoError::MissingObject) => StagedChanges::empty(),
            Err(err) => return Err(err),
        };

        paths.iter().for_each(|path| staged.remove(path));

        self.set_staged_changes_at(&head, staged).await
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

    pub async fn commit_staged(
        &self,
        message: Box<str>,
        sign_context: SignContext<'_>,
    ) -> RepoResult<RevisionId<D>, S::RepoStorageError> {
        // Load the staged diff for the current head.
        let old_head = self.head().await?;
        let staged = match self.staged_changes_at(&old_head).await {
            Ok(changes) => changes,
            Err(RepoError::MissingObject) => StagedChanges::empty(),
            Err(err) => return Err(err),
        };
        if staged.is_empty() {
            return Err(RepoError::NoStagedChanges);
        }

        // Pending changes are carried forward to the new head, minus changes that were committed.
        let pending = match self.pending_changes_at(&old_head).await {
            Ok(changes) => changes,
            Err(RepoError::MissingObject) => PendingChanges::empty(),
            Err(err) => return Err(err),
        };
        let staged_changes = staged.changes();

        // Store the staged diff and create a committed revision pointing at it.
        let repo_diff_ref = self.insert_repo_diff(staged.repo_diff()).await?;
        let timestamp = Timestamp::now();
        let patch = Patch::new_signed(
            repo_diff_ref.clone(),
            message.clone(),
            timestamp,
            sign_context,
        );
        let mut revision = Revision::from_parts(old_head.clone(), repo_diff_ref, Box::new([patch]));
        revision.commit(message, timestamp, sign_context);
        let revision_id = self.insert_revision(revision).await?;

        // Remove committed changes from pending if they still match the staged value.
        for (path, change) in staged_changes {
            if pending.get(&path).as_ref() == Some(&change) {
                pending.remove(&path);
            }
        }

        // Clear consumed state at the old head and store state for the new head before moving HEAD.
        self.set_staged_changes_at(&old_head, StagedChanges::empty())
            .await?;
        self.set_pending_changes_at(&old_head, PendingChanges::empty())
            .await?;
        self.set_staged_changes_at(&revision_id, StagedChanges::empty())
            .await?;
        self.set_pending_changes_at(&revision_id, pending).await?;
        self.set_head(revision_id.clone()).await?;

        Ok(revision_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::signature::{SignContext, generate_signing_key};
    use crate::storage::memory::MemoryRepoStorage;
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

        repo.set_pending_changes_at(&head, pending).await.unwrap();
        let paths = [foo.clone()];
        repo.stage(&paths).await.unwrap();

        let staged = repo.staged_changes_at(&head).await.unwrap();
        assert_eq!(staged.changes().len(), 1);
        assert_eq!(staged.changes().get(&foo), Some(&foo_change));

        let pending = repo.pending_changes_at(&head).await.unwrap();
        assert_eq!(pending.changes().len(), 2);
        assert_eq!(pending.changes().get(&foo), Some(&foo_change));
        assert_eq!(pending.changes().get(&bar), Some(&bar_change));

        let paths = [foo.clone()];
        repo.unstage(&paths).await.unwrap();
        let staged = repo.staged_changes_at(&head).await.unwrap();
        assert!(staged.is_empty());
    }

    #[tokio::test]
    async fn commit_staged_rejects_empty_staged_changes() {
        let (repo, _) = repo_with_head().await;
        let key_pair = generate_signing_key().unwrap();
        let result = repo
            .commit_staged("commit".into(), SignContext::new(&key_pair))
            .await;

        assert!(matches!(result, Err(RepoError::NoStagedChanges)));
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
        let pending = PendingChanges::empty();
        for (path, change) in changes {
            pending.0.set(path, change);
        }
        pending
    }
}
