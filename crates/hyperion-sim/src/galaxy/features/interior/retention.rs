//! Which remnants a cluster keeps: the kick law's distribution below the cluster's effective
//! escape speed at birth (plan 09, P09.T9.b and P09.T9.c; Design note 8).
//!
//! A remnant stays if its kick is below the escape speed its cluster had when it was born. That
//! speed varies through the cluster, and where a star was born is not known, so retention compares
//! the kick with one **effective** speed per cluster, the central birth escape speed times
//! [`EFFECTIVE_ESCAPE_FACTOR`] (Design note 8). In a Plummer sphere the escape speed at radius `r`
//! is `v₀ (1 + r² ÷ a²)^(−1/4)`, and for kicks well below it the retained share rises as the cube
//! of the escape speed, so the factor is the mass-weighted mean of `(1 + x²)^(−3/4)` to the power
//! one third: `(∫ 3x² (1 + x²)^(−13/4) dx)^(1/3)` = 0.7826 (ours, computed once; it belongs to the
//! generator version).
//!
//! # The quadrature
//!
//! Plan 08's [`speed_bin_shares_against`] gives, for a progenitor of initial mass `m`, the
//! probability of each remnant kind, kick mode and speed bin, bins in units of a reference speed
//! whose first edge is `SPEED_EDGES[0]` = 0.25. With the reference set to four times the effective
//! escape speed, the first bin is exactly "kicked below the escape speed", so one call per mass
//! node gives every kind's retention at once:
//!
//! - **The ordinary mode** and complete fallback (no kick) on the star's own speed: the first bin.
//! - **The low mode** on the pair's systemic speed (the brainstorm, "What is inside a cluster
//!   today"; ruling 126.3): its neutron stars stay bound to their companion, and the pair moves at
//!   the systemic speed of a Be/X-ray binary, a Maxwellian of σ = [`PAIR_SYSTEMIC_SIGMA`] = 12
//!   km/s, whose mean transverse speed is van den Heuvel et al.'s (2000, A&A 364, 563) 15 ± 6 km/s
//!   (σ = 15 ÷ √(π ÷ 2)). The low-mode share is kept with that Maxwellian's distribution function
//!   at the escape speed.
//! - **White dwarfs** take their 1 km/s Maxwellian, which matters only in open clusters.
//!
//! The masses are [`MASS_NODES`] nodes spaced evenly in `ln m` over 8–100 M☉ (plan 06's tracks end
//! at 100 M☉), weighted by the galaxy's mass function and the trapezoid rule in `ln m`; a kind's
//! retention is the weighted retained share over the weighted share of that kind.

use crate::galaxy::displaced::SPEED_EDGES;
use crate::galaxy::displaced::kick_bins::speed_bin_shares_against;
use crate::galaxy::imf::MassFunction;
use crate::math;
use crate::stellar::Composition;
use crate::stellar::remnant::{KickMode, RemnantKind, StandardKickLaw};
use crate::units::{KilometresPerSecond, SolarMasses};

/// The effective escape speed over the central birth escape speed (module documentation): 0.7826.
pub const EFFECTIVE_ESCAPE_FACTOR: f64 = 0.782_568_833_826_336;

/// The Maxwellian σ of a low-mode neutron star's pair's systemic speed, km/s (ruling 126.3; module
/// documentation).
pub const PAIR_SYSTEMIC_SIGMA: f64 = 12.0;

/// The progenitor-mass nodes of the quadrature (module documentation).
pub const MASS_NODES: usize = 16;

/// The least progenitor mass the quadrature covers, M☉: band E's lower edge.
const MASS_LO: f64 = 8.0;

/// The greatest, M☉: plan 06's tracks' top.
const MASS_HI: f64 = 100.0;

/// The retained share of each remnant kind at one effective escape speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retention {
    /// The share of neutron stars retained.
    pub neutron_stars: f64,
    /// The share of black holes retained.
    pub black_holes: f64,
    /// The share of white dwarfs retained, from their 1 km/s Maxwellian.
    pub white_dwarfs: f64,
    /// Neutron stars left per primary star formed, retained or not.
    pub neutron_stars_per_primary: f64,
    /// Black holes left per primary star formed, retained or not.
    pub black_holes_per_primary: f64,
}

/// The distribution function of a Maxwellian speed of scale σ at `v`: `erf(x ÷ √2) − √(2 ÷ π) x
/// e^(−x² ÷ 2)` with `x = v ÷ σ`.
#[must_use]
pub fn maxwell_cdf(v: f64, sigma: f64) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    let x = v / sigma;
    let root_two_over_pi = (2.0 / core::f64::consts::PI).sqrt();
    (math::erf(x * core::f64::consts::FRAC_1_SQRT_2)
        - root_two_over_pi * x * math::exp(-0.5 * x * x))
    .clamp(0.0, 1.0)
}

