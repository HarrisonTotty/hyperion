//! Quadratures over the multiplicity model (plan 11, P11.T1.c): the share of all stars below a
//! mass, the companions' mass per system, and the share of massive primaries a companion strips.
//!
//! Each is a fixed-node Gauss–Legendre quadrature with no randomness, over stellar companions
//! only: brown-dwarf companions are left out of all three (Design note 15). Plan 02's
//! `fates::stars_below`, plan 08's `displaced::binarity::stripped_share` and plan 15's P15.T4.b
//! call them once P11.T1.d wires them in; until then nothing calls them, so no output depends on
//! them. The panel scheme and node counts are part of the generator version from then on.

use super::dist::{
    CIRCULARISATION_PERIOD, MIN_COMPANION_MASS, MassRatioDistribution, PeriodDistribution,
    PeriodRegime, TWIN_MAX_PERIOD, TWIN_MIN_MASS_RATIO,
};
use super::model::MultiplicityModel;
use crate::galaxy::imf::{MASS_LIMIT_HI, MASS_LIMIT_LO, MassFunction};
use crate::galaxy::quad::{gl16, gl32_log};
use crate::math;
use crate::stellar::Composition;
use crate::units::consts::GM_SUN;
use crate::units::{Metres, Seconds, SolarMasses};

/// The widest panel of the quadratures over primary mass, in ln m.
const MAX_PANEL_LN_MASS: f64 = 0.5;

/// The widest panel of the quadrature over the period, in x = log₁₀(P ÷ 1 d).
const MAX_PANEL_LOG_PERIOD: f64 = 0.5;

/// The primary mass, M☉, below which the twins' range starts at the lowest mass ratio rather than
/// at 0.95: 0.08 ÷ 0.95, a kink of the mass-ratio law.
const TWIN_RANGE_KINK: f64 = MASS_LIMIT_LO / TWIN_MIN_MASS_RATIO;

/// The integral over the primary's mass of `f`, from 0.08 to 150 M☉, by [`gl32_log`] on panels
/// between the stellar range's ends, the mass function's breaks, the model's anchor masses,
/// [`TWIN_RANGE_KINK`] and `extra`, each split into equal parts no wider than
/// [`MAX_PANEL_LN_MASS`] in ln m.
#[must_use]
fn integrate_primaries(
    mf: &dyn MassFunction,
    model: &MultiplicityModel,
    extra: &[f64],
    mut f: impl FnMut(SolarMasses) -> f64,
) -> f64 {
    let mut edges: Vec<f64> = mf
        .breaks()
        .iter()
        .copied()
        .chain(model.anchor_masses())
        .chain([TWIN_RANGE_KINK])
        .chain(extra.iter().copied())
        .filter(|&m| m > MASS_LIMIT_LO && m < MASS_LIMIT_HI)
        .collect();
    edges.push(MASS_LIMIT_LO);
    edges.push(MASS_LIMIT_HI);
    edges.sort_by(f64::total_cmp);
    edges.dedup();
    let mut sum = 0.0;
    for pair in edges.windows(2) {
        let (start, end) = (math::ln(pair[0]), math::ln(pair[1]));
        let mut pieces = 1_u32;
        while (end - start) / f64::from(pieces) > MAX_PANEL_LN_MASS {
            pieces += 1;
        }
        let step = (end - start) / f64::from(pieces);
        for i in 0..pieces {
            let lo = start + step * f64::from(i);
            let hi = if i + 1 == pieces { end } else { lo + step };
            sum += gl32_log(
                |m| mf.pdf(m) * f(SolarMasses::new(m)),
                math::exp(lo),
                math::exp(hi),
            );
        }
    }
    sum
}

