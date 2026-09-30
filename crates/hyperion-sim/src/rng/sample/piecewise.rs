//! Piecewise samplers: a piecewise power law and a piecewise linear density.
//!
//! The piecewise power law carries the mass functions (Kroupa's three segments, Chabrier's
//! power-law tail). The piecewise linear density carries tabulated profiles: age distributions
//! and anything fitted offline. Both draw in two words: the first picks a segment by the integer
//! threshold rule, the second places the value inside it.
//!
//! The segment pick is the first word's [`Mark`](crate::rng::Mark) against [`Thresholds`] built
//! once from the segment integrals, with their total as the bound: the thresholds are
//! `ceil(Sᵢ ÷ S × 2⁵³)`, where `Sᵢ` are the running sums of the integrals in index order and `S`
//! their total, and the first segment whose threshold lies above the mark is taken. That is the
//! integer-threshold convention of the determinism plan's Design note 7, so it gives exactly the
//! answer of `uniform < Sᵢ ÷ S`; the last threshold is 2⁵³, above every mark, so a pick never
//! fails, and a segment of zero integral is never picked.

use std::error::Error;
use std::fmt;

use super::power_law::{BuildPowerLawError, PowerLaw, power_integral};
use crate::math;
use crate::rng::{Stream, Thresholds};

/// The segment the next word picks.
///
/// # Panics
///
/// Never: the last threshold is that of `S ÷ S = 1`, which is 2⁵³ and above every mark.
fn pick_segment(thresholds: &Thresholds, stream: &mut Stream) -> usize {
    stream
        .pick(thresholds)
        .expect("the last threshold is 2^53, above every 53-bit mark")
}

/// The running sums `[0, m₀, m₀ + m₁, …]` of segment integrals, in index order.
fn running_sums(masses: impl IntoIterator<Item = f64>) -> Vec<f64> {
    let mut sums = vec![0.0];
    let mut sum = 0.0;
    for mass in masses {
        sum += mass;
        sums.push(sum);
    }
    sums
}

/// The segment holding `x`: the last `i` with `breaks[i] ≤ x`, at most the last segment.
fn segment_of(breaks: &[f64], x: f64) -> usize {
    let segments = breaks.len() - 1;
    breaks[1..segments].partition_point(|&b| b <= x)
}

/// A piecewise density could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildPiecewiseError {
    /// Fewer than two breaks or knots: no segment.
    TooFewBreaks {
        /// How many were given.
        count: usize,
    },
    /// A list of exponents, coefficients or densities has the wrong length.
    LengthMismatch {
        /// The length the breaks or knots imply.
        expected: usize,
        /// The length given.
        found: usize,
    },
    /// A break, knot, exponent, coefficient or density is NaN or infinite.
    NonFinite,
    /// Break or knot `index` does not lie above the one before it.
    NotIncreasing {
        /// The offending position.
        index: usize,
    },
    /// A power law's first break is zero or negative.
    NonPositiveBreak,
    /// Coefficient or density `index` is negative.
    NegativeDensity {
        /// The offending position.
        index: usize,
    },
    /// Every segment has zero integral.
    ZeroTotal,
    /// An integral overflows `f64`.
    NotNormalisable,
    /// A truncation's range misses the support, or is empty.
    EmptyRange,
}

impl fmt::Display for BuildPiecewiseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewBreaks { count } => {
                write!(f, "a piecewise density needs two breaks, got {count}")
            }
            Self::LengthMismatch { expected, found } => {
                write!(
                    f,
                    "expected {expected} values per segment list, got {found}"
                )
            }
            Self::NonFinite => f.write_str("a piecewise density's values must be finite"),
            Self::NotIncreasing { index } => {
                write!(f, "break {index} does not lie above the one before it")
            }
            Self::NonPositiveBreak => f.write_str("a piecewise power law must start above 0"),
            Self::NegativeDensity { index } => write!(f, "density {index} is negative"),
            Self::ZeroTotal => f.write_str("a piecewise density must not be zero everywhere"),
            Self::NotNormalisable => f.write_str("a piecewise density's integral overflows"),
            Self::EmptyRange => f.write_str("the truncation range misses the density's support"),
        }
    }
}

impl Error for BuildPiecewiseError {}

