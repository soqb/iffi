#![no_std]
#![allow(clippy::drop_non_drop)]

//! Safe conversion between FFI-safe and ergonomic Rust types.
//!
//! An implementation of [`Iffi`] trait (which can be derived) on a type `T`
//! over a *[universe]* `U` (the type parameter on the trait)
//! provides assertions that a value of type `U` can be safely [transmuted] into a value of type `T`.
//!
//! [`iffi::try_from`][try_from] and [`iffi::into`][into] perform the conversion safely.
//!
//! [universe]: #universe
//! [transmuted]: core::mem::transmute
//!
//! # Glossary & Concepts
//!
//! ### FFI-safe
//! a type that can be used with [FFI].
//!
//! This can be a primitve, a type marked with `#[repr(C)]` or `#[repr(Int)]`,
//! or a type marked with `#[repr(transparent)]` with only one non-zero sized field which must be FFI-safe.
//!
//! [FFI]: https://doc.rust-lang.org/nomicon/FFI.html
//!
//! ### bit-pattern
//! a sequence of bits representing a potential value of a type.
//!
//! Each type defines some bit-patterns (with the same number of bits as the type)
//! that are valid and others that are invalid.
//!
//! For example, for `u16`, all bit-patterns are valid because `0x0000` through `0xffff`
//! are all correctly initialized `u16` values,
//! but for `bool` only the patterns `0x00` and `0x01`, (`false` and `true` respectively) are valid.
//! `0x02..=0xff` are all *invalid* bit-patterns for `bool`.
//!
//! An invalid bit-pattern is also called a niche.
//! Types like `u16` where all bit-patterns are valid are *nicheless*.
//!
//! This definition is subtly different to the one in the Unsafe Code Guidelines.
//! In the context of bit-patterns this crate assumes all bits are initialized,
//! so there is a one-to-one relationship between invalid bit patterns and niches.
//!
//! ### well-defined
//! a potential value represented in memory by a valid bit-pattern.
//!
//! A null reference or zeroed `NonZero*` is not well-defined, for example.
//!
//! ### nicheless
//! a type with no niches i.e. no invalid bit-patterns.
//!
//! The [`Nicheless`] trait cannot be automatically implemented
//! and must be derived or implemented manually.
//!
//! ZSTs are always nicheless. Uninhabited types are never nicheless.
//!
//! [A more technical definition] is availible in the Unsafe Code Guidelines.
//!
//! [A more technical definition]: https://rust-lang.github.io/unsafe-code-guidelines/glossary.html#niche
//!
//! ### layout
//! the combination of the size, alignment and ABI behaviour of a type.
//!
//! ### universe
//! a nicheless type with the same layout as some potentially non-nicheless type.
//!
//! [`MaybeInvalid<T>`] is a universe of all `T`.
//!
//! A type may have many universes.
//! As an example, `NonZeroU8` has the universes `MaybeInvalid<NonZeroU8>`
//! and `u8`.

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use core::{
    any::type_name,
    mem::{size_of, size_of_val},
};

pub use macros::{Iffi, Nicheless};

mod niche;
pub use niche::*;

mod error;
pub use error::*;

mod impls;

#[cfg_attr(feature = "alloc", path = "alloc_bits.rs")]
#[cfg_attr(not(feature = "alloc"), path = "nostd_bits.rs")]
mod bits;
pub use bits::{BitPattern, BitRanges};

mod maybe_invalid;
pub use maybe_invalid::*;

/// The core trait of the `iffi` crate.
///
/// This is typically implemented by deriving [`Iffi`],
/// which can be done for all FFI-safe structs and enums.
///
/// The set of possible values of `Self` that are well-defined
/// is a subset of the well-defined values of the `U` type parameter.
///
/// The `U` type parameter, called a *[universe]*, must be [`Nicheless`].
///
/// # Safety
/// The implementation must guarantee that `Self` and `U`
/// have *identical* layouts - the same size, alignment and ABI.
///
/// [`can_transmute`] must return not return `Ok(())`
/// unless `U` can safely be transmuted into `Self`.
///
/// [universe]: crate#universe
/// [`can_transmute`]: [`Iffi::can_transmute`]
pub unsafe trait Iffi<U: Nicheless = MaybeInvalid<Self>> {
    /// Checks for the safety of transmuting `U` into `Self`.
    /// Returns `Ok(())` if the value is safe, and [`Err(iffi::Error)`] otherwise.
    ///
    /// [`Err(iffi::Error)`]: [`Error`]
    fn can_transmute(superset: &U) -> Result<(), Error>;
}

/// Tries to convert an FFI-safe [nicheless] type to a more ergonomic one.
///
/// [nicheless]: Nicheless
pub fn try_from<T: Iffi<U>, U: Nicheless + core::fmt::Debug>(value: U) -> Result<T, Error> {
    T::can_transmute(&value)?;
    debug_assert_eq!(
        size_of_val(&value),
        size_of::<T>(),
        "tried converting from {} to {} but they are different sizes!",
        type_name::<U>(),
        type_name::<T>(),
    );
    // SAFETY: the superset and the subset are the same size and value is safe to transmute.
    unsafe { Ok(transmute::transmute(value)) }
}

