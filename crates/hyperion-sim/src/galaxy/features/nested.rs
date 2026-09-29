//! A feature's nested grid: levels of cells centred on the feature, each a shell around the last
//! (plan 09, P09.T20; brainstorm, "Dense features: clusters and the galactic centre").
//!
//! Level j is a block of `n × n × n` cells of edge `w × 2ʲ` centred on the feature. Its inner half
//! on every axis, `n ÷ 2` cells from `n ÷ 4` to `3n ÷ 4 − 1`, is exactly the volume of level j − 1,
//! which owns it; level 0 owns its whole block. The catalogue features use 16 cells and eight
//! levels, the galactic centre 32 and twelve, and the grid is generic over both. Its reach, the
//! half-width of the outermost block, is `n ÷ 2 × w × 2^(L − 1)`: 512 ly for w = 0.5 ly, 16 cells
//! and eight levels, 128 ly for the centre's 1 ⁄ 256 ly, 32 cells and twelve.
//!
//! The width is a power of two, so every cell edge is an exact binary fraction and a local
//! coordinate divided by an edge is exact: the feature's centre is a cell corner at every level,
//! and which cell holds a point is decided without rounding. Cells are half-open, `[lo, lo + e)` on
//! each axis, so every point of the grid's cube `[−reach, reach)³` has exactly one owner
//! ([`NestedGrid::owner_of`]). Positions here are local: light-years from the feature's centre in
//! the galactic axes.
//!
//! Cell coordinates are those of plan 01's member slot ([`MemberSlot`](crate::id::MemberSlot)):
//! cell `c` of a level spans `[(c − n ÷ 2) e, (c − n ÷ 2 + 1) e)`.

use std::error::Error;
use std::fmt;

use crate::galaxy::PointLy;
use crate::units::LightYears;

use super::interior::LocalCell;

/// The most levels a nested grid has: the centre's twelve, with room for plan 01's four level bits.
const MAX_LEVELS: u8 = 16;

/// The most cells per axis a level has: the centre's 32, with room for a sixth bit.
const MAX_CELLS_PER_AXIS: u8 = 64;

/// The range of a grid's width, as a power of two: 2⁻³⁰ to 2³⁰ ly.
const WIDTH_EXPONENTS: (i32, i32) = (-30, 30);

/// A [`NestedGrid`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildNestedGridError {
    /// The width is not a power of two from 2⁻³⁰ to 2³⁰ ly.
    WidthNotPowerOfTwo,
    /// The cells per axis are not a positive multiple of four up to 64.
    CellsPerAxis,
    /// The levels are not 1 to 16.
    Levels,
}

impl fmt::Display for BuildNestedGridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WidthNotPowerOfTwo => {
                "a nested grid's width must be a power of two from 2^-30 to 2^30 ly"
            }
            Self::CellsPerAxis => {
                "a nested grid's cells per axis must be a positive multiple of four up to 64"
            }
            Self::Levels => "a nested grid must have 1 to 16 levels",
        })
    }
}

impl Error for BuildNestedGridError {}

/// One cell of a nested grid: its level and its cell coordinates in the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NestedCell {
    level: u8,
    cell: [u8; 3],
}

impl NestedCell {
    /// Its level, 0 the innermost.
    #[must_use]
    pub const fn level(self) -> u8 {
        self.level
    }

    /// Its cell coordinates in the level, x, y, z, each below the grid's cells per axis.
    #[must_use]
    pub const fn cell(self) -> [u8; 3] {
        self.cell
    }
}

/// One level of a nested grid: its number and the edge of its cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NestedLevel {
    level: u8,
    edge: f64,
    half_width: f64,
}

impl NestedLevel {
    /// Its number, 0 the innermost.
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }

    /// The edge of its cells, `w × 2ʲ`.
    #[must_use]
    pub const fn edge(&self) -> LightYears {
        LightYears::new(self.edge)
    }

    /// The half-width of its block, `n ÷ 2` edges.
    #[must_use]
    pub const fn half_width(&self) -> LightYears {
        LightYears::new(self.half_width)
    }
}

