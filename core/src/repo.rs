use crate::commit::{Commit, CommitId};
use crate::diff::{FileDiff, RepoDiff};
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
    commits: LazyStorage<CommitId, Commit<HashType>, S>,
    repo_diffs: LazyStorage<HashType, RepoDiff<HashType>, S>,
    file_diffs: LazyStorage<HashType, FileDiff, S>,
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
    ) -> Option<Result<&Commit<HashType>, S::StorageError>> {
        self.commits.get(&id).await
    }

    // ...
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
