//! The orientation of an orbit's plane and periapsis in the system frame.

use std::f64::consts::{PI, TAU};

use super::BuildOrbitError;
use crate::math;
use crate::units::Radians;

/// Where an orbit's plane and periapsis point: inclination, longitude of the ascending node and
/// argument of periapsis, in the system frame (module docs, "Frame and conventions").
///
/// The rotation from the orbit's own plane to the system frame, the unit vectors P towards
/// periapsis and Q a quarter-turn ahead of it in the direction of motion, is computed once here,
/// so propagation needs no trigonometry of the angles.
///
/// # Examples
///
/// ```
/// use std::f64::consts::FRAC_PI_2;
/// use hyperion_sim::orbit::Orientation;
/// use hyperion_sim::units::Radians;
///
/// // A polar orbit whose ascending node is on +x and whose periapsis is over galactic north.
/// let polar = Orientation::new(Radians::new(FRAC_PI_2), Radians::ZERO, Radians::new(FRAC_PI_2))?;
/// let [x, y, z] = polar.periapsis_direction();
/// assert!(x.abs() < 1e-15 && y.abs() < 1e-15 && (z - 1.0).abs() < 1e-15);
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orientation {
    inclination: Radians,
    ascending_node: Radians,
    argument_of_periapsis: Radians,
    /// Unit vector towards periapsis.
    p: [f64; 3],
    /// Unit vector a quarter-turn ahead of periapsis, in the direction of motion.
    q: [f64; 3],
}

impl Orientation {
    /// An orientation from its three angles, rad.
    ///
    /// The inclination must lie in `[0, π]`; `−0` is taken as `0`. The ascending node and the
    /// argument of periapsis may be any finite angle and are reduced into `[0, 2π)`, a value
    /// already there being kept exactly. At inclination 0 or π the node is undefined and only the
    /// sum (at 0) or difference (at π) of the node and the argument places the periapsis; both
    /// are kept as given.
    ///
    /// # Errors
    ///
    /// - [`BuildOrbitError::InclinationOutOfRange`] for an inclination outside `[0, π]` or NaN.
    /// - [`BuildOrbitError::AngleNotFinite`] for a node or argument that is not finite.
    pub fn new(
        inclination: Radians,
        ascending_node: Radians,
        argument_of_periapsis: Radians,
    ) -> Result<Self, BuildOrbitError> {
        let i = inclination.value();
        if !(0.0..=PI).contains(&i) {
            return Err(BuildOrbitError::InclinationOutOfRange { radians: i });
        }
        let i = i + 0.0;
        let node = reduce_to_turn(ascending_node.value())?;
        let argument = reduce_to_turn(argument_of_periapsis.value())?;
        let (sin_i, cos_i) = math::sin_cos(i);
        let (sin_node, cos_node) = math::sin_cos(node);
        let (sin_arg, cos_arg) = math::sin_cos(argument);
        let p = [
            cos_node * cos_arg - sin_node * sin_arg * cos_i,
            sin_node * cos_arg + cos_node * sin_arg * cos_i,
            sin_arg * sin_i,
        ];
        let q = [
            -cos_node * sin_arg - sin_node * cos_arg * cos_i,
            -sin_node * sin_arg + cos_node * cos_arg * cos_i,
            cos_arg * sin_i,
        ];
        Ok(Self {
            inclination: Radians::new(i),
            ascending_node: Radians::new(node),
            argument_of_periapsis: Radians::new(argument),
            p,
            q,
        })
    }

    /// The inclination, rad, in `[0, π]`: 0 is prograde about galactic north.
    #[must_use]
    pub const fn inclination(&self) -> Radians {
        self.inclination
    }

    /// The longitude of the ascending node, rad, in `[0, 2π)`, from +x towards +y.
    #[must_use]
    pub const fn ascending_node(&self) -> Radians {
        self.ascending_node
    }

    /// The argument of periapsis, rad, in `[0, 2π)`, from the ascending node in the direction of
    /// motion.
    #[must_use]
    pub const fn argument_of_periapsis(&self) -> Radians {
        self.argument_of_periapsis
    }

    /// The unit vector from the primary towards periapsis, along the system frame's axes.
    #[must_use]
    pub const fn periapsis_direction(&self) -> [f64; 3] {
        self.p
    }

    /// The unit normal of the orbit's plane, along the orbital angular momentum: P × Q, which is
    /// `(sin i sin Ω, −sin i cos Ω, cos i)` up to rounding.
    #[must_use]
    pub fn normal(&self) -> [f64; 3] {
        super::cross(self.p, self.q)
    }

    /// The system-frame vector with components `along_p` along P and `along_q` along Q.
    pub(super) fn plane_to_system(&self, along_p: f64, along_q: f64) -> [f64; 3] {
        [
            along_p * self.p[0] + along_q * self.q[0],
            along_p * self.p[1] + along_q * self.q[1],
            along_p * self.p[2] + along_q * self.q[2],
        ]
    }

    /// The components along P and Q of a system-frame vector: its projection on the plane.
    pub(super) fn system_to_plane(&self, vector: [f64; 3]) -> (f64, f64) {
        (super::dot(vector, self.p), super::dot(vector, self.q))
    }
}

