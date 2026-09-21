//! The populations' age distributions at the epoch.
//!
//! An age is a signed number of Julian years at the epoch, the instant the fields describe: a
//! system of age `a` formed `a` years before it, and one of negative age forms `|a|` years after
//! it, during play. Distributions of still-forming populations therefore start at −H, the clock
//! window, so that stars are born in the window (brainstorm, "Time" and "Populations").
//!
//! Each distribution is a mixture of pieces on disjoint intervals, each piece uniform or an
//! exponential history, whose density rises as `exp(age ÷ τ)`: a formation rate that has declined
//! with timescale τ, so that more stars formed long ago (brainstorm, "Populations": the thin disc's
//! rate "declines exponentially with a timescale of 5–9 Gyr"). Every piece has a closed-form
//! density, distribution and inverse, so [`AgeDistribution::quantile`] is exact and a draw takes
//! one uniform.
//!
//! The ranges are those of the brainstorm's Populations table, with the shapes of plan 02's Design
//! note 13.

use std::error::Error;
use std::fmt;

use super::consts::{YEARS_PER_GIGAYEAR, YEARS_PER_MEGAYEAR};
use crate::math;
use crate::rng::Stream;
use crate::time::CLOCK_WINDOW_H;
use crate::units::Years;

/// The span of the thin disc's declining formation history, which began 10 Gyr ago.
pub const THIN_DISC_HISTORY: Years = Years::new(10.0 * YEARS_PER_GIGAYEAR);

/// The age that divides the young thin disc from the old: 100 Myr.
pub const YOUNG_AGE_LIMIT: Years = Years::new(100.0 * YEARS_PER_MEGAYEAR);

/// The age edges of the old thin disc's five sub-discs: 0.1, 1, 2, 4, 7 and 10 Gyr (plan 02,
/// Design note 9).
pub const SUB_DISC_AGE_EDGES: [Years; 6] = [
    YOUNG_AGE_LIMIT,
    Years::new(YEARS_PER_GIGAYEAR),
    Years::new(2.0 * YEARS_PER_GIGAYEAR),
    Years::new(4.0 * YEARS_PER_GIGAYEAR),
    Years::new(7.0 * YEARS_PER_GIGAYEAR),
    THIN_DISC_HISTORY,
];

/// The thick disc's ages, uniform on 10–12 Gyr.
pub const THICK_DISC_AGES: [Years; 2] = [
    Years::new(10.0 * YEARS_PER_GIGAYEAR),
    Years::new(12.0 * YEARS_PER_GIGAYEAR),
];

/// The bulge's ages, uniform on 8–12 Gyr.
pub const BULGE_AGES: [Years; 2] = [
    Years::new(8.0 * YEARS_PER_GIGAYEAR),
    Years::new(12.0 * YEARS_PER_GIGAYEAR),
];

/// The long bar's ages, uniform on 6–10 Gyr.
pub const LONG_BAR_AGES: [Years; 2] = [
    Years::new(6.0 * YEARS_PER_GIGAYEAR),
    Years::new(10.0 * YEARS_PER_GIGAYEAR),
];

/// The range inside which each halo component's narrow age range lies: 10–13 Gyr.
pub const HALO_AGES: [Years; 2] = [
    Years::new(10.0 * YEARS_PER_GIGAYEAR),
    Years::new(13.0 * YEARS_PER_GIGAYEAR),
];

/// The share of the systems of a 10 Gyr declining history that formed in the last 100 Myr.
///
/// With density `exp(age ÷ τ)` on 0–10 Gyr this is `expm1(0.1 Gyr ÷ τ) ÷ expm1(10 Gyr ÷ τ)`:
/// 0.32% at τ = 5 Gyr and 0.55% at 9 Gyr, the brainstorm's 0.3–0.6%. The young thin disc's share
/// of the galaxy is the thin disc's share times this (plan 02, Design note 3).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::ages::young_fraction;
/// use hyperion_sim::units::Years;
///
/// let share = young_fraction(Years::new(7e9));
/// assert!((0.004..0.005).contains(&share));
/// ```
#[must_use]
pub fn young_fraction(tau: Years) -> f64 {
    let tau = tau.value();
    math::exp_m1(YOUNG_AGE_LIMIT.value() / tau) / math::exp_m1(THIN_DISC_HISTORY.value() / tau)
}

