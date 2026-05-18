use super::DiffPolicy;
use crate::diff::hunk::Hunk;
use crate::diff::hunk::HunkCollection;

/// Trivial policy that replaces the whole file with a single hunk.
pub struct NaiveDiff;

impl DiffPolicy for NaiveDiff {
    fn diff(&self, src_buf: &[u8], dst_buf: &[u8]) -> HunkCollection {
        let src_len = src_buf.len() as u64;
        let hunks = Box::new([Hunk {
            offset: 0,
            len_before: src_len,
            content_after: Box::from(dst_buf),
        }]);

        HunkCollection::new(hunks)
    }
}
