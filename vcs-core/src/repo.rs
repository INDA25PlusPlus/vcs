mod index;
pub mod repo_storage;

use crate::commit::{CommitHeader, CommitId, CommitMetadata};
use crate::crypto::CryptoHash;
use crate::diff::file_diff::FileDiff;
use crate::diff::repo_diff::RepoDiff;
use crate::path::RepoPath;
use crate::repo::index::Index;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::{LazyStorage, Storage};
use std::error::Error;
use std::hash::Hash;

pub type RepoResult<T, E: Error + Send> = Result<T, RepoError<E>>;

#[derive(thiserror::Error, Debug)]
pub enum RepoError<E: Error + Send> {
    #[error("storage error: {0}")]
    StorageError(E),
    #[error("object missing from database")]
    MissingObject,
}

fn repo_result<T, E: Error>(result: Result<Option<T>, E>) -> RepoResult<T, E>
where
    E: Send,
{
    match result {
        Ok(Some(ok)) => Ok(ok),
        Ok(None) => Err(RepoError::MissingObject),
        Err(err) => Err(RepoError::StorageError(err)),
    }
}

pub struct LocalRepo<H: CryptoHash, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H>,
    S::RepoError: Error + Send,
{
    storage: S,

    head: CommitId,

    commit_headers: LazyStorage<CommitId, CommitHeader<H>, S>,
    commit_metadatas: LazyStorage<CommitId, CommitMetadata<H>, S>,

    repo_diffs: LazyStorage<H, RepoDiff<H>, S>,
    file_diffs: LazyStorage<H, FileDiff, S>,
}

impl<'repo, H: CryptoHash, S> LocalRepo<H, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H> + Send + Sync,
    S::RepoError: Error + Send,
{
    pub fn head(&self) -> &CommitId {
        &self.head
    }

    pub async fn commit_header(&self, id: CommitId) -> RepoResult<&CommitHeader<H>, S::RepoError> {
        repo_result(self.commit_headers.get(id).await)
    }

    pub async fn commit_metadata(
        &self,
        id: CommitId,
    ) -> RepoResult<&CommitMetadata<H>, S::RepoError> {
        repo_result(self.commit_metadatas.get(id).await)
    }

    pub async fn index(&self, id: CommitId) -> RepoResult<&Index<H>, S::RepoError> {
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
