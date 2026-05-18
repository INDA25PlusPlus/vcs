use bytes::Bytes;

use super::DiffPolicy;
use crate::diff::hunk::HunkCollection;
use crate::diff::operations::{Op, OpStreamExt};

/// Computes the shortest edit script between two byte slices using Myers' algorithm.
fn myers_ops(src: &[u8], dst: &[u8]) -> Vec<Op> {
    let n = src.len();
    let m = dst.len();

    if n == 0 && m == 0 {
        return vec![];
    }
    if n == 0 {
        return vec![Op::Insert(Bytes::copy_from_slice(dst))];
    }
    if m == 0 {
        return vec![Op::Delete(n)];
    }

    let max_d = n + m;
    let mut v = vec![0isize; 2 * max_d + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let idx = |k: isize| (k + max_d as isize) as usize;

    'done: for d in 0..=max_d {
        let mut v_copy = v.clone();

        for k in (-(d as isize))..=(d as isize) {
            let mut x = if k == -(d as isize) {
                v[idx(k + 1)]
            } else if k == d as isize {
                v[idx(k - 1)] + 1
            } else {
                std::cmp::max(v[idx(k - 1)] + 1, v[idx(k + 1)])
            };

            let mut y = x - k;
            while x < n as isize && y < m as isize && src[x as usize] == dst[y as usize] {
                x += 1;
                y += 1;
            }

            v_copy[idx(k)] = x;

            if x >= n as isize && y >= m as isize {
                trace.push(v_copy);
                break 'done;
            }
        }
        trace.push(v_copy.clone());
        v = v_copy;
    }

    reconstruct(&trace, src, dst, n, m, max_d)
}

fn reconstruct(
    trace: &[Vec<isize>],
    src: &[u8],
    dst: &[u8],
    n: usize,
    m: usize,
    max_d: usize,
) -> Vec<Op> {
    let idx = |k: isize| (k + max_d as isize) as usize;
    let mut ops = Vec::new();
    let mut x = n as isize;
    let mut y = m as isize;

    for d in (0..trace.len()).rev() {
        if d == 0 {
            while x > 0 || y > 0 {
                if x > 0 && y > 0 && src[(x - 1) as usize] == dst[(y - 1) as usize] {
                    ops.push(Op::Keep(1));
                    x -= 1;
                    y -= 1;
                } else if y > 0 {
                    ops.push(Op::Insert(Bytes::copy_from_slice(
                        &dst[(y - 1) as usize..y as usize],
                    )));
                    y -= 1;
                } else {
                    ops.push(Op::Delete(1));
                    x -= 1;
                }
            }
            break;
        }

        let v = &trace[d];
        let v_prev = &trace[d - 1];

        loop {
            let k = x - y;

            // Backtrack along diagonal
            while x > 0 && y > 0 && src[(x - 1) as usize] == dst[(y - 1) as usize] {
                ops.push(Op::Keep(1));
                x -= 1;
                y -= 1;
            }

            if x == 0 && y == 0 {
                break;
            }

            let k_curr = idx(k);
            let came_del = x > 0 && k != -(d as isize) && v_prev[idx(k - 1)] + 1 == v[k_curr];
            let came_ins = y > 0 && k != (d as isize) && v_prev[idx(k + 1)] == v[k_curr];

            if came_del {
                ops.push(Op::Delete(1));
                x -= 1;
            } else if came_ins {
                ops.push(Op::Insert(Bytes::copy_from_slice(
                    &dst[(y - 1) as usize..y as usize],
                )));
                y -= 1;
            } else if d > 0 {
                break;
            }
        }
    }

    ops.reverse();

    let mut merged = Vec::new();
    for op in ops {
        if let Some(last) = merged.last_mut() {
            match (last, &op) {
                (Op::Keep(t), Op::Keep(l)) => {
                    *t += l;
                    continue;
                }
                (Op::Delete(t), Op::Delete(l)) => {
                    *t += l;
                    continue;
                }
                (Op::Insert(b), Op::Insert(new_b)) => {
                    let mut c = b.to_vec();
                    c.extend_from_slice(new_b);
                    *b = Bytes::from(c);
                    continue;
                }
                _ => {}
            }
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
