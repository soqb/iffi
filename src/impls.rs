use core::num::*;

extern crate std;

use concat_arrays::concat_arrays;

use crate::{BitPattern, BitRanges, Error, ErrorKind, Iffi, MaybeInvalid, Nicheless};

// SAFETY: universe and type are the same.
unsafe impl<U: Nicheless> Iffi<U> for U {
    fn can_transmute(_: &U) -> Result<(), Error> {
        Ok(())
    }
}

// SAFETY: MaybeInvalid<T> is always valid
// because T is nicheless. The types have the same layout.
unsafe impl<U: Nicheless> Iffi<MaybeInvalid<U>> for U {
    fn can_transmute(_: &MaybeInvalid<U>) -> Result<(), Error> {
        Ok(())
    }
}

macro_rules! impl_nonzero_map {
    ($($ty:ty: $ty2:ty),+; $bytes:ident @ |$arg:pat_param| $fn:expr) => {
        $(
            const _: () = {
                #[inline]
                fn from_universe<U: Nicheless>(universe: &$ty2) -> Result<(), Error>
                where $ty: Iffi<U> {
                    const $bytes: usize = core::mem::size_of::<$ty2>();
                    let cb = |$arg: &$ty2| $fn;
                    cb(universe).map_err(Error::new::<$ty, U>)
                }

                unsafe impl Iffi<$ty2> for $ty {
                    fn can_transmute(superset: &$ty2) -> Result<(), Error> {
                        from_universe::<$ty2>(superset)
                    }
                }

                unsafe impl Iffi<MaybeInvalid<$ty>> for $ty {
                    fn can_transmute(superset: &MaybeInvalid<$ty>) -> Result<(), Error> {
                        let ptr = superset.as_ptr() as *const $ty2;
                        // SAFETY: NonZeroXXX and XXX have the same ABI
                        let ty2 = unsafe { &*ptr };
                        from_universe::<MaybeInvalid<$ty>>(ty2)
                    }
                }
            };
        )+
    };
}

impl_nonzero_map! {
    // SAFETY: non-zero types are the same size as their zeroable variants.
    // the only invalid variant for NonZero* is when they are zero.
    NonZeroU8: u8,
    NonZeroU16: u16,
    NonZeroU32: u32,
    NonZeroU64: u64,
    NonZeroU128: u128,
    NonZeroUsize: usize,
    NonZeroI8: i8,
    NonZeroI16: i16,
    NonZeroI32: i32,
    NonZeroI64: i64,
    NonZeroI128: i128,
    NonZeroIsize: isize;
    SIZE @ |num| (*num != 0).then_some(()).ok_or_else(|| ErrorKind::InvalidBitPattern {
        bits: BitPattern::from_le(&[0u8; SIZE]),
        valid: BitRanges::from_le(&[concat_arrays!([1u8], [0u8; SIZE - 1])..=[0xffu8; SIZE]])
    })

}
