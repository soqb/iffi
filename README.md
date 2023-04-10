Safe conversion between FFI-safe and ergonomic Rust types.

An implementation of [`Iffi`] trait (which can be derived) on a type `T`
over a *[universe]* `U` (the type parameter on the trait)
provides assertions that a value of type `U` can be safely [transmuted] into a value of type `T`.

[`iffi::try_from`][try_from] and [`iffi::into`][into] perform the conversion safely.

[universe]: #universe
[transmuted]: core::mem::transmute

# Glossary & Concepts

### FFI-safe
a type that can be used with [FFI].

This can be a primitve, a type marked with `#[repr(C)]` or `#[repr(Int)]`,
or a type marked with `#[repr(transparent)]` with only one non-zero sized field which must be FFI-safe.

[FFI]: https://doc.rust-lang.org/nomicon/FFI.html

### bit-pattern
a sequence of bits representing a potential value of a type.

Each type defines some bit-patterns (with the same number of bits as the type)
that are valid and others that are invalid.

For example, for `u16`, all bit-patterns are valid because `0x0000` through `0xffff`
are all correctly initialized `u16` values,
but for `bool` only the patterns `0x00` and `0x01`, (`false` and `true` respectively) are valid.
`0x02..=0xff` are all *invalid* bit-patterns for `bool`.

An invalid bit-pattern is also called a niche.
Types like `u16` where all bit-patterns are valid are *nicheless*.

This definition is subtly different to the one in the Unsafe Code Guidelines.
In the context of bit-patterns this crate assumes all bits are initialized,
so there is a one-to-one relationship between invalid bit patterns and niches.

### well-defined
a potential value represented in memory by a valid bit-pattern.

A null reference or zeroed `NonZero*` is not well-defined, for example.

### nicheless
a type with no niches i.e. no invalid bit-patterns.

The [`Nicheless`] trait cannot be automatically implemented
and must be derived or implemented manually.

ZSTs are always nicheless. Uninhabited types are never nicheless.

[A more technical definition] is availible in the Unsafe Code Guidelines.

[A more technical definition]: https://rust-lang.github.io/unsafe-code-guidelines/glossary.html#niche

### layout
the combination of the size, alignment and ABI behaviour of a type.

### universe
a nicheless type with the same layout as some potentially non-nicheless type.

[`MaybeInvalid<T>`] is a universe of all `T`.

A type may have many universes.
As an example, `NonZeroU8` has the universes `MaybeInvalid<NonZeroU8>`
and `u8`.