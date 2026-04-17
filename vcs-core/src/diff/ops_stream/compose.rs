use crate::diff::ops_stream::{Op, OpCursor};

/// Lazily composes two op streams into one equivalent stream.
pub struct Compose<A: Iterator<Item = Op>, B: Iterator<Item = Op>> {
    left: OpCursor<A>,
    right: OpCursor<B>,
    pending_right: Option<Op>,
}

impl<A: Iterator<Item = Op>, B: Iterator<Item = Op>> Compose<A, B> {
    /// Creates a lazy composition of two op streams.
    ///
    /// `left` is interpreted first and must map `A -> B`.
    /// `right` is interpreted second and must map `B -> C`.
    /// The resulting iterator yields the direct `A -> C` edit stream.
    pub fn new(left: A, right: B) -> Compose<A, B> {
        Compose {
            left: OpCursor::new(left),
            right: OpCursor::new(right),
            pending_right: None,
        }
    }
}

impl<A: Iterator<Item = Op>, B: Iterator<Item = Op>> Iterator for Compose<A, B> {
    type Item = Op;

    /// Yields the next op in the composed stream.
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let right_op = match self.pending_right.take().or_else(|| self.right.next()) {
                Some(op) => op,
                None => return self.left.next(),
            };

            match right_op {
                // Inserts from the second diff are already final.
                Op::Insert(buf) => return Some(Op::Insert(buf)),
                Op::Keep(right_len) => {
                    if let Some(left_op) = self.left.pull(right_len) {
                        match left_op {
                            Op::Keep(left_len) => {
                                if right_len > left_len {
                                    self.pending_right = Some(Op::Keep(right_len - left_len));
                                }
                                return Some(Op::Keep(left_len));
                            }
                            Op::Delete(left_len) => {
                                // A deletion from the first diff still wins here.
                                self.pending_right = Some(Op::Keep(right_len));
                                return Some(Op::Delete(left_len));
                            }
                            Op::Insert(left_buf) => {
                                // Keeping a newly inserted range forwards those inserted bytes.
                                if right_len > left_buf.len() {
                                    self.pending_right = Some(Op::Keep(right_len - left_buf.len()));
                                }
                                return Some(Op::Insert(left_buf));
                            }
                        }
                    }
                }
                Op::Delete(right_len) => {
                    if let Some(left_op) = self.left.pull(right_len) {
                        match left_op {
                            Op::Keep(left_len) => {
                                if right_len > left_len {
                                    self.pending_right = Some(Op::Delete(right_len - left_len));
                                }
                                return Some(Op::Delete(left_len));
                            }
                            Op::Delete(left_len) => {
                                // The later delete continues across already-deleted bytes.
                                self.pending_right = Some(Op::Delete(right_len));
                                return Some(Op::Delete(left_len));
                            }
                            Op::Insert(left_buf) => {
                                // Deleting bytes inserted by the first diff cancels them out.
                                if right_len > left_buf.len() {
                                    self.pending_right =
                                        Some(Op::Delete(right_len - left_buf.len()));
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

#[cfg(test)]
mod tests {
    use super::*;

    struct SimpleRng {
        state: u32,
    }

    impl SimpleRng {
        fn new(seed: u32) -> Self {
            Self { state: seed }
        }

        fn next_u8(&mut self) -> u8 {
            self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
            ((self.state >> 16) & 0xFF) as u8
        }
    }

    /// Applies an op stream to a byte slice.
    fn apply_ops(base: &[u8], ops: impl IntoIterator<Item = Op>) -> Box<[u8]> {
        let mut result = Vec::new();
        let mut cursor = base.iter().copied();

        for op in ops {
            match op {
                Op::Keep(len) => result.extend(cursor.by_ref().take(len)),
                Op::Delete(len) => {
                    assert!(len != 0);
                    cursor.nth(len - 1);
                }
                Op::Insert(buf) => result.extend(buf),
            }
        }

        result.extend(cursor);
        result.into_boxed_slice()
    }

    fn generate_base(rng: &mut SimpleRng) -> Box<[u8]> {
        let len = (rng.next_u8() as usize).clamp(5, 220);

        std::iter::repeat_with(|| rng.next_u8())
            .take(len)
            .collect::<Box<[u8]>>()
    }

    fn generate_op_stream(rng: &mut SimpleRng, mut len: usize) -> impl Iterator<Item = Op> {
        let random_keep = |rng: &mut SimpleRng, max_len: usize| {
            let len = (rng.next_u8() % 13 + 1) as usize;
            let len = std::cmp::min(len, max_len);
            Op::Keep(len)
        };
        let random_delete = |rng: &mut SimpleRng, max_len: usize| {
            let len = (rng.next_u8() % 9 + 1) as usize;
            let len = std::cmp::min(len, max_len);
            Op::Delete(len)
        };
        let random_insert = |rng: &mut SimpleRng| {
            let len = (rng.next_u8() % 15 + 1) as usize;
            let content = std::iter::repeat_with(|| rng.next_u8()).take(len).collect();
            Op::Insert(content)
        };

        let mut result = Vec::new();

        while len > 0 {
            let op = match rng.next_u8() % 3 {
                0 => random_keep(rng, len),
                1 => random_delete(rng, len),
                2 => random_insert(rng),
                _ => unreachable!("The rest of module 3 cannot be anything other than 0, 1, 2"),
            };
            if !matches!(op, Op::Insert(_)) {
                len -= op.len();
            }
            result.push(op);
        }

        result.into_iter()
    }

    #[test]
    fn test_edge_cases() {
        let mut rng = SimpleRng::new(42);

        // Two empty streams
        let empty1 = [].into_iter();
        let empty2 = [].into_iter();
        let result: Vec<Op> = Compose::new(empty1, empty2).collect();
        assert!(result.is_empty());

        // One empty stream
        let stream: Vec<Op> = generate_op_stream(&mut rng, 1024).collect();
        let empty: Vec<Op> = [].into_iter().collect();
        let result1: Vec<Op> =
            Compose::new(stream.clone().into_iter(), empty.clone().into_iter()).collect();
        let result2: Vec<Op> =
            Compose::new(empty.clone().into_iter(), stream.clone().into_iter()).collect();
        assert_eq!(result1, stream);
        assert_eq!(result2, stream);
    }

    #[test]
    fn test_compose_fuzzy() {
        let mut rng = SimpleRng::new(42);

        // Checks if composing three randomly generated streams produces the same result as applying them separately
        for i in 0..1000 {
            let base = generate_base(&mut rng);

            let a_vec: Vec<_> = generate_op_stream(&mut rng, base.len()).collect();
            let after_a = apply_ops(&base, a_vec.clone());

            let b_vec: Vec<_> = generate_op_stream(&mut rng, after_a.len()).collect();
            let after_b = apply_ops(&after_a, b_vec.clone());

            let c_vec: Vec<_> = generate_op_stream(&mut rng, after_b.len()).collect();
            let after_c = apply_ops(&after_b, c_vec.clone());

            let composed = Compose::new(
                Compose::new(a_vec.into_iter(), b_vec.into_iter()),
                c_vec.into_iter(),
            );
            let applied_composed = apply_ops(&base, composed);

            assert_eq!(
                after_c, applied_composed,
                "'compose_fuzzy()' failed on iteration: {}",
                i
            );
        }
    }
}
