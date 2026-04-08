//! Lazy op-stream adaptors used for transforming diffs.
//!
//! It exists for operations such as:
//!
//! - sequential composition of diffs
//! - periodic compaction of long compose pipelines
//! - delaying materialization until the final result is needed
//!

pub mod compact;
pub mod compose;
pub mod types;

pub use compact::Compact;
pub use compose::Compose;
pub use types::Op;

/// Extension methods for op streams.
pub trait OpStreamExt: Iterator<Item = Op> + Sized {
    /// Lazily composes two op streams.
    ///
    /// If `self` maps `A -> B` and `other` maps `B -> C`, the resulting stream maps `A -> C`.
    fn compose<O: Iterator<Item = Op>>(self, other: O) -> Compose<Self, O> {
        Compose::new(self, other)
    }

    /// Lazily compacts the stream into standard keep and edit runs.
    fn compact(self) -> Compact<Self> {
        Compact::new(self)
    }
}

impl<I: Iterator<Item = Op>> OpStreamExt for I {}