/// A feature's nested grid (module documentation).
///
/// # Examples
///
/// A globular's grid of half-light-year cells reaches 512 ly, and the cell that owns a point
/// 100 ly out along x is on level 5, whose cells are 16 ly wide:
///
/// ```
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::features::nested::NestedGrid;
/// use hyperion_sim::units::LightYears;
///
/// let grid = NestedGrid::new(LightYears::new(0.5), 16, 8)?;
/// assert_eq!(grid.reach().value(), 512.0);
/// let owner = grid.owner_of(&PointLy::new(100.0, 0.0, 0.0)).expect("inside the reach");
/// assert_eq!((owner.level(), owner.cell()), (5, [14, 8, 8]));
/// // The cells a 20 ly sphere about that point touches, every one owned and none twice.
/// let touched: Vec<_> = grid.cells_touching(&PointLy::new(100.0, 0.0, 0.0), LightYears::new(20.0)).collect();
/// assert!(touched.contains(&owner));
/// # Ok::<(), hyperion_sim::galaxy::features::nested::BuildNestedGridError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NestedGrid {
    /// The width's exponent: `w = 2^width_log2` ly.
    width_log2: i32,
    cells_per_axis: u8,
    levels: u8,
}

impl NestedGrid {
    /// The grid of `levels` levels of `cells_per_axis` cells whose innermost cells are `width`
    /// wide.
    ///
    /// # Errors
    ///
    /// [`BuildNestedGridError::WidthNotPowerOfTwo`] unless `width` is exactly a power of two from
    /// 2⁻³⁰ to 2³⁰ ly; [`BuildNestedGridError::CellsPerAxis`] unless `cells_per_axis` is a positive
    /// multiple of four up to 64, so that the inner half is whole cells and the centre a corner;
    /// [`BuildNestedGridError::Levels`] unless `levels` is 1 to 16.
    pub fn new(
        width: LightYears,
        cells_per_axis: u8,
        levels: u8,
    ) -> Result<Self, BuildNestedGridError> {
        let width_log2 =
            power_of_two_exponent(width.value()).ok_or(BuildNestedGridError::WidthNotPowerOfTwo)?;
        if cells_per_axis == 0
            || !cells_per_axis.is_multiple_of(4)
            || cells_per_axis > MAX_CELLS_PER_AXIS
        {
            return Err(BuildNestedGridError::CellsPerAxis);
        }
        if levels == 0 || levels > MAX_LEVELS {
            return Err(BuildNestedGridError::Levels);
        }
        Ok(Self {
            width_log2,
            cells_per_axis,
            levels,
        })
    }

    /// The edge of level 0's cells, `w`.
    #[must_use]
    pub fn width(&self) -> LightYears {
        LightYears::new(exp2i(self.width_log2))
    }

    /// The width's exponent: `w = 2^k` ly.
    #[must_use]
    pub const fn width_log2(&self) -> i32 {
        self.width_log2
    }

    /// Cells per axis in a level.
    #[must_use]
    pub const fn cells_per_axis(&self) -> u8 {
        self.cells_per_axis
    }

    /// The number of levels.
    #[must_use]
    pub const fn levels(&self) -> u8 {
        self.levels
    }

    /// Level `level`, or `None` past the last.
    #[must_use]
    pub fn level(&self, level: u8) -> Option<NestedLevel> {
        (level < self.levels).then(|| {
            let edge = exp2i(self.width_log2 + i32::from(level));
            NestedLevel {
                level,
                edge,
                half_width: edge * f64::from(self.cells_per_axis / 2),
            }
        })
    }

