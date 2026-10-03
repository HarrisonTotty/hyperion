//! Body-fixed frames (plan 14, design note 23, P14.T14.c): the rotating frame of a planet, moon or
//! dwarf planet, in which a later surface plan places its map.
//!
//! Plan 01's body frame ([`BodyPosition`]) is inertial: metres from the body's centre along the
//! galactic axes. A body's fixed frame turns with it. Its third axis is the pole, set by the
//! obliquity and the pole's drawn azimuth about the orbit's normal ([`pole_of`]); its first axis
//! is the prime meridian, at the rotation angle W(t) of [`RotationLaw`] from the ascending node of
//! the equator on the orbital plane; the second completes a right-handed set. For a locked body
//! the prime meridian faces the primary at pericentre ([`RotationInputs::sub_primary_angle`]).
//!
//! [`BodyFixedFrame`] holds the pole, the angle at the epoch and the law; [`body_fixed_at`] gives
//! the rotation from the inertial body frame to the fixed one at a time, as a
//! [`FrameRotation`].

use crate::coords::{BodyFixedRotation, BodyPosition};
use crate::math;
use crate::orbit::KeplerElements;
use crate::planetary::derive::rotation::{
    BuildRotationLawError, RotationInputs, RotationLaw, SpinState,
};
use crate::time::UniverseTime;
use crate::units::{Radians, Seconds, Years};

/// A body's rotating frame (design note 23): its pole, the prime meridian's angle at the epoch,
/// and the rotation law that turns it (P14.T14.c).
///
/// The rotation's rate is not constant, so `rate` is the whole law ([`RotationLaw`]): the despin
/// towards the lock and the locked rate after it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyFixedFrame {
    pole: [f64; 3],
    node: [f64; 3],
    quarter: [f64; 3],
    obliquity: Radians,
    w0: Radians,
    rate: RotationLaw,
}

