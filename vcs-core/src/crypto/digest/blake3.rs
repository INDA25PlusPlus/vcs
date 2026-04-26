use crate::crypto::digest::{CryptoDigest, CryptoHasher};

impl CryptoDigest for blake3::Hash {
    type Hasher = blake3::Hasher;

    fn bytes(&self) -> &[u8] {
        self.as_slice()
    }

    fn zero() -> Self {
        blake3::Hash::from_bytes([0; blake3::OUT_LEN])
    }
}

impl CryptoHasher for blake3::Hasher {
    type Output = blake3::Hash;

    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }

    fn finish(&self) -> Self::Output {
        self.finalize()
    }
}