/// Checks that `breaks` has at least two entries, all finite and strictly increasing.
fn check_breaks(breaks: &[f64]) -> Result<(), BuildPiecewiseError> {
    if breaks.len() < 2 {
        return Err(BuildPiecewiseError::TooFewBreaks {
            count: breaks.len(),
        });
    }
    if !breaks.iter().all(|b| b.is_finite()) {
        return Err(BuildPiecewiseError::NonFinite);
    }
    match (1..breaks.len()).find(|&i| breaks[i] <= breaks[i - 1]) {
        Some(index) => Err(BuildPiecewiseError::NotIncreasing { index }),
        None => Ok(()),
    }
}

/// Checks a list's length and that its values are finite.
fn check_values(values: &[f64], expected: usize) -> Result<(), BuildPiecewiseError> {
    if values.len() != expected {
        return Err(BuildPiecewiseError::LengthMismatch {
            expected,
            found: values.len(),
        });
    }
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(BuildPiecewiseError::NonFinite)
    }
}

/// Checks that densities are not negative.
fn check_non_negative(values: &[f64]) -> Result<(), BuildPiecewiseError> {
    match values.iter().position(|&v| v < 0.0) {
        Some(index) => Err(BuildPiecewiseError::NegativeDensity { index }),
        None => Ok(()),
    }
}

/// Checks that a total is positive and finite.
fn check_total(total: f64) -> Result<(), BuildPiecewiseError> {
    if !total.is_finite() {
        Err(BuildPiecewiseError::NotNormalisable)
    } else if total > 0.0 {
        Ok(())
    } else {
        Err(BuildPiecewiseError::ZeroTotal)
    }
}

/// One segment of a piecewise power law: `coefficient × x^−α` on the law's range.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PowerSegment {
    law: PowerLaw,
    coefficient: f64,
}

impl PowerSegment {
    /// `∫ₐᵇ coefficient × x^−α dx`, for `a ≤ b` inside the segment.
    fn integral(&self, a: f64, b: f64) -> f64 {
        self.coefficient * power_integral(self.law.exponent(), a, b)
    }
}

/// A piecewise power law: `cᵢ x^−αᵢ` on `[bᵢ, bᵢ₊₁)` for breaks `b₀ < … < b_m`.
///
/// The coefficients are chained for continuity ([`continuous`](Self::continuous)) or given
/// ([`with_coefficients`](Self::with_coefficients), which a scaled branch such as Chabrier's
/// high-mass tail needs). The density is not normalised: [`integral`](Self::integral) and
/// [`moment`](Self::moment) integrate it as given, and [`pdf`](Self::pdf) and [`cdf`](Self::cdf)
/// normalise over the support `[b₀, b_m]`. [`sample`](Self::sample) draws in two words.
///
/// # Examples
///
/// Kroupa's (2001) initial mass function, α = 0.3, 1.3 and 2.3 with breaks at 0.08 and 0.5 M☉,
/// from 0.01 to 150 M☉; the share of stellar systems in layer A's band:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::rng::{ObjectKey, PiecewisePowerLaw, Stream, tags};
///
/// let kroupa = PiecewisePowerLaw::continuous(&[0.01, 0.08, 0.5, 150.0], &[0.3, 1.3, 2.3])?;
/// let share = kroupa.integral(0.08, 0.5) / kroupa.integral(0.08, 150.0);
/// assert!((share - 0.76).abs() < 0.005);
///
/// let mut stream = Stream::open(Seed::new(9), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let mass = kroupa.truncated(0.08, 0.5)?.sample(&mut stream);
/// assert!((0.08..=0.5).contains(&mass));
/// # Ok::<(), hyperion_sim::rng::BuildPiecewiseError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PiecewisePowerLaw {
    segments: Vec<PowerSegment>,
    /// The breaks, `b₀ … b_m`.
    breaks: Vec<f64>,
    /// Running sums of the segment integrals, from 0 to the total.
    cumulative: Vec<f64>,
    /// The segment-pick thresholds, one per segment.
    thresholds: Thresholds,
}

impl PiecewisePowerLaw {
    /// A continuous piecewise power law: `c₀ = 1` and each `cᵢ₊₁ = cᵢ × bᵢ₊₁^(αᵢ₊₁ − αᵢ)`, so
    /// that the density meets itself at every break.
    ///
    /// # Errors
    ///
    /// As [`with_coefficients`](Self::with_coefficients).
    pub fn continuous(breaks: &[f64], exponents: &[f64]) -> Result<Self, BuildPiecewiseError> {
        check_breaks(breaks)?;
        check_values(exponents, breaks.len() - 1)?;
        let mut coefficients = Vec::with_capacity(exponents.len());
        let mut coefficient = 1.0;
        for (i, &exponent) in exponents.iter().enumerate() {
            if i > 0 {
                coefficient *= math::powf(breaks[i], exponent - exponents[i - 1]);
            }
            coefficients.push(coefficient);
        }
        Self::with_coefficients(breaks, exponents, &coefficients)
    }

