//! Tools for representing and transforming diffs.
//!
//! This module has three roles:
//!
//! - [`diff_policy::DiffPolicy`] turns raw source and destination bytes into an initial [`hunk_collection::HunkCollection`].
//! - [`file_diff::FileDiff`] and [`hunk::Hunk`] are the standard stored representation of the differences between two files.
//! - [`ops_stream`] is the lazy intermediate representation used for transformations such as
//!   sequential composition and periodic compaction.
//!
//! The intended workflow is:
//!
//! 1. Build a [`hunk_collection::HunkCollection`] from file contents with a [`diff_policy::DiffPolicy`], or load an existing one.
//! 2. Convert it into an op stream with [`hunk_collection::HunkCollection::into_ops`].
//! 3. Apply stream adaptors such as [`ops_stream::OpStreamExt::compose`] and [`ops_stream::OpStreamExt::compact`].
//! 4. Materialize the final compacted stream back into a [`hunk_collection::HunkCollection`] with
//!    [`ops_stream::compact::Compact::into_hunk_collection`].
//!
//! [`file_diff::FileDiff`] is the value type that should be stored, hashed, and exposed in the higher-level
//! API. The op-stream layer is the advanced representation used while transforming diffs. [`file_diff::FileDiff`]
//! represents a [`hunk_collection::HunkCollection`] + state change.

pub mod diff_policy;
pub mod file_diff;
pub mod hunk;
pub mod hunk_collection;
pub mod ops_stream;
pub mod repo_diff;
