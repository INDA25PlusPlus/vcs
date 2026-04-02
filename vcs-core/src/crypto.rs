use aws_lc_rs::signature::KeyPair;
use serde::{Deserializer, Serializer};
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

/// Type resulting from applying a cryptographically secure hashing algorithm to an object
pub trait CryptoHash: Clone {
    fn bytes(&self) -> &[u8];

    fn from_bytes(bytes: &[u8]) -> Self;

    // todo
}

/// Type that can be hashed using a cryptographically secure hashing algorithm
pub trait CryptoHashable {
    fn crypto_hash<H: CryptoHash>(&self) -> H;
}

#[macro_export]
macro_rules! crypto_hash {
    ($hash_type:ty; $($field:expr),*) => {
        todo!()
    };
}

#[derive(Copy, Clone)]
pub struct SignContext<'a> {
    key_pair: &'a aws_lc_rs::signature::Ed25519KeyPair,
}

impl<'a> SignContext<'a> {
    pub fn sign<H: CryptoHash>(&self, hash: &H) -> SignedHash<H> {
        SignedHash::sign(hash, self.key_pair)
    }
}

/// Signature of a hash of type `H`
#[derive(Clone)]
pub struct SignedHash<H: CryptoHash> {
    public_key: aws_lc_rs::signature::UnparsedPublicKey<Box<[u8]>>,
    signature: aws_lc_rs::signature::Signature,
    _hash_type: PhantomData<H>,
}

impl<H: CryptoHash> SignedHash<H> {
    /// Create a signature of `item` using a given key pair
    pub fn sign(hash: &H, key_pair: &aws_lc_rs::signature::Ed25519KeyPair) -> SignedHash<H> {
        SignedHash {
            public_key: aws_lc_rs::signature::UnparsedPublicKey::new(
                &aws_lc_rs::signature::ED25519,
                key_pair.public_key().as_ref().into(),
            ),
            signature: key_pair.sign(hash.bytes()),
            _hash_type: PhantomData,
        }
    }

    /// Verify that the signature matches `item`
    pub fn verify(&self, hash: &H) -> Result<(), aws_lc_rs::error::Unspecified> {
        self.public_key
            .verify(hash.bytes(), self.signature.as_ref())
    }
}

impl<H: CryptoHash> serde::Serialize for SignedHash<H> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!()
    }
}

impl<'de, H: CryptoHash> serde::Deserialize<'de> for SignedHash<H> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}

impl<H: CryptoHash> Debug for SignedHash<H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
