//! Tools for representing and transforming diffs.
//!
//! This module has three roles:
//!
//! - [`diff_policy::DiffPolicy`] turns raw source and destination bytes into an initial [`hunk_collection::HunkCollection`].
//! - [`file::FileDiff`] and [`hunk::Hunk`] are the standard stored representation of the differences between two files.
//! - [`operations`] is the lazy intermediate representation used for transformations such as
//!   sequential composition and periodic compaction.
//!
//! The intended workflow is:
//!
//! 1. Build a [`hunk_collection::HunkCollection`] from file contents with a [`diff_policy::DiffPolicy`], or load an existing one.
//! 2. Convert it into an op stream with [`hunk_collection::HunkCollection::into_ops`].
//! 3. Apply stream adaptors such as [`operations::OpStreamExt::compose`] and [`operations::OpStreamExt::compact`].
//! 4. Materialize the final compacted stream back into a [`hunk_collection::HunkCollection`] with
//!    [`operations::compact::Compact::into_hunk_collection`].
//!
//! [`file::FileDiff`] is the value type that should be stored, hashed, and exposed in the higher-level
//! API. The op-stream layer is the advanced representation used while transforming diffs. [`file::FileDiff`]
//! represents a [`hunk_collection::HunkCollection`] + state change.

pub mod diff_policy;
pub mod hunk;
pub mod hunk_collection;
pub mod operations;
pub mod repo_diff;
