use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::hunk_collection::HunkCollection;
use crypto_hash_derive::CryptoHash;
use std::fmt::{Debug, Formatter};

/// A change made to a file from one revision to another
#[derive(Clone, Debug, CryptoHash)]
pub enum FileChange<D: CryptoDigest + CryptoHash> {
    Create(FileRef<D>),
    Modify(FileDiffRef<D>),
    Delete,
}

/// The full contents of a file
#[derive(Clone)]
pub struct File {
    content: Box<[u8]>,
    executable_status: bool,
}

/// A collection of changes made to a file
#[derive(Clone, Debug)]
pub struct FileDiff {
    hunks: HunkCollection,
    executable_status: bool,
}

pub type FileRef<D> = D;

pub type FileDiffRef<D> = D;

impl Debug for File {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("File");

        match std::str::from_utf8(&self.content) {
            Ok(text) => dbg.field("content_after", &text),
            Err(_) => dbg.field("content_after", &self.content),
        };

        dbg.field("executable_status", &self.executable_status);
        dbg.finish()
    }
}
