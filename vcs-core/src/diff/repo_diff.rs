use std::collections::HashMap;

use crate::crypto::digest::{CryptoDigest, CryptoHash, CryptoHasher};
use crate::fs::path::RepoPath;

pub type RepoDiffRef<D> = D;

/// Per-file diffs for a commit.
#[derive(Clone, Debug)]
pub struct RepoDiff<D: CryptoDigest> {
    file_diffs: HashMap<RepoPath, D>,
}

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