    /// A piecewise power law with the given coefficients: `cᵢ x^−αᵢ` on `[bᵢ, bᵢ₊₁)`.
    ///
    /// # Errors
    ///
    /// - [`BuildPiecewiseError::TooFewBreaks`] for fewer than two breaks.
    /// - [`BuildPiecewiseError::LengthMismatch`] unless there is one exponent and one coefficient
    ///   per segment.
    /// - [`BuildPiecewiseError::NonFinite`] for a NaN or infinite value.
    /// - [`BuildPiecewiseError::NotIncreasing`] for breaks not strictly increasing.
    /// - [`BuildPiecewiseError::NonPositiveBreak`] if the first break is not positive.
    /// - [`BuildPiecewiseError::NegativeDensity`] for a negative coefficient.
    /// - [`BuildPiecewiseError::ZeroTotal`] if every coefficient is 0.
    /// - [`BuildPiecewiseError::NotNormalisable`] if an integral overflows.
    pub fn with_coefficients(
        breaks: &[f64],
        exponents: &[f64],
        coefficients: &[f64],
    ) -> Result<Self, BuildPiecewiseError> {
        check_breaks(breaks)?;
        let count = breaks.len() - 1;
        check_values(exponents, count)?;
        check_values(coefficients, count)?;
        if breaks[0] <= 0.0 {
            return Err(BuildPiecewiseError::NonPositiveBreak);
        }
        check_non_negative(coefficients)?;
        let segments = (0..count)
            .map(|i| {
                let law =
                    PowerLaw::new(exponents[i], breaks[i], breaks[i + 1]).map_err(|e| match e {
                        BuildPowerLawError::EmptyRange => {
                            BuildPiecewiseError::NotIncreasing { index: i + 1 }
                        }
                        BuildPowerLawError::NotNormalisable => BuildPiecewiseError::NotNormalisable,
                        BuildPowerLawError::NonFinite => BuildPiecewiseError::NonFinite,
                        BuildPowerLawError::LowerLimitNotPositive => {
                            BuildPiecewiseError::NonPositiveBreak
                        }
                    })?;
                Ok(PowerSegment {
                    law,
                    coefficient: coefficients[i],
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let masses: Vec<f64> = segments
            .iter()
            .map(|s| s.coefficient * s.law.integral())
            .collect();
        let cumulative = running_sums(masses.iter().copied());
        let total = cumulative[count];
        check_total(total)?;
        Ok(Self {
            thresholds: Thresholds::from_weights(&masses, total),
            segments,
            breaks: breaks.to_vec(),
            cumulative,
        })
    }

    /// The first break, where the support begins.
    #[must_use]
    pub fn lo(&self) -> f64 {
        self.breaks[0]
    }

    /// The last break, where the support ends.
    #[must_use]
    pub fn hi(&self) -> f64 {
        self.breaks[self.breaks.len() - 1]
    }

    /// The law restricted to `[lo, hi]` intersected with its support, with the same exponents and
    /// coefficients: the density of a mass band.
    ///
    /// # Errors
    ///
    /// [`BuildPiecewiseError::EmptyRange`] if the range does not overlap the support or is empty,
    /// [`BuildPiecewiseError::NonFinite`] if a limit is NaN, and otherwise as
    /// [`with_coefficients`](Self::with_coefficients) (a band whose segments all have zero
    /// coefficients is [`ZeroTotal`](BuildPiecewiseError::ZeroTotal)).
    pub fn truncated(&self, lo: f64, hi: f64) -> Result<Self, BuildPiecewiseError> {
        if lo.is_nan() || hi.is_nan() {
            return Err(BuildPiecewiseError::NonFinite);
        }
        let lo = lo.max(self.lo());
        let hi = hi.min(self.hi());
        if lo >= hi {
            return Err(BuildPiecewiseError::EmptyRange);
        }
        let mut breaks = vec![lo];
        let mut exponents = Vec::new();
        let mut coefficients = Vec::new();
        for (i, segment) in self.segments.iter().enumerate() {
            let (start, end) = (self.breaks[i], self.breaks[i + 1]);
            if end <= lo || start >= hi {
                continue;
            }
            breaks.push(end.min(hi));
            exponents.push(segment.law.exponent());
            coefficients.push(segment.coefficient);
        }
        Self::with_coefficients(&breaks, &exponents, &coefficients)
    }

    /// `∫ density dx` over `[lo, hi]` intersected with the support; 0 if they do not overlap.
    #[must_use]
    pub fn integral(&self, lo: f64, hi: f64) -> f64 {
        self.moment(lo, hi, 0.0)
    }

    /// `∫ x^p × density dx` over `[lo, hi]` intersected with the support: `p = 1` gives the mass
    /// in a band when the density counts stars by mass.
    ///
    /// Each segment's integrand is `cᵢ x^(p − αᵢ)`, which takes the logarithmic form where
    /// `αᵢ − p` is within 10⁻⁸ of 1.
    #[must_use]
    pub fn moment(&self, lo: f64, hi: f64, p: f64) -> f64 {
        let mut sum = 0.0;
        for (i, segment) in self.segments.iter().enumerate() {
            let a = lo.max(self.breaks[i]);
            let b = hi.min(self.breaks[i + 1]);
            if a < b {
                sum += segment.coefficient * power_integral(segment.law.exponent() - p, a, b);
            }
        }
        sum
    }

    /// The density at `x` normalised over the support; 0 outside it.
    #[must_use]
    pub fn pdf(&self, x: f64) -> f64 {
        if !(self.lo()..=self.hi()).contains(&x) {
            return 0.0;
        }
        let segment = &self.segments[segment_of(&self.breaks, x)];
        segment.coefficient * math::powf(x, -segment.law.exponent()) / self.total()
    }

    /// The cumulative distribution at `x` over the support.
    #[must_use]
    pub fn cdf(&self, x: f64) -> f64 {
        if x <= self.lo() {
            return 0.0;
        }
        if x >= self.hi() {
            return 1.0;
        }
        let i = segment_of(&self.breaks, x);
        let partial = self.segments[i].integral(self.breaks[i], x);
        ((self.cumulative[i] + partial) / self.total()).clamp(0.0, 1.0)
    }

    /// A draw: the first word picks a segment by its share of the total, the second draws inside
    /// it as [`Stream::power_law`] does. Two words.
    pub fn sample(&self, stream: &mut Stream) -> f64 {
        let segment = &self.segments[pick_segment(&self.thresholds, stream)];
        stream.power_law(&segment.law)
    }

    /// The integral over the whole support.
    fn total(&self) -> f64 {
        self.cumulative[self.cumulative.len() - 1]
    }
}

/// A piecewise linear density on knots `x₀ < … < x_m` with densities `f₀ … f_m ≥ 0`.
///
/// Linear between knots and zero outside `[x₀, x_m]`. It carries tabulated distributions, such as
/// ages and anything fitted offline. [`sample`](Self::sample) draws in two words.
///
/// # Examples
///
/// A star-formation history that rises and then declines, sampled for an age in Gyr:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::rng::{ObjectKey, PiecewiseLinear, Stream, tags};
///
/// let history = PiecewiseLinear::new(&[0.0, 2.0, 13.0], &[0.2, 1.0, 0.0])?;
/// let mut stream = Stream::open(Seed::new(4), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let age = history.sample(&mut stream);
/// assert!((0.0..=13.0).contains(&age));
/// assert!((history.cdf(2.0) - 1.2 / 6.7).abs() < 1e-12);
/// # Ok::<(), hyperion_sim::rng::BuildPiecewiseError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PiecewiseLinear {
    knots: Vec<f64>,
    densities: Vec<f64>,
    /// Running sums of the segment areas, from 0 to the total.
    cumulative: Vec<f64>,
    /// The segment-pick thresholds, one per segment.
    thresholds: Thresholds,
}

impl PiecewiseLinear {
    /// A density that is `densities[i]` at `knots[i]` and linear between.
    ///
    /// # Errors
    ///
    /// - [`BuildPiecewiseError::TooFewBreaks`] for fewer than two knots.
    /// - [`BuildPiecewiseError::LengthMismatch`] unless there is one density per knot.
    /// - [`BuildPiecewiseError::NonFinite`] for a NaN or infinite value.
    /// - [`BuildPiecewiseError::NotIncreasing`] for knots not strictly increasing.
    /// - [`BuildPiecewiseError::NegativeDensity`] for a negative density.
    /// - [`BuildPiecewiseError::ZeroTotal`] if every density is 0.
    /// - [`BuildPiecewiseError::NotNormalisable`] if the area overflows.
    pub fn new(knots: &[f64], densities: &[f64]) -> Result<Self, BuildPiecewiseError> {
        check_breaks(knots)?;
        check_values(densities, knots.len())?;
        check_non_negative(densities)?;
        let areas: Vec<f64> = knots
            .windows(2)
            .zip(densities.windows(2))
            .map(|(x, f)| trapezoid(f[0], f[1], x[1] - x[0]))
            .collect();
        let cumulative = running_sums(areas.iter().copied());
        let total = cumulative[cumulative.len() - 1];
        check_total(total)?;
        Ok(Self {
            thresholds: Thresholds::from_weights(&areas, total),
            knots: knots.to_vec(),
            densities: densities.to_vec(),
            cumulative,
        })
    }

    /// The first knot, where the support begins.
    #[must_use]
    pub fn lo(&self) -> f64 {
        self.knots[0]
    }

    /// The last knot, where the support ends.
    #[must_use]
    pub fn hi(&self) -> f64 {
        self.knots[self.knots.len() - 1]
    }

    /// The unnormalised density at `x`, which lies in segment `i`.
    fn density_in(&self, i: usize, x: f64) -> f64 {
        let (x0, x1) = (self.knots[i], self.knots[i + 1]);
        let (f0, f1) = (self.densities[i], self.densities[i + 1]);
        f0 + (f1 - f0) * ((x - x0) / (x1 - x0))
    }

    /// `∫ density dx` over `[lo, hi]` intersected with the support; 0 if they do not overlap.
    #[must_use]
    pub fn integral(&self, lo: f64, hi: f64) -> f64 {
        let mut sum = 0.0;
        for i in 0..self.knots.len() - 1 {
            let a = lo.max(self.knots[i]);
            let b = hi.min(self.knots[i + 1]);
            if a < b {
                sum += trapezoid(self.density_in(i, a), self.density_in(i, b), b - a);
            }
        }
        sum
    }

    /// The density at `x` normalised over the support; 0 outside it.
    #[must_use]
    pub fn pdf(&self, x: f64) -> f64 {
        if !(self.lo()..=self.hi()).contains(&x) {
            return 0.0;
        }
        self.density_in(segment_of(&self.knots, x), x) / self.total()
    }

    /// The cumulative distribution at `x` over the support.
    #[must_use]
    pub fn cdf(&self, x: f64) -> f64 {
        if x <= self.lo() {
            return 0.0;
        }
        if x >= self.hi() {
            return 1.0;
        }
        let i = segment_of(&self.knots, x);
        let x0 = self.knots[i];
        let partial = trapezoid(self.densities[i], self.density_in(i, x), x - x0);
        ((self.cumulative[i] + partial) / self.total()).clamp(0.0, 1.0)
    }

    /// A draw: the first word picks a segment by its share of the total area, the second places
    /// the value inside it. Two words.
    ///
    /// Inside a segment from `(x₀, f₀)` to `(x₁, f₁)` the uniform `u` of the second word, from
    /// [`Stream::uniform_open`], gives `t = u (f₀ + f₁) ÷ (f₀ + √(f₀² + u (f₁² − f₀²)))` and
    /// `x = x₀ + t (x₁ − x₀)`: the inverse of the segment's quadratic cumulative distribution,
    /// rationalised so that it needs no division by `f₁ − f₀`. The open uniform keeps `0 ÷ 0` out
    /// where `f₀ = 0`.
    pub fn sample(&self, stream: &mut Stream) -> f64 {
        let i = pick_segment(&self.thresholds, stream);
        let u = stream.uniform_open();
        linear_inverse(
            [self.knots[i], self.knots[i + 1]],
            [self.densities[i], self.densities[i + 1]],
            u,
        )
    }

    /// The area under the whole density.
    fn total(&self) -> f64 {
        self.cumulative[self.cumulative.len() - 1]
    }
}

/// The point of a linear segment from `(x₀, f₀)` to `(x₁, f₁)` below which a share `u` of its
/// area lies: `x₀ + t (x₁ − x₀)` with `t = u (f₀ + f₁) ÷ (f₀ + √(f₀² + u (f₁² − f₀²)))`, clamped
/// to the segment. `u` must lie in `(0, 1)` and `f₀ + f₁` be positive.
fn linear_inverse([x0, x1]: [f64; 2], [f0, f1]: [f64; 2], u: f64) -> f64 {
    let t = u * (f0 + f1) / (f0 + (f0 * f0 + u * (f1 * f1 - f0 * f0)).sqrt());
    (x0 + t * (x1 - x0)).clamp(x0, x1)
}

/// The area of a trapezoid of heights `f0` and `f1` and width `width`: `½ (f₀ + f₁) × width`.
#[expect(
    clippy::manual_midpoint,
    reason = "this operation order is part of the generator version, and std's midpoint does not \
              promise one"
)]
fn trapezoid(f0: f64, f1: f64, width: f64) -> f64 {
    0.5 * (f0 + f1) * width
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::Seed;
    use crate::rng::sample::uniform::{uniform_of, uniform_open_of};
    use crate::rng::{Mark, ObjectKey, Threshold, tags};

    /// The segment a word picks.
    fn pick(thresholds: &Thresholds, word: u64) -> usize {
        Mark::from_word(word).pick(thresholds).unwrap()
    }

    fn kroupa() -> PiecewisePowerLaw {
        // Kroupa 2001, MNRAS 322, 231, eq. 2: α₀ = 0.3 on 0.01–0.08 M☉, α₁ = 1.3 on 0.08–0.5 M☉
        // and α₂ = 2.3 above 0.5 M☉.
        PiecewisePowerLaw::continuous(&[0.01, 0.08, 0.5, 150.0], &[0.3, 1.3, 2.3]).unwrap()
    }

    fn stream(n: u64) -> Stream {
        Stream::open(
            Seed::new(0x91ec_e515),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy_item(n),
        )
    }

    /// The brainstorm's "Sizing the layers" table: of the systems from 0.08 to 150 M☉ under
    /// Kroupa's function, 76%, 9.8%, 11%, 2.3% and 0.64% fall in layers A to E.
    #[test]
    fn kroupa_band_shares_match_the_brainstorm() {
        let law = kroupa();
        let edges = [0.08, 0.5, 0.75, 2.5, 8.0, 150.0];
        let expected = [76.0, 9.8, 11.0, 2.3, 0.64];
        let total = law.integral(0.08, 150.0);
        for (band, share) in expected.into_iter().enumerate() {
            let actual = 100.0 * law.integral(edges[band], edges[band + 1]) / total;
            assert!(
                (actual - share).abs() < 0.5,
                "band {band}: {actual:.3}% against {share}%"
            );
        }
    }

    #[test]
    fn continuous_coefficients_meet_at_every_break() {
        let law = kroupa();
        for &b in &law.breaks[1..law.breaks.len() - 1] {
            let i = segment_of(&law.breaks, b);
            let left = law.segments[i - 1].coefficient
                * math::powf(b, -law.segments[i - 1].law.exponent());
            let right =
                law.segments[i].coefficient * math::powf(b, -law.segments[i].law.exponent());
            assert!((left - right).abs() / right < 1e-14, "break {b}");
        }
    }

    #[test]
    fn integral_and_moment_have_their_closed_forms() {
        // x⁻¹ on [1, e] then 2 x⁻² on [e, 2e], given.
        let e = std::f64::consts::E;
        let law =
            PiecewisePowerLaw::with_coefficients(&[1.0, e, 2.0 * e], &[1.0, 2.0], &[1.0, 2.0])
                .unwrap();
        assert!((law.integral(1.0, e) - 1.0).abs() < 1e-15);
        assert!((law.integral(e, 2.0 * e) - 1.0 / e).abs() < 1e-15);
        assert!((law.integral(0.0, 100.0) - (1.0 + 1.0 / e)).abs() < 1e-15);
        // ∫ x × x⁻¹ = e − 1; ∫ x × 2x⁻² = 2 ln 2, the logarithmic case.
        assert!((law.moment(1.0, e, 1.0) - (e - 1.0)).abs() < 1e-14);
        assert!((law.moment(e, 2.0 * e, 1.0) - 2.0 * math::ln(2.0)).abs() < 1e-14);
        assert_same_bits(law.integral(3.0 * e, 4.0 * e), 0.0);
    }

    #[test]
    fn cdf_integrates_pdf_and_ends_at_zero_and_one() {
        let law = kroupa();
        assert_same_bits(law.cdf(0.01), 0.0);
        assert_same_bits(law.cdf(150.0), 1.0);
        let total = law.integral(0.01, 150.0);
        for x in [0.02, 0.08, 0.3, 0.5, 1.0, 40.0] {
            let expected = law.integral(0.0, x) / total;
            assert!((law.cdf(x) - expected).abs() < 1e-14, "x = {x}");
        }
        // The pdf is the cdf's slope, by central differences.
        for x in [0.02, 0.1, 0.3, 0.7, 3.0, 40.0] {
            let h = x * 1e-6;
            let slope = (law.cdf(x + h) - law.cdf(x - h)) / (2.0 * h);
            assert!((law.pdf(x) - slope).abs() / slope < 1e-6, "x = {x}");
        }
        assert_same_bits(law.pdf(200.0), 0.0);
        assert_same_bits(law.pdf(0.005), 0.0);
    }

    #[test]
    fn a_draw_takes_two_words_and_picks_by_threshold() {
        let law = kroupa();
        let mut s = stream(0);
        let reference = s.clone();
        for n in 0..50 {
            let x = law.sample(&mut s);
            let segment = pick(&law.thresholds, reference.word_at(2 * n));
            let expected = law.segments[segment]
                .law
                .quantile(uniform_of(reference.word_at(2 * n + 1)));
            assert_same_bits(x, expected);
        }
        assert_eq!(s.position(), 100);
    }

    /// The pick is the integer form of `uniform < Sᵢ ÷ S`: identical answers on every mark.
    #[test]
    fn the_pick_equals_the_float_comparison() {
        let law = kroupa();
        let total = law.total();
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x71c4);
        for _ in 0..100_000 {
            let word = lcg.next_u64();
            let u = uniform_of(word);
            let by_float = law.cumulative[1..]
                .iter()
                .position(|&sum| u < sum / total)
                .unwrap();
            assert_eq!(pick(&law.thresholds, word), by_float);
        }
        assert_eq!(law.thresholds.as_slice().last(), Some(&Threshold::ALWAYS));
        assert_eq!(pick(&law.thresholds, u64::MAX), 2);
    }

