//! The multiplicity model: the share of primaries with companions, how many they have, and the
//! anchors every distribution of [`dist`](super::dist) is interpolated between (plan 11,
//! P11.T1.a, Design note 2).

use super::dist::{MASS_RATIO_ANCHORS, MassRatioAnchor, PERIOD_ANCHORS, PeriodAnchor};
use crate::math;
use crate::units::SolarMasses;

/// The most stellar companions a primary has, direct or in subsystems: four stars in all (rulings
/// 74 and 81). From 3 M☉ up these are Moe and Di Stefano's direct companions, of which Table 13
/// counts up to three, and subsystems come on top only below the cap: an O star with three
/// direct companions (about 38%) holds no subsystem, one with two may take one. Real sextuples
/// with early-B primaries exist (ν Sco, AR Cas; Offner et al. 2023, §2.1), and no survey
/// measures the subsystems of massive stars' companions (Sana et al. 2014, §4.3; Moe and Di
/// Stefano 2017, §11): a game-side cap, revisited if those statistics are measured.
///
/// The companion count of a multiple is capped here ([`MultiplicityModel::companion_count_pmf`]),
/// as Moe and Di Stefano's (2017, ApJS 230, 15, §9.4) own model is: their multiplicity fractions
/// run over n = 0–3 companions, "a Poisson distribution truncated to the interval n = [0, 3]",
/// and they find O-type primaries "almost exclusively in binaries, triples, and quadruples". So
/// the model makes no quintuples or sextuples. Tokovinin (2014, AJ 147, 87, §4.3) puts 1.3% of
/// solar-type systems at five stars or more.
pub const MAX_COMPANIONS: usize = 3;

/// Newton steps that solve for the ratio of the capped geometric count; from the uncapped ratio
/// the error is squared at each step, six leave it at the rounding of the polynomial, and two
/// more are margin.
const COUNT_RATIO_STEPS: u32 = 8;

/// One anchor of the multiplicity fractions: a primary mass and the multiple-system fraction and
/// companion frequency measured there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FractionAnchor {
    /// The primary mass, M☉.
    mass: f64,
    /// The share of primaries with at least one companion, MF.
    multiple_fraction: f64,
    /// The mean number of companions per primary, CF.
    companion_frequency: f64,
}

/// Moe and Di Stefano's (2017, ApJS 230, 15) Table 13, from 2 M☉ up, counted as they count:
/// companions of q > 0.1 that directly orbit the primary with log₁₀(P ÷ 1 d) < 8, re-checked
/// against the paper. Each row is the interval's mean primary mass as they evaluate their fits
/// (§9.1: 3.5, 7, 12 and 28 M☉), the total multiplicity frequency `f_mult;q>0.1` and the single
/// star fraction `F_n=0;q>0.1`.
///
/// | Interval          | M₁ (M☉) | `f_mult;q>0.1` | `F_n=0;q>0.1` |
/// | ----------------- | ------- | -------------- | ------------- |
/// | A/late-B, 2–5 M☉  | 3.5     | 0.84 ± 0.11  | 0.41 ± 0.08 |
/// | mid-B, 5–9 M☉     | 7       | 1.3 ± 0.2    | 0.24 ± 0.08 |
/// | early-B, 9–16 M☉  | 12      | 1.6 ± 0.2    | 0.16 ± 0.09 |
/// | O-type, > 16 M☉   | 28      | 2.1 ± 0.3    | 0.06 ± 0.06 |
///
/// Offner et al. (2023, PPVII, Table 1) give the same compilation by slightly different mass
/// intervals: companion frequencies 1.28, 1.55, 1.8 and 2.1 and triple-or-higher fractions 36, 45,
/// 57 and 68 ± 18% for 3–5, 5–8, 8–17 and 17–50 M☉.
#[cfg(test)]
pub(super) const COUNTED_ANCHORS: [(f64, f64, f64); 4] = [
    (3.5, 0.84, 0.41),
    (7.0, 1.3, 0.24),
    (12.0, 1.6, 0.16),
    (28.0, 2.1, 0.06),
];

