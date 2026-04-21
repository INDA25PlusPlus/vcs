use crate::crypto::digest::CryptoDigest;

pub struct Index<H: CryptoDigest> {
    pub repo_diff: H,
}
