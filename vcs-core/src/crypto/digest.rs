use std::io;
use std::io::Read;

/// Result of applying a cryptographically secure hashing algorithm to an object
pub trait CryptoDigest {
    fn bytes(&self) -> &[u8];
}

/// Type that can be hashed using a cryptographically secure hashing algorithm
pub trait CryptoHash {
    fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H);

    fn crypto_hash_slice<D: CryptoDigest, H: CryptoHasher<Output = D>>(data: &[Self], state: &mut H)
    where
        Self: Sized,
    {
        state.write_length_prefix(data.len());
        data.iter().for_each(|item| item.crypto_hash(state));
    }
}

/// Type implementing a cryptographically secure hashing algorithm
pub trait CryptoHasher {
    type Output: CryptoDigest;

    fn write(&mut self, bytes: &[u8]);

    fn finish(&self) -> Self::Output;

    fn write_reader<R: Read>(&mut self, reader: &mut R) -> io::Result<()> {
        let mut bytes = Vec::new();
        let _ = reader.read_to_end(&mut bytes)?;
        self.write(&bytes);
        Ok(())
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write(&[i]);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write(&i.to_le_bytes());
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write(&i.to_le_bytes());
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.write(&i.to_le_bytes());
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.write(&i.to_le_bytes());
    }

    #[inline]
    fn write_i8(&mut self, i: i8) {
        self.write_u8(i as u8);
    }

    #[inline]
    fn write_i16(&mut self, i: i16) {
        self.write_u16(i as u16);
    }

    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.write_u32(i as u32);
    }

    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_i128(&mut self, i: i128) {
        self.write_u128(i as u128);
    }

    #[inline]
    fn write_length_prefix(&mut self, size: usize) {
        self.write_u64(size as u64);
    }

    #[inline]
    fn write_str(&mut self, s: &str) {
        self.write(s.as_bytes());
        self.write_u8(0x00);
    }
}

/// Modified version of impls in [`core::hash::impls`]
mod impls {
    use super::*;
    use std::convert::Infallible;

    macro_rules! impl_write {
        ($(($ty:ty, $method:ident),)*) => {
            $(
                impl CryptoHash for $ty {
                    #[inline]
                    fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(
                        &self,
                        state: &mut H
                    ) {
                        state.$method(*self);
                    }

                    #[cfg(target_endian = "little")]
                    #[inline]
                    fn crypto_hash_slice<D: CryptoDigest, H: CryptoHasher<Output = D>>(
                        data: &[Self],
                        state: &mut H)
                    {
                        state.write(bytemuck::must_cast_slice(data));
                    }

                    #[cfg(target_endian = "big")]
                    #[inline]
                    fn crypto_hash_slice<D: CryptoDigest, H: CryptoHasher<Output = D>>(
                        data: &[Self],
                        state: &mut H
                    ) {
                        // Consistent endianness is required for deterministic hashing. We choose
                        // little-endian because it is native on most platforms and thus most
                        // performant in most cases. Unfortunately this means that allocating a
                        // temporary buffer is required for conversion on big-endian platforms.
                        let bytes: Vec<_> = data.iter().flat_map(|i| i.to_le_bytes()).collect();
                        state.write(bytemuck::must_cast_slice(&bytes));
                    }
                }
            )*
        };
    }

    impl_write! {
        (u8, write_u8),
        (u16, write_u16),
        (u32, write_u32),
        (u64, write_u64),
        (u128, write_u128),
        (i8, write_i8),
        (i16, write_i16),
        (i32, write_i32),
        (i64, write_i64),
        (i128, write_i128),
        // not implemented for usize/isize because their sizes are platform-dependent
    }

    impl CryptoHash for bool {
        #[inline]
        fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H) {
            state.write_u8(*self as u8)
        }
    }

    impl CryptoHash for char {
        #[inline]
        fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H) {
            state.write_u32(*self as u32)
        }
    }

    impl CryptoHash for str {
        #[inline]
        fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H) {
            state.write_str(self);
        }
    }

    impl CryptoHash for Infallible {
        #[inline]
        fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, _state: &mut H) {}
    }

    impl CryptoHash for () {
        #[inline]
        fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, _state: &mut H) {}
    }

    macro_rules! impl_tuples {
        ($(($($ty:ident),+),)*) => {
            $(
                impl<$($ty: CryptoHash),+> CryptoHash for ($($ty,)+) {
                    #[allow(non_snake_case)]
                    #[inline]
                    fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H) {
                        let ($(ref $ty,)+) = *self;
                        $($ty.crypto_hash(state);)+
                    }
                }
            )*
        };
    }

    impl_tuples! {
        (T1),
        (T1, T2),
        (T1, T2, T3),
        (T1, T2, T3, T4),
        (T1, T2, T3, T4, T5),
        (T1, T2, T3, T4, T5, T6),
        (T1, T2, T3, T4, T5, T6, T7),
        (T1, T2, T3, T4, T5, T6, T7, T8),
        (T1, T2, T3, T4, T5, T6, T7, T8, T9),
        (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10),
        (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11),
        (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12),
    }

    impl<T: CryptoHash> CryptoHash for [T] {
        #[inline]
        fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H) {
            CryptoHash::crypto_hash_slice(self, state);
        }
    }

    impl<T: CryptoHash> CryptoHash for &T {
        #[inline]
        fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H) {
            CryptoHash::crypto_hash(*self, state);
        }
    }

    impl<T: CryptoHash> CryptoHash for &mut T {
        #[inline]
        fn crypto_hash<D: CryptoDigest, H: CryptoHasher<Output = D>>(&self, state: &mut H) {
            CryptoHash::crypto_hash(*self, state);
        }
    }
}