/// The share of all stars, primaries and their stellar companions together, whose initial mass
/// is below `m`, for primaries drawn from `mf`.
///
/// This is the brainstorm's arbiter between the mass functions ("Sizing the layers"): the 20 pc
/// census has 69% of all its stars of 0.08 M☉ or more below 0.5 M☉ (Kirkpatrick et al. 2024, ApJS
/// 271, 55, Table 18). A primary of `m₁` counts 1 if it is below `m`, and each of its
/// [`companion_frequency`](MultiplicityModel::companion_frequency) companions counts the
/// probability that `q m₁ < m` under the mass-ratio law marginalised over the period
/// ([`companion_mass_ratio_cdf`](MultiplicityModel::companion_mass_ratio_cdf)). Integrated over
/// the stellar range in ln m by 32-point Gauss–Legendre panels no wider than 0.5, with edges at
/// every kink, including `m` and `m ÷ 0.95`, and divided by the number of all stars the same way.
///
/// # Examples
///
/// Under Chabrier's function as published, two thirds of all stars are below 0.5 M☉:
///
/// ```
/// use hyperion_sim::galaxy::imf::Chabrier;
/// use hyperion_sim::stellar::multiplicity::{MultiplicityModel, all_stars_fraction_below};
/// use hyperion_sim::units::SolarMasses;
///
/// let published = Chabrier::new(1.0)?;
/// let model = MultiplicityModel::default_v1();
/// let below = all_stars_fraction_below(&published, &model, SolarMasses::new(0.5));
/// assert!((below - 0.67).abs() < 0.02);
/// # Ok::<(), hyperion_sim::galaxy::imf::BuildChabrierError>(())
/// ```
#[must_use]
pub fn all_stars_fraction_below(
    mf: &dyn MassFunction,
    model: &MultiplicityModel,
    m: SolarMasses,
) -> f64 {
    let cut = m.value();
    let kinks = [cut, cut / TWIN_MIN_MASS_RATIO];
    let below = integrate_primaries(mf, model, &kinks, |m1| {
        let primary = if m1.value() < cut { 1.0 } else { 0.0 };
        let companions =
            model.companion_frequency(m1) * model.companion_mass_ratio_cdf(m1, cut / m1.value());
        primary + companions
    });
    let all = integrate_primaries(mf, model, &kinks, |m1| 1.0 + model.companion_frequency(m1));
    below / all
}

/// The mean initial mass of a system's stellar companions, per system, for primaries drawn from
/// `mf`: `∫ ξ(m₁) CF(m₁) m₁ E[q | m₁] dm₁ ÷ ∫ ξ(m₁) dm₁`, with the mean mass ratio marginalised
/// over the period ([`mean_companion_mass_ratio`](MultiplicityModel::mean_companion_mass_ratio)).
///
/// Integrated as [`all_stars_fraction_below`] is; the normalisation is the mass function's
/// closed-form integral. Plan 15's P15.T4.b fits Chabrier's high-mass scale against it.
#[must_use]
pub fn mean_companion_mass_per_system(
    mf: &dyn MassFunction,
    model: &MultiplicityModel,
) -> SolarMasses {
    let mass = integrate_primaries(mf, model, &[], |m1| {
        model.companion_frequency(m1) * m1.value() * model.mean_companion_mass_ratio(m1)
    });
    SolarMasses::new(mass / mf.integral(MASS_LIMIT_LO, MASS_LIMIT_HI))
}

/// The semi-major axis of a relative orbit of `period` about a total mass `total`, by Kepler's
/// third law, `a³ = G M (P ÷ 2π)²`, with the nominal GM☉ (IAU 2015 Resolution B3).
#[must_use]
fn semi_major_axis(total: SolarMasses, period: Seconds) -> Metres {
    let n = period.value() / core::f64::consts::TAU;
    Metres::new(math::cbrt(GM_SUN * total.value() * n * n))
}

/// x = log₁₀(P ÷ 1 d) of the circularisation period.
#[must_use]
fn log_circularisation_period() -> f64 {
    math::log10(CIRCULARISATION_PERIOD.value())
}

