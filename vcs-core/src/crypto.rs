pub mod signature;

use aws_lc_rs::signature::KeyPair;
use serde::{Deserializer, Serializer};
use std::fmt::Debug;

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

macro_rules! crypto_hash {
    ($hash_type:ty; $($field:expr),*) => {
        todo!()
    };
}
pub(crate) use crypto_hash;

#[cfg(test)]
mod tests {
    // todo: unit tests
}
