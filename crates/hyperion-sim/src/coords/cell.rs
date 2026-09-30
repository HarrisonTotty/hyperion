//! Light-year cells and generation cells.

use std::error::Error;
use std::fmt;

use super::ROOT_HALF_WIDTH_LY;

/// A 1 ly cube of the galactic frame, named by its low corner in whole light-years.
///
/// The cell `(x, y, z)` spans `[x, x + 1) ly` on the x axis and likewise on y and z, so the
/// planes x, y, z = 0 are cell faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LyCell {
    x: i32,
    y: i32,
    z: i32,
}

impl LyCell {
    /// The cell with the given low corner, in light-years.
    #[must_use]
    pub const fn new([x, y, z]: [i32; 3]) -> Self {
        Self { x, y, z }
    }

    /// The low corner, in light-years, as `[x, y, z]`.
    #[must_use]
    pub const fn to_array(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }

    /// The x coordinate of the low corner, light-years.
    #[must_use]
    pub const fn x(self) -> i32 {
        self.x
    }

    /// The y coordinate of the low corner, light-years.
    #[must_use]
    pub const fn y(self) -> i32 {
        self.y
    }

    /// The z coordinate of the low corner, light-years.
    #[must_use]
    pub const fn z(self) -> i32 {
        self.z
    }
}

/// The edge length of a generation cell, a power of two from 4 to 128 light-years.
///
/// The stellar layers use 8 to 128 ly (`k = 0..=4`), brown dwarfs 16 ly, rogue planets 4 ly
/// (`k = −1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CellSize {
    /// 4 ly, the rogue-planet layer.
    Ly4,
    /// 8 ly, layer A.
    Ly8,
    /// 16 ly, layer B and the brown dwarfs.
    Ly16,
    /// 32 ly, layer C.
    Ly32,
    /// 64 ly, layer D.
    Ly64,
    /// 128 ly, layer E.
    Ly128,
}

impl CellSize {
    /// Every size, smallest first.
    pub const ALL: [Self; 6] = [
        Self::Ly4,
        Self::Ly8,
        Self::Ly16,
        Self::Ly32,
        Self::Ly64,
        Self::Ly128,
    ];

    /// The size with the given base-2 logarithm of its edge in light-years, 2 to 7.
    #[must_use]
    pub const fn from_log2_ly(log2_ly: u32) -> Option<Self> {
        match log2_ly {
            2 => Some(Self::Ly4),
            3 => Some(Self::Ly8),
            4 => Some(Self::Ly16),
            5 => Some(Self::Ly32),
            6 => Some(Self::Ly64),
            7 => Some(Self::Ly128),
            _ => None,
        }
    }

    /// The base-2 logarithm of the edge in light-years, 2 to 7.
    #[must_use]
    pub const fn log2_ly(self) -> u32 {
        match self {
            Self::Ly4 => 2,
            Self::Ly8 => 3,
            Self::Ly16 => 4,
            Self::Ly32 => 5,
            Self::Ly64 => 6,
            Self::Ly128 => 7,
        }
    }

    /// The edge in light-years.
    #[must_use]
    pub const fn ly(self) -> u32 {
        1 << self.log2_ly()
    }

    /// The layer's `k`, with the edge `8 × 2ᵏ ly`: −1 for 4 ly through 4 for 128 ly.
    #[must_use]
    pub const fn k(self) -> i32 {
        match self {
            Self::Ly4 => -1,
            Self::Ly8 => 0,
            Self::Ly16 => 1,
            Self::Ly32 => 2,
            Self::Ly64 => 3,
            Self::Ly128 => 4,
        }
    }
}

/// A [`GenCell`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildGenCellError {
    /// The cell's light-year span does not fit in `i32` on the named axis (0, 1 or 2).
    CoordinateOutOfRange {
        /// Which axis, 0 for x, 1 for y, 2 for z.
        axis: usize,
        /// The offending cell coordinate.
        coordinate: i32,
    },
}

impl fmt::Display for BuildGenCellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateOutOfRange { axis, coordinate } => write!(
                f,
                "generation cell coordinate {coordinate} on axis {axis} spans light-years outside i32"
            ),
        }
    }
}

impl Error for BuildGenCellError {}

/// A generation cell: a cube of [`CellSize`] light-years on the grid of that size.
///
/// The cell `(x, y, z)` at size `s` spans `[x s, (x + 1) s) ly` on the x axis and likewise on y
/// and z. Its coordinates are the light-year coordinates divided by `s`, rounded down, so the same
/// point is in cell −1 of every size when its coordinate is −1 ly and in cell −2 at 8 ly when its
/// coordinate is −9 ly.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{CellSize, GenCell, LyCell};
///
/// let cell = GenCell::of_ly_cell(LyCell::new([-9, 0, 17]), CellSize::Ly8);
/// assert_eq!(cell.to_array(), [-2, 0, 2]);
/// assert_eq!(cell.origin(), LyCell::new([-16, 0, 16]));
/// assert!(cell.in_root_cube());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenCell {
    size: CellSize,
    x: i32,
    y: i32,
    z: i32,
}

