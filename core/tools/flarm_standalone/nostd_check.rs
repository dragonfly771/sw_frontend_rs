//! no_std build check for the flarm modules (type-check only, emits metadata).
//!
//! rustc --edition 2021 --crate-type rlib --crate-name micromath micromath_stub.rs
//! rustc --edition 2021 --crate-type lib --emit metadata --extern micromath=libmicromath.rlib -L . nostd_check.rs
#![no_std]
#![warn(missing_docs)]
#![deny(warnings)]

/// The FLARM core, exactly as it will live at `crate::flarm` in the firmware.
#[path = "../../src/flarm/mod.rs"]
pub mod flarm;
