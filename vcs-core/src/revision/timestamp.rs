use std::time::SystemTime;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
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
