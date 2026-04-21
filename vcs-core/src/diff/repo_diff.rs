use std::collections::HashMap;

use crate::crypto::digest::CryptoDigest;
use crate::path::RepoPath;

pub type RepoDiffRef<H> = H;

/// Per-file diffs for a commit.
#[derive(Debug)]
pub struct RepoDiff<H: CryptoDigest> {
    file_diffs: HashMap<RepoPath, H>,
}
