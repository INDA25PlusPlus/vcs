use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
use crate::fs::file::{File, FileDiff, FileDiffRef, FileRef};
use crate::repo::{PendingChanges, StagedChanges};
use crate::revision::{RevisionHeader, RevisionMetadata, RevisionRef};
use crate::storage::Storage;
use std::error::Error;

pub trait RepoStorage<D: CryptoDigest + CryptoHash>:
    Storage<(), RevisionRef<D>, Error = Self::RepoStorageError>
    + Storage<RevisionRef<D>, RevisionHeader<D>, Error = Self::RepoStorageError>
    + Storage<RevisionRef<D>, RevisionMetadata<D>, Error = Self::RepoStorageError>
    + Storage<RevisionRef<D>, PendingChanges<D>, Error = Self::RepoStorageError>
    + Storage<RevisionRef<D>, StagedChanges<D>, Error = Self::RepoStorageError>
    + Storage<RepoDiffRef<D>, RepoDiff<D>, Error = Self::RepoStorageError>
    + Storage<FileRef<D>, File, Error = Self::RepoStorageError>
    + Storage<FileDiffRef<D>, FileDiff, Error = Self::RepoStorageError>
    + Send
    + Sync
where
    D: Send,
{
    type RepoStorageError: Error + Send;
}
