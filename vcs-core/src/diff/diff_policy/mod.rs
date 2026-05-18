use crate::diff::hunk::HunkCollection;

/// Builds an initial [`HunkCollection`] from source and destination bytes.
pub trait DiffPolicy {
    fn diff(&self, src: &[u8], dst: &[u8]) -> HunkCollection;
}

pub mod myers;
pub mod naive;

pub use myers::MyersDiff;
pub use naive::NaiveDiff;
