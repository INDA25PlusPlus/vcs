pub mod diff_policy;
pub mod file_diff;
pub mod hunk;
pub mod ops_stream;
pub mod repo_diff;

pub use diff_policy::{DiffPolicy, MyersDiff, NaiveDiff};
pub use file_diff::FileDiff;
pub use hunk::Hunk;
pub use ops_stream::{Compact, Compose, Op, OpStreamExt};
pub use repo_diff::RepoDiff;
