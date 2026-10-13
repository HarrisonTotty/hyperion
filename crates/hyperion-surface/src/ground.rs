//! The authoritative ground: R05's collision interpolant over R09's synthesis, with the triangle
//! it came from (plan R10, T3 and Design note 4).
//!
//! The brainstorm defines the ground as "the piecewise-linear surface through the finest level's
//! vertices, on the same triangle diagonal as the mesh". R05 built that interpolant over any
//! [`HeightSource`] ([`finest_surface_height`](crate::patch::collision::finest_surface_height)),
//! and this module points it at R09's [`Synthesiser`], which it implements [`HeightSource`] for:
//! R09's [`BandLevel`] from R05's `u8` level, [`SynthCache`] as the cache, [`QueryHeightError`] as
//! the error, and R09's level bound and patch range forwarded. [`ground_at`] wraps it with the
//! triangle and its normal ([`GroundPoint`]); collision reads it and nothing else, and R11 stands
//! its rocks on it.
//!
//! # A point and its ground
//!
//! [`ground_at`] takes a body-fixed point, not a direction. The ground under it is the one along
//! the spheroid's normal through it: the point's foot on the datum, the unit direction d with
//! p = M·d + h·ν(d) (R05's Design note 5), found by Newton's method in the point's meridian plane
//! (`datum_foot`). The geocentric direction p ÷ |p| would put the query about f sin 2φ ÷ 2 of arc
//! away (some 11 km of an Earth's ground at 45°), and the datum's own direction of the point,
//! M⁻¹p ÷ |M⁻¹p|, exact on the datum, about |h| f sin 2φ ÷ a away at a height h (about 34 m at
//! 10 km and 45°), as the client's `datumDirection` records.
//! The height returned is the ground's at d, along ν(d), the datum of R07's reference spheroid
//! (R09's Design note 17).
//!
//! # What agrees with what
//!
//! At a vertex the ground is R09's [`Synthesiser::height_at`] there at the finest level, bit for
//! bit; on a shared edge it is the same from either triangle, across a cube-face edge too; and
//! interpolating in face coordinates, as the interpolant does, or in space, on the flat triangle
//! through the vertices' surface points, differs by the chord's sagitta and the warp's curvature
//! over one quad, nanometres (Design note 4). The drawn finest mesh carries the same heights
//! narrowed to `f32`, so collision and the picture agree to that narrowing (about 2 mm at
//! ±20 km): this module's tests hold all four on the synthetic worlds.

use hyperion_base::units::Metres;

use crate::cube::PatchKey;
use crate::field::FieldView;
use crate::height::HeightSample;
use crate::num::assert_finite;
use crate::patch::HeightSource;
use crate::patch::collision::{MeshTriangle, finest_surface_point};
use crate::spheroid::Spheroid;
use crate::synth::{BandLevel, QueryHeightError, SynthCache, Synthesiser};

/// A triangle of the finest level's mesh: its patch, at the finest level, its quad and the quad's
/// half (R05's [`MeshTriangle`]).
pub type FinestTriangle = MeshTriangle;

/// The authoritative ground under a point: its height, the triangle it lies on and that
/// triangle's normal, with the point's own place above the datum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundPoint {
    height: Metres,
    normal: [f64; 3],
    triangle: FinestTriangle,
    dir: [f64; 3],
    point_height: Metres,
}

impl GroundPoint {
    /// The ground's height, metres above R07's reference spheroid along its normal at
    /// [`dir`](Self::dir): R05's collision interpolant there at the finest level.
    #[must_use]
    pub const fn height(&self) -> Metres {
        self.height
    }

    /// The outward unit normal, body-fixed, of the flat triangle the ground lies on there.
    #[must_use]
    pub const fn normal(&self) -> [f64; 3] {
        self.normal
    }

    /// The finest level's triangle the ground lies on there.
    #[must_use]
    pub const fn triangle(&self) -> FinestTriangle {
        self.triangle
    }

