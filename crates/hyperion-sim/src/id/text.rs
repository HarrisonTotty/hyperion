//! Text forms of IDs: fixed-width lower-case hexadecimal, one string per ID.
//!
//! | Type          | Form                         | Example                                  |
//! | ------------- | ---------------------------- | ---------------------------------------- |
//! | [`SystemId`]  | 16 digits                    | `0200080020000000`                       |
//! | [`BodyId`]    | system, `.`, 4 digits        | `0200080020000000.0003`                  |
//! | [`EventWord`] | 16 digits                    | `0001ffffffffff00`                       |
//! | [`EventId`]   | subject, `:`, word           | `0200080020000000.0003:0001ffffffffff00` |
//!
//! Parsers accept exactly these forms. Upper case, a `0x` prefix, whitespace and wrong lengths are
//! rejected, each with its own variant, so that one ID has one string. The protocol sends IDs as
//! text because a JSON number loses integers above 2⁵³. The text forms are not part of the
//! generator version.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use super::body::BodyId;
use super::event::{DecodeEventWordError, EventId, EventSubject, EventWord};
use super::system::{DecodeSystemIdError, SystemId};

/// Why a fixed-width hexadecimal field did not parse. Shared with `rng::Seed`'s text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HexFault {
    Whitespace,
    HexPrefix,
    WrongLength(usize),
    UpperCase,
    InvalidDigit,
}

/// Parses exactly `digits` lower-case hexadecimal digits.
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

/// A [`SystemId`] did not parse from its text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseSystemIdError {
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
    /// The digits are a malformed ID.
    Decode(DecodeSystemIdError),
}

impl ParseSystemIdError {
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

impl fmt::Display for ParseSystemIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Whitespace => f.write_str("a system id contains no whitespace"),
            Self::HexPrefix => f.write_str("a system id has no 0x prefix"),
            Self::WrongLength { length } => {
                write!(
                    f,
                    "a system id is 16 hexadecimal digits, not {length} bytes"
                )
            }
            Self::UpperCase => f.write_str("a system id is lower case"),
            Self::InvalidDigit => f.write_str("a system id is hexadecimal digits only"),
            Self::Decode(e) => write!(f, "malformed system id: {e}"),
        }
    }
}

impl Error for ParseSystemIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::Whitespace
            | Self::HexPrefix
            | Self::WrongLength { .. }
            | Self::UpperCase
            | Self::InvalidDigit => None,
        }
    }
}

impl fmt::Display for SystemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.raw())
    }
}

impl fmt::LowerHex for SystemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.raw(), f)
    }
}

impl FromStr for SystemId {
    type Err = ParseSystemIdError;

    /// Parses exactly 16 lower-case hexadecimal digits, then validates them with
    /// [`SystemId::from_raw`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw = parse_lower_hex(s, 16).map_err(ParseSystemIdError::from_fault)?;
        Self::from_raw(raw).map_err(ParseSystemIdError::Decode)
    }
}

/// A [`BodyId`] did not parse from its text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseBodyIdError {
    /// The text contains whitespace.
    Whitespace,
    /// The text has no full stop between the system and the body index.
    MissingSeparator,
    /// The system part did not parse.
    System(ParseSystemIdError),
    /// The body index starts with `0x`.
    HexPrefix,
    /// The body index is not 4 bytes long.
    WrongLength {
        /// The length found, in bytes.
        length: usize,
    },
    /// A digit of the body index is upper case.
    UpperCase,
    /// A character of the body index is not a hexadecimal digit.
    InvalidDigit,
}

impl ParseBodyIdError {
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

impl fmt::Display for ParseBodyIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Whitespace => f.write_str("a body id contains no whitespace"),
            Self::MissingSeparator => f.write_str("a body id is a system id, '.' and an index"),
            Self::System(e) => write!(f, "invalid system in body id: {e}"),
            Self::HexPrefix => f.write_str("a body index has no 0x prefix"),
            Self::WrongLength { length } => {
                write!(
                    f,
                    "a body index is 4 hexadecimal digits, not {length} bytes"
                )
            }
            Self::UpperCase => f.write_str("a body index is lower case"),
            Self::InvalidDigit => f.write_str("a body index is hexadecimal digits only"),
        }
    }
}

