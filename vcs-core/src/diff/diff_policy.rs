use crate::diff::{file_diff::FileDiff, hunk::Hunk};

/// Builds an initial [`FileDiff`] from source and destination bytes.
pub trait DiffPolicy {
    fn diff(&self, src: &[u8], dst: &[u8]) -> FileDiff;
}

/// Trivial policy that replaces the whole file with a single hunk.
pub struct NaiveDiff;

impl DiffPolicy for NaiveDiff {
    fn diff(&self, src_buf: &[u8], dst_buf: &[u8]) -> FileDiff {
        let src_len = src_buf.len();
        let hunks = Box::new([Hunk {
            offset: 0,
            len_before: src_len,
            content_after: Box::from(dst_buf),
        }]);

        FileDiff::new(hunks)
    }
}

/// Placeholder for a future Myers diff implementation.
pub struct MyersDiff;

impl DiffPolicy for MyersDiff {
    fn diff(&self, _src: &[u8], _dst: &[u8]) -> FileDiff {
        todo!("Implement 'Myers Diff Algorithm'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC_DST_DATA: [(&[u8], &[u8]); 3] = [
        ("Hello".as_bytes(), "World".as_bytes()),
        ("".as_bytes(), "".as_bytes()),
        ("MLKLKMEFELUHMBOREJJEIWFEWFMAÖÖÖÖ".as_bytes(), "".as_bytes()),
    ];

    #[test]
    fn test_naive_diff_short() {
        let differ = NaiveDiff;

        // NaiveDiff always emits one full-file replacement hunk.
        for (src, dst) in SRC_DST_DATA {
            let diff = differ.diff(src, dst);
            assert!(diff.hunks.len() > 0);
            assert_eq!(diff.hunks[0].offset, 0);
            assert_eq!(diff.hunks[0].len_before, src.len());
            assert_eq!(*diff.hunks[0].content_after, *dst);
        }
    }
}
