//! Module for creating and modifying revisions.
//!
//! A revision is a certain version of the repository, containing a set of changes introduced since
//! the previous revision. In this way it is similar to a Git commit, although a revision differs in
//! that it may be either committed or uncommitted. An uncommitted revision represents a set of
//! changes and author metadata not yet pushed to the upstream, while a committed revision
//! has been either pushed to or pulled from the upstream. Furthermore, a committed revision is
//! considered permanent and can not be rebased, squashed or otherwise edited.

use crate::crypto::digest::{CryptoDigest, CryptoHash, CryptoHasher};
use crate::crypto::signature::SignContext;
use crate::diff::repo_diff::{RepoDiff, RepoDiffRef};
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

pub const INITIAL_REVISION_MESSAGE: &str = "Initial revision";

#[derive(Clone, Debug)]
pub struct Patch<D: CryptoDigest + CryptoHash> {
    repo_diff: RepoDiffRef<D>,
    author: Author<D>,
}

#[derive(Clone, Debug)]
pub struct RevisionHeader<D: CryptoDigest + CryptoHash> {
    pub repo_diff: RepoDiffRef<D>,
    pub parent: RevisionId<D>,
}

#[derive(Clone, Debug)]
pub struct RevisionMetadata<D: CryptoDigest + CryptoHash> {
    pub version: FormatVersion,
    pub patches: Box<[Patch<D>]>,
    pub committer: Option<Committer<D>>,
}

/// In-memory representation of a revision, separated into a `header` and `metadata` for efficient
/// loading of the most significant fields `repo_diff` and `parent`.
///
/// A value of this type is guaranteed to be a valid revision with valid hashes and signatures.
#[derive(Clone, Debug)]
pub struct Revision<D: CryptoDigest + CryptoHash> {
    header: RevisionHeader<D>,
    metadata: RevisionMetadata<D>,
}

impl<D: CryptoDigest + CryptoHash> Patch<D> {
    pub fn new_signed(
        repo_diff: RepoDiffRef<D>,
        message: Box<str>,
        timestamp: Timestamp,
        sign_context: SignContext,
    ) -> Patch<D> {
        let pre_signed = D::generate(&(&repo_diff, &message, &timestamp));
        let signature = sign_context.sign(&pre_signed);
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

impl<D: CryptoDigest + CryptoHash> CryptoHash for Patch<D> {
    fn crypto_hash<OutD: CryptoDigest, H: CryptoHasher<Output = OutD>>(&self, state: &mut H) {
        todo!()
    }
}

impl<D: CryptoDigest + CryptoHash> Revision<D>
where
    D: Eq + Hash + Send + Sync + Clone,
{
    pub fn new_initial(sign_context: SignContext<'_>) -> Revision<D> {
        let repo_diff = RepoDiff::<D>::empty();
        let mut rev = Revision {
            header: RevisionHeader {
                repo_diff: D::generate(&repo_diff),
                parent: D::zero(),
            },
            metadata: RevisionMetadata {
                version: FORMAT_VERSION,
                patches: Box::new([]),
                committer: None,
            },
        };
        rev.commit(
            INITIAL_REVISION_MESSAGE.to_string().into_boxed_str(),
            Timestamp::now(),
            sign_context,
        );
        rev
    }

    pub async fn new<S: RepoStorage<D>>(
        repo: &Repository<D, S>,
        parent: RevisionId<D>,
        patches: Box<[Patch<D>]>,
    ) -> Revision<D> {
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

    pub fn header(&self) -> &RevisionHeader<D> {
        &self.header
    }

    pub fn metadata(&self) -> &RevisionMetadata<D> {
        &self.metadata
    }

    /// Splits `self` into its storable parts. The header and metadata are guaranteed to be valid
    /// with respect to each other.
    pub fn into_parts(self) -> (RevisionHeader<D>, RevisionMetadata<D>) {
        (self.header, self.metadata)
    }

    pub fn is_committed(&self) -> bool {
        self.metadata.committer.is_some()
    }

    /// Returns the combined hash of this revision's patches and parent ID.
    pub fn revision_hash(&self) -> D {
        D::generate(&(
            &self.metadata.version,
            &self.header.parent,
            &self.metadata.patches,
        ))
    }

    /// Commits this revision, overwriting any previous commit metadata.
    pub fn commit(&mut self, message: Box<str>, timestamp: Timestamp, sign_context: SignContext) {
        let pre_signed = D::generate(&(&self.revision_hash(), &message, &timestamp));
        let signature = sign_context.sign(&pre_signed);
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
        message: Box<str>,
        timestamp: Timestamp,
        sign_context: SignContext,
    ) -> Revision<D> {
        let mut cloned = self.clone();
        cloned.commit(message, timestamp, sign_context);
        cloned
    }

    /// Creates a new uncommitted revision from this revision. Note that the new revision will have
    /// identical content and thus hash if this revision is already uncommitted.
    pub fn clone_uncommitted(&self) -> Revision<D> {
        let mut cloned = self.clone();
        cloned.uncommit();
        cloned
    }
}

impl<D: CryptoDigest + CryptoHash> CryptoHash for Revision<D> {
    fn crypto_hash<OutD: CryptoDigest, H: CryptoHasher<Output = OutD>>(&self, state: &mut H) {
        todo!()
    }
}
