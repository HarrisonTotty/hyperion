//! The reserved layer value: its prefixes, and members of catalogue features (prefix `0`).

use super::bits::{Field, assert_tiles_64};
use super::catalogue::{self, CatalogueSystemId, PinnedId};
use super::global;
use super::layer::{LAYER, RESERVED_LAYER_VALUE};
use super::nested::{FEATURE_SLOT, MEMBER_INDEX, MemberSlot};
use super::system::{BuildSystemIdError, DecodeSystemIdError, SystemIdKind};
use crate::coords::LyCell;

/// The first prefix bit under the reserved layer value, `[60]`: `0` for a feature member.
const PREFIX_1: Field = Field::new(60, 60);

/// Two prefix bits, `[60:59]`: `10` for a member of a feature on the global list.
pub(super) const PREFIX_2: Field = Field::new(60, 59);

/// Three prefix bits, `[60:58]`: `110` for pinned content, `111` for a catalogue system.
pub(super) const PREFIX_3: Field = Field::new(60, 58);

/// The reserved layer value in place, with every other bit zero.
pub(super) const RESERVED: u64 = LAYER.with(0, 0b111);

const _: () = assert!(LAYER.get_u8(RESERVED) == RESERVED_LAYER_VALUE);

/// Feature cell x, y, z: `[59:55]`, `[54:50]`, `[49:45]`.
const FEATURE_CELL: [Field; 3] = [Field::new(59, 55), Field::new(54, 50), Field::new(49, 45)];

/// The feature's candidate number in its feature cell, `[44:31]`.
const FEATURE_INDEX: Field = Field::new(44, 31);

const _: () = assert_tiles_64(&[
    LAYER,
    PREFIX_1,
    FEATURE_CELL[0],
    FEATURE_CELL[1],
    FEATURE_CELL[2],
    FEATURE_INDEX,
    FEATURE_SLOT.band(),
    FEATURE_SLOT.level(),
    FEATURE_SLOT.cell(0),
    FEATURE_SLOT.cell(1),
    FEATURE_SLOT.cell(2),
    MEMBER_INDEX,
]);

/// Validates an ID under the reserved layer value, dispatching on its prefix.
pub(super) const fn validate(raw: u64) -> Result<(), DecodeSystemIdError> {
    if PREFIX_1.get(raw) == 0 {
        return FEATURE_SLOT.validate(raw);
    }
    if PREFIX_2.get(raw) == 0b10 {
        return global::validate(raw);
    }
    // Prefixes `110` and `111`: every pinned number is well-formed, and a catalogue system's
    // alignment to its class's cell size is checked by the class's resolve.
    Ok(())
}

/// The kind of a validated ID under the reserved layer value.
pub(super) fn kind(raw: u64) -> SystemIdKind {
    if PREFIX_1.get(raw) == 0 {
        return SystemIdKind::FeatureMember(FeatureMemberId(raw));
    }
    if PREFIX_2.get(raw) == 0b10 {
        return global::kind(raw);
    }
    if PREFIX_3.get(raw) == catalogue::PINNED_PREFIX {
        SystemIdKind::Pinned(PinnedId::from_valid_raw(raw))
    } else {
        SystemIdKind::Catalogue(CatalogueSystemId::from_valid_raw(raw))
    }
}

/// A cell of the feature catalogue's grid: a cube of 4,096 ly, 32 per axis across the root cube.
///
/// Cell `(x, y, z)` spans `[4,096 x, 4,096 (x + 1))` ly on the x axis and likewise on y and z, so
/// coordinates run from −16 to 15. The same cubes are the sectors of designations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureCell {
    x: i32,
    y: i32,
    z: i32,
}

impl FeatureCell {
    /// The edge of a feature cell in light-years.
    pub const EDGE_LY: u32 = 4_096;

    /// Half the number of cells per axis: coordinates run from `−HALF` to `HALF − 1`.
    const HALF: i32 = 16;

