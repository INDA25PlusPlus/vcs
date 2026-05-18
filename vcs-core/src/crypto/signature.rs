use crate::crypto::digest::{CryptoDigest, CryptoHash, CryptoHasher};
use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::{ED25519_PUBLIC_KEY_LEN, Ed25519KeyPair, KeyPair};
use serde::{Deserializer, Serializer};
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

pub fn generate_signing_key() -> Result<Ed25519KeyPair, aws_lc_rs::error::Unspecified> {
    let random = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&random)?;
    Ok(Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())?)
}

// aws-lc-rs exposes the public key length, but keeps the Ed25519 signature length private.
const ED25519_SIGNATURE_LEN: usize = 64;

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

/// Signature of a hash of type `D`
#[derive(Clone)]
pub struct SignedDigest<D: CryptoDigest> {
    public_key: aws_lc_rs::signature::UnparsedPublicKey<Box<[u8]>>,
    signature: Box<[u8]>,
    _hash_type: PhantomData<D>,
}

impl<D: CryptoDigest> SignedDigest<D> {
    /// Create a signature of `hash` using a given key pair
    pub fn sign(hash: &D, key_pair: &Ed25519KeyPair) -> SignedDigest<D> {
        let signature = key_pair.sign(hash.bytes());
        SignedDigest {
            public_key: aws_lc_rs::signature::UnparsedPublicKey::new(
                &aws_lc_rs::signature::ED25519,
                key_pair.public_key().as_ref().into(),
            ),
            signature: signature.as_ref().into(),
            _hash_type: PhantomData,
        }
    }

    /// Verify that the signature matches `hash`
    pub fn verify(&self, hash: &D) -> Result<(), aws_lc_rs::error::Unspecified> {
        self.public_key
            .verify(hash.bytes(), self.signature.as_ref())
    }
}

impl<D: CryptoDigest> CryptoHash for SignedDigest<D> {
    fn crypto_hash<OutD: CryptoDigest, H: CryptoHasher<Output = OutD>>(&self, state: &mut H) {
        self.public_key.as_ref().crypto_hash(state);
        self.signature.as_ref().crypto_hash(state);
    }
}

impl<D: CryptoDigest> serde::Serialize for SignedDigest<D> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serde::Serialize::serialize(
            &(self.public_key.as_ref(), self.signature.as_ref()),
            serializer,
        )
    }
}

impl<'de, D: CryptoDigest> serde::Deserialize<'de> for SignedDigest<D> {
    fn deserialize<De>(deserializer: De) -> Result<Self, De::Error>
    where
        De: Deserializer<'de>,
    {
        let (public_key, signature) =
            <(Box<[u8]>, Box<[u8]>) as serde::Deserialize>::deserialize(deserializer)?;
        if public_key.len() != ED25519_PUBLIC_KEY_LEN {
            let expected = format!("a {ED25519_PUBLIC_KEY_LEN}-byte Ed25519 public key");
            return Err(serde::de::Error::invalid_length(
                public_key.len(),
                &expected.as_str(),
            ));
        }
        if signature.len() != ED25519_SIGNATURE_LEN {
            let expected = format!("a {ED25519_SIGNATURE_LEN}-byte Ed25519 signature");
            return Err(serde::de::Error::invalid_length(
                signature.len(),
                &expected.as_str(),
            ));
        }
        Ok(SignedDigest {
            public_key: aws_lc_rs::signature::UnparsedPublicKey::new(
                &aws_lc_rs::signature::ED25519,
                public_key,
            ),
            signature,
            _hash_type: PhantomData,
        })
    }
}

impl<D: CryptoDigest> Debug for SignedDigest<D> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedDigest")
            .field("public_key", &self.public_key)
            .field("signature", &self.signature.as_ref())
            .finish()
    }
}
