//! Lazy op-stream adaptors used for transforming diffs.
//!
//! It exists for operations such as:
//!
//! - sequential composition of diffs
//! - periodic compaction of long compose pipelines
//! - delaying materialization until the final result is needed
//!

pub mod compact;
pub mod compose;

use bytes::Bytes;

use crate::diff::operations::{compact::Compact, compose::Compose};

/// A single edit operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Keep this many source bytes unchanged.
    Keep(usize),
    /// Delete this many source bytes.
    Delete(usize),
    /// Insert these bytes at the current position.
    Insert(Bytes),
}

impl Op {
    /// Splits off the first `len` units from this [`Op`].
    ///
    /// `len` must be smaller than the current [`Op`] length.
    pub fn split_prefix(&mut self, len: usize) -> Op {
        debug_assert!(len < self.len());
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

    /// Returns the length contributed by this [`Op`].
    pub fn len(&self) -> usize {
        match self {
            Op::Keep(len) | Op::Delete(len) => *len,
            Op::Insert(buf) => buf.len(),
        }
    }

    /// Returns true if self is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Small cursor wrapper that supports partially consuming ops.
#[derive(Debug)]
pub(crate) struct OpCursor<I: Iterator<Item = Op>> {
    source: I,
    pending: Option<Op>,
}

impl<I: Iterator<Item = Op>> OpCursor<I> {
    pub(crate) fn new(iter: I) -> Self {
        Self {
            source: iter,
            pending: None,
        }
    }

    /// Pulls up to `amount` units from the next op.
    ///
    /// If the underlying iterator is exhausted, the remainder is treated as an implicit keep.
    pub(crate) fn pull(&mut self, amount: usize) -> Option<Op> {
        let mut current = match self.pending.take().or_else(|| self.next()) {
            Some(op) => op,
            None => return Some(Op::Keep(amount)), // Treat exhaustion as an implicit keep.
        };

        if amount < current.len() {
            let taken = current.split_prefix(amount);

            self.pending = Some(current);
            Some(taken)
        } else {
            Some(current)
        }
    }
}

impl<I: Iterator<Item = Op>> Iterator for OpCursor<I> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        self.pending.take().or_else(|| self.source.next())
    }
}

/// Extension methods for op streams.
pub trait OpStreamExt: Iterator<Item = Op> + Sized {
    /// Lazily composes two op streams.
    ///
    /// If `self` maps `A -> B` and `other` maps `B -> C`, the resulting stream maps `A -> C`.
    fn compose<O: Iterator<Item = Op>>(self, other: O) -> Compose<Self, O> {
        Compose::new(self, other)
    }

    /// Lazily compacts the stream into standard keep and edit runs.
    fn compact(self) -> Compact<Self> {
        Compact::new(self)
    }
}

impl<I: Iterator<Item = Op>> OpStreamExt for I {}
