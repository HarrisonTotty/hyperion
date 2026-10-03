//! A patch's normals: from the height's analytic gradient, body-fixed, as octahedral pairs (plan
//! R05, Design note 5).
//!
//! The gradient g of the height field at the spheroid point is taken into the tangent plane,
//! `g_t = g − (g · ν) ν`, and scaled by ρ ÷ (ρ + h), where ρ is the spheroid's normal-section
//! radius of curvature in the direction of `g_t`, 1/ρ = cos²α ÷ M + sin²α ÷ N with α the
//! direction's azimuth from north and M and N the meridional and prime-vertical radii (Euler's
//! theorem; R ÷ (R + h) on a sphere): a height gradient at height h is spread over a surface ρ + h
//! from the centre of curvature. The normal is `(ν − g_s) ÷ |ν − g_s|`, `g_s` the scaled gradient.
//! The scalar factor is exact along the meridian and the prime vertical; in between, the inverse of
//! (I + h S), S the shape operator, is not a multiple of `g_t`, an error of about (h ÷ R) e², 10⁻⁵
//! at 10 km, which the normals do not see.
//!
//! Octahedral encoding (Meyer et al. 2010, "On floating-point normal vectors", Computer Graphics
//! Forum 29, 1405; Cigolle et al. 2014, "A survey of efficient representations for independent
//! unit vectors", JCGT 3, 2): n ÷ (|x| + |y| + |z|) projected onto the octahedron, the lower
//! hemisphere folded over the diagonals, with the sign of a zero taken as +1. The pair is returned
//! as `f32`; the worker packs it into half floats for the GPU, outside this crate, which never
//! reads float bits.

use super::{HeightSource, NormalScale};
use crate::cube::{PATCH_QUADS, PatchKey};
use crate::spheroid::Spheroid;

/// +1 for x ≥ 0 (zeros of either sign included), −1 otherwise.
fn sign_not_zero(x: f64) -> f64 {
    if x < 0.0 { -1.0 } else { 1.0 }
}

/// The octahedral pair of the unit vector `n`.
#[must_use]
pub fn encode_octahedral(n: [f64; 3]) -> [f64; 2] {
    let l1 = n[0].abs() + n[1].abs() + n[2].abs();
    let (x, y) = (n[0] / l1, n[1] / l1);
    if n[2] >= 0.0 {
        [x, y]
    } else {
        [
            (1.0 - y.abs()) * sign_not_zero(x),
            (1.0 - x.abs()) * sign_not_zero(y),
        ]
    }
}

/// The unit vector of the octahedral pair `e`.
#[must_use]
pub fn decode_octahedral(e: [f64; 2]) -> [f64; 3] {
    let (x, y) = (e[0], e[1]);
    let z = 1.0 - x.abs() - y.abs();
    let (x, y) = if z < 0.0 {
        (
            (1.0 - y.abs()) * sign_not_zero(x),
            (1.0 - x.abs()) * sign_not_zero(y),
        )
    } else {
        (x, y)
    };
    let len = (x * x + y * y + z * z).sqrt();
    [x / len, y / len, z / len]
}

/// The surface normal at the unit direction `dir` for a height `h` metres and its gradient
/// `gradient` (body-fixed, per metre) on `figure` (see the module documentation).
#[must_use]
pub fn surface_normal(figure: &Spheroid, dir: [f64; 3], h: f64, gradient: [f64; 3]) -> [f64; 3] {
    let nu = figure.normal(dir);
    let along = gradient[0] * nu[0] + gradient[1] * nu[1] + gradient[2] * nu[2];
    let tangent = [0, 1, 2].map(|a| gradient[a] - along * nu[a]);
    let size = (tangent[0] * tangent[0] + tangent[1] * tangent[1] + tangent[2] * tangent[2]).sqrt();
    let scaled = if size > 0.0 {
        let unit = tangent.map(|t| t / size);
        let rho = figure.section_radius_m(dir, unit);
        let factor = rho / (rho + h);
        tangent.map(|t| t * factor)
    } else {
        tangent
    };
    let n = [0, 1, 2].map(|a| nu[a] - scaled[a]);
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    n.map(|c| c / len)
}

/// The patch's normals at `scale`, row by row, as octahedral pairs narrowed to `f32`.
///
/// # Errors
///
/// The source's own.
pub(crate) fn bake_normals<S: HeightSource>(
    source: &S,
    key: PatchKey,
    scale: NormalScale,
    cache: &mut S::Cache,
) -> Result<Vec<f32>, S::Error> {
    let figure = source.figure();
    let per_patch = match scale {
        NormalScale::Mesh => PATCH_QUADS,
        NormalScale::Double => 2 * PATCH_QUADS,
    };
    let side = scale.side();
    let mut out = Vec::with_capacity(2 * side * side);
    for y in 0..=per_patch {
        for x in 0..=per_patch {
            let dir = key.sample_dir(x, y, per_patch);
            let sample = source.height(cache, dir, key.level())?;
            let n = surface_normal(&figure, dir, sample.height_m, sample.gradient);
            for c in encode_octahedral(n) {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "an octahedral coordinate in [-1, 1] narrows to f32 by design"
                )]
                out.push(c as f32);
            }
        }
    }
    Ok(out)
}