    /// Every level, innermost first.
    pub fn all_levels(&self) -> impl Iterator<Item = NestedLevel> + '_ {
        (0..self.levels).filter_map(|j| self.level(j))
    }

    /// The half-width of the outermost block: how far the grid reaches from the feature's centre
    /// along an axis.
    #[must_use]
    pub fn reach(&self) -> LightYears {
        self.outermost().half_width()
    }

    /// The outermost level.
    #[must_use]
    fn outermost(self) -> NestedLevel {
        self.level(self.levels - 1)
            .expect("a grid has at least one level")
    }

    /// Whether `cell` coordinate `c` of a level lies in its inner half, which the level below owns.
    #[must_use]
    fn is_inner(self, c: u8) -> bool {
        let n = self.cells_per_axis;
        n / 4 <= c && c < 3 * n / 4
    }

    /// Whether level `level` owns its cell `cell`: level 0 owns all of its block, a higher level
    /// all but its inner half. `false` for a level or cell past the grid's.
    #[must_use]
    pub fn owns(&self, level: u8, cell: [u8; 3]) -> bool {
        level < self.levels
            && cell.iter().all(|&c| c < self.cells_per_axis)
            && (level == 0 || !cell.iter().all(|&c| self.is_inner(c)))
    }

    /// The owned cell `(level, cell)`, or `None` if the level does not own it.
    #[must_use]
    pub fn cell(&self, level: u8, cell: [u8; 3]) -> Option<NestedCell> {
        self.owns(level, cell).then_some(NestedCell { level, cell })
    }

    /// Every owned cell, level by level from the innermost, each level in x, then y, then z order.
    pub fn owned_cells(&self) -> impl Iterator<Item = NestedCell> + '_ {
        let n = self.cells_per_axis;
        (0..self.levels).flat_map(move |level| {
            (0..n).flat_map(move |x| {
                (0..n).flat_map(move |y| (0..n).filter_map(move |z| self.cell(level, [x, y, z])))
            })
        })
    }

    /// `cell`'s box in the feature's frame: its least corner and its edge, ly.
    ///
    /// # Panics
    ///
    /// If `cell` is not of this grid: a level past its last.
    #[must_use]
    pub fn local_cell(&self, cell: NestedCell) -> LocalCell {
        let level = self
            .level(cell.level)
            .expect("a nested cell of this grid is on one of its levels");
        let half = i32::from(self.cells_per_axis / 2);
        let min = cell
            .cell
            .map(|c| f64::from(i32::from(c) - half) * level.edge);
        LocalCell {
            min,
            edge: level.edge,
        }
    }

    /// The owned cell whose half-open box holds the local position `p`, or `None` outside the
    /// grid's cube `[−reach, reach)³` (or for a coordinate that is not a number): the lowest level
    /// whose block holds `p`, and there the cell `⌊p ÷ e⌋ + n ÷ 2`, exact because `e` is a power
    /// of two. It is [`cells_touching`](Self::cells_touching)'s inverse: the one owned cell a point
    /// lies in.
    #[must_use]
    pub fn owner_of(&self, p: &PointLy) -> Option<NestedCell> {
        let coords = [p.x, p.y, p.z];
        self.all_levels().find_map(|level| {
            let h = level.half_width;
            coords
                .iter()
                .all(|&c| (-h..h).contains(&c))
                .then(|| NestedCell {
                    level: level.level,
                    cell: coords.map(|c| cell_index(c, level.edge, self.cells_per_axis)),
                })
        })
    }

    /// The owned cells whose closed boxes meet the closed ball of `radius` about the local
    /// `centre`, level by level from the innermost, each level in x, then y, then z order. A cell
    /// of a level that does not own it is never yielded, so no volume is yielded twice.
    pub fn cells_touching(
        &self,
        centre: &PointLy,
        radius: LightYears,
    ) -> impl Iterator<Item = NestedCell> + '_ {
        let radius = radius.value();
        let centre = [centre.x, centre.y, centre.z];
        let n = self.cells_per_axis;
        let half = i32::from(n / 2);
        self.all_levels().flat_map(move |level| {
            let e = level.edge;
            // The index range on each axis whose closed cells meet [c − r, c + r].
            let range = centre.map(|c| {
                let lo = ((c - radius) / e).floor();
                let hi = ((c + radius) / e).floor();
                let lo = (lo + f64::from(half)).max(0.0);
                let hi = (hi + f64::from(half)).min(f64::from(n) - 1.0);
                (lo, hi)
            });
            let axis = move |a: usize| to_index_range(range[a]);
            axis(0).flat_map(move |x| {
                axis(1).flat_map(move |y| {
                    axis(2).filter_map(move |z| {
                        let cell = self.cell(level.level, [x, y, z])?;
                        (box_distance(&self.local_cell(cell), centre) <= radius).then_some(cell)
                    })
                })
            })
        })
    }
}

