use super::repo_storage::RepoStorage;
use super::{Repo, RepoError, RepoResult};
use crate::changeset::Changeset;
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::fs::map_ops::DashMapGuard;
use crate::fs::path::RepoPath;
use crate::revision::RevisionRef;
use crate::storage::StorageError;
use crypto_hash_derive::CryptoHash;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::hash::Hash;

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct Head<D: CryptoDigest + CryptoHash>(pub RevisionRef<D>);

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct PendingChanges<D: CryptoDigest + CryptoHash>(pub Changeset<D>);

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct StagedChanges<D: CryptoDigest + CryptoHash>(pub Changeset<D>);

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
        PendingChanges(Changeset::default())
    }
}

impl<D: CryptoDigest + CryptoHash> Default for StagedChanges<D> {
    fn default() -> Self {
        StagedChanges(Changeset::default())
    }
}

#[derive(Clone, Debug)]
pub struct RepoStatus<D: CryptoDigest + CryptoHash> {
    pub staged: StagedChanges<D>,
    pub pending: PendingChanges<D>,
}

impl<D: CryptoDigest + CryptoHash, S> Repo<D, S>
where
    D: Hash + Eq + Clone + Send + Sync,
    S: RepoStorage<D> + Send + Sync,
    S::RepoStorageError: Error + Send,
{
    pub async fn head(&self) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
        let head = self.head.get(&(), async |v| v.clone()).await?;

        Ok(head.0)
    }

    pub async fn set_head(&self, rev: RevisionRef<D>) -> RepoResult<(), S::RepoStorageError> {
        Ok(self.head.update(&(), async |_old_head| Head(rev)).await?)
    }

    pub(super) async fn pending_changes_at(
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

    pub(super) async fn staged_changes_at(
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

    pub(super) async fn update_staged_changes_at(
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
}
