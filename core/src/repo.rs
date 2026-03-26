use crate::commit::{Commit, CommitId};
use crate::diff::{FileDiff, RepoDiff};
use crate::storage::{KeyIndex, LazyStorage, Storage};
use std::hash::Hash;

pub trait RepoStorage<HashType>
where
    HashType: Send,
    Self: Storage<CommitId, Commit<HashType>, Error = Self::StorageError>,
    Self: Storage<HashType, RepoDiff<HashType>, Error = Self::StorageError>,
    Self: Storage<HashType, FileDiff, Error = Self::StorageError>,
    Self: KeyIndex<CommitId, Commit<HashType>, Error = Self::StorageError>,
    Self: KeyIndex<HashType, RepoDiff<HashType>, Error = Self::StorageError>,
    Self: KeyIndex<HashType, FileDiff, Error = Self::StorageError>,
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
    S: RepoStorage<HashType> + Send + Sync + Clone,
    S::StorageError: Send,
{
    pub async fn open(storage: S) -> Result<Self, S::StorageError> {
        // HACK: We should probably not clone thee later
        let commits = LazyStorage::new(storage.clone());
        let repo_diffs = LazyStorage::new(storage.clone());
        let file_diffs = LazyStorage::new(storage.clone());

        for id in <S as KeyIndex<CommitId, Commit<HashType>>>::keys(&storage).await? {
            commits.register(id);
        }
        for hash in <S as KeyIndex<HashType, RepoDiff<HashType>>>::keys(&storage).await? {
            repo_diffs.register(hash);
        }
        for hash in <S as KeyIndex<HashType, FileDiff>>::keys(&storage).await? {
            file_diffs.register(hash);
        }

        Ok(LocalRepo {
            commits,
            repo_diffs,
            file_diffs,
        })
    }

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
