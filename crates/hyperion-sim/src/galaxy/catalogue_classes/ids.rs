//! The identifiers of the catalogue-class grid: a class cell's key, and the conversions between a
//! key and the `111` IDs of its candidates (plan 09, P09.T1 and P09.T32).
//!
//! Plan 01's [`CatalogueSystemId`] lays out a `111` ID: a 6-bit class, a cell of 8 bits per axis
//! of 512 ly, a 24-bit candidate index and a 4-bit member (brainstorm, "Identifiers", the `111`
//! row: "Class (6), cell (24, at 8 bits per axis of 512 ly; coarser classes zero the low bits),
//! index (24), member (4)"). Its decode accepts every class value and every cell, and leaves the
//! canonical rule to the catalogue: an ID names a class cell only if its class is registered and
//! placed and its cell is aligned to the class's cell size ([`CatalogueCellKey::of`]), so that a
//! misaligned ID never aliases an aligned one.
//!
//! # The cell field
//!
//! The root cube spans ±65,536 ly ([`ROOT_HALF_WIDTH_LY`]), which is 256 cells of 512 ly per
//! axis: the 8 bits of each axis of the ID. A class whose cells are 2ᵏ × 512 ly has 256 ÷ 2ᵏ of
//! them per axis, 32 at 4,096 ly, and stores each of its coordinates shifted up by k, so that the
//! low k bits are zero ([`CatalogueCellKey::fine_cell`]).
//!
//! # A candidate's ID
//!
//! Candidate i of a cell is `candidate_id(i, 0)`: the class, the cell, the index i and member 0,
//! the entry itself. Members 1–15 are the entry's further members, such as P09.T34.a's surviving
//! donor (member 1). The cell's [`word`](CatalogueCellKey::word) is its candidate 0's ID with the
//! index and member zeroed, which is [`SystemId::cell_word`] of every one of its IDs.

use std::error::Error;
use std::fmt;

use super::registry::ClassId;
use crate::coords::{GalacticPosition, ROOT_HALF_WIDTH_LY};
use crate::id::{BuildSystemIdError, CatalogueSystemId, SystemId};

/// A catalogue-class cell: a class and a cell at the class's own size (plan 09, P09.T1).
///
/// The cell's coordinates count cells of the class's size, `2^cell_log2_ly` ly, from the origin, so
/// that the planes x = 0, y = 0 and z = 0 are cell faces. Its [`word`](Self::word) is the ID of the
/// cell's candidate 0 with the index and member zeroed, which keys the cell's streams, as plan 03
/// does for the grid (Design note 23). Keys order by class, then x, then y, then z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogueCellKey {
    class: ClassId,
    cell: [i32; 3],
}

/// A [`CatalogueCellKey`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildCatalogueCellKeyError {
    /// The class is never placed under the `111` prefix
    /// ([`ClassId::TIDAL_DISRUPTION_VICTIM`]).
    NotPlaced(ClassId),
    /// A coordinate lies outside the root cube at the class's cell size.
    OutsideRootCube {
        /// The axis, 0 for x, 1 for y, 2 for z.
        axis: usize,
        /// The offending coordinate, in cells of the class's size.
        coordinate: i32,
    },
    /// An ID's class value is free: no class is registered under it ([`CatalogueCellKey::of`]).
    FreeClass(u8),
    /// An ID's cell is not aligned to its class's cell size, so it names no cell of the class
    /// ([`CatalogueCellKey::of`]): plan 01's canonical rule, which the catalogue keeps.
    Misaligned {
        /// The ID's class.
        class: ClassId,
        /// The ID's cell, in 512 ly cells.
        cell: [i32; 3],
    },
}

impl fmt::Display for BuildCatalogueCellKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPlaced(class) => {
                write!(f, "the class {class} is never placed under the 111 prefix")
            }
            Self::OutsideRootCube { axis, coordinate } => write!(
                f,
                "catalogue cell coordinate {coordinate} on axis {axis} lies outside the root cube"
            ),
            Self::FreeClass(value) => write!(f, "no catalogue class has the value {value}"),
            Self::Misaligned {
                class,
                cell: [x, y, z],
            } => write!(
                f,
                "the 512 ly cell ({x}, {y}, {z}) is not aligned to the cells of the class {class}"
            ),
        }
    }
}

impl Error for BuildCatalogueCellKeyError {}