/// The probability that a companion of one mass ratio, on an orbit drawn from `periods` inside
/// `[x_lo, x_hi]`, has its periastron inside `ratio` times the separation of a circular orbit of
/// the circularisation period, with the eccentricity law of
/// [`eccentricity_distribution`](MultiplicityModel::eccentricity_distribution).
///
/// With `y = a ÷ a_circ = 10^((2/3)(x − x_circ))`, the cap on e holds the periastron at or beyond
/// `a_circ`, so for `ratio ≤ 1` only circular orbits with `a < ratio a_circ` interact, and for
/// `ratio > 1` every orbit with `y ≤ ratio` does and a wider one with probability
/// `(ratio − 1) ÷ (y − 1)`, the share of `[0, e_max]` whose periastron is inside. The two closed
/// parts are differences of the period distribution; the tail is 16-point Gauss–Legendre on
/// panels no wider than 0.5 in x, with edges at every component's limits.
#[must_use]
fn interacting_share(periods: &PeriodDistribution, ratio: f64, x_lo: f64, x_hi: f64) -> f64 {
    if x_hi <= x_lo || ratio <= 0.0 {
        return 0.0;
    }
    let x_circ = log_circularisation_period();
    let x_inside = x_circ + 1.5 * math::log10(ratio);
    let closed_hi = x_inside.clamp(x_lo, x_hi);
    let closed = periods.cdf(closed_hi) - periods.cdf(x_lo);
    if ratio <= 1.0 || closed_hi >= x_hi {
        return closed.max(0.0);
    }
    let tail = |x: f64| {
        let y = math::exp10((2.0 / 3.0) * (x - x_circ));
        periods.pdf(x) * (ratio - 1.0) / (y - 1.0)
    };
    let mut edges = vec![closed_hi, x_hi];
    edges.extend(
        periods
            .component_limits()
            .filter(|&x| x > closed_hi && x < x_hi),
    );
    edges.sort_by(f64::total_cmp);
    edges.dedup();
    let mut sum = closed.max(0.0);
    for pair in edges.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let mut pieces = 1_u32;
        while (end - start) / f64::from(pieces) > MAX_PANEL_LOG_PERIOD {
            pieces += 1;
        }
        let step = (end - start) / f64::from(pieces);
        for i in 0..pieces {
            let lo = start + step * f64::from(i);
            let hi = if i + 1 == pieces { end } else { lo + step };
            sum += gl16(tail, lo, hi);
        }
    }
    sum
}

/// The mean of `g(q)` over a mass-ratio law: its smooth part by 16-point Gauss–Legendre in the
/// law's own cumulative share, where the integrand is smooth even for a steep `q^γ`, and its
/// twins by the same rule on their uniform range.
#[must_use]
fn over_mass_ratios(law: &MassRatioDistribution, mut g: impl FnMut(f64) -> f64) -> f64 {
    if law.lo() >= 1.0 {
        return g(1.0);
    }
    let w = law.twin_share();
    let smooth_law = law.smooth_part();
    let smooth = gl16(|u| g(smooth_law.quantile(u)), 0.0, 1.0);
    if w <= 0.0 {
        return smooth;
    }
    let twin_lo = TWIN_MIN_MASS_RATIO.max(law.lo());
    let twins = gl16(&mut g, twin_lo, 1.0) / (1.0 - twin_lo);
    (1.0 - w) * smooth + w * twins
}

