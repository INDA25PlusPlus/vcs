use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignedDigest;
use crate::revision::timestamp::Timestamp;
use crypto_hash_derive::CryptoHash;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, CryptoHash, Serialize, Deserialize)]
pub enum AuthorSignature<D: CryptoDigest + CryptoHash> {
    Signature(SignedDigest<D>),
    // GitAuthor...
}

#[derive(Clone, Debug, CryptoHash, Serialize, Deserialize)]
pub struct Author<D: CryptoDigest + CryptoHash> {
    pub message: Box<str>,
    pub timestamp: Timestamp,
    pub signature: AuthorSignature<D>,
}

#[derive(Clone, Debug, CryptoHash, Serialize, Deserialize)]
pub struct Committer<D: CryptoDigest + CryptoHash> {
    pub message: Box<str>,
    pub timestamp: Timestamp,
    pub signature: SignedDigest<D>,
}
