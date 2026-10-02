//! Body-fixed positions: `f64` metres from a body's centre along axes that turn with the body.
//!
//! A point on a planet's surface, a landing site or the origin of a terrain patch keeps its
//! body-fixed coordinates while the body turns; its [`BodyPosition`] (the non-rotating body frame,
//! axes parallel to the galactic axes) moves at ω × r. [`BodyFixedRotation`] turns one into the
//! other, and nothing else does: no conversion exists without a rotation, and the two position
//! types cannot be mixed.
//!
//! # A position type, not a fourth frame
//!
//! [`Frame`](super::Frame) names where a ship or a camera *is*: the galactic frame, a system's or
//! a body's. The body-fixed axes are not one of those, and never will be. The camera and every
//! craft stay in the non-rotating body frame, which has no rotational fictitious forces (no
//! Coriolis or centrifugal term), so a flight model integrates there; a grounded craft is
//! recorded by its body-fixed position and turned into the body frame by the rotation of the
//! moment. The body frame is free-falling, not inertial: a flight model that adopts it still
//! integrates the other bodies' tidal residual (their pull on the craft less their pull on the
//! body, the indirect term), as Cowell and Encke propagation do (the rendering brainstorm's "The
//! floating origin is already in the simulation"; plan R02, Design note 6).
//!
//! # Direction
//!
//! [`BodyFixedRotation`] maps **body-fixed to body** axes. Its columns are the body-fixed axes
//! expressed in the body frame, so [`BodyFixedRotation::to_body`] is R · p and
//! [`BodyFixedRotation::to_body_fixed`] is Rᵀ · p. Galaxy plan 14's
//! [`body_fixed_at`](crate::planetary::frames::body_fixed_at) (P14.T14.c) returns its own
//! [`FrameRotation`](crate::planetary::frames::FrameRotation), whose rows are the fixed axes: the
//! transpose of this matrix. Plan 14 is asked to return this type, in this direction, so that
//! there is one rotation type; until then `BodyFixedRotation::from_rows` of that matrix's
//! transpose is the conversion.
//!
//! # Arithmetic
//!
//! Every product is written out in a fixed order, `r[0][0]*x + r[0][1]*y + r[0][2]*z`, with no
//! fused multiply-add, so the bits are the same on every target.
//!
//! The types guard against mixing frames:
//!
//! ```compile_fail
//! use hyperion_sim::coords::{BodyFixedPosition, BodyVector};
//! let _ = BodyFixedPosition::new([1.0, 0.0, 0.0]).translated(BodyVector::new([1.0, 0.0, 0.0]));
//! ```
//!
//! ```compile_fail
//! use hyperion_sim::coords::{BodyFixedPosition, BodyPosition};
//! let _: BodyPosition = BodyPosition::from(BodyFixedPosition::new([1.0, 0.0, 0.0]));
//! ```

use std::error::Error;
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

use super::frames::BodyPosition;
use super::vec3;
use crate::units::Metres;

/// The largest departure from orthonormality [`BodyFixedRotation::from_rows`] accepts: every
/// entry of R Rᵀ within this of the identity's.
///
/// A rotation built in `f64` from trigonometry and a cross product departs by a few ulp, about
/// 10⁻¹⁶, so this refuses only a matrix that is wrong; and a departure this size moves a point at
/// 6,371 km by at most 1.2 × 10⁻⁹ m through a round trip, inside the 10⁻⁸ m that a round trip at a
/// planet's surface is held to (plan R02, R02.T3).
pub const ROTATION_ORTHONORMAL_TOLERANCE: f64 = 1e-12;

/// A position in a body's body-fixed frame: `f64` metres from the body's centre, along axes that
/// turn with the body.
///
/// Resolution is sub-micrometre at a planet's surface.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{BodyFixedPosition, BodyFixedRotation};
///
/// // A landing site on the prime meridian at the equator of a 6,371 km body.
/// let site = BodyFixedPosition::new([6.371e6, 0.0, 0.0]);
/// let now = BodyFixedRotation::IDENTITY.to_body(&site);
/// assert_eq!(now.metres(), site.metres());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BodyFixedPosition([f64; 3]);

