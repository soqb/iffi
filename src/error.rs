use core::any::type_name;

use thiserror_no_std::Error;

use crate::{BitPattern, BitRanges, Iffi, Nicheless};

/// Potential errors converting with [`Iffi`].
#[derive(Error, Debug)]
pub enum ErrorKind {
    /// A non-nullable pointer was found to be null.
    #[error("Expected a pointer to not be null")]
    NullPtr,
    /// Failed to find an enum variant that matches the discriminant.
    #[error("Invalid enum discriminant {0}")]
    InvalidEnumDiscriminant(BitPattern),
    /// A general error describing any invalid bit-pattern.
    #[cfg_attr(
        feature = "alloc",
        error("Invalid bit-pattern; {bits} not in the ranges {valid}")
    )]
    #[cfg_attr(not(feature = "alloc"), error("Invalid bit-pattern; {bits}"))]
    InvalidBitPattern { bits: BitPattern, valid: BitRanges },
    #[cfg(feature = "std")]
    /// Any other error.
    #[error("{0}")]
    Custom(alloc::boxed::Box<dyn std::error::Error>),
}

impl PartialEq for ErrorKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidEnumDiscriminant(l), Self::InvalidEnumDiscriminant(r)) => l == r,
            (
                Self::InvalidBitPattern {
                    bits: l_bits,
                    valid: l_valid,
                },
                Self::InvalidBitPattern {
                    bits: r_bits,
                    valid: r_valid,
                },
            ) => l_bits == r_bits && l_valid == r_valid,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

/// An error encountered while converting with [`Iffi`], wrapped with type name information.
#[derive(Error, Debug, PartialEq)]
#[error("Failed to convert {from} into {into}; {error}")]
pub struct Error {
    #[source]
    pub error: ErrorKind,
    pub from: &'static str,
    pub into: &'static str,
}

impl Error {
    /// Creats a new error.
    pub fn new<T: Iffi<U>, U: Nicheless>(error: ErrorKind) -> Self {
        Self {
            error,
            from: type_name::<U>(),
            into: type_name::<T>(),
        }
    }
}