/// Duchêne and Kraus's (2013, ARA&A 51, 269) Table 1 below 2 M☉, and Moe and Di Stefano's
/// (2017, Table 13) counts from 2 M☉ up (ruling 74), one anchor per mass bin:
///
/// | Anchor (M☉) | MF      | CF      | Source                                               |
/// | ----------- | ------- | ------- | ---------------------------------------------------- |
/// | 0.09        | 0.22    | 0.22    | Duchêne and Kraus, ≲ 0.1 M☉: 22 (+6, −4)% both       |
/// | 0.25        | 0.26    | 0.33    | Duchêne and Kraus, 0.1–0.5 M☉: 26 ± 3%, 33 ± 5%      |
/// | 1.0         | 0.44    | 0.62    | Duchêne and Kraus, 0.7–1.3 M☉: 44 ± 2%, 62 ± 3%      |
/// | 3.5         | 0.688   | 1.070   | Moe and Di Stefano, 2–5 M☉ (`COUNTED_ANCHORS`)       |
/// | 7           | 0.874   | 1.861   | Moe and Di Stefano, 5–9 M☉                           |
/// | 12          | 0.919   | 2.275   | Moe and Di Stefano, 9–16 M☉                          |
/// | 28          | 0.961   | 2.884   | Moe and Di Stefano, > 16 M☉                          |
///
/// The solar-type pair is from Duchêne and Kraus's §3.1.2: the fraction is Raghavan et al.'s
/// (2010, ApJS 190, 1, §5.2) 44 ± 2%, and the frequency, 62 ± 3%, is Duquennoy and Mayor's (1991)
/// and Raghavan et al.'s together; both surveys count their brown-dwarf companions too, which the
/// desert keeps few, and the model counts all as stellar.
///
/// Duchêne and Kraus's figures for more massive stars (≥ 50%, ≥ 60% and ≥ 80%) are lower limits
/// and were once taken as values; Moe and Di Stefano's replace them. They count only companions
/// of q > 0.1 and log P < 8 that orbit the primary directly, while the model draws mass ratios
/// down to 0.08 M☉ ÷ m₁ (with the flat extension below q = 0.1 of ruling 43.2) and periods to its
/// laws' ends. So each anchor holds the counts extended over the model's own laws: with s the
/// share of the model's companions that they would count
/// (`MultiplicityModel::counted_share`, 0.785, 0.698, 0.703 and 0.718), each
/// companion is counted with probability s, and MF and CF are solved so that the counted
/// frequency and single fraction are Table 13's under [`MultiplicityModel::companion_count_pmf`]:
/// `MF (1 + r + r²) s = f_mult` and `1 − MF + MF [(1 − r)(1 − s) + (1 − r) r (1 − s)² + r² (1 −
/// s)³] = F_n=0`. At 28 M☉ the cap of [`MAX_COMPANIONS`] binds: no r ≤ 1 meets both, so every
/// multiple has three companions (r = 1) and MF meets the single fraction, which leaves the
/// counted frequency 2.07 against 2.1 ± 0.3 and the total under 2.9. The literals are those
/// solutions, which a test recomputes. The count ignores that a companion joining a companion
/// (plan 11's `L12` subsystems) is not one that orbits the primary directly.
const FRACTION_ANCHORS: [FractionAnchor; 7] = [
    FractionAnchor {
        mass: 0.09,
        multiple_fraction: 0.22,
        companion_frequency: 0.22,
    },
    FractionAnchor {
        mass: 0.25,
        multiple_fraction: 0.26,
        companion_frequency: 0.33,
    },
    FractionAnchor {
        mass: 1.0,
        multiple_fraction: 0.44,
        companion_frequency: 0.62,
    },
    FractionAnchor {
        mass: 3.5,
        multiple_fraction: 0.687_561_502_699_405_7,
        companion_frequency: 1.069_543_910_073_016_3,
    },
    FractionAnchor {
        mass: 7.0,
        multiple_fraction: 0.874_217_936_527_944,
        companion_frequency: 1.861_383_464_971_614,
    },
    FractionAnchor {
        mass: 12.0,
        multiple_fraction: 0.918_983_782_939_445,
        companion_frequency: 2.275_250_264_301_449_4,
    },
    FractionAnchor {
        mass: 28.0,
        multiple_fraction: 0.961_491_369_765_714_5,
        companion_frequency: 2.884_474_109_297_143_4,
    },
];

