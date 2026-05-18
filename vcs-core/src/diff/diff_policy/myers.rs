use bytes::Bytes;

use super::DiffPolicy;
use crate::diff::hunk::HunkCollection;
use crate::diff::operations::{Op, OpStreamExt};

/// Computes the shortest edit script between two byte slices using Myers' algorithm.
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

    let mut edit_distance = vec![vec![0usize; m + 1]; n + 1];

    // Initialize base cases
    for (i, row) in edit_distance.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in edit_distance[0].iter_mut().enumerate() {
        *val = j;
    }

    // Fill edit distance table
    for i in 1..=n {
        for j in 1..=m {
            if src[i - 1] == dst[j - 1] {
                edit_distance[i][j] = edit_distance[i - 1][j - 1];
            } else {
                edit_distance[i][j] = 1 + std::cmp::min(
                    edit_distance[i - 1][j],
                    std::cmp::min(edit_distance[i][j - 1], edit_distance[i - 1][j - 1]),
                );
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
        } else if j > 0 && (i == 0 || edit_distance[i][j - 1] < edit_distance[i - 1][j]) {
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
    use crate::diff::diff_policy::naive::NaiveDiff;

    const SRC_DST_DATA: [(&[u8], &[u8]); 3] = [
        ("Hello".as_bytes(), "World".as_bytes()),
        ("".as_bytes(), "".as_bytes()),
        ("MLKLKMEFELUHMBOREJJEIWFEWFMAÖÖÖÖ".as_bytes(), "".as_bytes()),
    ];

    /// Calculates the total size of a diff in bytes (bytes deleted + bytes inserted).
    fn diff_size(hunks: &[crate::diff::hunk::Hunk]) -> u64 {
        hunks
            .iter()
            .map(|hunk| hunk.len_before + hunk.content_after.len() as u64)
            .sum()
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
        assert_eq!(diff.hunks[0].len_before, src.len() as u64);
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
            let mut pos: u64 = 0;
            for hunk in diff.hunks.iter() {
                let pos_start = pos as usize;
                let pos_end = (pos + hunk.offset) as usize;
                result.extend_from_slice(&src[pos_start..pos_end]);
                pos += hunk.offset + hunk.len_before;
                result.extend_from_slice(&hunk.content_after);
            }
            result.extend_from_slice(&src[pos as usize..]);

            assert_eq!(result, dst);
        }
    }

    #[test]
    fn test_myers_diff_smaller_than_naive() {
        // Verify that Myers produces more compact diffs than naive replacement.
        let myers = MyersDiff;
        let naive = NaiveDiff;

        let test_cases = [
            (b"hello world".as_slice(), b"hello world!".as_slice()),
            (b"abcdefgh".as_slice(), b"abXdefgh".as_slice()),
            (
                b"the quick brown fox".as_slice(),
                b"the quick red fox".as_slice(),
            ),
        ];

        for (src, dst) in test_cases {
            let myers_diff = myers.diff(src, dst);
            let naive_diff = naive.diff(src, dst);

            let myers_size = diff_size(&myers_diff.hunks);
            let naive_size = diff_size(&naive_diff.hunks);

            assert!(
                myers_size <= naive_size,
                "Myers diff should be smaller than naive for ({:?} -> {:?}): \
                 Myers size={}, Naive size={}",
                String::from_utf8_lossy(src),
                String::from_utf8_lossy(dst),
                myers_size,
                naive_size
            );
        }
    }
}
