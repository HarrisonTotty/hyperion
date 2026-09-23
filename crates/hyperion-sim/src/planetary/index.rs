//! The layout of a body index: what the 16 bits of plan 01's [`BodyId`] mean (plan 14, design
//! note 3, P14.T1.c).
//!
//! The index is `slot << 8 | sub`, and it decodes without generating anything:
//!
//! | Slot          | Meaning                               | Sub |
//! | ------------- | ------------------------------------- | --- |
//! | `0x00`        | The stellar level                     | `0x00` the primary, `0x01`–`0x0F` plan 11's other components; in a free-floating brown dwarf's or rogue planet's system `0x00` is the object and `0x80`–`0x8F` are its rings |
//! | `0x01`–`0xBF` | Primordial planets, in generation order | `0x00` the planet, `0x01`–`0x7F` moons, `0x80`–`0x8F` rings |
//! | `0xC0`–`0xCF` | Second-generation planets             | as for planets |
//! | `0xE0`–`0xEF` | Belts, discs and the cometary halo     | `0x00` the population, `0x01`–`0xFF` its named members |
//! | other         | Reserved                              | |
//!
//! Every other value is rejected: slots `0xD0`–`0xDF` and `0xF0`–`0xFF`, and the sub-indices a
//! slot reserves (`0x10`–`0x7F` and `0x90`–`0xFF` of the stellar level, `0x90`–`0xFF` of a planet),
//! so 33,936 of the 65,536 values are bodies. Plan 01's [`BodyId`] accepts any `u16`.
//!
//! Slot `0x00` is plan 06's "index 0 is the primary" and plan 11's stars in hierarchy order at
//! body indices 0–15 (its `STAR_BODY_INDEX_END` = 16, plan 11 design note 5): plan 14 numbers
//! nothing there and generates no stellar-level body. Plan 13 puts a free-floating object at body
//! `0x0000`, its rings at sub `0x80` upward of the stellar level, and a rogue planet's moons in the
//! planet-level slots `0x01` upward (`0x0100`, `0x0200`, …), since the object is its system's
//! stellar level. Generation order is host by host in hierarchy order, then inside out, so an
//! index never depends on a later stage. The whole `u16` is the `sub` field of plan 01's second
//! counter word, so every body has streams of its own, apart from its system's.
//!
//! [`BodyId`]: crate::id::BodyId

use std::fmt;

use super::error::{DecodeBodyIndexError, EncodeBodyIndexError};
use crate::id::{BodyId, SystemId};

/// The end of the stellar level's component block: components are sub-indices 0–15 of slot
/// `0x00`, which is plan 11's `STAR_BODY_INDEX_END` = 16 (plan 11, design note 5).
pub const STELLAR_SUB_END: u8 = 16;

/// The last primordial planet slot, `0xBF`: planets are slots 1–191.
pub const LAST_PLANET_SLOT: u8 = 0xBF;

/// The first second-generation planet slot, `0xC0`; there are sixteen.
pub const SECOND_GENERATION_SLOT_START: u8 = 0xC0;

/// The first belt slot, `0xE0`; there are sixteen.
pub const BELT_SLOT_START: u8 = 0xE0;

/// Slots in the second-generation block and in the belt block, and rings of a planet or of a
/// free-floating object.
pub const BLOCK_LEN: u8 = 16;

/// The last moon sub-index of a planet, `0x7F`: moons are sub-indices 1–127.
pub const LAST_MOON_SUB: u8 = 0x7F;

/// The first ring sub-index, `0x80`, of a planet or of a free-floating object.
pub const RING_SUB_START: u8 = 0x80;

