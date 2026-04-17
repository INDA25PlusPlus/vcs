use crate::crypto::{CryptoHash, SignedHash};
use crate::revision::timestamp::Timestamp;

#[derive(Clone, Debug)]
pub enum AuthorSignature<H: CryptoHash> {
    Signature(SignedHash<H>),
    // GitAuthor...
}

#[derive(Clone, Debug)]
pub struct Author<H: CryptoHash> {
    pub message: String,
    pub timestamp: Timestamp,
    pub signature: AuthorSignature<H>,
}

#[derive(Clone, Debug)]
pub struct Committer<H: CryptoHash> {
    pub message: String,
    pub timestamp: Timestamp,
    pub signature: SignedHash<H>,
}
