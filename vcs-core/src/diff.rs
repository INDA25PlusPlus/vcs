//! Tools for representing and transforming diffs.
//!
//! This module has three roles:
//!
//! - [`DiffPolicy`] turns raw source and destination bytes into an initial [`FileDiff`].
//! - [`FileDiff`] and [`Hunk`] are the standard stored representation of the differences between two files.
//! - [`ops_stream`] is the lazy intermediate representation used for transformations such as
//!   sequential composition and periodic compaction.
//!
//! The intended workflow is:
//!
//! 1. Build a [`FileDiff`] from file contents with a [`DiffPolicy`], or load an existing one.
//! 2. Convert it into an op stream with [`FileDiff::into_ops`].
//! 3. Apply stream adaptors such as [`OpStreamExt::compose`] and [`OpStreamExt::compact`].
//! 4. Materialize the final compacted stream back into a [`FileDiff`] with
//!    [`Compact::into_file_diff`].
//!
//! [`FileDiff`] is the value type that should be stored, hashed, and exposed in the higher-level
//! API. The op-stream layer is the advanced representation used while transforming diffs.

pub mod diff_policy;
pub mod file_diff;
pub mod hunk;
pub mod ops_stream;
pub mod repo_diff;
