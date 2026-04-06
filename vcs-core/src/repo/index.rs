use crate::crypto::CryptoHash;

pub struct Index<H: CryptoHash> {
    pub repo_diff: H,
}