impl Error for ParseBodyIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::System(e) => Some(e),
            Self::Whitespace
            | Self::MissingSeparator
            | Self::HexPrefix
            | Self::WrongLength { .. }
            | Self::UpperCase
            | Self::InvalidDigit => None,
        }
    }
}

impl fmt::Display for BodyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:04x}", self.system(), self.body_index())
    }
}

impl FromStr for BodyId {
    type Err = ParseBodyIdError;

    /// Parses `<16 digits>.<4 digits>`, lower-case hexadecimal.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.chars().any(char::is_whitespace) {
            return Err(ParseBodyIdError::Whitespace);
        }
        let (system, index) = s
            .split_once('.')
            .ok_or(ParseBodyIdError::MissingSeparator)?;
        let system = system.parse().map_err(ParseBodyIdError::System)?;
        let index = parse_lower_hex(index, 4).map_err(ParseBodyIdError::from_fault)?;
        let index = u16::try_from(index).expect("four hexadecimal digits fit in u16");
        Ok(Self::new(system, index))
    }
}

/// An [`EventWord`] did not parse from its text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseEventWordError {
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
    /// The digits are not a valid event word.
    Decode(DecodeEventWordError),
}

impl ParseEventWordError {
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

impl fmt::Display for ParseEventWordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Whitespace => f.write_str("an event word contains no whitespace"),
            Self::HexPrefix => f.write_str("an event word has no 0x prefix"),
            Self::WrongLength { length } => {
                write!(
                    f,
                    "an event word is 16 hexadecimal digits, not {length} bytes"
                )
            }
            Self::UpperCase => f.write_str("an event word is lower case"),
            Self::InvalidDigit => f.write_str("an event word is hexadecimal digits only"),
            Self::Decode(e) => write!(f, "invalid event word: {e}"),
        }
    }
}

impl Error for ParseEventWordError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::Whitespace
            | Self::HexPrefix
            | Self::WrongLength { .. }
            | Self::UpperCase
            | Self::InvalidDigit => None,
        }
    }
}

impl fmt::Display for EventWord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.raw())
    }
}

impl fmt::LowerHex for EventWord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.raw(), f)
    }
}

impl FromStr for EventWord {
    type Err = ParseEventWordError;

    /// Parses exactly 16 lower-case hexadecimal digits, then validates them with
    /// [`EventWord::from_raw`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw = parse_lower_hex(s, 16).map_err(ParseEventWordError::from_fault)?;
        Self::from_raw(raw).map_err(ParseEventWordError::Decode)
    }
}

/// An [`EventId`] did not parse from its text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseEventIdError {
    /// The text contains whitespace.
    Whitespace,
    /// The text has no colon between the subject and the word.
    MissingSeparator,
    /// The subject is a system and did not parse.
    System(ParseSystemIdError),
    /// The subject is a body and did not parse.
    Body(ParseBodyIdError),
    /// The word did not parse.
    Word(ParseEventWordError),
}

impl fmt::Display for ParseEventIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Whitespace => f.write_str("an event id contains no whitespace"),
            Self::MissingSeparator => f.write_str("an event id is a subject, ':' and a word"),
            Self::System(e) => write!(f, "invalid system in event id: {e}"),
            Self::Body(e) => write!(f, "invalid body in event id: {e}"),
            Self::Word(e) => write!(f, "invalid word in event id: {e}"),
        }
    }
}

impl Error for ParseEventIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::System(e) => Some(e),
            Self::Body(e) => Some(e),
            Self::Word(e) => Some(e),
            Self::Whitespace | Self::MissingSeparator => None,
        }
    }
}

impl fmt::Display for EventSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System(system) => fmt::Display::fmt(system, f),
            Self::Body(body) => fmt::Display::fmt(body, f),
        }
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.subject(), self.word())
    }
}

impl FromStr for EventId {
    type Err = ParseEventIdError;