impl BodyFixedPosition {
    /// The body's centre.
    pub const ORIGIN: Self = Self([0.0; 3]);

    /// A position from its components in metres from the body's centre, along the body-fixed
    /// axes.
    #[must_use]
    pub const fn new(metres: [f64; 3]) -> Self {
        Self(metres)
    }

    /// The components in metres from the body's centre, along the body-fixed axes.
    #[must_use]
    pub const fn metres(&self) -> [f64; 3] {
        self.0
    }

    /// The distance from the body's centre.
    #[must_use]
    pub fn distance_from_origin(&self) -> Metres {
        Metres::new(vec3::length(self.0))
    }

    /// This position moved by a displacement along the body-fixed axes.
    #[must_use]
    pub fn translated(&self, displacement: BodyFixedVector) -> Self {
        Self(vec3::add(self.0, displacement.0))
    }

    /// The displacement from this position to `other`: `other − self`.
    #[must_use]
    pub fn displacement_to(&self, other: &Self) -> BodyFixedVector {
        BodyFixedVector(vec3::sub(other.0, self.0))
    }
}

/// A displacement along a body's body-fixed axes: `f64` metres.
///
/// A terrain vertex's offset from its patch's origin is one of these.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BodyFixedVector([f64; 3]);

impl BodyFixedVector {
    /// The zero displacement.
    pub const ZERO: Self = Self([0.0; 3]);

    /// A displacement from its components in metres along the body-fixed axes.
    #[must_use]
    pub const fn new(metres: [f64; 3]) -> Self {
        Self(metres)
    }

    /// The components in metres along the body-fixed axes.
    #[must_use]
    pub const fn metres(&self) -> [f64; 3] {
        self.0
    }

    /// The length in metres.
    #[must_use]
    pub fn length(&self) -> Metres {
        Metres::new(vec3::length(self.0))
    }
}

/// A displacement in a body's non-rotating frame: `f64` metres along the galactic axes.
///
/// The body-frame counterpart of [`BodyFixedVector`], as [`SystemVector`](super::SystemVector)
/// is the system frame's.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BodyVector([f64; 3]);

impl BodyVector {
    /// The zero displacement.
    pub const ZERO: Self = Self([0.0; 3]);

    /// A displacement from its components in metres along the galactic axes.
    #[must_use]
    pub const fn new(metres: [f64; 3]) -> Self {
        Self(metres)
    }

    /// The components in metres along the galactic axes.
    #[must_use]
    pub const fn metres(&self) -> [f64; 3] {
        self.0
    }

    /// The length in metres.
    #[must_use]
    pub fn length(&self) -> Metres {
        Metres::new(vec3::length(self.0))
    }
}

macro_rules! vector_ops {
    ($t:ty) => {
        impl Add for $t {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self(vec3::add(self.0, rhs.0))
            }
        }

        impl Sub for $t {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                Self(vec3::sub(self.0, rhs.0))
            }
        }

        impl Neg for $t {
            type Output = Self;
            fn neg(self) -> Self {
                Self(vec3::neg(self.0))
            }
        }

        impl Mul<f64> for $t {
            type Output = Self;
            fn mul(self, rhs: f64) -> Self {
                Self(vec3::scale(self.0, rhs))
            }
        }
    };
}

vector_ops!(BodyFixedVector);
vector_ops!(BodyVector);

/// A [`BodyFixedRotation`] could not be built from the rows given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildRotationError {
    /// An entry is infinite or NaN.
    NotFinite,
    /// Some entry of R Rᵀ departs from the identity's by more than
    /// [`ROTATION_ORTHONORMAL_TOLERANCE`].
    NotOrthonormal,
    /// The matrix is orthonormal but its determinant is −1: a reflection, not a rotation.
    Reflection,
}

