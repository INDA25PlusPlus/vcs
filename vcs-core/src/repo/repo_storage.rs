use crate::changeset::file::{File, FileDiff, FileDiffRef, FileRef};
use crate::changeset::{Changeset, ChangesetRef};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::repo::{Head, PendingChanges, StagedChanges};
use crate::revision::{RevisionHeader, RevisionMetadata, RevisionRef};
use crate::storage::Storage;
use std::error::Error;

pub trait RepoStorage<D: CryptoDigest + CryptoHash>:
    Storage<(), Head<D>, Error = Self::RepoStorageError>
    + Storage<RevisionRef<D>, RevisionHeader<D>, Error = Self::RepoStorageError>
    + Storage<RevisionRef<D>, RevisionMetadata<D>, Error = Self::RepoStorageError>
    + Storage<RevisionRef<D>, PendingChanges<D>, Error = Self::RepoStorageError>
    + Storage<RevisionRef<D>, StagedChanges<D>, Error = Self::RepoStorageError>
    + Storage<ChangesetRef<D>, Changeset<D>, Error = Self::RepoStorageError>
    + Storage<FileRef<D>, File, Error = Self::RepoStorageError>
    + Storage<FileDiffRef<D>, FileDiff, Error = Self::RepoStorageError>
    + Send
    + Sync
where
    D: Send,
{
    type RepoStorageError: Error + Send;
}
