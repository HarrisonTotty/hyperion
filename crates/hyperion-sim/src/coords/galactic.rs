//! The galactic frame: positions as a light-year cell plus a metre offset, and vectors in metres.

use std::error::Error;
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

use super::cell::{GenCell, LyCell};
use super::{ROOT_HALF_WIDTH_LY, vec3};
use crate::math;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Metres, MetresPerSecond, Seconds};

/// 2⁻⁵², the weight of the lowest of the 52 fraction bits of a position word.
const TWO_POW_MINUS_52: f64 = 1.0 / 4_503_599_627_370_496.0;

/// A [`GalacticPosition`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildGalacticPositionError {
    /// An offset component was a NaN or infinite.
    OffsetNotFinite {
        /// Which axis, 0 for x, 1 for y, 2 for z.
        axis: usize,
    },
    /// An offset component was below zero or not below one light-year.
    OffsetOutOfRange {
        /// Which axis, 0 for x, 1 for y, 2 for z.
        axis: usize,
    },
}

impl fmt::Display for BuildGalacticPositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OffsetNotFinite { axis } => write!(f, "offset on axis {axis} is not finite"),
            Self::OffsetOutOfRange { axis } => {
                write!(f, "offset on axis {axis} is not in [0, 1 ly)")
            }
        }
    }
}

impl Error for BuildGalacticPositionError {}

/// A position in the galactic frame: a 1 ly cell plus an offset in metres inside it.
///
/// The representation is canonical: each offset component is finite and in `[0, 1 ly)`, so one
/// point has one value, and the resolution is about 2 m anywhere in the galaxy. `new` rejects
/// anything else, and [`translated`](Self::translated) carries whole light-years into the cell.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{GalacticDisplacement, GalacticPosition, LyCell};
/// use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
///
/// let a = GalacticPosition::new(LyCell::new([26_000, 0, 0]), [0.0, 0.0, 0.0])?;
/// let b = a.translated(GalacticDisplacement::new([1.5 * METRES_PER_LIGHT_YEAR, 0.0, -4.0]))
///     .expect("a small step stays in range");
/// assert_eq!(b.cell(), LyCell::new([26_001, 0, -1]));
/// assert!((b.offset_metres()[0] - 0.5 * METRES_PER_LIGHT_YEAR).abs() < 1.0);
/// assert!((b.offset_metres()[2] - (METRES_PER_LIGHT_YEAR - 4.0)).abs() < 1.0);
/// # Ok::<(), hyperion_sim::coords::BuildGalacticPositionError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GalacticPosition {
    cell: LyCell,
    offset: [f64; 3],
}

impl GalacticPosition {
    /// The galactic centre.
    pub const ORIGIN: Self = Self {
        cell: LyCell::new([0, 0, 0]),
        offset: [0.0; 3],
    };

    /// Builds a position from its cell and its offset in metres.
    ///
    /// A negative zero offset is stored as positive zero.
    ///
    /// # Errors
    ///
    /// [`BuildGalacticPositionError::OffsetNotFinite`] for a NaN or infinite component, and
    /// [`BuildGalacticPositionError::OffsetOutOfRange`] for one outside `[0, 1 ly)`.
    pub fn new(cell: LyCell, offset: [f64; 3]) -> Result<Self, BuildGalacticPositionError> {
        let mut normalised = [0.0; 3];
        for (axis, component) in offset.into_iter().enumerate() {
            if !component.is_finite() {
                return Err(BuildGalacticPositionError::OffsetNotFinite { axis });
            }
            if !(0.0..METRES_PER_LIGHT_YEAR).contains(&component) {
                return Err(BuildGalacticPositionError::OffsetOutOfRange { axis });
            }
            normalised[axis] = component + 0.0;
        }
        Ok(Self {
            cell,
            offset: normalised,
        })
    }

    /// The 1 ly cell: the wire form's first part.
    #[must_use]
    pub const fn cell(&self) -> LyCell {
        self.cell
    }

    /// The offset inside the cell in metres, each component in `[0, 1 ly)`: the wire form's
    /// second part.
    #[must_use]
    pub const fn offset_metres(&self) -> [f64; 3] {
        self.offset
    }