impl fmt::Display for BuildRotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite => write!(f, "rotation matrix has an entry that is not finite"),
            Self::NotOrthonormal => write!(
                f,
                "rotation matrix is not orthonormal to {ROTATION_ORTHONORMAL_TOLERANCE:e}"
            ),
            Self::Reflection => write!(f, "rotation matrix is a reflection (determinant below 0)"),
        }
    }
}

impl Error for BuildRotationError {}

/// The rotation from a body's body-fixed axes to its non-rotating body frame, at one moment.
///
/// Its columns are the body-fixed axes expressed in the body frame (the galactic axes), so
/// [`to_body`](Self::to_body) is R · p and [`to_body_fixed`](Self::to_body_fixed) is Rᵀ · p. It is
/// orthonormal with determinant +1 to [`ROTATION_ORTHONORMAL_TOLERANCE`], which
/// [`from_rows`](Self::from_rows) checks once.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{BodyFixedPosition, BodyFixedRotation};
///
/// // A body turned +90° about its pole (+z): the body-fixed x axis now points along +y.
/// let turned = BodyFixedRotation::from_rows([
///     [0.0, -1.0, 0.0],
///     [1.0, 0.0, 0.0],
///     [0.0, 0.0, 1.0],
/// ])?;
/// let prime_meridian = BodyFixedPosition::new([1.0, 0.0, 0.0]);
/// assert_eq!(turned.to_body(&prime_meridian).metres(), [0.0, 1.0, 0.0]);
/// # Ok::<(), hyperion_sim::coords::BuildRotationError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyFixedRotation {
    rows: [[f64; 3]; 3],
}

impl BodyFixedRotation {
    /// No rotation: the body-fixed axes are the body frame's.
    pub const IDENTITY: Self = Self {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    /// A rotation from its rows, row-major: `rows[i][j]` is the body frame's axis i component of
    /// the body-fixed axis j.
    ///
    /// # Errors
    ///
    /// - [`BuildRotationError::NotFinite`] if any entry is infinite or NaN.
    /// - [`BuildRotationError::NotOrthonormal`] if some entry of R Rᵀ departs from the identity's
    ///   by more than [`ROTATION_ORTHONORMAL_TOLERANCE`].
    /// - [`BuildRotationError::Reflection`] if the determinant is below zero.
    pub fn from_rows(rows: [[f64; 3]; 3]) -> Result<Self, BuildRotationError> {
        if !rows.iter().all(|row| vec3::is_finite(*row)) {
            return Err(BuildRotationError::NotFinite);
        }
        for (i, a) in rows.iter().enumerate() {
            for (j, b) in rows.iter().enumerate() {
                let expected = if i == j { 1.0 } else { 0.0 };
                if (vec3::dot(*a, *b) - expected).abs() > ROTATION_ORTHONORMAL_TOLERANCE {
                    return Err(BuildRotationError::NotOrthonormal);
                }
            }
        }
        if vec3::dot(rows[0], vec3::cross(rows[1], rows[2])) < 0.0 {
            return Err(BuildRotationError::Reflection);
        }
        Ok(Self { rows })
    }

    /// The rows, row-major, as [`from_rows`](Self::from_rows) took them.
    #[must_use]
    pub const fn rows(&self) -> [[f64; 3]; 3] {
        self.rows
    }

    /// The body-frame position of a body-fixed position: R · p.
    #[must_use]
    pub fn to_body(&self, p: &BodyFixedPosition) -> BodyPosition {
        BodyPosition::new(self.apply(p.0))
    }

    /// The body-fixed position of a body-frame position: Rᵀ · p.
    #[must_use]
    pub fn to_body_fixed(&self, p: &BodyPosition) -> BodyFixedPosition {
        BodyFixedPosition(self.apply_transpose(p.metres()))
    }

    /// The body-frame displacement of a body-fixed displacement: R · v.
    #[must_use]
    pub fn vector_to_body(&self, v: &BodyFixedVector) -> BodyVector {
        BodyVector(self.apply(v.0))
    }

    /// The body-fixed displacement of a body-frame displacement: Rᵀ · v.
    #[must_use]
    pub fn vector_to_body_fixed(&self, v: &BodyVector) -> BodyFixedVector {
        BodyFixedVector(self.apply_transpose(v.0))
    }

    /// R · v, each component a dot product of a row with v in the fixed order x, y, z.
    fn apply(&self, v: [f64; 3]) -> [f64; 3] {
        let r = &self.rows;
        [
            r[0][0] * v[0] + r[0][1] * v[1] + r[0][2] * v[2],
            r[1][0] * v[0] + r[1][1] * v[1] + r[1][2] * v[2],
            r[2][0] * v[0] + r[2][1] * v[1] + r[2][2] * v[2],
        ]
    }

    /// Rᵀ · v, each component a dot product of a column with v in the fixed order x, y, z.
    fn apply_transpose(&self, v: [f64; 3]) -> [f64; 3] {
        let r = &self.rows;
        [
            r[0][0] * v[0] + r[1][0] * v[1] + r[2][0] * v[2],
            r[0][1] * v[0] + r[1][1] * v[1] + r[2][1] * v[2],
            r[0][2] * v[0] + r[1][2] * v[1] + r[2][2] * v[2],
        ]
    }
}

impl Default for BodyFixedRotation {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::math;

