#![no_std]
#![doc = include_str!("../README.md")]

extern crate self as reportable;

/// Derive [`Reportable`] using an explicit annotation on each enum variant.
///
/// See the [crate documentation](crate) for supported annotations.
pub use reportable_derive::Reportable;

/// Who should receive an error's diagnostic details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportTo {
    /// An internal failure for operators to investigate.
    ///
    /// The caller can still receive a generic failure response.
    Internal,
    /// An expected failure for the caller to handle.
    Caller,
}

/// Classifies an error without logging it or choosing a transport or retry policy.
///
/// This trait does not require [`core::error::Error`]. It can be implemented
/// manually when classification depends on a value rather than a variant.
pub trait Reportable {
    /// Return the intended recipient of the error's diagnostic details.
    fn report_to(&self) -> ReportTo;
}

impl<T: Reportable + ?Sized> Reportable for &T {
    fn report_to(&self) -> ReportTo {
        T::report_to(self)
    }
}
