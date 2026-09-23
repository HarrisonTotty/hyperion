//! The multiplicity model: the share of primaries with companions, how many they have, and the
//! anchors every distribution of [`dist`](super::dist) is interpolated between (plan 11,
//! P11.T1.a, Design note 2).

use super::dist::{
    MASS_RATIO_ANCHORS, MassRatioAnchor, O_STAR_CLOSE_FREQUENCY, O_STAR_WIDE_FREQUENCY,
    PERIOD_ANCHORS, PeriodAnchor,
};
use crate::math;
use crate::units::SolarMasses;

/// The most stellar companions a primary has: six stars in all.
///
/// The companion count of a multiple is capped here ([`MultiplicityModel::companion_count_pmf`]).
/// Duchêne and Kraus (2013, ARA&A 51, 269) trace the count of solar-type systems with n
/// components, `N(n) ∝ 2.5⁻ⁿ`, up to n = 6 (§3.1.5), and note that all eleven sextuples of the
/// Multiple Star Catalog are A stars (§5.1.1).
pub const MAX_COMPANIONS: usize = 5;

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

/// Duchêne and Kraus's companion frequency of B stars, 8–16 M☉, counting companions of q ≳ 0.1
/// only (§3.5.2): 100 ± 20%.
const B_STAR_SURVEYED_FREQUENCY: f64 = 1.0;

/// The share of the model's mass-ratio law at 11 M☉, marginalised over its periods, above
/// q = 0.1: 0.788. The literal is that share, which a test recomputes.
const B_STAR_SHARE_SURVEYED: f64 = 0.788_380_371_333_157_6;

/// The companions per B star of every mass ratio the model draws, down to 0.08 M☉ ÷ 11 M☉: the
/// surveyed 1.0 over the law's share above q = 0.1, 1.27.
const B_STAR_COMPANION_FREQUENCY: f64 = B_STAR_SURVEYED_FREQUENCY / B_STAR_SHARE_SURVEYED;