/// One of the old thin disc's five sub-discs, by age (plan 02, Design note 9).
///
/// The old thin disc is a set of discs by age, as in the Besançon model, so that a system's age,
/// height and vertical speed are correlated (brainstorm, "Orbits and time").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubDisc {
    /// Ages 0.1–1 Gyr.
    First,
    /// Ages 1–2 Gyr.
    Second,
    /// Ages 2–4 Gyr.
    Third,
    /// Ages 4–7 Gyr.
    Fourth,
    /// Ages 7–10 Gyr.
    Fifth,
}

impl SubDisc {
    /// Every sub-disc, youngest first.
    pub const ALL: [Self; 5] = [
        Self::First,
        Self::Second,
        Self::Third,
        Self::Fourth,
        Self::Fifth,
    ];

    /// The sub-disc's place in [`SubDisc::ALL`], 0–4.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
            Self::Third => 2,
            Self::Fourth => 3,
            Self::Fifth => 4,
        }
    }

    /// The sub-disc's age range, from [`SUB_DISC_AGE_EDGES`].
    #[must_use]
    pub const fn age_range(self) -> [Years; 2] {
        [
            SUB_DISC_AGE_EDGES[self.index()],
            SUB_DISC_AGE_EDGES[self.index() + 1],
        ]
    }

    /// The share of the thin disc's systems in this sub-disc, for a declining history of
    /// timescale `tau` on 0–10 Gyr. The five shares sum to `1 − `[`young_fraction`]`(tau)`.
    #[must_use]
    pub fn share(self, tau: Years) -> f64 {
        let tau = tau.value();
        let [lo, hi] = self.age_range();
        let whole = math::exp_m1(THIN_DISC_HISTORY.value() / tau);
        // ∫ₗₒʰⁱ e^(a/τ) da ÷ ∫₀¹⁰ e^(a/τ) da, with the common factor τ cancelled.
        math::exp(lo.value() / tau) * math::exp_m1((hi.value() - lo.value()) / tau) / whole
    }
}

/// The share φ(age) of a young population's stars that still sit in the features that bore them,
/// and so are not field stars (brainstorm, "Populations": the young field's ages "carry the factor
/// 1 − φ").
///
/// Only [`FeatureShare::None`] exists until plan 09 builds the features; the young disc's
/// constructor already takes the argument so that plan 09 changes no caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureShare {
    /// φ = 0 at every age: every young star is a field star.
    None,
}

/// An [`AgeDistribution`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildAgeDistributionError {
    /// An age or a timescale is NaN or infinite.
    NonFinite,
    /// The upper age does not lie above the lower.
    EmptyRange,
    /// The timescale of an exponential history is zero or negative.
    TimescaleNotPositive,
}

impl fmt::Display for BuildAgeDistributionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "an age distribution's ages and timescale must be finite",
            Self::EmptyRange => "an age distribution's upper age must lie above its lower age",
            Self::TimescaleNotPositive => "a formation history's timescale must be positive",
        })
    }
}

impl Error for BuildAgeDistributionError {}

/// The shape of one piece.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    /// Constant density.
    Uniform,
    /// Density ∝ `exp(age ÷ tau)`, `tau` in years.
    Exponential { tau: f64 },
}

/// One piece of a distribution: a shape on `[lo, hi]` holding `weight` of the probability.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    lo: f64,
    hi: f64,
    weight: f64,
    shape: Shape,
}

impl Piece {
    fn new(lo: f64, hi: f64, weight: f64, shape: Shape) -> Result<Self, BuildAgeDistributionError> {
        if !(lo.is_finite() && hi.is_finite()) {
            return Err(BuildAgeDistributionError::NonFinite);
        }
        if hi <= lo {
            return Err(BuildAgeDistributionError::EmptyRange);
        }
        if let Shape::Exponential { tau } = shape {
            if !tau.is_finite() {
                return Err(BuildAgeDistributionError::NonFinite);
            }
            if tau <= 0.0 {
                return Err(BuildAgeDistributionError::TimescaleNotPositive);
            }
        }
        Ok(Self {
            lo,
            hi,
            weight,
            shape,
        })
    }

