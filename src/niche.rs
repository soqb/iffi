use core::{
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroU128, NonZeroU16,
        NonZeroU32, NonZeroU64, NonZeroU8, Wrapping,
    },
    ptr::NonNull,
};

/// Indicates that a value of this type is FFI-safe and well-defined no matter the underlying bit-pattern.
///
/// Equally, that the type [has no niches].
///
/// If you are familiar with [`bytemuck`], types that can implement this trait
/// are a strict superset of types that can implement [`bytemuck::Pod`]
/// and a strict subset of types that can implement [`bytemuck::Zeroable`].
/// Namely, `Nicheless` types:
/// * must be FFI-safe (`#[repr(C)]`, `#[repr(Int)]`, or `#[repr(transparent)]` over an FFI-safe type).
/// * must be valid for *any* bit-pattern, not just all zeros.
/// * are allowed to have padding bytes.
/// * are allowed to contain pointers, atomics and interior-mutable types.
///
/// In the case of niche optimization (where `sizeof::<T>() == sizeof::<Option<T>>`),
/// the type `Option<T>` will implement `Nicheless` if the trait [`OneNiche`] is implemented for `T`.
///
/// [`Iffi`] is implemented automatically for all types that implement `Nicheless`,
/// both reflexively and over [`MaybeInvalid<Self>`].
///
/// # Safety
/// * All fields must be nicheless.
/// * The type must not be uninhabited.
/// * The type must not have pointer metadata.
/// * The type must be FFI-safe.
///
/// [has no niches]: crate#nicheless
/// [`bytemuck`]: https://docs.rs/bytemuck/latest/bytemuck
/// [`bytemuck::Pod`]: https://docs.rs/bytemuck/latest/bytemuck/trait.Pod.html
/// [`bytemuck::Zeroable`]: https://docs.rs/bytemuck/latest/bytemuck/trait.Zeroable.html
/// [`Iffi`]: crate::Iffi
/// [`MaybeInvalid<Self>`]: crate::MaybeInvalid
pub unsafe trait Nicheless: Sized {}

// SAFETY: does not have to be initialised
unsafe impl<T> Nicheless for MaybeUninit<T> {}

// SAFETY: all types below will always be trivially nicheless.
unsafe impl<T> Nicheless for PhantomData<T> {}
unsafe impl Nicheless for PhantomPinned {}
unsafe impl Nicheless for u8 {}
unsafe impl Nicheless for u16 {}
unsafe impl Nicheless for u32 {}
unsafe impl Nicheless for u64 {}
unsafe impl Nicheless for u128 {}
unsafe impl Nicheless for usize {}
unsafe impl Nicheless for i8 {}
unsafe impl Nicheless for i16 {}
unsafe impl Nicheless for i32 {}
unsafe impl Nicheless for i64 {}
unsafe impl Nicheless for i128 {}
unsafe impl Nicheless for isize {}
unsafe impl Nicheless for f32 {}
unsafe impl Nicheless for f64 {}

// SAFETY: all addresses (including null) are valid for pointers.
unsafe impl<T> Nicheless for *const T {}
unsafe impl<T> Nicheless for *mut T {}

// SAFETY: made up of nicheless types.
unsafe impl<T: Nicheless, const N: usize> Nicheless for [T; N] {}
unsafe impl<T: Nicheless> Nicheless for Wrapping<T> {}

#[cfg(feature = "atomics")]
mod atomics {
    use core::sync::atomic::*;

    use crate::Nicheless;

    // SAFETY: identical layout to non-atomic types.
    unsafe impl Nicheless for AtomicU8 {}
    unsafe impl Nicheless for AtomicU16 {}
    unsafe impl Nicheless for AtomicU32 {}
    unsafe impl Nicheless for AtomicU64 {}
    unsafe impl Nicheless for AtomicUsize {}
    unsafe impl Nicheless for AtomicI8 {}
    unsafe impl Nicheless for AtomicI16 {}
    unsafe impl Nicheless for AtomicI32 {}
    unsafe impl Nicheless for AtomicI64 {}
    unsafe impl Nicheless for AtomicIsize {}
    unsafe impl<T> Nicheless for AtomicPtr<T> {}
}

/// Indicates that the type has exactly one niche, which is filled by [niche optimization]
/// and so implements [`Nicheless`] in option-like enums (`Option<T>`, `Result<T, ()>`).
///
/// [niche optimization]: https://rust-lang.github.io/unsafe-code-guidelines/layout/enums.html#discriminant-elision-on-option-like-enums
///
/// # Safety
/// Type must have exactly one niche.
pub unsafe trait OneNiche {}

// SAFETY: `None` fills the single remaining open niche.
unsafe impl<T: OneNiche> Nicheless for Option<T> {}

// SAFETY: `Err(())` fills the single remaining open niche.
unsafe impl<T: OneNiche> Nicheless for Result<T, ()> {}

// SAFETY: `Ok(())` fills the single remaining open niche.
unsafe impl<E: OneNiche> Nicheless for Result<(), E> {}

// SAFETY: only cannot be null
unsafe impl<T> OneNiche for NonNull<T> {}
// SAFETY: only cannot be null
unsafe impl<'a, T> OneNiche for &'a T {}
// SAFETY: only cannot be null
unsafe impl<'a, T> OneNiche for &'a mut T {}

macro_rules! impl_one_niche_bulk {
    ($($ty:ty),+$(,)?) => {
        $(
            unsafe impl OneNiche for $ty {}
        )+
    };
}

impl_one_niche_bulk!(
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

macro_rules! impl_oneniche_fn {
    ({ $($mod:tt)* }) => {
        unsafe impl<R> OneNiche for $($mod)* fn() -> R {}
    };
    ({ $($mod:tt)* } $f:ident, $($t:ident,)*) => {
        impl_oneniche_fn!({ $($mod)* } $($t,)*);

        unsafe impl<R, $f, $($t),*> OneNiche for $($mod)* fn($f, $($t),*) -> R {}
    };
}

// SAFETY: function pointers are non-null pointers.
impl_oneniche_fn! {
    {} T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16,
}

impl_oneniche_fn! {
    { unsafe } T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16,
}

impl_oneniche_fn! {
    { extern "C" } T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16,
}

impl_oneniche_fn! {
    { unsafe extern "C" } T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16,
}
