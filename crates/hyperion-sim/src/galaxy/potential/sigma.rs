//! The bulge's velocity dispersion and the central black hole's mass (plan 02, P02.T6.e and
//! Design note 8).
//!
//! The brainstorm reads the black hole's mass from the M–σ relation at the bulge's projected
//! dispersion ("Galaxy parameters"), which its own model takes from plan 08's axisymmetric Jeans
//! table. Until then this module estimates it directly: the bulge made spherical, an isotropic
//! Jeans solution in the model's in-plane rotation curve, projected, and averaged inside the
//! projected half-mass radius, weighted by mass (plan 02, Risks, R4). The black hole and the
//! nuclear cluster feed the potential that σ is read from, so σ is computed without them; they
//! change it by well under a per cent at the effective radius.
//!
//! For a spherical tracer `ν(r)` the aperture average needs no projection integral: with `W(r)`
//! the volume of the sphere of radius r inside the cylinder of radius `R_e`, `(4π ÷ 3) (r³ − (r² −
//! R_e²)^(3÷2))` beyond `R_e`, integrating the Jeans equation by parts gives `∫ Σ σ_p² dA =
//! ∫₀^∞ W ν v_c² ÷ r dr` and `∫ Σ dA = ∫₀^∞ W ν ÷ a dr` for the exponential `ν = e^(−r ÷ a)`.

use super::model::{MassModel, bulge_second_moments};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::quad::{bisect, gl_panels};
use crate::math;
use crate::units::{Dex, KilometresPerSecond, LightYears, SolarMasses};

/// The M–σ relation of McConnell and Ma (2013, ApJ 764, 184, Table 2, all 72 galaxies):
/// `log₁₀(M ÷ M☉) = 8.32 + 5.64 log₁₀(σ ÷ 200 km/s)`, with an intrinsic scatter of 0.38 dex
/// (the scatter the parameters draw). Re-checked against the paper.
pub const M_SIGMA_INTERCEPT: f64 = 8.32;

/// The slope of the M–σ relation; see [`M_SIGMA_INTERCEPT`].
pub const M_SIGMA_SLOPE: f64 = 5.64;

/// The dispersion at which the M–σ relation is pinned, km/s.
pub const M_SIGMA_PIVOT: f64 = 200.0;

/// Radii at which the rotation curve is sampled for the Jeans integral, in units of the tracer's
/// scale: 16, log-spaced from 0.02 to 50, with cubic Hermite interpolation in `ln r` between
/// them. Inside 0.02 scales the weight of the integrand is below 10⁻⁵ of its peak.
const SAMPLES: usize = 16;
const SAMPLE_RANGE: (f64, f64) = (0.02, 50.0);

/// The index of the last interval between samples, as a float.
const LAST_CELL: f64 = 14.0;

/// The black hole's mass from the bulge's dispersion `sigma` and its offset from the relation,
/// `scatter`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::potential::sigma::black_hole_mass;
/// use hyperion_sim::units::{Dex, KilometresPerSecond};
///
/// // At 200 km/s the relation gives 10^8.32 M☉.
/// let m = black_hole_mass(KilometresPerSecond::new(200.0), Dex::new(0.0));
/// assert!((m.value() / 2.089e8 - 1.0).abs() < 1e-3);
/// ```
#[must_use]
pub fn black_hole_mass(sigma: KilometresPerSecond, scatter: Dex) -> SolarMasses {
    SolarMasses::new(math::exp10(
        M_SIGMA_INTERCEPT
            + M_SIGMA_SLOPE * math::log10(sigma.value() / M_SIGMA_PIVOT)
            + scatter.value(),
    ))
}

/// The scale `a` of the sphericalised bulge `ν ∝ e^(−r ÷ a)`: the spherical exponential with the
/// boxy bulge's `⟨r²⟩ = ⟨R²⟩ + ⟨z²⟩`, which is `12 a²` for it.
#[must_use]
pub fn sphericalised_scale(params: &GalaxyParams) -> LightYears {
    let (r2, z2) = bulge_second_moments(params.bulge());
    LightYears::new(((r2 + z2) / 12.0).sqrt())
}

/// The fraction of a spherical exponential's mass projected outside the cylinder of radius `x`
/// scales: `(1 ÷ 6) ∫_x^∞ (r² − x²)^(3÷2) e^(−r) dr`, by `r = x cosh t`.
fn projected_outside(x: f64) -> f64 {
    let t_end = math::acosh(1.0 + 60.0 / x);
    let edges = [0.0, 0.125, 0.25, 0.5, 1.0].map(|f| f * t_end);
    let x4 = x * x * x * x;
    gl_panels(
        |t| {
            let s = math::sinh(t);
            x4 * s * s * s * s * math::exp(-x * math::cosh(t))
        },
        &edges,
    ) / 6.0
}

/// The projected half-mass radius of a spherical exponential, in scales, by bisection: about
/// 2.0.
#[must_use]
pub fn effective_radius_in_scales() -> f64 {
    bisect(|x| 0.5 - projected_outside(x), 0.5, 5.0, 60)
}