impl BodyFixedFrame {
    /// The frame of a body on `orbit` about its primary, whose spin axis is tilted by `obliquity`
    /// from the orbit's normal at azimuth `pole_azimuth` from the orbit's ascending node, spinning
    /// at first with `primordial_period` and locked by its primary's tides after `locking_time`,
    /// in a system aged `age_at_epoch` at the epoch, with rotation angle `phase_at_epoch` then if
    /// it is not locked by then.
    ///
    /// # Errors
    ///
    /// As [`RotationLaw::new`].
    ///
    /// # Examples
    ///
    /// A locked planet's prime meridian faces its star at pericentre:
    ///
    /// ```
    /// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// use hyperion_sim::planetary::frames::{BodyFixedFrame, FrameSpin, body_fixed_at};
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::consts::METRES_PER_AU;
    /// use hyperion_sim::units::{GravitationalParameter, Metres, Radians, Seconds, SolarMasses, Years};
    ///
    /// let orientation = Orientation::new(Radians::new(0.3), Radians::new(1.0), Radians::new(2.0))?;
    /// let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(0.2));
    /// let orbit = KeplerElements::from_semi_major_axis(Metres::new(0.07 * METRES_PER_AU), mu, Eccentricity::new(0.05)?, orientation, Radians::ZERO)?;
    /// let spin = FrameSpin {
    ///     obliquity: Radians::new(0.1),
    ///     pole_azimuth: Radians::new(0.4),
    ///     primordial_period: Seconds::new(54_000.0),
    ///     locking_time: Seconds::new(1e13),
    ///     phase_at_epoch: Radians::ZERO,
    /// };
    /// let frame = BodyFixedFrame::new(&orbit, &spin, Years::new(4e9))?;
    /// // The mean anomaly at the epoch is zero: the planet is at pericentre, its star along −P.
    /// let [px, py, pz] = orientation.periapsis_direction();
    /// let meridian = body_fixed_at(&frame, UniverseTime::EPOCH).prime_meridian();
    /// let facing = -(meridian[0] * px + meridian[1] * py + meridian[2] * pz);
    /// assert!(facing > 0.99);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(
        orbit: &KeplerElements,
        spin: &FrameSpin,
        age_at_epoch: Years,
    ) -> Result<Self, BuildRotationLawError> {
        let pole = pole_of(orbit, spin.obliquity, spin.pole_azimuth);
        let (node, quarter) = equator_axes(orbit, pole);
        let p = orbit.orientation().periapsis_direction();
        let towards = [-p[0], -p[1], -p[2]];
        let sub_primary = math::atan2(dot(towards, quarter), dot(towards, node));
        let rate = RotationLaw::new(&RotationInputs {
            primordial_period: spin.primordial_period,
            locking_time: spin.locking_time,
            age_at_epoch,
            orbit: *orbit,
            sub_primary_angle: Radians::new(sub_primary),
            phase_at_epoch: spin.phase_at_epoch,
        })?;
        Ok(Self {
            pole,
            node,
            quarter,
            obliquity: spin.obliquity,
            w0: rate.angle_at(UniverseTime::EPOCH),
            rate,
        })
    }

    /// The pole, the spin axis's unit vector along the galactic axes, by the right-hand rule.
    #[must_use]
    pub const fn pole(&self) -> [f64; 3] {
        self.pole
    }

    /// The ascending node of the equator on the orbital plane, a unit vector along the galactic
    /// axes: where the rotation angle is measured from.
    #[must_use]
    pub const fn equator_node(&self) -> [f64; 3] {
        self.node
    }

    /// The equator's axis a quarter-turn east of [`equator_node`](Self::equator_node), pole ×
    /// node, a unit vector along the galactic axes: where the rotation angle is π ÷ 2 (P14.T46.b).
    #[must_use]
    pub const fn equator_quarter(&self) -> [f64; 3] {
        self.quarter
    }

    /// The obliquity, rad, in `[0, π]`: the angle between the pole and the orbit's normal
    /// (P14.T46.b).
    #[must_use]
    pub const fn obliquity(&self) -> Radians {
        self.obliquity
    }

    /// The rotation angle W at the epoch, rad, in `[0, 2π)`.
    #[must_use]
    pub const fn w0(&self) -> Radians {
        self.w0
    }

    /// The rotation law.
    #[must_use]
    pub const fn rate(&self) -> &RotationLaw {
        &self.rate
    }

    /// Whether the body is locked at `t`.
    #[must_use]
    pub fn state_at(&self, t: UniverseTime) -> SpinState {
        self.rate.state_at(t)
    }
}

/// A body's spin as [`BodyFixedFrame::new`] reads it: its obliquity and pole azimuth
/// (P14.T14.a), and its primordial period and locking time (P14.T14.b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameSpin {
    /// The obliquity, rad, in `[0, π]`: the angle between the spin axis and the orbit's normal.
    pub obliquity: Radians,
    /// The azimuth of the pole about the orbit's normal from the orbit's ascending node, rad.
    pub pole_azimuth: Radians,
    /// The primordial rotation period.
    pub primordial_period: Seconds,
    /// The despinning time from the system's birth; infinite for a body that never locks.
    pub locking_time: Seconds,
    /// The rotation angle at the epoch, if the body is not locked by then, rad.
    pub phase_at_epoch: Radians,
}

/// The rotation from a body's inertial frame to its fixed frame at one time: three orthonormal
/// rows, the prime meridian, the axis a quarter-turn east of it and the pole, each along the
/// galactic axes (P14.T14.c).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRotation([[f64; 3]; 3]);

impl FrameRotation {
    /// The matrix, whose rows are the fixed frame's axes in the inertial frame: a vector's fixed
    /// components are the matrix times its inertial ones.
    #[must_use]
    pub const fn matrix(&self) -> [[f64; 3]; 3] {
        self.0
    }

    /// The prime meridian's direction in the equator, along the galactic axes.
    #[must_use]
    pub const fn prime_meridian(&self) -> [f64; 3] {
        self.0[0]
    }

    /// The pole, along the galactic axes.
    #[must_use]
    pub const fn pole(&self) -> [f64; 3] {
        self.0[2]
    }

