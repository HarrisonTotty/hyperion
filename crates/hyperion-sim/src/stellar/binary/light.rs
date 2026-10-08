//! The census's bound on a pair's light (plan 11, P11.T17; rendering plan R06's ask B,
//! `decision-r06-census-cost.md` §7): before a system is generated, whether a pair of two stars
//! can hold a star bright enough to list, from fitted tables alone.
//!
//! The interface is fixed here, in P11.T17.a, and is unchanged by T17.b and T17.c (decided
//! 2026-10-07, `decision-p11-t16-hierarchy-bound.md` §5). [`pair_light_bound`] takes the two
//! stars' initial masses as P11.T16's `hierarchy_bound` lists them, bit for bit, and the pair's
//! drawn periastron, and answers `None` where the pair cannot be bounded, so that R06.T8.g
//! generates its record, or one of four verdicts:
//!
//! - [`PairLight::Detached`]: the pair cannot have interacted by the window's end, by the
//!   closed-form, conservative test below. Each star is its own single-star model.
//! - [`PairLight::Unchanged`]: each star of the pair living in the window is its own single-star
//!   model, and no product of the pair lives in it. The census treats it as `Detached`.
//! - [`PairLight::Remnants`]: no star or product of the pair lives in the window.
//! - [`PairLight::Bright`]: a living star may depart from its own single-star model, or a product
//!   may live. Each such star and product is no brighter than the magnitude, margin included; the
//!   census bounds each living star of the pair by the brighter of its own bound and it.
//!
//! T17.a returns `Detached` or `None`. T17.c adds `Unchanged`, `Remnants` and `Bright` from
//! T17.b's tables.
//!
//! # `Detached`
//!
//! A pair is two single stars wherever [`can_interact`](super::can_interact) passes it over
//! (design note 7): the engine runs no step of it before that age, and a run to a later age shows
//! no interaction before it (P11.T4.j's build-age contract), the stars accreting no wind while on
//! their own tracks. The bound answers `Detached` only where that test fails for **every** orbit
//! of periastron at least `periastron_min`, every eccentricity, every η and every mass of the
//! intervals given, with each star's largest radius R̂ᵢ from the reach table
//! ([`largest_radius_bound`](super::largest_radius_bound)) at the window's end, which bounds the
//! radius the engine reads. Writing `r_p` for the periastron, a pair is `Detached` when:
//!
//! 1. **The lobe test fails** (Eggleton 1983): R̂ᵢ < `f_L`(qᵢ,min) `periastron_min` for both stars,
//!    with qᵢ,min the least ratio mᵢ ÷ mⱼ of the intervals. The engine's test reaches at `r_p` if
//!    Rᵢ ≥ `f_L`(qᵢ) `r_p`, so it reaches at no `r_p` ≥ `periastron_min`.
//! 2. **P11.T4.j's coarse decay test fails** at `periastron_min`, every term at its largest over
//!    the eccentricity e. The engine's test reads the semi-latus rectum p₀ = `r_p` (1 + e) and the
//!    critical `p_c` = `r_c` (1 + e), `r_c` = maxᵢ Rᵢ ÷ `f_L`(qᵢ); divided by (1 + e)⁵ throughout:
//!    - the spins' reservoir takes a share Φ I ÷ (μ (1 + e)² `r_c`^{3/2} √`r_p`) of the orbit's
//!      angular momentum, with Φ = f₂(e) ÷ f₅(e) (Hut 1981; BSE equation 34) and I = Σ kᵢ mᵢ Rᵢ²
//!      (BSE equation 35's coarse bound). Since Φ ÷ (1 + e)² ≤ 1 and `r_c` ≥ Rᵢ ÷ `f_L`(qᵢ),
//!      the share is at most c ÷ √`r_p`, c = Σᵢ kᵢ √R̂ᵢ `f_L`(qᵢ,max)^{3/2} (1 + qᵢ,max),
//!      whichever star sets `r_c`, with kᵢ the core's k′₃ = 0.21 where the table says a star may
//!      have left its main sequence and the envelope's k′₂ = 0.1 elsewhere;
//!    - magnetic braking (BSE equation 50) at the equilibrium spin carries Φ³, and Φ³ ÷ (1 + e)⁵
//!      is at most [`BRAKING_ECCENTRICITY_FACTOR`], its value as e → 1; its integral is at most
//!      Σ R̂ᵢ³ over the whole age, of each star above BSE's 0.35 M☉ floor, with M ÷ μ at its
//!      largest over the intervals;
//!    - gravitational radiation (BSE equation 48) carries (1 + 7e²/8)(1 + e), so its term
//!      divided by (1 + e)⁵ is at most its value at e = 0, 10 β′ m₁ m₂ M `r_p` over the age.
//!
//!    The engine's test reaches if (`r_p` (1 − x)²)⁵ − 1.25 (B + W) ≤ `r_c`⁵, x the reservoir's
//!    share. With the bounds above that is at most (√`r_p` − c)¹⁰ − 1.25 (B̂ + Ŵ `r_p`) ≤
//!    `r̂_c`⁵, whose left side grows with `r_p` wherever its derivative at `periastron_min` is
//!    non-negative (that derivative itself grows with `r_p`), which the bound checks. So a pair that fails it at
//!    `periastron_min` fails it at every `r_p` beyond. A pair whose reservoir could take the whole
//!    orbit (c ≥ √`periastron_min`) is not `Detached`.
//!
//! 3. **No star can have died suddenly by the window's end** (a core's collapse, an electron
//!    capture or a pair instability, the reach table's earliest collapse of each star's cell). A
//!    kick can leave an eccentric orbit whose periastron the drawn one does not bound, and the
//!    engine runs such a pair from zero age when it can interact by the end of the clock window
//!    or holds a remnant by then (`run_pairs`): a later transfer onto the remnant would erode the
//!    companion before the window's end. The pre-test's build-age contract, and P11.T4.j's
//!    zero-miss gate, cover interactions before the pair's first supernova only. So such a pair is
//!    `None` (`decision-p11-t16-hierarchy-bound.md` §5). A white dwarf's formation never brings
//!    the periastron in: under isotropic, kick-free mass loss at any rate the periastron does not
//!    decrease (Veras et al. 2011, MNRAS 417, 2104, equation 21), the engine models no white
//!    dwarf's kick, and its stars accrete no wind on their own tracks. So the test at the drawn
//!    periastron, with both stars' largest radii, covers it. (Real white dwarfs may take kicks of
//!    about 1 km s⁻¹, El-Badry and Rix 2018, MNRAS 480, 4884, which this bound, like the engine,
//!    leaves out.)
//!
//! Every term is monotone in the R̂ᵢ, the masses and the age, so later ages and smaller periastra
//! only take pairs out of `Detached`. The engine's own span starts at the first arrival, after
//! zero age, so the whole age bounds it. The test is the engine's at version 21, before P11.T4.l's
//! wind-spin term and its passed-over collapses: its slow test runs again when they land.

