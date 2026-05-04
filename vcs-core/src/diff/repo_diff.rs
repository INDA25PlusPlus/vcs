use std::collections::HashMap;

use crate::crypto::digest::{CryptoDigest, CryptoHash, CryptoHasher};
use crate::fs::file::FileChange;
use crate::fs::path::RepoPath;

/// A collection of changes made to a repository from one revision to another
#[derive(Clone, Debug)]
pub struct RepoDiff<D: CryptoDigest> {
    file_diffs: HashMap<RepoPath, FileChange<D>>,
}

pub type RepoDiffRef<D> = D;

impl<D: CryptoDigest> RepoDiff<D> {
    pub(crate) fn empty() -> RepoDiff<D> {
        RepoDiff {
            file_diffs: HashMap::new(),
        }
    }
}

impl<D: CryptoDigest> CryptoHash for RepoDiff<D> {
    fn crypto_hash<OutD: CryptoDigest, H: CryptoHasher<Output = OutD>>(&self, state: &mut H) {
        todo!()
    }
}
