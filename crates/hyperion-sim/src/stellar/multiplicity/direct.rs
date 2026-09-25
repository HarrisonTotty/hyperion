//! Direct companions of primaries of 1.5 M☉ and more, drawn as Moe and Di Stefano (2017, ApJS
//! 230, 15) count them (plan 11, ruling 81): the number of companions that orbit the primary
//! directly, and each one's period and mass ratio, independently, from their Table 13 and their
//! fitted laws (eqs. 5–7, 9–23).
//!
//! Moe and Di Stefano count only companions of q > 0.1 with 0.2 < log₁₀(P ÷ 1 d) < 8 that orbit
//! the primary directly, at most three (§2, §9.4, Table 1). The hierarchy draw
//! ([`draw_hierarchy`](super::draw_hierarchy)) inserts each newest companion by period and redraws
//! that companion alone until the whole hierarchy passes Mardling and Aarseth's criterion
//! (ruling 81 as amended); subsystems of the companions come on top and are never counted here. Rejection thins close period ratios, so the
//! drawn period law carries a fitted correction ([`PERIOD_CORRECTION`]) under which the periods
//! that survive follow Moe and Di Stefano's law. None of this opens a stream.

use super::dist::{
    MassRatioDistribution, TWIN_REFERENCE_MASS_RATIO, excess_twin_fraction, gamma_large,
    gamma_small,
};
use super::model::{blend, capped_geometric_ratio, lerp};
use crate::math;
use crate::units::SolarMasses;

/// The primary masses, M☉, between which the hierarchy draw blends its two constructions,
/// linearly in ln M₁ (ruling 81): below 1.5 M☉ every system is drawn as Duchêne and Kraus's and
/// Raghavan's counts are fitted (companions joining the outer spine), above 3 M☉ every system's
/// direct companions are drawn as Moe and Di Stefano count them, and between the two a mark
/// picks the construction with the direct one's weight rising from 0 to 1.
pub(super) const DIRECT_BLEND_MASSES: (f64, f64) = (1.5, 3.0);

/// The weight of the direct construction for a primary of `m1`: 0 up to 1.5 M☉, 1 from 3 M☉,
/// linear in ln M₁ between ([`DIRECT_BLEND_MASSES`]).
#[must_use]
pub(super) fn direct_weight(m1: SolarMasses) -> f64 {
    let (lo, hi) = DIRECT_BLEND_MASSES;
    let m = m1.value();
    if m <= lo {
        0.0
    } else if m >= hi {
        1.0
    } else {
        math::ln(m / lo) / math::ln(hi / lo)
    }
}

/// Moe and Di Stefano's (2017, Table 13) direct-companion statistics, re-checked against the
/// paper: the interval's mean primary mass as they evaluate their fits (§9.1), M☉; the single
/// star fraction `F_n=0;q>0.1`; and the total multiplicity frequency `f_mult;q>0.1`.
///
/// | Interval          | M₁  | `F_n=0`     | `f_mult`    |
/// | ----------------- | --- | ----------- | ----------- |
/// | solar-type        | 1   | 0.60 ± 0.04 | 0.50 ± 0.04 |
/// | A/late-B, 2–5 M☉  | 3.5 | 0.41 ± 0.08 | 0.84 ± 0.11 |
/// | mid-B, 5–9 M☉     | 7   | 0.24 ± 0.08 | 1.3 ± 0.2   |
/// | early-B, 9–16 M☉  | 12  | 0.16 ± 0.09 | 1.6 ± 0.2   |
/// | O-type, > 16 M☉   | 28  | 0.06 ± 0.06 | 2.1 ± 0.3   |
///
/// Interpolated linearly in ln M₁ and held outside. The solar-type row only shapes the blend
/// region, 1.5–3 M☉.
pub(super) const DIRECT_ANCHORS: [(f64, f64, f64); 5] = [
    (1.0, 0.60, 0.50),
    (3.5, 0.41, 0.84),
    (7.0, 0.24, 1.3),
    (12.0, 0.16, 1.6),
    (28.0, 0.06, 2.1),
];

/// The single-star fraction and the multiplicity frequency of direct companions for a primary
/// of `m1`, interpolated in [`DIRECT_ANCHORS`].
#[must_use]
fn direct_statistics(m1: SolarMasses) -> (f64, f64) {
    let (i, t) = blend(&DIRECT_ANCHORS, |a| a.0, m1.value());
    let (a, b) = (DIRECT_ANCHORS[i], DIRECT_ANCHORS[i + 1]);
    (lerp(a.1, b.1, t), lerp(a.2, b.2, t))
}

