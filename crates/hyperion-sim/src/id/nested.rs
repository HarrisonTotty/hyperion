//! The member slot of a feature's nested grid: mass band, level, cell in the level and index.
//!
//! Catalogue features, dwarf cores and the galactic centre share one shape in their low bits:
//!
//! | Field                | Catalogue feature, dwarf core | Galactic centre         |
//! | -------------------- | ----------------------------- | ----------------------- |
//! | Mass band            | `[30:28]`                     | `[34:32]`               |
//! | Level                | `[27:25]`, 0–7                | `[31:28]`, 0–11         |
//! | Cell x, y, z         | `[24:13]`, 4 bits each        | `[27:13]`, 5 bits each  |
//! | Index                | `[12:0]`                      | `[12:0]`                |
//! | Band 7: zero         | `[27:13]`                     | `[31:16]`               |
//! | Band 7: list index   | `[12:0]`, 8,192 entries       | `[15:0]`, 65,536        |
//!
//! Level j is a block of cells centred on the feature; its inner half on every axis is exactly
//! level j − 1, which owns that volume. Mass band 7 marks a feature-level member, drawn on the
//! feature's own stream and not in a cell. Its index counts the feature-level list from 0, and the
//! bits between it and the band are zero: the level and cell of a catalogue feature or a dwarf
//! core, and at the galactic centre, whose list is the longest, the 16 bits `[31:16]` above a
//! 16-bit index (plan 01, Design note 17; ruled 2026-10-26, P11.T8.e.b). Widening the centre's
//! index only accepts bit patterns that were rejected before, so every earlier ID keeps its value.

use super::bits::Field;
use super::layer::Layer;
use super::system::{BuildSystemIdError, DecodeSystemIdError};

/// The mass-band value of a feature-level member.
pub(super) const FEATURE_LEVEL_BAND: u64 = 7;

/// The index of a member in its cell and band, or of a catalogue feature's or a dwarf core's
/// feature-level member: `[12:0]`. The centre's feature-level index is wider
/// ([`NestedSlotLayout::list_index`]).
pub(super) const MEMBER_INDEX: Field = Field::new(12, 0);

/// Where a member sits in its feature: in a nested cell for one mass band, or on the feature-level
/// list.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{Layer, MemberSlot};
///
/// // Candidate 40 of band C in cell (8, 15, 2) of level 3.
/// let slot = MemberSlot::InCell { band: Layer::C, level: 3, cell: [8, 15, 2], index: 40 };
/// // The first entry of the feature-level list, such as a central black hole.
/// let member_zero = MemberSlot::FeatureLevel { index: 0 };
/// # let _ = (slot, member_zero);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemberSlot {
    /// A member drawn in a cell of the nested grid, for one mass band.
    InCell {
        /// The mass band, numbered as the layers.
        band: Layer,
        /// The nested level: 0–7, or 0–11 at the galactic centre.
        level: u8,
        /// The cell in the level, x, y, z: each 0–15, or 0–31 at the galactic centre. The
        /// feature's centre is the corner between cells `n ÷ 2 − 1` and `n ÷ 2`.
        cell: [u8; 3],
        /// The candidate number in the cell and band, below [`MemberSlot::INDEX_LIMIT`].
        index: u16,
    },
    /// A member of the feature-level list (mass band 7): index 0 is member zero, such as a central
    /// black hole, and 1 upward are the feature's members of the catalogue classes.
    FeatureLevel {
        /// The member number: below [`MemberSlot::INDEX_LIMIT`], 8,192, on a catalogue feature's
        /// or a dwarf core's list, and below
        /// [`CentreMemberId::LIST_INDEX_LIMIT`](super::CentreMemberId::LIST_INDEX_LIMIT), 65,536,
        /// which every `u16` is, on the galactic centre's.
        index: u16,
    },
}

impl MemberSlot {
    /// The exclusive bound of a slot's index: 8,192 candidates per cell and band, and 8,192
    /// members on a catalogue feature's or a dwarf core's feature-level list. The galactic
    /// centre's list has
    /// [`CentreMemberId::LIST_INDEX_LIMIT`](super::CentreMemberId::LIST_INDEX_LIMIT).
    pub const INDEX_LIMIT: u16 = 1 << 13;
}

