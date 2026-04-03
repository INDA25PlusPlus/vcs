use crate::commit::{CommitHeader, CommitId, CommitMetadata};
use crate::crypto::CryptoHash;
use crate::repo::{LocalRepo, RepoResult, RepoStorage, repo_result};
use std::error::Error;
use std::hash::Hash;

/// Stores the ID of the commit which HEAD is currently pointing to, as well as lazily evaluated
/// reference to the commit's header and metadata fields.
pub struct Head<'repo, H: CryptoHash, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H>,
    S::StorageError: Error + Send,
{
    id: CommitId,
    commit_header: tokio::sync::OnceCell<&'repo CommitHeader<H>>,
    commit_metadata: tokio::sync::OnceCell<&'repo CommitMetadata<H>>,
    repo: &'repo LocalRepo<'repo, H, S>,
}

impl<'repo, H: CryptoHash, S, E> Head<'repo, H, S>
where
    H: Hash + Eq + Send + Sync,
    S: RepoStorage<H, StorageError = E>,
    S::StorageError: Error + Send,
{
    /// ID of commit which HEAD is pointing to
    pub fn id(&self) -> CommitId {
        self.id
    }

    /// Header of commit which HEAD is pointing to
    pub async fn commit_header(&self) -> RepoResult<&'repo CommitHeader<H>, E> {
        self.commit_header
            .get_or_try_init(async || repo_result(self.repo.commit_headers.get(self.id).await))
            .await
            .copied()
    }

    /// Metadata of commit which HEAD is pointing to
    pub async fn commit_metadata(&self) -> RepoResult<&'repo CommitMetadata<H>, E> {
        self.commit_metadata
            .get_or_try_init(async || repo_result(self.repo.commit_metadatas.get(self.id).await))
            .await
            .copied()
    }
}