    /// `expm1((hi − lo) ÷ τ)`, the exponential piece's normalisation over `τ e^(lo/τ)`.
    fn span(&self, tau: f64) -> f64 {
        math::exp_m1((self.hi - self.lo) / tau)
    }

    /// The piece's own density at `x` inside it, integrating to 1 over the piece.
    fn pdf(&self, x: f64) -> f64 {
        match self.shape {
            Shape::Uniform => 1.0 / (self.hi - self.lo),
            Shape::Exponential { tau } => math::exp((x - self.lo) / tau) / (tau * self.span(tau)),
        }
    }

    /// The piece's own distribution at `x`, clamped to `[0, 1]`.
    fn cdf(&self, x: f64) -> f64 {
        if x <= self.lo {
            return 0.0;
        }
        if x >= self.hi {
            return 1.0;
        }
        let f = match self.shape {
            Shape::Uniform => (x - self.lo) / (self.hi - self.lo),
            Shape::Exponential { tau } => math::exp_m1((x - self.lo) / tau) / self.span(tau),
        };
        f.clamp(0.0, 1.0)
    }

    /// The piece's own inverse at `u` in `[0, 1]`, clamped to the piece.
    fn quantile(&self, u: f64) -> f64 {
        let x = match self.shape {
            Shape::Uniform => self.lo + u * (self.hi - self.lo),
            Shape::Exponential { tau } => self.lo + tau * math::ln_1p(u * self.span(tau)),
        };
        x.clamp(self.lo, self.hi)
    }

    /// The piece's own mean age.
    fn mean(&self) -> f64 {
        match self.shape {
            Shape::Uniform => f64::midpoint(self.lo, self.hi),
            // ∫ a e^(a/τ) ÷ ∫ e^(a/τ) on [lo, hi] = hi − τ + (hi − lo) ÷ expm1((hi − lo) ÷ τ).
            Shape::Exponential { tau } => self.hi - tau + (self.hi - self.lo) / self.span(tau),
        }
    }
}

/// The distribution of ages at the epoch of one population or component.
///
/// A mixture of pieces on disjoint, ordered intervals, each uniform or an exponential history,
/// with closed-form [`pdf`](Self::pdf), [`cdf`](Self::cdf) and [`quantile`](Self::quantile). A
/// still-forming distribution starts at −H, [`CLOCK_WINDOW_H`]; its
/// [`born_fraction`](Self::born_fraction) is the share with age ≥ 0 at the epoch, and the unborn
/// remainder, about 10⁻⁵ of the young disc, is extra: a population's density is normalised so that
/// its born systems equal its share (plan 02, Design note 13).
///
/// # Examples
///
/// A young-disc system's age, drawn with one uniform:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::ages::{AgeDistribution, FeatureShare};
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
/// use hyperion_sim::units::Years;
///
/// let young = AgeDistribution::young_disc(Years::new(7e9), FeatureShare::None)?;
/// let mut stream = Stream::open(Seed::new(1), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let age = young.sample(&mut stream);
/// assert!(age.value() >= -1_000.0 && age.value() <= 1e8);
/// assert_eq!(young.min(), Years::new(-1_000.0));
/// # Ok::<(), hyperion_sim::galaxy::ages::BuildAgeDistributionError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AgeDistribution {
    pieces: Vec<Piece>,
    /// Running sums of the weights, from 0 to 1, one more than there are pieces.
    cumulative: Vec<f64>,
}

impl AgeDistribution {
    /// A distribution from pieces on ordered, disjoint intervals whose weights sum to 1.
    fn from_pieces(pieces: Vec<Piece>) -> Self {
        debug_assert!(!pieces.is_empty());
        debug_assert!(pieces.windows(2).all(|w| w[0].hi <= w[1].lo));
        let mut cumulative = Vec::with_capacity(pieces.len() + 1);
        let mut sum = 0.0;
        cumulative.push(sum);
        for piece in &pieces {
            sum += piece.weight;
            cumulative.push(sum);
        }
        debug_assert!((sum - 1.0).abs() < 1e-12, "weights sum to {sum}");
        Self { pieces, cumulative }
    }

