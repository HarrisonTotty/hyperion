//! The bulge's velocity dispersion and the central black hole's mass (plan 02, P02.T6.e and
//! Design note 8; plan 08, P08.T4.d).
//!
//! The brainstorm reads the black hole's mass from the M–σ relation at the bulge's projected
//! dispersion ("Galaxy parameters"), which its own model takes from plan 08's axisymmetric Jeans
//! solution. [`bulge_dispersion`] is that: plan 08's
//! [`bulge_projected_sigma`], the
//! relation's own `σ_e` (the line-of-sight `V² + σ²` along the major axis inside the effective radius,
//! averaged over inclination; ruling 105.1), solved straight from the mass model without the black hole and the nuclear cluster, since the
//! black hole's mass is what it sets (the two-phase build of P02.T6.e). It replaced plan 02's
//! spherical, isotropic estimate in plan 08's P08.T4.d.
//!
//! The effective radius is that of the bulge's tracer, its own body made axisymmetric in the plane
//! (plan 15, P15.T3.c): its projected half-mass radius face-on, [`effective_radius_in_scales`] of
//! its in-plane scale.

use super::model::MassModel;
use crate::galaxy::kinematics::spheroid::bulge_projected_sigma;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::quad::bisect;
use crate::math;
use crate::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS};
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

/// The panels in `t` of [`effective_radius_in_scales`]' integrals, each a 32-point rule: the
/// folded integrand changes fastest towards `t = 0`, where `t Γ(3, k X ÷ t)` switches on.
const PROJECTION_PANELS: [f64; 6] = [0.0, 0.0625, 0.125, 0.25, 0.5, 1.0];

/// The projected half-mass radius face-on of the body `exp(−((R ÷ s)^c + (|z| ÷ h)^c)^(1 ÷ c))`,
/// in units of `s`, for its boxiness `c`: the effective radius of the bulge's tracer (plan 02,
/// P02.T6.e, "the projected half-mass radius, by bisection"; plan 08, P08.T4.d; the tracer is the
/// bulge made axisymmetric, plan 15's P15.T3.c). `c` is finite and at least 1, where `m` is a norm
/// ([`BoxyBulge`](crate::galaxy::fields::bulge::BoxyBulge)); the galaxy draws it in 3–4, and the
/// tests hold 2–4.
///
/// With `z = R t` (in units of `s` and `h`, which do not change the radius in units of `s`) and
/// `k(t) = (1 + t^c)^(1 ÷ c)`, the body is `e^(−R k(t))`, so the mass outside the cylinder of
/// radius `X` is `4π ∫₀^∞ Γ(3, k X) ÷ k³ dt`, the radial integral in closed form with `Γ(3, y) =
/// e^(−y) (y² + 2y + 2)`, of a whole `8π ∫₀^∞ k^(−3) dt`. Since `k(1 ÷ t) = k(t) ÷ t`, the map `t →
/// 1 ÷ t` folds the range beyond 1 onto `[0, 1]`: outside is `∫₀¹ k^(−3) (Γ(3, k X) + t Γ(3, k X ÷
/// t)) dt` and the whole `2 ∫₀¹ k^(−3) (1 + t) dt`, both by `gl32` on five panels that narrow
/// towards `t = 0`. The radius is 60 bisections of the share outside against ½, from `[0.5, 5]`.
/// At `c = 2` it is the spherical exponential's root of `x² K₂(x) = 1`, 2.0270; at the boxiness's
/// 3, 3.5 and 4 it is 2.1501, 2.1845 and 2.2098 (tests), against 2.1218, 2.1485 and 2.1682 for the
/// spheroidal exponential of the same second moments that the tracer was until P15.T3.c. About
/// 20,000 exponentials, a fraction of a millisecond per galaxy.
///
/// # Panics
///
/// In builds with debug assertions, if `boxiness` is not finite or is below 1.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::potential::sigma::effective_radius_in_scales;
///
/// // A boxier body holds more of its light at large radii.
/// let (round, boxy) = (effective_radius_in_scales(2.0), effective_radius_in_scales(3.5));
/// assert!((round - 2.027).abs() < 1e-3 && (boxy - 2.1845).abs() < 1e-4);
/// ```
#[must_use]
pub fn effective_radius_in_scales(boxiness: f64) -> f64 {
    debug_assert!(
        boxiness.is_finite() && boxiness >= 1.0,
        "a boxiness of at least 1, not {boxiness}"
    );
    // Per node: t, its weight times k^(−3), and k.
    let mut nodes = [[0.0; 3]; 32 * (PROJECTION_PANELS.len() - 1)];
    let points = PROJECTION_PANELS.windows(2).flat_map(|panel| {
        let half = 0.5 * (panel[1] - panel[0]);
        let mid = panel[0] + half;
        GL32_NODES
            .iter()
            .zip(&GL32_WEIGHTS)
            .map(move |(&x, &w)| (mid + half * x, w * half))
    });
    for (node, (t, weight)) in nodes.iter_mut().zip(points) {
        let k = math::powf(1.0 + math::powf(t, boxiness), 1.0 / boxiness);
        *node = [t, weight / (k * k * k), k];
    }
    let whole = nodes
        .iter()
        .fold(0.0, |sum, &[t, weight, _]| sum + weight * (1.0 + t));
    let tail = |y: f64| math::exp(-y) * (y * y + 2.0 * y + 2.0);
    let outside = |x: f64| {
        nodes.iter().fold(0.0, |sum, &[t, weight, k]| {
            sum + weight * (tail(k * x) + t * tail(k * x / t))
        }) / (2.0 * whole)
    };
    bisect(|x| 0.5 - outside(x), 0.5, 5.0, 60)
}