/// The share of primaries of initial mass `m1` and composition `comp` whose innermost companion's
/// periastron is close enough to interact before the primary's core collapse: the provisional
/// companion-stripped share that plan 06's mark and plan 08's class table read (Design note 1).
///
/// `interacting_periastron(m1, q, comp)` is the largest periastron at which a pair of that
/// primary, mass ratio and composition interacts before the primary's core collapse: P11.T4.a's
/// `can_interact` threshold, which P11.T1.d supplies; until it exists a caller passes its own (the
/// tests pass periastra under 10 au). The share is the primary's
/// [`multiple_fraction`](MultiplicityModel::multiple_fraction) times the probability that its
/// innermost orbit, drawn from the model's period, mass-ratio and eccentricity laws as the
/// hierarchy draws it (Design notes 1 and 4), has a periastron under the threshold. Integrated
/// over the three period regimes of the mass-ratio law: in each, over the mass ratio by
/// 16-point Gauss–Legendre in the law's cumulative share, and for each ratio over the period and
/// the eccentricity as `interacting_share` does. No randomness; brown-dwarf companions are left
/// out.
///
/// # Panics
///
/// If `m1` is not positive and finite.
///
/// # Examples
///
/// With a threshold of 10 au, most massive primaries with a companion count:
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::multiplicity::{MultiplicityModel, stripped_share};
/// use hyperion_sim::units::{AstronomicalUnits, Metres, SolarMasses};
///
/// let model = MultiplicityModel::default_v1();
/// let ten_au =
///     |_: SolarMasses, _: f64, _: &Composition| Metres::from(AstronomicalUnits::new(10.0));
/// let m1 = SolarMasses::new(20.0);
/// let share = stripped_share(&model, m1, &Composition::SOLAR, ten_au);
/// assert!(share > 0.0 && share < model.multiple_fraction(m1));
/// ```
#[must_use]
pub fn stripped_share(
    model: &MultiplicityModel,
    m1: SolarMasses,
    comp: &Composition,
    interacting_periastron: impl Fn(SolarMasses, f64, &Composition) -> Metres,
) -> f64 {
    let periods = model.period_distribution(m1);
    let (support_lo, support_hi) = periods.support();
    let twin_edge = math::log10(TWIN_MAX_PERIOD.value());
    let close_edge = super::dist::CLOSE_MAX_LOG_PERIOD;
    let circular = Seconds::from(CIRCULARISATION_PERIOD);
    let mut inner = 0.0;
    for regime in PeriodRegime::ALL {
        let (lo, hi) = match regime {
            PeriodRegime::Twin => (support_lo, twin_edge),
            PeriodRegime::Close => (twin_edge, close_edge),
            PeriodRegime::Wide => (close_edge, support_hi),
        };
        let (lo, hi) = (lo.max(support_lo), hi.min(support_hi));
        if hi <= lo {
            continue;
        }
        let law = model.mass_ratio_in(m1, regime);
        inner += over_mass_ratios(&law, |q| {
            let total = m1 * (1.0 + q);
            let a_circ = semi_major_axis(total, circular);
            let ratio = interacting_periastron(m1, q, comp) / a_circ;
            interacting_share(&periods, ratio, lo, hi)
        });
    }
    model.multiple_fraction(m1) * inner
}