use std::ops::RangeInclusive;

use crate::orbit::roche_lobe_radius;
use crate::stellar::Composition;
use crate::stellar::sse::{CORE_GYRATION, ENVELOPE_GYRATION};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{Magnitudes, Metres, SolarMasses, Years};

use super::detached::{
    DECAY_SAFETY, GRAVITATIONAL_WAVE_RATE, MAGNETIC_BRAKING, MAGNETIC_BRAKING_FLOOR,
};
use super::reach::{ReachBound, ReachTable, fe_h_of, has_helium_excess};
use super::star::{G, positive};

/// What a pair of two stars can hold in a window of ages, as the census reads it (plan 11,
/// P11.T17; `decision-p11-t16-hierarchy-bound.md` §5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PairLight {
    /// The pair cannot have interacted by the window's end: each star is its own single-star
    /// model, and takes its own bound.
    Detached,
    /// Each star of the pair living in the window is its own single-star model, and no product of
    /// the pair lives in it: the census treats it as `Detached` (P11.T17.c; not yet answered).
    Unchanged,
    /// No star or product of the pair lives in the window (P11.T17.c; not yet answered).
    Remnants,
    /// A living star may depart from its own single-star model, or a product may live: each such
    /// star and product is no brighter than this absolute V magnitude at any age in the window,
    /// margin included. A star that does not depart is its own model; the census bounds each
    /// living star of the pair by the brighter of its own bound and this (P11.T17.c; not yet
    /// answered).
    Bright(Magnitudes),
}

/// The share of magnetic braking's term that eccentricity can add, at most, against a circular
/// orbit of the same periastron: Φ³ ÷ (1 + e)⁵, with Φ = f₂(e) ÷ f₅(e) (Hut 1981, A&A 99, 126),
/// has its supremum on [0, 1) as e → 1, where Φ = 14.4375 ÷ 4.375 = 3.3, so 3.3³ ÷ 32 =
/// 1.123 031 25, written here rounded up.
pub(crate) const BRAKING_ECCENTRICITY_FACTOR: f64 = 1.123_032;

