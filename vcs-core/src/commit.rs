//! Module for creating and reading commits.
//!
//! The commit process distinguishes between two steps: authoring, and committing. Authoring is the
//! act of creating changes to the repository files, and committing is the act of appending those
//! changes to the commit history in the form of a new commit. In some cases you want to copy,
//! merge or otherwise re-use changes introduced in a commit (the authoring step), e.g. by a
//! rebase, and so the metadata concerning the authoring and committing are split. When
//! re-committing a commit, the author metadata stays intact, while the committer metadata is
//! updated to match the new commit parent and timestamp. Additionally, the committer is required
//! to sign the commit again.
//!
//! # Commit data
//!
//! The data contained in a commit is as follows:
//!
//! Content (stays the same between rebases, commit copies etc.)
//! - Repo diff (hash) - List of changes introduced by this commit
//! - Author message - Description or other information about the changes
//! - Author timestamp
//! - Author signature (optional)
//! - Content hash - Hash of the above
//!
//! Format
//! - Format version
//!
//! Committer metadata (changes between rebases, commit copies etc.)
//! - Parent's commit hash
//! - Commit message - Additional information by the committer
//! - Commit timestamp
//! - Committer signature
//! - Commit hash - Combined hash of all fields above
//!
//! ID:s (relative to a given repo, not included in the content or commit hash)
//! - Commit ID
//! - Parent ID

pub mod timestamp;

use crate::commit::timestamp::Timestamp;
use crate::crypto::{CryptoHash, CryptoHashable, SignContext, SignedHash};
use crate::crypto_hash;
use crate::repo::RepoStorage;
use std::hash::Hash;

pub type CommitId = u64;

pub type FormatVersion = u16;

/// The current data format version.
pub const FORMAT_VERSION: FormatVersion = 0;

/// In-memory representation of a commit, separated into a `header` and `metadata` for efficient
/// loading of the most significant fields `repo_diff` and `parent_id`.
///
/// A value of this type is guaranteed to be a valid commit with valid hashes and signatures,
/// although the `commit_id` and `parent_id` fields are not guaranteed to be valid within the
/// context of a particular repo.
pub struct Commit<H: CryptoHash> {
    header: CommitHeader<H>,
    metadata: CommitMetadata<H>,
}

pub struct CommitHeader<H: CryptoHash> {
    pub repo_diff: H,
    pub parent_id: CommitId,
    pub depth: usize,
}

pub struct CommitMetadata<H: CryptoHash> {
    pub commit_id: CommitId,

    pub format_version: FormatVersion,

    pub author_message: Box<str>,
    pub author_timestamp: Timestamp,
    // signature of format_version, repo_diff, author_message, author_timestamp
    pub author_signature: Option<SignedHash<H>>,

    // hash of format_version, repo_diff, author_message, author_timestamp, author_signature
    pub content_hash: H,

    pub parent_commit_hash: H,

    // hash of file tree at this commit (do we want this?)
    // repo_hash: H,
    pub commit_message: Box<str>,
    pub commit_timestamp: Timestamp,
    // signature of content_hash, parent_commit_hash, commit_message, commit_timestamp
    pub committer_signature: SignedHash<H>,

    // hash of committer_signature
    pub commit_hash: H,
}

#[derive(thiserror::Error, Debug)]
pub enum CommitError {
    #[error("committer timestamp must be after author timestamp")]
    Timestamp,
    #[error("unsupported format version '{0}'")]
    FormatVersion(FormatVersion),
    #[error("content hash mismatch")]
    ContentHash,
    #[error("commit hash mismatch")]
    CommitHash,
    #[error("invalid author signature")]
    AuthorSignature,
    #[error("invalid committer signature")]
    CommitterSignature,
}

impl<H: CryptoHash> Commit<H> {
    fn new_internal(
        commit_id: CommitId,
        parent_id: CommitId,
        parent_commit_hash: H,
        repo_diff: H,
        author_message: Box<str>,
        author_timestamp: Timestamp,
        author_signature: Option<SignedHash<H>>,
        commit_message: Box<str>,
        commit_timestamp: Timestamp,
        committer_sign_context: SignContext,
    ) -> Result<Commit<H>, CommitError> {
        if author_timestamp > commit_timestamp {
            return Err(CommitError::Timestamp);
        }
        let content_hash = crypto_hash!(
            H;
            FORMAT_VERSION,
            &repo_diff,
            &author_message,
            author_timestamp,
            author_signature
        );
        let committer_pre_hash = crypto_hash!(
            H;
            &content_hash,
            &parent_commit_hash,
            &commit_message,
            commit_timestamp
        );
        let committer_signature = committer_sign_context.sign(committer_pre_hash);
        let commit_hash = crypto_hash!(H; committer_signature);
        Ok(Commit {
            header: CommitHeader {
                repo_diff,
                parent_id,
                depth: 0,
            },
            metadata: CommitMetadata {
                commit_id,
                format_version: FORMAT_VERSION,
                author_message,
                author_timestamp,
                author_signature,
                content_hash,
                parent_commit_hash,
                commit_message,
                commit_timestamp,
                committer_signature,
                commit_hash,
            },
        })
    }

    /// Create a new commit with the given fields.
    ///
    /// The returned commit is guaranteed to be valid if and only if `parent_commit_hash` is the
    /// commit hash of the commit with ID `parent_id` and `commit_id` is a valid new commit ID.
    pub fn new(
        commit_id: CommitId,
        parent_id: CommitId,
        parent_commit_hash: H,
        repo_diff: H,
        author_message: Box<str>,
        author_timestamp: Timestamp,
        author_sign_context: Option<SignContext>,
        commit_message: Box<str>,
        commit_timestamp: Timestamp,
        committer_sign_context: SignContext,
    ) -> Result<Commit<H>, CommitError> {
        let author_signature = author_sign_context.map(|sign_context| {
            let author_pre_hash = crypto_hash!(
                H;
                FORMAT_VERSION,
                &repo_diff,
                &author_message,
                author_timestamp
            );
            sign_context.sign(author_pre_hash)
        });
        Commit::new_internal(
            commit_id,
            parent_id,
            parent_commit_hash,
            repo_diff,
            author_message,
            author_timestamp,
            author_signature,
            commit_message,
            commit_timestamp,
            committer_sign_context,
        )
    }

    /// Create a new commit by rebasing `self` onto a new parent, and with updated committer
    /// metadata.
    pub fn recommit(
        &self,
        commit_id: CommitId,
        parent_id: CommitId,
        parent_commit_hash: H,
        commit_message: Box<str>,
        commit_timestamp: Timestamp,
        committer_sign_context: SignContext,
    ) -> Result<Commit<H>, CommitError> {
        Commit::new_internal(
            commit_id,
            parent_id,
            parent_commit_hash,
            self.header.repo_diff.clone(),
            self.metadata.author_message.clone(),
            self.metadata().author_timestamp,
            self.metadata().author_signature.clone(),
            commit_message,
            commit_timestamp,
            committer_sign_context,
        )
    }

    /// Extract the commit header and metadata into two owned values. The returned tuple is
    /// guaranteed to constitute a valid commit.
    pub fn into_parts(self) -> (CommitHeader<H>, CommitMetadata<H>) {
        (self.header, self.metadata)
    }

    pub fn header(&self) -> &CommitHeader<H> {
        &self.header
    }

    pub fn metadata(&self) -> &CommitMetadata<H> {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
