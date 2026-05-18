use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignedDigest;
use crate::revision::timestamp::Timestamp;
use crypto_hash_derive::CryptoHash;

#[derive(Clone, Debug, CryptoHash)]
pub enum AuthorSignature<D: CryptoDigest + CryptoHash> {
    Signature(SignedDigest<D>),
    // GitAuthor...
}

#[derive(Clone, Debug, CryptoHash)]
pub struct Author<D: CryptoDigest + CryptoHash> {
    pub message: Box<str>,
    pub timestamp: Timestamp,
    pub signature: AuthorSignature<D>,
}

#[derive(Clone, Debug, CryptoHash)]
pub struct Committer<D: CryptoDigest + CryptoHash> {
    pub message: Box<str>,
    pub timestamp: Timestamp,
    pub signature: SignedDigest<D>,
}
