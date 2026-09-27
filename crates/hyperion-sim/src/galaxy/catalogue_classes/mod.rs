//! Catalogue classes: rare hosts carved out of the cells and placed on grids of their own under
//! the `111` prefix (brainstorm, "Events in time"; plan 09, phase 7).
//!
//! This module holds, so far, what phase 1 builds (P09.T1): the class registry
//! ([`registry`], re-exported here) and the key of a catalogue-class cell ([`CatalogueCellKey`]).
//! The class grid, the processes and the carve-out follow in P09.T32–T35.

pub mod registry;

use std::error::Error;
use std::fmt;

pub use self::registry::{ALL_CLASSES, ClassId, FEATURE_LEVEL_LIST_ORDER};
use crate::coords::{GalacticPosition, ROOT_HALF_WIDTH_LY};
use crate::id::CatalogueSystemId;

/// A catalogue-class cell: a class and a cell at the class's own size (plan 09, P09.T1).
///
/// The cell's coordinates count cells of the class's size, `2^cell_log2_ly` ly, from the origin, so
/// that the planes x = 0, y = 0 and z = 0 are cell faces. Its [`word`](Self::word) is the ID of the
/// cell's candidate 0 with the index and member zeroed, which keys the cell's streams, as plan 03
/// does for the grid (Design note 23).
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

    /// The cell's word: the raw ID of its candidate 0, member 0, which keys its streams.
    ///
    /// # Panics
    ///
    /// Never: the key's class and cell were checked when it was built.
    #[must_use]
    pub fn word(&self) -> u64 {
        let id = CatalogueSystemId::new(self.class.value(), self.fine_cell(), 0, 0)
            .expect("a checked class cell makes a valid catalogue ID");
        crate::id::SystemId::from(id).raw()
    }

    fn log2(&self) -> u32 {
        self.class
            .cell_log2_ly()
            .expect("a key is only built for a placed class")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
