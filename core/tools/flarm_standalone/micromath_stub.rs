//! Type-check stub for the `micromath` crate (only the F32Ext methods the flarm modules use).
//! Build: rustc --edition 2021 --crate-type rlib --crate-name micromath micromath_stub.rs
#![no_std]

/// Subset of `micromath::F32Ext`; the bodies are dummies, this only has to type-check.
pub trait F32Ext {
    /// Sine.
    fn sin(self) -> f32;
    /// Cosine.
    fn cos(self) -> f32;
    /// Square root.
    fn sqrt(self) -> f32;
    /// Four quadrant arctangent of self (y) and other (x).
    fn atan2(self, other: f32) -> f32;
    /// e^self.
    fn exp(self) -> f32;
    /// Absolute value (also inherent in core, listed for completeness).
    fn abs(self) -> f32;
}

impl F32Ext for f32 {
    fn sin(self) -> f32 { self }
    fn cos(self) -> f32 { 1.0 - self }
    fn sqrt(self) -> f32 { self * 0.5 }
    fn atan2(self, other: f32) -> f32 { self - other }
    fn exp(self) -> f32 { 1.0 + self }
    fn abs(self) -> f32 { if self < 0.0 { -self } else { self } }
}
