mod id;
mod frame;
mod raw;

pub use id::*;
pub use frame::*;
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