/// Where a primary mass falls among ascending anchor masses: the index `i` of the anchor at or
/// below it and the weight `t` of anchor `i + 1`, linear in ln m. Below the first anchor `t` is 0
/// at the first; above the last it is 1 at the last, so a quantity is constant outside.
///
/// A value interpolated as `(1 − t) vᵢ + t vᵢ₊₁` is exactly `vᵢ` at anchor `i`'s mass.
///
/// A NaN mass is taken as below the first anchor.
#[must_use]
pub(super) fn blend<T>(anchors: &[T], mass_of: impl Fn(&T) -> f64, m: f64) -> (usize, f64) {
    debug_assert!(anchors.len() >= 2, "interpolation needs two anchors");
    let last = anchors.len() - 1;
    if m.is_nan() || m <= mass_of(&anchors[0]) {
        return (0, 0.0);
    }
    if m >= mass_of(&anchors[last]) {
        return (last - 1, 1.0);
    }
    let i = (0..last)
        .find(|&i| m < mass_of(&anchors[i + 1]))
        .expect("m lies strictly between the first and last anchors, so a segment holds it");
    let (lo, hi) = (mass_of(&anchors[i]), mass_of(&anchors[i + 1]));
    (i, math::ln(m / lo) / math::ln(hi / lo))
}

/// `(1 − t) a + t b`: exactly `a` at `t = 0` and `b` at `t = 1`.
#[must_use]
pub(super) fn lerp(a: f64, b: f64, t: f64) -> f64 {
    (1.0 - t) * a + t * b
}

/// The multiplicity of stars by primary mass: one model that every companion draw and every
/// quadrature over companions reads (plan 11, Design note 1).
///
/// It holds the anchors of Duchêne and Kraus (2013, ARA&A 51, 269, Table 1), Raghavan et al.
/// (2010, ApJS 190, 1) and Moe and Di Stefano (2017, ApJS 230, 15, Table 13) for the fractions and
/// counts here, and for the period, mass-ratio and
/// eccentricity distributions ([`PeriodDistribution`](super::PeriodDistribution),
/// [`MassRatioDistribution`](super::MassRatioDistribution) and
/// [`EccentricityDistribution`](super::EccentricityDistribution)). Every quantity is interpolated
/// linearly in ln m between its anchors and held constant outside them. "Companion" means a
/// stellar companion, of at least [`MIN_COMPANION_MASS`](super::MIN_COMPANION_MASS): brown-dwarf
/// companions are drawn apart and counted in no quadrature (Design note 15).
///
/// Once P11.T1.d and P11.T2.c wire it in, the model is part of the generator version, and
/// [`default_v1`](Self::default_v1) is the only one. Until then nothing generated reads it.
///
/// # Examples
///
/// Nearly half of Sun-like stars and most O stars have companions, a quarter of M dwarfs:
///
/// ```
/// use hyperion_sim::stellar::multiplicity::MultiplicityModel;
/// use hyperion_sim::units::SolarMasses;
///
/// let model = MultiplicityModel::default_v1();
/// assert_eq!(model.multiple_fraction(SolarMasses::new(1.0)), 0.44);
/// assert!(model.multiple_fraction(SolarMasses::new(0.25)) < 0.3);
/// assert!(model.multiple_fraction(SolarMasses::new(40.0)) >= 0.8);
/// // The mean companion count of a Sun-like multiple is frequency ÷ fraction.
/// let pmf = model.companion_count_pmf(SolarMasses::new(1.0));
/// let mean: f64 = pmf.iter().zip(0_u32..).map(|(p, n)| f64::from(n) * p).sum();
/// assert!((mean - 0.62).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiplicityModel {
    fractions: &'static [FractionAnchor],
    periods: &'static [PeriodAnchor],
    mass_ratios: &'static [MassRatioAnchor],
}

