//! The parser behind every 16-digit text form: a universe's seed, a system ID, an event word.
//!
//! Each text form is fixed-width lower-case hexadecimal, one string per value, so that a JSON
//! reader cannot round it and one value has one string. The parser accepts exactly that form and
//! names what is wrong with any other, and each caller turns the fault into its own error type.

use std::error::Error;
use std::fmt;

/// Why a fixed-width hexadecimal field did not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HexFault {
    /// The text contains whitespace.
    Whitespace,
    /// The text starts with `0x` or `0X`.
    HexPrefix,
    /// The text is not the expected number of bytes long; the length found, in bytes.
    WrongLength(usize),
    /// A digit is upper case.
    UpperCase,
    /// A character is not a hexadecimal digit.
    InvalidDigit,
}

/// Parses exactly `digits` lower-case hexadecimal digits.
///
/// # Errors
///
/// The first fault found, checked in this order: whitespace anywhere, a `0x` prefix, a length
/// other than `digits`, then the first upper-case or non-hexadecimal character.
pub fn parse_lower_hex(text: &str, digits: usize) -> Result<u64, HexFault> {
    if text.chars().any(char::is_whitespace) {
        return Err(HexFault::Whitespace);
    }
    if text.starts_with("0x") || text.starts_with("0X") {
        return Err(HexFault::HexPrefix);
    }
    if text.len() != digits {
        return Err(HexFault::WrongLength(text.len()));
    }
    let mut value = 0_u64;
    for b in text.bytes() {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => return Err(HexFault::UpperCase),
            _ => return Err(HexFault::InvalidDigit),
        };
        value = (value << 4) | u64::from(digit);
    }
    Ok(value)
}

impl fmt::Display for HexFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Whitespace => f.write_str("hexadecimal text contains no whitespace"),
            Self::HexPrefix => f.write_str("hexadecimal text has no 0x prefix"),
            Self::WrongLength(length) => {
                write!(f, "hexadecimal text of the wrong length, {length} bytes")
            }
            Self::UpperCase => f.write_str("hexadecimal text is lower case"),
            Self::InvalidDigit => f.write_str("hexadecimal text is hexadecimal digits only"),
        }
    }
}

impl Error for HexFault {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_the_lower_case_form_parses() {
        assert_eq!(parse_lower_hex("00000000deadbeef", 16), Ok(0xdead_beef));
        assert_eq!(parse_lower_hex("ffffffffffffffff", 16), Ok(u64::MAX));
        assert_eq!(parse_lower_hex("0003", 4), Ok(3));
    }

    #[test]
    fn each_malformation_has_its_own_fault_in_order() {
        for (text, digits, fault) in [
            (" 0000000deadbeef", 16, HexFault::Whitespace),
            ("0x00", 4, HexFault::HexPrefix),
            ("0X000000deadbeef", 16, HexFault::HexPrefix),
            ("deadbeef", 16, HexFault::WrongLength(8)),
            ("00000000DEADBEEF", 16, HexFault::UpperCase),
            ("000000000000000g", 16, HexFault::InvalidDigit),
            // Whitespace is found before a wrong length, a prefix before an upper-case digit.
            ("dead beef", 16, HexFault::Whitespace),
            ("0xABCDEF0123456", 16, HexFault::HexPrefix),
        ] {
            assert_eq!(parse_lower_hex(text, digits), Err(fault), "{text:?}");
        }
        assert_eq!(
            HexFault::WrongLength(8).to_string(),
            "hexadecimal text of the wrong length, 8 bytes"
        );
    }
}