/// The bound on the pair of stars of initial masses `a` and `b`, of drawn periastron
/// `periastron_min`, of `composition`, over the window of their `ages` (plan 11, P11.T17): `None`
/// where the pair cannot be bounded, so that the census generates its record.
///
/// The masses are the two stars' initial masses as P11.T16's `hierarchy_bound` lists them, bit for
/// bit, in either order, which gives the same answer bit for bit; the periastron is the pair's
/// drawn one, and any orbit of a periastron at least it is bounded too. The ages are the record's
/// light-time ages. Until P11.T17.c, a pair that is not [`PairLight::Detached`] is `None`, and so
/// is one with a star that may have died suddenly by the window's end (the module's item 3).
/// `None` also answers a mass below 0.08 M☉ or not finite, a periastron that is not a positive
/// number, a window that ends past 10^10.2 years, starts after it ends or is not a number, and a
/// composition with a helium excess. The bound reads fitted tables only, and no stream.
///
/// # Examples
///
/// The census asks whether a solar-mass star and its 0.6 M☉ companion, 9 Gyr old, can still be two
/// single stars: 2,000 au apart at periastron they can; 0.04 au apart they may already have
/// interacted, so the bound cannot say.
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::binary::{PairLight, pair_light_bound};
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{Metres, SolarMasses, Years};
///
/// let (sun, dwarf) = (SolarMasses::new(1.0), SolarMasses::new(0.6));
/// let au = METRES_PER_AU;
/// let ages = Years::new(9.0e9)..=Years::new(9.0e9);
/// let wide = pair_light_bound(sun, dwarf, Metres::new(2_000.0 * au), &Composition::SOLAR,
///     ages.clone());
/// assert_eq!(wide, Some(PairLight::Detached));
/// let close = pair_light_bound(sun, dwarf, Metres::new(0.04 * au), &Composition::SOLAR, ages);
/// assert_eq!(close, None);
/// ```
#[must_use]
pub fn pair_light_bound(
    a: SolarMasses,
    b: SolarMasses,
    periastron_min: Metres,
    composition: &Composition,
    ages: RangeInclusive<Years>,
) -> Option<PairLight> {
    pair_light_bound_over([a..=a, b..=b], periastron_min, composition, ages)
}

/// [`pair_light_bound`] over every pair whose initial masses lie in `masses`, each interval its
/// ends included: what it answers holds for each such pair. An interval whose start lies above
/// its end is `None`. The census passes exact masses (`decision-p11-t16-hierarchy-bound.md`
/// rejected an interval bound); the intervals let the tests check that the bound only shrinks as
/// they widen.
#[must_use]
pub(crate) fn pair_light_bound_over(
    masses: [RangeInclusive<SolarMasses>; 2],
    periastron_min: Metres,
    composition: &Composition,
    ages: RangeInclusive<Years>,
) -> Option<PairLight> {
    let (start, end) = (ages.start().value(), ages.end().value());
    if start.is_nan() || end.is_nan() || start > end || has_helium_excess(composition) {
        return None;
    }
    let periastron_rsun = periastron_min.value() / SOLAR_RADIUS_M;
    if !positive(periastron_rsun) {
        return None;
    }
    let mut ends = masses.map(|m| [m.start().value(), m.end().value()]);
    if ends
        .iter()
        .any(|&[lo, hi]| !(lo > 0.0 && hi >= lo && hi.is_finite()))
    {
        return None;
    }
    // A fixed order, so that the answer is the same bit for bit whichever star is given first.
    ends.sort_by(|x, y| x[0].total_cmp(&y[0]).then(x[1].total_cmp(&y[1])));
    let table = ReachTable::generator();
    let fe_h = fe_h_of(composition);
    let until = end.max(0.0);
    let reach = [
        table.bound(ends[0][0], ends[0][1], fe_h, until)?,
        table.bound(ends[1][0], ends[1][1], fe_h, until)?,
    ];
    // 3. A star that may have exploded by the window's end may have kicked the orbit in.
    if reach.iter().any(ReachBound::may_have_collapsed) {
        return None;
    }
    passed_over(ends, reach, periastron_rsun, until).then_some(PairLight::Detached)
}

