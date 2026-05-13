use std::{collections::VecDeque, mem};

use crate::diff::operations::Op;

/// Compacts an [`Op`] stream into the fewest possible [`Op`] while still representing the same diff.
///
/// A compacted stream uses the standard op-stream form:
/// - consecutive [`Op::Keep`] runs are merged
/// - every edit region between two keeps is emitted as at most one [`Op::Delete`]
///   followed by one [`Op::Insert`]
pub struct Compact<I: Iterator<Item = Op>> {
    iter: I,
    accumulation: Accumulation,
    pending: VecDeque<Op>,
}

/// The currently accumulated run.
enum Accumulation {
    Empty,
    Keep(usize),
    Edit { delete: usize, insert: Vec<u8> },
}

impl Accumulation {
    /// Flushes the current run into compacted ops and clears the accumulator.
    fn flush_into(&mut self, pending: &mut VecDeque<Op>) {
        match mem::replace(self, Self::Empty) {
            Self::Empty => {}
            Self::Keep(len) => pending.push_back(Op::Keep(len)),
            Self::Edit {
                delete: deleted,
                insert: inserted,
            } => {
                if deleted > 0 {
                    pending.push_back(Op::Delete(deleted));
                }
                if !inserted.is_empty() {
                    pending.push_back(Op::Insert(bytes::Bytes::from(inserted)));
                }
            }
        }
    }

    /// Extends the current run with one more non-empty op.
    fn push(&mut self, op: Op, pending: &mut VecDeque<Op>) {
        debug_assert!(!op.is_empty());

        match op {
            Op::Keep(len) => match self {
                Self::Empty => *self = Self::Keep(len),
                Self::Keep(total_len) => *total_len += len,
                Self::Edit { .. } => {
                    self.flush_into(pending);
                    *self = Self::Keep(len);
                }
            },
            Op::Delete(len) => match self {
                Self::Empty => {
                    *self = Self::Edit {
                        delete: len,
                        insert: Vec::new(),
                    }
                }
                Self::Keep(_) => {
                    self.flush_into(pending);
                    *self = Self::Edit {
                        delete: len,
                        insert: Vec::new(),
                    };
                }
                Self::Edit {
                    delete: deleted, ..
                } => *deleted += len,
            },
            Op::Insert(buf) => match self {
                Self::Empty => {
                    *self = Self::Edit {
                        delete: 0,
                        insert: buf.to_vec(),
                    }
                }
                Self::Keep(_) => {
                    self.flush_into(pending);
                    *self = Self::Edit {
                        delete: 0,
                        insert: buf.to_vec(),
                    };
                }
                Self::Edit {
                    insert: inserted, ..
                } => inserted.extend_from_slice(buf.as_ref()),
            },
        }
    }
}

impl<I: Iterator<Item = Op>> Compact<I> {
    /// Creates a compacting adaptor over an op stream.
    pub fn new(iter: I) -> Compact<I> {
        Compact {
            iter,
            accumulation: Accumulation::Empty,
            pending: VecDeque::new(),
        }
    }
}

impl<I: Iterator<Item = Op>> Iterator for Compact<I> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(op) = self.pending.pop_front() {
                return Some(op);
            }

            let Some(op) = self.iter.next() else {
                self.accumulation.flush_into(&mut self.pending);
                return self.pending.pop_front();
            };

            if op.is_empty() {
                continue;
            }

            self.accumulation.push(op, &mut self.pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_compact_consecutive_keep() {
        let ops = [
            Op::Delete(20),
            Op::Keep(10),
            Op::Keep(20),
            Op::Keep(30),
            Op::Insert(Bytes::from_static(b"hello")),
            Op::Keep(5),
            Op::Keep(1),
        ];
        let answer = vec![
            Op::Delete(20),
            Op::Keep(60),
            Op::Insert(Bytes::from_static(b"hello")),
            Op::Keep(6),
        ];

        let normalized_ops: Vec<_> = Compact::new(ops.into_iter()).collect();

        assert_eq!(answer, normalized_ops);
    }

    #[test]
    fn test_compact_consecutive_delete() {
        let ops = [
            Op::Delete(20),
            Op::Delete(10),
            Op::Keep(10),
            Op::Delete(20),
            Op::Insert(Bytes::from_static(b"hello")),
            Op::Delete(23),
            Op::Delete(30),
            Op::Keep(30),
            Op::Delete(70),
            Op::Insert(Bytes::from_static(b"hello")),
            Op::Delete(0),
        ];
        let answer = vec![
            Op::Delete(30),
            Op::Keep(10),
            Op::Delete(73),
            Op::Insert(Bytes::from_static(b"hello")),
            Op::Keep(30),
            Op::Delete(70),
            Op::Insert(Bytes::from_static(b"hello")),
        ];

        let normalized_ops: Vec<_> = Compact::new(ops.into_iter()).collect();

        assert_eq!(answer, normalized_ops);
    }

    #[test]
    fn test_compact_consecutive_insert() {
        let ops = [
            Op::Insert(Bytes::from_static(b"abc")),
            Op::Insert(Bytes::from_static(b"123")),
            Op::Keep(10),
            Op::Insert(Bytes::from_static(b"Hello")),
            Op::Insert(Bytes::from_static(b"World")),
            Op::Delete(70),
            Op::Insert(Bytes::from_static(b"!")),
            Op::Keep(30),
            Op::Delete(70),
            Op::Insert(Bytes::from_static(b"42")),
        ];
        let answer = vec![
            Op::Insert(Bytes::from_static(b"abc123")),
            Op::Keep(10),
            Op::Delete(70),
            Op::Insert(Bytes::from_static(b"HelloWorld!")),
            Op::Keep(30),
            Op::Delete(70),
            Op::Insert(Bytes::from_static(b"42")),
        ];

        let normalized_ops: Vec<_> = Compact::new(ops.into_iter()).collect();

        assert_eq!(answer, normalized_ops);
    }

    #[test]
    fn test_compact_ops_of_zero_len() {
        let ops = [
            Op::Delete(10),
            Op::Insert(Bytes::from_static(b"")),
            Op::Delete(0),
            Op::Keep(0),
            Op::Delete(70),
            Op::Insert(Bytes::from_static(b"42")),
            Op::Keep(0),
        ];
        let answer = vec![Op::Delete(80), Op::Insert(Bytes::from_static(b"42"))];

        let normalized_ops: Vec<_> = Compact::new(ops.into_iter()).collect();

        assert_eq!(answer, normalized_ops);
    }

    #[test]
    fn test_compact_arbitrary_case() {
        let ops = [
            Op::Delete(10),
            Op::Insert(Bytes::from_static(b"Hello")),
            Op::Delete(0),
            Op::Keep(10),
            Op::Keep(40),
            Op::Delete(0),
            Op::Keep(90),
            Op::Delete(70),
            Op::Delete(20),
            Op::Insert(Bytes::from_static(b"Goodbye")),
            Op::Keep(0),
            Op::Insert(Bytes::from_static(b"42")),
            Op::Insert(Bytes::from_static(b"43")),
            Op::Delete(70),
            Op::Keep(20),
            Op::Insert(Bytes::from_static(b"")),
        ];
        let answer = vec![
            Op::Delete(10),
            Op::Insert(Bytes::from_static(b"Hello")),
            Op::Keep(140),
            Op::Delete(160),
            Op::Insert(Bytes::from_static(b"Goodbye4243")),
            Op::Keep(20),
        ];

        let normalized_ops: Vec<_> = Compact::new(ops.into_iter()).collect();

        assert_eq!(answer, normalized_ops);
    }
}