    /// The swap from the private copy of Design note 7 to `Thresholds` moved nothing: the
    /// thresholds are still `ceil(Sᵢ ÷ S × 2⁵³)` of the running sums.
    #[test]
    fn segment_thresholds_are_those_of_the_running_sums() {
        let law = kroupa();
        let total = law.total();
        let expected: Vec<Threshold> = law.cumulative[1..]
            .iter()
            .map(|&sum| Threshold::from_probability(sum / total))
            .collect();
        assert_eq!(law.thresholds.as_slice(), expected);
        let linear = triangle();
        assert_eq!(
            linear.thresholds.as_slice(),
            [Threshold::from_probability(1.0 / 3.0), Threshold::ALWAYS]
        );
    }

    #[test]
    fn truncation_keeps_the_density_inside_the_band() {
        let law = kroupa();
        let band = law.truncated(0.3, 2.0).unwrap();
        assert_same_bits(band.lo(), 0.3);
        assert_same_bits(band.hi(), 2.0);
        assert_eq!(band.breaks, vec![0.3, 0.5, 2.0]);
        assert!((band.integral(0.3, 2.0) - law.integral(0.3, 2.0)).abs() < 1e-15);
        let clipped = law.truncated(-5.0, 1e9).unwrap();
        assert_eq!(clipped.breaks, law.breaks);
        assert_eq!(
            law.truncated(200.0, 300.0),
            Err(BuildPiecewiseError::EmptyRange)
        );
        assert_eq!(
            law.truncated(1.0, 1.0),
            Err(BuildPiecewiseError::EmptyRange)
        );
        assert_eq!(
            law.truncated(f64::NAN, 1.0),
            Err(BuildPiecewiseError::NonFinite)
        );
    }