/// Whether every pair of initial masses in `ends` (M☉, each `[lo, hi]`), of largest radii bounded
/// by `reach` at `until_years`, on any orbit of periastron at least `periastron_rsun` (R☉), fails
/// `can_interact`'s lobe and coarse decay tests by `until_years` (the module's items 1 and 2).
#[must_use]
fn passed_over(
    ends: [[f64; 2]; 2],
    reach: [ReachBound; 2],
    periastron_rsun: f64,
    until_years: f64,
) -> bool {
    let p = periastron_rsun;
    let radius = reach.map(|r| r.radius_rsun());
    // The least and greatest ratio mᵢ ÷ mⱼ, and Eggleton's lobe fraction at each: it grows with q.
    let q_min = [ends[0][0] / ends[1][1], ends[1][0] / ends[0][1]];
    let q_max = [ends[0][1] / ends[1][0], ends[1][1] / ends[0][0]];
    if q_min
        .iter()
        .chain(&q_max)
        .any(|&q| !(positive(q) && q.is_finite()))
    {
        return false;
    }
    let f_min = q_min.map(lobe_fraction);
    let f_max = q_max.map(lobe_fraction);
    // 1. The lobe test at the least periastron.
    if (0..2).any(|i| radius[i] >= f_min[i] * p) {
        return false;
    }
    let r_c = (0..2).map(|i| radius[i] / f_min[i]).fold(0.0, f64::max);
    // 2. The reservoir's share is at most c ÷ √r_p.
    let c = (0..2).fold(0.0, |sum, i| {
        let k = if reach[i].may_hold_core() {
            CORE_GYRATION
        } else {
            ENVELOPE_GYRATION
        };
        sum + k * radius[i].sqrt() * f_max[i] * f_max[i].sqrt() * (1.0 + q_max[i])
    });
    let s = p.sqrt() - c;
    if !positive(s) {
        return false;
    }
    // Braking: M ÷ μ = 2 + q + 1/q is largest at an end of the range of q = m₁ ÷ m₂.
    let m_over_mu = [q_min[0], q_max[0]]
        .iter()
        .map(|&q| 2.0 + q + 1.0 / q)
        .fold(0.0, f64::max);
    let braking_rate = 10.0 * MAGNETIC_BRAKING * G * m_over_mu * BRAKING_ECCENTRICITY_FACTOR;
    let braking = (0..2)
        .filter(|&i| ends[i][1] > MAGNETIC_BRAKING_FLOOR)
        .fold(0.0, |sum, i| sum + radius[i] * radius[i] * radius[i])
        * until_years;
    // Gravitational radiation: its term is `radiation` × r_p.
    let [m1, m2] = [ends[0][1], ends[1][1]];
    let radiation = 10.0 * GRAVITATIONAL_WAVE_RATE * m1 * m2 * (m1 + m2) * until_years;
    // The left side grows with r_p from here on only if its derivative is non-negative here.
    let s9 = crate::math::powi(s, 9);
    if 5.0 * s9 < DECAY_SAFETY * radiation * p.sqrt() {
        return false;
    }
    s9 * s - DECAY_SAFETY * (braking_rate * braking + radiation * p) > crate::math::powi(r_c, 5)
}

/// Eggleton's (1983) Roche-lobe radius over the separation for a star of mass ratio `q` = its
/// mass ÷ its companion's, as the engine reads it.
#[must_use]
fn lobe_fraction(q: f64) -> f64 {
    roche_lobe_radius(q, Metres::new(1.0)).value()
}

#[cfg(test)]
mod tests {
    use super::super::detached::{hut_f2, hut_f5};
    use super::*;

    use crate::units::consts::METRES_PER_AU as AU_M;

    fn solar(a: f64, b: f64, periastron_au: f64, age: f64) -> Option<PairLight> {
        pair_light_bound(
            SolarMasses::new(a),
            SolarMasses::new(b),
            Metres::new(periastron_au * AU_M),
            &Composition::SOLAR,
            Years::new(age)..=Years::new(age),
        )
    }

    /// Φ³ ÷ (1 + e)⁵ never exceeds the factor, Φ ÷ (1 + e)² never exceeds 1, and
    /// (1 + 7e²/8) ÷ (1 + e)⁴ never exceeds 1, on a fine grid of e over [0, 1].
    #[test]
    fn the_eccentricity_factors_are_their_suprema() {
        let mut largest_braking: f64 = 0.0;
        for n in 0..=100_000_u32 {
            let e = f64::from(n) / 100_000.0;
            let e2 = e * e;
            let phi = hut_f2(e2) / hut_f5(e2);
            let one_e = 1.0 + e;
            let braking = phi * phi * phi / crate::math::powi(one_e, 5);
            largest_braking = largest_braking.max(braking);
            assert!(phi / (one_e * one_e) <= 1.0 + 1e-15, "e {e}");
            assert!(
                (1.0 + 0.875 * e2) / crate::math::powi(one_e, 4) <= 1.0 + 1e-15,
                "e {e}"
            );
        }
        assert!(
            largest_braking <= BRAKING_ECCENTRICITY_FACTOR,
            "{largest_braking}"
        );
        assert!(BRAKING_ECCENTRICITY_FACTOR - largest_braking < 1e-5);
    }

