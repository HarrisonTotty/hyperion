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
//!
//! # The table (ruling 139.5)
//!
//! [`retention`] builds sixteen tracks and costs seconds, so a cluster's model reads
//! [`retention_tabulated`] instead, over `tables::cluster_retention`, which `hyperion-fit`'s task
//! `cluster_retention` writes from [`node_shares`]: at each of the quadrature's mass nodes and
//! [`FE_H_NODES`] values of \[Fe/H\] of `z_fit`, the neutron stars' and black holes' shares by
//! mode ([`RetentionShares`]), the ordinary mode's kicked below [`SPEED_NODES`] speeds even in
//! `log v` over 0.1–500 km/s. At run time the shares are linear in \[Fe/H\] and monotone cubic in
//! `log v` (Fritsch and Carlson 1980, SIAM J. Numer. Anal. 17, 238), the low mode's and the
//! envelope loss's Maxwellians are closed forms, and the weights are the galaxy's own mass
//! function, so Chabrier's and Kroupa's are both exact. [`retention`] stays as the reference: the
//! two agree within 0.005 (the fit's acceptance). The table is for the default kick law only. Its
//! run-time form is not the reference's term for term: the complete fallback and the envelope loss
//! are kept whole with their closed-form Maxwellian, where [`retention`] reads their first speed
//! bin; the two are the same law, and an edit to either must keep the acceptance.

use crate::galaxy::displaced::SPEED_EDGES;
use crate::galaxy::displaced::kick_bins::{
    RetentionShares, retention_shares, speed_bin_shares_against,
};
use crate::galaxy::imf::MassFunction;
use crate::math;
use crate::stellar::Composition;
use crate::stellar::composition::{Z_FIT_MAX, Z_FIT_MIN, Z_SOLAR};
use crate::stellar::remnant::{KickMode, RemnantKind, StandardKickLaw};
use crate::tables::cluster_retention;
use crate::units::{Dex, HeliumExcess, KilometresPerSecond, SolarMasses};

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
/// // A massive globular keeps a fifth to a third of its neutron stars and more of its black holes.
/// let kept = retention(&law, imf.as_ref(), &Composition::SOLAR, KilometresPerSecond::new(100.0));
/// assert!((0.18..0.34).contains(&kept.neutron_stars));
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

/// The \[Fe/H\] nodes of the retention table: 11 (ruling 139.5).
pub const FE_H_NODES: usize = 11;

/// The speed nodes of the retention table: 32 (ruling 139.5).
pub const SPEED_NODES: usize = 32;

/// The speeds the table spans, km/s: 0.1–500, even in `log v`.
pub const SPEED_RANGE_KM_S: (f64, f64) = (0.1, 500.0);

/// The quadrature's mass nodes, M☉: [`MASS_NODES`] even in `ln m` over 8–100 M☉.
#[must_use]
pub fn mass_nodes() -> [f64; MASS_NODES] {
    let (lo, hi) = (math::ln(MASS_LO), math::ln(MASS_HI));
    #[expect(clippy::cast_precision_loss, reason = "the node count is 16, exact")]
    let step = (hi - lo) / (MASS_NODES - 1) as f64;
    #[expect(clippy::cast_precision_loss, reason = "a node index below 16 is exact")]
    std::array::from_fn(|k| math::exp(lo + step * k as f64))
}

/// The table's \[Fe/H\] of `z_fit`, `log₁₀(z_fit ÷ 0.02)`, even over Z = 10⁻⁴–0.03, ascending.
///
/// # Panics
///
/// Never: eleven indices fit in `u32`.
#[must_use]
pub fn fe_h_nodes() -> [f64; FE_H_NODES] {
    let lo = math::log10(Z_FIT_MIN.value() / Z_SOLAR.value());
    let hi = math::log10(Z_FIT_MAX.value() / Z_SOLAR.value());
    let last = u32::try_from(FE_H_NODES - 1).expect("11 nodes");
    std::array::from_fn(|k| {
        let k = u32::try_from(k).expect("11 nodes");
        if k == last {
            hi
        } else {
            lo + (hi - lo) * f64::from(k) / f64::from(last)
        }
    })
}

/// The table's speeds, km/s, even in `log v` over [`SPEED_RANGE_KM_S`].
///
/// # Panics
///
/// Never: 32 indices fit in `u32`.
#[must_use]
pub fn speed_nodes() -> [f64; SPEED_NODES] {
    let (lo, hi) = (
        math::log10(SPEED_RANGE_KM_S.0),
        math::log10(SPEED_RANGE_KM_S.1),
    );
    let last = u32::try_from(SPEED_NODES - 1).expect("32 nodes");
    std::array::from_fn(|k| {
        let k = u32::try_from(k).expect("32 nodes");
        math::exp10(lo + (hi - lo) * f64::from(k) / f64::from(last))
    })
}

