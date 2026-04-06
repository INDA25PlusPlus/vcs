use crate::crypto::{CryptoHash, CryptoHashable};
use crate::path::RepoPath;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

pub type RepoDiffRef<H: CryptoHash> = H;

/// Per-file diffs for a commit.
#[derive(Debug)]
pub struct RepoDiff<H: CryptoHash> {
    file_diffs: HashMap<RepoPath, H>,
}

/// Byte-level edits for one file.
#[derive(Debug, Eq, PartialEq)]
pub struct FileDiff {
    hunks: Box<[Hunk]>,
}

/// One contiguous replacement in the file.
#[derive(Eq, PartialEq)]
struct Hunk {
    // Gap from the previous hunk, or from file start.
    offset: usize,
    // Number of source bytes replaced removed.
    len_before: usize,
    // Replacement bytes written at this position.
    content_after: Box<[u8]>,
}

impl FileDiff {
    fn new(hunks: Box<[Hunk]>) -> FileDiff {
        FileDiff { hunks }
    }
}

impl CryptoHashable for FileDiff {
    fn crypto_hash<H: CryptoHash>(&self) -> H {
        todo!()
    }
}

impl Debug for Hunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!("maybe print `content_after` as utf-8 instead?")
    }
}

/// Builds a file diff from source and destination bytes.
trait DiffPolicy {
    fn diff(&self, src_diff: &[u8], dst_diff: &[u8]) -> FileDiff;
}

/// Emits a single hunk for the whole file.
struct NaiveDiff;

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

struct MyersDiff;

impl DiffPolicy for MyersDiff {
    fn diff(&self, src_diff: &[u8], dst_diff: &[u8]) -> FileDiff {
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

        // NaiveDiff always replaces the entire source in one hunk.
        for (src, dst) in SRC_DST_DATA {
            let diff = differ.diff(src, dst);
            assert!(diff.hunks.len() > 0);
            assert_eq!(diff.hunks[0].offset, 0);
            assert_eq!(diff.hunks[0].len_before, src.len());
            assert_eq!(*diff.hunks[0].content_after, *dst);
        }
    }

    fn test_naive_fuzzy() {}

    fn test_naive_diff_from_file_content() {}
}
