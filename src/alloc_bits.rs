use core::{fmt, ops::RangeInclusive};

use alloc::{boxed::Box, format};
use bytemuck::Pod;
use smallvec::{SmallVec, ToSmallVec};

/// A dynamically-sized pattern of bits.
///
/// See [the glossary entry] for more detail.
///
/// [the glossary entry]: crate#bit-pattern
#[derive(PartialEq, Eq, Debug, Clone, PartialOrd, Ord)]
pub struct BitPattern(SmallVec<[u8; 20]>);

impl BitPattern {
    pub fn from_le<T: Pod>(value: &T) -> Self {
        #[allow(unused_mut)]
        let mut bytes = bytemuck::bytes_of(value).to_smallvec();
        #[cfg(target_endian = "big")]
        bytes.reverse();
        Self(bytes)
    }
}

impl fmt::Display for BitPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for byte in self.0.iter().rev() {
            write!(f, "{byte:02x}")?;
        }

        Ok(())
    }
}

/// A set of ranges of bit-patterns that are applicable for a type.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct BitRanges(Box<SmallVec<[RangeInclusive<BitPattern>; 1]>>);

impl fmt::Display for BitRanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(
                self.0
                    .iter()
                    .map(|pat| format!("{}..={}", pat.start(), pat.end())),
            )
            .finish()
    }
}

impl BitRanges {
    pub fn from_le<T: Pod>(value: &[RangeInclusive<T>]) -> Self {
        let boxed =
            SmallVec::from_iter(value.iter().map(|range| {
                BitPattern::from_le(range.start())..=BitPattern::from_le(range.end())
            }));
        Self(Box::new(boxed))
    }
}
