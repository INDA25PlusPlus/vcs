use super::repo_storage::RepoStorage;
use super::{CheckoutError, RefreshPendingChangesError, Repo, RepoError, RepoResult, RestoreError};
use crate::changeset::combine_changesets;
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::diff_policy::DiffPolicy;
use crate::fs::map_ops::DashMapGuard;
use crate::fs::path::RepoPath;
use crate::fs::{FileSystem, FileSystemWriteError, FileTree};
use crate::revision::RevisionRef;
use std::error::Error;
use std::hash::Hash;

impl<D: CryptoDigest + CryptoHash, S> Repo<D, S>
where
    D: Hash + Eq + Clone + Send + Sync,
    S: RepoStorage<D> + Send + Sync,
    S::RepoStorageError: Error + Send,
{
    pub(super) async fn file_tree_at(
        &self,
        rev: &RevisionRef<D>,
    ) -> RepoResult<FileTree<D>, S::RepoStorageError> {
        let mut rev = rev.clone();
        let mut changesets = Vec::new();

        loop {
            let header = self.get_revision_header(&rev).await?;
            let changeset = self
                .changesets
                .get(&header.changeset)
                .await
                .map_err(RepoError::from)?
                .clone();
            changesets.push(changeset);

            if header.parent == D::zero() {
                break;
            }
            rev = header.parent;
        }

        changesets.reverse();
        let combined_changeset = combine_changesets(&changesets, self.storage.as_ref()).await?;
        let combined_changeset = self
            .changesets
            .get(&combined_changeset)
            .await
            .map_err(RepoError::from)?
            .clone();

        FileTree::try_from(combined_changeset).map_err(RepoError::InvalidFileTree)
    }

    pub async fn checkout<F>(
        &self,
        file_system: &mut F,
        rev: RevisionRef<D>,
    ) -> Result<(), CheckoutError<F::Error, S::RepoStorageError>>
    where
        F: FileSystem,
    {
        let new_head_tree = self.file_tree_at(&rev).await?;
        let pending = self.pending_changes_at(&rev).await?;
        let fs_result: Result<(), FileSystemWriteError<F::Error, S::RepoStorageError>> =
            file_system
                .apply_pending_changes(self.storage.as_ref(), &new_head_tree, &pending, true)
                .await;
        fs_result?;

        self.set_head(rev).await?;
        Ok(())
    }

    pub async fn restore<F>(
        &self,
        file_system: &mut F,
        paths: &[RepoPath],
    ) -> Result<(), RestoreError<F::Error, S::RepoStorageError>>
    where
        F: FileSystem,
    {
        let head = self.head().await?;
        let head_tree = self.file_tree_at(&head).await?;
        let mut pending = self.pending_changes_at(&head).await?;

        {
            let pending_changes = DashMapGuard::new(&mut pending.0.changeset);
            for path in paths {
                pending_changes.remove(path);
            }
        }

        file_system
            .apply_pending_changes(self.storage.as_ref(), &head_tree, &pending, true)
            .await?;

        self.pending_changes
            .set(&head, pending)
            .await
            .map_err(RepoError::from)?;

        Ok(())
    }

    pub async fn refresh_pending_changes<F, P>(
        &self,
        file_system: &mut F,
        diff_policy: &P,
    ) -> Result<(), RefreshPendingChangesError<F::Error, S::RepoStorageError>>
    where
        F: FileSystem,
        P: DiffPolicy,
    {
        let head = self.head().await?;
        let head_tree = self.file_tree_at(&head).await?;
        let mut pending = self.pending_changes_at(&head).await?;

        file_system
            .update_pending_changes(
                diff_policy,
                self.storage.as_ref(),
                &head_tree,
                &mut pending,
                true,
            )
            .await?;

        self.pending_changes
            .set(&head, pending)
            .await
            .map_err(RepoError::from)?;

        Ok(())
    }
}