    #[test]
    fn a_wide_pair_is_detached_and_a_close_one_is_not() {
        assert_eq!(solar(1.0, 0.6, 2_000.0, 9.0e9), Some(PairLight::Detached));
        assert_eq!(solar(1.0, 0.6, 0.04, 9.0e9), None);
        // Young, a 1 au periastron is far outside two main-sequence stars' lobes; at 14 Gyr the
        // solar-mass star's giant branch reaches it.
        assert_eq!(solar(1.0, 0.6, 1.0, 1.0e9), Some(PairLight::Detached));
        assert_eq!(solar(1.0, 0.6, 1.0, 1.4e10), None);
        // The order of the masses does not matter.
        for (p, age) in [(5.0, 1.0e9), (0.3, 3.0e9), (40.0, 1.3e10)] {
            assert_eq!(solar(1.0, 0.6, p, age), solar(0.6, 1.0, p, age));
        }
    }

    /// A wider orbit, an earlier age and narrower mass intervals only bring a pair into
    /// `Detached`.
    #[test]
    fn detached_grows_with_the_periastron_and_shrinks_with_age_and_width() {
        let periastra: Vec<f64> = (0..60)
            .map(|k| 0.005 * crate::math::powi(1.25, k))
            .collect();
        let ages: Vec<f64> = (0..40).map(|k| 1.0e6 * crate::math::powi(1.3, k)).collect();
        for &(a, b) in &[
            (1.0, 0.6),
            (3.0, 2.5),
            (12.0, 4.0),
            (0.4, 0.1),
            (60.0, 55.0),
        ] {
            for &age in ages.iter().filter(|&&t| t < 1.5e10) {
                let detached: Vec<bool> = periastra
                    .iter()
                    .map(|&p| solar(a, b, p, age) == Some(PairLight::Detached))
                    .collect();
                let first = detached.iter().position(|&d| d).unwrap_or(detached.len());
                assert!(
                    detached[first..].iter().all(|&d| d),
                    "{a} + {b} at {age} yr: {detached:?}"
                );
            }
            for &p in &periastra {
                let detached: Vec<bool> = ages
                    .iter()
                    .filter(|&&t| t < 1.5e10)
                    .map(|&t| solar(a, b, p, t) == Some(PairLight::Detached))
                    .collect();
                let last = detached.iter().rposition(|&d| d).map_or(0, |k| k + 1);
                assert!(detached[..last].iter().all(|&d| d), "{a} + {b} at {p} au");
            }
        }
        // An interval holding a pair that is not `Detached` is not `Detached`.
        let at = |lo: f64, hi: f64| {
            pair_light_bound_over(
                [
                    SolarMasses::new(lo)..=SolarMasses::new(hi),
                    SolarMasses::new(0.6)..=SolarMasses::new(0.6),
                ],
                Metres::new(AU_M),
                &Composition::SOLAR,
                Years::new(6.0e9)..=Years::new(6.0e9),
            )
        };
        assert_eq!(at(0.9, 0.9), Some(PairLight::Detached));
        assert_eq!(solar(1.3, 0.6, 1.0, 6.0e9), None);
        assert_eq!(at(0.9, 1.3), None);
    }