/// A finite angle reduced into `[0, 2π)`, kept exactly if it is there already.
pub(super) fn reduce_to_turn(radians: f64) -> Result<f64, BuildOrbitError> {
    if !radians.is_finite() {
        return Err(BuildOrbitError::AngleNotFinite { radians });
    }
    let reduced = math::fmod(radians, TAU);
    Ok(if reduced >= 0.0 {
        reduced + 0.0
    } else {
        // Lifting a tiny negative remainder by 2π can round to 2π itself.
        let lifted = reduced + TAU;
        if lifted < TAU { lifted } else { 0.0 }
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;

    fn orientation(i: f64, node: f64, argument: f64) -> Orientation {
        Orientation::new(Radians::new(i), Radians::new(node), Radians::new(argument)).unwrap()
    }

    #[test]
    fn the_reference_orientation_is_the_frame_itself() {
        let o = orientation(0.0, 0.0, 0.0);
        let expected = [[1.0, 0.0, 0.0], [2.0, 3.0, 0.0], [0.0, 0.0, 1.0]];
        let actual = [
            o.periapsis_direction(),
            o.plane_to_system(2.0, 3.0),
            o.normal(),
        ];
        for (vector, expected) in actual.iter().zip(&expected) {
            for (component, expected) in vector.iter().zip(expected) {
                assert_same_bits(*component + 0.0, *expected);
            }
        }
    }

    #[test]
    fn the_basis_is_orthonormal_and_right_handed() {
        let mut lcg = Lcg::new(0x0e1e_0001);
        for _ in 0..1_000 {
            let o = orientation(
                PI * lcg.next_f64(),
                TAU * lcg.next_f64(),
                TAU * lcg.next_f64(),
            );
            let (p, q, w) = (o.p, o.q, o.normal());
            assert!((super::super::dot(p, p) - 1.0).abs() < 1e-15);
            assert!((super::super::dot(q, q) - 1.0).abs() < 1e-15);
            assert!(super::super::dot(p, q).abs() < 1e-15);
            let (sin_i, cos_i) = math::sin_cos(o.inclination().value());
            let (sin_node, cos_node) = math::sin_cos(o.ascending_node().value());
            let expected = [sin_i * sin_node, -sin_i * cos_node, cos_i];
            for axis in 0..3 {
                assert!(
                    (w[axis] - expected[axis]).abs() < 1e-15,
                    "{w:?} vs {expected:?}"
                );
            }
        }
    }

    #[test]
    fn a_retrograde_equatorial_orbit_turns_clockwise() {
        // At inclination π the motion at periapsis, along Q, is towards −y when P is +x.
        let o = orientation(PI, 0.0, 0.0);
        let [qx, qy, qz] = o.q;
        assert!(qx.abs() < 1e-15 && (qy + 1.0).abs() < 1e-15 && qz.abs() < 1e-15);
        assert!((o.normal()[2] + 1.0).abs() < 1e-15);
        let polar = orientation(FRAC_PI_2, FRAC_PI_2, 0.0);
        let [px, py, _] = polar.periapsis_direction();
        assert!(px.abs() < 1e-15 && (py - 1.0).abs() < 1e-15);
    }

    #[test]
    fn angles_are_reduced_into_one_turn() {
        let o = orientation(1.0, -FRAC_PI_2, 5.0 * PI);
        assert!((o.ascending_node().value() - 1.5 * PI).abs() < 1e-15);
        assert!((o.argument_of_periapsis().value() - PI).abs() < 1e-14);
        let kept = orientation(0.5, 6.0, 0.25);
        assert_same_bits(kept.ascending_node().value(), 6.0);
        assert_same_bits(kept.argument_of_periapsis().value(), 0.25);
        assert_same_bits(orientation(-0.0, -0.0, 0.0).inclination().value(), 0.0);
        assert_same_bits(orientation(0.0, -1e-300, 0.0).ascending_node().value(), 0.0);
    }

    #[test]
    fn out_of_range_angles_are_refused() {
        let bad = |i: f64, node: f64, argument: f64| {
            Orientation::new(Radians::new(i), Radians::new(node), Radians::new(argument))
                .unwrap_err()
        };
        assert_eq!(
            bad(-0.1, 0.0, 0.0),
            BuildOrbitError::InclinationOutOfRange { radians: -0.1 }
        );
        assert_eq!(
            bad(3.2, 0.0, 0.0),
            BuildOrbitError::InclinationOutOfRange { radians: 3.2 }
        );
        assert!(matches!(
            bad(f64::NAN, 0.0, 0.0),
            BuildOrbitError::InclinationOutOfRange { .. }
        ));
        assert_eq!(
            bad(1.0, f64::INFINITY, 0.0),
            BuildOrbitError::AngleNotFinite {
                radians: f64::INFINITY
            }
        );
        assert!(matches!(
            bad(1.0, 0.0, f64::NAN),
            BuildOrbitError::AngleNotFinite { .. }
        ));
    }
}
