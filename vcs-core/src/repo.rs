mod index;
pub mod repo_storage;

use crate::commit::{CommitHeader, CommitId, CommitMetadata};
use crate::crypto::CryptoHash;
use crate::diff::file_diff::FileDiff;
use crate::diff::repo_diff::RepoDiff;
use crate::path::RepoPath;
use crate::repo::index::Index;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::{LazyStorage, StorageResult};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct RepoHeader {
    pub head: CommitId,
    pub next_commit_id: CommitId,
}

pub struct LocalRepo<H: CryptoHash, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H>,
    S::RepoError: Error + Send,
{
    storage: Arc<S>,

    header: RepoHeader,

    commit_headers: LazyStorage<CommitId, CommitHeader<H>, S>,
    commit_metadatas: LazyStorage<CommitId, CommitMetadata<H>, S>,

    repo_diffs: LazyStorage<H, RepoDiff<H>, S>,
    file_diffs: LazyStorage<H, FileDiff, S>,
}

impl<H: CryptoHash, S> LocalRepo<H, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H> + Send + Sync,
    S::RepoError: Error + Send,
{
    pub fn head(&self) -> &CommitId {
        &self.header.head
    }

    pub async fn commit_header(
        &self,
        id: CommitId,
    ) -> StorageResult<&CommitHeader<H>, S::RepoError> {
        self.commit_headers.get(id).await
    }

    pub async fn commit_metadata(
        &self,
        id: CommitId,
    ) -> StorageResult<&CommitMetadata<H>, S::RepoError> {
        self.commit_metadatas.get(id).await
    }

    pub async fn index(&self, id: CommitId) -> StorageResult<&Index<H>, S::RepoError> {
        todo!()
        // repo_result(<S as Storage<CommitId, Index<H>>>::load(self, &id).await)
    }

    pub async fn make_commit(&mut self, message: String) {
        todo!()
    }

    pub async fn stage(&mut self, repo_path: &RepoPath) {
        todo!()
    }

    pub async fn unstage(&mut self, repo_path: &RepoPath) {
        todo!()
    }

    pub async fn checkout(&mut self, commit_id: &CommitId) {
        todo!()
    }

    async fn transition_diff(&self, from: &CommitId, to: &CommitId) -> RepoDiff<H> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
