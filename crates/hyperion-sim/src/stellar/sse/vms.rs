//! Very massive stars, 100–150 M☉ (plan 06, P06.T14): Hurley, Pols and Tout's (2000) formulae
//! evaluated as they stand above 100 M☉, the top of their fit, times three correction factors,
//! for the zero-age main sequence's luminosity and radius and for the main sequence's lifetime.
//!
//! Each factor is a quadratic in x = log₁₀(m ÷ 100 M☉) that is exactly 1 at 100 M☉ and at every
//! mass below, so nothing below 100 M☉ changes and every quantity is continuous in mass there.
//!
//! # The grid
//!
//! The non-rotating Geneva models of Yusof et al. (2013, MNRAS 433, 1114), the one grid that
//! prints the zero-age main sequence's luminosity and temperature (their Table 2) and the core
//! hydrogen-burning lifetime `τ_H` (Table 3) of 120 and 150 M☉ models at two metallicities, Z = 0.014
//! and 0.006. The grid has no 100 M☉ model, so the quadratics' two free coefficients each are
//! fitted by least squares to the grid over the formulae at the four (m, Z) points, with the
//! formulae read at the grid's Z. Radii are the grid's, from L and `T_eff` by Stefan–Boltzmann.
//!
//! | Quantity | Grid ÷ formulae at 120, 150 M☉ (Z = 0.014; 0.006) | Fit at 120, 150 M☉ | Worst residual |
//! | --- | --- | --- | --- |
//! | `L_ZAMS` | 0.9993, 0.9838; 0.9986, 0.9808 | 0.9989, 0.9823 | 0.15% |
//! | `R_ZAMS` | 0.8426, 0.8391; 0.9175, 0.9334 | 0.8800, 0.8862 | 5.6% |
//! | `t_MS` | 0.8148, 0.7865; 0.8033, 0.7758 | 0.8271, 0.7689 | 3.0% |
//!
//! The radius's residuals are the grid's own spread in Z (the formulae's radius falls with Z more
//! steeply than the grid's), which one factor per quantity cannot follow. The lifetime's fit is
//! held so that the lifetime at the formulae's own slope still falls with mass up to 150 M☉ (the
//! unconstrained least-squares quadratic dips below its end value, 0.765 at 138 M☉, and would make
//! a 150 M☉ star outlive a 140 M☉ one); its residuals rise from 0.7% to 3.0% for it.
//!
//! **A finding, not tuned:** the grid differs from the formulae by 8–16% in radius and 19–22% in
//! lifetime already at 120 M☉, so the offset is most likely there at 100 M☉ too, where the factors
//! must be 1. The quadratics therefore do nearly all their correcting between 100 and 120 M☉, with
//! slopes of −2.2 (radius) and −2.9 (lifetime) per dex at 100 M☉. Yusof et al.'s `τ_H` comes from
//! models with strong mass loss (their 120 M☉ model at Z = 0.014 leaves hydrogen burning at
//! 63.7 M☉), so the lifetime factor folds their winds into the constant-mass lifetime the
//! formulae give; the tracks integrate their own winds on top.
//!
//! The Eddington factor `Γₑ` = `κₑ` L ÷ (4π c G M), electron scattering alone, read at X = 0 (the
//! least opacity a photosphere can have; ruling 124.1), stays below 0.75 on the main sequence at
//! every metallicity (at most 0.65, under Sanyal et al.'s 2015 0.7; Yusof et al.'s end-of-hydrogen
//! Γ is 0.63–0.72). At the initial X it reaches 1.14 late on the main sequence at 150 M☉ and
//! Z = 10⁻⁴: a helium-enriched photosphere, which the tracks do not model. After the main sequence
//! HPT's formulae pass `Γₑ`(X = 0) = 1 (a finding, pinned in the tests), which P06.T39's wind is to
//! remove.

use crate::math;

/// The mass above which the corrections apply, M☉: the top of Hurley, Pols and Tout's fit.
pub(crate) const CORRECTED_ABOVE: f64 = 100.0;

/// (a, b) of the zero-age main sequence's luminosity factor 1 + a x + b x² (see the module).
const LUMINOSITY: [f64; 2] = [0.058_296_739_087_955_43, -0.901_415_188_397_333_8];

/// (a, b) of the zero-age main sequence's radius factor 1 + a x + b x² (see the module).
const RADIUS: [f64; 2] = [-2.225_272_194_981_249_3, 8.968_609_752_995_615];

/// (a, b) of the main-sequence lifetime's factor 1 + a x + b x² (see the module).
const LIFETIME: [f64; 2] = [-2.895_9, 8.991_483_263_011_18];

/// 1 + a x + b x² at x = log₁₀(m ÷ 100) for m above 100 M☉, and exactly 1 at or below it.
#[must_use]
fn factor([a, b]: [f64; 2], m: f64) -> f64 {
    if m > CORRECTED_ABOVE {
        let x = math::log10(m / CORRECTED_ABOVE);
        1.0 + x * (a + b * x)
    } else {
        1.0
    }
}

/// The factor on the zero-age main sequence's luminosity of a star of `m` M☉ (Tout et al.'s fit,
/// `zams::luminosity`): 1 up to 100 M☉, 0.982 at 150.
#[must_use]
pub(crate) fn luminosity_factor(m: f64) -> f64 {
    factor(LUMINOSITY, m)
}

/// The factor on the zero-age main sequence's radius of a star of `m` M☉ (`zams::radius`): 1 up to
/// 100 M☉, 0.886 at 150.
#[must_use]
pub(crate) fn radius_factor(m: f64) -> f64 {
    factor(RADIUS, m)
}

/// The factor on the main sequence's timescales of a star of `m` M☉ (HPT equation 4's `t_BGB`, and
/// with it `t_hook`, `t_MS` and the Hertzsprung gap, which are fractions of it): 1 up to 100 M☉,
/// 0.769 at 150.
#[must_use]
pub(crate) fn lifetime_factor(m: f64) -> f64 {
    factor(LIFETIME, m)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid's ratios to the formulae at 120 and 150 M☉ (the module's table), which each fit
    /// must meet within its stated residual.
    #[test]
    fn the_factors_meet_the_grid_within_their_residuals() {
        for (f, ratios, worst) in [
            (
                luminosity_factor as fn(f64) -> f64,
                [(120.0, 0.9993, 0.9986), (150.0, 0.9838, 0.9808)],
                0.002,
            ),
            (
                radius_factor,
                [(120.0, 0.8426, 0.9175), (150.0, 0.8391, 0.9334)],
                0.06,
            ),
            (
                lifetime_factor,
                [(120.0, 0.8148, 0.8033), (150.0, 0.7865, 0.7758)],
                0.031,
            ),
        ] {
            for (m, solar, poor) in ratios {
                for ratio in [solar, poor] {
                    let off = (f(m) / ratio - 1.0).abs();
                    assert!(off < worst, "{m} M☉: {} against {ratio}", f(m));
                }
            }
        }
    }

    /// Each factor is 1 up to 100 M☉ and continuous there to 10⁻¹².
    #[test]
    fn the_factors_are_one_up_to_a_hundred_solar_masses_and_continuous_there() {
        for f in [luminosity_factor, radius_factor, lifetime_factor] {
            for m in [0.1, 1.0, 50.0, 99.999_999, 100.0] {
                assert!((f(m) - 1.0).abs() < 1e-15, "{m}: {}", f(m));
            }
            // The next double above 100.
            assert!((f(100.0_f64.next_up()) - 1.0).abs() < 1e-12);
        }
    }
}