    #[test]
    fn invalid_power_laws_are_rejected_with_their_variant() {
        use BuildPiecewiseError as E;
        let build = PiecewisePowerLaw::with_coefficients;
        assert_eq!(build(&[1.0], &[], &[]), Err(E::TooFewBreaks { count: 1 }));
        assert_eq!(
            build(&[1.0, 2.0], &[1.0, 2.0], &[1.0]),
            Err(E::LengthMismatch {
                expected: 1,
                found: 2
            })
        );
        assert_eq!(
            build(&[1.0, 3.0, 2.0], &[1.0, 1.0], &[1.0, 1.0]),
            Err(E::NotIncreasing { index: 2 })
        );
        assert_eq!(build(&[0.0, 1.0], &[1.0], &[1.0]), Err(E::NonPositiveBreak));
        assert_eq!(build(&[1.0, 2.0], &[f64::NAN], &[1.0]), Err(E::NonFinite));
        assert_eq!(
            build(&[1.0, 2.0, 3.0], &[1.0, 1.0], &[1.0, -1.0]),
            Err(E::NegativeDensity { index: 1 })
        );
        assert_eq!(
            build(&[1.0, 2.0, 3.0], &[1.0, 1.0], &[0.0, 0.0]),
            Err(E::ZeroTotal)
        );
        assert_eq!(
            build(&[1.0, 1e300], &[-2.0], &[1.0]),
            Err(E::NotNormalisable)
        );
    }

