//! Universe IDs and the 16-hex-digit form every server `u64` takes in text.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// Digits in the text form of a `u64`.
const HEX64_DIGITS: usize = 16;

/// The identity of one save, drawn at random when the universe is created.
///
/// Two campaigns may share a seed and a generator version and still differ in their overlays, so a
/// save is named by this and not by its seed (plan 04, design note 16). Its text form, used for the
/// save's directory and on the wire, is 16 lowercase hexadecimal digits.
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
    /// 16 lowercase hexadecimal digits, zero-padded.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl FromStr for UniverseId {
    type Err = ParseHex64Error;

    /// Parses exactly 16 lowercase hexadecimal digits; upper case is refused so that one ID has
    /// one text form.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_hex64(text).map(Self)
    }
}

/// Text was not the 16-hex-digit form of a `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseHex64Error {
    /// The text was not 16 characters long.
    WrongLength,
    /// A character was not one of `0-9` or `a-f`.
    InvalidDigit,
}

impl fmt::Display for ParseHex64Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected 16 lowercase hexadecimal digits")
    }
}

impl Error for ParseHex64Error {}

/// Formats a `u64` as 16 lowercase hexadecimal digits.
pub(crate) fn format_hex64(value: u64) -> String {
    format!("{value:016x}")
}

/// Parses exactly 16 lowercase hexadecimal digits.
pub(crate) fn parse_hex64(text: &str) -> Result<u64, ParseHex64Error> {
    if text.len() != HEX64_DIGITS {
        return Err(ParseHex64Error::WrongLength);
    }
    text.bytes().try_fold(0_u64, |value, byte| {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => return Err(ParseHex64Error::InvalidDigit),
        };
        Ok((value << 4) | u64::from(digit))
    })
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
    fn hex64_rejects_uppercase_short_long_and_prefix() {
        assert_eq!(
            parse_hex64("00000000000004D2"),
            Err(ParseHex64Error::InvalidDigit)
        );
        assert_eq!(parse_hex64("4d2"), Err(ParseHex64Error::WrongLength));
        assert_eq!(
            parse_hex64("000000000000004d2"),
            Err(ParseHex64Error::WrongLength)
        );
        assert_eq!(
            parse_hex64("0x000000000004d2"),
            Err(ParseHex64Error::InvalidDigit)
        );
        // Sixteen bytes but not sixteen digits.
        assert_eq!(
            parse_hex64("00000000000004é"),
            Err(ParseHex64Error::InvalidDigit)
        );
    }

    #[test]
    fn hex64_round_trips() {
        for value in [0, 1, 0x4d2, 0x0123_4567_89ab_cdef, u64::MAX] {
            assert_eq!(parse_hex64(&format_hex64(value)), Ok(value));
        }
    }
}