/// The neutron stars' and black holes' shares of the table's node at mass `m` (M☉) and \[Fe/H\] of
/// `z_fit` `fe_h`, under the default kick law: what `hyperion-fit` tabulates.
#[must_use]
pub fn node_shares(m: f64, fe_h: f64) -> [RetentionShares; 2] {
    let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    let speeds = speed_nodes().map(KilometresPerSecond::new);
    retention_shares(
        &StandardKickLaw::default(),
        SolarMasses::new(m),
        &composition,
        &speeds,
    )
}

/// The table's row of mass node `k`, \[Fe/H\] node `f` and kind `kind` (0 neutron stars, 1 black
/// holes).
#[must_use]
pub const fn table_row(k: usize, f: usize, kind: usize) -> usize {
    (f * MASS_NODES + k) * 2 + kind
}

/// The monotone cubic through `values` at the evenly spaced abscissae `0, 1, …` at `place`,
/// clamped to the ends (Fritsch and Carlson 1980): harmonic-mean slopes, zero at a local extremum.
pub(crate) fn monotone_cubic(values: &[f64; SPEED_NODES], place: f64) -> f64 {
    let last = SPEED_NODES - 1;
    #[expect(clippy::cast_precision_loss, reason = "32 nodes")]
    let place = place.clamp(0.0, last as f64);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the place is clamped to 0..=31"
    )]
    let knot = (place.floor() as usize).min(last - 1);
    #[expect(clippy::cast_precision_loss, reason = "an index below 32")]
    let t = place - knot as f64;
    let secant = |j: usize| values[j + 1] - values[j];
    let slope = |j: usize| -> f64 {
        if j == 0 {
            return secant(0);
        }
        if j == last {
            return secant(last - 1);
        }
        let (left, right) = (secant(j - 1), secant(j));
        if left * right <= 0.0 {
            0.0
        } else {
            2.0 * left * right / (left + right)
        }
    };
    let (y0, y1) = (values[knot], values[knot + 1]);
    let (d0, d1) = (slope(knot), slope(knot + 1));
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * y0
        + (t3 - 2.0 * t2 + t) * d0
        + (-2.0 * t3 + 3.0 * t2) * y1
        + (t3 - t2) * d1
}

/// The retention of every kind at effective escape speed `v_eff` for progenitors of `composition`
/// on `mass_function` under the default kick law, from the table (module documentation; ruling
/// 139.5).
///
/// # Panics
///
/// If `v_eff` is not positive and finite.
#[must_use]
pub fn retention_tabulated(
    mass_function: &dyn MassFunction,
    composition: &Composition,
    v_eff: KilometresPerSecond,
) -> Retention {
    retention_from_rows(
        &cluster_retention::SHARES,
        &cluster_retention::ORDINARY_BELOW,
        mass_function,
        composition,
        v_eff,
    )
}

