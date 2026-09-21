//! Members of features on the global list (prefix `10`): the galactic centre, streams and dwarf
//! cores.

use super::bits::{Field, assert_tiles_64};
use super::layer::{LAYER, Layer};
use super::nested::{CENTRE_SLOT, FEATURE_LEVEL_BAND, FEATURE_SLOT, MEMBER_INDEX, MemberSlot};
use super::reserved::{PREFIX_2, RESERVED};
use super::system::{BuildSystemIdError, DecodeSystemIdError, SystemIdKind};

/// The sub-kind, `[58:57]`.
const SUB_KIND: Field = Field::new(58, 57);

const CENTRE: u64 = 0b00;
const STREAM: u64 = 0b01;
const DWARF_CORE: u64 = 0b10;

/// Zero bits under the centre's sub-kind.
const CENTRE_PADDING: Field = Field::new(56, 35);

/// Zero bits under a stream's sub-kind.
const STREAM_PADDING: Field = Field::new(56, 54);
/// The stream's number on the global list.
const STREAM_NUMBER: Field = Field::new(53, 42);
/// The member's mass band, 0–6.
const STREAM_BAND: Field = Field::new(41, 39);
/// The tube cell along the track.
const STREAM_ALONG: Field = Field::new(38, 25);
/// The tube cells across the track, a then b.
const STREAM_ACROSS: [Field; 2] = [Field::new(24, 19), Field::new(18, 13)];

/// Zero bits under a dwarf core's sub-kind.
const DWARF_PADDING: Field = Field::new(56, 33);
/// The dwarf core's number on the global list.
const DWARF_NUMBER: Field = Field::new(32, 31);

/// The prefix `10` in place under the reserved layer value, with a sub-kind.
const fn base(sub_kind: u64) -> u64 {
    SUB_KIND.with(PREFIX_2.with(RESERVED, 0b10), sub_kind)
}

const _: () = {
    assert_tiles_64(&[
        LAYER,
        PREFIX_2,
        SUB_KIND,
        CENTRE_PADDING,
        CENTRE_SLOT.band(),
        CENTRE_SLOT.level(),
        CENTRE_SLOT.cell(0),
        CENTRE_SLOT.cell(1),
        CENTRE_SLOT.cell(2),
        MEMBER_INDEX,
    ]);
    assert_tiles_64(&[
        LAYER,
        PREFIX_2,
        SUB_KIND,
        STREAM_PADDING,
        STREAM_NUMBER,
        STREAM_BAND,
        STREAM_ALONG,
        STREAM_ACROSS[0],
        STREAM_ACROSS[1],
        MEMBER_INDEX,
    ]);
    assert_tiles_64(&[
        LAYER,
        PREFIX_2,
        SUB_KIND,
        DWARF_PADDING,
        DWARF_NUMBER,
        FEATURE_SLOT.band(),
        FEATURE_SLOT.level(),
        FEATURE_SLOT.cell(0),
        FEATURE_SLOT.cell(1),
        FEATURE_SLOT.cell(2),
        MEMBER_INDEX,
    ]);
};

/// Validates an ID with prefix `10`.
pub(super) const fn validate(raw: u64) -> Result<(), DecodeSystemIdError> {
    match SUB_KIND.get(raw) {
        CENTRE if !CENTRE_PADDING.is_zero(raw) => Err(DecodeSystemIdError::PaddingBitsSet),
        CENTRE => CENTRE_SLOT.validate(raw),
        STREAM if !STREAM_PADDING.is_zero(raw) => Err(DecodeSystemIdError::PaddingBitsSet),
        STREAM if STREAM_BAND.get(raw) == FEATURE_LEVEL_BAND => {
            Err(DecodeSystemIdError::BandOutOfRange)
        }
        STREAM => Ok(()),
        DWARF_CORE if !DWARF_PADDING.is_zero(raw) => Err(DecodeSystemIdError::PaddingBitsSet),
        DWARF_CORE => FEATURE_SLOT.validate(raw),
        _ => Err(DecodeSystemIdError::UnknownSubKind),
    }
}

