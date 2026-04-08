pub mod diff;
pub mod diff_operations;
pub mod diff_policy;

pub use diff::{FileDiff, Hunk, RepoDiff};
pub use diff_operations::{Op, OpIter};
pub use diff_policy::{DiffPolicy, MyersDiff, NaiveDiff};