    /// The unit direction d of the point's foot on the datum, body-fixed: the point is
    /// M·d + h·ν(d), h its [`point_height`](Self::point_height).
    #[must_use]
    pub const fn dir(&self) -> [f64; 3] {
        self.dir
    }

    /// The point's own height above the datum along the same normal, metres: it is above the
    /// ground where this exceeds [`height`](Self::height).
    #[must_use]
    pub const fn point_height(&self) -> Metres {
        self.point_height
    }
}

/// The authoritative ground under the body-fixed point `p`, metres from the body's centre: R05's
/// collision interpolant over `synth` at the finest level, at the point's foot on the datum, with
/// the triangle it came from and that triangle's normal (see the [module documentation](self)).
///
/// `cache` is the caller's, and no result depends on what it holds. Every value is asserted
/// finite before it is returned (the brainstorm's NaN hazard).
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell a vertex's height reads that the view
/// does not hold.
///
/// # Panics
///
/// If `p` is not finite, is the body's centre, or is so far from it (beyond about 10¹⁵⁴ m) that
/// its squared distance from the axis overflows; or if a height is not finite (a bug).
pub fn ground_at<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    p: &[f64; 3],
) -> Result<GroundPoint, QueryHeightError> {
    ground_over(synth, cache, p)
}

/// [`ground_at`] over any height source: the tests' analytic fields too.
pub(crate) fn ground_over<S: HeightSource>(
    source: &S,
    cache: &mut S::Cache,
    p: &[f64; 3],
) -> Result<GroundPoint, S::Error> {
    let (dir, point_height) = datum_foot(&source.figure(), p);
    let point = finest_surface_point(source, dir, cache)?;
    let normal = point.normal();
    // Each part asserts its own, and this is the net under them all, before anything leaves.
    assert!(
        normal.iter().chain(&dir).all(|x| x.is_finite()),
        "the ground's normal and foot must be finite, got {normal:?} and {dir:?}"
    );
    Ok(GroundPoint {
        height: Metres::new(assert_finite(point.height_m())),
        normal,
        triangle: point.triangle(),
        dir,
        point_height: Metres::new(assert_finite(point_height)),
    })
}

/// The most Newton steps [`datum_foot`] takes. Newton's method doubles the digits it has at each
/// step, and from the datum's own direction of the point a point within tens of kilometres of an
/// Earth's datum starts with five, so it ends in two or three; the cap bounds the loop for a point
/// inside the evolute, where the step may not settle.
const FOOT_STEPS: u32 = 16;

/// The step, in radians of parametric latitude, below which [`datum_foot`] stops: the rounding of
/// a unit vector's components.
const FOOT_TOLERANCE: f64 = 2.0 * f64::EPSILON;