/// The kind of a validated ID with prefix `10`.
pub(super) fn kind(raw: u64) -> SystemIdKind {
    match SUB_KIND.get(raw) {
        CENTRE => SystemIdKind::Centre(CentreMemberId(raw)),
        STREAM => SystemIdKind::Stream(StreamMemberId(raw)),
        _ => SystemIdKind::DwarfCore(DwarfCoreMemberId(raw)),
    }
}

/// A member of the galactic centre: prefix `10`, sub-kind `00`.
///
/// The centre's nested grid has twelve levels of 32 cells per axis, from cells of 1 ÷ 256 ly up
/// to 8 ly and a reach of 128 ly (brainstorm, "Dense features"), so a level fills four bits and
/// levels 12–15 are rejected. Band 7 is the centre's feature-level list, whose member 0 is the
/// central black hole.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{CentreMemberId, MemberSlot, SystemId};
///
/// let black_hole = CentreMemberId::new(MemberSlot::FeatureLevel { index: 0 })?;
/// assert_eq!(black_hole, CentreMemberId::CENTRAL_BLACK_HOLE);
/// assert_eq!(SystemId::from(black_hole).raw(), 0xF000_0007_0000_0000);
/// # Ok::<(), hyperion_sim::id::BuildSystemIdError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CentreMemberId(u64);

impl CentreMemberId {
    /// The central black hole: member zero of the centre's feature-level list.
    pub const CENTRAL_BLACK_HOLE: Self =
        Self(CENTRE_SLOT.band().with(base(CENTRE), FEATURE_LEVEL_BAND));

    /// The member of the centre in `slot`.
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::IndexTooLarge`] or [`BuildSystemIdError::FieldOutOfRange`] for a slot
    /// field beyond its range (levels 0–11, cells 0–31); [`BuildSystemIdError::NotCanonical`] with
    /// [`DecodeSystemIdError::InnerCellOwnedByLowerLevel`] for a cell the level below owns.
    pub fn new(slot: MemberSlot) -> Result<Self, BuildSystemIdError> {
        let raw = CENTRE_SLOT.with_slot(base(CENTRE), slot)?;
        CENTRE_SLOT
            .validate(raw)
            .map_err(BuildSystemIdError::NotCanonical)?;
        Ok(Self(raw))
    }

    /// The raw ID.
    #[must_use]
    pub(super) const fn raw(self) -> u64 {
        self.0
    }

    /// Where the member sits in the centre.
    #[must_use]
    pub fn slot(self) -> MemberSlot {
        CENTRE_SLOT.slot(self.0)
    }
}

/// A member of a stream: prefix `10`, sub-kind `01`.
///
/// Members are a Poisson process in the tube's own coordinates, one cell along the track and two
/// across it (brainstorm, "Streams and accreted structure"). Streams have no feature-level
/// members, so the mass band is 0–6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamMemberId(u64);

impl StreamMemberId {
    /// The exclusive bound of a stream's number: 4,096.
    pub const NUMBER_LIMIT: u16 = 1 << 12;
    /// The exclusive bound of the cell along the track: 16,384.
    pub const ALONG_LIMIT: u16 = 1 << 14;
    /// The exclusive bound of each cell across the track: 64.
    pub const ACROSS_LIMIT: u8 = 1 << 6;

    /// Member `index` of `band` in tube cell (`along`, `across`) of stream `number`.
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::FieldOutOfRange`] for a number, `along` or `across` beyond its
    /// limit; [`BuildSystemIdError::IndexTooLarge`] for an index of 8,192 or more.
    pub fn new(
        number: u16,
        band: Layer,
        along: u16,
        across: [u8; 2],
        index: u16,
    ) -> Result<Self, BuildSystemIdError> {
        if number >= Self::NUMBER_LIMIT {
            return Err(BuildSystemIdError::FieldOutOfRange {
                field: "stream number",
            });
        }
        if along >= Self::ALONG_LIMIT {
            return Err(BuildSystemIdError::FieldOutOfRange {
                field: "cell along",
            });
        }
        if across.iter().any(|&a| a >= Self::ACROSS_LIMIT) {
            return Err(BuildSystemIdError::FieldOutOfRange {
                field: "cell across",
            });
        }
        if index >= MemberSlot::INDEX_LIMIT {
            return Err(BuildSystemIdError::IndexTooLarge);
        }
        let mut raw = STREAM_NUMBER.with(base(STREAM), u64::from(number));
        raw = STREAM_BAND.with(raw, u64::from(band.value()));
        raw = STREAM_ALONG.with(raw, u64::from(along));
        for (field, a) in STREAM_ACROSS.into_iter().zip(across) {
            raw = field.with(raw, u64::from(a));
        }
        Ok(Self(MEMBER_INDEX.with(raw, u64::from(index))))
    }

    /// The raw ID.
    #[must_use]
    pub(super) const fn raw(self) -> u64 {
        self.0
    }

    /// The stream's number on the global list.
    #[must_use]
    pub fn number(self) -> u16 {
        STREAM_NUMBER.get_u16(self.0)
    }

    /// The member's mass band.
    ///
    /// # Panics
    ///
    /// Never: decoding rejects band 7 for a stream member.
    #[must_use]
    pub fn band(self) -> Layer {
        Layer::from_value(STREAM_BAND.get_u8(self.0)).expect("a stream member's band is 0 to 6")
    }

    /// The tube cell along the track.
    #[must_use]
    pub fn along(self) -> u16 {
        STREAM_ALONG.get_u16(self.0)
    }

    /// The tube cells across the track, a then b.
    #[must_use]
    pub fn across(self) -> [u8; 2] {
        STREAM_ACROSS.map(|f| f.get_u8(self.0))
    }

    /// The candidate number in the tube cell and band.
    #[must_use]
    pub fn index(self) -> u16 {
        MEMBER_INDEX.get_u16(self.0)
    }
}

