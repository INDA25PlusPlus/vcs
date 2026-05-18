pub mod repo_storage;

use crypto_hash_derive::CryptoHash;

use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::fs::file::FileDiff;
use crate::fs::map_ops::DashMapGuard;
use crate::fs::path::RepoPath;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::timestamp::Timestamp;
use crate::revision::{Patch, Revision, RevisionHeader, RevisionMetadata, RevisionRef};
use crate::storage::cache::MutableCache;
use crate::storage::{StorageError, cache::FrozenCache};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;
use tokio::try_join;

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct Head<D: CryptoDigest + CryptoHash>(pub RevisionRef<D>);

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct PendingChanges<D: CryptoDigest + CryptoHash>(pub RepoDiff<D>);

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct StagedChanges<D: CryptoDigest + CryptoHash>(pub RepoDiff<D>);

impl<D: CryptoDigest + CryptoHash> PendingChanges<D> {
    pub fn empty() -> PendingChanges<D> {
        PendingChanges::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<D: CryptoDigest + CryptoHash> StagedChanges<D> {
    pub fn empty() -> StagedChanges<D> {
        StagedChanges::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<D: CryptoDigest + CryptoHash> Default for PendingChanges<D> {
    fn default() -> Self {
        PendingChanges(RepoDiff::default())
    }
}

impl<D: CryptoDigest + CryptoHash> Default for StagedChanges<D> {
    fn default() -> Self {
        StagedChanges(RepoDiff::default())
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
    head: MutableCache<(), Head<D>, S>,

    revision_headers: MutableCache<RevisionRef<D>, RevisionHeader<D>, S>,
    revision_metadatas: MutableCache<RevisionRef<D>, RevisionMetadata<D>, S>,

    pending_changes: MutableCache<RevisionRef<D>, PendingChanges<D>, S>,
    staged_changes: MutableCache<RevisionRef<D>, StagedChanges<D>, S>,

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
            head.set(&(), Head(init_rev_digest.clone())),
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

    pub async fn head(&self) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
        let head = self.head.get(&(), async |v| v.clone()).await?;

        Ok(head.0)
    }

    pub async fn set_head(
        &self,
        _revision_id: RevisionRef<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        todo!("set HEAD and update checkout state in core")
    }

    async fn pending_changes_at(
        &self,
        head: &RevisionRef<D>,
    ) -> RepoResult<PendingChanges<D>, S::RepoStorageError> {
        match self
            .pending_changes
            .get(head, async |pending| pending.clone())
            .await
        {
            Ok(pending) => Ok(pending),
            Err(StorageError::MissingObject) => Ok(PendingChanges::empty()),
            Err(StorageError::InternalError(err)) => Err(RepoError::StorageError(err)),
        }
    }

    async fn staged_changes_at(
        &self,
        head: &RevisionRef<D>,
    ) -> RepoResult<StagedChanges<D>, S::RepoStorageError> {
        match self
            .staged_changes
            .get(head, async |staged| staged.clone())
            .await
        {
            Ok(staged) => Ok(staged),
            Err(StorageError::MissingObject) => Ok(StagedChanges::empty()),
            Err(StorageError::InternalError(err)) => Err(RepoError::StorageError(err)),
        }
    }

    async fn update_staged_changes_at(
        &self,
        head: &RevisionRef<D>,
        f: impl AsyncFnOnce(&StagedChanges<D>) -> StagedChanges<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        Ok(self
            .staged_changes
            .update_or_else(head, f, async |_key| StagedChanges::empty())
            .await?)
    }

    pub async fn status(&self) -> RepoResult<RepoStatus<D>, S::RepoStorageError> {
        let head = self.head().await?;
        let pending = self.pending_changes_at(&head).await?;
        let staged = self.staged_changes_at(&head).await?;
        Ok(RepoStatus { staged, pending })
    }

    pub async fn stage(&self, paths: &[RepoPath]) -> RepoResult<(), S::RepoStorageError> {
        let head = self.head().await?;

        let pending = self.pending_changes_at(&head).await?;

        self.update_staged_changes_at(&head, async |staged| {
            let mut updated_staged = staged.clone();
            {
                let staged_changes = DashMapGuard::new(&mut updated_staged.0.changeset);
                for path in paths {
                    if let Some(change) = pending.0.changeset.get(path) {
                        staged_changes.insert(path.clone(), change.clone());
                    } else {
                        staged_changes.remove(path);
                    }
                }
            }
            updated_staged
        })
        .await?;
        Ok(())
    }

    pub async fn unstage(&self, paths: &[RepoPath]) -> RepoResult<(), S::RepoStorageError> {
        let head = self.head().await?;

        self.update_staged_changes_at(&head, async |staged| {
            let mut updated_staged = staged.clone();
            {
                let staged_changes = DashMapGuard::new(&mut updated_staged.0.changeset);
                for path in paths {
                    staged_changes.remove(path);
                }
            }
            updated_staged
        })
        .await?;
        Ok(())
    }

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
        parent: RevisionRef<D>,
        patches: Box<[Patch<D>]>,
    ) -> RepoResult<Revision<D>, S::RepoStorageError> {
        let repo_diff = todo!("combine patch repo diffs");
        let repo_diff_ref = self.insert_repo_diff(repo_diff).await?;

        Ok(Revision::from_parts(parent, repo_diff_ref, patches))
    }

    pub async fn get_revision_header(
        &self,
        revision_id: &RevisionRef<D>,
    ) -> RepoResult<RevisionHeader<D>, S::RepoStorageError> {
        let header = self
            .revision_headers
            .get(revision_id, async |header| header.clone())
            .await?;

        Ok(header)
    }

    pub async fn get_revision_metadata(
        &self,
        revision_id: &RevisionRef<D>,
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
    ) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
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
        author_message: Box<str>,
        committer_message: Box<str>,
        sign_context: SignContext<'_>,
    ) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
        // Load the staged diff for the current head.
        let old_head = self.head().await?;

        let (mut new_pending, staged) = try_join!(
            // Pending changes are carried forward to the new head and removed from the old head.
            self.pending_changes.replace_or_else(
                &old_head,
                PendingChanges::empty(),
                async |_key| PendingChanges::empty()
            ),
            // replace the staged changes at the old revision with an empty diff
            self.staged_changes
                .replace_or_else(&old_head, StagedChanges::empty(), async |_key| {
                    StagedChanges::empty()
                })
        )?;
        if staged.is_empty() {
            return Err(RepoError::NoStagedChanges);
        }

        {
            let new_pending = DashMapGuard::new(&mut new_pending.0.changeset);
            // todo fix: new pending should be set to:
            // current_working_dir - new_head
            // where current_working_dir = head + pending, new_head = head + staged
            // (`-` is creating a diff, `+` is applying a diff)
            new_pending.retain(|k, _v| !staged.0.changeset.contains_key(k));
        }

        let StagedChanges(repo_diff) = staged;
        let repo_diff_ref = self.insert_repo_diff(repo_diff).await?;

        let timestamp = Timestamp::now();
        let patch = Patch::new_signed(
            repo_diff_ref.clone(),
            author_message,
            timestamp,
            sign_context,
        );

        let mut revision = Revision::from_parts(old_head.clone(), repo_diff_ref, Box::new([patch]));
        revision.commit(committer_message, timestamp, sign_context);
        let revision_id = self.insert_revision(revision).await?;

        try_join!(
            async {
                self.pending_changes
                    .set(&revision_id, new_pending)
                    .await
                    .map_err(RepoError::StorageError)
            },
            // insert empty staged changes
            async {
                self.staged_changes
                    .set(&revision_id, StagedChanges::empty())
                    .await
                    .map_err(RepoError::StorageError)
            },
            self.set_head(revision_id.clone()),
        )?;

        Ok(revision_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::signature::{SignContext, generate_signing_key};
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

        repo.pending_changes.set(&head, pending).await.unwrap();
        let paths = [foo.clone()];
        repo.stage(&paths).await.unwrap();

        let staged = repo.staged_changes_at(&head).await.unwrap();
        assert_eq!(staged.0.changeset.len(), 1);
        assert_eq!(staged.0.changeset.get(&foo), Some(&foo_change));

        let pending = repo.pending_changes_at(&head).await.unwrap();
        assert_eq!(pending.0.changeset.len(), 2);
        assert_eq!(pending.0.changeset.get(&foo), Some(&foo_change));
        assert_eq!(pending.0.changeset.get(&bar), Some(&bar_change));

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
            .commit_staged(
                "author".into(),
                "commit".into(),
                SignContext::new(&key_pair),
            )
            .await;

        assert!(matches!(result, Err(RepoError::NoStagedChanges)));
    }

    async fn repo_with_head() -> (Repo<Digest, TestStorage>, Digest) {
        // Initialize manually to avoid hitting todo forn ow
        let storage = Arc::new(TestStorage::new());
        let head = Head(blake3::hash(b"head"));
        <TestStorage as crate::storage::Storage<(), Head<Digest>>>::store(
            storage.as_ref(),
            &(),
            &head,
        )
        .await
        .unwrap();
        let repo = Repo::load(storage).await;
        (repo, head.0)
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