/// The distance from the local point `p` to the closed box `cell`, ly.
#[must_use]
fn box_distance(cell: &LocalCell, p: [f64; 3]) -> f64 {
    let mut sum = 0.0;
    for (&lo, &c) in cell.min.iter().zip(&p) {
        let hi = lo + cell.edge;
        let d = if c < lo {
            lo - c
        } else if c > hi {
            c - hi
        } else {
            0.0
        };
        sum += d * d;
    }
    sum.sqrt()
}

/// The indices `lo..=hi` of a range of whole numbers clamped to a level, empty if `lo > hi` or
/// either is not a number.
fn to_index_range((lo, hi): (f64, f64)) -> impl Iterator<Item = u8> + Clone {
    let valid = lo <= hi && lo.is_finite() && hi.is_finite();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "both are whole numbers clamped to 0..=63 when the range is valid"
    )]
    let (lo, hi) = if valid { (lo as u8, hi as u8) } else { (1, 0) };
    lo..=hi
}

/// The cell holding coordinate `c` of a block of `n` cells of edge `e` that holds it: `⌊c ÷ e⌋ +
/// n ÷ 2`, in `0..n`.
#[must_use]
fn cell_index(c: f64, e: f64, n: u8) -> u8 {
    let i = (c / e).floor() + f64::from(n / 2);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "c lies in the block, so i is a whole number in 0..n"
    )]
    let i = i.clamp(0.0, f64::from(n) - 1.0) as u8;
    i
}

/// `2^k`, exact for `k` in −1,022..=1,023, by repeated doubling or halving of 1.
#[must_use]
fn exp2i(k: i32) -> f64 {
    let mut x = 1.0;
    let step = if k >= 0 { 2.0 } else { 0.5 };
    for _ in 0..k.unsigned_abs() {
        x *= step;
    }
    x
}