/// The companions' lowest mass: brown dwarfs are never counted here.
const _: () = assert!(MIN_COMPANION_MASS.value() >= MASS_LIMIT_LO);

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

    use super::*;
    use crate::Seed;
    use crate::galaxy::imf::{Chabrier, Kroupa};
    use crate::rng::{ObjectKey, Stream, tags};
    use crate::units::{AstronomicalUnits, Days};

    fn model() -> MultiplicityModel {
        MultiplicityModel::default_v1()
    }

    fn ten_au(_: SolarMasses, _: f64, _: &Composition) -> Metres {
        Metres::from(AstronomicalUnits::new(10.0))
    }

    /// The brainstorm's figures for all stars below 0.5 M☉ ("Sizing the layers"): 76.4% under
    /// Kroupa's function for primaries and 66.9% under Chabrier's as published, against the 20 pc
    /// census's 69% (Kirkpatrick et al. 2024, Table 18; the 75.9% once quoted as observed is
    /// Kroupa's function itself). The plan's brackets are 0.764 ± 0.005 and 0.67 ± 0.01.
    #[test]
    fn the_all_stars_fraction_brackets_the_census() {
        let model = model();
        let half = SolarMasses::new(0.5);
        let kroupa = all_stars_fraction_below(&Kroupa, &model, half);
        let published = all_stars_fraction_below(&Chabrier::new(1.0).unwrap(), &model, half);
        let default = all_stars_fraction_below(&Chabrier::provisional(), &model, half);
        println!(
            "all stars below 0.5 M☉: Kroupa {kroupa:.4}, Chabrier as published {published:.4}, \
             default (scale 0.68) {default:.4}; census 0.69"
        );
        assert!((kroupa - 0.764).abs() <= 0.005, "Kroupa: {kroupa}");
        assert!((published - 0.67).abs() <= 0.01, "Chabrier: {published}");
        assert!(published < 0.69 && 0.69 < kroupa);
    }

    #[test]
    fn the_all_stars_fraction_runs_from_nothing_to_everything() {
        let model = model();
        let mut last = 0.0;
        for m in [0.08, 0.1, 0.2, 0.5, 1.0, 2.0, 8.0, 40.0, 150.0] {
            let f = all_stars_fraction_below(&Kroupa, &model, SolarMasses::new(m));
            assert!(f >= last, "{f} below {m} M☉ falls under {last}");
            last = f;
        }
        let none = all_stars_fraction_below(&Kroupa, &model, SolarMasses::new(0.08));
        assert!(none.abs() < 1e-15, "{none}");
        let all = all_stars_fraction_below(&Kroupa, &model, SolarMasses::new(150.0 * 1.0001));
        assert!((all - 1.0).abs() < 1e-12, "{all}");
    }

    /// Stars per system lie in P11.T1.d's bracket, 1.33–1.45, under all three mass functions.
    /// The companions' initial mass per system is printed beside plan 02's stand-in's (the formed
    /// mass less the primaries') for T1.d, which wires the model in. The census counts 0.32–0.38
    /// stellar companions per system (plan 11, Risks), a present-day count.
    #[test]
    fn stars_per_system_lie_in_the_fates_bracket() {
        use crate::galaxy::fates::{ProvisionalFates, mean_formed_mass, mean_stars_per_system};
        let model = model();
        let published = Chabrier::new(1.0).unwrap();
        let default = Chabrier::provisional();
        for (name, mf) in [
            ("Kroupa", &Kroupa as &dyn MassFunction),
            ("Chabrier as published", &published),
            ("default", &default),
        ] {
            let systems = mf.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
            let count =
                integrate_primaries(mf, &model, &[], |m1| model.companion_frequency(m1)) / systems;
            let mass = mean_companion_mass_per_system(mf, &model).value();
            let primaries = integrate_primaries(mf, &model, &[], SolarMasses::value) / systems;
            let stand_in_count = mean_stars_per_system(mf, &ProvisionalFates) - 1.0;
            let stand_in_mass = mean_formed_mass(mf, &ProvisionalFates).value() - primaries;
            println!(
                "{name}: {:.4} stars per system, {mass:.4} M☉ of companions; plan 02's stand-in \
                 {:.4} and {stand_in_mass:.4} M☉",
                1.0 + count,
                1.0 + stand_in_count
            );
            assert!((1.33..=1.45).contains(&(1.0 + count)), "{count}");
            assert!(mass > 0.0 && mass < primaries, "{mass} against {primaries}");
        }
    }

    /// A two-sided p-value for a sample mean against its expectation.
    fn p_value(mean: f64, expected: f64, standard_error: f64) -> f64 {
        2.0 * normal_cdf(-((mean - expected) / standard_error).abs())
    }

    /// The quadratures agree with the model's own samplers: primaries from the mass function,
    /// counts from the count distribution, and each companion's period and then mass ratio, which
    /// is the path the hierarchy draw takes. Each per-system mean (stars below 0.5 M☉, all stars,
    /// companion mass) is tested against its quadrature at α = 10⁻³, systems being independent.
    #[test]
    fn the_quadratures_agree_with_the_samplers() {
        let model = model();
        let mf = Chabrier::provisional();
        let mut stream = Stream::open(Seed::new(11), tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let n = 40_000_u32;
        // Per system: stars below 0.5 M☉, all stars, companion mass; their sums and squares.
        let mut sums = [0.0; 3];
        let mut squares = [0.0; 3];
        for _ in 0..n {
            let m1 =
                SolarMasses::new(mf.quantile_in(MASS_LIMIT_LO, MASS_LIMIT_HI, stream.uniform()));
            let pmf = model.companion_count_pmf(m1);
            let total = pmf.iter().fold(0.0, |sum, p| sum + p);
            let count = stream.mark().pick_weighted(&pmf, total).unwrap();
            let mut system = [if m1.value() < 0.5 { 1.0 } else { 0.0 }, 1.0, 0.0];
            for _ in 0..count {
                let period = model.period_distribution(m1).sample(&mut stream);
                let q = model
                    .mass_ratio_distribution(m1, period)
                    .sample(&mut stream);
                let m2 = q * m1.value();
                system[0] += if m2 < 0.5 { 1.0 } else { 0.0 };
                system[1] += 1.0;
                system[2] += m2;
            }
            for (i, x) in system.into_iter().enumerate() {
                sums[i] += x;
                squares[i] += x * x;
            }
        }
        let n = f64::from(n);
        let systems = mf.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
        let half = 0.5;
        let expected = [
            integrate_primaries(&mf, &model, &[half, half / TWIN_MIN_MASS_RATIO], |m1| {
                let primary = if m1.value() < half { 1.0 } else { 0.0 };
                primary
                    + model.companion_frequency(m1)
                        * model.companion_mass_ratio_cdf(m1, half / m1.value())
            }) / systems,
            integrate_primaries(&mf, &model, &[], |m1| 1.0 + model.companion_frequency(m1))
                / systems,
            mean_companion_mass_per_system(&mf, &model).value(),
        ];
        for (i, name) in ["stars below 0.5 M☉", "stars", "companion mass"]
            .iter()
            .enumerate()
        {
            let mean = sums[i] / n;
            let error = ((squares[i] / n - mean * mean) / n).sqrt();
            let p = p_value(mean, expected[i], error);
            println!(
                "{name} per system: sampled {mean:.4} ± {error:.4}, quadrature {:.4}, p = {p:.3}",
                expected[i]
            );
            assert_p_value(name, p, ALPHA);
        }
        let fraction = all_stars_fraction_below(&mf, &model, SolarMasses::new(half));
        assert!(
            (fraction - expected[0] / expected[1]).abs() < 1e-12,
            "{fraction}"
        );
    }

    #[test]
    fn the_stripped_share_runs_from_none_to_every_multiple() {
        let model = model();
        let comp = Composition::SOLAR;
        for m in [8.0, 12.0, 20.0, 40.0, 100.0] {
            let m1 = SolarMasses::new(m);
            let none = stripped_share(&model, m1, &comp, |_, _, _| Metres::ZERO);
            assert!(none.abs() < 1e-15, "{none} at {m} M☉");
            let everything = stripped_share(&model, m1, &comp, |_, _, _| {
                Metres::from(AstronomicalUnits::new(1e9))
            });
            let mf = model.multiple_fraction(m1);
            assert!(
                (everything - mf).abs() < 1e-9,
                "{everything} against {mf} at {m} M☉"
            );
            let mut last = 0.0;
            for au in [0.01, 0.1, 1.0, 10.0, 100.0, 1e3, 1e4] {
                let share = stripped_share(&model, m1, &comp, |_, _, _| {
                    Metres::from(AstronomicalUnits::new(au))
                });
                assert!(
                    share >= last - 1e-15,
                    "{share} at {au} au falls under {last}"
                );
                last = share;
            }
        }
    }

    /// The stripped share by quadrature equals the share of sampled innermost orbits whose
    /// periastron lies inside 10 au, at three masses, at α = 10⁻³.
    #[test]
    fn the_stripped_share_agrees_with_sampled_orbits() {
        let model = model();
        let comp = Composition::SOLAR;
        let mut stream = Stream::open(Seed::new(12), tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let n = 20_000_u32;
        for m in [0.5, 8.0, 30.0] {
            let m1 = SolarMasses::new(m);
            let periods = model.period_distribution(m1);
            let mut inside = 0_u32;
            for _ in 0..n {
                let period = periods.sample(&mut stream);
                let q = model
                    .mass_ratio_distribution(m1, period)
                    .sample(&mut stream);
                let e = model.eccentricity_distribution(period).sample(&mut stream);
                let a = semi_major_axis(m1 * (1.0 + q), Seconds::from(period));
                if a * (1.0 - e) < ten_au(m1, q, &comp) {
                    inside += 1;
                }
            }
            let fraction = model.multiple_fraction(m1);
            let sampled = f64::from(inside) / f64::from(n);
            let share = stripped_share(&model, m1, &comp, ten_au);
            let expected = share / fraction;
            let error = (expected * (1.0 - expected) / f64::from(n)).sqrt();
            let p = p_value(sampled, expected, error);
            println!(
                "inside 10 au at {m} M☉: sampled {:.4}, quadrature {share:.4}, p = {p:.3}",
                fraction * sampled
            );
            assert_p_value(&format!("stripped share at {m} M☉"), p, ALPHA);
        }
    }

    /// The gap P11.T1.d closes, printed per mass: this share under the stand-in threshold against
    /// the constant the provisional mark reads today, plan 06's 0.25 (design note 11), which plan
    /// 08's `ClassTable::stripped_share_used` would return had plan 08 been built. Each share lies
    /// within the primary's multiple fraction.
    #[test]
    fn the_stripped_share_lies_within_the_multiple_fraction_and_its_gap_is_printed() {
        let model = model();
        let provisional = 0.25;
        for m in [8.0, 10.0, 12.0, 16.0, 20.0, 30.0, 50.0, 100.0, 150.0] {
            let m1 = SolarMasses::new(m);
            let share = stripped_share(&model, m1, &Composition::SOLAR, ten_au);
            println!(
                "{m:>5} M☉: stripped share {share:.4} under 10 au, provisional {provisional}, \
                 gap {:+.4}",
                share - provisional
            );
            assert!((0.0..=model.multiple_fraction(m1)).contains(&share));
        }
    }

    #[test]
    fn the_quadratures_are_pure() {
        let model = model();
        let mf = Chabrier::provisional();
        let half = SolarMasses::new(0.5);
        let m1 = SolarMasses::new(15.0);
        let first = (
            all_stars_fraction_below(&mf, &model, half),
            mean_companion_mass_per_system(&mf, &model).value(),
            stripped_share(&model, m1, &Composition::SOLAR, ten_au),
        );
        let _ = stripped_share(&model, SolarMasses::new(40.0), &Composition::SOLAR, ten_au);
        let _ = all_stars_fraction_below(&Kroupa, &model, SolarMasses::new(1.0));
        assert_same_bits(first.0, all_stars_fraction_below(&mf, &model, half));
        assert_same_bits(first.1, mean_companion_mass_per_system(&mf, &model).value());
        assert_same_bits(
            first.2,
            stripped_share(&model, m1, &Composition::SOLAR, ten_au),
        );
    }

    #[test]
    fn a_circular_orbit_of_the_circularisation_period_has_the_expected_size() {
        // Two solar masses at 12 days: a = (2 × (12 ÷ 365.25)²)^(1/3) au = 0.1292 au.
        let a = semi_major_axis(SolarMasses::new(2.0), Seconds::from(Days::new(12.0)));
        let au = AstronomicalUnits::from(a).value();
        assert!((au - 0.129_2).abs() < 2e-4, "{au}");
    }
}
