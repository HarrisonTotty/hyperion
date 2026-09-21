//! What a stream is keyed by: the universe seed and the object it draws for.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use super::TagScope;
use crate::id::{HexFault, parse_lower_hex};

/// A universe's seed: with [`GENERATOR_VERSION`](crate::GENERATOR_VERSION), what identifies a
/// universe.
///
/// It is the first key word of every stream. Its text form is exactly 16 lower-case hexadecimal
/// digits, as a [`SystemId`](crate::id::SystemId)'s is, so that one seed has one string and a
/// JSON reader cannot round it.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
///
/// let seed: Seed = "00000000deadbeef".parse()?;
/// assert_eq!(seed.get(), 0xdead_beef);
/// assert_eq!(seed.to_string(), "00000000deadbeef");
/// # Ok::<(), hyperion_sim::rng::ParseSeedError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seed(u64);

impl Seed {
    /// The seed with this value. Every `u64` is a seed.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The seed's value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Seed {
    /// Exactly 16 lower-case hexadecimal digits.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::LowerHex for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl FromStr for Seed {
    type Err = ParseSeedError;

    /// Parses exactly 16 lower-case hexadecimal digits.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_lower_hex(s, 16)
            .map(Self)
            .map_err(ParseSeedError::from_fault)
    }
}

/// A [`Seed`] did not parse from its text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseSeedError {
    /// The text contains whitespace.
    Whitespace,
    /// The text starts with `0x`.
    HexPrefix,
    /// The text is not 16 bytes long.
    WrongLength {
        /// The length found, in bytes.
        length: usize,
    },
    /// A digit is upper case.
    UpperCase,
    /// A character is not a hexadecimal digit.
    InvalidDigit,
}

impl ParseSeedError {
    fn from_fault(fault: HexFault) -> Self {
        match fault {
            HexFault::Whitespace => Self::Whitespace,
            HexFault::HexPrefix => Self::HexPrefix,
            HexFault::WrongLength(length) => Self::WrongLength { length },
            HexFault::UpperCase => Self::UpperCase,
            HexFault::InvalidDigit => Self::InvalidDigit,
        }
    }
}

impl fmt::Display for ParseSeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Whitespace => f.write_str("a seed contains no whitespace"),
            Self::HexPrefix => f.write_str("a seed has no 0x prefix"),
            Self::WrongLength { length } => {
                write!(f, "a seed is 16 hexadecimal digits, not {length} bytes")
            }
            Self::UpperCase => f.write_str("a seed is lower case"),
            Self::InvalidDigit => f.write_str("a seed is hexadecimal digits only"),
        }
    }
}

impl Error for ParseSeedError {}

/// The object a stream draws for: its counter word 0, its sub-object number and its scope.
///
/// | Scope                      | Word                                          | Sub              |
/// | -------------------------- | --------------------------------------------- | ---------------- |
/// | [`TagScope::Galaxy`]       | 0 for the galaxy, or an item number of a list | 0                |
/// | [`TagScope::Cell`]         | the cell word: a candidate ID, index zeroed   | 0                |
/// | [`TagScope::Feature`]      | the feature's object word                     | 0                |
/// | [`TagScope::System`]       | the [`SystemId`](crate::id::SystemId)'s raw value | 0            |
/// | [`TagScope::Body`]         | the body's system's raw value                 | the body index   |
///
/// The word becomes counter word 0 and `sub` the top 16 bits of counter word 1. Keys are built
/// from integers only: float bits are never hashed. A key of one scope may share its word with a
/// key of another (a cell's word is its candidate 0's ID, and body 0 shares its system's word
/// and `sub`), which is safe because a domain tag names one scope and [`Stream::open`] checks it.
///
/// [`Stream::open`]: super::Stream::open
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey {
    word: u64,
    sub: u16,
    scope: TagScope,
}

impl ObjectKey {
    /// A key from its parts. The public constructors and the conversions from IDs call this.
    #[must_use]
    pub(crate) const fn new(word: u64, sub: u16, scope: TagScope) -> Self {
        Self { word, sub, scope }
    }

