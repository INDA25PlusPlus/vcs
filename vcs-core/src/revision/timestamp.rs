use crypto_hash_derive::CryptoHash;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, CryptoHash, Serialize, Deserialize,
)]
pub struct Timestamp {
    unix_seconds: u64,
}

impl Timestamp {
    pub fn now() -> Timestamp {
        Timestamp {
            unix_seconds: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("current time should be after unix epoch")
                .as_secs(),
        }
    }

    // todo
}