    /// The components in the fixed frame, metres, of `position` in the inertial body frame.
    #[must_use]
    pub fn to_fixed(&self, position: &BodyPosition) -> [f64; 3] {
        let v = position.metres();
        self.0.map(|row| dot(row, v))
    }

    /// The same rotation as plan 01's [`BodyFixedRotation`], which maps body-fixed to body axes:
    /// the transpose of [`matrix`](Self::matrix), so that there is one rotation type
    /// (`coords/body_fixed.rs`, P14.T46.b).
    ///
    /// # Panics
    ///
    /// Never for a frame [`body_fixed_at`] gives, whose rows are orthonormal to well within
    /// [`ROTATION_ORTHONORMAL_TOLERANCE`](crate::coords::ROTATION_ORTHONORMAL_TOLERANCE) and
    /// right-handed.
    #[must_use]
    pub fn to_body_fixed_rotation(&self) -> BodyFixedRotation {
        let m = self.0;
        BodyFixedRotation::from_rows([
            [m[0][0], m[1][0], m[2][0]],
            [m[0][1], m[1][1], m[2][1]],
            [m[0][2], m[1][2], m[2][2]],
        ])
        .expect("a body-fixed frame's axes are orthonormal and right-handed")
    }
}

/// The rotation from `frame`'s body's inertial frame to its fixed frame at `t` (P14.T14.c,
/// design note 23): the equator's axes turned about the pole by the rotation angle W(t).
#[must_use]
pub fn body_fixed_at(frame: &BodyFixedFrame, t: UniverseTime) -> FrameRotation {
    let (sin_w, cos_w) = math::sin_cos(frame.rate.angle_at(t).value());
    let (x, y) = (frame.node, frame.quarter);
    let meridian = [
        cos_w * x[0] + sin_w * y[0],
        cos_w * x[1] + sin_w * y[1],
        cos_w * x[2] + sin_w * y[2],
    ];
    let east = [
        -sin_w * x[0] + cos_w * y[0],
        -sin_w * x[1] + cos_w * y[1],
        -sin_w * x[2] + cos_w * y[2],
    ];
    FrameRotation([meridian, east, frame.pole])
}

/// The pole of a body on `orbit` tilted by `obliquity` from the orbit's normal at `azimuth` from
/// its ascending node: cos ε ĥ + sin ε (cos ψ N̂ + sin ψ (ĥ × N̂)), with ĥ the normal and N̂ the
/// node's direction (P14.T14.a; design note 23).
#[must_use]
pub fn pole_of(orbit: &KeplerElements, obliquity: Radians, azimuth: Radians) -> [f64; 3] {
    let [node, quarter, normal] = plane_axes(orbit);
    let (sin_e, cos_e) = math::sin_cos(obliquity.value());
    let (sin_a, cos_a) = math::sin_cos(azimuth.value());
    let pole = [
        cos_e * normal[0] + sin_e * (cos_a * node[0] + sin_a * quarter[0]),
        cos_e * normal[1] + sin_e * (cos_a * node[1] + sin_a * quarter[1]),
        cos_e * normal[2] + sin_e * (cos_a * node[2] + sin_a * quarter[2]),
    ];
    unit(pole)
}

/// The orbital plane's node direction, the axis a quarter-turn ahead of it in the plane, and the
/// normal, along the galactic axes.
fn plane_axes(orbit: &KeplerElements) -> [[f64; 3]; 3] {
    let o = orbit.orientation();
    let (sin_i, cos_i) = math::sin_cos(o.inclination().value());
    let (sin_n, cos_n) = math::sin_cos(o.ascending_node().value());
    [
        [cos_n, sin_n, 0.0],
        [-sin_n * cos_i, cos_n * cos_i, sin_i],
        [sin_i * sin_n, -sin_i * cos_n, cos_i],
    ]
}

