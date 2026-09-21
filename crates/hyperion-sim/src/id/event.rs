//! Events: the event word (tag, bin or cycle number, number in the bin) and event IDs.
//!
//! An event is the ID of its system or body plus a 64-bit word (brainstorm, "Identifiers"):
//!
//! | Field | Bits      | Width | Notes                                                                  |
//! | ----- | --------- | ----- | ---------------------------------------------------------------------- |
//! | Tag   | `[63:48]` | 16    | From the registry, [`event_tags`](super::event_tags); 0 is never valid |
//! | k     | `[47:8]`  | 40    | Bin or cycle number, two's complement, −2³⁹ to 2³⁹ − 1                 |
//! | j     | `[7:0]`   | 8     | Number within the bin                                                  |
//!
//! The bin's contents are a pure function of (seed, subject, tag, k), so an [`EventId`] names one
//! event exactly and can be regenerated on its own.

use std::error::Error;
use std::fmt;

use super::bits::{Field, assert_tiles_64};
use super::body::BodyId;
use super::system::SystemId;

/// The event tag, `[63:48]`.
const TAG: Field = Field::new(63, 48);
/// The bin or cycle number, `[47:8]`.
const BIN: Field = Field::new(47, 8);
/// The number within the bin, `[7:0]`.
const NUMBER: Field = Field::new(7, 0);

const _: () = assert_tiles_64(&[TAG, BIN, NUMBER]);

/// A registered event tag: the kind of an event, a 16-bit number from the registry in
/// [`event_tags`](super::event_tags).
///
/// Tags exist only as that registry's constants and through [`from_number`](Self::from_number),
/// so every `EventTag` is registered. Numbers are never reused, and 0 is never valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventTag(u16);

impl EventTag {
    /// A registry entry. Only `event_tags!` calls this.
    #[must_use]
    pub(super) const fn registered(number: u16) -> Self {
        Self(number)
    }

    /// The tag's number.
    #[must_use]
    pub const fn number(self) -> u16 {
        self.0
    }
}

/// An [`EventBin`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildEventBinError {
    /// The value is outside the signed 40-bit range, −2³⁹ to 2³⁹ − 1.
    OutOfRange {
        /// The offending value.
        value: i64,
    },
}

impl fmt::Display for BuildEventBinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange { value } => {
                write!(f, "event bin {value} is outside the signed 40-bit range")
            }
        }
    }
}

impl Error for BuildEventBinError {}

/// A bin or cycle number: a signed 40-bit integer, −2³⁹ to 2³⁹ − 1.
///
/// Bins run both ways from the epoch, so a bin before it is negative. Forty bits hold a bin of a
/// day over the whole source horizon many times over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventBin(i64);

impl EventBin {
    /// The smallest bin, −2³⁹.
    pub const MIN: Self = Self(-(1 << 39));
    /// The largest bin, 2³⁹ − 1.
    pub const MAX: Self = Self((1 << 39) - 1);

    /// The bin with this number.
    ///
    /// # Errors
    ///
    /// [`BuildEventBinError::OutOfRange`] outside −2³⁹ to 2³⁹ − 1.
    pub const fn new(value: i64) -> Result<Self, BuildEventBinError> {
        if value < Self::MIN.0 || value > Self::MAX.0 {
            return Err(BuildEventBinError::OutOfRange { value });
        }
        Ok(Self(value))
    }

    /// The bin from the low 40 bits of `field`, sign-extended from bit 39. Higher bits are ignored.
    #[must_use]
    pub const fn from_field(field: u64) -> Self {
        Self((field << 24).cast_signed() >> 24)
    }

    /// The bin as a 40-bit two's-complement field.
    #[must_use]
    pub const fn to_field(self) -> u64 {
        self.0.cast_unsigned() & BIN.max()
    }

    /// The bin's number.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// An event word does not decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodeEventWordError {
    /// The tag field is 0, which is never valid.
    ZeroTag,
    /// The tag field is not a registered event tag.
    UnregisteredTag {
        /// The tag's number.
        number: u16,
    },
}

impl fmt::Display for DecodeEventWordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTag => f.write_str("event tag 0 is never valid"),
            Self::UnregisteredTag { number } => {
                write!(f, "event tag {number:#06x} is not registered")
            }
        }
    }
}

impl Error for DecodeEventWordError {}

/// The 64-bit word that names an event of a system or body: tag, bin and number in the bin.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{EventBin, EventWord, event_tags};
///
/// let word = EventWord::new(event_tags::SELF_TEST, EventBin::new(-1)?, 3);
/// assert_eq!(word.raw(), 0x0001_FFFF_FFFF_FF03);
/// assert_eq!(word.bin().get(), -1);
/// assert_eq!(EventWord::from_raw(word.raw()), Ok(word));
/// # Ok::<(), hyperion_sim::id::BuildEventBinError>(())
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventWord(u64);

impl EventWord {
    /// Event `number` of bin `bin` under `tag`.
    #[must_use]
    pub fn new(tag: EventTag, bin: EventBin, number: u8) -> Self {
        let raw = TAG.with(0, u64::from(tag.number()));
        let raw = BIN.with(raw, bin.to_field());
        Self(NUMBER.with(raw, u64::from(number)))
    }

    /// Decodes a raw word.
    ///
    /// # Errors
    ///
    /// [`DecodeEventWordError::ZeroTag`] for tag 0; [`DecodeEventWordError::UnregisteredTag`] for
    /// a tag the registry does not hold.
    pub fn from_raw(raw: u64) -> Result<Self, DecodeEventWordError> {
        let number = TAG.get_u16(raw);
        if number == 0 {
            return Err(DecodeEventWordError::ZeroTag);
        }
        match EventTag::from_number(number) {
            Some(_) => Ok(Self(raw)),
            None => Err(DecodeEventWordError::UnregisteredTag { number }),
        }
    }