    /// The reach table's reader and the bound are pinned bit for bit (the determinism audit): the
    /// bound's radius and flags at every 61st mass node, each node of `FE_H_NODES` and every 13th
    /// age edge, edges included, and the verdicts of 256 fixed queries.
    #[test]
    fn the_reach_bound_and_the_verdicts_are_pinned() {
        use super::super::reach::{
            AGE_BINS, FE_H_NODES, MASS_CELLS, MASS_SUBDIVISIONS, age_edge_years, mass_node_msun,
        };
        use crate::stellar::binary::largest_radius_bound;
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::GoldenWriter;

        let mut w = GoldenWriter::new();
        w.header(crate::GENERATOR_VERSION.get());
        for n in (0..=MASS_CELLS * MASS_SUBDIVISIONS).step_by(61) {
            let m = SolarMasses::new(mass_node_msun(n));
            for &fe_h in &FE_H_NODES {
                let comp = Composition::from_fe_h(
                    crate::units::Dex::new(fe_h),
                    crate::units::HeliumExcess::ZERO,
                );
                for k in (0..AGE_BINS).step_by(13) {
                    let age = age_edge_years(k);
                    let label = format!("mass node {n} [Fe/H] {fe_h:.3} age bin {k}");
                    match largest_radius_bound(m..=m, &comp, Years::new(age)) {
                        Some(bound) => {
                            w.f64(&label, bound.radius_rsun());
                            w.line(&format!(
                                "  core {} collapsed {}",
                                bound.may_hold_core(),
                                bound.may_have_collapsed()
                            ));
                        }
                        None => w.line(&format!("{label}: none")),
                    }
                }
            }
        }
        let mut mix = Mix(SEED ^ 0x0001_9014);
        let log_uniform =
            |u: f64, lo: f64, hi: f64| lo * crate::math::exp(u * crate::math::ln(hi / lo));
        for i in 0..256_u32 {
            let a = log_uniform(mix.unit(), 0.08, 150.0);
            let b = log_uniform(mix.unit(), 0.08, a);
            let p = log_uniform(mix.unit(), 1.0, 1.0e5);
            let age = log_uniform(mix.unit(), 1.0e5, 1.4e10);
            let fe_h = -2.5 + 2.8 * mix.unit();
            let comp = Composition::from_fe_h(
                crate::units::Dex::new(fe_h),
                crate::units::HeliumExcess::ZERO,
            );
            let verdict = pair_light_bound(
                SolarMasses::new(a),
                SolarMasses::new(b),
                Metres::new(p * SOLAR_RADIUS_M),
                &comp,
                Years::new(age)..=Years::new(age),
            );
            w.line(&format!(
                "query {i:03}: {a:.6} + {b:.6} M☉, periastron {p:.6e} R☉, age {age:.6e} yr, \
                 [Fe/H] {fe_h:.4}: {verdict:?}"
            ));
        }
        golden!("stellar/pair_light", w.as_str());
    }

    /// The bound is the same bit for bit whichever star is given first, over 10⁴ random queries.
    #[test]
    fn the_order_of_the_stars_does_not_matter() {
        let mut mix = Mix(SEED ^ 0x05ed);
        let log_uniform =
            |u: f64, lo: f64, hi: f64| lo * crate::math::exp(u * crate::math::ln(hi / lo));
        let mut detached = 0_u32;
        for _ in 0..10_000 {
            let a = SolarMasses::new(log_uniform(mix.unit(), 0.08, 150.0));
            let b = SolarMasses::new(log_uniform(mix.unit(), 0.08, 150.0));
            let p = Metres::new(log_uniform(mix.unit(), 1.0, 1.0e5) * SOLAR_RADIUS_M);
            let age = Years::new(log_uniform(mix.unit(), 1.0e5, 1.4e10));
            let comp = Composition::from_fe_h(
                crate::units::Dex::new(-2.5 + 2.8 * mix.unit()),
                crate::units::HeliumExcess::ZERO,
            );
            let one_way = pair_light_bound(a, b, p, &comp, age..=age);
            assert_eq!(one_way, pair_light_bound(b, a, p, &comp, age..=age));
            detached += u32::from(one_way == Some(PairLight::Detached));
        }
        assert!(detached > 1_000, "the sample reaches Detached: {detached}");
    }

    /// A pair one of whose stars may have died suddenly by the window's end is not bounded, since
    /// a kick can bring its periastron in; a white dwarf's birth, which only widens the orbit, is
    /// no bar.
    #[test]
    fn a_pair_whose_star_may_have_exploded_is_not_bounded() {
        assert_eq!(solar(20.0, 1.0, 1.0e4, 1.0e6), Some(PairLight::Detached));
        assert_eq!(solar(20.0, 1.0, 1.0e4, 5.0e7), None);
        assert_eq!(solar(1.0, 20.0, 1.0e4, 5.0e7), None);
        // The Sun's white dwarf by 13 Gyr, 10⁴ au from a red dwarf.
        assert_eq!(solar(1.0, 0.3, 1.0e4, 1.3e10), Some(PairLight::Detached));
    }

    /// At version 21 a pair the pre-test passes over keeps its drawn orbit through its stars'
    /// collapses, with no kick and no supernova record (P11.T4.l's finding F3, which the
    /// version-22 batch fixes), so its stars are their own models across a supernova. When T4.l
    /// lands this fails: then a collapse can hand the pair to the engine with its kick, and
    /// P11.T17's collapse rule (the module's item 3) must still hold, or `Detached` must bound
    /// the periastron after the kick (plan 11, P11.T17's 22-batch note).
    #[test]
    fn a_passed_over_pairs_collapse_keeps_its_drawn_orbit_at_version_21() {
        use crate::stellar::binary::evolve;
        let input = super::super::tests::pair(15.0, 3.0, 1.0e5, 0.0, 0.02);
        let until = Years::new(1.0e8);
        assert!(!can_interact(&input, until));
        let timeline = evolve(&input, until);
        assert_eq!(timeline.segments().len(), 1);
        assert!(timeline.supernovae().is_empty());
        assert_eq!(timeline.state_at(until).orbit(), Some(input.orbit()));
        // The bound does not lean on it: the primary may have collapsed, so the pair is `None`.
        let [m1, m2] = input.masses();
        let p = input.orbit().periapsis();
        assert_eq!(
            pair_light_bound(m1, m2, p, &Composition::SOLAR, until..=until),
            None
        );
    }

