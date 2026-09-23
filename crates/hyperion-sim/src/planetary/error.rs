//! The errors of plan 14's body addressing: building a [`BodyIndex`] from its parts, decoding one
//! from its 16 bits, and resolving a body of a system (P14.T1.b).
//!
//! [`BodyIndex`]: super::BodyIndex

use std::error::Error;
use std::fmt;

use super::index::{BodySlot, BodySub};
use crate::galaxy::placement::ResolveSystemError;

/// A [`BodyIndex`](super::BodyIndex) could not be built from a slot and a sub-index: the pair is
/// not in plan 14's layout (design note 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncodeBodyIndexError {
    /// The slot's number is outside its block: a planet is numbered 1–191, a second-generation
    /// planet and a belt 0–15.
    SlotOutOfRange(BodySlot),
    /// The sub-index's number is outside its block: a component is numbered 1–15, a moon 1–127,
    /// a ring 0–15 and a belt member 1–255.
    SubOutOfRange(BodySub),
    /// The kind of sub-index does not occur in the kind of slot: components belong to the
    /// stellar level, moons to planets, rings to planets and to a free-floating object, members
    /// to belts.
    SubNotInSlot {
        /// The slot.
        slot: BodySlot,
        /// The sub-index it cannot hold.
        sub: BodySub,
    },
}

impl fmt::Display for EncodeBodyIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SlotOutOfRange(slot) => write!(f, "{slot} is outside its block of slots"),
            Self::SubOutOfRange(sub) => write!(f, "{sub} is outside its block of sub-indices"),
            Self::SubNotInSlot { slot, sub } => write!(f, "{slot} holds no {sub}"),
        }
    }
}

impl Error for EncodeBodyIndexError {}

/// Sixteen bits are not a body index of plan 14's layout (design note 3).
///
/// Plan 01's [`BodyId`](crate::id::BodyId) accepts any `u16`; what the values mean, and which are
/// unused, is plan 14's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodeBodyIndexError {
    /// The high byte is a reserved slot: `0xD0`–`0xDF` or `0xF0`–`0xFF`.
    ReservedSlot {
        /// The raw index.
        raw: u16,
    },
    /// The low byte is a sub-index its slot reserves: `0x10`–`0x7F` or `0x90`–`0xFF` of the
    /// stellar level, or `0x90`–`0xFF` of a planet.
    ReservedSub {
        /// The raw index.
        raw: u16,
    },
}

impl fmt::Display for DecodeBodyIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedSlot { raw } => {
                write!(f, "body index {raw:04x} is in a reserved slot")
            }
            Self::ReservedSub { raw } => {
                write!(f, "body index {raw:04x} is a sub-index its slot reserves")
            }
        }
    }
}

impl Error for DecodeBodyIndexError {}

/// A body of a system could not be resolved: the system does not exist, its index is not in the
/// layout, or the system has no such body.
///
/// A body ID that arrives from outside the sim (a save, the protocol, a designation typed by a
/// player) goes through this, because nothing outside can promise that it names a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolveBodyError {
    /// The body's system does not resolve (plan 03).
    NoSuchSystem(ResolveSystemError),
    /// The index decodes, but the system holds no body there.
    NoSuchBody,
    /// The index is not in plan 14's layout.
    MalformedIndex(DecodeBodyIndexError),
}

impl fmt::Display for ResolveBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchSystem(e) => write!(f, "the body's system does not resolve: {e}"),
            Self::NoSuchBody => f.write_str("the system holds no such body"),
            Self::MalformedIndex(e) => write!(f, "the body index is malformed: {e}"),
        }
    }
}

impl Error for ResolveBodyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NoSuchSystem(e) => Some(e),
            Self::MalformedIndex(e) => Some(e),
            Self::NoSuchBody => None,
        }
    }
}

impl From<DecodeBodyIndexError> for ResolveBodyError {
    fn from(e: DecodeBodyIndexError) -> Self {
        Self::MalformedIndex(e)
    }
}

impl From<ResolveSystemError> for ResolveBodyError {
    fn from(e: ResolveSystemError) -> Self {
        Self::NoSuchSystem(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Layer;

    /// Lower case at the start (numbers and hexadecimal digits aside) and no full stop at the end.
    fn assert_rust_error_style(text: &str) {
        let first = text.chars().next().expect("a message");
        assert!(!first.is_uppercase(), "{text:?} starts with a capital");
        assert!(!text.ends_with('.'), "{text:?} ends with a full stop");
        assert!(!text.is_empty());
    }

    #[test]
    fn every_error_message_is_lower_case_without_a_full_stop() {
        let encode = [
            EncodeBodyIndexError::SlotOutOfRange(BodySlot::Planet(0)),
            EncodeBodyIndexError::SubOutOfRange(BodySub::Moon(200)),
            EncodeBodyIndexError::SubNotInSlot {
                slot: BodySlot::Belt(0),
                sub: BodySub::Ring(0),
            },
        ];
        let decode = [
            DecodeBodyIndexError::ReservedSlot { raw: 0xd000 },
            DecodeBodyIndexError::ReservedSub { raw: 0x0190 },
        ];
        let resolve = [
            ResolveBodyError::NoSuchSystem(ResolveSystemError::NoSuchSystem),
            ResolveBodyError::NoSuchSystem(ResolveSystemError::LayerNotGenerated(
                Layer::BrownDwarf,
            )),
            ResolveBodyError::NoSuchBody,
            ResolveBodyError::MalformedIndex(DecodeBodyIndexError::ReservedSlot { raw: 0xf123 }),
        ];
        let texts: Vec<String> = encode
            .iter()
            .map(ToString::to_string)
            .chain(decode.iter().map(ToString::to_string))
            .chain(resolve.iter().map(ToString::to_string))
            .collect();
        for text in &texts {
            assert_rust_error_style(text);
        }
        assert_eq!(
            DecodeBodyIndexError::ReservedSlot { raw: 0xd000 }.to_string(),
            "body index d000 is in a reserved slot"
        );
        assert_eq!(
            encode[0].to_string(),
            "planet 0 is outside its block of slots"
        );
        assert_eq!(encode[2].to_string(), "belt 0 holds no ring 0");
        assert_eq!(
            ResolveBodyError::NoSuchBody.to_string(),
            "the system holds no such body"
        );
    }

    #[test]
    fn a_resolve_error_names_its_cause() {
        let decode = DecodeBodyIndexError::ReservedSub { raw: 0x0190 };
        let wrapped = ResolveBodyError::from(decode);
        assert_eq!(wrapped, ResolveBodyError::MalformedIndex(decode));
        assert_eq!(
            wrapped.source().map(ToString::to_string),
            Some(decode.to_string())
        );
        let system = ResolveBodyError::from(ResolveSystemError::NoSuchSystem);
        assert!(system.source().is_some());
        assert!(ResolveBodyError::NoSuchBody.source().is_none());
    }

    #[test]
    fn errors_are_send_sync_and_static() {
        fn check<E: Error + Send + Sync + 'static>() {}
        check::<EncodeBodyIndexError>();
        check::<DecodeBodyIndexError>();
        check::<ResolveBodyError>();
    }
}