/// `k` if `w` is exactly `2^k` with `k` in [`WIDTH_EXPONENTS`], else `None`.
#[must_use]
fn power_of_two_exponent(w: f64) -> Option<i32> {
    if !(w.is_normal() && w > 0.0) {
        return None;
    }
    let (lo, hi) = WIDTH_EXPONENTS;
    (lo..=hi).find(|&k| exp2i(k).total_cmp(&w).is_eq())
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;

    fn unit(lcg: &mut Lcg) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "53 bits are exact")]
        let u = (lcg.next_u64() >> 11) as f64;
        u / 9_007_199_254_740_992.0
    }

    fn feature_grid() -> NestedGrid {
        NestedGrid::new(LightYears::new(0.5), 16, 8).unwrap()
    }

    fn centre_grid() -> NestedGrid {
        NestedGrid::new(LightYears::new(1.0 / 256.0), 32, 12).unwrap()
    }

    fn contains(cell: &LocalCell, p: [f64; 3]) -> bool {
        cell.min
            .iter()
            .zip(&p)
            .all(|(&lo, &c)| lo <= c && c < lo + cell.edge)
    }

    #[test]
    fn with_half_light_year_cells_the_reach_is_512_ly() {
        let grid = feature_grid();
        assert_eq!(grid.reach(), LightYears::new(512.0));
        let edges: Vec<f64> = grid.all_levels().map(|l| l.edge().value()).collect();
        assert_eq!(edges, [0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]);
        // The centre's: 1 ⁄ 256 ly to 8 ly, reach 128 ly.
        let centre = centre_grid();
        assert_eq!(centre.reach(), LightYears::new(128.0));
        assert_eq!(centre.level(11).unwrap().edge(), LightYears::new(8.0));
        assert_eq!(centre.level(12), None);
    }

    #[test]
    fn only_powers_of_two_and_whole_quarters_build_a_grid() {
        for bad in [0.0, -0.5, 0.3, 3.0, f64::NAN, f64::INFINITY, 1e-12] {
            assert_eq!(
                NestedGrid::new(LightYears::new(bad), 16, 8),
                Err(BuildNestedGridError::WidthNotPowerOfTwo),
                "{bad}"
            );
        }
        let w = LightYears::new(1.0);
        assert_eq!(
            NestedGrid::new(w, 6, 8),
            Err(BuildNestedGridError::CellsPerAxis)
        );
        assert_eq!(
            NestedGrid::new(w, 0, 8),
            Err(BuildNestedGridError::CellsPerAxis)
        );
        assert_eq!(NestedGrid::new(w, 16, 0), Err(BuildNestedGridError::Levels));
        assert_eq!(
            NestedGrid::new(w, 16, 17),
            Err(BuildNestedGridError::Levels)
        );
        assert_eq!(
            NestedGrid::new(LightYears::new(1.0 / 64.0), 16, 8)
                .unwrap()
                .width_log2(),
            -6
        );
    }

    #[test]
    fn the_owned_cells_fill_the_cube_s_volume_exactly() {
        for grid in [
            feature_grid(),
            NestedGrid::new(LightYears::new(2.0), 8, 3).unwrap(),
        ] {
            let n = u64::from(grid.cells_per_axis());
            let per_level = n * n * n;
            let inner = per_level / 8;
            let mut counts = vec![0_u64; usize::from(grid.levels())];
            let mut volume = 0.0;
            for cell in grid.owned_cells() {
                counts[usize::from(cell.level())] += 1;
                let e = grid.local_cell(cell).edge;
                volume += e * e * e;
            }
            assert_eq!(counts[0], per_level);
            assert!(counts[1..].iter().all(|&c| c == per_level - inner));
            let side = 2.0 * grid.reach().value();
            assert!((volume - side * side * side).abs() <= 1e-12 * volume);
        }
    }

    #[test]
    fn every_point_of_the_cube_has_exactly_one_owned_cell() {
        let mut lcg = Lcg::new(0x0920_0001);
        for grid in [feature_grid(), centre_grid()] {
            let cells: Vec<(NestedCell, LocalCell)> = grid
                .owned_cells()
                .map(|c| (c, grid.local_cell(c)))
                .collect();
            let reach = grid.reach().value();
            for i in 0..400 {
                // Points spread in log radius, so that every level is sampled.
                let scale = reach * exp2i(-(i % 14));
                let p = [0; 3].map(|_| (2.0 * unit(&mut lcg) - 1.0) * scale);
                let holders: Vec<NestedCell> = cells
                    .iter()
                    .filter(|(_, b)| contains(b, p))
                    .map(|(c, _)| *c)
                    .collect();
                assert_eq!(holders.len(), 1, "{p:?}: {holders:?}");
                assert_eq!(
                    grid.owner_of(&PointLy::new(p[0], p[1], p[2])),
                    Some(holders[0])
                );
            }
        }
    }

    #[test]
    fn the_centre_is_a_corner_and_the_boundaries_are_half_open() {
        let grid = feature_grid();
        let owner = |x, y, z| grid.owner_of(&PointLy::new(x, y, z)).unwrap();
        assert_eq!(
            owner(0.0, 0.0, 0.0),
            NestedCell {
                level: 0,
                cell: [8, 8, 8]
            }
        );
        assert_eq!(owner(-1e-300, 0.0, 0.0).cell(), [7, 8, 8]);
        // Level 0's block is [−4, 4); 4 itself is level 1's.
        assert_eq!(
            owner(4.0, 0.0, 0.0),
            NestedCell {
                level: 1,
                cell: [12, 8, 8]
            }
        );
        assert_eq!(owner(-4.0, 0.0, 0.0).level(), 0);
        assert_eq!(grid.owner_of(&PointLy::new(512.0, 0.0, 0.0)), None);
        assert_eq!(owner(-512.0, 0.0, 0.0).cell(), [0, 8, 8]);
        assert_eq!(grid.owner_of(&PointLy::new(f64::NAN, 0.0, 0.0)), None);
        for level in 0..8 {
            let l = grid.level(level).unwrap();
            // The block's centre is the corner between cells 7 and 8 at every level.
            let corner = grid.local_cell(NestedCell {
                level,
                cell: [8, 8, 8],
            });
            for c in corner.min {
                assert_same_bits(c, 0.0);
            }
            assert_same_bits(l.half_width().value(), 8.0 * l.edge().value());
        }
    }

    #[test]
    fn cells_touching_equals_a_brute_force_scan() {
        let mut lcg = Lcg::new(0x0920_0002);
        for grid in [
            feature_grid(),
            NestedGrid::new(LightYears::new(0.25), 8, 5).unwrap(),
        ] {
            let all: Vec<(NestedCell, LocalCell)> = grid
                .owned_cells()
                .map(|c| (c, grid.local_cell(c)))
                .collect();
            let reach = grid.reach().value();
            for i in 0..60 {
                let scale = 1.3 * reach * exp2i(-(i % 9));
                let c = [0; 3].map(|_| (2.0 * unit(&mut lcg) - 1.0) * scale);
                let radius = scale * unit(&mut lcg) * 0.5;
                let brute: Vec<NestedCell> = all
                    .iter()
                    .filter(|(_, b)| box_distance(b, c) <= radius)
                    .map(|(cell, _)| *cell)
                    .collect();
                let walked: Vec<NestedCell> = grid
                    .cells_touching(&PointLy::new(c[0], c[1], c[2]), LightYears::new(radius))
                    .collect();
                assert_eq!(walked, brute, "sphere of {radius} ly at {c:?}");
            }
            // A sphere outside the cube touches nothing; one holding it touches everything.
            let far = PointLy::new(3.0 * reach, 0.0, 0.0);
            assert_eq!(
                grid.cells_touching(&far, LightYears::new(0.5 * reach))
                    .count(),
                0
            );
            let whole = grid
                .cells_touching(&PointLy::new(0.0, 0.0, 0.0), LightYears::new(2.0 * reach))
                .count();
            assert_eq!(whole, all.len());
        }
    }

    #[test]
    fn an_owned_cell_is_one_plan_01_accepts_in_a_member_id() {
        use crate::id::{FeatureCell, FeatureMemberId, FeatureRef, Layer, MemberSlot};
        let feature = FeatureRef::new(FeatureCell::new([0, 6, 0]).unwrap(), 3).unwrap();
        let grid = feature_grid();
        let n = grid.cells_per_axis();
        for level in 0..grid.levels() {
            for x in 0..n {
                for y in 0..n {
                    for z in 0..n {
                        let slot = MemberSlot::InCell {
                            band: Layer::B,
                            level,
                            cell: [x, y, z],
                            index: 0,
                        };
                        assert_eq!(
                            FeatureMemberId::new(feature, slot).is_ok(),
                            grid.owns(level, [x, y, z])
                        );
                    }
                }
            }
        }
    }
}