    fn triangle() -> PiecewiseLinear {
        PiecewiseLinear::new(&[0.0, 1.0, 3.0], &[0.0, 2.0, 0.0]).unwrap()
    }

    #[test]
    fn linear_integral_pdf_and_cdf_are_the_trapezoids() {
        let law = triangle();
        assert!((law.integral(0.0, 3.0) - 3.0).abs() < 1e-15);
        assert!((law.integral(0.0, 0.5) - 0.25).abs() < 1e-15);
        assert!((law.integral(2.0, 10.0) - 0.5).abs() < 1e-15);
        assert!((law.pdf(1.0) - 2.0 / 3.0).abs() < 1e-15);
        assert!((law.pdf(2.0) - 1.0 / 3.0).abs() < 1e-15);
        assert!((law.cdf(1.0) - 1.0 / 3.0).abs() < 1e-15);
        assert!((law.cdf(2.0) - 2.5 / 3.0).abs() < 1e-15);
        assert_same_bits(law.pdf(-1.0), 0.0);
        assert_same_bits(law.cdf(-1.0), 0.0);
        assert_same_bits(law.cdf(4.0), 1.0);
    }

    /// The rationalised inverse lands where the segment's cdf equals the uniform, for rising,
    /// falling, flat and zero-ended segments.
    #[test]
    fn the_linear_inverse_solves_the_segment_cdf() {
        for (f0, f1) in [(0.0, 2.0), (2.0, 0.0), (1.0, 1.0), (0.5, 3.0), (3.0, 0.5)] {
            let law = PiecewiseLinear::new(&[1.0, 4.0], &[f0, f1]).unwrap();
            let mut lcg = hyperion_testkit::lcg::Lcg::new(0x11ea);
            for _ in 0..1_000 {
                let u = uniform_open_of(lcg.next_u64());
                let x = linear_inverse([1.0, 4.0], [f0, f1], u);
                assert!((law.cdf(x) - u).abs() < 1e-12, "({f0}, {f1}), u = {u}");
            }
            let extremes = [uniform_open_of(0), uniform_open_of(u64::MAX)];
            for u in extremes {
                let x = linear_inverse([1.0, 4.0], [f0, f1], u);
                assert!((1.0..=4.0).contains(&x), "({f0}, {f1}), u = {u}: {x}");
            }
        }
    }