    /// A position from float light-years, or `None` if a coordinate is not finite or its whole
    /// part does not fit in `i32`.
    ///
    /// The offset is `(ly − ⌊ly⌋) × 1 ly`, good to the precision the float had: a float
    /// light-year steps by 2 m at 1 ly, 17 m at 10 ly and 69 km at 50,000 ly.
    #[must_use]
    pub fn from_light_years(ly: [f64; 3]) -> Option<Self> {
        let mut cell = [0_i32; 3];
        let mut offset = [0.0; 3];
        for axis in 0..3 {
            let mut whole = ly[axis].floor();
            let mut metres = (ly[axis] - whole) * METRES_PER_LIGHT_YEAR;
            if metres >= METRES_PER_LIGHT_YEAR {
                // Only a fraction that rounded to 1, just below a whole light-year: carry it.
                metres = 0.0;
                whole += 1.0;
            }
            cell[axis] = whole_to_i32(whole)?;
            offset[axis] = metres;
        }
        // The offsets are finite and in [0, 1 ly) by construction.
        Self::new(LyCell::new(cell), offset).ok()
    }

    /// The position as float light-years, lossy far from the origin (a float light-year steps
    /// by 69 km at 50,000 ly, as a float metre does).
    #[must_use]
    pub fn to_light_years_f64(&self) -> [f64; 3] {
        let [x, y, z] = self.cell.to_array();
        [
            f64::from(x) + self.offset[0] / METRES_PER_LIGHT_YEAR,
            f64::from(y) + self.offset[1] / METRES_PER_LIGHT_YEAR,
            f64::from(z) + self.offset[2] / METRES_PER_LIGHT_YEAR,
        ]
    }

    /// The position as float metres from the galactic centre, lossy far from the origin (about
    /// 65 km at 50,000 ly). For local geometry use [`displacement_to`](Self::displacement_to).
    #[must_use]
    pub fn to_metres_f64(&self) -> [f64; 3] {
        let [x, y, z] = self.cell.to_array();
        [
            math::mul_add(f64::from(x), METRES_PER_LIGHT_YEAR, self.offset[0]),
            math::mul_add(f64::from(y), METRES_PER_LIGHT_YEAR, self.offset[1]),
            math::mul_add(f64::from(z), METRES_PER_LIGHT_YEAR, self.offset[2]),
        ]
    }

    /// The vector from this position to `other`, in metres.
    ///
    /// The integer cells are subtracted first and converted to metres, and the offsets'
    /// difference is added, so nearby positions far from the origin lose nothing: the error is
    /// that of an `f64` at the size of the separation, about 1 m below one light-year.
    #[must_use]
    pub fn displacement_to(&self, other: &Self) -> GalacticDisplacement {
        let from = self.cell.to_array();
        let to = other.cell.to_array();
        let mut metres = [0.0; 3];
        for axis in 0..3 {
            // Exact: both cells are below 2^31 in magnitude, so their difference is below 2^32.
            let cells = f64::from(to[axis]) - f64::from(from[axis]);
            // The whole light-years and the offsets' difference meet in a single rounding.
            metres[axis] = math::mul_add(
                cells,
                METRES_PER_LIGHT_YEAR,
                other.offset[axis] - self.offset[axis],
            );
        }
        GalacticDisplacement(metres)
    }

    /// This position moved by `displacement`, renormalised so that whole light-years are carried
    /// into the cell, or `None` if the displacement is not finite or the cell would leave `i32`.
    #[must_use]
    pub fn translated(&self, displacement: GalacticDisplacement) -> Option<Self> {
        if !vec3::is_finite(displacement.0) {
            return None;
        }
        let from = self.cell.to_array();
        let mut cell = [0_i32; 3];
        let mut offset = [0.0; 3];
        for axis in 0..3 {
            let metres = displacement.0[axis];
            let mut whole = (metres / METRES_PER_LIGHT_YEAR).floor();
            // Reject a displacement beyond the i32 range before correcting the floor, so that
            // the corrections below run at most once each.
            whole_to_i32(whole)?;
            // One rounding of the exact `metres − whole ly`; the quotient's rounding can leave
            // the floor one light-year out, which the corrections absorb.
            let mut remainder = math::mul_add(-whole, METRES_PER_LIGHT_YEAR, metres);
            if remainder < 0.0 {
                remainder += METRES_PER_LIGHT_YEAR;
                whole -= 1.0;
            }
            if remainder >= METRES_PER_LIGHT_YEAR {
                remainder -= METRES_PER_LIGHT_YEAR;
                whole += 1.0;
            }
            let mut total = self.offset[axis] + remainder;
            if total >= METRES_PER_LIGHT_YEAR {
                // Exact: total is in [1 ly, 2 ly), within a factor of two of 1 ly.
                total -= METRES_PER_LIGHT_YEAR;
                whole += 1.0;
            }
            cell[axis] = from[axis].checked_add(whole_to_i32(whole)?)?;
            offset[axis] = total;
        }
        // The offsets are finite and in [0, 1 ly) by construction.
        Self::new(LyCell::new(cell), offset).ok()
    }