/// The shape of a nested grid's slot fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct NestedSlotLayout {
    level_bits: u32,
    cell_bits_per_axis: u32,
    levels: u64,
    /// The width of the feature-level list's index under band 7, from bit 0.
    list_index_bits: u32,
}

/// A catalogue feature's or a dwarf core's slot: eight levels of 16 cells per axis, and a
/// feature-level list of 8,192 entries.
pub(super) const FEATURE_SLOT: NestedSlotLayout = NestedSlotLayout {
    level_bits: 3,
    cell_bits_per_axis: 4,
    levels: 8,
    list_index_bits: 13,
};

/// The galactic centre's slot: twelve levels of 32 cells per axis, and a feature-level list of
/// 65,536 entries (P11.T8.e.b).
pub(super) const CENTRE_SLOT: NestedSlotLayout = NestedSlotLayout {
    level_bits: 4,
    cell_bits_per_axis: 5,
    levels: 12,
    list_index_bits: 16,
};

impl NestedSlotLayout {
    /// The cell field of an axis, 0 for x, 1 for y, 2 for z.
    #[must_use]
    pub(super) const fn cell(self, axis: u32) -> Field {
        Field::above(
            MEMBER_INDEX.hi() + 1 + (2 - axis) * self.cell_bits_per_axis,
            self.cell_bits_per_axis,
        )
    }

    /// The level field.
    #[must_use]
    pub(super) const fn level(self) -> Field {
        Field::above(self.cell(0).hi() + 1, self.level_bits)
    }

    /// The mass-band field.
    #[must_use]
    pub(super) const fn band(self) -> Field {
        Field::above(self.level().hi() + 1, 3)
    }

    /// The feature-level list's index under band 7: `[12:0]` for a catalogue feature or a dwarf
    /// core, `[15:0]` at the galactic centre.
    #[must_use]
    pub(super) const fn list_index(self) -> Field {
        Field::above(0, self.list_index_bits)
    }

    /// The bits under band 7 between the list's index and the band, which must be zero: `[27:13]`
    /// for a catalogue feature or a dwarf core (its level and cell), `[31:16]` at the centre.
    #[must_use]
    pub(super) const fn list_padding(self) -> Field {
        Field::new(self.band().lo() - 1, self.list_index().hi() + 1)
    }

    /// The exclusive bound of the feature-level list's index: 8,192, or 65,536 at the centre.
    #[must_use]
    pub(super) const fn list_index_limit(self) -> u32 {
        1 << self.list_index_bits
    }

    /// Cells per axis in a level.
    #[must_use]
    pub(super) const fn cells_per_axis(self) -> u64 {
        1 << self.cell_bits_per_axis
    }

    /// Whether a cell coordinate lies in the inner half of its level: 4–11 of 16, 8–23 of 32.
    #[must_use]
    const fn is_inner(self, c: u64) -> bool {
        let n = self.cells_per_axis();
        n / 4 <= c && c < 3 * n / 4
    }

    /// Validates the slot fields of `raw`.
    ///
    /// # Errors
    ///
    /// In this order: [`DecodeSystemIdError::FeatureLevelFieldsSet`] for band 7 with a bit set
    /// between the list's index and the band ([`list_padding`](Self::list_padding): the level
    /// and cell, `[27:13]`, of a catalogue feature or a dwarf core, `[31:16]` at the centre);
    /// [`DecodeSystemIdError::LevelOutOfRange`] for a level beyond the grid's;
    /// [`DecodeSystemIdError::InnerCellOwnedByLowerLevel`] for a cell of level 1 or more whose
    /// three coordinates all lie in the inner half.
    pub(super) const fn validate(self, raw: u64) -> Result<(), DecodeSystemIdError> {
        if self.band().get(raw) == FEATURE_LEVEL_BAND {
            if !self.list_padding().is_zero(raw) {
                return Err(DecodeSystemIdError::FeatureLevelFieldsSet);
            }
            return Ok(());
        }
        let level = self.level().get(raw);
        let [x, y, z] = [
            self.cell(0).get(raw),
            self.cell(1).get(raw),
            self.cell(2).get(raw),
        ];
        if level >= self.levels {
            return Err(DecodeSystemIdError::LevelOutOfRange);
        }
        if level >= 1 && self.is_inner(x) && self.is_inner(y) && self.is_inner(z) {
            return Err(DecodeSystemIdError::InnerCellOwnedByLowerLevel);
        }
        Ok(())
    }