/// Converts an ergonomic Rust type to an FFI-safe [nicheless] type.
///
/// [nicheless]: Nicheless
pub fn into<T: Iffi<U>, U: Nicheless>(safe: T) -> U {
    debug_assert_eq!(
        size_of::<U>(),
        size_of_val(&safe),
        "tried converting {} into {} but they are different sizes!",
        type_name::<T>(),
        type_name::<U>(),
    );
    // SAFETY: the Iffi trait guarantees that T::Univserse is a superset of T
    unsafe { transmute::transmute(safe) }
}

#[cfg(test)]
mod tests {
    use core::{
        marker::PhantomData,
        num::{NonZeroU32, NonZeroU8},
    };

    use crate::{self as iffi, *};
    use iffi::Iffi;

    macro_rules! roundtrip {
        ($expr:expr) => {
            assert_eq!(Ok($expr), try_from(into($expr)));
        };
    }

    macro_rules! assert_fails {
        ($ty:ty = $expr:expr => $error:expr) => {
            let attempt: Result<$ty, _> = try_from($expr);
            match attempt {
                Err(Error { error, .. }) => assert_eq!(error, $error),
                _ => panic!("expected error"),
            }
        };
    }

    #[test]
    fn conversions_fails() {
        assert_fails!(NonZeroU8 = 0 => ErrorKind::InvalidBitPattern {
            bits: BitPattern::from_le(&0u8),
            valid: BitRanges::from_le(&[1u8..=0xff])
        });

        assert_fails!(NonZeroU32 = 0 => ErrorKind::InvalidBitPattern {
            bits: BitPattern::from_le(&[0u8; 4]),
            valid: BitRanges::from_le(&[1u32..=0xffffffff])
        });
    }

    #[test]
    fn derive_iffi() {
        #[derive(Iffi)]
        #[repr(C)]
        struct A {
            a: u8,
            b: NonZeroU8,
            #[iffi(with = "u8")]
            c: u8,
        }

        #[derive(Iffi, Clone, Copy, PartialEq, Debug)]
        #[repr(isize, align(1024))]
        enum TA {
            A,
            B,
            D(u32) = 150,
            E { a: u16, b: NonZeroU8 },
        }

        roundtrip!(TA::A);
        roundtrip!(TA::B);
        roundtrip!(TA::D(5));
        roundtrip!(TA::E {
            a: 100,
            b: NonZeroU8::new(3).unwrap(),
        });
    }

    #[test]
    fn derive_generics() {
        #[derive(Iffi)]
        #[repr(C)]
        struct A<T: Iffi, U> {
            b: T,
            a: PhantomData<U>,
        }
    }

    #[test]
    fn nested() {
        #[derive(Iffi, PartialEq, Debug)]
        #[repr(C)]
        struct A;

        #[derive(Iffi, PartialEq, Debug)]
        #[repr(C)]
        struct B(A);

        #[derive(Iffi, PartialEq, Debug)]
        #[repr(C)]
        struct C {
            b: B,
        }

        #[derive(Iffi, PartialEq, Debug)]
        #[repr(u8)]
        enum D {
            A(A),
            B(B),
            C(C),
            D { c: C },
            E,
        }

        roundtrip!(A);
        roundtrip!(B(A));
        roundtrip!(C { b: B(A) });
        roundtrip!(D::A(A));
        roundtrip!(D::B(B(A)));
        roundtrip!(D::C(C { b: B(A) }));
        roundtrip!(D::D { c: C { b: B(A) } });
        roundtrip!(D::E);

        #[derive(Iffi, PartialEq, Debug, Clone, Copy)]
        #[repr(C)]
        struct Deep1(NonZeroU8);
        #[derive(Iffi, PartialEq, Debug, Clone, Copy)]
        #[repr(C)]
        struct Deep2(Deep1);
        #[derive(Iffi, PartialEq, Debug, Clone, Copy)]
        #[repr(C)]
        struct Deep3(Deep2);
        #[derive(Iffi, PartialEq, Debug, Clone, Copy)]
        #[repr(C)]
        struct Deep4(Deep3);
        #[derive(Iffi, PartialEq, Debug, Clone, Copy)]
        #[repr(C)]
        struct Deep5(Deep4);
        #[derive(Iffi, PartialEq, Debug, Clone, Copy)]
        #[repr(C)]
        struct Deep6(Deep5);
        #[derive(Iffi, PartialEq, Debug, Clone, Copy)]
        #[repr(C)]
        struct Deep7(Deep6);
        #[derive(Iffi, PartialEq, Debug, Clone, Copy)]
        #[repr(C)]
        struct Deep8(Deep7);

        roundtrip!(Deep8(Deep7(Deep6(Deep5(Deep4(Deep3(Deep2(Deep1(
            NonZeroU8::new(5).unwrap(),
        )))))))));

        let invalid: MaybeInvalid<Deep8> = MaybeInvalid::zeroed();
        let from: Result<Deep8, _> = try_from(invalid);
        assert_eq!(
            from,
            Err(Error {
                error: ErrorKind::InvalidBitPattern {
                    bits: BitPattern::from_le(&0u8),
                    valid: BitRanges::from_le(&[1u8..=0xff])
                },
                from: type_name::<MaybeInvalid<NonZeroU8>>(),
                into: type_name::<NonZeroU8>()
            })
        )
    }
}
