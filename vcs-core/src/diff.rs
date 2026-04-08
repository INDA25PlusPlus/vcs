use crate::crypto::{CryptoHash, CryptoHashable};
use itertools::Itertools;

use crate::path::RepoPath;
use bytes::Bytes;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::usize;

pub type RepoDiffRef<H: CryptoHash> = H;

/// Per-file diffs for a commit.
#[derive(Debug)]
pub struct RepoDiff<H: CryptoHash> {
    file_diffs: HashMap<RepoPath, H>,
}

/// Byte-level edits for one file.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct FileDiff {
    /// OBS: Invariant: The offsets of the hunks are in strict ascending order
    hunks: Box<[Hunk]>,
}

/// One contiguous replacement in the file.
#[derive(Eq, PartialEq, Clone, Default)]
pub struct Hunk {
    // Gap from the previous hunk, or from file start.
    offset: usize,
    // Number of source bytes replaced removed.
    len_before: usize,
    // Replacement bytes written at this position.
    content_after: Box<[u8]>,
}

impl PartialOrd for Hunk {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.offset.cmp(&other.offset))
    }
}

enum BaseOffset {
    BaseChange {
        base_offset: usize,
        base_len: usize,
    },
    InsertChange {
        base_offset: usize,
        base_len: usize,
        relative_offset: usize,
    },
}

impl BaseOffset {
    fn offset(&self) -> usize {
        match self {
            BaseOffset::BaseChange { base_offset, .. } => *base_offset,
            BaseOffset::InsertChange { base_offset, .. } => *base_offset,
        }
    }

    fn len(&self) -> usize {
        match self {
            BaseOffset::BaseChange { base_len, .. } => *base_len,
            BaseOffset::InsertChange { base_len, .. } => *base_len,
        }
    }
}

#[derive(PartialEq, Debug)]
struct BaseTranslation {
    pub start: usize,
    pub end: usize,
    pub relative_start: Option<usize>,
    pub relative_end: Option<usize>,
}

enum Op {
    Keep(usize),
    Delete(usize),
    Insert(Bytes),
}

impl Op {
    fn split_of(&mut self, len: usize) -> Op {
        match self {
            Op::Keep(total_len) => {
                *total_len -= len;
                Op::Keep(len)
            }
            Op::Delete(total_len) => {
                *total_len -= len;
                Op::Delete(len)
            }
            Op::Insert(buf) => {
                let left = buf.split_to(len);
                Op::Insert(left)
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Op::Keep(len) | Op::Delete(len) => *len,
            Op::Insert(buf) => buf.len(),
        }
    }
}

struct OpStream(Vec<Op>);

impl OpStream {
    fn into_hunks(self) -> Box<[Hunk]> {
        let mut hunks = Vec::new();
        let mut current_hunk: Option<Hunk> = None;
        let mut offset: usize = 0;

        for op in self.0 {
            match op {
                Op::Keep(len) => {
                    if let Some(current_hunk) = current_hunk.take() {
                        hunks.push(current_hunk);
                    }

                    offset += len;
                }
                Op::Delete(len) => {
                    let mut hunk = current_hunk.unwrap_or_else(|| {
                        let h = Hunk {
                            offset: offset,
                            len_before: 0,
                            content_after: Box::new([]),
                        };
                        offset = 0;
                        h
                    });

                    hunk.len_before += len;
                    offset += len;

                    current_hunk = Some(hunk);
                }
                Op::Insert(buf) => {
                    let mut hunk = current_hunk.unwrap_or_else(|| {
                        let h = Hunk {
                            offset: offset,
                            len_before: 0,
                            content_after: Box::new([]),
                        };
                        offset = 0;
                        h
                    });

                    let mut new_content = hunk.content_after.into_vec();
                    new_content.extend_from_slice(&buf);
                    hunk.content_after = new_content.into_boxed_slice();

                    current_hunk = Some(hunk);
                }
            }
        }

        if let Some(hunk) = current_hunk {
            hunks.push(hunk);
        }

        hunks.into_boxed_slice()
    }
}

struct OpStreamIter<I: Iterator<Item = Op>> {
    iter: I,
    backlog: Option<Op>,
}

impl<I: Iterator<Item = Op>> OpStreamIter<I> {
    pub fn pull(&mut self, amount: usize) -> Option<Op> {
        let mut current = match self.backlog.take().or_else(|| self.next()) {
            Some(op) => op,
            None => return Some(Op::Keep(amount)), // Pretend there is stuff left
        };

        if amount < current.len() {
            let taken = current.split_of(amount);

            self.backlog = Some(current);
            Some(taken)
        } else {
            Some(current)
        }
    }
}

impl<I: Iterator<Item = Op>> Iterator for OpStreamIter<I> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        self.backlog.take().or_else(|| self.iter.next())
    }
}