impl GenCell {
    /// A cell of the given size and coordinates on that size's grid.
    ///
    /// # Errors
    ///
    /// [`BuildGenCellError::CoordinateOutOfRange`] if the cell's light-years, `[c s, (c + 1) s)`,
    /// do not fit in `i32` on some axis.
    pub fn new(size: CellSize, [x, y, z]: [i32; 3]) -> Result<Self, BuildGenCellError> {
        let span = i64::from(size.ly());
        for (axis, coordinate) in [x, y, z].into_iter().enumerate() {
            let low = i64::from(coordinate) * span;
            let high = low + span - 1;
            if i32::try_from(low).is_err() || i32::try_from(high).is_err() {
                return Err(BuildGenCellError::CoordinateOutOfRange { axis, coordinate });
            }
        }
        Ok(Self { size, x, y, z })
    }

    /// The cell of the given size that contains a light-year cell: each coordinate divided by
    /// the size and rounded down (an arithmetic shift).
    #[must_use]
    pub const fn of_ly_cell(cell: LyCell, size: CellSize) -> Self {
        let shift = size.log2_ly();
        Self {
            size,
            x: cell.x >> shift,
            y: cell.y >> shift,
            z: cell.z >> shift,
        }
    }

    /// The cell's size.
    #[must_use]
    pub const fn size(self) -> CellSize {
        self.size
    }

    /// The coordinates on the grid of this size, as `[x, y, z]`.
    #[must_use]
    pub const fn to_array(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }

    /// The low corner in light-years.
    #[must_use]
    pub const fn origin(self) -> LyCell {
        let shift = self.size.log2_ly();
        LyCell {
            x: self.x << shift,
            y: self.y << shift,
            z: self.z << shift,
        }
    }

    /// Whether the cell lies inside the root cube: `−65,536 ≤ c s` and `(c + 1) s ≤ 65,536` on
    /// every axis.
    #[must_use]
    pub fn in_root_cube(self) -> bool {
        let span = i64::from(self.size.ly());
        let half = i64::from(ROOT_HALF_WIDTH_LY);
        [self.x, self.y, self.z].into_iter().all(|c| {
            let low = i64::from(c) * span;
            -half <= low && low + span <= half
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_know_their_logarithms_edges_and_k() {
        for (size, log2, ly, k) in [
            (CellSize::Ly4, 2, 4, -1),
            (CellSize::Ly8, 3, 8, 0),
            (CellSize::Ly16, 4, 16, 1),
            (CellSize::Ly32, 5, 32, 2),
            (CellSize::Ly64, 6, 64, 3),
            (CellSize::Ly128, 7, 128, 4),
        ] {
            assert_eq!(size.log2_ly(), log2);
            assert_eq!(size.ly(), ly);
            assert_eq!(size.k(), k);
            assert_eq!(CellSize::from_log2_ly(log2), Some(size));
        }
        assert_eq!(CellSize::from_log2_ly(8), None);
        assert_eq!(CellSize::ALL.len(), 6);
    }

    #[test]
    fn of_ly_cell_rounds_negative_coordinates_down() {
        for size in CellSize::ALL {
            assert_eq!(
                GenCell::of_ly_cell(LyCell::new([-1, -1, -1]), size).to_array(),
                [-1, -1, -1]
            );
            assert_eq!(
                GenCell::of_ly_cell(LyCell::new([0, 0, 0]), size).to_array(),
                [0, 0, 0]
            );
        }
        let at_8 = |ly| GenCell::of_ly_cell(LyCell::new([ly, 0, 0]), CellSize::Ly8).to_array()[0];
        assert_eq!(at_8(-8), -1);
        assert_eq!(at_8(-9), -2);
        assert_eq!(at_8(7), 0);
        assert_eq!(at_8(8), 1);
    }

    #[test]
    fn the_coordinate_planes_are_cell_faces_at_every_size() {
        for size in CellSize::ALL {
            let below = GenCell::of_ly_cell(LyCell::new([-1, -1, -1]), size);
            let above = GenCell::of_ly_cell(LyCell::new([0, 0, 0]), size);
            assert_ne!(below, above);
            assert_eq!(above.origin(), LyCell::new([0, 0, 0]));
            let edge = i32::try_from(size.ly()).unwrap();
            assert_eq!(below.origin(), LyCell::new([-edge, -edge, -edge]));
        }
    }

    #[test]
    fn root_cube_membership_is_inclusive_of_the_far_face() {
        let edge = GenCell::new(CellSize::Ly128, [511, -512, 0]).unwrap();
        assert!(edge.in_root_cube());
        assert!(
            !GenCell::new(CellSize::Ly128, [512, 0, 0])
                .unwrap()
                .in_root_cube()
        );
        assert!(
            !GenCell::new(CellSize::Ly128, [0, -513, 0])
                .unwrap()
                .in_root_cube()
        );
        assert!(
            GenCell::new(CellSize::Ly4, [16_383, -16_384, 0])
                .unwrap()
                .in_root_cube()
        );
        assert!(
            !GenCell::new(CellSize::Ly4, [16_384, 0, 0])
                .unwrap()
                .in_root_cube()
        );
    }

    #[test]
    fn cells_whose_light_years_overflow_i32_are_rejected() {
        assert_eq!(
            GenCell::new(CellSize::Ly8, [0, i32::MAX / 8 + 1, 0]),
            Err(BuildGenCellError::CoordinateOutOfRange {
                axis: 1,
                coordinate: i32::MAX / 8 + 1
            })
        );
        assert!(GenCell::new(CellSize::Ly8, [i32::MAX / 8, 0, 0]).is_ok());
        assert!(GenCell::new(CellSize::Ly8, [i32::MIN / 8, 0, 0]).is_ok());
        assert!(GenCell::new(CellSize::Ly8, [0, 0, i32::MIN / 8 - 1]).is_err());
    }
}
