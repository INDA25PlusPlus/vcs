use std::fmt::{Debug, Formatter};

/// One contiguous edit in a file.
///
/// A hunk replaces `len_before` bytes at a position with `content_after`.
/// `offset` is stored relative to the previous hunk rather than as an absolute file index.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Hunk {
    /// Number of source bytes between the previous hunk and this one.
    ///
    /// For the first hunk, this is measured from the start of the file.
    pub offset: usize,
    /// Number of source bytes removed at this position.
    pub len_before: usize,
    /// Bytes inserted at this position.
    pub content_after: Box<[u8]>,
}

impl Debug for Hunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hunk")
            .field("offset", &self.offset)
            .field("len_before", &self.len_before)
            .field("content_after", &self.content_after)
            .finish()
    }
}
