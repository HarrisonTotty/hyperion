//! The bulge's velocity dispersion and the central black hole's mass (plan 02, P02.T6.e and
//! Design note 8; plan 08, P08.T4.d).
//!
//! The brainstorm reads the black hole's mass from the M–σ relation at the bulge's projected
//! dispersion ("Galaxy parameters"), which its own model takes from plan 08's axisymmetric Jeans
//! solution. [`bulge_dispersion`] is that: plan 08's
//! [`bulge_projected_sigma`](crate::galaxy::kinematics::spheroid::bulge_projected_sigma), the
//! bulge's line-of-sight dispersion seen face-on and mass-weighted inside its effective radius,
//! solved straight from the mass model without the black hole and the nuclear cluster, since the
//! black hole's mass is what it sets (the two-phase build of P02.T6.e). It replaced plan 02's
//! spherical, isotropic estimate in plan 08's P08.T4.d.
//!
//! The effective radius of the axisymmetrised bulge seen face-on is that of a spherical
//! exponential of its radial scale, [`EFFECTIVE_RADIUS_IN_SCALES`] of it.

use super::model::MassModel;
use crate::galaxy::kinematics::spheroid::bulge_projected_sigma;
use crate::galaxy::params::GalaxyParams;
#[cfg(test)]
use crate::galaxy::quad::gl_panels;
use crate::math;
use crate::units::{Dex, KilometresPerSecond, SolarMasses};

/// The M–σ relation of McConnell and Ma (2013, ApJ 764, 184, Table 2, all 72 galaxies):
/// `log₁₀(M ÷ M☉) = 8.32 + 5.64 log₁₀(σ ÷ 200 km/s)`, with an intrinsic scatter of 0.38 dex
/// (the scatter the parameters draw). Re-checked against the paper.
pub const M_SIGMA_INTERCEPT: f64 = 8.32;

/// The slope of the M–σ relation; see [`M_SIGMA_INTERCEPT`].
pub const M_SIGMA_SLOPE: f64 = 5.64;

/// The dispersion at which the M–σ relation is pinned, km/s.
pub const M_SIGMA_PIVOT: f64 = 200.0;

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

/// The fraction of a spherical exponential's mass projected outside the cylinder of radius `x`
/// scales: `(1 ÷ 6) ∫_x^∞ (r² − x²)^(3÷2) e^(−r) dr`, by `r = x cosh t`. Only the tests evaluate
/// it, to pin [`EFFECTIVE_RADIUS_IN_SCALES`].
#[cfg(test)]
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

/// The projected half-mass radius of a spherical exponential, in scales: the root of `x² K₂(x) =
/// 1`.
///
/// It is found by bisection on the mass fraction projected outside the cylinder of radius `x`,
/// 60 steps from `[0.5, 5]`, and written down here because it is the same for every galaxy:
/// recomputing it took 0.4 ms of every parameter build. A test repeats the bisection and
/// requires these very bits.
pub const EFFECTIVE_RADIUS_IN_SCALES: f64 = 2.026_996_389_655_237_4;

/// The bulge's projected velocity dispersion inside its effective radius, seen face-on, from the
/// mass model of `params` without the black hole and the nuclear cluster (plan 08, P08.T4.d, which
/// replaced plan 02's Design note 8 estimate).
///
/// It is [`bulge_projected_sigma`] of [`MassModel::without_centre`]: some 64 evaluations of the
/// model's vertical force, a few tens of milliseconds of the parameters' build.
#[must_use]
pub fn bulge_dispersion(params: &GalaxyParams) -> KilometresPerSecond {
    bulge_projected_sigma(&MassModel::without_centre(params), params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::quad::bisect;

    /// The projected half-mass radius of a spherical exponential solves `x² K₂(x) = 1`, since
    /// the mass outside the cylinder is `x² K₂(x) ÷ 2`: `x = 2.0270`, which a Newton step from
    /// `K₂(2) = 0.253 760` and `d(x² K₂) ÷ dx = −x² K₁(x)`, `K₁(2) = 0.139 866`, confirms.
    #[test]
    fn the_effective_radius_of_an_exponential_sphere() {
        let x = EFFECTIVE_RADIUS_IN_SCALES;
        assert!((x - 2.027_0).abs() < 2e-4, "{x}");
        assert!((projected_outside(x) - 0.5).abs() < 1e-14);
        assert!((projected_outside(1e-3) - 1.0).abs() < 1e-6);
    }

    /// The constant is the bisection's result to the last bit, so writing it down changed no
    /// output.
    #[test]
    fn the_effective_radius_is_the_bisected_root() {
        let bisected = bisect(|x| 0.5 - projected_outside(x), 0.5, 5.0, 60);
        assert!(
            bisected.total_cmp(&EFFECTIVE_RADIUS_IN_SCALES).is_eq(),
            "{bisected:?}"
        );
    }
}