    #[test]
    fn a_linear_draw_takes_two_words() {
        let law = triangle();
        let mut s = stream(3);
        let reference = s.clone();
        for n in 0..50 {
            let x = law.sample(&mut s);
            let i = pick(&law.thresholds, reference.word_at(2 * n));
            let u = uniform_open_of(reference.word_at(2 * n + 1));
            let expected = linear_inverse(
                [law.knots[i], law.knots[i + 1]],
                [law.densities[i], law.densities[i + 1]],
                u,
            );
            assert_same_bits(x, expected);
        }
        assert_eq!(s.position(), 100);
    }

    #[test]
    fn a_zero_density_segment_is_never_drawn() {
        let law = PiecewiseLinear::new(&[0.0, 1.0, 2.0, 3.0], &[1.0, 0.0, 0.0, 1.0]).unwrap();
        let mut s = stream(2);
        for _ in 0..10_000 {
            let x = law.sample(&mut s);
            assert!(x <= 1.0 || x >= 2.0, "{x} in the empty segment");
        }
        assert_eq!(s.position(), 20_000);
    }

    #[test]
    fn invalid_linear_densities_are_rejected_with_their_variant() {
        use BuildPiecewiseError as E;
        assert_eq!(
            PiecewiseLinear::new(&[0.0, 1.0], &[1.0]),
            Err(E::LengthMismatch {
                expected: 2,
                found: 1
            })
        );
        assert_eq!(
            PiecewiseLinear::new(&[0.0, 0.0], &[1.0, 1.0]),
            Err(E::NotIncreasing { index: 1 })
        );
        assert_eq!(
            PiecewiseLinear::new(&[0.0, 1.0], &[1.0, -0.1]),
            Err(E::NegativeDensity { index: 1 })
        );
        assert_eq!(
            PiecewiseLinear::new(&[0.0, 1.0], &[0.0, 0.0]),
            Err(E::ZeroTotal)
        );
        assert_eq!(
            PiecewiseLinear::new(&[0.0, f64::INFINITY], &[1.0, 1.0]),
            Err(E::NonFinite)
        );
    }
}