    /// The distance to `other`, in metres.
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> Metres {
        self.displacement_to(other).length()
    }

    /// Whether the position lies inside the root cube, `±`[`ROOT_HALF_WIDTH_LY`] on each axis.
    #[must_use]
    pub fn in_root_cube(&self) -> bool {
        self.cell
            .to_array()
            .into_iter()
            .all(|c| (-ROOT_HALF_WIDTH_LY..ROOT_HALF_WIDTH_LY).contains(&c))
    }
}

/// A float that holds a whole number, as an `i32`, or `None` if out of range.
fn whole_to_i32(whole: f64) -> Option<i32> {
    if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&whole) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "whole is an integer inside the i32 range"
    )]
    Some(whole as i32)
}

/// A displacement in the galactic frame: `f64` metres along the galactic axes.
///
/// Its precision is that of an `f64` at its own size, not at the size of the galaxy: about 1 m
/// below one light-year of separation, 128 m at 100 ly.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GalacticDisplacement([f64; 3]);

impl GalacticDisplacement {
    /// A displacement from its components in metres.
    #[must_use]
    pub const fn new(metres: [f64; 3]) -> Self {
        Self(metres)
    }

    /// The components in metres.
    #[must_use]
    pub const fn metres(&self) -> [f64; 3] {
        self.0
    }

    /// The length in metres.
    #[must_use]
    pub fn length(&self) -> Metres {
        Metres::new(vec3::length(self.0))
    }

    /// The dot product with another displacement, m².
    #[must_use]
    pub fn dot(&self, other: &Self) -> f64 {
        vec3::dot(self.0, other.0)
    }
}

impl Add for GalacticDisplacement {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(vec3::add(self.0, rhs.0))
    }
}

impl Sub for GalacticDisplacement {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(vec3::sub(self.0, rhs.0))
    }
}

impl Neg for GalacticDisplacement {
    type Output = Self;
    fn neg(self) -> Self {
        Self(vec3::neg(self.0))
    }
}

impl Mul<f64> for GalacticDisplacement {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self(vec3::scale(self.0, rhs))
    }
}

/// A velocity in the galactic frame: `f64` metres per second along the galactic axes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GalacticVelocity([f64; 3]);

impl GalacticVelocity {
    /// A velocity from its components in metres per second.
    #[must_use]
    pub const fn new(metres_per_second: [f64; 3]) -> Self {
        Self(metres_per_second)
    }

    /// The components in metres per second.
    #[must_use]
    pub const fn metres_per_second(&self) -> [f64; 3] {
        self.0
    }

    /// The speed in metres per second.
    #[must_use]
    pub fn speed(&self) -> MetresPerSecond {
        MetresPerSecond::new(vec3::length(self.0))
    }

    /// The displacement covered at this velocity over a duration: straight-line drift.
    #[must_use]
    pub fn displacement_over(&self, duration: Seconds) -> GalacticDisplacement {
        GalacticDisplacement(vec3::scale(self.0, duration.value()))
    }
}

impl Add for GalacticVelocity {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(vec3::add(self.0, rhs.0))
    }
}

impl Sub for GalacticVelocity {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(vec3::sub(self.0, rhs.0))
    }
}

impl Neg for GalacticVelocity {
    type Output = Self;
    fn neg(self) -> Self {
        Self(vec3::neg(self.0))
    }
}

