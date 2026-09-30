//! Wire primitives shared by every message: 64-bit values and body IDs as hexadecimal text, the
//! universe clock and galactic positions.
//!
//! JSON numbers lose integers above 2⁵³ once they reach JavaScript, so every `u64` crosses the wire
//! as exactly 16 lowercase hexadecimal digits, and a body ID as its system's 16 digits, a full stop
//! and its body index as 4 more. Upper case, a `0x` prefix and any other length are refused, so
//! that one value has one string.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The number of hexadecimal digits in the text form of a `u64`.
const HEX64_DIGITS: usize = 16;

/// A 64-bit value's text form was not exactly 16 lowercase hexadecimal digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseHex64Error {
    /// The text is not 16 bytes long.
    WrongLength,
    /// A character is not one of `0`–`9` or `a`–`f`.
    InvalidDigit,
}

impl fmt::Display for ParseHex64Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength | Self::InvalidDigit => {
                f.write_str("expected 16 lowercase hexadecimal digits")
            }
        }
    }
}

impl Error for ParseHex64Error {}

/// Formats `value` as its 16-digit lowercase hexadecimal text form.
fn encode(value: u64) -> String {
    format!("{value:016x}")
}

/// Parses the 16-digit lowercase hexadecimal text form of a `u64`.
fn decode(text: &str) -> Result<u64, ParseHex64Error> {
    if text.len() != HEX64_DIGITS {
        return Err(ParseHex64Error::WrongLength);
    }
    hex_value(text.as_bytes()).ok_or(ParseHex64Error::InvalidDigit)
}

/// The value of lowercase hexadecimal digits, most significant first; `None` if a byte is not one
/// of `0`–`9` or `a`–`f`. The caller bounds the length, at most 16 digits.
#[must_use]
fn hex_value(digits: &[u8]) -> Option<u64> {
    digits.iter().try_fold(0_u64, |value, &byte| {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => return None,
        };
        Some((value << 4) | u64::from(digit))
    })
}

/// Defines a `u64` newtype whose wire form is 16 lowercase hexadecimal digits.
macro_rules! hex64_newtype {
    ($(#[$meta:meta])* $name:ident, $what:literal) => {
        $(#[$meta])*
        ///
        /// On the wire it is exactly 16 lowercase hexadecimal digits. The text is checked whenever
        /// a value is built, so a held value is always well formed.
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
        #[serde(try_from = "String")]
        #[ts(export, type = "string")]
        pub struct $name(String);

        impl $name {
            #[doc = concat!("Encodes ", $what, " for the wire.")]
            ///
            /// # Examples
            ///
            /// ```
            #[doc = concat!("use hyperion_protocol::", stringify!($name), ";")]
            ///
            #[doc = concat!("let hex = ", stringify!($name), "::from_u64(1234);")]
            /// assert_eq!(hex.as_str(), "00000000000004d2");
            /// ```
            #[must_use]
            pub fn from_u64(value: u64) -> Self {
                Self(encode(value))
            }

            #[doc = concat!("Decodes ", $what, " received from the wire.")]
            ///
            /// # Examples
            ///
            /// ```
            #[doc = concat!("use hyperion_protocol::{", stringify!($name), ", ParseHex64Error};")]
            ///
            #[doc = concat!(
                "let hex = ", stringify!($name), "::try_from(\"ffffffffffffffff\".to_owned())?;"
            )]
            /// assert_eq!(hex.to_u64(), u64::MAX);
            /// # Ok::<(), ParseHex64Error>(())
            /// ```
            #[must_use]
            pub fn to_u64(&self) -> u64 {
                decode(&self.0).expect("the text was validated when the value was built")
            }

            /// The text form, exactly as it crosses the wire.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = ParseHex64Error;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                decode(&text)?;
                Ok(Self(text))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

hex64_newtype!(
    /// A universe's seed, the root of everything generated in it.
    SeedHex,
    "a seed"
);

hex64_newtype!(
    /// A universe's identity: a random `u64` drawn by the server when the universe is created,
    /// distinct from its seed so that two saves of one seed stay apart.
    UniverseIdHex,
    "a universe ID"
);

hex64_newtype!(
    /// The seed of a body's surface map: one block output of the universe's seed on `body.surface`,
    /// keyed by the body's ID, which depends on nothing else about the body (plan 14, P14.T23).
    SurfaceSeedHex,
    "a surface seed"
);

hex64_newtype!(
    /// A star system's ID, which also says where to regenerate the system from.
    ///
    /// The server resolves every system ID a client sends before using it, since a well-formed ID
    /// need not name a system.
    SystemIdHex,
    "a system ID"
);

/// The number of hexadecimal digits in the body index of a body ID's text form.
const BODY_INDEX_DIGITS: usize = 4;

/// The number of bytes in a body ID's text form: the system's digits, a full stop, the index's.
const BODY_ID_LEN: usize = HEX64_DIGITS + 1 + BODY_INDEX_DIGITS;

/// A body ID's text form was not a system's 16 lowercase hexadecimal digits, a full stop and 4
/// lowercase hexadecimal digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseBodyIdHexError {
    /// The text is not 21 bytes long.
    WrongLength,
    /// The 17th byte is not a full stop.
    MissingSeparator,
    /// A character of either part is not one of `0`–`9` or `a`–`f`.
    InvalidDigit,
}

impl fmt::Display for ParseBodyIdHexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength | Self::MissingSeparator | Self::InvalidDigit => f.write_str(
                "expected 16 lowercase hexadecimal digits, a full stop and 4 lowercase \
                 hexadecimal digits",
            ),
        }
    }
}

