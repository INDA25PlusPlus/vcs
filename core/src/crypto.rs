use serde::{Deserializer, Serializer};
use std::fmt::{Debug, Formatter};

pub trait CryptoHash {
    fn crypto_hash(&self) -> blake3::Hash;
}

pub struct SignedHash {
    public_key: aws_lc_rs::signature::ParsedPublicKey,
    timestamp: usize,
    signature: aws_lc_rs::signature::Signature,
}

impl SignedHash {
    pub fn sign(hash: &blake3::Hash, key_pair: aws_lc_rs::signature::Ed25519KeyPair) -> SignedHash {
        todo!()
    }

    pub fn verify(
        &self,
        unsigned_hash: &blake3::Hash,
    ) -> Result<(), aws_lc_rs::error::KeyRejected> {
        todo!()
    }
}

impl serde::Serialize for SignedHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!()
    }
}

impl<'de> serde::Deserialize<'de> for SignedHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}

impl Debug for SignedHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