/// The retention of every kind at effective escape speed `v_eff` for progenitors of `composition`
/// on `mass_function` under `law` (module documentation).
///
/// # Panics
///
/// If `v_eff` is not positive and finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::features::interior::retention::retention;
/// use hyperion_sim::galaxy::imf::MassFunctionKind;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::remnant::StandardKickLaw;
/// use hyperion_sim::units::KilometresPerSecond;
///
/// let imf = MassFunctionKind::default().to_mass_function();
/// let law = StandardKickLaw::default();
/// // A massive globular keeps about a fifth of its neutron stars and more of its black holes.
/// let kept = retention(&law, imf.as_ref(), &Composition::SOLAR, KilometresPerSecond::new(100.0));
/// assert!((0.18..0.26).contains(&kept.neutron_stars));
/// assert!(kept.black_holes > kept.neutron_stars);
/// ```
#[must_use]
pub fn retention(
    law: &StandardKickLaw,
    mass_function: &dyn MassFunction,
    composition: &Composition,
    v_eff: KilometresPerSecond,
) -> Retention {
    let v = v_eff.value();
    assert!(v > 0.0 && v.is_finite(), "an escape speed of {v} km/s");
    let reference = KilometresPerSecond::new(v / SPEED_EDGES[0]);
    let low_kept = maxwell_cdf(v, PAIR_SYSTEMIC_SIGMA);
    let (lo, hi) = (math::ln(MASS_LO), math::ln(MASS_HI));
    #[expect(clippy::cast_precision_loss, reason = "the node count is 16, exact")]
    let step = (hi - lo) / (MASS_NODES - 1) as f64;
    let mut kept = [0.0; 3];
    let mut total = [0.0; 3];
    for k in 0..MASS_NODES {
        #[expect(clippy::cast_precision_loss, reason = "a node index below 16 is exact")]
        let m = math::exp(lo + step * k as f64);
        let end = k == 0 || k == MASS_NODES - 1;
        let weight = mass_function.pdf(m) * m * if end { 0.5 } else { 1.0 };
        let shares = speed_bin_shares_against(law, SolarMasses::new(m), composition, reference);
        for (i, kind) in [
            RemnantKind::NeutronStar,
            RemnantKind::BlackHole,
            RemnantKind::WhiteDwarf,
        ]
        .into_iter()
        .enumerate()
        {
            let mut here = 0.0;
            for mode in [
                KickMode::Ordinary,
                KickMode::FallbackNone,
                KickMode::WhiteDwarf,
            ] {
                here += shares.mode_bins(kind, mode)[0];
            }
            let low: f64 = shares.mode_bins(kind, KickMode::Low).iter().sum();
            here += low * low_kept;
            kept[i] += weight * here;
            total[i] += weight * shares.kind_share(kind);
        }
    }
    let share = |i: usize| {
        if total[i] > 0.0 {
            kept[i] / total[i]
        } else {
            0.0
        }
    };
    // `Σ w ξ m` is `∫ ξ d(ln m)` over the nodes' spacing, so each kind's rate per primary is its
    // weighted share times the spacing over the function's integral.
    let per_primary = step / mass_function.integral(0.0, 150.0);
    Retention {
        neutron_stars: share(0),
        black_holes: share(1),
        white_dwarfs: maxwell_cdf(v, law.params().wd_sigma_km_s),
        neutron_stars_per_primary: total[0] * per_primary,
        black_holes_per_primary: total[1] * per_primary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::imf::MassFunctionKind;

    fn kept(v: f64) -> Retention {
        let imf = MassFunctionKind::default().to_mass_function();
        retention(
            &StandardKickLaw::default(),
            imf.as_ref(),
            &Composition::SOLAR,
            KilometresPerSecond::new(v),
        )
    }

    #[test]
    fn the_effective_factor_is_the_plummer_mean() {
        // A midpoint sum of 3x² (1 + x²)^(−13/4) in x = tan t.
        let steps = 200_000;
        let mut sum = 0.0;
        for i in 0..steps {
            let angle = (f64::from(i) + 0.5) / f64::from(steps) * core::f64::consts::FRAC_PI_2;
            let (sin, cos) = math::sin_cos(angle);
            let x = sin / cos;
            let dx = core::f64::consts::FRAC_PI_2 / f64::from(steps) / (cos * cos);
            sum += 3.0 * x * x * math::powf(1.0 + x * x, -3.25) * dx;
        }
        assert!((math::cbrt(sum) - EFFECTIVE_ESCAPE_FACTOR).abs() < 1e-6);
    }

    #[test]
    fn the_maxwell_cdf_runs_from_zero_to_one() {
        assert!(maxwell_cdf(0.0, 5.0).abs() < 1e-15);
        assert!((maxwell_cdf(100.0, 5.0) - 1.0).abs() < 1e-12);
        // The median of a Maxwellian is 1.5382 σ.
        assert!((maxwell_cdf(1.538_172, 1.0) - 0.5).abs() < 1e-5);
    }

    /// P09.T9.b's windows, ruling 126.3: 18–26% at an effective 100 km/s, 15–25% at 50, 5–17% at
    /// 20, and under 1% for a 10⁴ M☉ open cluster (tested at an effective 4.7 km/s, above its 5.3
    /// × 0.78 ≈ 4.1, so the test is the stricter).
    #[test]
    fn neutron_stars_are_retained_as_the_brainstorm_says() {
        let at = |v: f64| kept(v).neutron_stars;
        let (r100, r50, r20, open) = (at(100.0), at(50.0), at(20.0), at(4.7));
        eprintln!("retention {r100} {r50} {r20} open {open}");
        assert!((0.18..=0.26).contains(&r100), "{r100} at 100 km/s");
        assert!((0.15..=0.25).contains(&r50), "{r50} at 50 km/s");
        assert!((0.05..=0.17).contains(&r20), "{r20} at 20 km/s");
        assert!(open < 0.01, "{open} at 4.7 km/s");
        assert!(r100 >= r50 && r50 >= r20 && r20 >= open);
    }

    #[test]
    fn white_dwarfs_leave_only_the_smallest_clusters() {
        assert!(kept(0.5).white_dwarfs < 0.05);
        assert!(kept(6.0).white_dwarfs > 0.99);
    }
}