impl CatalogueCellKey {
    /// The cell `cell` of `class`, in cells of the class's size.
    ///
    /// # Errors
    ///
    /// [`BuildCatalogueCellKeyError::NotPlaced`] for a class with no cell size, and
    /// [`BuildCatalogueCellKeyError::OutsideRootCube`] for a coordinate outside the root cube.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
    ///
    /// let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [1, -2, 0])?;
    /// // A 4,096 ly class cell is eight 512 ly cells on each axis.
    /// assert_eq!(key.fine_cell(), [8, -16, 0]);
    /// # Ok::<(), hyperion_sim::galaxy::catalogue_classes::BuildCatalogueCellKeyError>(())
    /// ```
    pub fn new(class: ClassId, cell: [i32; 3]) -> Result<Self, BuildCatalogueCellKeyError> {
        let log2 = class
            .cell_log2_ly()
            .ok_or(BuildCatalogueCellKeyError::NotPlaced(class))?;
        let half = ROOT_HALF_WIDTH_LY >> log2;
        for (axis, &coordinate) in cell.iter().enumerate() {
            if !(-half..half).contains(&coordinate) {
                return Err(BuildCatalogueCellKeyError::OutsideRootCube { axis, coordinate });
            }
        }
        Ok(Self { class, cell })
    }

    /// The class cell a `111` ID names, the check every catalogue ID from outside goes through
    /// before it is resolved (P09.T32).
    ///
    /// Plan 01's decode accepts every class value and every cell; this is the canonical rule it
    /// leaves to the catalogue. The cell is the ID's 512 ly cell shifted down by the class's
    /// `cell_log2_ly − 9`, exactly, since its low bits are zero.
    ///
    /// # Errors
    ///
    /// - [`BuildCatalogueCellKeyError::FreeClass`] for a class value no class is registered under
    ///   (9–63).
    /// - [`BuildCatalogueCellKeyError::NotPlaced`] for
    ///   [`TIDAL_DISRUPTION_VICTIM`](ClassId::TIDAL_DISRUPTION_VICTIM), which is never placed.
    /// - [`BuildCatalogueCellKeyError::Misaligned`] for a cell whose low bits are not zero at the
    ///   class's size ([`CatalogueSystemId::is_aligned_to`]).
    ///
    /// # Examples
    ///
    /// A 4,096 ly class's ID read from a save names a cell only if its 512 ly coordinates are
    /// multiples of eight; a twin with a low bit set names nothing:
    ///
    /// ```
    /// use hyperion_sim::galaxy::catalogue_classes::{
    ///     BuildCatalogueCellKeyError, CatalogueCellKey, ClassId,
    /// };
    /// use hyperion_sim::id::CatalogueSystemId;
    ///
    /// let merger = ClassId::STELLAR_MERGER;
    /// let aligned = CatalogueSystemId::new(merger.value(), [8, -16, 0], 3, 0)?;
    /// let key = CatalogueCellKey::of(aligned)?;
    /// assert_eq!(key.cell(), [1, -2, 0]);
    /// assert_eq!(key.candidate_id(3, 0)?, aligned);
    ///
    /// let twin = CatalogueSystemId::new(merger.value(), [9, -16, 0], 3, 0)?;
    /// assert_eq!(
    ///     CatalogueCellKey::of(twin),
    ///     Err(BuildCatalogueCellKeyError::Misaligned { class: merger, cell: [9, -16, 0] })
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn of(id: CatalogueSystemId) -> Result<Self, BuildCatalogueCellKeyError> {
        let class = ClassId::from_value(id.class())
            .ok_or(BuildCatalogueCellKeyError::FreeClass(id.class()))?;
        let log2 = class
            .cell_log2_ly()
            .ok_or(BuildCatalogueCellKeyError::NotPlaced(class))?;
        let fine = id.cell();
        if !id.is_aligned_to(log2) {
            return Err(BuildCatalogueCellKeyError::Misaligned { class, cell: fine });
        }
        // The low bits are zero, so the arithmetic shift is exact for a negative coordinate too.
        Self::new(class, fine.map(|c| c >> (log2 - 9)))
    }

    /// The cell of `class` that holds `position`, or `None` for a class that is never placed.
    #[must_use]
    pub fn containing(class: ClassId, position: &GalacticPosition) -> Option<Self> {
        let log2 = class.cell_log2_ly()?;
        let cell = position.cell().to_array().map(|ly| ly >> log2);
        Self::new(class, cell).ok()
    }

    /// The class.
    #[must_use]
    pub const fn class(&self) -> ClassId {
        self.class
    }

    /// The cell's coordinates, in cells of the class's size.
    #[must_use]
    pub const fn cell(&self) -> [i32; 3] {
        self.cell
    }

    /// The cell's coordinates in 512 ly cells, as a `111` ID stores them: the class's coordinates
    /// shifted up by `cell_log2_ly − 9`, so that the low bits are zero.
    #[must_use]
    pub fn fine_cell(&self) -> [i32; 3] {
        let shift = self.log2() - 9;
        self.cell.map(|c| c << shift)
    }