    /// Ages uniform on `[lo, hi]`.
    ///
    /// # Errors
    ///
    /// [`BuildAgeDistributionError::NonFinite`] for a NaN or infinite age, and
    /// [`BuildAgeDistributionError::EmptyRange`] unless `lo < hi`.
    pub fn uniform(lo: Years, hi: Years) -> Result<Self, BuildAgeDistributionError> {
        let piece = Piece::new(lo.value(), hi.value(), 1.0, Shape::Uniform)?;
        Ok(Self::from_pieces(vec![piece]))
    }

    /// Ages on `[lo, hi]` with density ∝ `exp(age ÷ tau)`: a formation rate declining with
    /// timescale `tau` since the time `hi` before the epoch.
    ///
    /// # Errors
    ///
    /// As [`uniform`](Self::uniform), and [`BuildAgeDistributionError::TimescaleNotPositive`]
    /// unless `tau > 0`.
    pub fn exponential_history(
        tau: Years,
        lo: Years,
        hi: Years,
    ) -> Result<Self, BuildAgeDistributionError> {
        let piece = Piece::new(
            lo.value(),
            hi.value(),
            1.0,
            Shape::Exponential { tau: tau.value() },
        )?;
        Ok(Self::from_pieces(vec![piece]))
    }

    /// One sub-disc of the old thin disc: the declining history of timescale `tau` inside the
    /// sub-disc's age range.
    ///
    /// # Errors
    ///
    /// As [`exponential_history`](Self::exponential_history).
    pub fn old_thin_disc(tau: Years, bin: SubDisc) -> Result<Self, BuildAgeDistributionError> {
        let [lo, hi] = bin.age_range();
        Self::exponential_history(tau, lo, hi)
    }

    /// The young thin disc: the same declining history continued from 100 Myr to −H, times
    /// `1 − φ(age)` for the stars still in features.
    ///
    /// # Errors
    ///
    /// As [`exponential_history`](Self::exponential_history).
    pub fn young_disc(
        tau: Years,
        feature_share: FeatureShare,
    ) -> Result<Self, BuildAgeDistributionError> {
        match feature_share {
            FeatureShare::None => Self::exponential_history(tau, -clock_window(), YOUNG_AGE_LIMIT),
        }
    }

    /// The nuclear disc: 90% uniform on 8–12 Gyr, 5% on 1–8 Gyr and 5% from −H to 1 Gyr.
    ///
    /// Nogueras-Lara et al. (2020, Nature Astronomy 4, 377) find that some 80% or more of the
    /// nuclear disc's stars formed over 8 Gyr ago, followed by a quiet phase, then about 5% of its
    /// mass in a burst around 1 Gyr ago and activity continuing in the last tens of Myr. The
    /// weights are plan 02's (Design note 13): the old share within "80% or more", the quiet phase
    /// as a low uniform rate, and the last gigayear's 5% spread uniformly to the present so that
    /// the disc still forms stars, as the brainstorm's "a few per cent formed in the last
    /// gigayear" asks.
    #[must_use]
    pub fn nuclear_disc() -> Self {
        let gyr = YEARS_PER_GIGAYEAR;
        let uniform = |lo: f64, hi: f64, weight: f64| Piece {
            lo,
            hi,
            weight,
            shape: Shape::Uniform,
        };
        Self::from_pieces(vec![
            uniform(-clock_window().value(), gyr, 0.05),
            uniform(gyr, 8.0 * gyr, 0.05),
            uniform(8.0 * gyr, 12.0 * gyr, 0.90),
        ])
    }

    /// The youngest age, which is −H for a still-forming distribution.
    #[must_use]
    pub fn min(&self) -> Years {
        Years::new(self.pieces[0].lo)
    }

    /// The oldest age.
    #[must_use]
    pub fn max(&self) -> Years {
        Years::new(self.pieces[self.pieces.len() - 1].hi)
    }

