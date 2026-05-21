use super::repo_storage::RepoStorage;
use super::{PendingChanges, Repo, RepoError, RepoResult, StagedChanges};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::fs::map_ops::DashMapGuard;
use crate::revision::Patch;
use crate::revision::RevisionRef;
use crate::revision::timestamp::Timestamp;
use std::error::Error;
use std::hash::Hash;
use tokio::try_join;

impl<D: CryptoDigest + CryptoHash, S> Repo<D, S>
where
    D: Hash + Eq + Clone + Send + Sync,
    S: RepoStorage<D> + Send + Sync,
    S::RepoStorageError: Error + Send,
{
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

        let StagedChanges(changeset) = staged;
        let changeset_digest = self.insert_changeset(changeset).await?;

        let timestamp = Timestamp::now();
        let patch = Patch::new_signed(changeset_digest, author_message, timestamp, sign_context);

        let mut revision = self
            .create_revision(old_head.clone(), Box::new([patch]))
            .await?;
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
