use core::{fmt, ops::RangeInclusive};

use bytemuck::Pod;

/// A statically-sized pattern of bits.
///
/// See [the glossary entry] for more detail.
///
/// Currently supports up to 20 bytes.
/// If more space is needed, consider enabling the `"alloc"` feature.
///
/// [the glossary entry]: crate#bit-pattern
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct BitPattern {
    bytes: [u8; 20],
    len: usize,
}

impl BitPattern {
    pub fn from_le<T: Pod>(value: &T) -> Self {
        let mut bytes = bytemuck::bytes_of(value);
        let len = bytes.len().min(20);
        #[allow(unused_mut)]
        let mut buf = [0u8; 20];

        buf[..len].copy_from_slice(&bytes[..len]);

        #[cfg(target_endian = "big")]
        buf[..len].reverse();
        Self { bytes: buf, len }
    }
}

impl fmt::Display for BitPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for byte in self.bytes[0..self.len].iter().rev() {
            write!(f, "{byte:02x}")?;
        }

        Ok(())
    }
}

/// A set of ranges of bit-patterns that are applicable for a type.
///
/// Serves no purpose without the `"alloc"` feature.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct BitRanges(());

impl BitRanges {
    pub fn from_le<T: Pod>(_: &[RangeInclusive<T>]) -> Self {
        Self(())
    }
}
