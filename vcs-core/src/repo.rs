mod head;
mod index;

use crate::commit::{CommitHeader, CommitId, CommitMetadata};
use crate::crypto::CryptoHash;
use crate::diff::file_diff::FileDiff;
use crate::diff::repo_diff::RepoDiff;
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
    #[error("commits missing common ancestor")]
    MissingAncestor,
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

    async fn get_commits_lca(
        &self,
        id_1: CommitId,
        id_2: CommitId,
    ) -> Result<CommitId, RepoError<Self::StorageError>> {
        if id_1 == id_2 {
            return Ok(id_1);
        }

        let load_two_commit_headers = async |commit_1: CommitId,
                                             commit_2: CommitId|
               -> Result<
            (CommitHeader<H>, CommitHeader<H>),
            RepoError<Self::StorageError>,
        > {
            let header_1 = <Self as Storage<CommitId, CommitHeader<H>>>::load(&self, &commit_1);
            let header_2 = <Self as Storage<CommitId, CommitHeader<H>>>::load(&self, &commit_2);

            let (header_1, header_2) =
                tokio::try_join!(header_1, header_2).map_err(|err| RepoError::StorageError(err))?;
            let header_1 = header_1.ok_or(RepoError::MissingObject)?;
            let header_2 = header_2.ok_or(RepoError::MissingObject)?;

            Ok((header_1, header_2))
        };

        let (header_1, header_2) = load_two_commit_headers(id_1, id_2).await?;

        let (mut header_deep, mut header_shallow, id_shallow) = if header_1.depth > header_2.depth {
            (header_1, header_2, id_2)
        } else {
            (header_2, header_1, id_1)
        };

        while header_deep.depth > header_shallow.depth && header_deep.parent_id != id_shallow {
            header_deep =
                <Self as Storage<CommitId, CommitHeader<H>>>::load(&self, &header_deep.parent_id)
                    .await
                    .map_err(|err| RepoError::StorageError(err))?
                    .ok_or(RepoError::MissingObject)?;
        }

        if header_deep.parent_id == id_shallow {
            return Ok(id_shallow);
        }

        while header_deep.parent_id != header_shallow.parent_id && header_deep.depth > 0 {
            (header_deep, header_shallow) =
                load_two_commit_headers(header_deep.parent_id, header_shallow.parent_id).await?;
        }

        // This being false would imply the commits don't have a shared ancestor, which should be impossible
        // TODO: return error maybe?
        debug_assert!(header_deep.depth > 0);

        Ok(header_deep.parent_id)
    }
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
