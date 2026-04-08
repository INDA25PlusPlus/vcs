use std::collections::VecDeque;

use bytes::Bytes;

use crate::{
    crypto::{CryptoHash, CryptoHashable},
    diff::{Compact, Hunk, Op, OpStreamExt},
};

/// Byte-level edits for a single file.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct FileDiff {
    pub hunks: Box<[Hunk]>,
}

/// Converts hunks into an op stream.
struct HunkOpStream<I: Iterator<Item = Hunk>> {
    hunks: I,
    pending_ops: VecDeque<Op>,
    previous_deleted_len: usize,
}

impl FileDiff {
    pub fn new(hunks: Box<[Hunk]>) -> Self {
        Self { hunks }
    }

    /// Exposes the diff as a lazy op stream.
    pub fn into_ops(self) -> impl Iterator<Item = Op> {
        HunkOpStream {
            hunks: self.hunks.into_iter(),
            pending_ops: VecDeque::new(),
            previous_deleted_len: 0,
        }
    }

    /// Composes `self` with `other` and materializes the result back into hunks.
    pub fn compose(self, other: FileDiff) -> Self {
        self.into_ops()
            .compose(other.into_ops())
            .compact()
            .into_file_diff()
    }
}

impl<I: Iterator<Item = Op>> Compact<I> {
    /// Transforms a compacted op stream into a 'FileDiff'
    ///
    /// This assumes the stream has already been compacted:
    /// one `Keep` op between edit regions, and each edit region consists of at the most
    /// one `Delete` followed by at most one `Insert`.
    pub fn into_file_diff(self) -> FileDiff {
        let mut hunks = Vec::new();
        let mut offset = 0usize;
        let mut len_before = 0usize;
        let mut content_after = Vec::new();

        let mut flush_hunk =
            |offset: &mut usize, delete_len: &mut usize, insert_bytes: &mut Vec<u8>| {
                if *delete_len == 0 && insert_bytes.is_empty() {
                    return;
                }

                // Hunk offsets are stored relative to the previous hunk's offset, independent of len_before
                // 'Keep' op on the other hand does depend on 'Delete' op
                let next_offset = *delete_len;
                hunks.push(Hunk {
                    offset: std::mem::take(offset),
                    len_before: std::mem::take(delete_len),
                    content_after: std::mem::take(insert_bytes).into_boxed_slice(),
                });
                *offset = next_offset;
            };

        for op in self {
            match op {
                Op::Keep(len) => {
                    flush_hunk(&mut offset, &mut len_before, &mut content_after);
                    offset += len;
                }
                Op::Delete(len) => {
                    debug_assert_eq!(
                        len_before, 0,
                        "compact streams emit at most one delete per edit region"
                    );
                    debug_assert!(
                        content_after.is_empty(),
                        "compact streams must emit delete before insert within an edit region"
                    );
                    len_before = len;
                }
                Op::Insert(buf) => {
                    debug_assert!(
                        !buf.is_empty(),
                        "compact streams must not contain empty insert operations"
                    );
                    debug_assert!(
                        content_after.is_empty(),
                        "compact streams emit at most one insert per edit region"
                    );
                    // TODO: This causes unnecessary copying, fix this
                    content_after = buf.to_vec();
                }
            }
        }
        flush_hunk(&mut offset, &mut len_before, &mut content_after);

        FileDiff::new(hunks.into_boxed_slice())
    }
}

impl<I: Iterator<Item = Hunk>> Iterator for HunkOpStream<I> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(op) = self.pending_ops.pop_front() {
            return Some(op);
        }

        let hunk = self.hunks.next()?;

        // Offsets are stored relative to the previous hunk, including its deletion span.
        let keep_len = hunk.offset.saturating_sub(self.previous_deleted_len);

        if keep_len > 0 {
            self.pending_ops.push_back(Op::Keep(keep_len));
        }
        if hunk.len_before > 0 {
            self.pending_ops.push_back(Op::Delete(hunk.len_before));
        }
        if !hunk.content_after.is_empty() {
            self.pending_ops
                .push_back(Op::Insert(Bytes::from(hunk.content_after)));
        }

        self.previous_deleted_len = hunk.len_before;

        self.pending_ops.pop_front()
    }
}

impl CryptoHashable for FileDiff {
    fn crypto_hash<H: CryptoHash>(&self) -> H {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_diff() -> FileDiff {
        FileDiff::new(Box::from([
            Hunk {
                content_after: Box::from("111".as_bytes()),
                len_before: 2,
                offset: 0,
            },
            Hunk {
                content_after: Box::from("2".as_bytes()),
                len_before: 0,
                offset: 2,
            },
            Hunk {
                content_after: Box::new([]),
                len_before: 4,
                offset: 2,
            },
            Hunk {
                content_after: Box::from("3456".as_bytes()),
                len_before: 4,
                offset: 5,
            },
        ]))
    }

    #[test]
    fn test_file_diff_to_op_stream_translation() {
        let ops: Vec<_> = sample_diff().into_ops().collect();

        assert_eq!(
            ops,
            vec![
                Op::Delete(2),
                Op::Insert(Bytes::from_static(b"111")),
                Op::Insert(Bytes::from_static(b"2")),
                Op::Keep(2),
                Op::Delete(4),
                Op::Keep(1),
                Op::Delete(4),
                Op::Insert(Bytes::from_static(b"3456")),
            ]
        );
    }

    #[test]
    fn test_op_stream_to_file_diff_translation() {
        let ops = vec![
            Op::Delete(2),
            Op::Insert(Bytes::from_static(b"111")),
            Op::Insert(Bytes::from_static(b"2")),
            Op::Keep(2),
            Op::Delete(4),
            Op::Keep(1),
            Op::Delete(4),
            Op::Keep(0),
            Op::Insert(Bytes::from_static(b"3456")),
        ];

        let diff: FileDiff = ops.into_iter().compact().into_file_diff();

        assert_eq!(
            diff,
            FileDiff::new(Box::from([
                Hunk {
                    offset: 0,
                    len_before: 2,
                    content_after: Box::from("1112".as_bytes()),
                },
                Hunk {
                    offset: 4,
                    len_before: 4,
                    content_after: Box::new([]),
                },
                Hunk {
                    offset: 5,
                    len_before: 4,
                    content_after: Box::from("3456".as_bytes()),
                },
            ]))
        );
    }

    #[test]
    fn test_compact_stream_to_file_diff_insert_only() {
        let ops = vec![
            Op::Keep(3),
            Op::Insert(Bytes::from_static(b"abc")),
            Op::Keep(2),
        ];

        let diff = ops.into_iter().compact().into_file_diff();

        assert_eq!(
            diff,
            FileDiff::new(Box::from([Hunk {
                offset: 3,
                len_before: 0,
                content_after: Box::from("abc".as_bytes()),
            }]))
        );
    }

    #[test]
    fn test_compact_stream_to_file_diff_identity() {
        let ops = vec![Op::Keep(3), Op::Keep(2), Op::Keep(4)];

        let diff = ops.into_iter().compact().into_file_diff();

        assert_eq!(diff, FileDiff::new(Box::new([])));
    }
}
