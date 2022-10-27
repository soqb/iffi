use core::{
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroU128, NonZeroU16,
        NonZeroU32, NonZeroU64, NonZeroU8,
    },
    ptr::NonNull, mem::{size_of_val, size_of}, any::type_name,
};

use crate::{Error, Iffi};

/// Implementing this trait marks that `Option<T>` has the same size as `T`
/// which in turn implements [the `Iffi` trait] for `Option<T>`
///
/// [the `Iffi` trait]: [`crate::Iffi`]
pub unsafe trait SameSizeOption {}
unsafe impl<T> SameSizeOption for NonNull<T> {}
unsafe impl<'a, T> SameSizeOption for &'a T {}
unsafe impl<'a, T> SameSizeOption for &'a mut T {}
#[cfg(feature = "alloc")]
mod alloc_impls {
    use super::SameSizeOption;
    use alloc::{
        boxed::Box,
        rc::{self, Rc},
        string::String,
        sync::{self, Arc},
        vec::Vec,
    };

    unsafe impl<T> SameSizeOption for Box<T> {}
    unsafe impl SameSizeOption for String {}
    unsafe impl<T> SameSizeOption for Vec<T> {}
    unsafe impl<T> SameSizeOption for Rc<T> {}
    unsafe impl<T> SameSizeOption for Arc<T> {}
    unsafe impl<T> SameSizeOption for rc::Weak<T> {}
    unsafe impl<T> SameSizeOption for sync::Weak<T> {}
}

macro_rules! impl_bulk {
    ($($ty:ty),+$(,)?) => {
        $(
            unsafe impl SameSizeOption for $ty {}
        )+
    };
}

impl_bulk!(
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

// SAFETY: relies on implementation for `T`
unsafe impl<T: Iffi + SameSizeOption> Iffi for Option<T> {
    type Subset = T::Subset;

    fn can_transmute(&self) -> Result<(), Error> {
        debug_assert_eq!(
            size_of_val(&self),
            size_of::<T>(),
            "`iffi::niche::SameSizeOption` is implemented for {} but they are not the same size!",
            type_name::<T>(),
        );
        self.as_ref()
            .map_or_else(|| Err(Error::FoundNone), |value| value.can_transmute())
    }
}
