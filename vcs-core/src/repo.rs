mod index;
pub mod repo_storage;

use crate::crypto::CryptoHash;
use crate::diff::file_diff::FileDiff;
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::path::RepoPath;
use crate::repo::index::Index;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::{Patch, RevisionHeader, RevisionId, RevisionMetadata};
use crate::storage::{LazyStorage, StorageResult};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

pub struct Repository<H: CryptoHash, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H>,
    S::RepoError: Error + Send,
{
    storage: Arc<S>,

    head: RevisionId<H>,

    revision_headers: LazyStorage<RevisionId<H>, RevisionHeader<H>, S>,
    revision_metadatas: LazyStorage<RevisionId<H>, RevisionMetadata<H>, S>,

    repo_diffs: LazyStorage<H, RepoDiff<H>, S>,
    file_diffs: LazyStorage<H, FileDiff, S>,
}

impl<H: CryptoHash, S> Repository<H, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H> + Send + Sync,
    S::RepoError: Error + Send,
{
    pub fn head(&self) -> &RevisionId<H> {
        &self.head
    }

    pub async fn revision_header(
        &self,
        id: RevisionId<H>,
    ) -> StorageResult<&RevisionHeader<H>, S::RepoError> {
        self.revision_headers.get(id).await
    }

    pub async fn commit_metadata(
        &self,
        id: RevisionId<H>,
    ) -> StorageResult<&RevisionMetadata<H>, S::RepoError> {
        self.revision_metadatas.get(id).await
    }

    /// Generates a new repo diff from a series of patches, stores it to storage and returns its
    /// hash.
    pub async fn squash(&self, patches: &[Patch<H>]) -> RepoDiffRef<H> {
        todo!()
    }

    pub async fn index(&self, id: RevisionId<H>) -> StorageResult<&Index<H>, S::RepoError> {
        todo!()
        // repo_result(<S as Storage<CommitId, Index<H>>>::load(self, &id).await)
    }

    pub async fn create_patch(&mut self, message: String) {
        todo!()
    }

    pub async fn stage(&mut self, repo_path: &RepoPath) {
        todo!()
    }

    pub async fn unstage(&mut self, repo_path: &RepoPath) {
        todo!()
    }

    pub async fn checkout(&mut self, commit_id: &RevisionId<H>) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