/// Duchêne and Kraus's (2013, ARA&A 51, 269) Table 1, "Multiplicity properties for Population I
/// MS stars and field BDs", one anchor per mass bin, re-checked against the paper:
///
/// | Bin (M☉)   | Anchor | MF (Table 1)            | CF (Table 1)            |
/// | ---------- | ------ | ----------------------- | ----------------------- |
/// | ≲ 0.1      | 0.09   | 22 (+6, −4)%            | 22 (+6, −4)%            |
/// | 0.1–0.5    | 0.25   | 26 ± 3%                 | 33 ± 5%                 |
/// | 0.7–1.3    | 1.0    | 44 ± 2%                 | 62 ± 3%                 |
/// | 1.5–5      | 2.7    | ≥ 50%                   | 100 ± 10%               |
/// | 8–16       | 11     | ≥ 60%                   | 100 ± 20% (q ≳ 0.1)     |
/// | ≳ 16       | 30     | ≥ 80%                   | 130 ± 20% (q ≳ 0.1)     |
///
/// The anchor masses are the plan's (Design note 2), near each bin's geometric centre. The three
/// massive fractions are the paper's lower limits, taken at their values. The solar-type pair is
/// from §3.1.2: the fraction is Raghavan et al.'s (2010, ApJS 190, 1, §5.2) 44 ± 2%, and the
/// frequency, 62 ± 3%, is Duquennoy and Mayor's (1991) and Raghavan et al.'s together; both
/// surveys count their brown-dwarf companions too, which the desert keeps few, and the model
/// counts all as stellar.
///
/// The B and O stars' frequencies count only companions of q ≳ 0.1, the surveys' limit, while the
/// model draws mass ratios down to 0.08 M☉ ÷ m₁ (Design note 3). Their anchors therefore hold the
/// measured count extended over the model's own mass-ratio law: 1.27 at 11 M☉
/// ([`B_STAR_COMPANION_FREQUENCY`]) and 1.63 at 30 M☉, the sum of
/// [`O_STAR_CLOSE_FREQUENCY`] and [`O_STAR_WIDE_FREQUENCY`]. Counted above q = 0.1 the model
/// gives Duchêne and Kraus's 1.0 and 1.3 again.
const FRACTION_ANCHORS: [FractionAnchor; 6] = [
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
        mass: 2.7,
        multiple_fraction: 0.50,
        companion_frequency: 1.0,
    },
    FractionAnchor {
        mass: 11.0,
        multiple_fraction: 0.60,
        companion_frequency: B_STAR_COMPANION_FREQUENCY,
    },
    FractionAnchor {
        mass: 30.0,
        multiple_fraction: 0.80,
        companion_frequency: O_STAR_CLOSE_FREQUENCY + O_STAR_WIDE_FREQUENCY,
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
/// It holds the anchors of Duchêne and Kraus (2013, ARA&A 51, 269, Table 1) and Raghavan et al.
/// (2010, ApJS 190, 1) for the fractions and counts here, and for the period, mass-ratio and
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
    /// The first model: Duchêne and Kraus's anchors as Design note 2 sets them, and the period,
    /// mass-ratio and eccentricity anchors of Design note 3.
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
    /// multiple is CF ÷ MF *after* the cap: it solves `r + r² + r³ + r⁴ = CF ÷ MF − 1` by a fixed
    /// number of Newton steps from the uncapped ratio 1 − MF ÷ CF. The mean of this distribution
    /// is therefore [`companion_frequency`](Self::companion_frequency) exactly, which is the
    /// measured quantity; the uncapped ratio would fall 4% short of it at 11 M☉. For Sun-like
    /// primaries the shares of single, double, triple and higher systems are 56 : 31 : 9 : 4,
    /// against Raghavan et al.'s (2010, §5.2) observed 56 : 33 : 8 : 3.
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
/// multiple, is `mean`: the root of `S(r) = r + r² + r³ + r⁴ = mean − 1` on [0, 1].
///
/// `S` is increasing and convex, and the start, the uncapped ratio `1 − 1 ÷ mean`, has
/// `S ≤ mean − 1`, so the first step lands at or above the root and the rest descend to it. A mean
/// of 1 gives 0, and every mean the anchors give lies within 1–2.2.
#[must_use]
fn capped_geometric_ratio(mean: f64) -> f64 {
    debug_assert!(
        (1.0..=5.0).contains(&mean),
        "a multiple's mean count {mean} must lie in 1–5"
    );
    let target = mean - 1.0;
    let mut r = 1.0 - 1.0 / mean;
    for _ in 0..COUNT_RATIO_STEPS {
        let s = r * (1.0 + r * (1.0 + r * (1.0 + r)));
        let slope = 1.0 + r * (2.0 + r * (3.0 + r * 4.0));
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
        assert_same_bits(model.multiple_fraction(light), 0.22);
        assert_same_bits(model.companion_frequency(light), 0.22);
        assert_same_bits(model.multiple_fraction(heavy), 0.80);
        let o_star = O_STAR_CLOSE_FREQUENCY + O_STAR_WIDE_FREQUENCY;
        assert_same_bits(model.companion_frequency(heavy), o_star);
        // Halfway in ln m between 0.25 and 1 M☉ is 0.5 M☉.
        let mid = SolarMasses::new(0.5);
        assert!((model.multiple_fraction(mid) - 0.35).abs() < 1e-15);
        assert!((model.companion_frequency(mid) - 0.475).abs() < 1e-15);
        for m in masses() {
            let (mf, cf) = (model.multiple_fraction(m), model.companion_frequency(m));
            assert!((0.22..=0.80).contains(&mf), "MF {mf} at {m:?}");
            assert!(mf <= cf && cf <= o_star, "CF {cf} against MF {mf} at {m:?}");
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
    /// after it where multiples are rich, by 4.1% at the most (at 11 M☉); the solved ratio closes
    /// the gap.
    #[test]
    fn the_cap_would_cost_the_uncapped_ratio_four_per_cent_at_the_most() {
        let model = model();
        let mut worst: f64 = 0.0;
        for m in masses() {
            let (mf, cf) = (model.multiple_fraction(m), model.companion_frequency(m));
            let r = 1.0 - mf / cf;
            let uncapped = 1.0 / (1.0 - r);
            assert!((uncapped - cf / mf).abs() < 1e-12);
            let capped = 1.0 + r + r * r + r * r * r + r * r * r * r;
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
        assert!((0.03..0.045).contains(&worst), "{worst}");
    }

    #[test]
    fn the_capped_ratio_solves_its_polynomial() {
        for i in 0_u32..=100 {
            let mean = 1.0 + f64::from(i) * 0.02;
            let r = capped_geometric_ratio(mean);
            let s = r + r * r + r * r * r + r * r * r * r;
            assert!((s - (mean - 1.0)).abs() < 1e-14, "S({r}) = {s} for {mean}");
        }
        assert_same_bits(capped_geometric_ratio(1.0), 0.0);
    }

    /// Design note 2's figure for Sun-like primaries, 56 : 31 : 9 : 4, against Raghavan et al.'s
    /// (2010) observed 56 : 33 : 8 : 3.
    #[test]
    fn sun_like_systems_split_as_the_plan_says() {
        let pmf = model().companion_count_pmf(SolarMasses::new(1.0));
        let higher = pmf[3] + pmf[4] + pmf[5];
        let shares = [pmf[0], pmf[1], pmf[2], higher].map(|p| 100.0 * p);
        println!("single : double : triple : higher = {shares:.1?} (Raghavan: 56 : 33 : 8 : 3)");
        for (share, expected) in shares.iter().zip([56.0, 31.0, 9.0, 4.0]) {
            assert!((share - expected).abs() < 0.5, "{shares:?}");
        }
        for (share, observed) in shares.iter().zip([56.0, 33.0, 8.0, 3.0]) {
            assert!(
                (share - observed).abs() < 2.0,
                "{shares:?} against Raghavan"
            );
        }
    }

    /// Counted as the surveys count them, above q = 0.1, the B and O stars' anchors give Duchêne
    /// and Kraus's frequencies again (§3.5.2), and the literals are the quotients they are
    /// documented as.
    #[test]
    fn the_massive_anchors_count_the_surveys_companions_above_a_tenth() {
        let model = model();
        for (m, surveyed) in [(11.0, 1.0), (30.0, 1.3)] {
            let m1 = SolarMasses::new(m);
            let above = 1.0 - model.companion_mass_ratio_cdf(m1, 0.1);
            let counted = model.companion_frequency(m1) * above;
            println!("{m} M☉: {counted:.6} companions of q ≥ 0.1 per star, surveyed {surveyed}");
            assert!((counted - surveyed).abs() < 1e-12, "{counted} at {m} M☉");
        }
        let b_star = 1.0 - model.companion_mass_ratio_cdf(SolarMasses::new(11.0), 0.1);
        assert_same_bits(b_star, B_STAR_SHARE_SURVEYED);
    }

    #[test]
    fn a_very_low_mass_multiple_is_a_binary() {
        let pmf = model().companion_count_pmf(SolarMasses::new(0.09));
        assert!((pmf[1] - 0.22).abs() < 1e-15, "{pmf:?}");
        assert!(pmf[2..].iter().all(|&p| p.abs() < 1e-15), "{pmf:?}");
    }
}