impl IntoIterator for OpStream {
    type Item = Op;
    type IntoIter = OpStreamIter<std::vec::IntoIter<Op>>;

    fn into_iter(self) -> Self::IntoIter {
        OpStreamIter {
            iter: self.0.into_iter(),
            backlog: None,
        }
    }
}

impl FromIterator<Hunk> for OpStream {
    fn from_iter<T: IntoIterator<Item = Hunk>>(hunks: T) -> Self {
        let mut ops = Vec::new();
        let mut prev_len_before = 0usize;

        for hunk in hunks {
            let actual_keep = hunk.offset.saturating_sub(prev_len_before);

            if actual_keep > 0 {
                ops.push(Op::Keep(actual_keep));
            }
            if hunk.len_before > 0 {
                ops.push(Op::Delete(hunk.len_before));
            }
            if !hunk.content_after.is_empty() {
                ops.push(Op::Insert(Bytes::from(hunk.content_after)));
            }

            prev_len_before = hunk.len_before;
        }

        OpStream(ops)
    }
}

impl FileDiff {
    pub fn new(hunks: Box<[Hunk]>) -> FileDiff {
        FileDiff { hunks }
    }

    pub fn unify(self, other: FileDiff) -> FileDiff {
        if self.hunks.is_empty() && other.hunks.is_empty() {
            return FileDiff {
                hunks: Box::new([]),
            };
        }
        if other.hunks.is_empty() {
            return self;
        }
        if self.hunks.is_empty() {
            return other;
        }

        // Maybe do directly from other instead of other.hunks; thus abstract over FileDiff instead
        let other_ops: OpStream = other.hunks.into_iter().collect();

        let merged_ops = self.merge_streams(other_ops.into_iter());
        let merged_hunks: Box<[Hunk]> = merged_ops.into_hunks();

        // let translated_b = self.translate_to_base_coords(&other.hunks);
        // let merged_diffs = self.merge(translated_b, other.hunks);

        FileDiff::new(merged_hunks)
    }

    fn merge_streams<I: Iterator<Item = Op>>(self, other_ops: OpStreamIter<I>) -> OpStream {
        let b_ops = other_ops;

        // Maybe cache self stream
        let a_ops: OpStream = self.hunks.into_iter().collect();
        let mut a_ops_iter = a_ops.into_iter();
        let mut final_ops = Vec::new();

        for b_op in b_ops {
            match b_op {
                Op::Insert(buf) => final_ops.push(Op::Insert(buf)),
                Op::Keep(mut b_len) => {
                    while b_len > 0 {
                        let a_op = a_ops_iter.pull(b_len).expect("Stream A ended unprompted!");

                        match a_op {
                            Op::Keep(a_len) => {
                                b_len -= a_len;
                                final_ops.push(Op::Keep(a_len));
                            }
                            Op::Delete(a_len) => {
                                final_ops.push(Op::Delete(a_len));
                            }
                            Op::Insert(a_buf) => {
                                b_len -= a_buf.len();
                                final_ops.push(Op::Insert(a_buf));
                            }
                        }
                    }
                }
                Op::Delete(mut b_len) => {
                    while b_len > 0 {
                        let a_op = a_ops_iter.pull(b_len).expect("Stream A ended unprompted!");

                        match a_op {
                            Op::Keep(a_len) => {
                                final_ops.push(Op::Delete(a_len));
                                b_len -= a_len;
                            }
                            Op::Delete(a_len) => {
                                final_ops.push(Op::Delete(a_len));
                            }
                            Op::Insert(a_buf) => {
                                b_len -= a_buf.len();
                            }
                        }
                    }
                }
            }
        }

        for a_op in a_ops_iter {
            final_ops.push(a_op);
        }

        OpStream(final_ops)
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
    use aws_lc_rs::rand;

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

    #[test]
    fn test_naive_fuzzy() {}

    #[test]
    fn test_naive_diff_from_file_content() {}

    #[test]
    fn test_unify_fuzzy() {
        rand
    }
}