    /// The feature cell with these coordinates.
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::CellOutsideRootCube`] unless every coordinate is in −16..=15.
    pub fn new(coordinates: [i32; 3]) -> Result<Self, BuildSystemIdError> {
        if coordinates
            .iter()
            .any(|c| !(-Self::HALF..Self::HALF).contains(c))
        {
            return Err(BuildSystemIdError::CellOutsideRootCube);
        }
        let [x, y, z] = coordinates;
        Ok(Self { x, y, z })
    }

    /// The feature cell containing a light-year cell, or `None` outside the root cube.
    #[must_use]
    pub fn of_ly_cell(cell: LyCell) -> Option<Self> {
        Self::new(cell.to_array().map(|c| c >> 12)).ok()
    }

    /// The coordinates, x, y, z.
    #[must_use]
    pub const fn to_array(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }

    /// The low corner in light-years.
    #[must_use]
    pub const fn origin(self) -> LyCell {
        LyCell::new([self.x << 12, self.y << 12, self.z << 12])
    }

    /// The stored (offset, unsigned) coordinates, each 0–31.
    #[must_use]
    pub(super) fn stored(self) -> [u8; 3] {
        self.to_array().map(|c| {
            u8::try_from(c + Self::HALF).expect("a feature cell coordinate is in −16..=15")
        })
    }

    /// The cell with these stored coordinates, each 0–31.
    #[must_use]
    fn from_stored(stored: [u8; 3]) -> Self {
        let [x, y, z] = stored.map(|s| i32::from(s) - Self::HALF);
        Self { x, y, z }
    }
}

/// A reference to a catalogue feature: its feature cell and its candidate number there.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{FeatureCell, FeatureRef};
///
/// let feature = FeatureRef::new(FeatureCell::new([0, 0, 0])?, 5)?;
/// // The word a feature's streams are keyed by: its members' IDs with the slot zeroed.
/// assert_eq!(feature.object_word(), 0xE842_0002_8000_0000);
/// # Ok::<(), hyperion_sim::id::BuildSystemIdError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureRef {
    cell: FeatureCell,
    index: u16,
}

impl FeatureRef {
    /// The exclusive bound of a feature's candidate number: 16,384 per feature cell.
    pub const INDEX_LIMIT: u16 = 1 << 14;

    /// The feature with candidate number `index` in `cell`.
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::IndexTooLarge`] if `index` is [`INDEX_LIMIT`](Self::INDEX_LIMIT) or
    /// more.
    pub fn new(cell: FeatureCell, index: u16) -> Result<Self, BuildSystemIdError> {
        if index >= Self::INDEX_LIMIT {
            return Err(BuildSystemIdError::IndexTooLarge);
        }
        Ok(Self { cell, index })
    }

    /// The feature cell.
    #[must_use]
    pub const fn cell(self) -> FeatureCell {
        self.cell
    }

    /// The candidate number in the feature cell.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.index
    }

    /// The feature's `ObjectKey` word: a member ID with bits `[30:0]` zero.
    #[must_use]
    pub fn object_word(self) -> u64 {
        let mut raw = RESERVED;
        for (field, stored) in FEATURE_CELL.into_iter().zip(self.cell.stored()) {
            raw = field.with(raw, u64::from(stored));
        }
        FEATURE_INDEX.with(raw, u64::from(self.index))
    }

    /// The feature named by a member ID's high bits.
    fn of_raw(raw: u64) -> Self {
        Self {
            cell: FeatureCell::from_stored(FEATURE_CELL.map(|f| f.get_u8(raw))),
            index: FEATURE_INDEX.get_u16(raw),
        }
    }
}

/// A member of a catalogue feature: reserved layer, prefix `0`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{FeatureCell, FeatureMemberId, FeatureRef, Layer, MemberSlot, SystemId};
///
/// let feature = FeatureRef::new(FeatureCell::new([0, 0, 0])?, 5)?;
/// let slot = MemberSlot::InCell { band: Layer::C, level: 3, cell: [8, 15, 2], index: 40 };
/// let member = FeatureMemberId::new(feature, slot)?;
/// assert_eq!(SystemId::from(member).raw(), 0xE842_0002_A71E_4028);
/// assert_eq!((member.feature(), member.slot()), (feature, slot));
/// # Ok::<(), hyperion_sim::id::BuildSystemIdError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureMemberId(u64);

