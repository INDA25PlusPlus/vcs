use crate::crypto::CryptoHash;
use crate::path::RepoPath;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

#[derive(Debug)]
pub struct RepoDiff<HashType> {
    file_diffs: HashMap<RepoPath, HashType>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct FileDiff {
    hunks: Box<[Hunk]>,
}

#[derive(Eq, PartialEq)]
struct Hunk {
    // byte offset from end of previous hunk, or start of file
    offset: usize,
    len_before: usize,
    content_after: Box<[u8]>,
}

impl FileDiff {
    pub fn new(buf: &[u8]) -> FileDiff {
        todo!()
    }

    pub fn apply(&self, buf: &mut Vec<u8>) {
        todo!()
    }
}

impl CryptoHash for FileDiff {
    fn crypto_hash(&self) -> blake3::Hash {
        todo!()
    }
}

impl Debug for Hunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!("maybe print `content_after` as utf-8 instead?")
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
