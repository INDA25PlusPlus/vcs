use crate::commit::{Commit, CommitId};
use crate::diff::{FileDiff, RepoDiff};
use crate::path::RepoPath;
use crate::storage::Storage;
use crate::storage::lazy::{Evaluator, LazyCache, LazyStorage};
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

pub struct LocalRepo<'a, HashType, S: RepoStorage<HashType>>
where
    HashType: Hash + Eq + Send + Sync,
{
    commits: LazyStorage<CommitId, Commit<HashType>, S>,
    repo_diffs: LazyStorage<HashType, RepoDiff<HashType>, S>,
    file_diffs: LazyStorage<HashType, FileDiff, S>,

    file_tree: LazyCache<(CommitId, RepoPath), Box<[u8]>, LocalRepoEvaluator<'a, HashType, S>>,
}

pub struct LocalRepoEvaluator<'a, HashType, S: RepoStorage<HashType>>
where
    HashType: Hash + Eq + Send + Sync,
{
    commits: &'a LazyStorage<CommitId, Commit<HashType>, S>,
    repo_diffs: &'a LazyStorage<HashType, RepoDiff<HashType>, S>,
    file_diffs: &'a LazyStorage<HashType, FileDiff, S>,
    // ...
}

impl<'a, HashType, S: RepoStorage<HashType>> Evaluator<(CommitId, RepoPath), Box<[u8]>>
    for LocalRepoEvaluator<'a, HashType, S>
where
    HashType: Hash + Eq + Send + Sync,
{
    async fn evaluate(&self, key: &(CommitId, RepoPath)) -> Option<Box<[u8]>> {
        let (commit_id, file_path) = key;
        todo!()
    }
}

impl<'a, HashType, S> LocalRepo<'a, HashType, S>
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

    // ...
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