impl MultiplicityModel {
    /// The first model: Duchêne and Kraus's anchors as Design note 2 sets them below 2 M☉ and
    /// Moe and Di Stefano's counts above (ruling 74), and the period, mass-ratio and eccentricity
    /// anchors of Design note 3.
    #[must_use]
    pub const fn default_v1() -> Self {
        Self {
            fractions: &FRACTION_ANCHORS,
            periods: &PERIOD_ANCHORS,
            mass_ratios: &MASS_RATIO_ANCHORS,
        }
    }

    /// The period anchors, for [`dist`](super::dist).
    #[must_use]
    pub(super) fn period_anchors(&self) -> &'static [PeriodAnchor] {
        self.periods
    }

    /// The mass-ratio anchors, for [`dist`](super::dist).
    #[must_use]
    pub(super) fn mass_ratio_anchors(&self) -> &'static [MassRatioAnchor] {
        self.mass_ratios
    }

    /// Every anchor mass of the fractions, periods and mass ratios, M☉, ascending and without
    /// repeats: where a quantity of the model has a kink, for quadratures' panel edges.
    #[must_use]
    pub(super) fn anchor_masses(&self) -> Vec<f64> {
        let mut masses: Vec<f64> = self
            .fractions
            .iter()
            .map(|a| a.mass)
            .chain(self.periods.iter().map(PeriodAnchor::mass))
            .chain(self.mass_ratios.iter().map(MassRatioAnchor::mass))
            .collect();
        masses.sort_by(f64::total_cmp);
        masses.dedup();
        masses
    }

    /// The share of primaries of initial mass `m1` that have at least one stellar companion, MF,
    /// in [0, 1].
    #[must_use]
    pub fn multiple_fraction(&self, m1: SolarMasses) -> f64 {
        let (i, t) = blend(self.fractions, |a| a.mass, m1.value());
        lerp(
            self.fractions[i].multiple_fraction,
            self.fractions[i + 1].multiple_fraction,
            t,
        )
    }

    /// The share of primaries of initial mass `m1` that the hierarchy draw makes multiple: the
    /// spine construction's [`multiple_fraction`](Self::multiple_fraction) and Moe and Di
    /// Stefano's direct one, `1 − F_n=0` of Table 13, mixed with the direct construction's weight
    /// (0 up to 1.5 M☉, 1 from 3 M☉, linear in ln M₁ between; ruling 81), in [0, 1].
    #[must_use]
    pub fn drawn_multiple_fraction(&self, m1: SolarMasses) -> f64 {
        let w = super::direct::direct_weight(m1);
        lerp(
            self.multiple_fraction(m1),
            super::direct::direct_multiple_fraction(m1),
            w,
        )
    }

    /// The mean number of stellar companions of a primary of initial mass `m1`, CF, counting the
    /// single ones as zero; at least [`multiple_fraction`](Self::multiple_fraction).
    #[must_use]
    pub fn companion_frequency(&self, m1: SolarMasses) -> f64 {
        let (i, t) = blend(self.fractions, |a| a.mass, m1.value());
        lerp(
            self.fractions[i].companion_frequency,
            self.fractions[i + 1].companion_frequency,
            t,
        )
    }

    /// The distribution of the number of stellar companions of a primary of initial mass `m1`:
    /// entry n is the probability of exactly n, for 0 to [`MAX_COMPANIONS`]; the entries sum to 1.
    ///
    /// A single has probability 1 − MF. A multiple has 1 + G companions, where G is geometric,
    /// `P(G = k) = (1 − r) rᵏ`, capped at [`MAX_COMPANIONS`] − 1 so that the tail's weight goes
    /// to the largest count (Design note 2). The ratio r is chosen so that the mean count of a
    /// multiple is CF ÷ MF *after* the cap: it solves `r + r² = CF ÷ MF − 1` by a fixed number
    /// of Newton steps from the uncapped ratio 1 − MF ÷ CF. The mean of this distribution is
    /// therefore [`companion_frequency`](Self::companion_frequency) exactly, which is the measured
    /// quantity. For Sun-like primaries the shares of single, double, triple and quadruple systems
    /// are 56 : 30 : 9 : 4, against Raghavan et al.'s (2010, §5.2) observed 56 ± 2 : 33 ± 2 :
    /// 8 ± 1 : 3 ± 1 and Tokovinin's (2014, §4.3) 4.3% quadruples. Where the cap binds, a mean of
    /// three companions per multiple, every multiple has three (the O stars, ruling 74).
    #[must_use]
    pub fn companion_count_pmf(&self, m1: SolarMasses) -> [f64; MAX_COMPANIONS + 1] {
        let fraction = self.multiple_fraction(m1);
        let r = capped_geometric_ratio(self.companion_frequency(m1) / fraction);
        let mut pmf = [0.0; MAX_COMPANIONS + 1];
        pmf[0] = 1.0 - fraction;
        let mut power = 1.0;
        for p in &mut pmf[1..MAX_COMPANIONS] {
            *p = fraction * (1.0 - r) * power;
            power *= r;
        }
        pmf[MAX_COMPANIONS] = fraction * power;
        pmf
    }
}