impl Error for ParseBodyIdHexError {}

/// Parses a body ID's text form into its system's value and its body index.
fn decode_body_id(text: &str) -> Result<(u64, u16), ParseBodyIdHexError> {
    let bytes = text.as_bytes();
    if bytes.len() != BODY_ID_LEN {
        return Err(ParseBodyIdHexError::WrongLength);
    }
    let (system, rest) = bytes.split_at(HEX64_DIGITS);
    let Some((&b'.', index)) = rest.split_first() else {
        return Err(ParseBodyIdHexError::MissingSeparator);
    };
    let system = hex_value(system).ok_or(ParseBodyIdHexError::InvalidDigit)?;
    let index = hex_value(index).ok_or(ParseBodyIdHexError::InvalidDigit)?;
    let index = u16::try_from(index).expect("four hexadecimal digits fit in a u16");
    Ok((system, index))
}

/// A body's ID: its system's ID and its 16-bit index within the system, as plan 01's `BodyId`
/// writes it.
///
/// On the wire it is the system's 16 lowercase hexadecimal digits, a full stop and the body index
/// as 4 lowercase hexadecimal digits: `0200080020000000.0100` is body `0x0100`, the first planet,
/// of system `0200080020000000`. The index's layout is plan 14's (its design note 3): stars at
/// `0x0000`–`0x000f`, planets from `0x0100` with their moons and rings under them. The text is
/// checked whenever a value is built, so a held value is always well formed; the server resolves
/// every body ID a client sends before using it, since a well-formed ID need not name a body.
///
/// # Examples
///
/// ```
/// use hyperion_protocol::{BodyIdHex, ParseBodyIdHexError};
///
/// // The first planet of a system, and the same ID read back from the wire.
/// let planet = BodyIdHex::from_parts(0x0200_0800_2000_0000, 0x0100);
/// assert_eq!(planet.as_str(), "0200080020000000.0100");
/// let read = BodyIdHex::try_from("0200080020000000.0100".to_owned())?;
/// assert_eq!(read.to_parts(), (0x0200_0800_2000_0000, 0x0100));
/// # Ok::<(), ParseBodyIdHexError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(try_from = "String")]
#[ts(export, type = "string")]
pub struct BodyIdHex(String);

impl BodyIdHex {
    /// Encodes body `body_index` of the system whose ID is `system` for the wire.
    #[must_use]
    pub fn from_parts(system: u64, body_index: u16) -> Self {
        Self(format!("{system:016x}.{body_index:04x}"))
    }

