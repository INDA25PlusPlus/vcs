use bytes::Bytes;

use crate::diff::{
    hunk::Hunk,
    hunk_collection::HunkCollection,
    operations::{Op, OpStreamExt},
};

/// Builds an initial [`HunkCollection`] from source and destination bytes.
pub trait DiffPolicy {
    fn diff(&self, src: &[u8], dst: &[u8]) -> HunkCollection;
}

/// Trivial policy that replaces the whole file with a single hunk.
pub struct NaiveDiff;

impl DiffPolicy for NaiveDiff {
    fn diff(&self, src_buf: &[u8], dst_buf: &[u8]) -> HunkCollection {
        let src_len = src_buf.len();
        let hunks = Box::new([Hunk {
            offset: 0,
            len_before: src_len.try_into().expect("src_len should fit into u64"),
            content_after: Box::from(dst_buf),
        }]);

        HunkCollection::new(hunks)
    }
}

/// Computes the shortest edit script between two byte slices.
fn myers_ops(src: &[u8], dst: &[u8]) -> Vec<Op> {
    let (n, m) = (src.len(), dst.len());

    // Trivial cases
    if n == 0 && m == 0 {
        return vec![];
    }
    if n == 0 {
        return vec![Op::Insert(Bytes::copy_from_slice(dst))];
    }
    if m == 0 {
        return vec![Op::Delete(n)];
    }

    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    // Initialize base cases
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate() {
        *val = j;
    }

    // Fill DP table
    for i in 1..=n {
        for j in 1..=m {
            if src[i - 1] == dst[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            } else {
                dp[i][j] =
                    1 + std::cmp::min(dp[i - 1][j], std::cmp::min(dp[i][j - 1], dp[i - 1][j - 1]));
            }
        }
    }

    // Backtrack to reconstruct the edit script
    let mut ops: Vec<Op> = Vec::new();
    let (mut i, mut j) = (n, m);

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && src[i - 1] == dst[j - 1] {
            ops.push(Op::Keep(1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] < dp[i - 1][j]) {
            ops.push(Op::Insert(Bytes::copy_from_slice(&dst[j - 1..j])));
            j -= 1;
        } else {
            ops.push(Op::Delete(1));
            i -= 1;
        }
    }

    ops.reverse();

    let mut merged: Vec<Op> = Vec::new();
    for op in ops {
        if let Some(Op::Keep(total)) = merged.last_mut()
            && let Op::Keep(len) = op
        {
            *total += len;
            continue;
        }
        merged.push(op);
    }

    merged
}

pub struct MyersDiff;

impl DiffPolicy for MyersDiff {
    fn diff(&self, src: &[u8], dst: &[u8]) -> HunkCollection {
        myers_ops(src, dst)
            .into_iter()
            .compact()
            .into_hunk_collection()
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
            assert!(!diff.hunks.is_empty());
            assert_eq!(diff.hunks[0].offset, 0);
            assert_eq!(diff.hunks[0].len_before, src.len().try_into().unwrap());
            assert_eq!(*diff.hunks[0].content_after, *dst);
        }
    }

    #[test]
    fn test_myers_diff_identity() {
        let differ = MyersDiff;

        let test_cases = [
            b"".as_slice(),
            b"Hello",
            b"This is a longer string to test identity",
        ];

        for src in test_cases {
            let diff = differ.diff(src, src);
            assert!(
                diff.hunks.is_empty(),
                "identity diff should have no hunks, but got: {:?}",
                diff.hunks
            );
        }
    }

    #[test]
    fn test_myers_diff_pure_insert() {
        let differ = MyersDiff;
        let dst = b"badonkadonk";
        let diff = differ.diff(b"", dst);

        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].offset, 0);
        assert_eq!(diff.hunks[0].len_before, 0);
        assert_eq!(diff.hunks[0].content_after.as_ref(), dst);
    }

    #[test]
    fn test_myers_diff_pure_delete() {
        let differ = MyersDiff;
        let src = b"badonkadonk";
        let diff = differ.diff(src, b"");

        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].offset, 0);
        assert_eq!(diff.hunks[0].len_before, src.len());
        assert!(diff.hunks[0].content_after.is_empty());
    }

    #[test]
    fn test_myers_diff_roundtrip() {
        let differ = MyersDiff;

        // Various test cases: verify applying the diff to src produces dst.
        let test_cases = [
            (b"".as_slice(), b"".as_slice()),
            (b"a".as_slice(), b"a".as_slice()),
            (b"abcdef".as_slice(), b"axcdef".as_slice()),
            (b"abc".as_slice(), b"abcdef".as_slice()),
            (b"abcdef".as_slice(), b"abc".as_slice()),
            ("MLKLKMEFELUEWFMAÖÖÖÖ".as_bytes(), b"".as_slice()),
        ];

        for (src, dst) in test_cases {
            let diff = differ.diff(src, dst);

            let mut result = Vec::new();
            let mut pos = 0;
            for hunk in diff.hunks.iter() {
                result.extend_from_slice(&src[pos..pos + hunk.offset]);
                pos += hunk.offset + hunk.len_before;
                result.extend_from_slice(&hunk.content_after);
            }
            result.extend_from_slice(&src[pos..]);

            assert_eq!(result, dst);
        }
    }
}
