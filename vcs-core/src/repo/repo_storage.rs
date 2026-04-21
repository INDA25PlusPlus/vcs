use crate::crypto::CryptoHash;
use crate::diff::hunk_collection::HunkCollection;
use crate::diff::repo_diff::RepoDiff;
use crate::repo::index::Index;
use crate::revision::{RevisionHeader, RevisionId, RevisionMetadata};
use crate::storage::Storage;
use std::error::Error;

pub trait RepoStorage<H: CryptoHash>:
    Storage<RevisionId<H>, RevisionHeader<H>, Error = Self::RepoError>
    + Storage<RevisionId<H>, RevisionMetadata<H>, Error = Self::RepoError>
    + Storage<RevisionId<H>, Index<H>, Error = Self::RepoError>
    + Storage<H, RepoDiff<H>, Error = Self::RepoError>
    + Storage<H, HunkCollection, Error = Self::RepoError>
    + Send
    + Sync
where
    H: Send,
{
    type RepoError: Error + Send;
}