    /// The cell's edge in light-years.
    #[must_use]
    pub fn edge_ly(&self) -> u32 {
        1 << self.log2()
    }

    /// The cell's volume in cubic light-years: 2²⁷ at 512 ly, 2³⁶ at 4,096 ly, each exact.
    #[must_use]
    pub fn volume_ly3(&self) -> f64 {
        let edge = f64::from(self.edge_ly());
        edge * edge * edge
    }

    /// The least corner of the cell's closed box, in whole light-years.
    #[must_use]
    pub fn origin_ly(&self) -> [i32; 3] {
        let log2 = self.log2();
        self.cell.map(|c| c << log2)
    }

    /// Member `member` of candidate `index` of the cell: `candidate_id(index, 0)` is the
    /// candidate's entry, the ID its streams are keyed by.
    ///
    /// # Errors
    ///
    /// [`BuildSystemIdError::IndexTooLarge`] for an index of 2²⁴ or more, and
    /// [`BuildSystemIdError::FieldOutOfRange`] for a member of 16 or more. The class and the cell
    /// were checked when the key was built.
    pub fn candidate_id(
        &self,
        index: u32,
        member: u8,
    ) -> Result<CatalogueSystemId, BuildSystemIdError> {
        CatalogueSystemId::new(self.class.value(), self.fine_cell(), index, member)
    }

    /// The cell's word: the raw ID of its candidate 0, member 0, which keys its streams.
    ///
    /// # Panics
    ///
    /// Never: the key's class and cell were checked when it was built.
    #[must_use]
    pub fn word(&self) -> u64 {
        let id = self
            .candidate_id(0, 0)
            .expect("a checked class cell makes a valid catalogue ID");
        SystemId::from(id).raw()
    }

    fn log2(&self) -> u32 {
        self.class
            .cell_log2_ly()
            .expect("a key is only built for a placed class")
    }
}

