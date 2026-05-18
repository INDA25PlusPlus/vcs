//! Tools for representing and transforming diffs.
//!
//! This module has three roles:
//!
//! - [`diff_policy::DiffPolicy`] turns raw source and destination bytes into an initial
//!   [`hunk::HunkCollection`].
//! - [`hunk::HunkCollection`] and [`hunk::Hunk`] are the standard stored representation
//!   of the differences between two files.
//! - [`operations`] is the lazy intermediate representation used for transformations such as
//!   sequential composition and periodic compaction.
//!
//! The intended workflow is:
//!
//! 1. Build a [`hunk::HunkCollection`] from file contents with a
//!    [`diff_policy::DiffPolicy`], or load an existing one.
//! 2. Convert it into an op stream with [`hunk::HunkCollection::into_ops`].
//! 3. Apply stream adaptors such as [`operations::OpStreamExt::compose`] and
//!    [`operations::OpStreamExt::compact`].
//! 4. Materialize the final compacted stream back into a [`hunk::HunkCollection`] with
//!    [`operations::compact::Compact::into_hunk_collection`].
//!
//! [`crate::changeset::file::FileDiff`] is the value type that should be stored, hashed, and exposed in
//! the higher-level API. The op-stream layer is the advanced representation used while transforming
//! diffs. [`crate::changeset::file::FileDiff`] represents a [`hunk::HunkCollection`] + state
//! change.

pub mod diff_policy;
pub mod hunk;
pub mod operations;
