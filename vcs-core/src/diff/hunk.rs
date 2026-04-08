use std::fmt::{Debug, Formatter};

/// One contiguous edit in a file.
#[derive(Eq, PartialEq, Clone, Default)]
pub struct Hunk {
    // Bytes from the previous hunk, or from the file start.
    pub offset: usize,
    // Source bytes removed at this position.
    pub len_before: usize,
    // Bytes inserted at this position.
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