/// The entry an ID's member belongs to: the same class, cell and index, member 0.
///
/// # Panics
///
/// Never: every field but the member is the ID's own, and member 0 is in range.
#[must_use]
pub(super) fn entry_of(id: CatalogueSystemId) -> CatalogueSystemId {
    CatalogueSystemId::new(id.class(), id.cell(), id.index(), 0)
        .expect("an ID's own class, cell and index with member 0 make a valid ID")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::catalogue_classes::ALL_CLASSES;

    #[test]
    fn keys_cover_the_root_cube_at_every_size_and_are_aligned() {
        for class in ALL_CLASSES {
            let Some(log2) = class.cell_log2_ly() else {
                assert_eq!(
                    CatalogueCellKey::new(class, [0, 0, 0]),
                    Err(BuildCatalogueCellKeyError::NotPlaced(class))
                );
                continue;
            };
            let half = ROOT_HALF_WIDTH_LY >> log2;
            for cell in [[-half, 0, half - 1], [half - 1, -half, 0], [0, 0, 0]] {
                let key = CatalogueCellKey::new(class, cell).unwrap();
                let id = CatalogueSystemId::new(class.value(), key.fine_cell(), 0, 0).unwrap();
                assert!(id.is_aligned_to(log2), "{class} {cell:?}");
                assert_eq!(key.edge_ly(), 1 << log2);
            }
            assert!(CatalogueCellKey::new(class, [half, 0, 0]).is_err());
            assert!(CatalogueCellKey::new(class, [0, -half - 1, 0]).is_err());
        }
    }

    #[test]
    fn containing_agrees_with_the_cell_edges() {
        let p = GalacticPosition::from_light_years([-1.5, 5_000.2, 4_095.9]).unwrap();
        let key = CatalogueCellKey::containing(ClassId::CORE_COLLAPSE, &p).unwrap();
        assert_eq!(key.cell(), [-1, 9, 7]);
        let key = CatalogueCellKey::containing(ClassId::STELLAR_MERGER, &p).unwrap();
        assert_eq!(key.cell(), [-1, 1, 0]);
        assert_eq!(
            CatalogueCellKey::containing(ClassId::TIDAL_DISRUPTION_VICTIM, &p),
            None
        );
    }

    #[test]
    fn words_differ_between_classes_and_cells() {
        let a = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, 0, 0]).unwrap();
        let b = CatalogueCellKey::new(ClassId::TYPE_IA, [0, 0, 0]).unwrap();
        let c = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, 1, 0]).unwrap();
        assert_ne!(a.word(), b.word());
        assert_ne!(a.word(), c.word());
    }

    /// The cell field's figures (P09.T32, issue #127): 256 cells of 512 ly per axis span the
    /// root cube's ±65,536 ly, and 32 of 4,096 ly.
    #[test]
    fn the_cell_field_spans_the_root_cube_in_256_cells_of_512_ly() {
        let per_axis = |class: ClassId| {
            let log2 = class.cell_log2_ly().unwrap();
            2 * (ROOT_HALF_WIDTH_LY >> log2)
        };
        assert_eq!(ROOT_HALF_WIDTH_LY, 65_536);
        assert_eq!(per_axis(ClassId::CORE_COLLAPSE), 256);
        assert_eq!(per_axis(ClassId::STELLAR_MERGER), 32);
        assert_eq!(256, 1 << 8, "8 bits per axis");
        let corner = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [-128, 127, 0]).unwrap();
        assert_eq!(corner.origin_ly(), [-65_536, 65_024, 0]);
        assert_eq!(corner.volume_ly3(), 134_217_728.0);
        let coarse = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [-16, 15, 0]).unwrap();
        assert_eq!(coarse.origin_ly(), [-65_536, 61_440, 0]);
        assert_eq!(coarse.volume_ly3(), 68_719_476_736.0);
    }

    /// Every ID of every placed class and aligned cell maps back to its key; every free value,
    /// the unplaced class and every misaligned cell is refused with its own variant.
    #[test]
    fn of_inverts_candidate_id_and_refuses_what_is_not_canonical() {
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x0932);
        for class in ALL_CLASSES {
            for _ in 0..200 {
                let fine = [0, 1, 2].map(|_| i32::try_from(lcg.next_below(256)).unwrap() - 128);
                let index = u32::try_from(lcg.next_below(1 << 24)).unwrap();
                let member = u8::try_from(lcg.next_below(16)).unwrap();
                let id = CatalogueSystemId::new(class.value(), fine, index, member).unwrap();
                let Some(log2) = class.cell_log2_ly() else {
                    assert_eq!(
                        CatalogueCellKey::of(id),
                        Err(BuildCatalogueCellKeyError::NotPlaced(class))
                    );
                    continue;
                };
                if id.is_aligned_to(log2) {
                    let key = CatalogueCellKey::of(id).unwrap();
                    assert_eq!(key.class(), class);
                    assert_eq!(key.fine_cell(), fine);
                    assert_eq!(key.candidate_id(index, member), Ok(id));
                    assert_eq!(SystemId::from(id).cell_word(), key.word());
                    assert_eq!(entry_of(id), key.candidate_id(index, 0).unwrap());
                } else {
                    assert_eq!(
                        CatalogueCellKey::of(id),
                        Err(BuildCatalogueCellKeyError::Misaligned { class, cell: fine })
                    );
                }
            }
        }
        for value in 9..CatalogueSystemId::CLASS_LIMIT {
            let id = CatalogueSystemId::new(value, [0, 0, 0], 0, 0).unwrap();
            assert_eq!(
                CatalogueCellKey::of(id),
                Err(BuildCatalogueCellKeyError::FreeClass(value))
            );
        }
    }

    #[test]
    fn candidate_ids_refuse_an_index_or_member_past_the_fields() {
        let key = CatalogueCellKey::new(ClassId::TYPE_IA, [3, -4, 5]).unwrap();
        assert_eq!(
            key.candidate_id(CatalogueSystemId::INDEX_LIMIT, 0),
            Err(BuildSystemIdError::IndexTooLarge)
        );
        assert!(matches!(
            key.candidate_id(0, CatalogueSystemId::MEMBER_LIMIT),
            Err(BuildSystemIdError::FieldOutOfRange { .. })
        ));
        let last = key
            .candidate_id(CatalogueSystemId::INDEX_LIMIT - 1, 15)
            .unwrap();
        assert_eq!(last.index(), CatalogueSystemId::INDEX_LIMIT - 1);
        assert_eq!(last.member(), 15);
    }

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        let messages = [
            BuildCatalogueCellKeyError::NotPlaced(ClassId::TIDAL_DISRUPTION_VICTIM).to_string(),
            BuildCatalogueCellKeyError::OutsideRootCube {
                axis: 1,
                coordinate: 300,
            }
            .to_string(),
            BuildCatalogueCellKeyError::FreeClass(9).to_string(),
            BuildCatalogueCellKeyError::Misaligned {
                class: ClassId::STELLAR_MERGER,
                cell: [1, 0, 0],
            }
            .to_string(),
        ];
        for message in messages {
            let first = message.chars().next().unwrap();
            assert!(!first.is_uppercase(), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
    }
}
