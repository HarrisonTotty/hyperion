//! The parser behind every 16-digit text form: a universe's seed, a system ID, an event word.
//!
//! Each text form is fixed-width lower-case hexadecimal, one string per value, so that a JSON
//! reader cannot round it and one value has one string. The parser accepts exactly that form and
//! names what is wrong with any other, and each caller turns the fault into its own error type.

/// Why a fixed-width hexadecimal field did not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HexFault {
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
pub(crate) fn parse_lower_hex(text: &str, digits: usize) -> Result<u64, HexFault> {
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
