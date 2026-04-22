use crate::crypto::digest::CryptoDigest;
use aws_lc_rs::signature::KeyPair;
use serde::{Deserializer, Serializer};
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

#[derive(Copy, Clone)]
pub struct SignContext<'key> {
    key_pair: &'key aws_lc_rs::signature::Ed25519KeyPair,
}

impl<'key> SignContext<'key> {
    pub fn sign<H: CryptoDigest>(&self, hash: &H) -> SignedHash<H> {
        SignedHash::sign(hash, self.key_pair)
    }
}

/// Signature of a hash of type `H`
#[derive(Clone)]
pub struct SignedHash<H: CryptoDigest> {
    public_key: aws_lc_rs::signature::UnparsedPublicKey<Box<[u8]>>,
    signature: aws_lc_rs::signature::Signature,
    _hash_type: PhantomData<H>,
}

impl<H: CryptoDigest> SignedHash<H> {
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

impl<H: CryptoDigest> serde::Serialize for SignedHash<H> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!()
    }
}

impl<'de, H: CryptoDigest> serde::Deserialize<'de> for SignedHash<H> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}

impl<H: CryptoDigest> Debug for SignedHash<H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