/// The high byte of a body index: which level of the system it belongs to.
///
/// The variants are declared, and so order, as their slots do. Each variant carries the body's
/// number in its block: a planet is `Planet(n)` in slot n (1–191, in generation order), a
/// second-generation planet `SecondGeneration(n)` in slot `0xC0` + n and a belt `Belt(n)` in slot
/// `0xE0` + n (0–15 both).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BodySlot {
    /// Slot `0x00`: the stars and brown-dwarf components of plan 11, or a free-floating object.
    Stellar,
    /// A primordial planet, slots 1–191.
    Planet(u8),
    /// A second-generation planet (plan 14, design note 11), slots `0xC0`–`0xCF`.
    SecondGeneration(u8),
    /// A belt, a disc or the cometary halo, slots `0xE0`–`0xEF`.
    Belt(u8),
}

impl fmt::Display for BodySlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stellar => f.write_str("the stellar level"),
            Self::Planet(n) => write!(f, "planet {n}"),
            Self::Belt(n) => write!(f, "belt {n}"),
            Self::SecondGeneration(n) => write!(f, "second-generation planet {n}"),
        }
    }
}

/// The low byte of a body index: which body of its slot.
///
/// Each variant carries the body's number in its block: `Component(n)`, `Moon(n)` and
/// `Member(n)` are sub-index n (1–15, 1–127 and 1–255), and `Ring(n)` is sub-index `0x80` + n
/// (0–15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BodySub {
    /// Sub-index 0: the slot's own body. The primary star or the free-floating object of the
    /// stellar level, the planet itself, or a belt's population.
    Primary,
    /// A component of the stellar level other than the primary, 1–15 in plan 11's hierarchy
    /// order.
    Component(u8),
    /// A moon of a planet, 1–127.
    Moon(u8),
    /// A ring of a planet, or of a free-floating object at the stellar level, 0–15.
    Ring(u8),
    /// A named member of a belt, 1–255.
    Member(u8),
}

impl fmt::Display for BodySub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primary => f.write_str("primary"),
            Self::Component(n) => write!(f, "component {n}"),
            Self::Moon(n) => write!(f, "moon {n}"),
            Self::Ring(n) => write!(f, "ring {n}"),
            Self::Member(n) => write!(f, "member {n}"),
        }
    }
}

/// A body index in plan 14's layout: `slot << 8 | sub` (design note 3).
///
/// Every value of the type is canonical: it is built by [`BodyIndex::new`] or checked by
/// [`TryFrom<u16>`], so [`slot`](Self::slot) and [`sub`](Self::sub) cannot fail. Indices order by
/// their raw value, which is the order a system lists its bodies in: the stellar level, the planets
/// in generation order each followed by its moons and rings, the second-generation planets, then
/// the belts.
///
/// # Examples
///
/// A body ID read from a save is checked once, and its parts read without generating anything:
///
/// ```
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub, DecodeBodyIndexError};
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let saved = BodyId::new(system, 0x0203);
///
/// let index = BodyIndex::try_from(saved.body_index())?;
/// assert_eq!((index.slot(), index.sub()), (BodySlot::Planet(2), BodySub::Moon(3)));
/// // A moon's parent is its planet.
/// assert_eq!(index.parent(), Some(BodyIndex::new(BodySlot::Planet(2), BodySub::Primary)?));
/// assert_eq!(index.body_id(system), saved);
///
/// // 0xD0 is a reserved slot.
/// assert_eq!(
///     BodyIndex::try_from(0xd001),
///     Err(DecodeBodyIndexError::ReservedSlot { raw: 0xd001 })
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyIndex(u16);

impl BodyIndex {
    /// Body `0x0000`: a system's primary star (plan 06), or a free-floating object (plan 13).
    pub const PRIMARY: Self = Self(0);

