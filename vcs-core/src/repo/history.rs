use super::repo_storage::RepoStorage;
use super::{Repo, RepoError, RepoResult};
use crate::changeset::{Changeset, ChangesetRef, combine_changesets};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::fs::FileTree;
use crate::revision::{Patch, Revision, RevisionHeader, RevisionMetadata, RevisionRef};
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

    pub async fn insert_changeset(
        &self,
        changeset: Changeset<D>,
    ) -> RepoResult<ChangesetRef<D>, S::RepoStorageError> {
        let changeset_digest = changeset.to_digest();
        self.changesets.insert(&changeset_digest, changeset).await?;

        Ok(changeset_digest)
    }

    pub async fn create_revision(
        &self,
        parent: RevisionRef<D>,
        patches: Box<[Patch<D>]>,
    ) -> RepoResult<Revision<D>, S::RepoStorageError> {
        let mut changesets = Vec::with_capacity(patches.len());
        for patch in patches.iter() {
            changesets.push(self.changesets.get(patch.changeset()).await?.clone());
        }
        let changeset_digest = combine_changesets(&changesets, self.storage.as_ref()).await?;

        Ok(Revision::from_parts(parent, changeset_digest, patches))
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
        self.changesets.get(&header.changeset).await?;

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