    /// Decodes the ID into its system's value and its body index.
    ///
    /// # Panics
    ///
    /// Never: the text was checked when the value was built.
    #[must_use]
    pub fn to_parts(&self) -> (u64, u16) {
        decode_body_id(&self.0).expect("the text was validated when the value was built")
    }

    /// The text form, exactly as it crosses the wire.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BodyIdHex {
    type Error = ParseBodyIdHexError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        decode_body_id(&text)?;
        Ok(Self(text))
    }
}

impl fmt::Display for BodyIdHex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An instant on the universe clock: coordinate time in the galaxy's rest frame.
///
/// Zero is the epoch, the instant the galaxy's fields describe. `seconds` may be negative, and
/// `nanos` (0 to 999,999,999) always counts forward from it, so 1.5 s before the epoch is
/// `{"seconds": -2, "nanos": 500000000}`. Every second of the source horizon, 8.3 × 10¹² s either
/// side of the epoch, is far inside the 2⁵³ that a JavaScript number holds exactly, so `seconds` is
/// a JSON number.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize, TS,
)]
#[ts(export)]
pub struct UniverseTime {
    /// Whole seconds since the epoch, rounded towards negative infinity.
    #[ts(type = "number")]
    pub seconds: i64,
    /// Nanoseconds after `seconds`, below 10⁹.
    pub nanos: u32,
}