    /// The index of `sub` in `slot`.
    ///
    /// # Errors
    ///
    /// - [`EncodeBodyIndexError::SlotOutOfRange`] if the slot's number is outside its block.
    /// - [`EncodeBodyIndexError::SubNotInSlot`] if the kind of sub-index does not occur in the
    ///   kind of slot, such as a moon of the stellar level or a ring of a belt.
    /// - [`EncodeBodyIndexError::SubOutOfRange`] if the sub-index's number is outside its block.
    pub fn new(slot: BodySlot, sub: BodySub) -> Result<Self, EncodeBodyIndexError> {
        let slot_byte = match slot {
            BodySlot::Stellar => Some(0),
            BodySlot::Planet(n) => (1..=LAST_PLANET_SLOT).contains(&n).then_some(n),
            BodySlot::SecondGeneration(n) => {
                (n < BLOCK_LEN).then(|| SECOND_GENERATION_SLOT_START + n)
            }
            BodySlot::Belt(n) => (n < BLOCK_LEN).then(|| BELT_SLOT_START + n),
        }
        .ok_or(EncodeBodyIndexError::SlotOutOfRange(slot))?;
        let allowed = match sub {
            BodySub::Primary => true,
            BodySub::Component(_) => matches!(slot, BodySlot::Stellar),
            BodySub::Moon(_) => matches!(slot, BodySlot::Planet(_) | BodySlot::SecondGeneration(_)),
            BodySub::Ring(_) => !matches!(slot, BodySlot::Belt(_)),
            BodySub::Member(_) => matches!(slot, BodySlot::Belt(_)),
        };
        if !allowed {
            return Err(EncodeBodyIndexError::SubNotInSlot { slot, sub });
        }
        let sub_byte = match sub {
            BodySub::Primary => Some(0),
            BodySub::Component(n) => (1..STELLAR_SUB_END).contains(&n).then_some(n),
            BodySub::Moon(n) => (1..=LAST_MOON_SUB).contains(&n).then_some(n),
            BodySub::Ring(n) => (n < BLOCK_LEN).then(|| RING_SUB_START + n),
            BodySub::Member(n) => (n >= 1).then_some(n),
        }
        .ok_or(EncodeBodyIndexError::SubOutOfRange(sub))?;
        Ok(Self(u16::from_be_bytes([slot_byte, sub_byte])))
    }

    /// The slot and sub-index that `raw` encodes.
    ///
    /// # Errors
    ///
    /// - [`DecodeBodyIndexError::ReservedSlot`] if the high byte is `0xD0`–`0xDF` or
    ///   `0xF0`–`0xFF`.
    /// - [`DecodeBodyIndexError::ReservedSub`] if the low byte is a sub-index its slot reserves.
    pub fn decode(raw: u16) -> Result<(BodySlot, BodySub), DecodeBodyIndexError> {
        let [slot_byte, sub_byte] = raw.to_be_bytes();
        let reserved_sub = DecodeBodyIndexError::ReservedSub { raw };
        let rings = RING_SUB_START..RING_SUB_START + BLOCK_LEN;
        let planet_sub = |sub: u8| match sub {
            0 => Ok(BodySub::Primary),
            1..=LAST_MOON_SUB => Ok(BodySub::Moon(sub)),
            _ if rings.contains(&sub) => Ok(BodySub::Ring(sub - RING_SUB_START)),
            _ => Err(reserved_sub),
        };
        match slot_byte {
            0 => {
                let sub = match sub_byte {
                    0 => BodySub::Primary,
                    _ if sub_byte < STELLAR_SUB_END => BodySub::Component(sub_byte),
                    _ if rings.contains(&sub_byte) => BodySub::Ring(sub_byte - RING_SUB_START),
                    _ => return Err(reserved_sub),
                };
                Ok((BodySlot::Stellar, sub))
            }
            1..=LAST_PLANET_SLOT => Ok((BodySlot::Planet(slot_byte), planet_sub(sub_byte)?)),
            _ if (SECOND_GENERATION_SLOT_START..SECOND_GENERATION_SLOT_START + BLOCK_LEN)
                .contains(&slot_byte) =>
            {
                let slot = BodySlot::SecondGeneration(slot_byte - SECOND_GENERATION_SLOT_START);
                Ok((slot, planet_sub(sub_byte)?))
            }
            _ if (BELT_SLOT_START..BELT_SLOT_START + BLOCK_LEN).contains(&slot_byte) => {
                let sub = if sub_byte == 0 {
                    BodySub::Primary
                } else {
                    BodySub::Member(sub_byte)
                };
                Ok((BodySlot::Belt(slot_byte - BELT_SLOT_START), sub))
            }
            _ => Err(DecodeBodyIndexError::ReservedSlot { raw }),
        }
    }

