use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignedDigest;
use crate::revision::timestamp::Timestamp;

#[derive(Clone, Debug)]
pub enum AuthorSignature<D: CryptoDigest + CryptoHash> {
    Signature(SignedDigest<D>),
    // GitAuthor...
}

#[derive(Clone, Debug)]
pub struct Author<D: CryptoDigest + CryptoHash> {
    pub message: Box<str>,
    pub timestamp: Timestamp,
    pub signature: AuthorSignature<D>,
}

#[derive(Clone, Debug)]
pub struct Committer<D: CryptoDigest + CryptoHash> {
    pub message: Box<str>,
    pub timestamp: Timestamp,
    pub signature: SignedDigest<D>,
}
