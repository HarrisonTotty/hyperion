//! Pinned content (prefix `110`) and catalogue systems (prefix `111`).

use super::bits::{Field, assert_tiles_64};
use super::layer::LAYER;
use super::reserved::{PREFIX_3, RESERVED};
use super::system::BuildSystemIdError;

/// The prefix of pinned content in [`PREFIX_3`].
pub(super) const PINNED_PREFIX: u64 = 0b110;

/// The prefix of a catalogue system in [`PREFIX_3`].
const CATALOGUE_PREFIX: u64 = 0b111;

/// The pinned number, `[57:0]`.
const PINNED_NUMBER: Field = Field::new(57, 0);

/// The catalogue class, `[57:52]`.
const CLASS: Field = Field::new(57, 52);
/// The catalogue cell x, y, z: 8 bits each.
const CELL: [Field; 3] = [Field::new(51, 44), Field::new(43, 36), Field::new(35, 28)];
/// The candidate number in the catalogue cell, `[27:4]`.
const INDEX: Field = Field::new(27, 4);
/// The member of the entry, `[3:0]`.
const MEMBER: Field = Field::new(3, 0);

const _: () = {
    assert_tiles_64(&[LAYER, PREFIX_3, PINNED_NUMBER]);
    assert_tiles_64(&[
        LAYER, PREFIX_3, CLASS, CELL[0], CELL[1], CELL[2], INDEX, MEMBER,
    ]);
};

/// Pinned content: prefix `110`, then an opaque 58-bit number.
///
/// The overlay design will give the number structure; until then every value is well-formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PinnedId(u64);

impl PinnedId {
    /// The exclusive bound of a pinned number: 2⁵⁸.
    pub const NUMBER_LIMIT: u64 = 1 << 58;