    /// The raw 16 bits, as plan 01's [`BodyId`] holds them.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// The body's slot.
    #[must_use]
    pub fn slot(self) -> BodySlot {
        self.parts().0
    }

    /// The body's sub-index within its slot.
    #[must_use]
    pub fn sub(self) -> BodySub {
        self.parts().1
    }

    /// The body this one belongs to, where the index alone says: a moon's or a ring's parent is
    /// its planet (sub-index 0 of its slot), a ring of the stellar level belongs to the
    /// free-floating object at `0x0000`, and a belt member to its belt's population.
    ///
    /// `None` for a body whose parent the system resolves: a planet orbits a host that plan 14's
    /// placement chose, a component of the stellar level sits in plan 11's hierarchy, and a
    /// primary star or a belt's population has no parent body.
    #[must_use]
    pub fn parent(self) -> Option<Self> {
        match self.sub() {
            BodySub::Primary | BodySub::Component(_) => None,
            BodySub::Moon(_) | BodySub::Ring(_) | BodySub::Member(_) => {
                let [slot_byte, _] = self.0.to_be_bytes();
                Some(Self(u16::from_be_bytes([slot_byte, 0])))
            }
        }
    }

    /// The body's ID in `system`.
    #[must_use]
    pub const fn body_id(self, system: SystemId) -> BodyId {
        BodyId::new(system, self.0)
    }

    /// The slot and sub-index, which cannot fail for a value of the type.
    #[must_use]
    fn parts(self) -> (BodySlot, BodySub) {
        Self::decode(self.0).expect("every BodyIndex is built canonical and decodes")
    }
}

impl TryFrom<u16> for BodyIndex {
    type Error = DecodeBodyIndexError;

    /// The index `raw`, if it is in the layout.
    fn try_from(raw: u16) -> Result<Self, Self::Error> {
        Self::decode(raw).map(|_| Self(raw))
    }
}

