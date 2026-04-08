use std::{
    collections::{HashMap, VecDeque},
    fmt::{Debug, Formatter},
};

use bytes::Bytes;

use crate::crypto::{CryptoHash, CryptoHashable};
use crate::{
    diff::{Op, OpIter},
    path::RepoPath,
};

pub type RepoDiffRef<H: CryptoHash> = H;

/// Per-file diffs for a commit.
#[derive(Debug)]
pub struct RepoDiff<H: CryptoHash> {
    file_diffs: HashMap<RepoPath, H>,
}

/// Byte-level edits for one file.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct FileDiff {
    pub hunks: Box<[Hunk]>,
}

/// One contiguous replacement in the file.
#[derive(Eq, PartialEq, Clone, Default)]
pub struct Hunk {
    // Gap from the previous hunk, or from file start.
    pub offset: usize,
    // Number of source bytes replaced removed.
    pub len_before: usize,
    // Replacement bytes written at this position.
    pub content_after: Box<[u8]>,
}

/// Iterator bridge between hunks and ops
pub struct HunkToOpIter<I: Iterator<Item = Hunk>> {
    pub hunks_iter: I,
    pub backlog: VecDeque<Op>,
    pub prev_len_before: usize,
}

impl Debug for Hunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!("maybe print `content_after` as utf-8 instead?")
    }
}

impl FileDiff {
    pub fn new(hunks: Box<[Hunk]>) -> FileDiff {
        FileDiff { hunks }
    }

    pub fn unify(self, other: FileDiff) -> FileDiff {
        self.into_iter().unify(other.into_iter()).collect()
    }
}

impl IntoIterator for FileDiff {
    type Item = Op;
    type IntoIter = OpIter<HunkToOpIter<std::vec::IntoIter<Hunk>>>;

    fn into_iter(self) -> Self::IntoIter {
        OpIter {
            iter: HunkToOpIter {
                hunks_iter: self.hunks.into_iter(),
                backlog: VecDeque::new(),
                prev_len_before: 0,
            },
            backlog: None,
        }
    }
}

impl FromIterator<Op> for FileDiff {
    fn from_iter<T: IntoIterator<Item = Op>>(iter: T) -> Self {
        let mut hunks = Vec::new();
        let mut current_hunk: Option<Hunk> = None;
        let mut offset: usize = 0;
        let mut cached_content: Vec<u8> = Vec::new();

        for op in iter {
            match op {
                Op::Keep(len) => {
                    if let Some(mut current_hunk) = current_hunk.take() {
                        if !cached_content.is_empty() {
                            current_hunk.content_after = cached_content.into_boxed_slice();
                        }
                        hunks.push(current_hunk);
                        cached_content = Vec::new();
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

                    cached_content.extend_from_slice(&buf);

                    current_hunk = Some(hunk);
                }
            }
        }

        if let Some(mut hunk) = current_hunk {
            if !cached_content.is_empty() {
                hunk.content_after = cached_content.into_boxed_slice();
            }
            hunks.push(hunk);
        }

        FileDiff {
            hunks: hunks.into_boxed_slice(),
        }
    }
}

impl<I: Iterator<Item = Hunk>> Iterator for HunkToOpIter<I> {
    type Item = Op;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(op) = self.backlog.pop_front() {
            return Some(op);
        }

        let hunk = self.hunks_iter.next()?;

        let actual_keep = hunk.offset.saturating_sub(self.prev_len_before);

        if actual_keep > 0 {
            self.backlog.push_back(Op::Keep(actual_keep));
        }
        if hunk.len_before > 0 {
            self.backlog.push_back(Op::Delete(hunk.len_before));
        }
        if !hunk.content_after.is_empty() {
            self.backlog
                .push_back(Op::Insert(Bytes::from(hunk.content_after)));
        }

        self.prev_len_before = hunk.len_before;

        self.backlog.pop_front()
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

