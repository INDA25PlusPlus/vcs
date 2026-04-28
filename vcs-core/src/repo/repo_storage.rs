use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::file_diff::{FileDiff, FileDiffRef};
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::repo::{PendingChanges, StagedChanges};
use crate::revision::{RevisionHeader, RevisionId, RevisionMetadata};
use crate::storage::Storage;
use std::error::Error;

pub trait RepoStorage<D: CryptoDigest + CryptoHash>:
    Storage<(), RevisionId<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, RevisionHeader<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, RevisionMetadata<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, PendingChanges<D>, Error = Self::RepoStorageError>
    + Storage<RevisionId<D>, StagedChanges<D>, Error = Self::RepoStorageError>
    + Storage<RepoDiffRef<D>, RepoDiff<D>, Error = Self::RepoStorageError>
    + Storage<FileDiffRef<D>, FileDiff, Error = Self::RepoStorageError>
    + Send
    + Sync
where
    D: Send,
{
    type RepoStorageError: Error + Send;
}