/// [`retention_tabulated`] over the rows `shares` and `ordinary_below`, laid out as the table's
/// ([`table_row`]): what `hyperion-fit` checks a new table with before it is committed.
///
/// # Panics
///
/// If `v_eff` is not positive and finite, or the rows are fewer than the table's.
#[must_use]
pub fn retention_from_rows(
    shares: &[[f64; 4]],
    ordinary_below: &[[f64; SPEED_NODES]],
    mass_function: &dyn MassFunction,
    composition: &Composition,
    v_eff: KilometresPerSecond,
) -> Retention {
    let rows = 2 * MASS_NODES * FE_H_NODES;
    assert!(
        shares.len() >= rows && ordinary_below.len() >= rows,
        "a retention table of {rows} rows"
    );
    let v = v_eff.value();
    assert!(v > 0.0 && v.is_finite(), "an escape speed of {v} km/s");
    let law = StandardKickLaw::default();
    let low_kept = maxwell_cdf(v, PAIR_SYSTEMIC_SIGMA);
    let envelope_kept = maxwell_cdf(v, law.params().wd_sigma_km_s);
    // Place in [Fe/H] of z_fit and in log v.
    let nodes = fe_h_nodes();
    let x = math::log10(composition.z_fit().value() / Z_SOLAR.value())
        .clamp(nodes[0], nodes[FE_H_NODES - 1]);
    let f = nodes
        .windows(2)
        .position(|w| x <= w[1])
        .unwrap_or(FE_H_NODES - 2);
    let frac = (x - nodes[f]) / (nodes[f + 1] - nodes[f]);
    let (log_lo, log_hi) = (
        math::log10(SPEED_RANGE_KM_S.0),
        math::log10(SPEED_RANGE_KM_S.1),
    );
    #[expect(clippy::cast_precision_loss, reason = "32 nodes")]
    let place = (math::log10(v) - log_lo) / (log_hi - log_lo) * (SPEED_NODES - 1) as f64;
    let masses = mass_nodes();
    let (lo, hi) = (math::ln(MASS_LO), math::ln(MASS_HI));
    #[expect(clippy::cast_precision_loss, reason = "the node count is 16, exact")]
    let step = (hi - lo) / (MASS_NODES - 1) as f64;
    let mut kept = [0.0; 2];
    let mut total = [0.0; 2];
    for (k, &m) in masses.iter().enumerate() {
        let end = k == 0 || k == MASS_NODES - 1;
        let weight = mass_function.pdf(m) * m * if end { 0.5 } else { 1.0 };
        for kind in 0..2 {
            let at = |ff: usize| {
                let row = table_row(k, ff, kind);
                let [share, fallback, low, envelope] = shares[row];
                let ordinary = monotone_cubic(&ordinary_below[row], place).clamp(0.0, share);
                (
                    share,
                    ordinary + fallback + envelope * envelope_kept + low * low_kept,
                )
            };
            let (s0, k0) = at(f);
            let (s1, k1) = at(f + 1);
            kept[kind] += weight * (k0 + frac * (k1 - k0));
            total[kind] += weight * (s0 + frac * (s1 - s0));
        }
    }
    let share = |i: usize| {
        if total[i] > 0.0 {
            (kept[i] / total[i]).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
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

    /// P09.T9.b's windows, ruling 126.3 as ruling 137.3 shifts them to the measured low-mode
    /// share w = 0.2675 (126.3's were stated at w = 0.181, each moved by (w − 0.181)(`P_low(< v)` −
    /// `P_DM25(< v)`)): 26–34% at an effective 100 km/s, 24–34% at 50, 10–22% at 20, and under 1%
    /// for a 10⁴ M☉ open cluster (tested at an effective 4.7 km/s, above its 5.3 × 0.78 ≈ 4.1, so
    /// the test is the stricter).
    #[test]
    fn neutron_stars_are_retained_as_the_brainstorm_says() {
        let at = |v: f64| kept(v).neutron_stars;
        let (r100, r50, r20, open) = (at(100.0), at(50.0), at(20.0), at(4.7));
        eprintln!("retention {r100} {r50} {r20} open {open}");
        assert!((0.26..=0.34).contains(&r100), "{r100} at 100 km/s");
        assert!((0.24..=0.34).contains(&r50), "{r50} at 50 km/s");
        assert!((0.10..=0.22).contains(&r20), "{r20} at 20 km/s");
        assert!(open < 0.01, "{open} at 4.7 km/s");
        assert!(r100 >= r50 && r50 >= r20 && r20 >= open);
    }

    #[test]
    fn white_dwarfs_leave_only_the_smallest_clusters() {
        assert!(kept(0.5).white_dwarfs < 0.05);
        assert!(kept(6.0).white_dwarfs > 0.99);
    }

    /// Ruling 139.5: the table agrees with the quadrature it was made from, within 0.005 where
    /// retention is small and 3% above 0.05, off its nodes (the fit's acceptance covers 64 points).
    #[test]
    fn the_table_agrees_with_the_quadrature_off_its_nodes() {
        let law = StandardKickLaw::default();
        for (kind, fe_h, v) in [
            (MassFunctionKind::Chabrier, -1.1, 45.0),
            (MassFunctionKind::Kroupa, -0.43, 17.0),
        ] {
            let imf = kind.to_mass_function();
            let c = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            let v = KilometresPerSecond::new(v);
            let exact = retention(&law, imf.as_ref(), &c, v);
            let table = retention_tabulated(imf.as_ref(), &c, v);
            for (a, b) in [
                (table.neutron_stars, exact.neutron_stars),
                (table.black_holes, exact.black_holes),
                (
                    table.neutron_stars_per_primary,
                    exact.neutron_stars_per_primary,
                ),
            ] {
                let err = (a - b).abs();
                assert!(
                    if b > 0.05 {
                        err <= 0.03 * b
                    } else {
                        err <= 0.005
                    },
                    "{kind:?} [Fe/H] {fe_h} {v:?}: {a} against {b}"
                );
            }
        }
    }

    /// The tabulated retention's five fields at a few metallicities and speeds for both mass
    /// functions, bit for bit.
    #[test]
    fn tabulated_retention_golden() {
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::GoldenWriter;
        let mut w = GoldenWriter::new();
        w.header(crate::GENERATOR_VERSION.get());
        for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
            let imf = kind.to_mass_function();
            for fe_h in [-2.0, -0.7, 0.1] {
                let c = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
                for v in [2.0, 20.0, 90.0] {
                    let r = retention_tabulated(imf.as_ref(), &c, KilometresPerSecond::new(v));
                    let label = format!("{kind:?} [Fe/H] {fe_h} v {v}");
                    w.f64(&format!("{label} ns"), r.neutron_stars);
                    w.f64(&format!("{label} bh"), r.black_holes);
                    w.f64(&format!("{label} wd"), r.white_dwarfs);
                    w.f64(
                        &format!("{label} ns per primary"),
                        r.neutron_stars_per_primary,
                    );
                    w.f64(
                        &format!("{label} bh per primary"),
                        r.black_holes_per_primary,
                    );
                }
            }
        }
        golden!("galaxy/features/retention", w.as_str());
    }
}