/// The equator's axes of a body with pole `pole` on `orbit`: the ascending node of the equator on
/// the orbital plane, ĥ × p̂ normalised, and p̂ × that; about a pole along the normal, where the
/// node is undefined, the orbit's own node.
fn equator_axes(orbit: &KeplerElements, pole: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let [orbit_node, _, normal] = plane_axes(orbit);
    let crossed = cross(normal, pole);
    let length = dot(crossed, crossed).sqrt();
    let node = if length > 1e-9 {
        [
            crossed[0] / length,
            crossed[1] / length,
            crossed[2] / length,
        ]
    } else {
        orbit_node
    };
    let quarter = unit(cross(pole, node));
    (node, quarter)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    [v[0] / length, v[1] / length, v[2] / length]
}

#[cfg(test)]
mod tests {
    use core::f64::consts::TAU;

    use super::*;
    use crate::orbit::{Eccentricity, Orientation};
    use crate::time::Span;
    use crate::units::consts::{METRES_PER_AU, SECONDS_PER_JULIAN_YEAR};
    use crate::units::{GravitationalParameter, Metres, SolarMasses};

    fn orbit(e: f64, m0: f64) -> KeplerElements {
        let orientation =
            Orientation::new(Radians::new(0.7), Radians::new(2.1), Radians::new(4.0)).unwrap();
        KeplerElements::from_semi_major_axis(
            Metres::new(0.05 * METRES_PER_AU),
            GravitationalParameter::from_solar_masses(SolarMasses::new(0.3)),
            Eccentricity::new(e).unwrap(),
            orientation,
            Radians::new(m0),
        )
        .unwrap()
    }

    fn frame(orbit: &KeplerElements, obliquity: f64, locking_years: f64) -> BodyFixedFrame {
        let spin = FrameSpin {
            obliquity: Radians::new(obliquity),
            pole_azimuth: Radians::new(1.3),
            primordial_period: Seconds::new(54_000.0),
            locking_time: Seconds::new(locking_years * SECONDS_PER_JULIAN_YEAR),
            phase_at_epoch: Radians::new(0.25),
        };
        BodyFixedFrame::new(orbit, &spin, Years::new(4e9)).unwrap()
    }

