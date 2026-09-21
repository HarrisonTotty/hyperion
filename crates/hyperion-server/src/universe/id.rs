//! Universe IDs, whose text form is the wire's: [`UniverseIdHex`], 16 lowercase hexadecimal
//! digits.

use std::fmt;
use std::str::FromStr;

use hyperion_protocol::{ParseHex64Error, UniverseIdHex};

/// The identity of one save, drawn at random when the universe is created.
///
/// Two campaigns may share a seed and a generator version and still differ in their overlays, so a
/// save is named by this and not by its seed (plan 04, design note 16). Its text form, used for the
/// save's directory and on the wire, is that of [`UniverseIdHex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UniverseId(u64);

impl UniverseId {
    /// Wraps a raw ID.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw ID.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for UniverseId {
    /// The wire form: 16 lowercase hexadecimal digits, zero-padded.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(UniverseIdHex::from(*self).as_str())
    }
}

impl FromStr for UniverseId {
    type Err = ParseHex64Error;

    /// Parses the wire form. Upper case is refused, so that one ID has one text form.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        UniverseIdHex::try_from(text.to_owned()).map(|hex| Self::from(&hex))
    }
}

impl From<UniverseId> for UniverseIdHex {
    fn from(id: UniverseId) -> Self {
        Self::from_u64(id.0)
    }
}

impl From<&UniverseIdHex> for UniverseId {
    fn from(hex: &UniverseIdHex) -> Self {
        Self(hex.to_u64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn universe_id_text_is_sixteen_lowercase_hex_digits() {
        assert_eq!(UniverseId::new(0x4d2).to_string(), "00000000000004d2");
        assert_eq!(UniverseId::new(u64::MAX).to_string(), "ffffffffffffffff");
        assert_eq!("00000000000004d2".parse(), Ok(UniverseId::new(0x4d2)));
        assert_eq!("ffffffffffffffff".parse(), Ok(UniverseId::new(u64::MAX)));
    }

    #[test]
    fn universe_id_text_refuses_what_the_wire_refuses() {
        for (text, error) in [
            ("00000000000004D2", ParseHex64Error::InvalidDigit),
            ("4d2", ParseHex64Error::WrongLength),
            ("000000000000004d2", ParseHex64Error::WrongLength),
            ("0x000000000004d2", ParseHex64Error::InvalidDigit),
        ] {
            assert_eq!(text.parse::<UniverseId>(), Err(error), "parsing {text:?}");
        }
    }

    #[test]
    fn universe_id_round_trips_through_its_wire_type() {
        for raw in [0, 1, 0x4d2, 0x0123_4567_89ab_cdef, u64::MAX] {
            let id = UniverseId::new(raw);
            let hex = UniverseIdHex::from(id);
            assert_eq!(hex.as_str(), id.to_string());
            assert_eq!(UniverseId::from(&hex), id);
        }
    }
}
