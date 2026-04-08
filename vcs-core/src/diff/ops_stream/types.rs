use bytes::Bytes;

/// A single edit operation in the op stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Keep(usize),
    Delete(usize),
    Insert(Bytes),
}

impl Op {
    /// Splits off the first `len` units from this op.
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

    /// Returns the op length.
    pub fn len(&self) -> usize {
        match self {
            Op::Keep(len) | Op::Delete(len) => *len,
            Op::Insert(buf) => buf.len(),
        }
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
