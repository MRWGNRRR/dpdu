#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_imports,
    unused_import_braces,
    unused_qualifications
)]

//! A core implementation of the D-PDU API (ISO 22900-2).
//!
//! This crate just provides the type definitions in order to use the API with Rust code.
//!
//! For a crate that actually uses this API, you can check the [ecu_diagnostics crate](https://docs.rs/ecu_diagnostics/latest/ecu_diagnostics/)
//!
//! NOTE: To match the rust naming convention, enums and structure names have been slightly renamed.
mod enums;
mod functions;
mod structures;

/// Utilities for working with bit flags.
#[cfg(feature = "bitflags")]
pub mod bitflags;

/// Utilities for debugging and inspecting FFI data structures.
///
/// This module provides helper types and implementations for safely
/// formatting raw pointers, C-compatible structures, and other data
/// used during FFI interactions.
#[cfg(feature = "debug")]
pub mod debug;

pub use enums::*;
pub use functions::*;
use std::ffi::c_void;
pub use structures::*;

/// Undefined ID value
pub const PDU_ID_UNDEF: u32 = 0xFFFFFFFE;

/// Undefined handle value
pub const PDU_HANDLE_UNDEF: u32 = 0xFFFFFFFF;

/// PDU Event callback function type
pub type EventCallbackFn = unsafe extern "system" fn(
    event_type: PduEvtData,
    h_mod: u32,
    h_cll: u32,
    p_cll_tag: *mut c_void,
    p_api_tag: *mut c_void,
);