/// `W(x) × 3 ÷ 4π`: the volume of the sphere of radius x inside the cylinder of radius
/// `x_e`, in units of `4π ÷ 3`.
fn cylinder_volume(x: f64, x_e: f64) -> f64 {
    let outside = x * x - x_e * x_e;
    if outside > 0.0 {
        x * x * x - outside * outside.sqrt()
    } else {
        x * x * x
    }
}

/// The mass-weighted projected dispersion inside the effective radius of the exponential tracer
/// of scale `a` (ly), in the rotation curve of `model`, km/s.
pub fn aperture_dispersion(model: &MassModel, a: f64) -> f64 {
    // v_c² and its slope in ln r at the sample radii, everything in the model included.
    let step = math::ln(SAMPLE_RANGE.1 / SAMPLE_RANGE.0) / f64::from(u8::try_from(SAMPLES - 1).expect("16"));
    let mut v2 = [0.0; SAMPLES];
    let mut slope = [0.0; SAMPLES];
    for k in 0..SAMPLES {
        let u = math::ln(SAMPLE_RANGE.0) + step * f64::from(u8::try_from(k).expect("below 16"));
        let r = a * math::exp(u);
        let at = model.extended_in_plane(r);
        let radius = LightYears::new(r);
        v2[k] = at.v_circ_sq;
        slope[k] = at.slope;
        for c in model.spherical() {
            v2[k] += c.v_circ_sq(radius);
            slope[k] += c.v_circ_sq_slope(radius);
        }
    }
    let v_circ_sq = |x: f64| {
        let u = (math::ln(x / SAMPLE_RANGE.0) / step).max(0.0);
        if x < SAMPLE_RANGE.0 {
            // Solid body inside the first sample.
            return v2[0] * (x / SAMPLE_RANGE.0) * (x / SAMPLE_RANGE.0);
        }
        let index = u.floor().min(LAST_CELL);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number clamped to 0–14"
        )]
        let k = index as usize;
        let t = u - index;
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * v2[k]
            + (-2.0 * t3 + 3.0 * t2) * v2[k + 1]
            + step * ((t3 - 2.0 * t2 + t) * slope[k] + (t3 - t2) * slope[k + 1])
    };
    let x_e = effective_radius_in_scales();
    let edges = [0.0, 0.5 * x_e, x_e, 2.0 * x_e, 4.0 * x_e, 8.0 * x_e, 16.0 * x_e, 60.0];
    let weighted = gl_panels(
        |x| {
            if x <= 0.0 {
                return 0.0;
            }
            cylinder_volume(x, x_e) * math::exp(-x) * v_circ_sq(x) / x
        },
        &edges,
    );
    let mass = gl_panels(|x| cylinder_volume(x, x_e) * math::exp(-x), &edges);
    (weighted / mass).sqrt()
}

/// The bulge's projected velocity dispersion inside its effective radius, from the mass model of
/// `params` without the black hole and the nuclear cluster (plan 02, Design note 8).
///
/// The bulge is made spherical as the exponential of the same `⟨r²⟩`
/// ([`sphericalised_scale`]); `σ_r²(r) = (1 ÷ ν) ∫_r^∞ ν v_c² ÷ r′ dr′` is the isotropic Jeans
/// solution in the model's in-plane rotation curve, sampled at 16 radii; the result is the
/// square root of the mass-weighted mean of the projected `σ²` inside the projected half-mass
/// radius ([`effective_radius_in_scales`]).
#[must_use]
pub fn bulge_dispersion(params: &GalaxyParams) -> KilometresPerSecond {
    let model = MassModel::without_centre(params);
    KilometresPerSecond::new(aperture_dispersion(
        &model,
        sphericalised_scale(params).value(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The projected half-mass radius of a spherical exponential solves `x² K₂(x) = 1`, since
    /// the mass outside the cylinder is `x² K₂(x) ÷ 2`: `x = 2.0270`, which a Newton step from
    /// `K₂(2) = 0.253 760` and `d(x² K₂) ÷ dx = −x² K₁(x)`, `K₁(2) = 0.139 866`, confirms.
    #[test]
    fn the_effective_radius_of_an_exponential_sphere() {
        let x = effective_radius_in_scales();
        assert!((x - 2.027_0).abs() < 2e-4, "{x}");
        assert!((projected_outside(1e-3) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_cylinder_volume_is_the_sphere_inside_the_cylinder() {
        assert!((cylinder_volume(1.0, 2.0) - 1.0).abs() < 1e-15);
        // Far out the sphere inside a cylinder of radius 1 tends to the cylinder's volume per
        // unit length times the sphere's diameter: (4π ÷ 3)⁻¹ × π × 2x = 1.5 x.
        let x = 1e4;
        assert!((cylinder_volume(x, 1.0) / (1.5 * x) - 1.0).abs() < 1e-6);
    }
}
