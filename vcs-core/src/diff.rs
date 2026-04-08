use crate::crypto::{CryptoHash, CryptoHashable};
use itertools::Itertools;

use crate::path::RepoPath;
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
#[derive(Eq, PartialEq, Clone)]
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

impl FileDiff {
    pub fn new(hunks: Box<[Hunk]>) -> FileDiff {
        FileDiff { hunks }
    }

    pub fn combine(self, other: FileDiff) -> FileDiff {
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

        let translated_b = self.translate_to_base_coords(&other.hunks);
        let merged_diffs = self.merge(translated_b, other.hunks);

        FileDiff::new(merged_diffs)
    }

    fn merge(self, translated_b: Box<[BaseTranslation]>, b_hunks: Box<[Hunk]>) -> Box<[Hunk]> {
        let mut a_hunks_it = self.hunks.into_iter().peekable();
        let mut b_hunks_it = translated_b.into_iter().zip(b_hunks).peekable();

        let mut merged_hunks = vec![];

        enum ActiveState {
            A(Hunk, usize, usize), // hunk, absolute offset, previous a's content len
            B(Hunk, BaseTranslation),
        }

        let mut active_hunk: Option<ActiveState> = None;
        let mut offset_accumulation = 0;

        loop {
            if active_hunk.is_none() {
                match (a_hunks_it.peek(), b_hunks_it.peek()) {
                    (None, None) => break,
                    (Some(a), None) => {
                        let content_len = a.content_after.len();
                        offset_accumulation += a.offset;
                        active_hunk = Some(ActiveState::A(
                            a_hunks_it.next().unwrap(),
                            offset_accumulation,
                            content_len,
                        ));
                    }
                    (None, Some(b)) => {
                        let b = b_hunks_it.next().unwrap();
                        active_hunk = Some(ActiveState::B(b.1, b.0));
                    }
                    (Some(a), Some((b_offset, _b))) => {
                        if offset_accumulation + a.offset <= b_offset.start {
                            let content_len = a.content_after.len();
                            offset_accumulation += a.offset;
                            active_hunk = Some(ActiveState::A(
                                a_hunks_it.next().unwrap(),
                                offset_accumulation,
                                content_len,
                            ));
                        } else {
                            let b = b_hunks_it.next().unwrap();
                            active_hunk = Some(ActiveState::B(b.1, b.0));
                        }
                    }
                }
                continue;
            }

            let current = active_hunk.as_mut().unwrap();

            if let Some((b_offset, b_hunk)) = b_hunks_it.peek() {
                let b_start = b_offset.start;
                let b_relative_start = b_offset.relative_start;
                let b_end = b_offset.end;
                let b_relative_end = b_offset.relative_end;

                // NOTE: maybe merge all the overlapping b and a first
                if let Some(b_relative_start) = b_relative_start {
                    let next_a = a_hunks_it.peek();
                    if next_a.is_none() || offset_accumulation + next_a.unwrap().offset > b_start {
                        // has to be A, cause the currents end can't be in an insert
                        let (a_hunk, abs_pos, content_len) = match current {
                            ActiveState::A(hunk, abs_pos, content_len) => {
                                (*hunk, *abs_pos, *content_len)
                            }
                            _ => unreachable!(),
                        };
                        let overlap_size = content_len - b_relative_start;
                        let internal_del_len = std::cmp::min(overlap_size, b_hunk.len_before);

                        let mut new_content = std::mem::take(&mut a_hunk.content_after).into_vec();

                        new_content.splice(
                            b_relative_start..b_relative_start + internal_del_len,
                            b_hunk.content_after.iter().cloned(),
                        );

                        a_hunk.content_after = new_content.into_boxed_slice();
                        let spillover_len = b_hunk.len_before - internal_del_len;
                        a_hunk.len_before += spillover_len;

                        if let Some(b_relative_end) = b_relative_end {
                            current = &mut ActiveState::B(
                                a_hunk,
                                BaseTranslation {
                                    start: abs_pos,
                                    end: b_end,
                                    relative_start: None,
                                    relative_end: Some(b_relative_end),
                                },
                            );
                        } else {
                            current = &mut ActiveState::A(a_hunk, abs_pos, content_len);
                        }

                        b_hunks_it.next();
                    }
                } else {
                }

                if let Some(b_realtive_end) = b_relative_end {
                } else {
                }

                match *b_offset {
                    BaseOffset::BaseChange { base_offset, .. } => {
                        if base_offset == current.offset + current.len_before {
                            current.len_before += b_hunk.len_before;

                            let mut new_content =
                                std::mem::take(&mut current.content_after).into_vec();
                            new_content.extend_from_slice(&b_hunk.content_after);
                            current.content_after = new_content.into_boxed_slice();

                            b_hunks_it.next();
                        }
                    }
                    BaseOffset::InsertChange {
                        base_offset,
                        relative_offset,
                        ..
                    } => {
                        if base_offset == current.offset {
                            let overlap_size = current.content_after.len() - relative_offset;
                            let internal_del_len = std::cmp::min(overlap_size, b_hunk.len_before);

                            let mut new_content =
                                std::mem::take(&mut current.content_after).into_vec();

                            new_content.splice(
                                relative_offset..relative_offset + internal_del_len,
                                b_hunk.content_after.iter().cloned(),
                            );

                            current.content_after = new_content.into_boxed_slice();

                            let spillover_len = b_hunk.len_before - internal_del_len;
                            current.len_before += spillover_len;

                            b_hunks_it.next();
                        }
                    }
                }
            }

            if let Some(a_hunk) = a_hunks_it.peek() {
                if current.offset + current.len_before >= a_hunk.offset {
                    let overlap_size = current.offset + current.len_before - a_hunk.offset;
                    let internal_del_len = std::cmp::min(overlap_size, a_hunk.content_after.len());

                    let mut new_content = std::mem::take(&mut current.content_after).into_vec();
                    new_content.extend_from_slice(&a_hunk.content_after[internal_del_len..]);
                    current.content_after = new_content.into_boxed_slice();

                    current.len_before -= internal_del_len;

                    a_hunks_it.next();
                }
            }

            // if let Some(active_hunk) = active_hunk.take() {
            //     merged_hunks.push(active_hunk);
            // }
        }

        merged_hunks.into_boxed_slice()
    }