    #[test]
    fn input_outside_the_bound_is_not_bounded() {
        let ages = || Years::new(1.0e9)..=Years::new(1.0e9);
        let one = SolarMasses::new(1.0);
        let wide = Metres::new(1.0e4 * AU_M);
        let solar = &Composition::SOLAR;
        assert_eq!(
            pair_light_bound(SolarMasses::new(0.05), one, wide, solar, ages()),
            None
        );
        assert_eq!(
            pair_light_bound(SolarMasses::new(f64::NAN), one, wide, solar, ages()),
            None
        );
        assert_eq!(
            pair_light_bound(one, one, Metres::new(0.0), solar, ages()),
            None
        );
        assert_eq!(
            pair_light_bound(one, one, Metres::new(f64::NAN), solar, ages()),
            None
        );
        assert_eq!(
            pair_light_bound(
                one,
                one,
                wide,
                solar,
                Years::new(2.0e10)..=Years::new(2.0e10)
            ),
            None
        );
        assert_eq!(
            pair_light_bound(one, one, wide, solar, Years::new(2.0e9)..=Years::new(1.0e9)),
            None
        );
        let helium = Composition::from_fe_h(
            crate::units::Dex::ZERO,
            crate::units::HeliumExcess::new(0.01),
        );
        assert_eq!(pair_light_bound(one, one, wide, &helium, ages()), None);
        assert_eq!(
            pair_light_bound_over(
                [SolarMasses::new(2.0)..=SolarMasses::new(1.0), one..=one],
                wide,
                solar,
                ages()
            ),
            None
        );
        // A mass that is not finite, or whose ratio to its companion's overflows, is not bounded.
        let young_age = || Years::new(1.0e5)..=Years::new(1.0e5);
        assert_eq!(
            pair_light_bound(
                SolarMasses::new(f64::INFINITY),
                one,
                wide,
                solar,
                young_age()
            ),
            None
        );
        assert_eq!(
            pair_light_bound(
                SolarMasses::new(1.0e308),
                SolarMasses::new(0.08),
                wide,
                solar,
                young_age()
            ),
            None
        );
        // A heavier star than the tracks' top reads the top, as its track does. Young, it is a
        // main-sequence star of some 20 R☉; by 1 Gyr it has collapsed.
        let young = || Years::new(1.0e5)..=Years::new(1.0e5);
        assert_eq!(
            pair_light_bound(SolarMasses::new(180.0), one, wide, solar, young()),
            Some(PairLight::Detached)
        );
        assert_eq!(
            pair_light_bound(SolarMasses::new(180.0), one, wide, solar, ages()),
            None
        );
    }

    // --- The generator's own pairs against the pre-test ------------------------------------------

    use super::super::testing::{Mix, generated_pairs};
    use crate::galaxy::Galaxy;
    use crate::id::Layer;
    use crate::stellar::binary::{BinaryInput, can_interact};

    const SEED: u64 = 0x0b17_0017_a11c_e5ed;