impl FeatureMemberId {
    /// The member of `feature` in `slot`.
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::IndexTooLarge`] or [`BuildSystemIdError::FieldOutOfRange`] for a slot
    /// field beyond its range (levels 0–7, cells 0–15); [`BuildSystemIdError::NotCanonical`] with
    /// [`DecodeSystemIdError::InnerCellOwnedByLowerLevel`] for a cell the level below owns.
    pub fn new(feature: FeatureRef, slot: MemberSlot) -> Result<Self, BuildSystemIdError> {
        let raw = FEATURE_SLOT.with_slot(feature.object_word(), slot)?;
        FEATURE_SLOT
            .validate(raw)
            .map_err(BuildSystemIdError::NotCanonical)?;
        Ok(Self(raw))
    }

    /// The raw ID.
    #[must_use]
    pub(super) const fn raw(self) -> u64 {
        self.0
    }

    /// The feature.
    #[must_use]
    pub fn feature(self) -> FeatureRef {
        FeatureRef::of_raw(self.0)
    }

    /// Where the member sits in the feature.
    #[must_use]
    pub fn slot(self) -> MemberSlot {
        FEATURE_SLOT.slot(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{Layer, SystemId};

    fn feature(c: [i32; 3], index: u16) -> FeatureRef {
        FeatureRef::new(FeatureCell::new(c).unwrap(), index).unwrap()
    }

    #[test]
    fn hand_computed_feature_members() {
        // Layer 7 [63:61], prefix 0, cells stored as 16 at [59:55], [54:50], [49:45], feature 5
        // at [44:31], band 2 [30:28], level 3 [27:25], cell 8, 15, 2 at [24:21], [20:17],
        // [16:13], index 40.
        let expected = (7 << 61)
            | (16 << 55)
            | (16 << 50)
            | (16 << 45)
            | (5 << 31)
            | (2 << 28)
            | (3 << 25)
            | (8 << 21)
            | (15 << 17)
            | (2 << 13)
            | 0x28;
        assert_eq!(expected, 0xE842_0002_A71E_4028);
        let slot = MemberSlot::InCell {
            band: Layer::C,
            level: 3,
            cell: [8, 15, 2],
            index: 40,
        };
        let member = FeatureMemberId::new(feature([0, 0, 0], 5), slot).unwrap();
        assert_eq!(member.raw(), expected);

        // Feature-level member 0 of the last feature in cell (−16, 15, 0).
        let level = FeatureMemberId::new(
            feature([-16, 15, 0], 16_383),
            MemberSlot::FeatureLevel { index: 0 },
        )
        .unwrap();
        assert_eq!(
            level.raw(),
            (7 << 61) | (31 << 50) | (16 << 45) | (16_383 << 31) | (7 << 28)
        );
        assert_eq!(level.raw(), 0xE07E_1FFF_F000_0000);
    }

    #[test]
    fn feature_members_round_trip_over_field_extremes() {
        let slots = [
            MemberSlot::FeatureLevel { index: 0 },
            MemberSlot::FeatureLevel { index: 8_191 },
            MemberSlot::InCell {
                band: Layer::A,
                level: 0,
                cell: [7, 8, 7],
                index: 0,
            },
            MemberSlot::InCell {
                band: Layer::RoguePlanet,
                level: 7,
                cell: [15, 15, 15],
                index: 8_191,
            },
            MemberSlot::InCell {
                band: Layer::E,
                level: 1,
                cell: [0, 11, 4],
                index: 1,
            },
        ];
        for c in [[-16, -16, -16], [15, 15, 15], [0, -1, 7]] {
            for index in [0, 1, 16_383] {
                let f = feature(c, index);
                for slot in slots {
                    let member = FeatureMemberId::new(f, slot).unwrap();
                    let id = SystemId::from_raw(member.raw()).unwrap();
                    assert_eq!(id.kind(), SystemIdKind::FeatureMember(member));
                    assert_eq!((member.feature(), member.slot()), (f, slot));
                    assert_eq!(f.object_word(), member.raw() & !((1 << 31) - 1));
                }
            }
        }
    }

    #[test]
    fn feature_member_rules_have_their_own_errors() {
        let base = feature([1, 2, 3], 9).object_word();
        // Band 7 with a level or a cell set.
        let band7 = FEATURE_SLOT.band().with(base, 7);
        assert_eq!(
            SystemId::from_raw(FEATURE_SLOT.level().with(band7, 2)),
            Err(DecodeSystemIdError::FeatureLevelFieldsSet)
        );
        assert_eq!(
            SystemId::from_raw(FEATURE_SLOT.cell(2).with(band7, 1)),
            Err(DecodeSystemIdError::FeatureLevelFieldsSet)
        );
        // An inner cell of level 2.
        let mut inner = FEATURE_SLOT.level().with(base, 2);
        for axis in 0..3 {
            inner = FEATURE_SLOT.cell(axis).with(inner, 6);
        }
        assert_eq!(
            SystemId::from_raw(inner),
            Err(DecodeSystemIdError::InnerCellOwnedByLowerLevel)
        );
        assert_eq!(
            FeatureMemberId::new(
                feature([1, 2, 3], 9),
                MemberSlot::InCell {
                    band: Layer::A,
                    level: 2,
                    cell: [6, 6, 6],
                    index: 0,
                }
            ),
            Err(BuildSystemIdError::NotCanonical(
                DecodeSystemIdError::InnerCellOwnedByLowerLevel
            ))
        );
    }

    #[test]
    fn feature_cells_and_refs_check_their_ranges() {
        assert_eq!(
            FeatureCell::new([16, 0, 0]),
            Err(BuildSystemIdError::CellOutsideRootCube)
        );
        assert_eq!(
            FeatureCell::new([0, -17, 0]),
            Err(BuildSystemIdError::CellOutsideRootCube)
        );
        assert_eq!(
            FeatureRef::new(FeatureCell::new([0; 3]).unwrap(), 16_384),
            Err(BuildSystemIdError::IndexTooLarge)
        );
        let cell = FeatureCell::of_ly_cell(LyCell::new([-1, 4_096, 65_535])).unwrap();
        assert_eq!(cell.to_array(), [-1, 1, 15]);
        assert_eq!(cell.origin(), LyCell::new([-4_096, 4_096, 61_440]));
        assert_eq!(FeatureCell::of_ly_cell(LyCell::new([65_536, 0, 0])), None);
    }
}
