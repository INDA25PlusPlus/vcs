mod index;
pub mod repo_storage;

use crate::crypto::digest::CryptoDigest;
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

pub struct Repository<D: CryptoDigest, S>
where
    D: Hash + Eq + Send + Sync,
    S: RepoStorage<D>,
    S::RepoError: Error + Send,
{
    storage: Arc<S>,

    head: RevisionId<D>,

    revision_headers: LazyStorage<RevisionId<D>, RevisionHeader<D>, S>,
    revision_metadatas: LazyStorage<RevisionId<D>, RevisionMetadata<D>, S>,

    repo_diffs: LazyStorage<D, RepoDiff<D>, S>,
    file_diffs: LazyStorage<D, FileDiff, S>,
}

impl<D: CryptoDigest, S> Repository<D, S>
where
    D: Hash + Eq + Send + Sync,
    S: RepoStorage<D> + Send + Sync,
    S::RepoError: Error + Send,
{
    pub fn head(&self) -> &RevisionId<D> {
        &self.head
    }

    pub async fn revision_header(
        &self,
        id: RevisionId<D>,
    ) -> StorageResult<&RevisionHeader<D>, S::RepoError> {
        self.revision_headers.get(id).await
    }

    pub async fn commit_metadata(
        &self,
        id: RevisionId<D>,
    ) -> StorageResult<&RevisionMetadata<D>, S::RepoError> {
        self.revision_metadatas.get(id).await
    }

    /// Generates a new repo diff from a series of patches, stores it to storage and returns its
    /// hash.
    pub async fn squash(&self, patches: &[Patch<D>]) -> RepoDiffRef<D> {
        todo!()
    }

    pub async fn index(&self, id: RevisionId<D>) -> StorageResult<&Index<D>, S::RepoError> {
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

    pub async fn checkout(&mut self, commit_id: &RevisionId<D>) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