    /// The stellar layers.
    const LAYERS: [Layer; 5] = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];

    fn milky_way() -> Galaxy {
        super::super::testing::milky_way(SEED)
    }

    /// What one layer's pairs came to, at one class of ages.
    #[derive(Debug, Default)]
    struct Tally {
        pairs: u64,
        /// Pairs the pre-test passes over at their window's end.
        passed_over: u64,
        /// Pairs the bound calls `Detached`.
        detached: u64,
        /// Pairs a star of which may have died suddenly by the window's end, which the bound
        /// therefore leaves unbounded (the module's item 3).
        collapsed: u64,
        /// Pairs the bound calls `Detached` that the pre-test passes, described.
        missed: Vec<String>,
    }

    impl Tally {
        fn merge(&mut self, other: Self) {
            self.pairs += other.pairs;
            self.passed_over += other.passed_over;
            self.detached += other.detached;
            self.collapsed += other.collapsed;
            self.missed.extend(other.missed);
        }

        fn check(&mut self, input: &BinaryInput, age: f64) {
            let [m1, m2] = input.masses();
            let periastron = input.orbit().periapsis();
            let at = Years::new(age);
            let bound = pair_light_bound(m1, m2, periastron, input.composition(), at..=at);
            let interacts = can_interact(input, at);
            let collapsed = [m1, m2].iter().any(|&m| {
                crate::stellar::binary::largest_radius_bound(m..=m, input.composition(), at)
                    .is_some_and(|bound| bound.may_have_collapsed())
            });
            self.collapsed += u64::from(collapsed);
            self.pairs += 1;
            self.passed_over += u64::from(!interacts);
            if bound == Some(PairLight::Detached) {
                self.detached += 1;
                if interacts {
                    self.missed.push(format!(
                        "{:.4} + {:.4} M_sun, periastron {:.4e} R_sun, e {:.3}, [Fe/H] {:.3}, \
                         age {age:.4e} yr",
                        m1.value(),
                        m2.value(),
                        periastron.value() / SOLAR_RADIUS_M,
                        input.orbit().eccentricity().value(),
                        input.composition().fe_h().value(),
                    ));
                }
            }
        }
    }

    /// Each pair of `pairs` checked at its own age and at a young age, log-uniform over
    /// 10⁵–10⁸ years from `salt`'s stream, in eight shares (in turn on wasm32-wasip1): the old and
    /// young tallies.
    fn tally(pairs: &[(BinaryInput, f64)], salt: u64) -> [Tally; 2] {
        const SHARES: usize = 8;
        let mut mix = Mix(SEED ^ salt ^ 0x0000_a9e5);
        let young: Vec<f64> = pairs
            .iter()
            .map(|_| crate::math::exp10(5.0 + 3.0 * mix.unit()))
            .collect();
        let share = |k: usize| {
            let mut part = [Tally::default(), Tally::default()];
            for (i, (input, age)) in pairs.iter().enumerate().skip(k).step_by(SHARES) {
                part[0].check(input, *age);
                part[1].check(input, young[i]);
            }
            part
        };
        #[cfg(not(target_family = "wasm"))]
        let parts: Vec<[Tally; 2]> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..SHARES)
                .map(|k| {
                    let share = &share;
                    scope.spawn(move || share(k))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("a share's thread"))
                .collect()
        });
        #[cfg(target_family = "wasm")]
        let parts: Vec<[Tally; 2]> = (0..SHARES).map(share).collect();
        let mut total = [Tally::default(), Tally::default()];
        for [old, young] in parts {
            total[0].merge(old);
            total[1].merge(young);
        }
        total
    }

    /// Checks `per_layer` generated pairs of each stellar layer, printing each layer's shares, and
    /// asserts that no `Detached` pair passes the pre-test.
    fn no_detached_pair_interacts(per_layer: usize) {
        let galaxy = milky_way();
        let mut missed = 0;
        for (salt, layer) in (0_u64..).zip(LAYERS) {
            let pairs = generated_pairs(&galaxy, layer, per_layer, SEED ^ salt);
            for (class, t) in ["old (the record's age)", "young (1e5-1e8 yr)"]
                .iter()
                .zip(tally(&pairs, salt))
            {
                #[expect(clippy::cast_precision_loss, reason = "counts below 2⁵³")]
                let share = |k: u64| k as f64 / t.pairs as f64;
                println!(
                    "layer {layer:?}, {class}: {} pairs; the pre-test passes over {} ({:.4}); \
                     Detached {} ({:.4}), {:.4} of those passed over; a star may have collapsed \
                     in {} ({:.4})",
                    t.pairs,
                    t.passed_over,
                    share(t.passed_over),
                    t.detached,
                    share(t.detached),
                    share(t.detached) / share(t.passed_over.max(1)),
                    t.collapsed,
                    share(t.collapsed),
                );
                for line in t.missed.iter().take(20) {
                    println!("  Detached but passed by the pre-test: {line}");
                }
                missed += t.missed.len();
            }
        }
        assert_eq!(missed, 0, "Detached pairs that the pre-test passes");
    }

    /// P11.T17.a: 2,000 of the generator's pairs, 400 of each stellar layer, each at its record's
    /// age and at a young one: no `Detached` pair passes `can_interact` at its window's end.
    #[test]
    fn detached_generated_pairs_are_passed_over() {
        no_detached_pair_interacts(400);
    }

    /// P11.T17.a's slow test: as [`detached_generated_pairs_are_passed_over`] over 10⁵ pairs of
    /// each stellar layer, recording each layer's share of `Detached` against the pre-test's.
    #[test]
    #[ignore = "slow: 5 × 10⁵ generated pairs, each through the pre-test twice"]
    fn no_detached_pair_is_passed_by_the_pre_test() {
        no_detached_pair_interacts(100_000);
    }
}