    /// Earth's mean radius, m (IAU 2015 nominal volumetric radius, 6,371 km).
    const EARTH_RADIUS_M: f64 = 6.371e6;

    fn quarter_turn_about_z() -> BodyFixedRotation {
        BodyFixedRotation::from_rows([[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]).unwrap()
    }

    /// A rotation about a pole tilted from +z, at prime-meridian angle `angle` (radians, already
    /// reduced by the caller). Its columns are the fixed x axis, cos · e₁ + sin · e₂; the fixed y
    /// axis, −sin · e₁ + cos · e₂; and the pole.
    fn earth_like(angle: f64) -> BodyFixedRotation {
        // The orbit normal tilted by the obliquity about x: Earth's pole in axes whose z is the
        // orbit normal (the orbit map's assumption for an unrotated body). Earth's obliquity,
        // 23.44° (IERS Conventions 2010, IAU 2006 precession: ε₀ = 84,381.406″).
        let tilt = 23.44_f64.to_radians();
        let (s, c) = math::sin_cos(tilt);
        let pole = [0.0, -s, c];
        let e1 = [1.0, 0.0, 0.0];
        let e2 = vec3::cross(pole, e1);
        let (sw, cw) = math::sin_cos(angle);
        let x = vec3::add(vec3::scale(e1, cw), vec3::scale(e2, sw));
        let y = vec3::add(vec3::scale(e1, -sw), vec3::scale(e2, cw));
        let columns = [x, y, pole];
        let rows = [
            [columns[0][0], columns[1][0], columns[2][0]],
            [columns[0][1], columns[1][1], columns[2][1]],
            [columns[0][2], columns[1][2], columns[2][2]],
        ];
        BodyFixedRotation::from_rows(rows).unwrap()
    }

    #[test]
    fn identity_round_trips_bit_for_bit() {
        let p = BodyFixedPosition::new([6.371e6, -1.25e-3, 3.0e5]);
        let body = BodyFixedRotation::IDENTITY.to_body(&p);
        for (a, b) in body.metres().into_iter().zip(p.metres()) {
            assert_same_bits(a, b);
        }
        let back = BodyFixedRotation::IDENTITY.to_body_fixed(&body);
        for (a, b) in back.metres().into_iter().zip(p.metres()) {
            assert_same_bits(a, b);
        }
        assert_eq!(BodyFixedRotation::default(), BodyFixedRotation::IDENTITY);
    }

    #[test]
    fn a_quarter_turn_maps_the_prime_meridian_to_plus_y() {
        let r = quarter_turn_about_z();
        let same = |a: [f64; 3], b: [f64; 3]| {
            for (x, y) in a.into_iter().zip(b) {
                assert_same_bits(x, y);
            }
        };
        let (x_hat, y_hat) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        same(r.to_body(&BodyFixedPosition::new(x_hat)).metres(), y_hat);
        same(r.to_body_fixed(&BodyPosition::new(y_hat)).metres(), x_hat);
        same(
            r.vector_to_body(&BodyFixedVector::new(x_hat)).metres(),
            y_hat,
        );
        same(
            r.vector_to_body_fixed(&BodyVector::new(y_hat)).metres(),
            x_hat,
        );
    }

    #[test]
    fn a_century_of_earth_rotation_round_trips_to_ten_nanometres() {
        // 2.2 × 10⁵ rad is about a century of Earth's rotation (the brainstorm's figure), reduced
        // by the caller to [0, 2π) before the rotation is built.
        let w = 2.2e5_f64 % std::f64::consts::TAU;
        let r = earth_like(w);
        let mut rng = hyperion_testkit::lcg::Lcg::new(0x0b0d_f1ed);
        for _ in 0..1000 {
            let lon = rng.next_f64() * std::f64::consts::TAU;
            let z = rng.next_f64() * 2.0 - 1.0;
            let rho = (1.0 - z * z).sqrt();
            let (sl, cl) = math::sin_cos(lon);
            let p = BodyFixedPosition::new([
                EARTH_RADIUS_M * rho * cl,
                EARTH_RADIUS_M * rho * sl,
                EARTH_RADIUS_M * z,
            ]);
            let back = r.to_body_fixed(&r.to_body(&p));
            for (a, b) in back.metres().into_iter().zip(p.metres()) {
                assert!((a - b).abs() <= 1e-8, "{a} against {b}");
            }
        }
    }

    #[test]
    fn the_orthonormality_limit_is_ten_to_the_minus_twelve() {
        let with = |e: f64| {
            BodyFixedRotation::from_rows([[1.0, e, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
        };
        assert!(with(5e-13).is_ok());
        assert_eq!(with(2e-12), Err(BuildRotationError::NotOrthonormal));
    }

    #[test]
    fn non_rotations_are_refused() {
        assert_eq!(
            BodyFixedRotation::from_rows([[1.0, 1e-9, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
            Err(BuildRotationError::NotOrthonormal)
        );
        assert_eq!(
            BodyFixedRotation::from_rows([[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
            Err(BuildRotationError::NotOrthonormal)
        );
        assert_eq!(
            BodyFixedRotation::from_rows([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]),
            Err(BuildRotationError::Reflection)
        );
        assert_eq!(
            BodyFixedRotation::from_rows([[f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
            Err(BuildRotationError::NotFinite)
        );
        assert_eq!(
            BuildRotationError::Reflection.to_string(),
            "rotation matrix is a reflection (determinant below 0)"
        );
    }

    #[test]
    fn body_fixed_positions_and_vectors_compose() {
        let a = BodyFixedPosition::new([1.0, 2.0, 3.0]);
        let step = BodyFixedVector::new([0.5, -2.0, 1.0]);
        let b = a.translated(step);
        assert_eq!(a.displacement_to(&b), step);
        assert_eq!(step + BodyFixedVector::ZERO - step, BodyFixedVector::ZERO);
        assert_eq!(-step * 2.0, BodyFixedVector::new([-1.0, 4.0, -2.0]));
        assert_same_bits(
            BodyFixedVector::new([3.0, 4.0, 12.0]).length().value(),
            13.0,
        );
        assert_same_bits(BodyVector::new([3.0, 4.0, 12.0]).length().value(), 13.0);
        assert_same_bits(
            BodyFixedPosition::new([0.0, 3.0, 4.0])
                .distance_from_origin()
                .value(),
            5.0,
        );
        assert_eq!(
            BodyVector::new([1.0, 0.0, 0.0]) - BodyVector::ZERO,
            -BodyVector::new([-1.0, 0.0, 0.0])
        );
    }
}
