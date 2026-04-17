use crate::commit::{CommitHeader, CommitId, CommitMetadata};
use crate::crypto::CryptoHash;
use crate::diff::file_diff::FileDiff;
use crate::diff::repo_diff::RepoDiff;
use crate::repo::index::Index;
use crate::storage::Storage;
use std::error::Error;

pub trait RepoStorage<H: CryptoHash>:
    Storage<CommitId, CommitHeader<H>, Error = Self::RepoError>
    + Storage<CommitId, CommitMetadata<H>, Error = Self::RepoError>
    + Storage<CommitId, Index<H>, Error = Self::RepoError>
    + Storage<H, RepoDiff<H>, Error = Self::RepoError>
    + Storage<H, FileDiff, Error = Self::RepoError>
    + Send
    + Sync
where
    H: Send,
{
    type RepoError: Error + Send;
}