/// The foot of the body-fixed point `p` on `figure`, the unit direction d whose normal passes
/// through it, and its height h along that normal, metres: the inverse of
/// [`Spheroid::surface_point`], p = M·d + h·ν(d).
///
/// In the point's meridian plane, at a distance ρ from the axis and z along it, the datum's point
/// of parametric latitude u is (a cos u, c sin u), its normal is along (c cos u, a sin u), and the
/// foot is the root of f(u) = a ρ sin u − c z cos u − (a² − c²) sin u cos u. Newton's method from
/// the datum's own direction, d = M⁻¹p ÷ |M⁻¹p| (exact for a point on the datum), turns
/// (cos u, sin u) by atan of each step and renormalises, so it takes no trigonometric function;
/// then d = (cos u ρ̂, sin u). A sphere's foot is its direction, the first step's being zero.
///
/// The foot is unique on p's side of the axis, the datum's nearest point to p, for any point
/// outside the spheroid's evolute, which lies within (a² − c²) ÷ c of the centre (43 km on
/// WGS 84); a point inside it has several, of which this is the one Newton's method reaches from
/// the datum's direction. Its form, the start, the terms of f and its slope in their order, the
/// turn, [`FOOT_TOLERANCE`] and [`FOOT_STEPS`], is pinned output: R10's ground goldens carry its
/// bits.
///
/// # Panics
///
/// If `p` is not finite, is the body's centre, or is beyond about 10¹⁵⁴ m from it, where its
/// squared distance from the axis overflows.
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "the textbook names of the radii a and c, the point p, its axial z and the height h"
)]
pub(crate) fn datum_foot(figure: &Spheroid, p: &[f64; 3]) -> ([f64; 3], f64) {
    assert!(
        p.iter().all(|x| x.is_finite()) && p.iter().any(|x| x.abs() > 0.0),
        "a point must be finite and not the body's centre, got {p:?}"
    );
    let (a, c) = (figure.equatorial_radius_m, figure.polar_radius_m);
    let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
    let z = p[2];
    let dir = if rho > 0.0 {
        let (mut cos_u, mut sin_u) = unit_2(rho / a, z / c);
        let k = (a - c) * (a + c);
        for _ in 0..FOOT_STEPS {
            let f = a * rho * sin_u - c * z * cos_u - k * sin_u * cos_u;
            let slope = a * rho * cos_u + c * z * sin_u - k * (cos_u * cos_u - sin_u * sin_u);
            let step = f / slope;
            if !step.is_finite() {
                break;
            }
            (cos_u, sin_u) = unit_2(cos_u + step * sin_u, sin_u - step * cos_u);
            if step.abs() <= FOOT_TOLERANCE {
                break;
            }
        }
        crate::cube::unit_dir([cos_u * p[0] / rho, cos_u * p[1] / rho, sin_u])
    } else {
        // On the axis: a pole, z non-zero since p is not the centre.
        [0.0, 0.0, z.signum()]
    };
    let on_datum = figure.point(dir);
    let nu = figure.normal(dir);
    let h =
        (p[0] - on_datum[0]) * nu[0] + (p[1] - on_datum[1]) * nu[1] + (p[2] - on_datum[2]) * nu[2];
    (dir, assert_finite(h))
}

/// (`x`, `y`) scaled to unit length.
#[must_use]
fn unit_2(x: f64, y: f64) -> (f64, f64) {
    let len = (x * x + y * y).sqrt();
    (x / len, y / len)
}

/// R05's height source over R09's synthesis (plan R10, T3): a level's height is
/// [`Synthesiser::height_at`] at that band level, and the bound and range are R09.T8's.
impl<F: FieldView> HeightSource for Synthesiser<'_, F> {
    type Cache = SynthCache;
    type Error = QueryHeightError;

    /// The field's datum, its header's spheroid.
    fn figure(&self) -> Spheroid {
        self.field().header().figure()
    }

    /// [`Synthesiser::height_at`] at band level `level`.
    ///
    /// # Panics
    ///
    /// If `level` is above [`crate::cube::MAX_LEVEL`], and as [`Synthesiser::height_at`].
    fn height(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: u8,
    ) -> Result<HeightSample, QueryHeightError> {
        self.height_at(cache, dir, band(level))
    }

    /// R09.T8's level bound ([`crate::synth::level_bound_m`], from the bounds the synthesiser
    /// holds): hard in its structural and channel terms, and a 1 − 10⁻⁹ per-point quantile in its
    /// crater and rim terms ([`crate::synth::levels::CRATER_BOUND_TAIL`]).
    ///
    /// # Panics
    ///
    /// If `level` is above [`crate::cube::MAX_LEVEL`].
    fn level_bound_m(&self, level: u8) -> f64 {
        Synthesiser::level_bound_m(self, band(level))
    }

    /// [`Synthesiser::height_range_m`], which carries the same quantile in its crater terms.
    fn height_range_m(&self, key: PatchKey) -> (f64, f64) {
        Synthesiser::height_range_m(self, key)
    }
}

/// R05's `u8` level as R09's band level.
///
/// # Panics
///
/// If `level` is above [`crate::cube::MAX_LEVEL`]: no patch, and so no query of R05's, has one.
#[must_use]
fn band(level: u8) -> BandLevel {
    BandLevel::new(level).unwrap_or_else(|e| panic!("{e}: R05's levels are band levels"))
}

#[cfg(test)]
mod golden;
#[cfg(test)]
mod tests;
