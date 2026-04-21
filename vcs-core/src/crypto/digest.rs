use std::io;
use std::io::Read;

/// Result of applying a cryptographically secure hashing algorithm to an object
pub trait CryptoDigest {
    fn bytes(&self) -> &[u8];
}

/// Type that can be hashed using a cryptographically secure hashing algorithm
pub trait CryptoHash {
    fn update_hasher<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H);
}

/// Type implementing a cryptographically secure hashing algorithm
pub trait CryptoHasher {
    type Output: CryptoDigest;

    fn update(&mut self, bytes: &[u8]);

    fn update_reader<R: Read>(&mut self, reader: &mut R) -> io::Result<()> {
        let mut bytes = Vec::new();
        let _ = reader.read_to_end(&mut bytes)?;
        self.update(&bytes);
        Ok(())
    }

    fn finalize(&self) -> Self::Output;
}