    /// The pinned content with this number.
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::FieldOutOfRange`] for a number of 2⁵⁸ or more.
    pub fn new(number: u64) -> Result<Self, BuildSystemIdError> {
        if number >= Self::NUMBER_LIMIT {
            return Err(BuildSystemIdError::FieldOutOfRange {
                field: "pinned number",
            });
        }
        Ok(Self(
            PINNED_NUMBER.with(PREFIX_3.with(RESERVED, PINNED_PREFIX), number),
        ))
    }

    /// Wraps a validated raw ID with prefix `110`.
    #[must_use]
    pub(super) const fn from_valid_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw ID.
    #[must_use]
    pub(super) const fn raw(self) -> u64 {
        self.0
    }

    /// The pinned number.
    #[must_use]
    pub fn number(self) -> u64 {
        PINNED_NUMBER.get(self.0)
    }
}

/// A catalogue system: prefix `111`, then class, catalogue cell, index and member.
///
/// Catalogue systems are the hosts of rare events, carved out of the grid by their marks and
/// placed on a coarse grid of their own per class (brainstorm, "Events in time"). The cell field
/// has 8 bits per axis of 512 ly, stored as the coordinate plus 128; a class with coarser cells
/// zeroes the low bits, which its resolve checks with [`is_aligned_to`](Self::is_aligned_to).
/// Member 0 is the catalogue entry itself; 1–15 are further members of the entry.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{CatalogueSystemId, SystemId};
///
/// let entry = CatalogueSystemId::new(3, [8, -64, 26], 118, 1)?;
/// assert_eq!(SystemId::from(entry).raw(), 0xFC38_8409_A000_0761);
/// assert!(entry.is_aligned_to(10)); // 1,024 ly cells: even coordinates
/// assert!(!entry.is_aligned_to(11)); // 2,048 ly cells: coordinates divisible by 4
/// # Ok::<(), hyperion_sim::id::BuildSystemIdError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogueSystemId(u64);

impl CatalogueSystemId {
    /// The exclusive bound of a class: 64. The registry of classes belongs to the catalogue.
    pub const CLASS_LIMIT: u8 = 1 << 6;
    /// The edge of the finest catalogue cell in light-years.
    pub const CELL_EDGE_LY: u32 = 512;
    /// The exclusive bound of the candidate number: 2²⁴.
    pub const INDEX_LIMIT: u32 = 1 << 24;
    /// The exclusive bound of the member: 16.
    pub const MEMBER_LIMIT: u8 = 1 << 4;

    /// The index and member fields together, `[27:0]`, zeroed in a cell word.
    pub(super) const INDEX_AND_MEMBER: Field = Field::new(27, 0);

    /// The offset added to a cell coordinate to store it.
    const OFFSET: i32 = 128;

    /// Member `member` of candidate `index` of `class` in the 512 ly catalogue cell `cell`.
    ///
    /// # Errors
    ///
    /// - [`BuildSystemIdError::FieldOutOfRange`] for a class of 64 or more, or a member of 16 or
    ///   more.
    /// - [`BuildSystemIdError::CellOutsideRootCube`] unless every coordinate is in −128..=127.
    /// - [`BuildSystemIdError::IndexTooLarge`] for an index of 2²⁴ or more.
    pub fn new(
        class: u8,
        cell: [i32; 3],
        index: u32,
        member: u8,
    ) -> Result<Self, BuildSystemIdError> {
        if class >= Self::CLASS_LIMIT {
            return Err(BuildSystemIdError::FieldOutOfRange {
                field: "catalogue class",
            });
        }
        if member >= Self::MEMBER_LIMIT {
            return Err(BuildSystemIdError::FieldOutOfRange {
                field: "catalogue member",
            });
        }
        if index >= Self::INDEX_LIMIT {
            return Err(BuildSystemIdError::IndexTooLarge);
        }
        let mut raw = CLASS.with(PREFIX_3.with(RESERVED, CATALOGUE_PREFIX), u64::from(class));
        for (field, c) in CELL.into_iter().zip(cell) {
            let stored = u64::try_from(c + Self::OFFSET)
                .ok()
                .filter(|&s| s <= field.max())
                .ok_or(BuildSystemIdError::CellOutsideRootCube)?;
            raw = field.with(raw, stored);
        }
        raw = INDEX.with(raw, u64::from(index));
        Ok(Self(MEMBER.with(raw, u64::from(member))))
    }

    /// Wraps a validated raw ID with prefix `111`.
    #[must_use]
    pub(super) const fn from_valid_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw ID.
    #[must_use]
    pub(super) const fn raw(self) -> u64 {
        self.0
    }

    /// The catalogue class, 0–63.
    #[must_use]
    pub fn class(self) -> u8 {
        CLASS.get_u8(self.0)
    }

    /// The catalogue cell in 512 ly units, x, y, z, each −128..=127.
    #[must_use]
    pub fn cell(self) -> [i32; 3] {
        self.stored_cell().map(|s| i32::from(s) - Self::OFFSET)
    }

    /// The candidate number in the catalogue cell.
    #[must_use]
    pub fn index(self) -> u32 {
        INDEX.get_u32(self.0)
    }

    /// The member of the entry: 0 is the entry itself.
    #[must_use]
    pub fn member(self) -> u8 {
        MEMBER.get_u8(self.0)
    }

    /// Whether the cell is aligned to a class's cell of `2^cell_log2_ly` ly: true when the low
    /// `cell_log2_ly − 9` bits of each axis are zero.
    ///
    /// Every ID is aligned to 512 ly cells (`cell_log2_ly = 9`). Nothing is aligned to a size
    /// below 512 ly, which the field cannot express. Cells of 2¹⁶ ly are the largest that tile
    /// the root cube with the coordinate planes as faces.
    #[must_use]
    pub fn is_aligned_to(self, cell_log2_ly: u32) -> bool {
        let Some(low_bits) = cell_log2_ly.checked_sub(9) else {
            return false;
        };
        // Two's complement: the low bits of a negative coordinate are zero exactly when it is a
        // multiple of the cell size, as for a positive one.
        // Beyond 62 bits the mask saturates to every bit, which only 0 passes, as it should.
        let mask = 1_i64
            .checked_shl(low_bits)
            .and_then(|bit| bit.checked_sub(1))
            .unwrap_or(-1);
        self.cell().iter().all(|&c| i64::from(c) & mask == 0)
    }

    /// The stored (offset, unsigned) cell coordinates, each 0–255.
    #[must_use]
    pub(super) fn stored_cell(self) -> [u8; 3] {
        CELL.map(|f| f.get_u8(self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{SystemId, SystemIdKind};

    #[test]
    fn hand_computed_pinned_and_catalogue_ids() {
        let pin = PinnedId::new(42).unwrap();
        assert_eq!(pin.raw(), (0b111_110 << 58) | 0x2A);
        assert_eq!(pin.raw(), 0xF800_0000_0000_002A);
        let last = PinnedId::new(PinnedId::NUMBER_LIMIT - 1).unwrap();
        assert_eq!(last.raw(), 0xFBFF_FFFF_FFFF_FFFF);

        // Class 3, stored cell (136, 61, 154), index 118, member 1.
        let entry = CatalogueSystemId::new(3, [8, -67, 26], 118, 1).unwrap();
        assert_eq!(
            entry.raw(),
            (0b111_111 << 58) | (3 << 52) | (136 << 44) | (61 << 36) | (154 << 28) | (118 << 4) | 1
        );
        assert_eq!(entry.raw(), 0xFC38_83D9_A000_0761);
    }

    #[test]
    fn every_pattern_under_the_two_prefixes_decodes_and_round_trips() {
        let mut lcg = hyperion_testkit::lcg::Lcg::new(110);
        for _ in 0..10_000 {
            let bits = lcg.next_u64() & ((1 << 58) - 1);
            let pinned = SystemId::from_raw((0b111_110 << 58) | bits).unwrap();
            let SystemIdKind::Pinned(p) = pinned.kind() else {
                panic!("{pinned:?} is pinned");
            };
            assert_eq!(PinnedId::new(p.number()), Ok(p));

            let catalogue = SystemId::from_raw((0b111_111 << 58) | bits).unwrap();
            let SystemIdKind::Catalogue(c) = catalogue.kind() else {
                panic!("{catalogue:?} is a catalogue system");
            };
            assert_eq!(
                CatalogueSystemId::new(c.class(), c.cell(), c.index(), c.member()),
                Ok(c)
            );
        }
    }

    #[test]
    fn builders_check_ranges() {
        assert_eq!(
            PinnedId::new(1 << 58),
            Err(BuildSystemIdError::FieldOutOfRange {
                field: "pinned number"
            })
        );
        assert_eq!(
            CatalogueSystemId::new(64, [0; 3], 0, 0),
            Err(BuildSystemIdError::FieldOutOfRange {
                field: "catalogue class"
            })
        );
        assert_eq!(
            CatalogueSystemId::new(0, [0; 3], 0, 16),
            Err(BuildSystemIdError::FieldOutOfRange {
                field: "catalogue member"
            })
        );
        assert_eq!(
            CatalogueSystemId::new(0, [0; 3], 1 << 24, 0),
            Err(BuildSystemIdError::IndexTooLarge)
        );
        assert_eq!(
            CatalogueSystemId::new(0, [128, 0, 0], 0, 0),
            Err(BuildSystemIdError::CellOutsideRootCube)
        );
        assert_eq!(
            CatalogueSystemId::new(0, [0, -129, 0], 0, 0),
            Err(BuildSystemIdError::CellOutsideRootCube)
        );
        let corner = CatalogueSystemId::new(63, [-128, 127, -128], (1 << 24) - 1, 15).unwrap();
        assert_eq!(corner.cell(), [-128, 127, -128]);
    }

    #[test]
    fn alignment_reads_the_low_bits_of_each_axis() {
        let at = |cell| CatalogueSystemId::new(0, cell, 0, 0).unwrap();
        let odd = at([1, 0, 0]);
        assert!(odd.is_aligned_to(9));
        assert!(!odd.is_aligned_to(10));
        assert!(!odd.is_aligned_to(8));
        let quad = at([-4, 8, 0]);
        assert!(quad.is_aligned_to(11));
        assert!(!quad.is_aligned_to(12));
        let coarse = at([-128, 0, 0]);
        assert!(coarse.is_aligned_to(16));
        assert!(!at([64, 0, 0]).is_aligned_to(16));
        assert!(at([0, 0, 0]).is_aligned_to(40));
        assert!(!coarse.is_aligned_to(40));
    }
}