/// The share of primaries of `m1` with at least one direct companion, `1 − F_n=0`.
#[must_use]
pub(super) fn direct_multiple_fraction(m1: SolarMasses) -> f64 {
    1.0 - direct_statistics(m1).0
}

/// The distribution of the number of direct companions of a primary of `m1`, 0 to 3: Moe and
/// Di Stefano's `F_n=0` for none, and for a multiple 1 plus a geometric count capped at three
/// whose mean is `f_mult ÷ (1 − F_n=0)`, the count model of
/// [`companion_count_pmf`](super::MultiplicityModel::companion_count_pmf). It reproduces both
/// statistics of Table 13 exactly (their eq. 28 relates them), where Moe and Di Stefano assume a
/// Poisson law truncated to n = 0–3 (§9.4); its binary and triple-plus shares are then within their
/// 1σ at every interval (0.40 and 0.19 against 0.37 ± 0.06 and 0.22 ± 0.07 at 3.5 M☉; 0.27 and
/// 0.67 against 0.21 ± 0.11 and 0.73 ± 0.16 at 28 M☉).
#[must_use]
pub(super) fn direct_count_pmf(m1: SolarMasses) -> [f64; 4] {
    let (single, frequency) = direct_statistics(m1);
    let multiple = 1.0 - single;
    let r = capped_geometric_ratio(frequency / multiple);
    [
        single,
        multiple * (1.0 - r),
        multiple * (1.0 - r) * r,
        multiple * r * r,
    ]
}

/// The shortest and longest periods, as x = log₁₀(P ÷ 1 d), of a direct companion: Moe and Di
/// Stefano's range, 0.2–8 (§2).
pub(super) const DIRECT_LOG_PERIOD_RANGE: (f64, f64) = (0.2, 8.0);

/// The lowest mass ratio of a direct companion: q = 0.1, the lowest Moe and Di Stefano count.
/// For every primary of the direct construction (1.5 M☉ and more) a companion of q = 0.1 is at
/// least 0.15 M☉, a star.
pub(super) const DIRECT_MIN_MASS_RATIO: f64 = 0.1;

/// Moe and Di Stefano's (2017, eqs. 20–23) frequency of companions of q > 0.3 per decade of
/// period, `f_logP;q>0.3(M₁, P)`, at x = `log_period` = log₁₀(P ÷ 1 d), re-checked against the
/// paper, with α = 0.018 and Δ = 0.7:
///
/// - eq. 20: `f_logP<1 = 0.020 + 0.04 log M₁ + 0.07 (log M₁)²`;
/// - eq. 21: `f_logP=2.7 = 0.039 + 0.07 log M₁ + 0.01 (log M₁)²`;
/// - eq. 22: `f_logP=5.5 = 0.078 − 0.05 log M₁ + 0.04 (log M₁)²`;
/// - eq. 23: flat at `f_logP<1` over 0.2–1; linear to `f_logP=2.7 − αΔ` at 2.0; rising at α
///   across 2.0–3.4; linear from `f_logP=2.7 + αΔ` to `f_logP=5.5` across 3.4–5.5; and falling
///   as `exp(−0.3 (log P − 5.5))` across 5.5–8.
///
/// Zero outside 0.2–8.
#[must_use]
pub(super) fn frequency_above_three_tenths(m1: SolarMasses, log_period: f64) -> f64 {
    const ALPHA: f64 = 0.018;
    const DELTA: f64 = 0.7;
    let l = math::log10(m1.value());
    let short = 0.020 + 0.04 * l + 0.07 * l * l;
    let middle = 0.039 + 0.07 * l + 0.01 * l * l;
    let long = 0.078 - 0.05 * l + 0.04 * l * l;
    let x = log_period;
    let (lo, hi) = DIRECT_LOG_PERIOD_RANGE;
    if !(lo..=hi).contains(&x) {
        0.0
    } else if x < 1.0 {
        short
    } else if x < 2.7 - DELTA {
        short + (x - 1.0) / (1.7 - DELTA) * (middle - short - ALPHA * DELTA)
    } else if x < 2.7 + DELTA {
        middle + ALPHA * (x - 2.7)
    } else if x < 5.5 {
        middle + ALPHA * DELTA + (x - 2.7 - DELTA) / (2.8 - DELTA) * (long - middle - ALPHA * DELTA)
    } else {
        long * math::exp(-0.3 * (x - 5.5))
    }
}

