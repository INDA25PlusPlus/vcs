use crate::commit::{Commit, CommitId};
use crate::diff::{FileDiff, RepoDiff};
use crate::path::RepoPath;
use crate::storage::{LazyStorage, Storage};
use std::hash::Hash;

pub trait RepoStorage<HashType>:
    Storage<CommitId, Commit<HashType>, Error = Self::StorageError>
    + Storage<HashType, RepoDiff<HashType>, Error = Self::StorageError>
    + Storage<HashType, FileDiff, Error = Self::StorageError>
where
    HashType: Send,
{
    type StorageError;
}

pub struct LocalRepo<HashType, S: RepoStorage<HashType>>
where
    HashType: Hash + Eq + Send + Sync,
{
    head: CommitId,
    commits: LazyStorage<CommitId, Commit<HashType>, S>,
    repo_diffs: LazyStorage<HashType, RepoDiff<HashType>, S>,
    file_diffs: LazyStorage<HashType, FileDiff, S>,
    indicies: LazyStorage<HashType, RepoDiff<HashType>, S>,
}

impl<HashType, S> LocalRepo<HashType, S>
where
    HashType: Hash + Eq + Send + Sync,
    S: RepoStorage<HashType> + Send + Sync,
    S::StorageError: Send,
{
    pub async fn get_commit(
        &self,
        id: CommitId,
    ) -> Result<Option<&Commit<HashType>>, S::StorageError> {
        self.commits.get(id).await
    }

    pub async fn head_commit(&self) -> &Commit<HashType> {
        todo!()
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

    async fn transition_diff(&self, from: &CommitId, to: &CommitId) -> RepoDiff<HashType> {
        todo!()
    }

    pub fn head(&self) -> &CommitId {
        &self.head
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