    /// The galaxy as a whole: word 0.
    ///
    /// This is the same key as [`galaxy_item(0)`](Self::galaxy_item). A tag is used with one
    /// form or the other, never both.
    #[must_use]
    pub const fn galaxy() -> Self {
        Self::new(0, 0, TagScope::Galaxy)
    }

    /// Item `n` of a galaxy-wide list, such as a progenitor or a halo component.
    #[must_use]
    pub const fn galaxy_item(n: u64) -> Self {
        Self::new(n, 0, TagScope::Galaxy)
    }

    /// A generation cell, by its word: the ID of the cell's candidate 0 with the index zeroed,
    /// [`SystemId::cell_word`](crate::id::SystemId::cell_word).
    #[must_use]
    pub const fn cell(word: u64) -> Self {
        Self::new(word, 0, TagScope::Cell)
    }

    /// A feature, by its object word, such as
    /// [`FeatureRef::object_word`](crate::id::FeatureRef::object_word).
    #[must_use]
    pub const fn feature(word: u64) -> Self {
        Self::new(word, 0, TagScope::Feature)
    }

    /// Counter word 0 of every stream opened for this object.
    #[must_use]
    pub const fn word(self) -> u64 {
        self.word
    }

    /// The sub-object number: a body's index, otherwise 0.
    #[must_use]
    pub const fn sub(self) -> u16 {
        self.sub
    }

    /// The kind of object this key names, which must be its tag's scope.
    #[must_use]
    pub const fn scope(self) -> TagScope {
        self.scope
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_text_is_sixteen_lower_case_digits() {
        for value in [0, 1, 0xdead_beef, u64::MAX, 1 << 63] {
            let seed = Seed::new(value);
            let text = seed.to_string();
            assert_eq!(text.len(), 16);
            assert_eq!(text.parse::<Seed>(), Ok(seed));
        }
        assert_eq!(Seed::new(0xabc).to_string(), "0000000000000abc");
        assert_eq!(format!("{:x}", Seed::new(0xabc)), "abc");
    }

    #[test]
    fn seed_parser_rejects_each_malformation_with_its_own_variant() {
        let cases = [
            ("00000000DEADBEEF", ParseSeedError::UpperCase),
            ("0x000000deadbeef", ParseSeedError::HexPrefix),
            (" 0000000deadbeef", ParseSeedError::Whitespace),
            ("deadbeef", ParseSeedError::WrongLength { length: 8 }),
            ("000000000000000g", ParseSeedError::InvalidDigit),
        ];
        for (text, error) in cases {
            assert_eq!(text.parse::<Seed>(), Err(error), "{text:?}");
        }
        assert_eq!(
            ParseSeedError::WrongLength { length: 8 }.to_string(),
            "a seed is 16 hexadecimal digits, not 8 bytes"
        );
    }

    #[test]
    fn a_seed_above_two_to_the_fifty_three_survives_its_text_form() {
        let seed = Seed::new((1 << 53) + 1);
        assert_eq!(seed.to_string().parse::<Seed>(), Ok(seed));
    }

    #[test]
    fn constructors_fix_word_sub_and_scope() {
        assert_eq!(ObjectKey::galaxy(), ObjectKey::galaxy_item(0));
        let item = ObjectKey::galaxy_item(7);
        assert_eq!(
            (item.word(), item.sub(), item.scope()),
            (7, 0, TagScope::Galaxy)
        );
        let cell = ObjectKey::cell(0x0200_0800_2000_0000);
        assert_eq!(
            (cell.word(), cell.sub(), cell.scope()),
            (0x0200_0800_2000_0000, 0, TagScope::Cell)
        );
        let feature = ObjectKey::feature(42);
        assert_eq!(
            (feature.word(), feature.sub(), feature.scope()),
            (42, 0, TagScope::Feature)
        );
    }
}