/// The smooth mass-ratio law of a direct companion, without its twins: Moe and Di Stefano's
/// broken power law on q = 0.1–1, `γ_smallq` below 0.3 and `γ_largeq` above (eqs. 9–11, 13–15).
#[must_use]
fn smooth_counted_law(m1: SolarMasses, log_period: f64) -> MassRatioDistribution {
    let m = m1.value();
    MassRatioDistribution::smooth(
        DIRECT_MIN_MASS_RATIO,
        &[
            (TWIN_REFERENCE_MASS_RATIO, gamma_small(m, log_period)),
            (1.0, gamma_large(m, log_period)),
        ],
    )
}

/// The mass-ratio law of a direct companion of a primary of `m1` at x = `log_period`: Moe and Di
/// Stefano's broken power law on q = 0.1–1 with their excess twins (eqs. 5–7), the law their
/// counts are of.
#[must_use]
pub(super) fn direct_mass_ratio_law(m1: SolarMasses, log_period: f64) -> MassRatioDistribution {
    smooth_counted_law(m1, log_period).with_twins(excess_twin_fraction(m1, log_period))
}

/// Moe and Di Stefano's frequency of companions of q > 0.1 per decade of period,
/// `f_logP;q>0.1(M₁, P)`: `f_logP;q>0.3` ([`frequency_above_three_tenths`]) times the ratio of
/// companions above q = 0.1 to those above 0.3 under their mass-ratio law, `1 + (1 − F_twin) R`,
/// where R is the smooth law's share on q = 0.1–0.3 over its share above 0.3 (the excess twins
/// all lie above 0.3). This is how they compute it (§9.3, "we calculate the frequency
/// `f_logP;q>0.1(M₁, P)` of companions with q > 0.1 per decade of orbital period").
#[must_use]
pub(super) fn frequency_above_a_tenth(m1: SolarMasses, log_period: f64) -> f64 {
    let above = frequency_above_three_tenths(m1, log_period);
    if above <= 0.0 {
        return 0.0;
    }
    let low = smooth_counted_law(m1, log_period).cdf(TWIN_REFERENCE_MASS_RATIO);
    let twins = excess_twin_fraction(m1, log_period);
    above * (1.0 + (1.0 - twins) * low / (1.0 - low))
}

/// The primary masses, M☉, of [`PERIOD_CORRECTION`]'s rows: Moe and Di Stefano's mean masses of
/// the A/late-B, mid-B, early-B and O-type intervals.
pub(super) const CORRECTION_MASSES: [f64; 4] = [3.5, 7.0, 12.0, 28.0];

/// The periods, as x = log₁₀(P ÷ 1 d), of [`PERIOD_CORRECTION`]'s columns: the centres of the
/// bins 0.2–1, 1–2, …, 7–8.
pub(super) const CORRECTION_LOG_PERIODS: [f64; 8] = [0.6, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5];

/// The correction of the drawn period law of direct companions, by primary mass
/// ([`CORRECTION_MASSES`]) and period ([`CORRECTION_LOG_PERIODS`]): the drawn density is
/// `f_logP;q>0.1` times this factor, interpolated linearly in x between the columns and in ln M₁
/// between the rows, and held outside both.
///
/// Rejection of unstable sets thins sets with close periods, and so the periods that survive
/// are not the law drawn. The factors are solved so that they are: an iteration of
/// `c ← c × target ÷ measured` over the eight bins, with the survivors of rejection measured in
/// sets drawn by the hierarchy draw itself at each row's mass (the test
/// `fit_the_direct_period_correction`, ignored and slow, prints the table). Not tuned by hand.
///
/// The fit is at the Sun-like point, with each newest companion redrawn up to 42 times (ruling 81
/// as amended), and above 8 M☉ it absorbs the provisional stripped mark (P11.T2.c's seam, 0.25
/// with a 10 au periastron), which holds a quarter of those systems' innermost orbits inside
/// 10 au; when P11.T1.d replaces the seam the table is refitted. After twelve iterations every
/// bin's share after rejection is within 0.2% of its target. The targets are the bin shares of
/// Moe and Di Stefano's own eqs. 20–23 law, normalised; the absolute frequencies per decade then
/// follow from the count law.
pub(super) const PERIOD_CORRECTION: [[f64; 8]; 4] = [
    [
        0.780_872_618_560_769_2,
        0.966_048_399_154_477_8,
        1.013_897_455_043_749_2,
        1.090_300_449_722_920_3,
        1.170_775_225_837_900_3,
        1.030_600_264_255_199_2,
        1.003_434_895_938_529_3,
        0.834_563_137_282_256,
    ],
    [
        0.655_298_097_743_451_2,
        0.902_803_932_230_942_2,
        1.125_156_762_493_910_7,
        1.265_000_273_191_130_3,
        1.243_411_936_695_628_9,
        1.091_519_968_340_668_7,
        0.879_625_811_518_900_3,
        0.654_765_857_772_372_7,
    ],
    [
        0.591_462_501_890_336_8,
        0.856_576_371_499_201_2,
        1.130_812_678_850_639_4,
        1.190_129_829_779_486_2,
        1.415_713_172_418_716_2,
        1.220_910_780_413_867_2,
        0.890_155_151_078_511_2,
        0.649_055_812_364_610_1,
    ],
    [
        0.601_567_784_755_253_2,
        0.963_733_140_156_550_3,
        1.261_017_909_312_618_4,
        1.379_533_010_205_946,
        1.447_795_519_134_836,
        1.103_477_631_644_218_7,
        0.761_294_776_937_575_1,
        0.543_431_748_170_855_4,
    ],
];