    /// Transform the offsets of b_hunks so they correspond to where that change would happen in the original file
    fn translate_to_base_coords(&self, b_hunks: &[Hunk]) -> Box<[BaseTranslation]> {
        // Start with very slow approach
        let map_to_base_offset = |b_offset: usize| -> (usize, Option<usize>) {
            let mut a_it = self.hunks.iter().peekable();
            let mut running_offset: isize = 0;
            let mut accumulated_offset: usize = 0;

            while let Some(&a_hunk) = a_it.peek() {
                accumulated_offset += a_hunk.offset;

                let a_running_offset = (accumulated_offset as isize + running_offset) as usize;
                let a_inserted_len = a_hunk.content_after.len();
                let a_end = a_running_offset + a_inserted_len;

                if b_offset < a_running_offset {
                    break;
                }

                if b_offset >= a_running_offset && b_offset < a_end {
                    let relative_offset = Some(b_offset - a_running_offset);
                    return (accumulated_offset, relative_offset);
                }

                running_offset += a_inserted_len as isize - a_hunk.len_before as isize;
                a_it.next();
            }

            ((b_offset as isize - running_offset) as usize, None)
        };

        let mut translations = vec![];

        let mut b_it = b_hunks.iter();
        let mut accumulated_offset: usize = 0;
        while let Some(b_hunk) = b_it.next() {
            accumulated_offset += b_hunk.offset;
            let (start, relative_start) = map_to_base_offset(accumulated_offset);
            let (end, relative_end) = map_to_base_offset(accumulated_offset + b_hunk.len_before);

            translations.push(BaseTranslation {
                start,
                end,
                relative_start,
                relative_end,
            });
        }

        translations.into_boxed_slice()
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
    fn test_translation_offset() {
        /*
        -------
        | |
        | |
        |A|
        |B|
        |C|
        |D|
        |E|
        |F|
        | |
        |1|
        |2|
        | |
        | |
        | |
        |q|
        |w|
        |e|
        |r|
        |t|
        |y|
        ------
         */
        let a_offsets = [[2 as usize, 1, 3], [1, 10, 12], [1, 6, 5]];
        let a_content_after = [
            ["ABCDEF", "12", "qwerty"],
            ["", "", ""],
            ["xyz", "-+*/", "987"],
        ];
        let a_len_before = [[0 as usize, 0, 0], [2, 7, 1], [2, 3, 1]];

        let b_offsets = [[1 as usize, 7, 4, 2, 2], [0, 4, 3, 11, 5], [0, 8, 3, 4, 2]];
        let b_content_after = [
            ["Hello", "World", "!", "Rust", "Cargo"],
            ["", "", "", "", ""],
            ["XY", "cm", "-_-", "P", "{}[]|"],
        ];
        let b_len_before = [[0 as usize, 0, 0, 0, 0], [3, 2, 7, 3, 1], [4, 2, 2, 1, 4]];

        let chaos_coords = [
            BaseTranslation {
                start: 0,
                end: 3,
                relative_start: None,
                relative_end: None,
            },
            BaseTranslation {
                start: 7,
                end: 7,
                relative_start: Some(0),
                relative_end: Some(2),
            },
            BaseTranslation {
                start: 7,
                end: 11,
                relative_start: Some(3),
                relative_end: None,
            },
            BaseTranslation {
                start: 12,
                end: 12,
                relative_start: Some(1),
                relative_end: Some(2),
            },
            BaseTranslation {
                start: 13,
                end: 17,
                relative_start: None,
                relative_end: None,
            },
        ];

        let translated_coords = [
            [
                [
                    BaseTranslation {
                        start: 1,
                        end: 1,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 2,
                        end: 2,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 4,
                        end: 4,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 6,
                        end: 6,
                        relative_start: Some(0),
                        relative_end: Some(0),
                    },
                    BaseTranslation {
                        start: 6,
                        end: 6,
                        relative_start: Some(2),
                        relative_end: Some(2),
                    },
                ],
                [
                    BaseTranslation {
                        start: 3,
                        end: 3,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 10,
                        end: 10,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 21,
                        end: 21,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 24,
                        end: 24,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 26,
                        end: 26,
                        relative_start: None,
                        relative_end: None,
                    },
                ],
            ],
            [
                [
                    BaseTranslation {
                        start: 0,
                        end: 2,
                        relative_start: None,
                        relative_end: Some(1),
                    },
                    BaseTranslation {
                        start: 2,
                        end: 2,
                        relative_start: Some(2),
                        relative_end: Some(4),
                    },
                    BaseTranslation {
                        start: 2,
                        end: 6,
                        relative_start: Some(5),
                        relative_end: Some(0),
                    },
                    BaseTranslation {
                        start: 6,
                        end: 7,
                        relative_start: Some(4),
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 9,
                        end: 10,
                        relative_start: None,
                        relative_end: None,
                    },
                ],
                [
                    BaseTranslation {
                        start: 0,
                        end: 5,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 6,
                        end: 8,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 9,
                        end: 24,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 28,
                        end: 31,
                        relative_start: None,
                        relative_end: None,
                    },
                    BaseTranslation {
                        start: 33,
                        end: 34,
                        relative_start: None,
                        relative_end: None,
                    },
                ],
            ],
        ];

        let get_a_hunks = |index: usize| {
            a_offsets[index]
                .into_iter()
                .zip(a_content_after[index])
                .zip(a_len_before[index])
                .map(|((offset, content_after), len_before)| Hunk {
                    offset,
                    content_after: Box::from(content_after.as_bytes()),
                    len_before,
                })
        };

        let get_b_hunks = |index: usize| {
            b_offsets[index]
                .into_iter()
                .zip(b_content_after[index])
                .zip(b_len_before[index])
                .map(|((offset, content_after), len_before)| Hunk {
                    offset,
                    content_after: Box::from(content_after.as_bytes()),
                    len_before,
                })
        };

        let a_hunks_addition: Box<_> = get_a_hunks(0).collect();
        let a_addition = FileDiff::new(a_hunks_addition);
        let a_hunks_deletions: Box<_> = get_a_hunks(1).collect();
        let a_deletion = FileDiff::new(a_hunks_deletions);

        let b_hunks_addition: Box<_> = get_b_hunks(0).collect();
        let b_hunks_deletion: Box<_> = get_b_hunks(1).collect();

        // B: 0
        let b_coords = a_addition.translate_to_base_coords(&b_hunks_addition);
        for (answer, b) in translated_coords[0][0].iter().zip(b_coords) {
            assert_eq!(*answer, b);
        }

        let b_coords = a_deletion.translate_to_base_coords(&b_hunks_addition);
        for (answer, b) in translated_coords[0][1].iter().zip(b_coords) {
            assert_eq!(*answer, b);
        }

        // B: 1
        let b_coords = a_addition.translate_to_base_coords(&b_hunks_deletion);
        for (answer, b) in translated_coords[1][0].iter().zip(b_coords) {
            assert_eq!(*answer, b);
        }

        let b_coords = a_deletion.translate_to_base_coords(&b_hunks_deletion);
        for (answer, b) in translated_coords[1][1].iter().zip(b_coords) {
            assert_eq!(*answer, b);
        }

        let a_hunks_chaos: Box<_> = get_a_hunks(2).collect();
        let a_chaos = FileDiff::new(a_hunks_chaos);
        let b_hunks_chaos: Box<_> = get_b_hunks(2).collect();

        let b_coords = a_chaos.translate_to_base_coords(&b_hunks_chaos);
        for (answer, b) in chaos_coords.iter().zip(b_coords) {
            assert_eq!(*answer, b);
        }

        // let b_coords = a_deletion.translate_to_base_coords(&b_hunks_addition);
        // assert_eq!(translated_coords[0][1].as_ref(), b_coords.as_ref());
    }
}