impl From<BodyIndex> for u16 {
    fn from(index: BodyIndex) -> Self {
        index.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `raw` is a body of design note 3's table, written from the table alone.
    fn in_the_table(raw: u16) -> bool {
        let slot = raw >> 8;
        let sub = raw & 0xff;
        let planet_sub = sub <= 0x7f || (0x80..=0x8f).contains(&sub);
        match slot {
            0x00 => sub <= 0x0f || (0x80..=0x8f).contains(&sub),
            0x01..=0xcf => planet_sub,
            0xe0..=0xef => true,
            _ => false,
        }
    }

    #[test]
    fn every_value_round_trips_and_exactly_the_table_decodes() {
        let mut bodies = 0_u32;
        for raw in 0..=u16::MAX {
            let decoded = BodyIndex::decode(raw);
            assert_eq!(decoded.is_ok(), in_the_table(raw), "{raw:04x}: {decoded:?}");
            if let Ok((slot, sub)) = decoded {
                bodies += 1;
                let index = BodyIndex::new(slot, sub).expect("a decoded pair encodes");
                assert_eq!(index.get(), raw, "{slot:?} {sub:?}");
                assert_eq!((index.slot(), index.sub()), (slot, sub));
                assert_eq!(BodyIndex::try_from(raw), Ok(index));
                assert_eq!(u16::from(index), raw);
            } else {
                assert_eq!(BodyIndex::try_from(raw).err(), decoded.err());
            }
        }
        assert_eq!(bodies, 33_936);
    }

    #[test]
    fn reserved_values_name_what_is_reserved() {
        for raw in [0xd000, 0xdfff, 0xf000, 0xffff] {
            assert_eq!(
                BodyIndex::decode(raw),
                Err(DecodeBodyIndexError::ReservedSlot { raw })
            );
        }
        for raw in [0x0010, 0x007f, 0x0090, 0x00ff, 0x0190, 0xbfff, 0xc0a0] {
            assert_eq!(
                BodyIndex::decode(raw),
                Err(DecodeBodyIndexError::ReservedSub { raw })
            );
        }
    }

    #[test]
    fn the_stellar_level_keeps_plan_eleven_s_sixteen_stars() {
        for raw in 0..u16::from(STELLAR_SUB_END) {
            let (slot, sub) = BodyIndex::decode(raw).expect("a star");
            assert_eq!(slot, BodySlot::Stellar);
            let expected = if raw == 0 {
                BodySub::Primary
            } else {
                BodySub::Component(u8::try_from(raw).unwrap())
            };
            assert_eq!(sub, expected);
        }
        // Nothing of the stellar level lies at 16 or above except a free-floating object's rings.
        assert_eq!(
            BodyIndex::decode(u16::from(STELLAR_SUB_END)),
            Err(DecodeBodyIndexError::ReservedSub { raw: 16 })
        );
        assert_eq!(BodyIndex::PRIMARY.get(), 0);
        assert_eq!(
            BodyIndex::decode(0x0100),
            Ok((BodySlot::Planet(1), BodySub::Primary))
        );
    }

    #[test]
    fn blocks_start_where_the_table_says() {
        let at = |slot, sub| BodyIndex::new(slot, sub).unwrap().get();
        assert_eq!(at(BodySlot::Stellar, BodySub::Component(15)), 0x000f);
        assert_eq!(at(BodySlot::Stellar, BodySub::Ring(0)), 0x0080);
        assert_eq!(at(BodySlot::Planet(1), BodySub::Moon(1)), 0x0101);
        assert_eq!(at(BodySlot::Planet(0xbf), BodySub::Ring(15)), 0xbf8f);
        assert_eq!(at(BodySlot::SecondGeneration(0), BodySub::Primary), 0xc000);
        assert_eq!(
            at(BodySlot::SecondGeneration(15), BodySub::Moon(127)),
            0xcf7f
        );
        assert_eq!(at(BodySlot::Belt(0), BodySub::Primary), 0xe000);
        assert_eq!(at(BodySlot::Belt(15), BodySub::Member(255)), 0xefff);
    }

    #[test]
    fn out_of_block_parts_are_rejected() {
        use EncodeBodyIndexError::{SlotOutOfRange, SubNotInSlot, SubOutOfRange};
        let cases = [
            (
                BodySlot::Planet(0),
                BodySub::Primary,
                SlotOutOfRange(BodySlot::Planet(0)),
            ),
            (
                BodySlot::Planet(0xc0),
                BodySub::Primary,
                SlotOutOfRange(BodySlot::Planet(0xc0)),
            ),
            (
                BodySlot::Belt(16),
                BodySub::Primary,
                SlotOutOfRange(BodySlot::Belt(16)),
            ),
            (
                BodySlot::SecondGeneration(16),
                BodySub::Primary,
                SlotOutOfRange(BodySlot::SecondGeneration(16)),
            ),
            (
                BodySlot::Stellar,
                BodySub::Component(0),
                SubOutOfRange(BodySub::Component(0)),
            ),
            (
                BodySlot::Stellar,
                BodySub::Component(16),
                SubOutOfRange(BodySub::Component(16)),
            ),
            (
                BodySlot::Planet(1),
                BodySub::Moon(0),
                SubOutOfRange(BodySub::Moon(0)),
            ),
            (
                BodySlot::Planet(1),
                BodySub::Moon(128),
                SubOutOfRange(BodySub::Moon(128)),
            ),
            (
                BodySlot::Planet(1),
                BodySub::Ring(16),
                SubOutOfRange(BodySub::Ring(16)),
            ),
            (
                BodySlot::Belt(0),
                BodySub::Member(0),
                SubOutOfRange(BodySub::Member(0)),
            ),
        ];
        for (slot, sub, error) in cases {
            assert_eq!(BodyIndex::new(slot, sub), Err(error), "{slot:?} {sub:?}");
        }
        let misplaced = [
            (BodySlot::Stellar, BodySub::Moon(1)),
            (BodySlot::Stellar, BodySub::Member(1)),
            (BodySlot::Planet(3), BodySub::Component(1)),
            (BodySlot::Planet(3), BodySub::Member(1)),
            (BodySlot::SecondGeneration(2), BodySub::Component(2)),
            (BodySlot::Belt(1), BodySub::Moon(1)),
            (BodySlot::Belt(1), BodySub::Ring(0)),
            (BodySlot::Belt(1), BodySub::Component(1)),
        ];
        for (slot, sub) in misplaced {
            assert_eq!(
                BodyIndex::new(slot, sub),
                Err(SubNotInSlot { slot, sub }),
                "{slot:?} {sub:?}"
            );
        }
    }

    #[test]
    fn parents_are_what_the_index_alone_says() {
        let index = |slot, sub| BodyIndex::new(slot, sub).unwrap();
        let planet = index(BodySlot::Planet(4), BodySub::Primary);
        assert_eq!(planet.parent(), None);
        assert_eq!(
            index(BodySlot::Planet(4), BodySub::Moon(9)).parent(),
            Some(planet)
        );
        assert_eq!(
            index(BodySlot::Planet(4), BodySub::Ring(2)).parent(),
            Some(planet)
        );
        let second = index(BodySlot::SecondGeneration(1), BodySub::Primary);
        assert_eq!(
            index(BodySlot::SecondGeneration(1), BodySub::Moon(1)).parent(),
            Some(second)
        );
        let belt = index(BodySlot::Belt(2), BodySub::Primary);
        assert_eq!(belt.parent(), None);
        assert_eq!(
            index(BodySlot::Belt(2), BodySub::Member(200)).parent(),
            Some(belt)
        );
        assert_eq!(BodyIndex::PRIMARY.parent(), None);
        assert_eq!(
            index(BodySlot::Stellar, BodySub::Component(3)).parent(),
            None
        );
        assert_eq!(
            index(BodySlot::Stellar, BodySub::Ring(1)).parent(),
            Some(BodyIndex::PRIMARY)
        );
        // Every canonical index's parent is itself canonical and in the same slot.
        for raw in 0..=u16::MAX {
            if let Ok(index) = BodyIndex::try_from(raw)
                && let Some(parent) = index.parent()
            {
                assert_eq!(parent.slot(), index.slot());
                assert_eq!(parent.sub(), BodySub::Primary);
            }
        }
    }

    #[test]
    fn indices_order_as_a_system_lists_its_bodies() {
        let index = |slot, sub| BodyIndex::new(slot, sub).unwrap();
        let listed = [
            BodyIndex::PRIMARY,
            index(BodySlot::Stellar, BodySub::Component(1)),
            index(BodySlot::Planet(1), BodySub::Primary),
            index(BodySlot::Planet(1), BodySub::Moon(1)),
            index(BodySlot::Planet(1), BodySub::Ring(0)),
            index(BodySlot::Planet(2), BodySub::Primary),
            index(BodySlot::SecondGeneration(0), BodySub::Primary),
            index(BodySlot::Belt(0), BodySub::Primary),
            index(BodySlot::Belt(0), BodySub::Member(1)),
        ];
        assert!(listed.windows(2).all(|w| w[0] < w[1]));
        // Slots order as their raw values do, so sorting by slot keeps a system's list order.
        let slots: Vec<BodySlot> = listed.iter().map(|i| i.slot()).collect();
        assert!(slots.windows(2).all(|w| w[0] <= w[1]), "{slots:?}");
        let mut previous = None;
        for raw in 0..=u16::MAX {
            if let Ok(index) = BodyIndex::try_from(raw) {
                let slot = index.slot();
                assert!(previous <= Some(slot), "{raw:04x}");
                previous = Some(slot);
            }
        }
    }
}
