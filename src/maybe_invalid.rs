use core::{fmt, mem::MaybeUninit};

use crate::Nicheless;

/// Represents a value of the type `T` that may not be well-defined.
///
/// This type guarantees that `MaybeInvalid<T>` and `T`
/// have the same, size, alignment and ABI.
///
/// Despite that this type uses [`MaybeUninit`] internally,
/// the value must always be initialized.
///
/// `MaybeInvalid<T>` is always well-defined if `T: Nicheless`.
#[repr(transparent)]
pub struct MaybeInvalid<T>(MaybeUninit<T>);

impl<T> MaybeInvalid<T> {
    /// Creates a valid value.
    pub fn new(valid: T) -> Self {
        Self(MaybeUninit::new(valid))
    }

    /// Creates a potentially invalid value.
    pub fn zeroed() -> Self {
        Self(MaybeUninit::zeroed())
    }

    /// Gets a shared reference to the value in this container as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        // SAEFTY: u8 has an alignment of 1 so is never unaligned
        unsafe {
            core::slice::from_raw_parts(self.as_ptr() as *const u8, core::mem::size_of::<Self>())
        }
    }

    /// Extracts the value from this container.
    ///
    /// # Safety
    ///
    /// It is up to the caller to ensure the value in this container is valid.
    /// For example, the following code causes UB:
    /// ```no_run
    /// # use core::num::NonZeroU8;
    /// # use iffi::MaybeInvalid;
    /// let invalid = MaybeInvalid::<NonZeroU8>::zeroed();
    /// // UB because NonZeroU8 cannot be zero.
    /// let zero = unsafe { invalid.assume_valid() };
    /// ```
    pub unsafe fn assume_valid(self) -> T {
        self.0.assume_init()
    }

    /// Gets a shared reference to the value in this container.
    ///
    /// # Safety
    ///
    /// See [`assume_valid`].
    ///
    /// [`assume_valid`]: Self::assume_valid
    pub unsafe fn assume_valid_ref(&self) -> &T {
        self.0.assume_init_ref()
    }

    /// Gets a mutable (unique) reference to the value in this container.
    ///
    /// # Safety
    ///
    /// See [`assume_valid`].
    ///
    /// [`assume_valid`]: Self::assume_valid
    pub unsafe fn assume_valid_mut(&mut self) -> &mut T {
        self.0.assume_init_mut()
    }

    /// Gets a pointer to the contained value
    pub fn as_ptr(&self) -> *const T {
        self.0.as_ptr()
    }
}

// SAFETY: MaybeInvalid does not require a well-defined contained value.
unsafe impl<T> Nicheless for MaybeInvalid<T> {}

impl<T: Nicheless> MaybeInvalid<T> {
    /// Extracts the value in this container.
    ///
    /// Only implemented for [`Nicheless`] types.
    ///
    /// If the type is not nicheless, use [`assume_valid`].
    ///
    /// # Example
    /// ```rust
    /// # use iffi::MaybeInvalid;
    /// let valid = MaybeInvalid::new(5u8);
    /// assert_eq!(valid.into_inner(), 5)
    /// ```
    ///
    /// [`assume_valid`]: MaybeInvalid::assume_valid
    pub fn into_inner(self) -> T {
        // SAFETY: nicheless types are always well-defined.
        unsafe { self.assume_valid() }
    }

    /// Gets a shared reference to the value in this container.
    ///
    /// Only implemented for [`Nicheless`] types.
    ///
    /// If the type is not nicheless, use [`assume_valid_ref`].
    ///
    /// [`assume_valid_ref`]: MaybeInvalid::assume_valid_ref
    pub fn get(&self) -> &T {
        // SAFETY: nicheless types are always well-defined.
        unsafe { self.assume_valid_ref() }
    }

    /// Gets a mutable (unique) reference to the value in this container.
    ///
    /// Only implemented for [`Nicheless`] types.
    ///
    /// If the type is not nicheless, use [`assume_valid_mut`].
    ///
    /// [`assume_valid_mut`]: MaybeInvalid::assume_valid_mut
    pub fn get_mut(&mut self) -> &mut T {
        // SAFETY: nicheless types are always well-defined.
        unsafe { self.assume_valid_mut() }
    }
}

impl<T> fmt::Debug for MaybeInvalid<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MaybeInvalid")
            .field(&self.as_bytes())
            .finish()
    }
}
