use std::collections::VecDeque;

use bytes::Bytes;

use crate::{
    crypto::{CryptoHash, CryptoHashable},
    diff::{Hunk, Op, OpStreamExt},
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
        self.into_ops().compose(other.into_ops()).collect()
    }
}

impl FromIterator<Op> for FileDiff {
    fn from_iter<T: IntoIterator<Item = Op>>(ops: T) -> Self {
        let mut hunks = Vec::new();
        let mut open_hunk: Option<Hunk> = None;
        let mut pending_offset = 0;
        let mut inserted_bytes = Vec::new();

        for op in ops {
            match op {
                Op::Keep(len) => {
                    if let Some(mut hunk) = open_hunk.take() {
                        // A keep ends the current hunk, so flush any buffered inserts.
                        if !inserted_bytes.is_empty() {
                            hunk.content_after = inserted_bytes.into_boxed_slice();
                        }
                        hunks.push(hunk);
                        inserted_bytes = Vec::new();
                    }

                    pending_offset += len;
                }
                Op::Delete(len) => {
                    let mut hunk = open_hunk.unwrap_or_else(|| {
                        // Start a new hunk at the current gap.
                        let new_hunk = Hunk {
                            offset: pending_offset,
                            len_before: 0,
                            content_after: Box::new([]),
                        };
                        pending_offset = 0;
                        new_hunk
                    });

                    hunk.len_before += len;
                    pending_offset += len;

                    open_hunk = Some(hunk);
                }
                Op::Insert(buf) => {
                    let hunk = open_hunk.unwrap_or_else(|| {
                        // Inserts at the same position are merged into one hunk.
                        let new_hunk = Hunk {
                            offset: pending_offset,
                            len_before: 0,
                            content_after: Box::new([]),
                        };
                        pending_offset = 0;
                        new_hunk
                    });

                    inserted_bytes.extend_from_slice(&buf);

                    open_hunk = Some(hunk);
                }
            }
        }

        if let Some(mut hunk) = open_hunk {
            if !inserted_bytes.is_empty() {
                hunk.content_after = inserted_bytes.into_boxed_slice();
            }
            hunks.push(hunk);
        }

        Self {
            hunks: hunks.into_boxed_slice(),
        }
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
            Op::Insert(Bytes::from_static(b"3456")),
        ];

        let diff: FileDiff = ops.into_iter().collect();

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
}
