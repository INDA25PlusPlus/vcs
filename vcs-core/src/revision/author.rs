use crate::crypto::digest::CryptoDigest;
use crate::crypto::signature::SignedHash;
use crate::revision::timestamp::Timestamp;

#[derive(Clone, Debug)]
pub enum AuthorSignature<H: CryptoDigest> {
    Signature(SignedHash<H>),
    // GitAuthor...
}

#[derive(Clone, Debug)]
pub struct Author<H: CryptoDigest> {
    pub message: String,
    pub timestamp: Timestamp,
    pub signature: AuthorSignature<H>,
}

#[derive(Clone, Debug)]
pub struct Committer<H: CryptoDigest> {
    pub message: String,
    pub timestamp: Timestamp,
    pub signature: SignedHash<H>,
}
