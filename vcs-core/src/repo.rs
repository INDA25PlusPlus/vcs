mod head;
mod index;

use crate::commit::{CommitHeader, CommitId, CommitMetadata};
use crate::crypto::CryptoHash;
use crate::diff::{FileDiff, RepoDiff};
use crate::path::RepoPath;
use crate::repo::head::Head;
use crate::repo::index::Index;
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

pub trait RepoStorage<H: CryptoHash>:
    Storage<CommitId, CommitHeader<H>, Error = Self::StorageError>
    + Storage<CommitId, CommitMetadata<H>, Error = Self::StorageError>
    + Storage<CommitId, Index<H>>
    + Storage<H, RepoDiff<H>, Error = Self::StorageError>
    + Storage<H, FileDiff, Error = Self::StorageError>
    + Send
    + Sync
where
    H: Send,
{
    type StorageError: Error + Send;
}

pub struct LocalRepo<'repo, H: CryptoHash, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H>,
    S::StorageError: Error + Send,
{
    storage: S,

    head: Head<'repo, H, S>,

    commit_headers: LazyStorage<CommitId, CommitHeader<H>, S>,
    commit_metadatas: LazyStorage<CommitId, CommitMetadata<H>, S>,

    repo_diffs: LazyStorage<H, RepoDiff<H>, S>,
    file_diffs: LazyStorage<H, FileDiff, S>,
}

impl<'repo, H: CryptoHash, S> LocalRepo<'repo, H, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H> + Send + Sync,
    S::StorageError: Error + Send,
{
    pub fn head(&self) -> &Head<'repo, H, S> {
        &self.head
    }

    pub async fn commit_header(
        &self,
        id: CommitId,
    ) -> RepoResult<&CommitHeader<H>, S::StorageError> {
        repo_result(self.commit_headers.get(id).await)
    }

    pub async fn commit_metadata(
        &self,
        id: CommitId,
    ) -> RepoResult<&CommitMetadata<H>, S::StorageError> {
        repo_result(self.commit_metadatas.get(id).await)
    }

    pub async fn index(&self, id: CommitId) -> RepoResult<&Index<H>, S::StorageError> {
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