/// A correction table's factor at `m1` and x = `log_period`.
#[must_use]
fn correction_at(table: &[[f64; 8]; 4], m1: SolarMasses, log_period: f64) -> f64 {
    let (i, t) = blend(&CORRECTION_MASSES, |&m| m, m1.value());
    let row = |r: &[f64; 8]| {
        let (j, u) = blend(&CORRECTION_LOG_PERIODS, |&x| x, log_period);
        lerp(r[j], r[j + 1], u)
    };
    lerp(row(&table[i]), row(&table[i + 1]), t)
}

/// The knots of the drawn period law: every 0.1 in x across 0.2–8, which holds every kink of eq.
/// 23 (1, 2, 3.4, 5.5).
const PERIOD_KNOTS: usize = 79;

/// The drawn period law of a direct companion of one primary: a density linear in x between
/// knots every 0.1 across [`DIRECT_LOG_PERIOD_RANGE`], through `f_logP;q>0.1` times the
/// correction at each knot.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DirectPeriods {
    knots: [(f64, f64); PERIOD_KNOTS],
    total: f64,
}

impl DirectPeriods {
    /// The law for a primary of `m1` under the correction `table`.
    #[must_use]
    pub(super) fn new(m1: SolarMasses, table: &[[f64; 8]; 4]) -> Self {
        let mut knots = [(0.0, 0.0); PERIOD_KNOTS];
        for (i, knot) in (0_u32..).zip(&mut knots) {
            let x = if i + 1 == 79 {
                DIRECT_LOG_PERIOD_RANGE.1
            } else {
                DIRECT_LOG_PERIOD_RANGE.0 + 0.1 * f64::from(i)
            };
            *knot = (
                x,
                frequency_above_a_tenth(m1, x) * correction_at(table, m1, x),
            );
        }
        let total = knots.windows(2).fold(0.0, |sum, pair| {
            let ((x0, f0), (x1, f1)) = (pair[0], pair[1]);
            sum + f0.midpoint(f1) * (x1 - x0)
        });
        Self { knots, total }
    }