/// A member of a dwarf-galaxy core: prefix `10`, sub-kind `10`.
///
/// The core's number takes two bits; the slot follows the catalogue-feature rules exactly, in the
/// same bit positions `[30:0]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DwarfCoreMemberId(u64);

impl DwarfCoreMemberId {
    /// The exclusive bound of a dwarf core's number: 4.
    pub const NUMBER_LIMIT: u8 = 1 << 2;

    /// The member of dwarf core `number` in `slot`.
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::FieldOutOfRange`] for a number of 4 or more; otherwise as
    /// [`FeatureMemberId::new`](super::FeatureMemberId::new).
    pub fn new(number: u8, slot: MemberSlot) -> Result<Self, BuildSystemIdError> {
        if number >= Self::NUMBER_LIMIT {
            return Err(BuildSystemIdError::FieldOutOfRange {
                field: "dwarf core number",
            });
        }
        let raw = DWARF_NUMBER.with(base(DWARF_CORE), u64::from(number));
        let raw = FEATURE_SLOT.with_slot(raw, slot)?;
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

    /// The dwarf core's number on the global list.
    #[must_use]
    pub fn number(self) -> u8 {
        DWARF_NUMBER.get_u8(self.0)
    }

    /// Where the member sits in the core.
    #[must_use]
    pub fn slot(self) -> MemberSlot {
        FEATURE_SLOT.slot(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{FeatureCell, FeatureMemberId, FeatureRef, SystemId};

    #[test]
    fn hand_computed_global_members() {
        // Layer 7, prefix 10, sub-kind 00, band 7 at [34:32], everything else zero.
        assert_eq!(
            CentreMemberId::CENTRAL_BLACK_HOLE.raw(),
            (0b111 << 61) | (0b10 << 59) | (7 << 32)
        );
        assert_eq!(
            CentreMemberId::CENTRAL_BLACK_HOLE.raw(),
            0xF000_0007_0000_0000
        );

        let centre = CentreMemberId::new(MemberSlot::InCell {
            band: Layer::A,
            level: 9,
            cell: [2, 17, 18],
            index: 12,
        })
        .unwrap();
        assert_eq!(
            centre.raw(),
            (0b111_1000 << 57) | (9 << 28) | (2 << 23) | (17 << 18) | (18 << 13) | 0xC
        );
        assert_eq!(centre.raw(), 0xF000_0000_9146_400C);

        let stream = StreamMemberId::new(87, Layer::A, 5_121, [3, 23], 9).unwrap();
        assert_eq!(
            stream.raw(),
            (0b111_1001 << 57) | (87 << 42) | (5_121 << 25) | (3 << 19) | (23 << 13) | 9
        );
        assert_eq!(stream.raw(), 0xF201_5C28_021A_E009);

        let dwarf = DwarfCoreMemberId::new(
            1,
            MemberSlot::InCell {
                band: Layer::B,
                level: 2,
                cell: [4, 7, 12],
                index: 3,
            },
        )
        .unwrap();
        assert_eq!(
            dwarf.raw(),
            (0b111_1010 << 57)
                | (1 << 31)
                | (1 << 28)
                | (2 << 25)
                | (4 << 21)
                | (7 << 17)
                | (12 << 13)
                | 3
        );
        assert_eq!(dwarf.raw(), 0xF400_0000_948F_8003);
    }

    #[test]
    fn every_padding_bit_is_rejected() {
        let valid = [
            (CentreMemberId::CENTRAL_BLACK_HOLE.raw(), CENTRE_PADDING),
            (
                StreamMemberId::new(4_095, Layer::E, 16_383, [63, 63], 8_191)
                    .unwrap()
                    .raw(),
                STREAM_PADDING,
            ),
            (
                DwarfCoreMemberId::new(3, MemberSlot::FeatureLevel { index: 1 })
                    .unwrap()
                    .raw(),
                DWARF_PADDING,
            ),
        ];
        for (raw, padding) in valid {
            assert!(SystemId::from_raw(raw).is_ok());
            for bit in padding.lo()..=padding.hi() {
                assert_eq!(
                    SystemId::from_raw(raw | (1 << bit)),
                    Err(DecodeSystemIdError::PaddingBitsSet),
                    "bit {bit} of {raw:#018x}"
                );
            }
        }
    }

    #[test]
    fn global_rules_have_their_own_errors() {
        let unknown = SUB_KIND.with(base(0), 0b11);
        assert_eq!(
            SystemId::from_raw(unknown),
            Err(DecodeSystemIdError::UnknownSubKind)
        );
        assert_eq!(
            SystemId::from_raw(unknown | 1),
            Err(DecodeSystemIdError::UnknownSubKind)
        );
        for level in 12..16 {
            assert_eq!(
                SystemId::from_raw(CENTRE_SLOT.level().with(base(CENTRE), level)),
                Err(DecodeSystemIdError::LevelOutOfRange)
            );
        }
        assert_eq!(
            SystemId::from_raw(CENTRE_SLOT.level().with(base(CENTRE), 11)).map(SystemId::raw),
            Ok(CENTRE_SLOT.level().with(base(CENTRE), 11))
        );
        assert_eq!(
            SystemId::from_raw(STREAM_BAND.with(base(STREAM), 7)),
            Err(DecodeSystemIdError::BandOutOfRange)
        );
        assert_eq!(
            SystemId::from_raw(
                CENTRE_SLOT
                    .cell(0)
                    .with(CentreMemberId::CENTRAL_BLACK_HOLE.raw(), 1)
            ),
            Err(DecodeSystemIdError::FeatureLevelFieldsSet)
        );
        let mut inner = CENTRE_SLOT.level().with(base(CENTRE), 5);
        for axis in 0..3 {
            inner = CENTRE_SLOT.cell(axis).with(inner, 23);
        }
        assert_eq!(
            SystemId::from_raw(inner),
            Err(DecodeSystemIdError::InnerCellOwnedByLowerLevel)
        );
        let mut dwarf_inner = FEATURE_SLOT.level().with(base(DWARF_CORE), 7);
        for axis in 0..3 {
            dwarf_inner = FEATURE_SLOT.cell(axis).with(dwarf_inner, 4);
        }
        assert_eq!(
            SystemId::from_raw(dwarf_inner),
            Err(DecodeSystemIdError::InnerCellOwnedByLowerLevel)
        );
    }

    #[test]
    fn builders_check_ranges() {
        assert_eq!(
            StreamMemberId::new(4_096, Layer::A, 0, [0, 0], 0),
            Err(BuildSystemIdError::FieldOutOfRange {
                field: "stream number"
            })
        );
        assert_eq!(
            StreamMemberId::new(0, Layer::A, 16_384, [0, 0], 0),
            Err(BuildSystemIdError::FieldOutOfRange {
                field: "cell along"
            })
        );
        assert_eq!(
            StreamMemberId::new(0, Layer::A, 0, [0, 64], 0),
            Err(BuildSystemIdError::FieldOutOfRange {
                field: "cell across"
            })
        );
        assert_eq!(
            StreamMemberId::new(0, Layer::A, 0, [0, 0], 8_192),
            Err(BuildSystemIdError::IndexTooLarge)
        );
        assert_eq!(
            DwarfCoreMemberId::new(4, MemberSlot::FeatureLevel { index: 0 }),
            Err(BuildSystemIdError::FieldOutOfRange {
                field: "dwarf core number"
            })
        );
        assert_eq!(
            CentreMemberId::new(MemberSlot::InCell {
                band: Layer::A,
                level: 1,
                cell: [8, 8, 8],
                index: 0
            }),
            Err(BuildSystemIdError::NotCanonical(
                DecodeSystemIdError::InnerCellOwnedByLowerLevel
            ))
        );
    }

    #[test]
    fn global_members_round_trip_over_field_extremes() {
        for slot in [
            MemberSlot::FeatureLevel { index: 0 },
            MemberSlot::FeatureLevel { index: 8_191 },
            MemberSlot::InCell {
                band: Layer::RoguePlanet,
                level: 11,
                cell: [31, 0, 31],
                index: 8_191,
            },
            MemberSlot::InCell {
                band: Layer::A,
                level: 0,
                cell: [15, 16, 15],
                index: 0,
            },
        ] {
            let centre = CentreMemberId::new(slot).unwrap();
            assert_eq!(
                SystemId::from_raw(centre.raw()).unwrap().kind(),
                SystemIdKind::Centre(centre)
            );
            assert_eq!(centre.slot(), slot);
        }
        for (number, band, along, across, index) in [
            (0, Layer::A, 0, [0, 0], 0),
            (4_095, Layer::RoguePlanet, 16_383, [63, 63], 8_191),
            (1, Layer::C, 8_192, [32, 1], 17),
        ] {
            let stream = StreamMemberId::new(number, band, along, across, index).unwrap();
            assert_eq!(
                SystemId::from_raw(stream.raw()).unwrap().kind(),
                SystemIdKind::Stream(stream)
            );
            assert_eq!(
                (
                    stream.number(),
                    stream.band(),
                    stream.along(),
                    stream.across(),
                    stream.index()
                ),
                (number, band, along, across, index)
            );
        }
        let feature = FeatureRef::new(FeatureCell::new([3, -3, 0]).unwrap(), 77).unwrap();
        for number in 0..4 {
            for slot in [
                MemberSlot::FeatureLevel { index: 5 },
                MemberSlot::InCell {
                    band: Layer::D,
                    level: 7,
                    cell: [15, 4, 11],
                    index: 8_191,
                },
            ] {
                let dwarf = DwarfCoreMemberId::new(number, slot).unwrap();
                assert_eq!(
                    SystemId::from_raw(dwarf.raw()).unwrap().kind(),
                    SystemIdKind::DwarfCore(dwarf)
                );
                assert_eq!((dwarf.number(), dwarf.slot()), (number, slot));
                // The low 31 bits sit where a catalogue feature member's do.
                let member = SystemId::from(FeatureMemberId::new(feature, slot).unwrap()).raw();
                assert_eq!(dwarf.raw() & ((1 << 31) - 1), member & ((1 << 31) - 1));
            }
        }
    }
}
