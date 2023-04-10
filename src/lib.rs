#![no_std]
#![allow(clippy::drop_non_drop)]
#![doc = include_str!("../README.md")]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use core::{
    any::type_name,
    mem::{size_of, size_of_val},
};

pub use iffi_macros::{Iffi, Nicheless};

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
