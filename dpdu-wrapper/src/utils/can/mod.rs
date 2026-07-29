mod frame;
mod id;
mod raw;

pub use frame::*;
pub use id::*;
pub use raw::*;

/// Frame Create Errors
#[derive(Debug)]
pub enum FrameCreateError {
    /// Data in header does not match supplied.
    NotEnoughData,

    /// Invalid data length not 0-8 for Classic packet or valid for FD.
    InvalidDataLength,

    /// Invalid ID.
    InvalidCanId,
}