    /// The x = log₁₀(P ÷ 1 d) below which a share `u` of the law lies: the segment holding the
    /// share, then the rationalised root of its quadratic cumulative distribution.
    #[must_use]
    pub(super) fn quantile(&self, u: f64) -> f64 {
        let target = u.clamp(0.0, 1.0) * self.total;
        let mut below = 0.0;
        let last = PERIOD_KNOTS - 2;
        for (i, pair) in self.knots.windows(2).enumerate() {
            let ((x0, f0), (x1, f1)) = (pair[0], pair[1]);
            let area = f0.midpoint(f1) * (x1 - x0);
            if target <= below + area || i == last {
                let v = if area > 0.0 {
                    ((target - below) / area).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let root = f0 + (f0 * f0 + v * (f1 * f1 - f0 * f0)).sqrt();
                let t = if root > 0.0 { v * (f0 + f1) / root } else { v };
                return x0 + t * (x1 - x0);
            }
            below += area;
        }
        DIRECT_LOG_PERIOD_RANGE.1
    }

    /// The share of the law in `[lo, hi]` of x.
    #[cfg(test)]
    #[must_use]
    pub(super) fn share(&self, lo: f64, hi: f64) -> f64 {
        let cdf = |x: f64| {
            let mut below = 0.0;
            for pair in self.knots.windows(2) {
                let ((x0, f0), (x1, f1)) = (pair[0], pair[1]);
                if x <= x1 {
                    let f = f0 + (f1 - f0) * (x - x0).max(0.0) / (x1 - x0);
                    return (below + f0.midpoint(f) * (x - x0).max(0.0)) / self.total;
                }
                below += f0.midpoint(f1) * (x1 - x0);
            }
            1.0
        };
        cdf(hi) - cdf(lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eqs. 20–22 at 2.7 M☉ are the knots that the A-star period anchor was built from, and eq.
    /// 23 is continuous at every join.
    #[test]
    fn moe_and_di_stefanos_period_law_is_their_equations() {
        let a = SolarMasses::new(2.7);
        assert!((frequency_above_three_tenths(a, 0.5) - 0.050_279_779_358_418_24).abs() < 1e-15);
        assert!((frequency_above_three_tenths(a, 2.7) - 0.071_1).abs() < 2e-4);
        for m in [1.0, 3.5, 7.0, 12.0, 28.0] {
            let m1 = SolarMasses::new(m);
            for x in [1.0, 2.0, 3.4, 5.5] {
                let (below, above) = (
                    frequency_above_three_tenths(m1, x - 1e-9),
                    frequency_above_three_tenths(m1, x),
                );
                assert!((below - above).abs() < 1e-8, "{m} M☉ at {x}");
            }
            assert!(frequency_above_three_tenths(m1, 8.1).abs() < 1e-300);
        }
    }

    /// Integrated over 0.2–8, `f_logP;q>0.1` gives Moe and Di Stefano's `f_mult;q>0.1` of Table 13
    /// within its 1σ at each interval, which their fits are built to do (§9.4).
    #[test]
    fn the_period_law_integrates_to_table_13s_frequencies() {
        for &(m, _, frequency) in &DIRECT_ANCHORS {
            let m1 = SolarMasses::new(m);
            let steps = 7_800_u32;
            let total = (0..steps).fold(0.0, |sum, i| {
                let x = 0.2 + (f64::from(i) + 0.5) * 7.8 / f64::from(steps);
                sum + frequency_above_a_tenth(m1, x) * 7.8 / f64::from(steps)
            });
            let sigma = [0.04, 0.11, 0.2, 0.2, 0.3][DIRECT_ANCHORS
                .iter()
                .position(|a| (a.0 - m).abs() < 1e-12)
                .expect("an anchor")];
            println!("{m} M☉: ∫ f_logP;q>0.1 = {total:.3} against f_mult {frequency}");
            assert!((total - frequency).abs() <= sigma, "{total} at {m} M☉");
        }
    }

    #[test]
    fn the_direct_count_reproduces_table_13() {
        for &(m, single, frequency) in &DIRECT_ANCHORS {
            let pmf = direct_count_pmf(SolarMasses::new(m));
            let mean = pmf[1] + 2.0 * pmf[2] + 3.0 * pmf[3];
            assert!((pmf[0] - single).abs() < 1e-15, "{pmf:?}");
            assert!((mean - frequency).abs() < 1e-12, "{pmf:?}");
            assert!((pmf.iter().sum::<f64>() - 1.0).abs() < 1e-15);
        }
    }

    #[test]
    fn the_blend_runs_from_none_at_one_and_a_half_to_all_at_three_solar_masses() {
        assert!(direct_weight(SolarMasses::new(1.5)).abs() < 1e-300);
        assert!((direct_weight(SolarMasses::new(3.0)) - 1.0).abs() < 1e-15);
        assert!((direct_weight(SolarMasses::new(2.121_320_343_559_642)) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn the_drawn_period_law_is_normalised_and_inverts() {
        let law = DirectPeriods::new(SolarMasses::new(12.0), &PERIOD_CORRECTION);
        assert!((law.share(0.2, 8.0) - 1.0).abs() < 1e-12);
        for i in 0..=20 {
            let u = f64::from(i) / 20.0;
            let x = law.quantile(u);
            assert!((law.share(0.2, x) - u).abs() < 1e-9, "{u} → {x}");
        }
    }
}