/// A position in the `GALACTIC` frame, carried exactly as a light-year cell plus a metre offset.
///
/// The origin is the galactic centre, +x runs along the bar's long axis and +z is galactic north,
/// so that the galaxy rotates counter-clockwise seen from the north; +y completes a right-handed
/// set. The point is `cell_ly + offset_m`, where each offset component lies in `[0, 1 ly)`, so one
/// point has one value and the resolution is about 2 m anywhere in the galaxy. A single `f64` of
/// light-years would resolve only 65 km at 50,000 ly.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GalacticPosition {
    /// The 1 ly cell, as whole light-years along x, y and z (the cell's lower corner).
    pub cell_ly: [i32; 3],
    /// Offset from the cell's lower corner in metres along x, y and z, each in `[0, 1 ly)`.
    pub offset_m: [f64; 3],
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::testing::assert_wire_form;

    #[test]
    fn seed_hex_wire_form() {
        let seed = SeedHex::from_u64(1234);
        assert_wire_form(&seed, json!("00000000000004d2"));
        assert_eq!(seed.to_u64(), 1234);
    }

    #[test]
    fn universe_and_system_ids_share_the_hex_wire_form() {
        assert_wire_form(
            &UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
            json!("0123456789abcdef"),
        );
        assert_wire_form(
            &SystemIdHex::from_u64(0x0200_0800_2000_0000),
            json!("0200080020000000"),
        );
    }

    #[test]
    fn a_surface_seed_is_sixteen_hex_digits() {
        let seed = SurfaceSeedHex::from_u64(u64::MAX - 1);
        assert_wire_form(&seed, json!("fffffffffffffffe"));
        assert_eq!(
            SurfaceSeedHex::try_from("FFFFFFFFFFFFFFFE".to_owned()),
            Err(ParseHex64Error::InvalidDigit)
        );
    }

    #[test]
    fn hex64_rejects_uppercase_short_long_and_prefix() {
        let cases = [
            ("00000000000004D2", ParseHex64Error::InvalidDigit),
            ("4d2", ParseHex64Error::WrongLength),
            ("", ParseHex64Error::WrongLength),
            ("000000000000004d2", ParseHex64Error::WrongLength),
            ("0x00000000000004d2", ParseHex64Error::WrongLength),
            ("0x000000000004d2", ParseHex64Error::InvalidDigit),
            (" 0000000000004d2", ParseHex64Error::InvalidDigit),
            ("000000000000004g", ParseHex64Error::InvalidDigit),
            ("-000000000000001", ParseHex64Error::InvalidDigit),
            // Sixteen bytes but eight characters: the length is right, the digits are not.
            ("éééééééé", ParseHex64Error::InvalidDigit),
        ];
        for (text, expected) in cases {
            assert_eq!(
                SeedHex::try_from(text.to_owned()),
                Err(expected),
                "parsing {text:?}"
            );
            let error = serde_json::from_value::<SystemIdHex>(json!(text)).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("expected 16 lowercase hexadecimal digits"),
                "unexpected serde error for {text:?}: {error}"
            );
        }
    }

    #[test]
    fn hex64_rejects_a_json_number() {
        let error = serde_json::from_value::<UniverseIdHex>(json!(1234)).unwrap_err();
        assert!(
            error.to_string().contains("invalid type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hex64_round_trips_zero_and_max() {
        for (value, text) in [
            (0, "0000000000000000"),
            (u64::MAX, "ffffffffffffffff"),
            (1 << 63, "8000000000000000"),
        ] {
            let hex = UniverseIdHex::from_u64(value);
            assert_eq!(hex.as_str(), text);
            assert_eq!(hex.to_string(), text);
            assert_eq!(hex.to_u64(), value);
            assert_eq!(UniverseIdHex::try_from(text.to_owned()), Ok(hex));
        }
    }

    #[test]
    fn hex64_orders_as_its_value() {
        let small = SeedHex::from_u64(0x0f);
        let large = SeedHex::from_u64(0xa0);
        assert!(small < large);
    }

    #[test]
    fn parse_hex64_error_text() {
        assert_eq!(
            ParseHex64Error::WrongLength.to_string(),
            "expected 16 lowercase hexadecimal digits"
        );
        assert_eq!(
            ParseHex64Error::InvalidDigit.to_string(),
            "expected 16 lowercase hexadecimal digits"
        );
    }

    #[test]
    fn body_id_hex_wire_form() {
        let planet = BodyIdHex::from_parts(0x0200_0800_2000_0000, 0x0100);
        assert_wire_form(&planet, json!("0200080020000000.0100"));
        assert_eq!(planet.to_string(), "0200080020000000.0100");
    }

    #[test]
    fn body_id_hex_round_trips_every_extreme() {
        for (system, index, text) in [
            (0, 0, "0000000000000000.0000"),
            (u64::MAX, u16::MAX, "ffffffffffffffff.ffff"),
            (1 << 63, 1 << 15, "8000000000000000.8000"),
            (0x0123_4567_89ab_cdef, 0x0a0f, "0123456789abcdef.0a0f"),
        ] {
            let id = BodyIdHex::from_parts(system, index);
            assert_eq!(id.as_str(), text);
            assert_eq!(id.to_parts(), (system, index));
            assert_eq!(BodyIdHex::try_from(text.to_owned()), Ok(id));
        }
    }

    #[test]
    fn body_id_hex_round_trips_every_body_index() {
        let system = 0x0200_0800_2000_0000;
        for index in 0..=u16::MAX {
            let id = BodyIdHex::from_parts(system, index);
            let read = BodyIdHex::try_from(id.as_str().to_owned()).unwrap();
            assert_eq!(read.to_parts(), (system, index));
        }
    }

    #[test]
    fn body_id_hex_rejects_every_malformed_form() {
        let cases = [
            ("", ParseBodyIdHexError::WrongLength),
            ("0200080020000000", ParseBodyIdHexError::WrongLength),
            ("0200080020000000.", ParseBodyIdHexError::WrongLength),
            ("0200080020000000.100", ParseBodyIdHexError::WrongLength),
            ("0200080020000000.00100", ParseBodyIdHexError::WrongLength),
            ("200080020000000.0100", ParseBodyIdHexError::WrongLength),
            (
                "0200080020000000:0100",
                ParseBodyIdHexError::MissingSeparator,
            ),
            (
                "02000800200000000.100",
                ParseBodyIdHexError::MissingSeparator,
            ),
            ("0200080020000000.010A", ParseBodyIdHexError::InvalidDigit),
            ("020008002000000A.0100", ParseBodyIdHexError::InvalidDigit),
            ("0200080020000000.0x10", ParseBodyIdHexError::InvalidDigit),
            ("0x00080020000000.0100", ParseBodyIdHexError::InvalidDigit),
            ("0200080020000000. 100", ParseBodyIdHexError::InvalidDigit),
            ("0200080020000000.-100", ParseBodyIdHexError::InvalidDigit),
            ("020008002000000g.0100", ParseBodyIdHexError::InvalidDigit),
            // Twenty-one bytes, but the index is two characters of two bytes each.
            ("0200080020000000.éé", ParseBodyIdHexError::InvalidDigit),
            // Twenty-one bytes, with the separator's byte inside a two-byte character.
            (
                "020008002000000é.010",
                ParseBodyIdHexError::MissingSeparator,
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(
                BodyIdHex::try_from(text.to_owned()),
                Err(expected),
                "parsing {text:?}"
            );
            let error = serde_json::from_value::<BodyIdHex>(json!(text)).unwrap_err();
            assert!(
                error.to_string().contains(
                    "expected 16 lowercase hexadecimal digits, a full stop and 4 lowercase \
                     hexadecimal digits"
                ),
                "unexpected serde error for {text:?}: {error}"
            );
        }
    }

    #[test]
    fn body_id_hex_rejects_a_json_number() {
        let error = serde_json::from_value::<BodyIdHex>(json!(256)).unwrap_err();
        assert!(
            error.to_string().contains("invalid type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn body_id_hex_orders_by_system_then_index() {
        let first = BodyIdHex::from_parts(1, u16::MAX);
        let second = BodyIdHex::from_parts(2, 0);
        assert!(first < second);
        assert!(BodyIdHex::from_parts(1, 0x00ff) < BodyIdHex::from_parts(1, 0x0100));
    }

    #[test]
    fn parse_body_id_hex_error_text() {
        for error in [
            ParseBodyIdHexError::WrongLength,
            ParseBodyIdHexError::MissingSeparator,
            ParseBodyIdHexError::InvalidDigit,
        ] {
            assert_eq!(
                error.to_string(),
                "expected 16 lowercase hexadecimal digits, a full stop and 4 lowercase hexadecimal \
                 digits"
            );
        }
    }

    #[test]
    fn universe_time_wire_form() {
        // 1.5 s before the epoch: the seconds round down and the nanoseconds count forward.
        assert_wire_form(
            &UniverseTime {
                seconds: -2,
                nanos: 500_000_000,
            },
            json!({ "seconds": -2, "nanos": 500_000_000 }),
        );
        assert_wire_form(
            &UniverseTime::default(),
            json!({ "seconds": 0, "nanos": 0 }),
        );
    }

    #[test]
    fn universe_time_keeps_the_source_horizon_exact() {
        // −(H + L) = −(1,000 + 2¹⁸) Julian years, the earliest emission time a query can reach.
        let seconds = -(1_000 + (1 << 18)) * 31_557_600;
        assert_wire_form(
            &UniverseTime { seconds, nanos: 1 },
            json!({ "seconds": -8_304_193_094_400_i64, "nanos": 1 }),
        );
    }

    /// Ruling 64.7 of 2026-09-22: a float the client writes with seventeen significant digits is
    /// read back exactly. `558138600491200.44` m is the shortest text of its `f64`, and
    /// `serde_json` without its `float_roundtrip` feature, which the workspace turns on, reads it
    /// one ulp high.
    #[test]
    fn a_seventeen_digit_float_is_read_exactly() {
        let sent = 558_138_600_491_200.44_f64;
        let read: GalacticPosition = serde_json::from_str(&format!(
            r#"{{"cell_ly": [0, 26000, 0], "offset_m": [{sent:?}, 0.5, 0.25]}}"#
        ))
        .unwrap();
        assert_eq!(read.offset_m[0].to_bits(), sent.to_bits());
        assert_eq!(serde_json::to_string(&sent).unwrap(), "558138600491200.44");
    }

    #[test]
    fn galactic_position_wire_form() {
        assert_wire_form(
            &GalacticPosition {
                cell_ly: [26_000, -1, 0],
                offset_m: [0.0, 9.0e15, 4_730_365_236_290_400.0],
            },
            json!({
                "cell_ly": [26_000, -1, 0],
                "offset_m": [0.0, 9.0e15, 4_730_365_236_290_400.0],
            }),
        );
    }
}
