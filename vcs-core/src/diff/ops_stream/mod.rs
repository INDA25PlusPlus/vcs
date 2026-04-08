pub mod compact;
pub mod compose;
pub mod types;

pub use compact::Compact;
pub use compose::Compose;
pub use types::Op;

/// Extension methods for op streams.
pub trait OpStreamExt: Iterator<Item = Op> + Sized {
    /// Lazily composes two op streams.
    fn compose<O: Iterator<Item = Op>>(self, other: O) -> Compose<Self, O> {
        Compose::new(self, other)
    }

    /// Lazily compacts the stream
    fn compact(self) -> Compact<Self> {
        Compact::new(self)
    }
}

impl<I: Iterator<Item = Op>> OpStreamExt for I {}
