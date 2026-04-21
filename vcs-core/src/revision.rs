//! Module for creating and modifying revisions.
//!
//! A revision is a certain version of the repository, containing a set of changes introduced since
//! the previous revision. In this way it is similar to a Git commit, although a revision differs in
//! that it may be either committed or uncommitted. An uncommitted revision represents a set of
//! changes and author metadata not yet pushed to the upstream, while a committed revision
//! has been either pushed to or pulled from the upstream. Furthermore, a committed revision is
//! considered permanent and can not be rebased, squashed or otherwise edited.

use crate::crypto::{CryptoHash, SignContext};
use crate::crypto_hash;
use crate::diff::repo_diff::RepoDiffRef;
use crate::repo::Repository;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::author::{Author, AuthorSignature, Committer};
use crate::revision::timestamp::Timestamp;
use std::hash::Hash;

pub mod author;
pub mod timestamp;

pub type RevisionId<H> = H;

pub type FormatVersion = u16;

/// The current data format version for revisions.
pub const FORMAT_VERSION: FormatVersion = 0;

#[derive(Clone, Debug)]
pub struct Patch<H: CryptoHash> {
    repo_diff: RepoDiffRef<H>,
    author: Author<H>,
}

#[derive(Clone, Debug)]
pub struct RevisionHeader<H: CryptoHash> {
    pub repo_diff: RepoDiffRef<H>,
    pub parent: RevisionId<H>,
}

#[derive(Clone, Debug)]
pub struct RevisionMetadata<H: CryptoHash> {
    pub version: FormatVersion,
    pub patches: Box<[Patch<H>]>,
    pub committer: Option<Committer<H>>,
}

/// In-memory representation of a revision, separated into a `header` and `metadata` for efficient
/// loading of the most significant fields `repo_diff` and `parent`.
///
/// A value of this type is guaranteed to be a valid revision with valid hashes and signatures.
#[derive(Clone, Debug)]
pub struct Revision<H: CryptoHash> {
    header: RevisionHeader<H>,
    metadata: RevisionMetadata<H>,
}

impl<H: CryptoHash> Patch<H> {
    pub fn new_signed(
        repo_diff: RepoDiffRef<H>,
        message: String,
        timestamp: Timestamp,
        sign_context: SignContext,
    ) -> Patch<H> {
        let pre_signed = todo!(); // crypto_hash!(H; repo_diff, message, timestamp);
        let signature = sign_context.sign(pre_signed);
        Patch {
            repo_diff,
            author: Author {
                message,
                timestamp,
                signature: AuthorSignature::Signature(signature),
            },
        }
    }
}

impl<H: CryptoHash> Revision<H>
where
    H: Eq + Hash + Send + Sync,
{
    pub async fn new<S: RepoStorage<H>>(
        repo: &Repository<H, S>,
        parent: RevisionId<H>,
        patches: Box<[Patch<H>]>,
        timestamp: Timestamp,
    ) -> Revision<H> {
        let repo_diff = repo.squash(&patches).await;
        Revision {
            header: RevisionHeader { repo_diff, parent },
            metadata: RevisionMetadata {
                version: FORMAT_VERSION,
                patches,
                committer: None,
            },
        }
    }

    pub fn header(&self) -> &RevisionHeader<H> {
        &self.header
    }

    pub fn metadata(&self) -> &RevisionMetadata<H> {
        &self.metadata
    }

    /// Splits `self` into its storable parts. The header and metadata are guaranteed to be valid
    /// with respect to each other.
    pub fn into_parts(self) -> (RevisionHeader<H>, RevisionMetadata<H>) {
        (self.header, self.metadata)
    }

    pub fn is_committed(&self) -> bool {
        self.metadata.committer.is_some()
    }

    /// Returns the combined hash of this revision's patches and parent ID.
    pub fn revision_hash(&self) -> H {
        crypto_hash!(H; self.metadata.version, self.header.parent_hash, self.metadata.patches)
    }

    /// Commits this revision, overwriting any previous commit metadata.
    pub fn commit(&mut self, message: String, timestamp: Timestamp, sign_context: SignContext) {
        let pre_signed = todo!(); // crypto_hash!(H; self.revision_hash(), message, timestamp);
        let signature = sign_context.sign(pre_signed);
        self.metadata.committer = Some(Committer {
            message,
            timestamp,
            signature,
        });
    }

    /// Strips this revision of its commit metadata, if there is any.
    pub fn uncommit(&mut self) {
        self.metadata.committer = None;
    }

    /// Creates a new committed revision from this revision. Re-commits the revision if it is
    /// already committed.
    pub fn clone_committed(
        &self,
        message: String,
        timestamp: Timestamp,
        sign_context: SignContext,
    ) -> Revision<H> {
        let mut cloned = self.clone();
        cloned.commit(message, timestamp, sign_context);
        cloned
    }

    /// Creates a new uncommitted revision from this revision. Note that the new revision will have
    /// identical content and thus hash if this revision is already uncommitted.
    pub fn clone_uncommitted(&self) -> Revision<H> {
        let mut cloned = self.clone();
        cloned.uncommit();
        cloned
    }
}
