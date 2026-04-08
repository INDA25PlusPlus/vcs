use bytes::Bytes;

/// 'Op' stands for operations and represents the different actions a hunk can take from a data streaming perspective
pub enum Op {
    Keep(usize),
    Delete(usize),
    Insert(Bytes),
}

impl Op {
    /// Breaks away a part of length 'len' from 'Op'
    pub fn split_of(&mut self, len: usize) -> Op {
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

    // Returns the len of the Op
    pub fn len(&self) -> usize {
        match self {
            Op::Keep(len) | Op::Delete(len) => *len,
            Op::Insert(buf) => buf.len(),
        }
    }
}

/// 'Op' iterator which adds the pull method to arbitrary 'Op' iterator
pub struct OpIter<I: Iterator<Item = Op>> {
    pub iter: I,
    pub backlog: Option<Op>,
}

/// Iterator handling the unification of two 'Op' iterators
pub struct Unify<A: Iterator<Item = Op>, B: Iterator<Item = Op>> {
    pub a: OpIter<A>,
    pub b: OpIter<B>,
    pub current_b: Option<Op>,
}

impl<I: Iterator<Item = Op>> OpIter<I> {
    /// Makes it possible to 'pull' an 'Op'. If amount is less than the 'Op' len, then 'Op' will 'split_of'
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

impl<I: Iterator<Item = Op>> Iterator for OpIter<I> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        self.backlog.take().or_else(|| self.iter.next())
    }
}

impl<A: Iterator<Item = Op>, B: Iterator<Item = Op>> Iterator for Unify<A, B> {
    type Item = Op;

    /// Iteratively unifies two 'Op' iterators
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let b_op = match self.current_b.take().or_else(|| self.b.next()) {
                Some(op) => op,
                None => return self.a.next(),
            };

            match b_op {
                Op::Insert(buf) => return Some(Op::Insert(buf)),
                Op::Keep(mut b_len) => {
                    if let Some(a_op) = self.a.pull(b_len) {
                        match a_op {
                            Op::Keep(a_len) => {
                                if b_len > a_len {
                                    self.current_b = Some(Op::Keep(b_len - a_len));
                                }
                                return Some(Op::Keep(a_len));
                            }
                            Op::Delete(a_len) => {
                                self.current_b = Some(Op::Keep(b_len));
                                return Some(Op::Delete(a_len));
                            }
                            Op::Insert(a_buf) => {
                                if b_len > a_buf.len() {
                                    self.current_b = Some(Op::Keep(b_len - a_buf.len()));
                                }
                                return Some(Op::Insert(a_buf));
                            }
                        }
                    }
                }
                Op::Delete(mut b_len) => {
                    if let Some(a_op) = self.a.pull(b_len) {
                        match a_op {
                            Op::Keep(a_len) => {
                                if b_len > a_len {
                                    self.current_b = Some(Op::Delete(b_len - a_len));
                                }
                                return Some(Op::Delete(a_len));
                            }
                            Op::Delete(a_len) => {
                                self.current_b = Some(Op::Delete(b_len));
                                return Some(Op::Delete(a_len));
                            }
                            Op::Insert(a_buf) => {
                                if b_len > a_buf.len() {
                                    self.current_b = Some(Op::Delete(b_len - a_buf.len()));
                                }
                                continue;
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<I: Iterator<Item = Op>> OpIter<I> {
    pub fn unify<O: Iterator<Item = Op>>(self, other: OpIter<O>) -> OpIter<Unify<Self, O>> {
        OpIter {
            iter: Unify {
                a: OpIter {
                    iter: self,
                    backlog: None,
                },
                b: other,
                current_b: None,
            },
            backlog: None,
        }
    }
}