impl Default for MultiplicityModel {
    fn default() -> Self {
        Self::default_v1()
    }
}

/// The ratio r of a geometric count capped at [`MAX_COMPANIONS`] companions whose mean, for a
/// multiple, is `mean`: the root of `S(r) = r + r² = mean − 1` on [0, 1].
///
/// `S` is increasing and convex, and the start, the uncapped ratio `1 − 1 ÷ mean`, has
/// `S ≤ mean − 1`, so the first step lands at or above the root and the rest descend to it. A mean
/// of 1 gives 0 and a mean of three, the cap, gives 1: every multiple then has three companions.
#[must_use]
pub(super) fn capped_geometric_ratio(mean: f64) -> f64 {
    const _: () = assert!(MAX_COMPANIONS == 3, "S(r) is written for a cap of three");
    debug_assert!(
        (1.0..=3.0 + 1e-12).contains(&mean),
        "a multiple's mean count {mean} must lie in 1–3"
    );
    let target = mean - 1.0;
    let mut r = 1.0 - 1.0 / mean;
    for _ in 0..COUNT_RATIO_STEPS {
        let s = r * (1.0 + r);
        let slope = 1.0 + 2.0 * r;
        r -= (s - target) / slope;
    }
    r.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn model() -> MultiplicityModel {
        MultiplicityModel::default_v1()
    }

    fn mean_count(pmf: &[f64; MAX_COMPANIONS + 1]) -> f64 {
        (0_u32..)
            .zip(pmf)
            .fold(0.0, |sum, (n, p)| sum + f64::from(n) * p)
    }

    /// A sweep of primary masses over the stellar range, 0.08–150 M☉, and a little beyond.
    fn masses() -> impl Iterator<Item = SolarMasses> {
        (0_u32..=200).map(|i| SolarMasses::new(0.05 * math::exp(f64::from(i) * 0.04)))
    }

    #[test]
    fn the_anchors_are_reproduced_exactly() {
        let model = model();
        for a in FRACTION_ANCHORS {
            let m = SolarMasses::new(a.mass);
            assert_same_bits(model.multiple_fraction(m), a.multiple_fraction);
            assert_same_bits(model.companion_frequency(m), a.companion_frequency);
        }
    }

    #[test]
    fn fractions_are_constant_outside_the_anchors_and_between_them_in_between() {
        let model = model();
        let light = SolarMasses::new(0.08);
        let heavy = SolarMasses::new(150.0);
        let o_star = FRACTION_ANCHORS[6];
        assert_same_bits(model.multiple_fraction(light), 0.22);
        assert_same_bits(model.companion_frequency(light), 0.22);
        assert_same_bits(model.multiple_fraction(heavy), o_star.multiple_fraction);
        assert_same_bits(model.companion_frequency(heavy), o_star.companion_frequency);
        // Halfway in ln m between 0.25 and 1 M☉ is 0.5 M☉.
        let mid = SolarMasses::new(0.5);
        assert!((model.multiple_fraction(mid) - 0.35).abs() < 1e-15);
        assert!((model.companion_frequency(mid) - 0.475).abs() < 1e-15);
        for m in masses() {
            let (mf, cf) = (model.multiple_fraction(m), model.companion_frequency(m));
            assert!(
                (0.22..=o_star.multiple_fraction).contains(&mf),
                "MF {mf} at {m:?}"
            );
            assert!(
                mf <= cf && cf <= 3.0 * mf * (1.0 + 1e-15),
                "CF {cf} against MF {mf} at {m:?}"
            );
        }
    }

    #[test]
    fn the_count_distribution_sums_to_one_and_its_mean_is_the_companion_frequency() {
        let model = model();
        for m in masses() {
            let pmf = model.companion_count_pmf(m);
            let total = pmf.iter().fold(0.0, |sum, p| sum + p);
            assert!((total - 1.0).abs() < 1e-15, "sum {total} at {m:?}");
            assert!(pmf.iter().all(|&p| (0.0..=1.0).contains(&p)), "{pmf:?}");
            let (mf, cf) = (model.multiple_fraction(m), model.companion_frequency(m));
            assert!((pmf[0] - (1.0 - mf)).abs() < 1e-15);
            let mean = mean_count(&pmf);
            assert!(
                (mean - cf).abs() < 1e-12,
                "mean {mean} against CF {cf} at {m:?}"
            );
            // The mean count of a multiple is frequency ÷ fraction, to 1% (the plan's bracket)
            // and in fact to rounding.
            let multiple_mean = mean / mf;
            assert!(
                (multiple_mean / (cf / mf) - 1.0).abs() < 0.01,
                "a multiple's mean {multiple_mean} at {m:?}"
            );
        }
    }

    /// The plan's ratio, 1 − MF ÷ CF, gives the frequency before the cap exactly and falls short
    /// after it where multiples are rich; the solved ratio closes the gap, and is never below it.
    #[test]
    fn the_solved_ratio_closes_the_uncapped_ratios_shortfall() {
        let model = model();
        let mut worst: f64 = 0.0;
        for m in masses() {
            let (mf, cf) = (model.multiple_fraction(m), model.companion_frequency(m));
            let r = 1.0 - mf / cf;
            let uncapped = 1.0 / (1.0 - r);
            assert!((uncapped - cf / mf).abs() < 1e-12);
            let capped = 1.0 + r + r * r;
            worst = worst.max(1.0 - capped / uncapped);
            let solved = capped_geometric_ratio(cf / mf);
            assert!(
                solved >= r - 1e-15,
                "the solved ratio {solved} is below {r}"
            );
        }
        println!(
            "the uncapped ratio's worst shortfall: {:.2}%",
            100.0 * worst
        );
        assert!(worst > 0.0, "{worst}");
    }

    #[test]
    fn the_capped_ratio_solves_its_polynomial() {
        for i in 0_u32..=100 {
            let mean = 1.0 + f64::from(i) * 0.02;
            let r = capped_geometric_ratio(mean);
            let s = r + r * r;
            assert!((s - (mean - 1.0)).abs() < 1e-14, "S({r}) = {s} for {mean}");
        }
        assert_same_bits(capped_geometric_ratio(1.0), 0.0);
        assert_same_bits(capped_geometric_ratio(3.0), 1.0);
    }

    /// Sun-like primaries split 56 : 30 : 9 : 4 into single, double, triple and quadruple systems,
    /// within 2σ of Raghavan et al.'s (2010, §5.2) observed 56 ± 2 : 33 ± 2 : 8 ± 1 : 3 ± 1, and
    /// their quadruples are Tokovinin's (2014, §4.3) 4.3%.
    #[test]
    fn sun_like_systems_split_as_the_surveys_find() {
        let pmf = model().companion_count_pmf(SolarMasses::new(1.0));
        let shares = pmf.map(|p| 100.0 * p);
        println!(
            "single : double : triple : quadruple = {shares:.1?} (Raghavan: 56 : 33 : 8 : 3; \
             Tokovinin 4.3% quadruples)"
        );
        for (share, expected) in shares.iter().zip([56.0, 30.3, 9.4, 4.3]) {
            assert!((share - expected).abs() < 0.05, "{shares:?}");
        }
        for (share, (observed, sigma)) in
            shares
                .iter()
                .zip([(56.0, 2.0), (33.0, 2.0), (8.0, 1.0), (3.0, 1.0)])
        {
            assert!(
                (share - observed).abs() < 2.0 * sigma,
                "{shares:?} against Raghavan"
            );
        }
        assert!(
            (shares[3] - 4.3).abs() < 0.05,
            "{shares:?} against Tokovinin"
        );
    }

    /// Counted as Moe and Di Stefano count them, each companion with the model's share s of
    /// q > 0.1 and log P < 8, the massive anchors give their Table 13's frequency and single
    /// fraction again, except at 28 M☉, where the cap binds: every multiple there has three
    /// companions, the single fraction is met and the frequency is within its 1σ.
    #[test]
    fn the_massive_anchors_count_moe_and_di_stefanos_companions() {
        let model = model();
        for (anchor, (m, frequency, single)) in FRACTION_ANCHORS[3..].iter().zip(COUNTED_ANCHORS) {
            assert_same_bits(anchor.mass, m);
            let m1 = SolarMasses::new(m);
            let s = model.counted_share(m1);
            let pmf = model.companion_count_pmf(m1);
            let counted = s * mean_count(&pmf);
            let none = (0_i32..)
                .zip(pmf)
                .fold(0.0, |sum, (n, p)| sum + p * math::powi(1.0 - s, n));
            println!(
                "{m} M☉: share counted {s:.4}; counted frequency {counted:.4} (Table 13 \
                 {frequency}), single {none:.4} ({single}); pmf {pmf:.4?}"
            );
            assert!((none - single).abs() < 1e-12, "{none} at {m} M☉");
            if m < 20.0 {
                assert!((counted - frequency).abs() < 1e-12, "{counted} at {m} M☉");
            } else {
                assert_same_bits(pmf[1], 0.0);
                assert!(
                    counted < frequency && frequency - counted < 0.3,
                    "{counted}"
                );
                assert!(model.companion_frequency(m1) < 2.9);
            }
        }
    }

    #[test]
    fn a_very_low_mass_multiple_is_a_binary() {
        let pmf = model().companion_count_pmf(SolarMasses::new(0.09));
        assert!((pmf[1] - 0.22).abs() < 1e-15, "{pmf:?}");
        assert!(pmf[2..].iter().all(|&p| p.abs() < 1e-15), "{pmf:?}");
    }
}
