#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{mem::{size_of_val, size_of}, any::type_name, num::{NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128}};

use thiserror_no_std::Error;

pub use iffi_macro::iffi;

/// The core trait of the `iffi` crate.
///
/// The set of possible values of `T::Subset` is a subset of the possible values of `T`.
///
///
/// # Safety
/// The implementation must guarantee that `T` and `T::Subset`
/// have *identical* layouts. This is usually done by deriving [`Iffi`].
/// [`can_transmute`] must return not return `Ok(())`
/// unless `T` can safely be transmuted into `T::Subset`.
///
/// [`can_transmute`]: [`Iffi::can_transmute`]
pub unsafe trait Iffi {
    /// The unsafe version of this type.
    type Subset;

    /// Checks for the safety of transmuting `T` into `T::Subset`.
    /// Returns `Ok(())` if the value is safe, and [`Err(iffi::Error)`] otherwise.
    ///
    /// [`Err(iffi::Error)`]: [`Error`]
    fn can_transmute(&self) -> Result<(), Error>;
}

/// Potential errors converting into safe types.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    /// Failed to convert an integer to an enum.
    #[error("Invalid enum variant {0}")]
    InvalidEnumVariant(u128),
    /// A non-nullable pointer was found to be null.
    #[error("Null pointer found")]
    FoundNullPtr,
    /// An optional value was not present.
    #[error("A value contained none.")]
    FoundNone,
}

macro_rules! impl_copy {
    ($($ty:ty),*$(,)?) => {
        $(
            // SAFETY: safe and unsafe types *are* the same type
            unsafe impl Iffi for $ty {
                type Subset = $ty;

                fn can_transmute(&self) -> Result<(), Error> {
                    Ok(())
                }
            }
        )*
    };
}

impl_copy!(
    (),
    u8,
    u16,
    u32,
    u64,
    u128,
    i8,
    i16,
    i32,
    i64,
    i128,
    f32,
    f64,
    NonZeroU8,
    NonZeroU16,
    NonZeroU32,
    NonZeroU64,
    NonZeroU128,
    NonZeroI8,
    NonZeroI16,
    NonZeroI32,
    NonZeroI64,
    NonZeroI128,
);

pub mod niche;

pub fn try_from<T: Iffi>(notsafe: T) -> Result<T::Subset, Error>
where
    T::Subset: Sized,
{
    notsafe.can_transmute()?;
    debug_assert_eq!(
        size_of_val(&notsafe),
        size_of::<T::Subset>(),
        "tried converting {} from {} but they are different sizes!",
        type_name::<T::Subset>(),
        type_name::<T>(),
    );
    // SAFETY: `T` and `T::Safe` are the same size and `notsafe` is safe to transmute.
    unsafe { Ok(transmute::transmute(notsafe)) }
}

pub fn into<T: Iffi>(safe: T::Subset) -> T
where
    T: Sized,
{
    debug_assert_eq!(
        size_of::<T>(),
        size_of_val(&safe),
        "tried converting {} into {} but they are different sizes!",
        type_name::<T::Subset>(),
        type_name::<T>(),
    );
    // SAFETY: the `Iffi` trait guarantees that `T::Safe` is a subset of `T`
    unsafe { transmute::transmute(safe) }
}

#[cfg(test)]
mod tests {
    use core::num::{NonZeroI128, NonZeroU128};

    use crate as iffi;
    use iffi::{iffi, into, try_from, Error};

    #[test]
    fn test() {
        #[repr(C)]
        #[iffi(UnsafeType)]
        pub struct SafeType {
            tuple: u8,
        }

        #[repr(C)]
        #[iffi(UnsafeVariants)]
        #[derive(Debug, PartialEq)]
        pub enum SafeVariants {
            A,
            B = 0x100,
            C,
        }

        assert_eq!(try_from(UnsafeVariants(0)), Ok(SafeVariants::A));
        assert_eq!(try_from(UnsafeVariants(0x100)), Ok(SafeVariants::B));
        assert_eq!(try_from(UnsafeVariants(0x101)), Ok(SafeVariants::C));
        assert_eq!(
            try_from(UnsafeVariants(0x102)),
            Err(Error::InvalidEnumVariant(0x102))
        );
    }

    #[test]
    fn niche() {
        #[repr(C)]
        #[iffi(MySameNiche)]
        struct MyNiche {
            non_zero: NonZeroI128,
            zeroable: Option<NonZeroU128>,
        }
    }

    #[test]
    fn roundtrip() {
        #[repr(C)]
        #[iffi(IntUnsafe)]
        #[derive(Debug, PartialEq)]
        struct Int(u8);

        assert_eq!(Int(0), try_from(into::<IntUnsafe>(Int(0))).unwrap())
    }
}