/// The bulge's `σ_e`, which M–σ reads, from the mass model of `params` without the black hole and
/// the nuclear cluster (plan 08, P08.T4.d with ruling 105.1, which replaced plan 02's Design note 8
/// estimate).
///
/// It is [`bulge_projected_sigma`] of [`MassModel::without_centre`]: some 117 evaluations of the
/// model's forces, about a hundred milliseconds. The parameters solve it when their black hole's
/// dispersion or mass is first read, not when they are built.
#[must_use]
pub fn bulge_dispersion(params: &GalaxyParams) -> KilometresPerSecond {
    bulge_projected_sigma(&MassModel::without_centre(params), params)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::fields::bulge::BoxyBulge;
    use crate::galaxy::quad::gl_panels;
    use crate::units::LightYears;

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

    /// At `c = 2` the body is the spherical exponential, whose projected half-mass radius solves
    /// `x² K₂(x) = 1`, since the mass outside the cylinder is `x² K₂(x) ÷ 2`: `x = 2.0270`, which a
    /// Newton step from `K₂(2) = 0.253 760` and `d(x² K₂) ÷ dx = −x² K₁(x)`, `K₁(2) = 0.139 866`,
    /// confirms. The folded integral meets the spherical one, `(1 ÷ 6) ∫ (r² − x²)^(3÷2) e^(−r)
    /// dr`, to 10⁻¹²; until P15.T3.c the latter's root, 2.026 996 389 655 237 4, was a constant.
    #[test]
    fn the_effective_radius_of_an_exponential_sphere() {
        let x = effective_radius_in_scales(2.0);
        assert!((x - 2.027_0).abs() < 2e-4, "{x}");
        assert!((projected_outside(x) - 0.5).abs() < 1e-12);
        assert!((projected_outside(1e-3) - 1.0).abs() < 1e-6);
        let spherical = bisect(|x| 0.5 - projected_outside(x), 0.5, 5.0, 60);
        assert!(
            (x / spherical - 1.0).abs() < 1e-12,
            "{x} against {spherical}"
        );
        assert!((spherical - 2.026_996_389_655_237_4).abs() < 1e-15);
    }

    /// At the boxinesses of plan 02's draw, half the body's mass lies outside the cylinder of the
    /// effective radius, by a direct quadrature of its density in `(R, z)` that knows nothing of
    /// the fold: `2π ∫ R ∫ e^(−m) dz dR` on panels in each, with `m` the bulge field's own
    /// ([`BoxyBulge::radius`] with `a = b`). The radii agree with an independent adaptive
    /// quadrature (`SciPy`'s `quad` and `brentq`, 2026-10-30): 2.150 053, 2.184 531 and 2.209 796.
    #[test]
    fn the_effective_radius_holds_half_the_boxy_body() {
        let edges = [0.0, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0];
        for (c, independent) in [(3.0, 2.150_053), (3.5, 2.184_531), (4.0, 2.209_796)] {
            let x = effective_radius_in_scales(c);
            assert!((x - independent).abs() < 2e-6, "c {c}: {x}");
            let one = LightYears::new(1.0);
            let body = BoxyBulge::new(1.0, one, one, one, c).unwrap();
            let column = |r: f64| 2.0 * gl_panels(|z| math::exp(-body.radius(r, 0.0, z)), &edges);
            let inside = gl_panels(|r| r * column(r), &[0.0, 0.5, 1.0, 1.5, x]);
            let whole = gl_panels(|r| r * column(r), &edges);
            assert!(
                (inside / whole - 0.5).abs() < 1e-9,
                "c {c}: {}",
                inside / whole
            );
        }
    }

    /// The radius is a fixed sequence of operations: the same bits every time.
    #[test]
    fn the_effective_radius_is_deterministic() {
        assert_same_bits(
            effective_radius_in_scales(3.3),
            effective_radius_in_scales(3.3),
        );
    }
}