    fn at(seconds: f64) -> UniverseTime {
        UniverseTime::EPOCH
            .checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    /// Times of the first pericentres after the epoch.
    fn pericentres(orbit: &KeplerElements, count: u32) -> Vec<UniverseTime> {
        let p = orbit.period().value();
        let first = (TAU - orbit.mean_anomaly_at_epoch().value()) / TAU * p;
        (0..count).map(|k| at(first + f64::from(k) * p)).collect()
    }

    /// The cosine of the angle between the prime meridian and the direction to the primary at
    /// pericentre.
    fn facing(orbit: &KeplerElements, rotation: &FrameRotation) -> f64 {
        let p = orbit.orientation().periapsis_direction();
        -dot(rotation.prime_meridian(), p)
    }

    /// (c) A synchronously locked body's prime meridian faces its primary at every pericentre,
    /// whatever its obliquity.
    #[test]
    fn a_locked_body_faces_its_primary_at_every_pericentre() {
        let orbit = orbit(0.05, 1.0);
        for obliquity in [0.0, 0.2, 2.5] {
            let frame = frame(&orbit, obliquity, 1e6);
            for t in pericentres(&orbit, 40) {
                let rotation = body_fixed_at(&frame, t);
                // The primary lies in the prime meridian's half-plane.
                let p = orbit.orientation().periapsis_direction();
                let towards = [-p[0], -p[1], -p[2]];
                assert!(dot(towards, rotation.matrix()[1]).abs() < 1e-6);
                assert!(dot(towards, rotation.prime_meridian()) > 0.0);
                if obliquity < 1e-12 {
                    assert!(facing(&orbit, &rotation) > 1.0 - 1e-9);
                }
            }
        }
    }

    /// (c) A body in the 3:2 state has its prime meridian along the line to its primary at every
    /// pericentre, facing it at every other one.
    #[test]
    fn a_three_to_two_body_alternates_at_pericentre() {
        let orbit = orbit(0.2, 1.0);
        let frame = frame(&orbit, 0.0, 1e6);
        let signs: Vec<bool> = pericentres(&orbit, 12)
            .into_iter()
            .map(|t| {
                let cos = facing(&orbit, &body_fixed_at(&frame, t));
                assert!(cos.abs() > 1.0 - 1e-9, "{cos}");
                cos > 0.0
            })
            .collect();
        assert!(signs.windows(2).all(|w| w[0] != w[1]), "{signs:?}");
    }

    /// (c) `body_fixed_at` is a rotation, orthonormal to 10⁻¹² with determinant 1, at every time,
    /// before, across and after a lock.
    #[test]
    fn body_fixed_at_is_a_rotation() {
        let orbit = orbit(0.05, 0.3);
        for (obliquity, locking) in [(0.0, 1e6), (0.4, 4e9 + 100.0), (3.0, f64::INFINITY)] {
            let frame = frame(&orbit, obliquity, locking);
            for k in -50_i32..50 {
                let t = at(f64::from(k) * 7.3e8);
                let m = body_fixed_at(&frame, t).matrix();
                for i in 0..3 {
                    for j in 0..3 {
                        let expected = if i == j { 1.0 } else { 0.0 };
                        assert!((dot(m[i], m[j]) - expected).abs() < 1e-12, "{i}{j} at {k}");
                    }
                }
                let det = dot(cross(m[0], m[1]), m[2]);
                assert!((det - 1.0).abs() < 1e-12);
            }
        }
    }

    /// The pole makes the obliquity with the orbit's normal.
    #[test]
    fn the_pole_is_tilted_by_the_obliquity() {
        let orbit = orbit(0.05, 0.3);
        let normal = orbit.orientation().normal();
        for obliquity in [0.0, 0.3, 1.7, 3.1] {
            let pole = pole_of(&orbit, Radians::new(obliquity), Radians::new(0.9));
            assert!((math::acos(dot(pole, normal).clamp(-1.0, 1.0)) - obliquity).abs() < 1e-9);
        }
    }

    /// P14.T46.b (b): `to_body_fixed_rotation` is the transpose of the frame's matrix, passes
    /// `BodyFixedRotation::from_rows`'s checks, and maps a body-fixed axis to the inertial one.
    #[test]
    fn to_body_fixed_rotation_is_the_transpose() {
        let orbit = orbit(0.05, 0.3);
        let frame = frame(&orbit, 0.4, 4e9 + 100.0);
        assert!((frame.obliquity().value() - 0.4).abs() < 1e-15);
        let quarter = cross(frame.pole(), frame.equator_node());
        for (a, b) in quarter.iter().zip(frame.equator_quarter()) {
            assert!((a - b).abs() < 1e-15);
        }
        for k in -5_i32..5 {
            let rotation = body_fixed_at(&frame, at(f64::from(k) * 3.1e9));
            let m = rotation.matrix();
            let r = rotation.to_body_fixed_rotation().rows();
            for i in 0..3 {
                for j in 0..3 {
                    assert!((r[i][j] - m[j][i]).abs() < f64::MIN_POSITIVE);
                }
            }
            let meridian = rotation
                .to_body_fixed_rotation()
                .to_body(&crate::coords::BodyFixedPosition::new([1.0, 0.0, 0.0]));
            for (a, b) in meridian.metres().iter().zip(rotation.prime_meridian()) {
                assert!((a - b).abs() < 1e-15);
            }
        }
    }

    /// A position fixed on the rotating body turns once per rotation in the inertial frame.
    #[test]
    fn to_fixed_inverts_the_rotation() {
        let orbit = orbit(0.05, 0.3);
        let frame = frame(&orbit, 0.4, f64::INFINITY);
        let rotation = body_fixed_at(&frame, at(1e5));
        let m = rotation.matrix();
        let inertial = BodyPosition::new(m[1].map(|x| 2.0 * x));
        let fixed = rotation.to_fixed(&inertial);
        assert!(
            (fixed[0]).abs() < 1e-12 && (fixed[1] - 2.0).abs() < 1e-12 && fixed[2].abs() < 1e-12
        );
    }
}