    /// The slot of a validated `raw`: under band 7 the list's index
    /// ([`list_index`](Self::list_index)), in a cell [`MEMBER_INDEX`].
    #[must_use]
    pub(super) fn slot(self, raw: u64) -> MemberSlot {
        match Layer::from_value(self.band().get_u8(raw)) {
            Some(band) => MemberSlot::InCell {
                band,
                level: self.level().get_u8(raw),
                cell: [0, 1, 2].map(|axis| self.cell(axis).get_u8(raw)),
                index: MEMBER_INDEX.get_u16(raw),
            },
            None => MemberSlot::FeatureLevel {
                index: self.list_index().get_u16(raw),
            },
        }
    }

    /// `raw` with the slot fields set, after checking each part against its field's range.
    ///
    /// The layout's rules (the inner-cell rule) are left to [`validate`](Self::validate).
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::IndexTooLarge`] for an index in a cell of 8,192 or more, or a
    /// feature-level index at or past the list's [`list_index_limit`](Self::list_index_limit)
    /// (8,192; at the centre 65,536, which no `u16` reaches);
    /// [`BuildSystemIdError::FieldOutOfRange`] for a level or cell coordinate beyond the grid's.
    pub(super) fn with_slot(self, raw: u64, slot: MemberSlot) -> Result<u64, BuildSystemIdError> {
        match slot {
            MemberSlot::InCell {
                band,
                level,
                cell,
                index,
            } => {
                if u64::from(level) >= self.levels {
                    return Err(BuildSystemIdError::FieldOutOfRange { field: "level" });
                }
                if cell.iter().any(|&c| u64::from(c) >= self.cells_per_axis()) {
                    return Err(BuildSystemIdError::FieldOutOfRange {
                        field: "nested cell",
                    });
                }
                if index >= MemberSlot::INDEX_LIMIT {
                    return Err(BuildSystemIdError::IndexTooLarge);
                }
                let mut raw = self.band().with(raw, u64::from(band.value()));
                raw = self.level().with(raw, u64::from(level));
                for (axis, c) in (0..3).zip(cell) {
                    raw = self.cell(axis).with(raw, u64::from(c));
                }
                Ok(MEMBER_INDEX.with(raw, u64::from(index)))
            }
            MemberSlot::FeatureLevel { index } => {
                if u32::from(index) >= self.list_index_limit() {
                    return Err(BuildSystemIdError::IndexTooLarge);
                }
                let raw = self.band().with(raw, FEATURE_LEVEL_BAND);
                let raw = self.list_padding().with(raw, 0);
                Ok(self.list_index().with(raw, u64::from(index)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_fields_sit_where_the_tables_put_them() {
        let pairs = |layout: NestedSlotLayout| {
            [
                layout.band(),
                layout.level(),
                layout.cell(0),
                layout.cell(1),
                layout.cell(2),
                MEMBER_INDEX,
            ]
            .map(|f| (f.hi(), f.lo()))
        };
        assert_eq!(
            pairs(FEATURE_SLOT),
            [(30, 28), (27, 25), (24, 21), (20, 17), (16, 13), (12, 0)]
        );
        assert_eq!(
            pairs(CENTRE_SLOT),
            [(34, 32), (31, 28), (27, 23), (22, 18), (17, 13), (12, 0)]
        );
        // Band 7's view: the zero bits, then the list's index.
        let list = |layout: NestedSlotLayout| {
            [layout.list_padding(), layout.list_index()].map(|f| (f.hi(), f.lo()))
        };
        assert_eq!(list(FEATURE_SLOT), [(27, 13), (12, 0)]);
        assert_eq!(list(CENTRE_SLOT), [(31, 16), (15, 0)]);
        assert_eq!(FEATURE_SLOT.list_index_limit(), 8_192);
        assert_eq!(CENTRE_SLOT.list_index_limit(), 65_536);
    }

    #[test]
    fn level_zero_accepts_every_cell_and_level_three_accepts_the_shell() {
        let raw_at = |level: u64, [x, y, z]: [u64; 3]| {
            let mut raw = FEATURE_SLOT.level().with(0, level);
            raw = FEATURE_SLOT.cell(0).with(raw, x);
            raw = FEATURE_SLOT.cell(1).with(raw, y);
            FEATURE_SLOT.cell(2).with(raw, z)
        };
        let accepted = |level| {
            let mut n = 0;
            for x in 0..16 {
                for y in 0..16 {
                    for z in 0..16 {
                        if FEATURE_SLOT.validate(raw_at(level, [x, y, z])).is_ok() {
                            n += 1;
                        }
                    }
                }
            }
            n
        };
        assert_eq!(accepted(0), 4_096);
        assert_eq!(accepted(3), 16 * 16 * 16 - 8 * 8 * 8);
        assert_eq!(accepted(3), 3_584);
        assert_eq!(
            FEATURE_SLOT.validate(raw_at(1, [4, 11, 7])),
            Err(DecodeSystemIdError::InnerCellOwnedByLowerLevel)
        );
        assert_eq!(FEATURE_SLOT.validate(raw_at(1, [3, 11, 7])), Ok(()));
        assert_eq!(FEATURE_SLOT.validate(raw_at(1, [4, 12, 7])), Ok(()));
    }

    #[test]
    fn the_centre_uses_the_inner_range_8_to_23_and_twelve_levels() {
        let raw_at = |level: u64, c: u64| {
            let mut raw = CENTRE_SLOT.level().with(0, level);
            for axis in 0..3 {
                raw = CENTRE_SLOT.cell(axis).with(raw, c);
            }
            raw
        };
        assert_eq!(
            CENTRE_SLOT.validate(raw_at(11, 8)),
            Err(DecodeSystemIdError::InnerCellOwnedByLowerLevel)
        );
        assert_eq!(
            CENTRE_SLOT.validate(raw_at(1, 23)),
            Err(DecodeSystemIdError::InnerCellOwnedByLowerLevel)
        );
        assert_eq!(CENTRE_SLOT.validate(raw_at(11, 7)), Ok(()));
        assert_eq!(CENTRE_SLOT.validate(raw_at(11, 24)), Ok(()));
        assert_eq!(CENTRE_SLOT.validate(raw_at(0, 16)), Ok(()));
        for level in 12..16 {
            assert_eq!(
                CENTRE_SLOT.validate(raw_at(level, 0)),
                Err(DecodeSystemIdError::LevelOutOfRange)
            );
        }
    }

    /// Band 7 accepts every index of its list and rejects every bit between the index and the
    /// band: a catalogue feature's or a dwarf core's level and cell, `[27:13]`, as before, and at
    /// the centre `[31:16]`, whose low three bits of the cell's z, `[15:13]`, are now the index's.
    #[test]
    fn band_seven_requires_zero_bits_between_the_list_index_and_the_band() {
        for (layout, indices) in [
            (FEATURE_SLOT, &[0, 1, 8_191][..]),
            (CENTRE_SLOT, &[0, 1, 8_191, 8_192, 65_535][..]),
        ] {
            let base = layout.band().with(0, FEATURE_LEVEL_BAND);
            for &index in indices {
                let raw = layout.list_index().with(base, index);
                assert_eq!(layout.validate(raw), Ok(()), "{index}");
                let slot = MemberSlot::FeatureLevel {
                    index: u16::try_from(index).unwrap(),
                };
                assert_eq!(layout.slot(raw), slot);
            }
            let padding = layout.list_padding();
            for bit in padding.lo()..=padding.hi() {
                assert_eq!(
                    layout.validate(base | (1 << bit)),
                    Err(DecodeSystemIdError::FeatureLevelFieldsSet),
                    "bit {bit}"
                );
            }
            // The level, and each cell coordinate at its largest, reach above the index.
            for f in [
                layout.level(),
                layout.cell(0),
                layout.cell(1),
                layout.cell(2),
            ] {
                assert_eq!(
                    layout.validate(f.with(base, f.max())),
                    Err(DecodeSystemIdError::FeatureLevelFieldsSet)
                );
            }
        }
        // For a catalogue feature index 8,192 is bit 13, its cell's z; at the centre it is the
        // index's own bit 13.
        let feature = FEATURE_SLOT.band().with(0, FEATURE_LEVEL_BAND);
        assert_eq!(
            FEATURE_SLOT.validate(FEATURE_SLOT.cell(2).with(feature, 1)),
            Err(DecodeSystemIdError::FeatureLevelFieldsSet)
        );
        let centre = CENTRE_SLOT.band().with(0, FEATURE_LEVEL_BAND);
        let centre = CENTRE_SLOT.cell(2).with(centre, 1);
        assert_eq!(CENTRE_SLOT.validate(centre), Ok(()));
        assert_eq!(
            CENTRE_SLOT.slot(centre),
            MemberSlot::FeatureLevel { index: 8_192 }
        );
    }

    #[test]
    fn slots_round_trip_and_ranges_are_checked() {
        let slots = [
            MemberSlot::FeatureLevel { index: 0 },
            MemberSlot::FeatureLevel { index: 8_191 },
            MemberSlot::InCell {
                band: Layer::RoguePlanet,
                level: 7,
                cell: [15, 0, 15],
                index: 8_191,
            },
            MemberSlot::InCell {
                band: Layer::A,
                level: 0,
                cell: [5, 5, 5],
                index: 0,
            },
        ];
        for slot in slots {
            let raw = FEATURE_SLOT.with_slot(0, slot).unwrap();
            assert_eq!(FEATURE_SLOT.validate(raw), Ok(()));
            assert_eq!(FEATURE_SLOT.slot(raw), slot);
        }
        let in_cell = |level, cell, index| MemberSlot::InCell {
            band: Layer::B,
            level,
            cell,
            index,
        };
        assert_eq!(
            FEATURE_SLOT.with_slot(0, in_cell(8, [0; 3], 0)),
            Err(BuildSystemIdError::FieldOutOfRange { field: "level" })
        );
        assert_eq!(
            CENTRE_SLOT.with_slot(0, in_cell(12, [0; 3], 0)),
            Err(BuildSystemIdError::FieldOutOfRange { field: "level" })
        );
        assert_eq!(
            FEATURE_SLOT.with_slot(0, in_cell(0, [16, 0, 0], 0)),
            Err(BuildSystemIdError::FieldOutOfRange {
                field: "nested cell"
            })
        );
        assert!(CENTRE_SLOT.with_slot(0, in_cell(0, [31, 0, 0], 0)).is_ok());
        assert_eq!(
            FEATURE_SLOT.with_slot(0, in_cell(0, [0; 3], 8_192)),
            Err(BuildSystemIdError::IndexTooLarge)
        );
        assert_eq!(
            FEATURE_SLOT.with_slot(0, MemberSlot::FeatureLevel { index: 8_192 }),
            Err(BuildSystemIdError::IndexTooLarge)
        );
        // In a cell the centre's index is 13 bits, as every layout's.
        assert_eq!(
            CENTRE_SLOT.with_slot(0, in_cell(0, [0; 3], 8_192)),
            Err(BuildSystemIdError::IndexTooLarge)
        );
        // On the centre's list every u16 builds and round-trips: 65,536 is no u16, and is tested
        // where it can be written, as bit 16 of a raw ID (above) and as a designation.
        for index in [0, 8_191, 8_192, u16::MAX] {
            let slot = MemberSlot::FeatureLevel { index };
            let raw = CENTRE_SLOT.with_slot(0, slot).unwrap();
            assert_eq!(raw, (7 << 32) | u64::from(index));
            assert_eq!(CENTRE_SLOT.validate(raw), Ok(()));
            assert_eq!(CENTRE_SLOT.slot(raw), slot);
        }
        // Building a list member writes every slot bit, the zero bits above its index included.
        let every_slot_bit = (1 << 35) - 1;
        assert_eq!(
            CENTRE_SLOT.with_slot(every_slot_bit, MemberSlot::FeatureLevel { index: 5 }),
            Ok((7 << 32) | 5)
        );
    }
}
