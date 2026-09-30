//! Cylindrical coordinates about the galactic axis and the local named directions.

use std::ops::Neg;

use super::galactic::GalacticPosition;
use super::vec3;
use crate::math;
use crate::units::{Metres, Radians};

/// A unit vector along the galactic axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitVector([f64; 3]);

impl UnitVector {
    /// +x, along the bar's long axis.
    pub const X: Self = Self([1.0, 0.0, 0.0]);

    /// +y.
    pub const Y: Self = Self([0.0, 1.0, 0.0]);

    /// +z, galactic north.
    pub const NORTH: Self = Self([0.0, 0.0, 1.0]);

    /// The unit vector along `components`, or `None` if they are zero or not finite.
    #[must_use]
    pub fn from_components(components: [f64; 3]) -> Option<Self> {
        if !vec3::is_finite(components) {
            return None;
        }
        let length = vec3::length(components);
        if length <= 0.0 {
            return None;
        }
        let [x, y, z] = components;
        Some(Self([x / length, y / length, z / length]))
    }

    /// The components along the galactic axes.
    #[must_use]
    pub const fn components(&self) -> [f64; 3] {
        self.0
    }

    /// The dot product, the cosine of the angle between the two.
    #[must_use]
    pub fn dot(&self, other: &Self) -> f64 {
        vec3::dot(self.0, other.0)
    }

    /// The cross product, which is a unit vector only when the two are perpendicular.
    #[must_use]
    pub fn cross(&self, other: &Self) -> [f64; 3] {
        vec3::cross(self.0, other.0)
    }
}

impl Neg for UnitVector {
    type Output = Self;
    fn neg(self) -> Self {
        Self(vec3::neg(self.0))
    }
}

/// Cylindrical coordinates about the galactic z axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cylindrical {
    radius: Metres,
    azimuth: Radians,
    height: Metres,
}

impl Cylindrical {
    /// R, the distance from the z axis.
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }

    /// φ, the angle from +x towards +y, in `(−π, π]`.
    #[must_use]
    pub const fn azimuth(&self) -> Radians {
        self.azimuth
    }

    /// z, the height above the mid-plane, positive towards galactic north.
    #[must_use]
    pub const fn height(&self) -> Metres {
        self.height
    }
}

/// The local named directions at a point off the z axis.
///
/// Coreward points at the z axis, spinward along the rotation (counter-clockwise seen from the
/// north), north along +z; rimward, antispinward and south are their opposites. (Rimward,
/// spinward, north) is right-handed. They are local: undefined on the axis and turning across a
/// chart close to the centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Directions {
    coreward: UnitVector,
    spinward: UnitVector,
}

impl Directions {
    /// Towards the z axis: `(−x, −y, 0) ÷ R`.
    #[must_use]
    pub const fn coreward(&self) -> UnitVector {
        self.coreward
    }

    /// Away from the z axis.
    #[must_use]
    pub fn rimward(&self) -> UnitVector {
        -self.coreward
    }

    /// Along the rotation: `(−y, x, 0) ÷ R`.
    #[must_use]
    pub const fn spinward(&self) -> UnitVector {
        self.spinward
    }

    /// Against the rotation.
    #[must_use]
    pub fn antispinward(&self) -> UnitVector {
        -self.spinward
    }

    /// Galactic north, +z.
    #[must_use]
    pub const fn north(&self) -> UnitVector {
        UnitVector::NORTH
    }

    /// Galactic south, −z.
    #[must_use]
    pub fn south(&self) -> UnitVector {
        -UnitVector::NORTH
    }
}

impl GalacticPosition {
    /// The cylindrical coordinates of this position about the galactic z axis.
    ///
    /// Computed from the full position in float metres, so R is good to about 65 km at the
    /// edge of the cube, which is all any use of R (a density, a frequency) can resolve.
    #[must_use]
    pub fn to_cylindrical(&self) -> Cylindrical {
        let [x, y, z] = self.to_metres_f64();
        Cylindrical {
            radius: Metres::new((x * x + y * y).sqrt()),
            azimuth: Radians::new(math::atan2(y, x)),
            height: Metres::new(z),
        }
    }