    /// The raw word.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// The event tag.
    #[must_use]
    pub const fn tag(self) -> EventTag {
        EventTag::registered(TAG.get_u16(self.0))
    }

    /// The bin or cycle number.
    #[must_use]
    pub const fn bin(self) -> EventBin {
        EventBin::from_field(BIN.get(self.0))
    }

    /// The number within the bin.
    #[must_use]
    pub const fn number(self) -> u8 {
        NUMBER.get_u8(self.0)
    }
}

impl fmt::Debug for EventWord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EventWord({:#018x})", self.0)
    }
}

/// What an event happens to: a system or one of its bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventSubject {
    /// An event of a system as a whole, or of a member such as the central black hole.
    System(SystemId),
    /// An event of one body.
    Body(BodyId),
}

impl EventSubject {
    /// The subject's system.
    #[must_use]
    pub const fn system(self) -> SystemId {
        match self {
            Self::System(system) => system,
            Self::Body(body) => body.system(),
        }
    }
}

impl From<SystemId> for EventSubject {
    fn from(system: SystemId) -> Self {
        Self::System(system)
    }
}

impl From<BodyId> for EventSubject {
    fn from(body: BodyId) -> Self {
        Self::Body(body)
    }
}

/// An event: its subject and its word. The text form is `<subject>:<16 hexadecimal digits>`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{EventBin, EventId, EventWord, SystemId, event_tags};
///
/// let host = SystemId::from_raw(0xF000_0007_0000_0000)?;
/// let word = EventWord::new(event_tags::SELF_TEST, EventBin::new(12)?, 0);
/// let event = EventId::new(host.into(), word);
/// assert_eq!(event.to_string(), "f000000700000000:0001000000000c00");
/// assert_eq!(event.to_string().parse::<EventId>()?, event);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId {
    subject: EventSubject,
    word: EventWord,
}

impl EventId {
    /// The event of `subject` named by `word`.
    #[must_use]
    pub const fn new(subject: EventSubject, word: EventWord) -> Self {
        Self { subject, word }
    }

    /// The subject.
    #[must_use]
    pub const fn subject(self) -> EventSubject {
        self.subject
    }

    /// The word.
    #[must_use]
    pub const fn word(self) -> EventWord {
        self.word
    }
}

#[cfg(test)]
mod tests {
    use super::super::event_tags::SELF_TEST;
    use super::*;

    #[test]
    fn minus_one_is_forty_ones_and_decodes_back() {
        let word = EventWord::new(SELF_TEST, EventBin::new(-1).unwrap(), 0);
        assert_eq!(BIN.get(word.raw()), (1 << 40) - 1);
        assert_eq!(word.raw(), 0x0001_FFFF_FFFF_FF00);
        assert_eq!(word.bin().get(), -1);
    }

    #[test]
    fn the_signed_40_bit_boundaries_hold() {
        let min = -(1_i64 << 39);
        let max = (1_i64 << 39) - 1;
        assert_eq!(EventBin::new(min), Ok(EventBin::MIN));
        assert_eq!(EventBin::new(max), Ok(EventBin::MAX));
        assert_eq!(
            EventBin::new(min - 1),
            Err(BuildEventBinError::OutOfRange { value: min - 1 })
        );
        assert_eq!(
            EventBin::new(max + 1),
            Err(BuildEventBinError::OutOfRange { value: max + 1 })
        );
        assert_eq!(EventBin::MIN.to_field(), 1 << 39);
        assert_eq!(EventBin::MAX.to_field(), (1 << 39) - 1);
        for k in [min, min + 1, -2, -1, 0, 1, max - 1, max] {
            let bin = EventBin::new(k).unwrap();
            assert_eq!(EventBin::from_field(bin.to_field()), bin);
            let word = EventWord::new(SELF_TEST, bin, 255);
            assert_eq!(
                (word.tag(), word.bin(), word.number()),
                (SELF_TEST, bin, 255)
            );
            assert_eq!(EventWord::from_raw(word.raw()), Ok(word));
        }
        assert_eq!(EventBin::from_field(u64::MAX).get(), -1);
    }

    #[test]
    fn zero_and_unregistered_tags_are_rejected_with_distinct_variants() {
        assert_eq!(
            EventWord::from_raw(0x0000_0000_0000_0001),
            Err(DecodeEventWordError::ZeroTag)
        );
        assert_eq!(
            EventWord::from_raw(0xFFFF_0000_0000_0000),
            Err(DecodeEventWordError::UnregisteredTag { number: 0xFFFF })
        );
        assert_eq!(
            EventWord::from_raw(0x0002_0000_0000_0000),
            Err(DecodeEventWordError::UnregisteredTag { number: 2 })
        );
        assert!(EventWord::from_raw(0x0001_8000_0000_0000).is_ok());
    }

    #[test]
    fn subjects_know_their_system() {
        let system = SystemId::from_raw(0xF800_0000_0000_002A).unwrap();
        let body = BodyId::new(system, 7);
        assert_eq!(EventSubject::from(system).system(), system);
        assert_eq!(EventSubject::from(body).system(), system);
        let word = EventWord::new(SELF_TEST, EventBin::new(5).unwrap(), 1);
        let id = EventId::new(body.into(), word);
        assert_eq!((id.subject(), id.word()), (EventSubject::Body(body), word));
    }
}
