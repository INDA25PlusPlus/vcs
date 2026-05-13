use crate::crypto::digest::{CryptoDigest, CryptoHash, CryptoHasher};
use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use serde::{Deserializer, Serializer};
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

#[derive(Copy, Clone)]
pub struct SignContext<'key> {
    key_pair: &'key Ed25519KeyPair,
}

impl<'key> SignContext<'key> {
    pub fn new(key_pair: &'key Ed25519KeyPair) -> SignContext<'key> {
        SignContext { key_pair }
    }

    pub fn sign<D: CryptoDigest>(&self, hash: &D) -> SignedDigest<D> {
        SignedDigest::sign(hash, self.key_pair)
    }
}

impl<'key> From<&'key Ed25519KeyPair> for SignContext<'key> {
    fn from(value: &'key Ed25519KeyPair) -> Self {
        SignContext::new(value)
    }
}

/// Signature of a hash of type `H`
#[derive(Clone)]
pub struct SignedDigest<D: CryptoDigest> {
    public_key: aws_lc_rs::signature::UnparsedPublicKey<Box<[u8]>>,
    signature: aws_lc_rs::signature::Signature,
    _hash_type: PhantomData<D>,
}

impl<D: CryptoDigest> SignedDigest<D> {
    /// Create a signature of `item` using a given key pair
    pub fn sign(hash: &D, key_pair: &Ed25519KeyPair) -> SignedDigest<D> {
        SignedDigest {
            public_key: aws_lc_rs::signature::UnparsedPublicKey::new(
                &aws_lc_rs::signature::ED25519,
                key_pair.public_key().as_ref().into(),
            ),
            signature: key_pair.sign(hash.bytes()),
            _hash_type: PhantomData,
        }
    }

    /// Verify that the signature matches `item`
    pub fn verify(&self, hash: &D) -> Result<(), aws_lc_rs::error::Unspecified> {
        self.public_key
            .verify(hash.bytes(), self.signature.as_ref())
    }
}

impl<D: CryptoDigest> CryptoHash for SignedDigest<D> {
    fn crypto_hash<OutD: CryptoDigest, H: CryptoHasher<Output = OutD>>(&self, state: &mut H) {
        todo!()
    }
}

impl<D: CryptoDigest> serde::Serialize for SignedDigest<D> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!()
    }
}

impl<'de, D: CryptoDigest> serde::Deserialize<'de> for SignedDigest<D> {
    fn deserialize<De>(deserializer: De) -> Result<Self, De::Error>
    where
        De: Deserializer<'de>,
    {
        todo!()
    }
}

impl<D: CryptoDigest> Debug for SignedDigest<D> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