impl Mul<f64> for GalacticVelocity {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self(vec3::scale(self.0, rhs))
    }
}

impl GenCell {
    /// A position inside this cell from one 64-bit word per axis.
    ///
    /// On each axis the top `log2(size)` bits of the word are the whole light-year inside the
    /// cell and the next 52 bits are the offset fraction: `offset = fraction × 2⁻⁵² × 1 ly`. The
    /// offset is therefore never one `f64` across a coarse cell, and 52 bits rather than 53 keep
    /// it strictly below one light-year after rounding. The low bits of the word are unused.
    ///
    /// This is part of the generator version.
    ///
    /// # Panics
    ///
    /// In debug builds, if an offset reaches one light-year, which the construction prevents.
    #[must_use]
    pub fn position_from_words(&self, words: [u64; 3]) -> GalacticPosition {
        let log2 = self.size().log2_ly();
        let origin = self.origin().to_array();
        let mut cell = [0_i32; 3];
        let mut offset = [0.0; 3];
        for axis in 0..3 {
            let word = words[axis];
            let ly = word >> (64 - log2);
            let fraction = (word << log2) >> 12;
            #[expect(
                clippy::cast_precision_loss,
                reason = "fraction is below 2^52, exact in f64"
            )]
            let fraction = fraction as f64;
            let metres = fraction * TWO_POW_MINUS_52 * METRES_PER_LIGHT_YEAR;
            debug_assert!(
                metres < METRES_PER_LIGHT_YEAR,
                "offset {metres} reached one light-year"
            );
            let ly = i32::try_from(ly).expect("a whole light-year inside a cell is below 128");
            // The constructor guarantees every light-year of the cell fits in i32.
            cell[axis] = origin[axis] + ly;
            offset[axis] = metres;
        }
        GalacticPosition {
            cell: LyCell::new(cell),
            offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::super::cell::CellSize;
    use super::*;

    const LY: f64 = METRES_PER_LIGHT_YEAR;

    fn position(cell: [i32; 3], offset: [f64; 3]) -> GalacticPosition {
        GalacticPosition::new(LyCell::new(cell), offset).unwrap()
    }

    /// The brainstorm's "about 2 m anywhere": just below one light-year the offsets step by 2 m.
    #[test]
    fn offset_resolution_just_under_a_light_year_is_two_metres() {
        assert_same_bits(LY - LY.next_down(), 2.0);
        assert_same_bits(LY.next_down(), LY - 2.0);
    }

    #[test]
    fn new_rejects_offsets_outside_the_cell_and_normalises_negative_zero() {
        assert_eq!(
            GalacticPosition::new(LyCell::new([0; 3]), [0.0, -1.0, 0.0]),
            Err(BuildGalacticPositionError::OffsetOutOfRange { axis: 1 })
        );
        assert_eq!(
            GalacticPosition::new(LyCell::new([0; 3]), [0.0, 0.0, LY]),
            Err(BuildGalacticPositionError::OffsetOutOfRange { axis: 2 })
        );
        assert_eq!(
            GalacticPosition::new(LyCell::new([0; 3]), [f64::NAN, 0.0, 0.0]),
            Err(BuildGalacticPositionError::OffsetNotFinite { axis: 0 })
        );
        let p = position([0; 3], [-0.0, 0.0, LY.next_down()]);
        assert!(p.offset_metres()[0].is_sign_positive());
    }

    #[test]
    fn displacement_subtracts_cells_first() {
        let a = position([50_000, 0, 0], [1.0, 0.0, 0.0]);
        let b = position([50_000, 0, 0], [3.5, 0.0, 0.0]);
        assert_same_bits(a.displacement_to(&b).metres()[0], 2.5);
        let c = position([50_001, -3, 7], [0.0, 0.0, 0.0]);
        let d = a.displacement_to(&c).metres();
        assert_same_bits(d[0], LY - 1.0);
        assert_same_bits(d[1], -3.0 * LY);
        assert_same_bits(d[2], 7.0 * LY);
        assert_same_bits(a.distance_to(&b).value(), 2.5);
    }

    #[test]
    fn translation_carries_whole_light_years_in_both_directions() {
        let a = position([0, 0, 0], [0.0, 0.0, 0.0]);
        let b = a
            .translated(GalacticDisplacement::new([-4.0, 2.5 * LY, LY]))
            .unwrap();
        assert_eq!(b.cell(), LyCell::new([-1, 2, 1]));
        assert_same_bits(b.offset_metres()[0], LY - 4.0);
        assert_same_bits(b.offset_metres()[1], 0.5 * LY);
        assert_same_bits(b.offset_metres()[2], 0.0);
        assert_eq!(
            a.translated(GalacticDisplacement::new([f64::NAN, 0.0, 0.0])),
            None
        );
        assert_eq!(
            a.translated(GalacticDisplacement::new([3e25, 0.0, 0.0])),
            None
        );
        assert_eq!(
            position([i32::MAX, 0, 0], [0.0; 3])
                .translated(GalacticDisplacement::new([LY, 0.0, 0.0])),
            None
        );
    }

    fn random_position(g: &mut Lcg) -> GalacticPosition {
        let cell =
            |g: &mut Lcg| i32::try_from(g.next_below(2 * 65_536)).unwrap() - ROOT_HALF_WIDTH_LY;
        let cell = [cell(g), cell(g), cell(g)];
        let offset = [g.next_f64() * LY, g.next_f64() * LY, g.next_f64() * LY];
        GalacticPosition::new(LyCell::new(cell), offset).unwrap()
    }

    /// A position reached from `a` by a random displacement of up to `within_ly` per axis.
    fn translated_from(g: &mut Lcg, a: &GalacticPosition, within_ly: f64) -> GalacticPosition {
        let step = |g: &mut Lcg| (2.0 * g.next_f64() - 1.0) * within_ly * LY;
        let d = GalacticDisplacement::new([step(g), step(g), step(g)]);
        a.translated(d).unwrap()
    }

    /// An independent position whose cell is within `within_ly` of `a`'s on each axis.
    fn independent_near(g: &mut Lcg, near: &GalacticPosition, within_ly: u64) -> GalacticPosition {
        let span = i32::try_from(within_ly).unwrap();
        let step = |g: &mut Lcg| i32::try_from(g.next_below(2 * within_ly + 1)).unwrap() - span;
        let cell = near.cell().to_array().map(|c| c + step(g));
        let offset = [g.next_f64() * LY, g.next_f64() * LY, g.next_f64() * LY];
        GalacticPosition::new(LyCell::new(cell), offset).unwrap()
    }

    /// The brainstorm's "about 2 m anywhere": a point reached by a translation is found again by
    /// the displacement to it, within 4 m, at any separation up to 100 ly and anywhere in the
    /// cube. Cells are subtracted before anything is rounded, so the distance from the origin
    /// costs nothing.
    #[test]
    fn translate_by_displacement_round_trips_anywhere_in_the_cube() {
        let mut g = Lcg::new(0xc00d);
        for _ in 0..10_000 {
            let a = random_position(&mut g);
            for within_ly in [1.0, 100.0] {
                let b = translated_from(&mut g, &a, within_ly);
                let back = a.translated(a.displacement_to(&b)).unwrap();
                let error = back.distance_to(&b).value();
                assert!(error <= 4.0, "{error} m at {a:?} -> {b:?}");
                assert_eq!(GalacticPosition::new(b.cell(), b.offset_metres()), Ok(b));
                assert_eq!(
                    GalacticPosition::new(back.cell(), back.offset_metres()),
                    Ok(back)
                );
            }
        }
    }

    /// For two unrelated points the displacement is one `f64` per axis, so it carries the
    /// rounding of an `f64` at the separation: 2 m below 1 ly, but 128 m spacing at 100 ly. The
    /// round trip is within 4 m plus that rounding.
    #[test]
    fn displacement_between_unrelated_points_rounds_only_at_the_separation() {
        let mut g = Lcg::new(0x0dd5);
        for _ in 0..10_000 {
            let a = random_position(&mut g);
            for within_ly in [1, 100] {
                let b = independent_near(&mut g, &a, within_ly);
                let d = a.displacement_to(&b);
                let back = a.translated(d).unwrap();
                let error = back.distance_to(&b).value();
                let rounding = d.length().value() * f64::EPSILON;
                assert!(error <= 4.0 + rounding, "{error} m at {a:?} -> {b:?}");
            }
        }
    }

    #[test]
    fn light_year_floats_round_trip_and_getters_rebuild_the_position() {
        let mut g = Lcg::new(0x1234);
        for _ in 0..10_000 {
            let p = random_position(&mut g);
            assert_eq!(GalacticPosition::new(p.cell(), p.offset_metres()), Ok(p));
            let ly = p.to_light_years_f64();
            let q = GalacticPosition::from_light_years(ly).unwrap();
            let d = p.displacement_to(&q).metres();
            assert!(
                d.iter().all(|c| c.abs() <= 1e-9 * LY),
                "{p:?} -> {ly:?} -> {q:?}"
            );
        }
        assert_eq!(
            GalacticPosition::from_light_years([f64::NAN, 0.0, 0.0]),
            None
        );
        assert_eq!(GalacticPosition::from_light_years([3e9, 0.0, 0.0]), None);
        let edge = GalacticPosition::from_light_years([-0.5, 1.0_f64.next_down(), 2.0]).unwrap();
        assert_eq!(edge.cell(), LyCell::new([-1, 0, 2]));
        assert!(edge.offset_metres()[1] < LY);
        // −10⁻²⁰ ly minus its floor rounds to exactly 1: the light-year is carried, not clamped.
        let carried = GalacticPosition::from_light_years([-1e-20, 0.0, -3.0]).unwrap();
        assert_eq!(
            carried,
            GalacticPosition::ORIGIN
                .translated(GalacticDisplacement::new([0.0, 0.0, -3.0 * LY]))
                .unwrap()
        );
        assert_eq!(
            GalacticPosition::from_light_years([f64::from(i32::MAX) + 0.5, 0.0, 0.0])
                .map(|p| p.cell().x()),
            Some(i32::MAX)
        );
    }

    /// The resolution the two float conversions document.
    #[test]
    fn float_light_years_resolve_what_the_docs_say() {
        let step = |ly: f64| (ly.next_up() - ly) * LY;
        assert!((step(1.0) - 2.1).abs() < 0.01);
        assert!((step(10.0) - 16.8).abs() < 0.01);
        assert!((step(50_000.0) - 68_836.0).abs() < 1.0);
    }

    #[test]
    fn root_cube_membership_follows_the_cell() {
        assert!(position([65_535, -65_536, 0], [LY.next_down(), 0.0, 0.0]).in_root_cube());
        assert!(!position([65_536, 0, 0], [0.0; 3]).in_root_cube());
        assert!(!position([0, -65_537, 0], [0.0; 3]).in_root_cube());
    }

    #[test]
    fn position_words_stay_inside_the_cell() {
        for size in CellSize::ALL {
            let cell = GenCell::new(size, [-1, 3, 0]).unwrap();
            let low = cell.position_from_words([0; 3]);
            assert_eq!(low.cell(), cell.origin());
            assert_same_bits(low.offset_metres()[0], 0.0);
            let high = cell.position_from_words([u64::MAX; 3]);
            let edge = i32::try_from(size.ly()).unwrap();
            let [ox, oy, oz] = cell.origin().to_array();
            assert_eq!(
                high.cell(),
                LyCell::new([ox + edge - 1, oy + edge - 1, oz + edge - 1])
            );
            for c in high.offset_metres() {
                assert!(c < LY);
                assert_same_bits(c, LY - 2.0);
            }
            assert_eq!(GenCell::of_ly_cell(high.cell(), size), cell);
            assert_eq!(GenCell::of_ly_cell(low.cell(), size), cell);
        }
    }

    #[test]
    fn velocity_drifts_a_displacement() {
        let v = GalacticVelocity::new([3.0, 0.0, -4.0]);
        assert_same_bits(v.speed().value(), 5.0);
        let d = v.displacement_over(Seconds::new(10.0));
        assert_eq!(d, GalacticDisplacement::new([30.0, 0.0, -40.0]));
        assert_eq!(-d + d * 2.0 - d, GalacticDisplacement::default());
        assert_eq!((v + v - v) * 1.0, v);
        assert_eq!(-v, GalacticVelocity::new([-3.0, 0.0, 4.0]));
    }
}