    /// The probability density at `age`, per year; 0 outside the pieces.
    #[must_use]
    pub fn pdf(&self, age: Years) -> f64 {
        let x = age.value();
        self.pieces
            .iter()
            .filter(|p| (p.lo..=p.hi).contains(&x))
            .map(|p| p.weight * p.pdf(x))
            .next()
            .unwrap_or(0.0)
    }

    /// The probability that a system is younger than `age`.
    #[must_use]
    pub fn cdf(&self, age: Years) -> f64 {
        let x = age.value();
        // The last piece starting at or below x; pieces wholly below it contribute their weight.
        let i = self.pieces.partition_point(|p| p.lo <= x);
        if i == 0 {
            return 0.0;
        }
        let piece = &self.pieces[i - 1];
        (self.cumulative[i - 1] + piece.weight * piece.cdf(x)).clamp(0.0, 1.0)
    }

    /// The age below which a fraction `u` of systems lie, for `u` in `[0, 1]`: the inverse of
    /// [`cdf`](Self::cdf), in closed form.
    #[must_use]
    pub fn quantile(&self, u: f64) -> Years {
        let u = u.clamp(0.0, 1.0);
        // The piece whose share of the running sum holds u, skipping pieces of zero weight.
        let last = self.pieces.len() - 1;
        let i = self.cumulative[1..=last]
            .partition_point(|&c| c <= u)
            .min(last);
        let piece = &self.pieces[i];
        let local = if piece.weight > 0.0 {
            ((u - self.cumulative[i]) / piece.weight).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Years::new(piece.quantile(local))
    }

    /// An age drawn from the distribution: [`quantile`](Self::quantile) of one uniform from
    /// `stream`. One word.
    pub fn sample(&self, stream: &mut Stream) -> Years {
        self.quantile(stream.uniform())
    }

    /// The share of systems with age ≥ 0 at the epoch: 1 unless the distribution runs to −H.
    #[must_use]
    pub fn born_fraction(&self) -> f64 {
        1.0 - self.cdf(Years::ZERO)
    }

    /// The probability that a system already born at the epoch is younger than `age`: the
    /// distribution restricted to age ≥ 0. It is 0 at `age ≤ 0`.
    ///
    /// # Panics
    ///
    /// If no system of the distribution is born, which no constructor here produces.
    #[must_use]
    pub fn born_cdf(&self, age: Years) -> f64 {
        let unborn = self.cdf(Years::ZERO);
        let born = 1.0 - unborn;
        assert!(born > 0.0, "an age distribution with no born system");
        if age.value() <= 0.0 {
            return 0.0;
        }
        ((self.cdf(age) - unborn) / born).clamp(0.0, 1.0)
    }

    /// The mean age.
    #[must_use]
    pub fn mean(&self) -> Years {
        Years::new(
            self.pieces
                .iter()
                .fold(0.0, |sum, p| sum + p.weight * p.mean()),
        )
    }

    /// The ages where the density has a kink or a jump: every piece's ends, ascending, without
    /// repeats.
    pub(crate) fn edges(&self) -> Vec<f64> {
        let mut edges = Vec::with_capacity(2 * self.pieces.len());
        for piece in &self.pieces {
            if edges.last() != Some(&piece.lo) {
                edges.push(piece.lo);
            }
            edges.push(piece.hi);
        }
        edges
    }
}

/// H, the clock window, in Julian years: exactly 1,000.
fn clock_window() -> Years {
    Years::new(CLOCK_WINDOW_H.as_julian_years_f64())
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

    use super::*;
    use crate::Seed;
    use crate::galaxy::quad::gl_panels;
    use crate::rng::{ObjectKey, tags};

    const GYR: f64 = YEARS_PER_GIGAYEAR;

    fn gyr(x: f64) -> Years {
        Years::new(x * GYR)
    }

    fn examples() -> Vec<(&'static str, AgeDistribution)> {
        let mut all = vec![
            (
                "young",
                AgeDistribution::young_disc(gyr(7.0), FeatureShare::None).unwrap(),
            ),
            ("nuclear", AgeDistribution::nuclear_disc()),
            (
                "thick",
                AgeDistribution::uniform(THICK_DISC_AGES[0], THICK_DISC_AGES[1]).unwrap(),
            ),
            (
                "history",
                AgeDistribution::exponential_history(gyr(5.0), Years::ZERO, THIN_DISC_HISTORY)
                    .unwrap(),
            ),
        ];
        for bin in SubDisc::ALL {
            all.push((
                "sub-disc",
                AgeDistribution::old_thin_disc(gyr(9.0), bin).unwrap(),
            ));
        }
        all
    }

    #[test]
    fn young_fraction_matches_the_brainstorm() {
        let at_5 = 100.0 * young_fraction(gyr(5.0));
        let at_9 = 100.0 * young_fraction(gyr(9.0));
        assert!((at_5 - 0.32).abs() <= 0.005, "{at_5}% at 5 Gyr");
        assert!((at_9 - 0.55).abs() <= 0.005, "{at_9}% at 9 Gyr");
    }

    #[test]
    fn sub_disc_shares_sum_to_the_old_disc() {
        for tau in [5.0, 7.0, 9.0] {
            let sum: f64 = SubDisc::ALL.iter().map(|b| b.share(gyr(tau))).sum();
            let expected = 1.0 - young_fraction(gyr(tau));
            assert!((sum - expected).abs() < 1e-14, "τ = {tau}: {sum}");
            // Older bins hold more systems per gigayear: the rate has declined.
            let per_gyr: Vec<f64> = SubDisc::ALL
                .iter()
                .map(|b| {
                    let [lo, hi] = b.age_range();
                    b.share(gyr(tau)) / ((hi.value() - lo.value()) / GYR)
                })
                .collect();
            assert!(per_gyr.windows(2).all(|w| w[0] < w[1]));
        }
    }

    #[test]
    fn quantile_inverts_cdf() {
        for (name, ages) in examples() {
            for i in 0..=1_000 {
                let u = f64::from(i) / 1_000.0;
                let age = ages.quantile(u);
                assert!(age >= ages.min() && age <= ages.max(), "{name}: {age:?}");
                let back = ages.cdf(age);
                assert!(
                    (back - u).abs() < 1e-12,
                    "{name}: u = {u} came back as {back}"
                );
            }
            assert_eq!(ages.quantile(0.0), ages.min(), "{name}");
            assert_eq!(ages.quantile(1.0), ages.max(), "{name}");
        }
    }

    /// The density integrates to 1 and to the distribution, and gives the closed-form mean.
    #[test]
    fn pdf_cdf_and_mean_agree_with_quadrature() {
        for (name, ages) in examples() {
            let edges = ages.edges();
            let total = gl_panels(|a| ages.pdf(Years::new(a)), &edges);
            assert!((total - 1.0).abs() < 1e-12, "{name}: ∫pdf = {total}");
            let mean = gl_panels(|a| a * ages.pdf(Years::new(a)), &edges);
            let scale = ages.max().value();
            assert!(
                ((mean - ages.mean().value()) / scale).abs() < 1e-12,
                "{name}: mean {mean} against {:?}",
                ages.mean()
            );
            let middle = f64::midpoint(edges[0], edges[1]);
            let partial = gl_panels(|a| ages.pdf(Years::new(a)), &[edges[0], middle]);
            assert!(
                (partial - ages.cdf(Years::new(middle))).abs() < 1e-12,
                "{name}"
            );
        }
    }

    #[test]
    fn the_young_disc_starts_exactly_at_minus_h() {
        let young = AgeDistribution::young_disc(gyr(6.0), FeatureShare::None).unwrap();
        assert_eq!(young.min(), Years::new(-1_000.0));
        assert_same_bits(young.min().value(), -CLOCK_WINDOW_H.as_julian_years_f64());
        assert_eq!(young.max(), YOUNG_AGE_LIMIT);
        assert_eq!(AgeDistribution::nuclear_disc().min(), Years::new(-1_000.0));
    }

    /// The unborn sliver: 10⁶ draws, of which H ÷ 100 Myr, about 10, are negative.
    #[test]
    fn a_young_sample_has_the_expected_unborn_share() {
        let young = AgeDistribution::young_disc(gyr(7.0), FeatureShare::None).unwrap();
        let unborn = 1.0 - young.born_fraction();
        let naive = clock_window().value() / YOUNG_AGE_LIMIT.value();
        assert!(
            ((unborn - naive) / naive).abs() < 0.02,
            "{unborn} against {naive}"
        );
        let mut stream = Stream::open(
            Seed::new(0x00a9_e500_0000_0001),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        );
        let n = 1_000_000_u32;
        let negative = (0..n)
            .filter(|_| young.sample(&mut stream).value() < 0.0)
            .count();
        assert_eq!(stream.position(), u64::from(n), "one word per draw");
        assert_poisson_count(
            "unborn young-disc systems",
            u64::try_from(negative).unwrap(),
            f64::from(n) * unborn,
            ALPHA,
        );
    }

    #[test]
    fn born_fractions_and_born_cdf() {
        let thick = AgeDistribution::uniform(THICK_DISC_AGES[0], THICK_DISC_AGES[1]).unwrap();
        assert_same_bits(thick.born_fraction(), 1.0);
        assert_same_bits(thick.born_cdf(gyr(11.0)), thick.cdf(gyr(11.0)));
        let nuclear = AgeDistribution::nuclear_disc();
        let unborn = 0.05 * 1_000.0 / (GYR + 1_000.0);
        assert!((1.0 - nuclear.born_fraction() - unborn).abs() < 1e-15);
        assert!((nuclear.cdf(gyr(1.0)) - 0.05).abs() < 1e-15);
        assert!((nuclear.cdf(gyr(8.0)) - 0.10).abs() < 1e-15);
        assert_same_bits(nuclear.born_cdf(Years::new(-10.0)), 0.0);
        assert_same_bits(nuclear.born_cdf(gyr(12.0)), 1.0);
        let at_one = nuclear.born_cdf(gyr(1.0));
        assert!((at_one - (0.05 - unborn) / (1.0 - unborn)).abs() < 1e-15);
    }

    #[test]
    fn edges_are_the_piece_ends() {
        let nuclear = AgeDistribution::nuclear_disc().edges();
        assert_eq!(nuclear, vec![-1_000.0, GYR, 8.0 * GYR, 12.0 * GYR]);
        let gap = AgeDistribution::from_pieces(vec![
            Piece::new(0.0, 1.0, 0.5, Shape::Uniform).unwrap(),
            Piece::new(2.0, 3.0, 0.5, Shape::Uniform).unwrap(),
        ]);
        assert_eq!(gap.edges(), vec![0.0, 1.0, 2.0, 3.0]);
        assert_same_bits(gap.cdf(Years::new(1.5)), 0.5);
        assert_same_bits(gap.pdf(Years::new(1.5)), 0.0);
        // A flat stretch of the distribution inverts to the start of the next piece.
        assert_same_bits(gap.quantile(0.5).value(), 2.0);
    }

    #[test]
    fn exponential_history_means_lean_old() {
        let history =
            AgeDistribution::exponential_history(gyr(5.0), Years::ZERO, THIN_DISC_HISTORY).unwrap();
        assert!(history.mean() > gyr(5.0));
        let flat_limit =
            AgeDistribution::exponential_history(gyr(1e6), Years::ZERO, THIN_DISC_HISTORY).unwrap();
        assert!((flat_limit.mean().value() / GYR - 5.0).abs() < 1e-5);
    }

    #[test]
    fn bad_arguments_are_rejected_with_their_variant() {
        use BuildAgeDistributionError as E;
        assert_eq!(
            AgeDistribution::uniform(gyr(2.0), gyr(1.0)),
            Err(E::EmptyRange)
        );
        assert_eq!(
            AgeDistribution::uniform(gyr(1.0), gyr(1.0)),
            Err(E::EmptyRange)
        );
        assert_eq!(
            AgeDistribution::uniform(Years::new(f64::NAN), gyr(1.0)),
            Err(E::NonFinite)
        );
        assert_eq!(
            AgeDistribution::exponential_history(Years::ZERO, Years::ZERO, gyr(1.0)),
            Err(E::TimescaleNotPositive)
        );
        assert_eq!(
            AgeDistribution::young_disc(Years::new(f64::INFINITY), FeatureShare::None),
            Err(E::NonFinite)
        );
    }
}