    /// The local named directions at this position, or `None` on the z axis, where they are
    /// undefined.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::coords::{GalacticPosition, LyCell, UnitVector};
    ///
    /// let on_x = GalacticPosition::new(LyCell::new([26_000, 0, 0]), [0.0; 3])?;
    /// let d = on_x.directions().expect("off the axis");
    /// assert_eq!(d.spinward(), UnitVector::Y);
    /// assert_eq!(d.coreward(), -UnitVector::X);
    /// assert_eq!(GalacticPosition::ORIGIN.directions(), None);
    /// # Ok::<(), hyperion_sim::coords::BuildGalacticPositionError>(())
    /// ```
    #[must_use]
    pub fn directions(&self) -> Option<Directions> {
        let [x, y, _] = self.to_metres_f64();
        let radius = (x * x + y * y).sqrt();
        if radius <= 0.0 {
            return None;
        }
        Some(Directions {
            coreward: UnitVector([-x / radius, -y / radius, 0.0]),
            spinward: UnitVector([-y / radius, x / radius, 0.0]),
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::super::cell::LyCell;
    use super::*;
    use crate::units::consts::METRES_PER_LIGHT_YEAR;

    fn at(cell: [i32; 3], offset: [f64; 3]) -> GalacticPosition {
        GalacticPosition::new(LyCell::new(cell), offset).unwrap()
    }

    fn assert_close(a: [f64; 3], b: [f64; 3], tolerance: f64) {
        for axis in 0..3 {
            assert!((a[axis] - b[axis]).abs() <= tolerance, "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn on_the_x_axis_spinward_is_plus_y() {
        let d = at([26_000, 0, 0], [0.0; 3]).directions().unwrap();
        assert_eq!(d.spinward(), UnitVector::Y);
        assert_eq!(d.coreward(), -UnitVector::X);
        assert_eq!(d.rimward(), UnitVector::X);
        assert_eq!(d.antispinward(), -UnitVector::Y);
        assert_eq!(d.north(), UnitVector::NORTH);
        assert_eq!(d.south(), -UnitVector::NORTH);
        assert_close(d.south().components(), [0.0, 0.0, -1.0], 0.0);
    }

    #[test]
    fn on_the_y_axis_spinward_is_minus_x_and_coreward_minus_y() {
        let d = at([0, 300, -5], [0.0; 3]).directions().unwrap();
        assert_eq!(d.spinward(), -UnitVector::X);
        assert_eq!(d.coreward(), -UnitVector::Y);
    }

    #[test]
    fn the_directions_are_unit_perpendicular_and_right_handed() {
        let mut g = Lcg::new(0xd1e);
        for _ in 0..1_000 {
            let cell = |g: &mut Lcg| i32::try_from(g.next_below(131_072)).unwrap() - 65_536;
            let p = at(
                [cell(&mut g), cell(&mut g), cell(&mut g)],
                [g.next_f64() * METRES_PER_LIGHT_YEAR, 0.0, 0.0],
            );
            let Some(d) = p.directions() else { continue };
            let (core, spin) = (d.coreward(), d.spinward());
            assert!(core.dot(&spin).abs() <= 1e-15);
            assert!((core.dot(&core) - 1.0).abs() <= 1e-15);
            assert!((spin.dot(&spin) - 1.0).abs() <= 1e-15);
            assert_close(d.rimward().cross(&spin), d.north().components(), 1e-15);
        }
    }

    #[test]
    fn the_z_axis_has_no_directions() {
        assert_eq!(GalacticPosition::ORIGIN.directions(), None);
        assert_eq!(at([0, 0, 500], [0.0, 0.0, 12.0]).directions(), None);
        assert_eq!(at([0, 0, -500], [0.0; 3]).directions(), None);
        assert!(at([0, 0, 0], [1.0, 0.0, 0.0]).directions().is_some());
    }

    #[test]
    fn cylindrical_coordinates_measure_the_azimuth_from_x_towards_y() {
        let c = at([0, 26_000, 100], [0.0; 3]).to_cylindrical();
        assert_same_bits(c.radius().value(), 26_000.0 * METRES_PER_LIGHT_YEAR);
        assert!((c.azimuth().value() - core::f64::consts::FRAC_PI_2).abs() < 1e-15);
        assert_same_bits(c.height().value(), 100.0 * METRES_PER_LIGHT_YEAR);
        let c = at([-1, -1, 0], [0.0; 3]).to_cylindrical();
        assert!((c.azimuth().value() + 3.0 * core::f64::consts::FRAC_PI_4).abs() < 1e-15);
        assert_same_bits(
            GalacticPosition::ORIGIN.to_cylindrical().radius().value(),
            0.0,
        );
    }

    #[test]
    fn unit_vectors_normalise_and_reject_zero() {
        let u = UnitVector::from_components([3.0, 0.0, 4.0]).unwrap();
        assert_close(u.components(), [0.6, 0.0, 0.8], 1e-16);
        assert_eq!(UnitVector::from_components([0.0; 3]), None);
        assert_eq!(UnitVector::from_components([f64::INFINITY, 0.0, 0.0]), None);
        assert_close(
            UnitVector::X.cross(&UnitVector::Y),
            UnitVector::NORTH.components(),
            0.0,
        );
    }
}