    struct SimpleRng {
        state: u32,
    }

    impl SimpleRng {
        fn new(seed: u32) -> Self {
            Self { state: seed }
        }

        fn next_u8(&mut self) -> u8 {
            // Classic LCG constants (used by glibc)
            self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
            // We take the high bits because they are "more random" in LCGs
            ((self.state >> 16) & 0xFF) as u8
        }
    }

    fn apply_diff(base: &[u8], diff: &FileDiff) -> Box<[u8]> {
        let mut out = Vec::new();
        let mut absolute_offset = 0;
        let mut start_of_content = 0;

        for hunk in diff.hunks.iter() {
            absolute_offset += hunk.offset;

            if absolute_offset > start_of_content {
                out.extend_from_slice(&base[start_of_content..absolute_offset]);
            }

            out.extend_from_slice(&hunk.content_after);

            start_of_content = absolute_offset + hunk.len_before;
        }

        if start_of_content < base.len() {
            out.extend_from_slice(&base[start_of_content..]);
        }

        out.into_boxed_slice()
    }

    #[test]
    fn test_apply_diff() {
        let base: Box<[u8]> =
            Box::from("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".as_bytes());
        let answer: Box<[u8]> =
            Box::from("1112cdi3456nopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".as_bytes());

        let diff = FileDiff::new(Box::from([
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
                content_after: Box::from("".as_bytes()),
                len_before: 4,
                offset: 2,
            },
            Hunk {
                content_after: Box::from("3456".as_bytes()),
                len_before: 4,
                offset: 5,
            },
        ]));

        let result = apply_diff(&base, &diff);

        assert_eq!(result, answer)
    }

    #[test]
    fn test_unify_fuzzy() {
        let mut rng = SimpleRng { state: 42 };

        let generate_base = |rng: &mut SimpleRng| {
            let len = std::cmp::min(std::cmp::max(rng.next_u8() as usize, 5), 220);
            let base: Box<[u8]> = std::iter::repeat_with(|| rng.next_u8())
                .take(len as usize)
                .collect();
            base
        };

        let generate_diff = |rng: &mut SimpleRng, len: usize| {
            let mut generate_hunk = |min_offset, offset_range, max_remove| {
                let offset = min_offset + rng.next_u8() as usize % (offset_range + 1);
                let len_before = std::cmp::min(rng.next_u8() as usize, max_remove);

                let len = rng.next_u8() as usize % 15;
                let content: Box<[u8]> = std::iter::repeat_with(|| rng.next_u8())
                    .take(len as usize)
                    .collect();

                Hunk {
                    content_after: content,
                    len_before: len_before as usize,
                    offset: offset as usize,
                }
            };

            let mut diff = Vec::new();
            let mut offset = 0;
            let mut prev_remove = 0;
            while offset + prev_remove + 1 < len {
                let available = len - (offset + prev_remove + 1);
                let min_offset = prev_remove + 1;
                let range = available % 12;
                let remove = (available - range) % 9;

                let hunk = generate_hunk(min_offset, range, remove);
                offset += hunk.offset;
                prev_remove = hunk.len_before;
                diff.push(hunk);
            }

            diff
        };

        for i in 0..1000 {
            let base = generate_base(&mut rng);
            let a_diff = generate_diff(&mut rng, base.len());
            let a_file_diff = FileDiff {
                hunks: a_diff.into_boxed_slice(),
            };
            let result = apply_diff(&base, &a_file_diff);

            let b_diff = generate_diff(&mut rng, result.len());
            let b_file_diff = FileDiff {
                hunks: b_diff.into_boxed_slice(),
            };
            let true_result = apply_diff(&result, &b_file_diff);

            let result = apply_diff(&base, &a_file_diff.unify(b_file_diff));

            assert_eq!(
                true_result, result,
                "Unify Fuzzing failed on iteration: {}",
                i
            );
        }
    }
}
