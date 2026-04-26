use crate::crypto::digest::CryptoDigest;

#[derive(Clone, Debug)]
pub struct Index<H: CryptoDigest> {
    pub repo_diff: H,
}