    /// Parses `<subject>:<16 digits>`, where the subject is a system or, with a full stop, a body.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.chars().any(char::is_whitespace) {
            return Err(ParseEventIdError::Whitespace);
        }
        let (subject, word) = s
            .split_once(':')
            .ok_or(ParseEventIdError::MissingSeparator)?;
        let subject = if subject.contains('.') {
            EventSubject::Body(subject.parse().map_err(ParseEventIdError::Body)?)
        } else {
            EventSubject::System(subject.parse().map_err(ParseEventIdError::System)?)
        };
        let word = word.parse().map_err(ParseEventIdError::Word)?;
        Ok(Self::new(subject, word))
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::id::event_tags::SELF_TEST;
    use crate::id::testing::random_valid_id;
    use crate::id::{EventBin, EventTag};

    const A_ORIGIN: &str = "0200080020000000";

    #[test]
    fn every_kind_round_trips_through_text() {
        let mut lcg = Lcg::new(0x7e87);
        for _ in 0..10_000 {
            let system = random_valid_id(&mut lcg);
            let text = system.to_string();
            assert_eq!(text.len(), 16);
            assert_eq!(text, format!("{system:016x}"));
            assert_eq!(text.parse::<SystemId>(), Ok(system));

            let body = BodyId::new(system, u16::try_from(lcg.next_below(1 << 16)).unwrap());
            assert_eq!(body.to_string().parse::<BodyId>(), Ok(body));

            let k = i64::try_from(lcg.next_below(1 << 40)).unwrap() - (1 << 39);
            let j = u8::try_from(lcg.next_below(256)).unwrap();
            let word = EventWord::new(SELF_TEST, EventBin::new(k).unwrap(), j);
            assert_eq!(word.to_string().parse::<EventWord>(), Ok(word));
            for subject in [EventSubject::System(system), EventSubject::Body(body)] {
                let event = EventId::new(subject, word);
                assert_eq!(event.to_string().parse::<EventId>(), Ok(event));
            }
        }
    }

    #[test]
    fn a_word_above_two_to_the_53_survives_the_text_form() {
        // 2⁵³ + 1 is not representable in an f64, which is why the protocol sends text.
        let id = SystemId::from_raw(0x8000_0000_0000_0001).unwrap();
        assert!(id.raw() > 1 << 53);
        assert_eq!(id.to_string().parse::<SystemId>().unwrap().raw(), id.raw());
        let body: BodyId = "83ffffffffffffff.ffff".parse().unwrap();
        assert_eq!(body.system().raw(), 0x83FF_FFFF_FFFF_FFFF);
        assert_eq!(body.body_index(), 0xFFFF);
    }

    #[test]
    fn system_id_rejections_have_their_own_variants() {
        let parse = |s: &str| s.parse::<SystemId>();
        assert_eq!(
            parse(A_ORIGIN).map(SystemId::raw),
            Ok(0x0200_0800_2000_0000)
        );
        assert_eq!(
            parse("020008002000000A"),
            Err(ParseSystemIdError::UpperCase)
        );
        assert_eq!(
            parse("0x0200080020000000"),
            Err(ParseSystemIdError::HexPrefix)
        );
        assert_eq!(
            parse("0X0200080020000000"),
            Err(ParseSystemIdError::HexPrefix)
        );
        assert_eq!(
            parse(" 0200080020000000"),
            Err(ParseSystemIdError::Whitespace)
        );
        assert_eq!(
            parse("0200080020000000\n"),
            Err(ParseSystemIdError::Whitespace)
        );
        assert_eq!(
            parse("02000800 0000000"),
            Err(ParseSystemIdError::Whitespace)
        );
        assert_eq!(
            parse("020008002000000"),
            Err(ParseSystemIdError::WrongLength { length: 15 })
        );
        assert_eq!(
            parse("02000800200000000"),
            Err(ParseSystemIdError::WrongLength { length: 17 })
        );
        assert_eq!(
            parse(""),
            Err(ParseSystemIdError::WrongLength { length: 0 })
        );
        assert_eq!(
            parse("+200080020000000"),
            Err(ParseSystemIdError::InvalidDigit)
        );
        assert_eq!(
            parse("020008002000000g"),
            Err(ParseSystemIdError::InvalidDigit)
        );
        assert_eq!(
            parse("02000800200000é"),
            Err(ParseSystemIdError::InvalidDigit)
        );
        assert_eq!(
            parse("1c00000000000000"),
            Err(ParseSystemIdError::Decode(
                DecodeSystemIdError::SpareBitsSet
            ))
        );
    }

    #[test]
    fn body_and_event_rejections_have_their_own_variants() {
        let body = |s: &str| s.parse::<BodyId>();
        assert_eq!(body("0200080020000000.0003").map(BodyId::body_index), Ok(3));
        assert_eq!(
            body("0200080020000000"),
            Err(ParseBodyIdError::MissingSeparator)
        );
        assert_eq!(
            body("0200080020000000.003"),
            Err(ParseBodyIdError::WrongLength { length: 3 })
        );
        assert_eq!(
            body("0200080020000000.000A"),
            Err(ParseBodyIdError::UpperCase)
        );
        assert_eq!(
            body("0200080020000000.0x03"),
            Err(ParseBodyIdError::HexPrefix)
        );
        assert_eq!(
            body("0200080020000000.00-3"),
            Err(ParseBodyIdError::InvalidDigit)
        );
        assert_eq!(
            body("0200080020000000. 003"),
            Err(ParseBodyIdError::Whitespace)
        );
        assert_eq!(
            body("020008002000000.0003"),
            Err(ParseBodyIdError::System(ParseSystemIdError::WrongLength {
                length: 15
            }))
        );
        assert_eq!(
            body("0200080020000000.0003.0001"),
            Err(ParseBodyIdError::WrongLength { length: 9 })
        );

        let word = |s: &str| s.parse::<EventWord>();
        assert_eq!(word("0001ffffffffff00").map(|w| w.bin().get()), Ok(-1));
        assert_eq!(
            word("0001FFFFFFFFFF00"),
            Err(ParseEventWordError::UpperCase)
        );
        assert_eq!(
            word("0x01ffffffffff00"),
            Err(ParseEventWordError::HexPrefix)
        );
        assert_eq!(
            word("0000ffffffffff00"),
            Err(ParseEventWordError::Decode(DecodeEventWordError::ZeroTag))
        );
        assert_eq!(
            word("0002000000000000"),
            Err(ParseEventWordError::Decode(
                DecodeEventWordError::UnregisteredTag { number: 2 }
            ))
        );
        assert_eq!(
            word("0001ff"),
            Err(ParseEventWordError::WrongLength { length: 6 })
        );

        let event = |s: &str| s.parse::<EventId>();
        let good = format!("{A_ORIGIN}.0001:0001000000000000");
        assert_eq!(
            event(&good).map(|e| e.word().tag()),
            Ok(EventTag::from_number(1).unwrap())
        );
        assert_eq!(event(A_ORIGIN), Err(ParseEventIdError::MissingSeparator));
        assert_eq!(
            event(&format!("{A_ORIGIN}: 001000000000000")),
            Err(ParseEventIdError::Whitespace)
        );
        assert_eq!(
            event(&format!("{A_ORIGIN}:000100000000000:")),
            Err(ParseEventIdError::Word(ParseEventWordError::InvalidDigit))
        );
        assert_eq!(
            event(&format!("{A_ORIGIN}.1:0001000000000000")),
            Err(ParseEventIdError::Body(ParseBodyIdError::WrongLength {
                length: 1
            }))
        );
        assert_eq!(
            event("1c00000000000000:0001000000000000"),
            Err(ParseEventIdError::System(ParseSystemIdError::Decode(
                DecodeSystemIdError::SpareBitsSet
            )))
        );
        assert_eq!(
            event(&format!("{A_ORIGIN}:0001000000000000x")),
            Err(ParseEventIdError::Word(ParseEventWordError::WrongLength {
                length: 17
            }))
        );
    }
}
