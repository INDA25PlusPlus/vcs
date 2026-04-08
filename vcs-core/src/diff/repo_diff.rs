use std::collections::HashMap;

use crate::{crypto::CryptoHash, path::RepoPath};

pub type RepoDiffRef<H: CryptoHash> = H;

/// Per-file diffs for a commit.
#[derive(Debug)]
pub struct RepoDiff<H: CryptoHash> {
    file_diffs: HashMap<RepoPath, H>,
}
