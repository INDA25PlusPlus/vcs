pub mod digest;
pub mod signature;

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
