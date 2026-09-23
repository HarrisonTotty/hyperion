//! The period, mass-ratio and eccentricity distributions of a companion's orbit (plan 11,
//! P11.T1.b, Design note 3), and the limits of what a companion is.
//!
//! Each distribution has its density, its cumulative distribution and a sampler that takes a
//! stream the caller opened and draws exactly one word from it, by inverse transform. None opens a
//! stream, so this module registers no domain tag: the tags arrive with the hierarchy draw
//! (P11.T2.a), which decides which stream and which draw number each value comes from.
//!
//! Every constant is re-checked against Duchêne and Kraus (2013, _Stellar Multiplicity_, ARA&A
//! 51, 269), Raghavan et al. (2010, _A Survey of Stellar Families_, ApJS 190, 1) and, for O stars,
//! Sana et al. (2012, _Binary interaction dominates the evolution of massive stars_, Science 337,
//! 444). Where the plan's figure differs from the papers the papers' is used, and the item says so.

use super::model::{MultiplicityModel, blend, lerp};
use crate::galaxy::imf::MASS_LIMIT_LO;
use crate::math;
use crate::rng::{PowerLaw, Stream};
use crate::units::consts::{GM_JUPITER, GM_SUN};
use crate::units::{Days, SolarMasses};

/// The lightest stellar companion: the hydrogen-burning limit of the stellar range, 0.08 M☉, the
/// same as the lightest grid primary ([`MASS_LIMIT_LO`]).
///
/// A mass ratio is drawn on `[MIN_COMPANION_MASS ÷ m₁, 1]`, so a companion's mass `q m₁` is at
/// least this to within the rounding of one multiplication.
pub const MIN_COMPANION_MASS: SolarMasses = SolarMasses::new(MASS_LIMIT_LO);

/// The lightest substellar companion (Design note 15): 13 Jupiter masses, about 0.012 41 M☉, the
/// deuterium-burning limit that the IAU Working Group on Extrasolar Planets' working definition
/// (2003) takes as the upper mass of a planet.
/// It is where plan 06's substellar cooling (P06.T13) and plan 13's giant-planet cooling meet.
///
/// Brown-dwarf companions continue the mass-ratio law of [`MassRatioDistribution`] from
/// [`MIN_COMPANION_MASS`] ÷ m₁ down to this mass ÷ m₁ (P11.T2.d).
pub const MIN_SUBSTELLAR_COMPANION_MASS: SolarMasses = SolarMasses::new(13.0 * GM_JUPITER / GM_SUN);

/// The period below which a pair's orbit is circular: 12 days.
///
/// Raghavan et al. (2010, §5.3.4 and Figure 14) find the solar-type pairs "with periods below
/// 12 days are circularized", with one exception, and draw the limit at 12 days. The plan's
/// 11.6 days is not in the paper and is replaced by its figure. Duchêne and Kraus (2013, §5.1.4)
/// note that measured circularisation periods run from under 1 day to about 20 days with primary
/// mass and age; one period serves every pair here.
pub const CIRCULARISATION_PERIOD: Days = Days::new(12.0);

/// The lower limit of every log-normal period component, in x = log₁₀(P ÷ 1 d): 0.1 d.
///
/// Raghavan et al.'s log-normal puts 0.4% of Sun-like companions below it, where no main-sequence
/// pair is observed. The physical lower limit, two stars that touch, is the hierarchy draw's.
pub const LOG_PERIOD_MIN: f64 = -1.0;

/// The upper limit of every log-normal period component, in x = log₁₀(P ÷ 1 d): about 2.7 × 10⁸
/// years, a separation of a parsec or more, beyond the half tidal radius that the hierarchy draw
/// cuts orbits at (Design note 4).
pub const LOG_PERIOD_MAX: f64 = 11.0;

/// The period below which a low-mass pair may be a twin, q ≥ [`TWIN_MIN_MASS_RATIO`] (Design
/// note 3): 100 days.
pub(super) const TWIN_MAX_PERIOD: Days = Days::new(100.0);

/// The mass ratio above which a pair counts as a twin, Raghavan et al.'s (2010, §5.3.5)
/// "like-mass pairs (M₂ ÷ M₁ > 0.95)".
pub(super) const TWIN_MIN_MASS_RATIO: f64 = 0.95;

/// The period, as x = log₁₀(P ÷ 1 d), that divides a massive primary's close companions from its
/// wide ones in the mass-ratio law: 3.5, about 3,000 days, the upper edge of the range Sana et
/// al. (2012) fitted, whose sample reaches "up to about nine years".
pub(super) const CLOSE_MAX_LOG_PERIOD: f64 = 3.5;

/// Steps of the safeguarded Newton inversion of a mixture's cumulative distribution. A step is a
/// bisection whenever Newton's would leave the bracket or fail to halve the step before last, so
/// the bracket shrinks at least as fast as by bisection every other step, and near the root each
/// Newton step squares the error; thirty-two leave the root at the rounding of the distribution
/// in every test.
const INVERSION_STEPS: u32 = 32;

/// The largest number of components of a [`PeriodDistribution`]: two anchors of two each.
const MAX_PERIOD_COMPONENTS: usize = 4;

/// √(2π).
const SQRT_TAU: f64 = 2.506_628_274_631_000_5;

/// The shape of one component of a period anchor, in x = log₁₀(P ÷ 1 d).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PeriodShape {
    /// A normal in x of this mean and standard deviation, truncated to [`LOG_PERIOD_MIN`]–
    /// [`LOG_PERIOD_MAX`].
    LogNormal { mean: f64, sigma: f64 },
    /// A density ∝ x^`exponent` on `[lo, hi]`, 0 < lo: Sana et al.'s (2012) form for O stars,
    /// and Öpik's law, flat in x, at an exponent of 0.
    LogPowerLaw { exponent: f64, lo: f64, hi: f64 },
}

/// One anchor of the period distribution: a primary mass and the weighted components of the
/// distribution there, whose weights sum to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PeriodAnchor {
    /// The primary mass, M☉.
    mass: f64,
    /// Each component's weight and shape.
    components: &'static [(f64, PeriodShape)],
}

impl PeriodAnchor {
    /// The primary mass, M☉.
    #[must_use]
    pub(super) fn mass(&self) -> f64 {
        self.mass
    }
}

/// Sana et al.'s (2012) binary fraction of O stars, companions of q ≥ 0.1 inside their fitted
/// range of log₁₀(P ÷ 1 d), 0.15–3.5: 0.69 ± 0.09. The surveys of B and O stars count companions
/// down to q ≈ 0.1 only: Duchêne and Kraus (2013, §3.5.2 and Figure 1, "for B and O stars, only
/// companions down to q ≈ 0.1 are included"), and Sana et al.'s fit, whose mass ratios run over
/// 0.1–1 (de Mink et al. 2014, ApJ 782, 7, §2; Sana et al. 2013, A&A 550, A107, Table 5).
pub(super) const O_STAR_CLOSE_SURVEYED: f64 = 0.69;

/// The O stars' companions of q ≥ 0.1 outside Sana et al.'s range: Duchêne and Kraus's 1.3 less
/// the close 0.69.
pub(super) const O_STAR_WIDE_SURVEYED: f64 = 0.61;

/// The share of the O stars' close mass-ratio law, γ = −0.1 on [0.08 M☉ ÷ 30 M☉, 1], above
/// q = 0.1: 0.878. The literal is that share, which a test recomputes.
const O_STAR_CLOSE_SHARE_SURVEYED: f64 = 0.878_344_245_709_802;

/// The share of the O stars' wide mass-ratio law, γ = −0.5 on [0.08 M☉ ÷ 30 M☉, 1], above
/// q = 0.1: 0.721. The literal is that share, which a test recomputes.
const O_STAR_WIDE_SHARE_SURVEYED: f64 = 0.721_004_759_673_168_2;

/// The close companions per O star of every mass ratio the model draws, down to
/// 0.08 M☉ ÷ 30 M☉: the surveyed 0.69 over its law's share above q = 0.1, 0.786.
pub(super) const O_STAR_CLOSE_FREQUENCY: f64 = O_STAR_CLOSE_SURVEYED / O_STAR_CLOSE_SHARE_SURVEYED;

/// The wide companions per O star of every mass ratio the model draws: the surveyed 0.61 over its
/// law's share above q = 0.1, 0.846.
pub(super) const O_STAR_WIDE_FREQUENCY: f64 = O_STAR_WIDE_SURVEYED / O_STAR_WIDE_SHARE_SURVEYED;

/// The share of an O star's companions in Sana et al.'s close component, 0.786 ÷ 1.632 = 0.481.
const O_STAR_CLOSE_WEIGHT: f64 =
    O_STAR_CLOSE_FREQUENCY / (O_STAR_CLOSE_FREQUENCY + O_STAR_WIDE_FREQUENCY);

/// The period anchors (Design note 3), each a primary mass and its distribution of
/// x = log₁₀(P ÷ 1 d); between two anchors the distribution is their mixture, weighted linearly in
/// ln m, and outside it is the end anchor's. Separations are turned into periods by Kepler's
/// third law at the anchor's mean system mass, `m₁ (1 + E[q])`, with the mean mass ratio of
/// [`MASS_RATIO_ANCHORS`].
///
/// - **0.09 M☉**: log-normal, mean 3.92, σ 0.5. Duchêne and Kraus (2013, Table 1 and §3.3.3):
///   unimodal, peak separation a ≈ 4.5 au, σ(log P) ≈ 0.5 (0.4–0.5 in the text); 4.5 au at
///   0.175 M☉ is 8,300 days.
/// - **0.25 M☉**: log-normal, mean 3.85, σ 1.3. Duchêne and Kraus (Table 1 and §3.2.3):
///   a ≈ 5.3 au and σ(log P) ≈ 1.3 from the RECONS sample; 5.3 au at 0.42 M☉ is 10^3.84 days, so
///   the plan's mean of 3.85 stands. The plan's width of 1.95, which it marked as from memory, is
///   replaced by the paper's 1.3.
/// - **1 M☉**: log-normal, mean 5.03, σ 2.28, Raghavan et al.'s (2010, §5.3.3) fit to their
///   solar-type pairs, a mean of 293 years; Duchêne and Kraus round it to P ≈ 250 years and 2.3.
/// - **2.7 M☉**: bimodal, as Duchêne and Kraus find for A stars (Table 1 and §3.4.3: a peak among
///   spectroscopic binaries at P ≈ 10 days and one among visual binaries at about 350 au). A
///   close log-normal of mean 1.0 and σ 1.0 with weight 0.4, and a wide one of mean 6.09 (350 au
///   at 3.78 M☉) and σ 1.3 with weight 0.6. The paper gives the peaks and not the widths, so the
///   widths and weights are set to its §3.4.2 figures: with a companion frequency of 1, this
///   puts 0.40 spectroscopic companions (P < 10³ days) per star, within the 30–45% of field A
///   stars (Abt 1983), and 0.39 visual ones over 50–2000 au, against the VAST survey's 40 ± 4%
///   there and Kouwenhoven et al.'s 37% in Sco-Cen.
/// - **30 M☉**: Sana et al.'s (2012) close component, a density ∝ x^−0.55 on x = 0.15–3.5
///   (π = −0.55 ± 0.22) holding 0.69 of the 1.3 companions of q ≥ 0.1 per star, and a wide one
///   flat in x (Öpik's law) from 3.5 to 7.76, 10⁴ au at 40.5 M☉, holding the remaining 0.61:
///   Duchêne and Kraus (§3.5.3) combine "short period binaries (log P ≲ 1, 30% of all high-mass
///   stars) and a power law period distribution extending out to ≳ 10⁴ AU". The weights are
///   those counts extended to every mass ratio the model draws, [`O_STAR_CLOSE_FREQUENCY`] and
///   [`O_STAR_WIDE_FREQUENCY`], 0.48 and 0.52. Counted as the surveys count, q ≥ 0.1, this gives
///   0.30 of O stars a companion inside 10 days and 0.43 visual companions over two decades of
///   separation, against their 45 ± 5%. Sana et al.'s x range is their fitted range (de Mink et
///   al. 2014, §2).
///
/// The plan's "close component whose weight rises to 0.7 for O stars" is read as Sana et al.'s
/// 0.69 companions of q ≥ 0.1 per O star inside 3,000 days.
pub(super) const PERIOD_ANCHORS: [PeriodAnchor; 5] = [
    PeriodAnchor {
        mass: 0.09,
        components: &[(
            1.0,
            PeriodShape::LogNormal {
                mean: 3.92,
                sigma: 0.5,
            },
        )],
    },
    PeriodAnchor {
        mass: 0.25,
        components: &[(
            1.0,
            PeriodShape::LogNormal {
                mean: 3.85,
                sigma: 1.3,
            },
        )],
    },
    PeriodAnchor {
        mass: 1.0,
        components: &[(
            1.0,
            PeriodShape::LogNormal {
                mean: 5.03,
                sigma: 2.28,
            },
        )],
    },
    PeriodAnchor {
        mass: 2.7,
        components: &[
            (
                0.4,
                PeriodShape::LogNormal {
                    mean: 1.0,
                    sigma: 1.0,
                },
            ),
            (
                0.6,
                PeriodShape::LogNormal {
                    mean: 6.09,
                    sigma: 1.3,
                },
            ),
        ],
    },
    PeriodAnchor {
        mass: 30.0,
        components: &[
            (
                O_STAR_CLOSE_WEIGHT,
                PeriodShape::LogPowerLaw {
                    exponent: -0.55,
                    lo: 0.15,
                    hi: CLOSE_MAX_LOG_PERIOD,
                },
            ),
            (
                1.0 - O_STAR_CLOSE_WEIGHT,
                PeriodShape::LogPowerLaw {
                    exponent: 0.0,
                    lo: CLOSE_MAX_LOG_PERIOD,
                    hi: 7.76,
                },
            ),
        ],
    },
];

/// One anchor of the mass-ratio law: a primary mass, the exponent γ of `q^γ` for close and for
/// wide pairs, and the share of twins among pairs under [`TWIN_MAX_PERIOD`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MassRatioAnchor {
    /// The primary mass, M☉.
    mass: f64,
    /// γ for pairs with x = log₁₀(P ÷ 1 d) up to [`CLOSE_MAX_LOG_PERIOD`].
    gamma_close: f64,
    /// γ for wider pairs.
    gamma_wide: f64,
    /// The share of pairs under [`TWIN_MAX_PERIOD`] that are twins.
    twin_share: f64,
}

impl MassRatioAnchor {
    /// The primary mass, M☉.
    #[must_use]
    pub(super) fn mass(&self) -> f64 {
        self.mass
    }
}

/// The mass-ratio anchors (Design note 3): `f(q) ∝ q^γ` on `[0.08 M☉ ÷ m₁, 1]`, with γ from
/// Duchêne and Kraus (2013, Table 1), re-checked:
///
/// | Anchor (M☉) | γ close | γ wide | Twins | Table 1 (and the text)                            |
/// | ----------- | ------- | ------ | ----- | ------------------------------------------------- |
/// | 0.09        | 4.2     | 4.2    | 0.3   | 4.2 ± 1.0 (Burgasser et al. 2006; §3.3.4)         |
/// | 0.25        | 0.4     | 0.4    | 0.3   | 0.4 ± 0.2 (0.39 ± 0.23, Delfosse et al.; §3.2.4)  |
/// | 1.0         | 0.3     | 0.3    | 0.3   | 0.3 ± 0.1 (0.28 ± 0.05, Raghavan et al.; §3.1.4)  |
/// | 2.7         | −0.5    | −0.5   | 0     | −0.5 ± 0.2 (−0.45 ± 0.15, Kouwenhoven; §3.4.4)    |
/// | 30          | −0.1    | −0.5   | 0     | −0.1 ± 0.6 close, −0.5 ± 0.1 wide (§3.5.4)        |
///
/// The O stars' close figure is Sana et al.'s (2012) for P ≤ 3,000 days and their wide one is for
/// separations of 100 au or more.
///
/// The plan lists 0.4, 0.3, −0.5 for wide A and B pairs and −0.1 for close O pairs; the table
/// gives the A and B figure for all their pairs, so it is used at both periods, and the VLM
/// figure, 4.2, is added at the fractions' own 0.09 M☉ anchor. Close and wide divide at
/// [`CLOSE_MAX_LOG_PERIOD`], the end of Sana et al.'s range.
///
/// **Twins** (Design note 3, "a twin excess for periods under 100 days"): among pairs under
/// [`TWIN_MAX_PERIOD`] a share of 0.3 has q uniform on [0.95, 1], for primaries up to 1 M☉, fading
/// linearly in ln m to none at 2.7 M☉. The share is fitted here to three figures, and is an
/// estimate from small numbers. Raghavan et al. (§5.3.5) have 16 pairs under 100 days and 16% of
/// their q > 0.9 pairs that close; with the 37–40 pairs above q = 0.9 that their Figure 16 and
/// their 27 like-mass pairs of about 250 imply, that puts about 6 of the 16 above q = 0.9,
/// against 2 for the smooth law, a share of 0.28. Duchêne and Kraus's §5.3 finds an excess of
/// q ≥ 0.98 at P ≤ 43 d among 0.5–2 M☉ binaries that is 2–3% of all spectroscopic binaries (Lucy
/// 2006; Simon and Obbie 2009); the model gives 2.4% of those under 10⁴ days. Galactic O stars
/// show no twin population (Sana et al. 2012), and the early claims of one among A stars were
/// selection effects (Duchêne and Kraus §3.4.4).
pub(super) const MASS_RATIO_ANCHORS: [MassRatioAnchor; 5] = [
    MassRatioAnchor {
        mass: 0.09,
        gamma_close: 4.2,
        gamma_wide: 4.2,
        twin_share: 0.3,
    },
    MassRatioAnchor {
        mass: 0.25,
        gamma_close: 0.4,
        gamma_wide: 0.4,
        twin_share: 0.3,
    },
    MassRatioAnchor {
        mass: 1.0,
        gamma_close: 0.3,
        gamma_wide: 0.3,
        twin_share: 0.3,
    },
    MassRatioAnchor {
        mass: 2.7,
        gamma_close: -0.5,
        gamma_wide: -0.5,
        twin_share: 0.0,
    },
    MassRatioAnchor {
        mass: 30.0,
        gamma_close: -0.1,
        gamma_wide: -0.5,
        twin_share: 0.0,
    },
];

/// Φ(z), the standard normal distribution.
#[must_use]
fn normal_cdf(z: f64) -> f64 {
    0.5 * math::erfc(-z * core::f64::consts::FRAC_1_SQRT_2)
}

/// φ(z), the standard normal density.
#[must_use]
fn normal_pdf(z: f64) -> f64 {
    math::exp(-0.5 * z * z) / SQRT_TAU
}

/// The root in `[lo, hi]` of `cdf(x) = target` for an increasing `cdf`, where `eval` gives the
/// cumulative distribution and its density at a point: Newton's method, safeguarded by
/// bisection whenever a Newton step would leave the bracket or would not halve the step before
/// last (the `rtsafe` of Press et al., _Numerical Recipes_, §9.4).
///
/// A fixed [`INVERSION_STEPS`] steps and no test of convergence, so the result is a fixed sequence
/// of operations on its inputs. The point returned is the one of smallest residual seen: once
/// Newton's step has converged to nothing, the halving test would send the next step back to
/// bisection, away from the root.
#[must_use]
fn invert(eval: impl Fn(f64) -> (f64, f64), target: f64, lo: f64, hi: f64) -> f64 {
    let (mut lo, mut hi) = (lo, hi);
    let mut x = lo.midpoint(hi);
    let mut step = hi - lo;
    let mut step_before = step;
    let (cdf, mut pdf) = eval(x);
    let mut residual = cdf - target;
    let mut best = (residual.abs(), x);
    for _ in 0..INVERSION_STEPS {
        if residual < 0.0 {
            lo = x;
        } else {
            hi = x;
        }
        let newton_leaves = ((x - hi) * pdf - residual) * ((x - lo) * pdf - residual) > 0.0;
        let newton_slow = (2.0 * residual).abs() > (step_before * pdf).abs();
        step_before = step;
        if pdf <= 0.0 || newton_leaves || newton_slow {
            step = 0.5 * (hi - lo);
            x = lo + step;
        } else {
            step = residual / pdf;
            x -= step;
        }
        let (cdf, density) = eval(x);
        residual = cdf - target;
        pdf = density;
        if residual.abs() < best.0 {
            best = (residual.abs(), x);
        }
    }
    best.1
}

/// A normal in x, truncated to `[lo, hi]`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TruncatedNormal {
    mean: f64,
    sigma: f64,
    lo: f64,
    hi: f64,
    /// Φ at the lower limit.
    below: f64,
    /// Φ(hi) − Φ(lo), the untruncated normal's probability inside the limits.
    inside: f64,
}

impl TruncatedNormal {
    #[must_use]
    fn new(mean: f64, sigma: f64, lo: f64, hi: f64) -> Self {
        let below = normal_cdf((lo - mean) / sigma);
        let inside = normal_cdf((hi - mean) / sigma) - below;
        Self {
            mean,
            sigma,
            lo,
            hi,
            below,
            inside,
        }
    }

    #[must_use]
    fn cdf_pdf(&self, x: f64) -> (f64, f64) {
        if x < self.lo {
            return (0.0, 0.0);
        }
        if x > self.hi {
            return (1.0, 0.0);
        }
        let z = (x - self.mean) / self.sigma;
        let cdf = ((normal_cdf(z) - self.below) / self.inside).clamp(0.0, 1.0);
        (cdf, normal_pdf(z) / (self.sigma * self.inside))
    }

    #[must_use]
    fn quantile(&self, u: f64) -> f64 {
        // Φ⁻¹ needs an argument strictly inside (0, 1); inside the limits it always is but for
        // rounding at the ends.
        let p = (self.below + u * self.inside).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON / 2.0);
        (self.mean + self.sigma * math::normal_quantile(p)).clamp(self.lo, self.hi)
    }
}

/// One built component of a [`PeriodDistribution`].
#[derive(Debug, Clone, Copy, PartialEq)]
enum Component {
    Normal(TruncatedNormal),
    Power(PowerLaw),
}

impl Component {
    #[must_use]
    fn new(shape: PeriodShape) -> Self {
        match shape {
            PeriodShape::LogNormal { mean, sigma } => Self::Normal(TruncatedNormal::new(
                mean,
                sigma,
                LOG_PERIOD_MIN,
                LOG_PERIOD_MAX,
            )),
            PeriodShape::LogPowerLaw { exponent, lo, hi } => Self::Power(
                PowerLaw::new(-exponent, lo, hi).expect("the anchors' power laws are valid"),
            ),
        }
    }

    #[must_use]
    fn cdf_pdf(&self, x: f64) -> (f64, f64) {
        match self {
            Self::Normal(n) => n.cdf_pdf(x),
            Self::Power(p) => (p.cdf(x), p.pdf(x)),
        }
    }

    #[must_use]
    fn quantile(&self, u: f64) -> f64 {
        match self {
            Self::Normal(n) => n.quantile(u),
            Self::Power(p) => p.quantile(u),
        }
    }

    #[must_use]
    fn support(&self) -> (f64, f64) {
        match self {
            Self::Normal(n) => (n.lo, n.hi),
            Self::Power(p) => (p.lo(), p.hi()),
        }
    }
}

/// The distribution of a companion's orbital period, in x = log₁₀(P ÷ 1 d), for one primary mass
/// ([`MultiplicityModel::period_distribution`]).
///
/// A mixture of at most four truncated components with fixed support (see the anchors on
/// [`MultiplicityModel::period_distribution`]). [`sample`](Self::sample) and
/// [`sample_in`](Self::sample_in) draw one word each and invert the cumulative distribution:
/// in closed form for one component, and by a fixed number of safeguarded Newton steps for a
/// mixture.
///
/// # Examples
///
/// Sun-like companions peak near 10⁵ days, and a draw restricted to a range stays inside it:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
/// use hyperion_sim::stellar::multiplicity::MultiplicityModel;
/// use hyperion_sim::units::{Days, SolarMasses};
///
/// let periods = MultiplicityModel::default_v1().period_distribution(SolarMasses::new(1.0));
/// assert!((periods.cdf(5.03) - 0.5).abs() < 1e-3);
/// let mut stream = Stream::open(Seed::new(7), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let close = periods.sample_in(&mut stream, Days::new(1.0), Days::new(100.0));
/// assert!((1.0..=100.0).contains(&close.value()));
/// assert_eq!(stream.position(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodDistribution {
    /// Each component's weight and the component; the first `len` are in use.
    components: [(f64, Component); MAX_PERIOD_COMPONENTS],
    len: usize,
}

impl PeriodDistribution {
    #[must_use]
    fn used(&self) -> &[(f64, Component)] {
        &self.components[..self.len]
    }

    /// Every component's lower and upper limit, in x = log₁₀(P ÷ 1 d): the points where the
    /// density may jump, for quadratures that put panel edges there.
    pub(super) fn component_limits(&self) -> impl Iterator<Item = f64> + '_ {
        self.used().iter().flat_map(|(_, c)| {
            let (lo, hi) = c.support();
            [lo, hi]
        })
    }

    /// The limits of the distribution's support, in x = log₁₀(P ÷ 1 d).
    #[must_use]
    pub fn support(&self) -> (f64, f64) {
        self.used()
            .iter()
            .map(|(_, c)| c.support())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (a, b)| {
                (lo.min(a), hi.max(b))
            })
    }

    #[must_use]
    fn cdf_pdf(&self, x: f64) -> (f64, f64) {
        let (cdf, pdf) = self.used().iter().fold((0.0, 0.0), |(cdf, pdf), (w, c)| {
            let (f, p) = c.cdf_pdf(x);
            (cdf + w * f, pdf + w * p)
        });
        (cdf.clamp(0.0, 1.0), pdf)
    }

    /// The density per unit x = log₁₀(P ÷ 1 d) at `log_period`.
    #[must_use]
    pub fn pdf(&self, log_period: f64) -> f64 {
        self.cdf_pdf(log_period).1
    }

    /// The probability that x = log₁₀(P ÷ 1 d) is at most `log_period`.
    #[must_use]
    pub fn cdf(&self, log_period: f64) -> f64 {
        self.cdf_pdf(log_period).0
    }

    /// The x = log₁₀(P ÷ 1 d) below which a share `u` of companions lies, for `u` in [0, 1],
    /// within `[lo, hi]` of the support.
    #[must_use]
    fn quantile_between(&self, u: f64, lo: f64, hi: f64) -> f64 {
        let used = self.used();
        if let [(_, only)] = used {
            return only.quantile(u).clamp(lo, hi);
        }
        // The mixture's quantile lies between its components' own.
        let (a, b) = used
            .iter()
            .map(|(_, c)| c.quantile(u))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| {
                (a.min(q), b.max(q))
            });
        let (a, b) = (a.max(lo), b.min(hi));
        if a >= b {
            return a.min(hi);
        }
        invert(|x| self.cdf_pdf(x), u, a, b)
    }

    /// The x = log₁₀(P ÷ 1 d) below which a share `u` of companions lies, for `u` in [0, 1]:
    /// the inverse of [`cdf`](Self::cdf).
    #[must_use]
    pub fn quantile(&self, u: f64) -> f64 {
        let (lo, hi) = self.support();
        self.quantile_between(u.clamp(0.0, 1.0), lo, hi)
    }

    /// A period drawn from the distribution: the [`quantile`](Self::quantile) of
    /// [`Stream::uniform_open`]. One word.
    pub fn sample(&self, stream: &mut Stream) -> Days {
        Days::new(math::exp10(self.quantile(stream.uniform_open())))
    }

    /// A period drawn from the distribution restricted to `[lo, hi]`, by inverse transform: the
    /// share `u` of [`Stream::uniform_open`] between the cumulative distribution at the two
    /// limits. One word.
    ///
    /// This is the exact conditional draw that Design note 1 asks for, on a range the caller
    /// chooses (an interacting range, or its complement). The limits are clamped to the support
    /// first, so the period lies in the clamped range: a range that lies wholly outside the
    /// support gives the support's nearer end, and one inside it that holds no probability (a
    /// gap between components) gives a period of the gap.
    ///
    /// # Panics
    ///
    /// Unless `0 < lo ≤ hi`, both finite.
    pub fn sample_in(&self, stream: &mut Stream, lo: Days, hi: Days) -> Days {
        assert!(
            lo.value() > 0.0 && lo <= hi && hi.value().is_finite(),
            "sample_in needs 0 < lo ≤ hi, got {lo:?} and {hi:?}"
        );
        let (s_lo, s_hi) = self.support();
        let a = math::log10(lo.value()).clamp(s_lo, s_hi);
        let b = math::log10(hi.value()).clamp(s_lo, s_hi);
        let (f_a, f_b) = (self.cdf(a), self.cdf(b));
        let p = f_a + stream.uniform_open() * (f_b - f_a);
        Days::new(math::exp10(self.quantile_between(p, a, b)))
    }
}

/// Which part of a mass-ratio law a period selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PeriodRegime {
    /// Under [`TWIN_MAX_PERIOD`]: the close exponent, and twins.
    Twin,
    /// From [`TWIN_MAX_PERIOD`] to x = [`CLOSE_MAX_LOG_PERIOD`]: the close exponent.
    Close,
    /// Wider: the wide exponent.
    Wide,
}

impl PeriodRegime {
    /// The three regimes in ascending period.
    pub(super) const ALL: [Self; 3] = [Self::Twin, Self::Close, Self::Wide];

    #[must_use]
    fn of(period: Days) -> Self {
        if period < TWIN_MAX_PERIOD {
            Self::Twin
        } else if math::log10(period.value()) <= CLOSE_MAX_LOG_PERIOD {
            Self::Close
        } else {
            Self::Wide
        }
    }
}

/// `∫ qᵏ dq` over `[lo, 1]`, for `0 < lo ≤ 1`: `(1 − lo^(k+1)) ÷ (k + 1)`, or `−ln lo` at k = −1.
#[must_use]
fn power_integral(k: f64, lo: f64) -> f64 {
    let g = k + 1.0;
    let log_lo = math::ln(lo);
    if g.abs() < 1e-8 {
        -log_lo
    } else {
        -math::exp_m1(g * log_lo) / g
    }
}

/// The distribution of a companion's mass ratio q = m₂ ÷ m₁ for one primary mass and period
/// ([`MultiplicityModel::mass_ratio_distribution`]): `q^γ` on `[0.08 M☉ ÷ m₁, 1]`, with a share of
/// twins uniform on [0.95, 1] for close low-mass pairs.
///
/// For a primary of 0.08 M☉ or less the range is empty and every companion has q = 1.
///
/// # Examples
///
/// No companion is below the hydrogen-burning limit:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
/// use hyperion_sim::stellar::multiplicity::MultiplicityModel;
/// use hyperion_sim::units::{Days, SolarMasses};
///
/// let m1 = SolarMasses::new(0.4);
/// let ratios = MultiplicityModel::default_v1().mass_ratio_distribution(m1, Days::new(10.0));
/// let mut stream = Stream::open(Seed::new(2), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let q = ratios.sample(&mut stream);
/// assert!(q >= ratios.lo() && q <= 1.0);
/// assert!((ratios.lo() - 0.2).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassRatioDistribution {
    /// The lowest mass ratio, 0.08 M☉ ÷ m₁, or 1 when that is at least 1.
    lo: f64,
    /// The exponent γ of `q^γ`.
    gamma: f64,
    /// `q^γ` on `[lo, 1]`, or `None` when the range is empty.
    power: Option<PowerLaw>,
    /// The share of twins.
    twin_share: f64,
    /// The twins' lowest mass ratio, `max(0.95, lo)`.
    twin_lo: f64,
}

impl MassRatioDistribution {
    #[must_use]
    fn new(lo: f64, gamma: f64, twin_share: f64) -> Self {
        if lo >= 1.0 {
            return Self {
                lo: 1.0,
                gamma,
                power: None,
                twin_share: 0.0,
                twin_lo: 1.0,
            };
        }
        let power = PowerLaw::new(-gamma, lo, 1.0).expect("q^γ on [lo, 1] with lo < 1 is valid");
        Self {
            lo,
            gamma,
            power: Some(power),
            twin_share,
            twin_lo: TWIN_MIN_MASS_RATIO.max(lo),
        }
    }

    /// The lowest mass ratio: 0.08 M☉ ÷ m₁, or 1 for a primary of 0.08 M☉ or less.
    #[must_use]
    pub fn lo(&self) -> f64 {
        self.lo
    }

    /// The exponent γ of the smooth law `q^γ`.
    #[must_use]
    pub fn gamma(&self) -> f64 {
        self.gamma
    }

    /// The share of twins, uniform on [0.95, 1]: positive only for low-mass pairs under 100 days.
    #[must_use]
    pub fn twin_share(&self) -> f64 {
        self.twin_share
    }

    /// The same law without its twins: `q^γ` alone.
    #[must_use]
    pub(super) fn smooth_part(&self) -> Self {
        Self {
            twin_share: 0.0,
            ..*self
        }
    }

    /// The twins' part of the density at `q`, before their weight.
    #[must_use]
    fn twin_pdf(&self, q: f64) -> f64 {
        if (self.twin_lo..=1.0).contains(&q) {
            1.0 / (1.0 - self.twin_lo)
        } else {
            0.0
        }
    }

    #[must_use]
    fn twin_cdf(&self, q: f64) -> f64 {
        ((q - self.twin_lo) / (1.0 - self.twin_lo)).clamp(0.0, 1.0)
    }

    /// The density at `q`; 0 everywhere for a primary of 0.08 M☉ or less, whose companions all
    /// have q = 1.
    #[must_use]
    pub fn pdf(&self, q: f64) -> f64 {
        let Some(power) = &self.power else {
            return 0.0;
        };
        let w = self.twin_share;
        (1.0 - w) * power.pdf(q) + w * self.twin_pdf(q)
    }

    /// The probability that the mass ratio is at most `q`.
    #[must_use]
    pub fn cdf(&self, q: f64) -> f64 {
        let Some(power) = &self.power else {
            return if q >= 1.0 { 1.0 } else { 0.0 };
        };
        let w = self.twin_share;
        ((1.0 - w) * power.cdf(q) + w * self.twin_cdf(q)).clamp(0.0, 1.0)
    }

    /// The mean mass ratio.
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.power.is_none() {
            return 1.0;
        }
        let smooth =
            power_integral(self.gamma + 1.0, self.lo) / power_integral(self.gamma, self.lo);
        let w = self.twin_share;
        (1.0 - w) * smooth + w * 0.5 * (self.twin_lo + 1.0)
    }

    /// The mass ratio below which a share `u` of companions lies, for `u` in [0, 1]: the inverse
    /// of [`cdf`](Self::cdf), in closed form below the twins and by a fixed number of Newton steps
    /// among them.
    #[must_use]
    pub fn quantile(&self, u: f64) -> f64 {
        let Some(power) = &self.power else {
            return 1.0;
        };
        let u = u.clamp(0.0, 1.0);
        let w = self.twin_share;
        if w <= 0.0 {
            return power.quantile(u);
        }
        let below_twins = (1.0 - w) * power.cdf(self.twin_lo);
        if u <= below_twins {
            return power.quantile(u / (1.0 - w)).min(self.twin_lo);
        }
        let eval = |q: f64| {
            let cdf = (1.0 - w) * power.cdf(q) + w * self.twin_cdf(q);
            let pdf = (1.0 - w) * power.pdf(q) + w * self.twin_pdf(q);
            (cdf, pdf)
        };
        invert(eval, u, self.twin_lo, 1.0).clamp(self.twin_lo, 1.0)
    }

    /// A mass ratio drawn from the distribution: the [`quantile`](Self::quantile) of
    /// [`Stream::uniform`]. One word, also for a primary whose companions all have q = 1.
    pub fn sample(&self, stream: &mut Stream) -> f64 {
        self.quantile(stream.uniform())
    }
}

/// The distribution of a companion orbit's eccentricity for one period
/// ([`MultiplicityModel::eccentricity_distribution`]): circular under
/// [`CIRCULARISATION_PERIOD`], and uniform on `[0, e_max]` above it.
///
/// Duchêne and Kraus (2013, §5.1.4) find the distribution "essentially flat" beyond about 100 days
/// at every primary mass, and "inconsistent with the so-called thermal distribution"; Raghavan et
/// al. (2010, §5.3.4) find it roughly flat above their 12-day circularisation limit. The plan's
/// thermal law above 10³ days is Duquennoy and Mayor's (1991), whose own bias-corrected
/// distribution Duchêne and Kraus show to be flat beyond e ≈ 0.3, and is replaced by the flat
/// law. The cap `e_max = 1 − (P ÷ P_circ)^(−2/3)` keeps the periastron `a (1 − e)` outside the
/// separation of a circular orbit of the circularisation period about the same masses, as Design
/// note 3 asks; it is the rising upper envelope of the period–eccentricity plane that both papers
/// describe.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::multiplicity::MultiplicityModel;
/// use hyperion_sim::units::Days;
///
/// let model = MultiplicityModel::default_v1();
/// assert_eq!(model.eccentricity_distribution(Days::new(5.0)).e_max(), 0.0);
/// // Eight times the circularisation period: the separation is 4 times the circular one's.
/// let wide = model.eccentricity_distribution(Days::new(96.0));
/// assert!((wide.e_max() - 0.75).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EccentricityDistribution {
    /// The largest eccentricity, in [0, 1); 0 for a circular orbit.
    e_max: f64,
}

impl EccentricityDistribution {
    #[must_use]
    fn for_period(period: Days) -> Self {
        if period < CIRCULARISATION_PERIOD {
            return Self { e_max: 0.0 };
        }
        let ratio = period / CIRCULARISATION_PERIOD;
        Self {
            e_max: 1.0 - math::powf(ratio, -2.0 / 3.0),
        }
    }

    /// The largest eccentricity the orbit can have, in [0, 1): 0 under the circularisation
    /// period.
    #[must_use]
    pub fn e_max(&self) -> f64 {
        self.e_max
    }

    /// The density at `e`; 0 everywhere for a circular orbit, whose eccentricity is always 0.
    #[must_use]
    pub fn pdf(&self, e: f64) -> f64 {
        if self.e_max > 0.0 && (0.0..=self.e_max).contains(&e) {
            1.0 / self.e_max
        } else {
            0.0
        }
    }

    /// The probability that the eccentricity is at most `e`.
    #[must_use]
    pub fn cdf(&self, e: f64) -> f64 {
        if e < 0.0 {
            0.0
        } else if self.e_max > 0.0 {
            (e / self.e_max).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// The eccentricity below which a share `u` of orbits lies, for `u` in [0, 1].
    #[must_use]
    pub fn quantile(&self, u: f64) -> f64 {
        self.e_max * u.clamp(0.0, 1.0)
    }

    /// An eccentricity drawn from the distribution, in `[0, e_max)`: `e_max` times
    /// [`Stream::uniform`]. One word, also for a circular orbit.
    pub fn sample(&self, stream: &mut Stream) -> f64 {
        self.quantile(stream.uniform())
    }
}

impl MultiplicityModel {
    /// The distribution of a companion's period, in x = log₁₀(P ÷ 1 d), for a primary of initial
    /// mass `m1`.
    ///
    /// Between two anchors it is their mixture, the upper anchor's weight rising linearly in ln m;
    /// outside them it is the end anchor's. The anchors, in M☉ (Duchêne and Kraus 2013, Table 1,
    /// §3; Raghavan et al. 2010, §5.3.3; Sana et al. 2012):
    ///
    /// | Anchor | Distribution of x                                                           |
    /// | ------ | --------------------------------------------------------------------------- |
    /// | 0.09   | normal, mean 3.92, σ 0.5                                                    |
    /// | 0.25   | normal, mean 3.85, σ 1.3                                                    |
    /// | 1.0    | normal, mean 5.03, σ 2.28                                                   |
    /// | 2.7    | 0.4 × normal (1.0, 1.0) + 0.6 × normal (6.09, 1.3)                          |
    /// | 30     | 0.48 × x^−0.55 on 0.15–3.5 + 0.52 × flat on 3.5–7.76                        |
    ///
    /// Every normal is truncated to [`LOG_PERIOD_MIN`]–[`LOG_PERIOD_MAX`]. Its sources and the
    /// corrections to the plan's figures are in this module's anchor table.
    #[must_use]
    pub fn period_distribution(&self, m1: SolarMasses) -> PeriodDistribution {
        let anchors = self.period_anchors();
        let (i, t) = blend(anchors, |a| a.mass, m1.value());
        let placeholder = (0.0, Component::new(anchors[i].components[0].1));
        let mut components = [placeholder; MAX_PERIOD_COMPONENTS];
        let mut len = 0;
        for (anchor, share) in [(&anchors[i], 1.0 - t), (&anchors[i + 1], t)] {
            if share <= 0.0 {
                continue;
            }
            for &(weight, shape) in anchor.components {
                components[len] = (share * weight, Component::new(shape));
                len += 1;
            }
        }
        PeriodDistribution { components, len }
    }

    /// The mass-ratio law for a primary of `m1` in one period regime.
    ///
    /// # Panics
    ///
    /// If `m1` is not positive and finite.
    #[must_use]
    pub(super) fn mass_ratio_in(
        &self,
        m1: SolarMasses,
        regime: PeriodRegime,
    ) -> MassRatioDistribution {
        assert!(
            m1.value() > 0.0 && m1.value().is_finite(),
            "a mass-ratio law needs a positive primary mass, got {m1:?}"
        );
        let anchors = self.mass_ratio_anchors();
        let (i, t) = blend(anchors, |a| a.mass, m1.value());
        let (a, b) = (&anchors[i], &anchors[i + 1]);
        let (gamma, twins) = match regime {
            PeriodRegime::Twin => (
                lerp(a.gamma_close, b.gamma_close, t),
                lerp(a.twin_share, b.twin_share, t),
            ),
            PeriodRegime::Close => (lerp(a.gamma_close, b.gamma_close, t), 0.0),
            PeriodRegime::Wide => (lerp(a.gamma_wide, b.gamma_wide, t), 0.0),
        };
        MassRatioDistribution::new(MIN_COMPANION_MASS / m1, gamma, twins)
    }

    /// The distribution of the mass ratio q = m₂ ÷ m₁ of a companion of a primary of initial mass
    /// `m1` on an orbit of `period`: `q^γ` on `[0.08 M☉ ÷ m₁, 1]`, γ linear in ln m between the
    /// anchors of Duchêne and Kraus (2013, Table 1), close or wide by period, with twins under
    /// 100 days for low-mass primaries (see this module's anchor table for each figure).
    ///
    /// # Panics
    ///
    /// If `m1` is not positive and finite.
    #[must_use]
    pub fn mass_ratio_distribution(&self, m1: SolarMasses, period: Days) -> MassRatioDistribution {
        self.mass_ratio_in(m1, PeriodRegime::of(period))
    }

    /// The distribution of a companion orbit's eccentricity at `period`, the same for every
    /// primary mass (Duchêne and Kraus 2013, §5.1.4: "remarkably little dependency on primary
    /// mass").
    #[must_use]
    pub fn eccentricity_distribution(&self, period: Days) -> EccentricityDistribution {
        EccentricityDistribution::for_period(period)
    }

    /// The share of a primary's companions in each period regime, and the mass-ratio law there.
    ///
    /// # Panics
    ///
    /// If `m1` is not positive and finite.
    #[must_use]
    pub(super) fn mass_ratio_regimes(&self, m1: SolarMasses) -> [(f64, MassRatioDistribution); 3] {
        let periods = self.period_distribution(m1);
        let twin_edge = periods.cdf(math::log10(TWIN_MAX_PERIOD.value()));
        let close_edge = periods.cdf(CLOSE_MAX_LOG_PERIOD).max(twin_edge);
        let shares = [twin_edge, close_edge - twin_edge, 1.0 - close_edge];
        let mut regimes = PeriodRegime::ALL.map(|r| (0.0, self.mass_ratio_in(m1, r)));
        for (regime, share) in regimes.iter_mut().zip(shares) {
            regime.0 = share;
        }
        regimes
    }

    /// The probability that a companion of a primary of initial mass `m1` has a mass ratio of at
    /// most `q`, over every period: the mass-ratio law marginalised over the period distribution.
    ///
    /// This is the law a quadrature over companions reads: plan 02's `stars_below` and
    /// `mean_present_mass` through P11.T1.d, and
    /// [`all_stars_fraction_below`](super::all_stars_fraction_below).
    ///
    /// # Panics
    ///
    /// If `m1` is not positive and finite.
    #[must_use]
    pub fn companion_mass_ratio_cdf(&self, m1: SolarMasses, q: f64) -> f64 {
        self.mass_ratio_regimes(m1)
            .iter()
            .fold(0.0, |sum, (share, law)| sum + share * law.cdf(q))
            .clamp(0.0, 1.0)
    }

    /// The mean mass ratio of a companion of a primary of initial mass `m1`, over every period.
    ///
    /// # Panics
    ///
    /// If `m1` is not positive and finite.
    #[must_use]
    pub fn mean_companion_mass_ratio(&self, m1: SolarMasses) -> f64 {
        self.mass_ratio_regimes(m1)
            .iter()
            .fold(0.0, |sum, (share, law)| sum + share * law.mean())
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::Seed;
    use crate::galaxy::quad::gl32;
    use crate::rng::{ObjectKey, tags};

    /// The sample size of every Kolmogorov–Smirnov test (P11.T1.b).
    const SAMPLES: u32 = 100_000;

    /// Five primary masses, M☉, from M dwarfs to O stars, each between or at anchors.
    const MASSES: [f64; 5] = [0.15, 0.5, 1.0, 4.0, 30.0];

    /// One period in each regime of the mass-ratio law, days.
    const REGIME_PERIODS: [f64; 3] = [10.0, 1_000.0, 1e5];

    /// The mass ratio down to which the surveys of B and O stars count companions.
    const SURVEY_MIN_MASS_RATIO: f64 = 0.1;

    fn model() -> MultiplicityModel {
        MultiplicityModel::default_v1()
    }

    fn stream(item: u64) -> Stream {
        Stream::open(
            Seed::new(0x5EED_0011),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy_item(item),
        )
    }

    /// A sweep of primary masses over the stellar range, between and at the anchors.
    fn mass_sweep() -> impl Iterator<Item = SolarMasses> {
        (0_u32..=60).map(|i| SolarMasses::new(0.08 * math::exp(f64::from(i) * 0.125_6)))
    }

    fn ks(name: &str, mut samples: Vec<f64>, cdf: impl Fn(f64) -> f64) {
        let result = ks_one_sample(&mut samples, cdf);
        println!(
            "{name}: D = {:.5}, p = {:.3}",
            result.statistic, result.p_value
        );
        assert_p_value(name, result.p_value, ALPHA);
    }

    /// ∫ over `[a, b]` of `f` by 32-point panels a fortieth of the range wide.
    fn integral(f: impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
        let step = (b - a) / 40.0;
        (0_u32..40).fold(0.0, |sum, i| {
            let lo = a + step * f64::from(i);
            sum + gl32(&f, lo, lo + step)
        })
    }

    #[test]
    fn periods_pass_kolmogorov_smirnov_at_five_masses() {
        let model = model();
        for (item, m) in (0_u64..).zip(MASSES) {
            let periods = model.period_distribution(SolarMasses::new(m));
            let mut s = stream(item);
            let samples = (0..SAMPLES)
                .map(|_| math::log10(periods.sample(&mut s).value()))
                .collect();
            ks(&format!("log P at {m} M☉"), samples, |x| periods.cdf(x));
        }
    }

    #[test]
    fn restricted_periods_pass_kolmogorov_smirnov_at_five_masses() {
        let model = model();
        for (item, m) in (10_u64..).zip(MASSES) {
            let periods = model.period_distribution(SolarMasses::new(m));
            let (lo, hi) = (Days::new(3.0), Days::new(3e4));
            let (a, b) = (math::log10(lo.value()), math::log10(hi.value()));
            let (f_a, f_b) = (periods.cdf(a), periods.cdf(b));
            let mut s = stream(item);
            let samples: Vec<f64> = (0..SAMPLES)
                .map(|_| math::log10(periods.sample_in(&mut s, lo, hi).value()))
                .collect();
            assert!(
                samples
                    .iter()
                    .all(|&x| (a - 1e-12..=b + 1e-12).contains(&x))
            );
            ks(
                &format!("log P in 3–30,000 d at {m} M☉"),
                samples,
                |x| ((periods.cdf(x) - f_a) / (f_b - f_a)).clamp(0.0, 1.0),
            );
        }
    }

    #[test]
    fn mass_ratios_pass_kolmogorov_smirnov_at_five_masses() {
        let model = model();
        for (item, m) in (20_u64..).step_by(3).zip(MASSES) {
            for (offset, p) in (0_u64..).zip(REGIME_PERIODS) {
                let law = model.mass_ratio_distribution(SolarMasses::new(m), Days::new(p));
                let mut s = stream(item + offset);
                let samples = (0..SAMPLES).map(|_| law.sample(&mut s)).collect();
                ks(&format!("q at {m} M☉ and {p} d"), samples, |q| law.cdf(q));
            }
        }
    }

    #[test]
    fn eccentricities_pass_kolmogorov_smirnov_at_five_periods() {
        let model = model();
        // Periods above the circularisation period: under it every orbit is circular.
        for (item, p) in (40_u64..).zip([15.0, 30.0, 300.0, 1e4, 1e7]) {
            let law = model.eccentricity_distribution(Days::new(p));
            let mut s = stream(item);
            let samples = (0..SAMPLES).map(|_| law.sample(&mut s)).collect();
            ks(&format!("e at {p} d"), samples, |e| law.cdf(e));
        }
    }

    /// The brainstorm: periods "log-normal, peaking near 10⁵ days for Sun-like primaries".
    #[test]
    fn the_sun_like_period_mode_is_near_ten_to_the_fifth_days() {
        let periods = model().period_distribution(SolarMasses::new(1.0));
        let mode = (0_u32..=12_000)
            .map(|i| -1.0 + f64::from(i) * 0.001)
            .max_by(|&a, &b| periods.pdf(a).total_cmp(&periods.pdf(b)))
            .unwrap();
        println!("Sun-like period mode: 10^{mode:.3} d");
        assert!((mode - 5.0).abs() <= 0.1, "{mode}");
    }

    #[test]
    fn no_companion_is_below_the_hydrogen_burning_limit() {
        let model = model();
        let mut s = stream(50);
        for m1 in mass_sweep() {
            for p in REGIME_PERIODS {
                let law = model.mass_ratio_distribution(m1, Days::new(p));
                for _ in 0..2_000 {
                    let q = law.sample(&mut s);
                    let m2 = q * m1.value();
                    assert!(q <= 1.0, "q = {q} at {m1:?}");
                    assert!(
                        m2 >= MIN_COMPANION_MASS.value() * (1.0 - f64::EPSILON),
                        "a companion of {m2} M☉ at {m1:?} and {p} d"
                    );
                }
            }
        }
    }

    #[test]
    fn eccentric_orbits_keep_periastron_outside_the_circularisation_separation() {
        let model = model();
        let mut s = stream(51);
        for i in 0_u32..=80 {
            let period = Days::new(math::exp10(-1.0 + f64::from(i) * 0.15));
            let law = model.eccentricity_distribution(period);
            let floor = if period < CIRCULARISATION_PERIOD {
                1.0
            } else {
                math::powf(CIRCULARISATION_PERIOD / period, 2.0 / 3.0)
            };
            for _ in 0..500 {
                let e = law.sample(&mut s);
                assert!((0.0..1.0).contains(&e), "e = {e} at {period:?}");
                // a (1 − e) ≥ a_circ, with a ∝ P^(2/3) at fixed masses.
                assert!(1.0 - e >= floor * (1.0 - 1e-12), "e = {e} at {period:?}");
                if period < CIRCULARISATION_PERIOD {
                    assert_same_bits(e, 0.0);
                }
            }
        }
    }

    #[test]
    fn densities_integrate_to_their_cumulative_distributions() {
        let model = model();
        for m1 in mass_sweep().step_by(6) {
            let periods = model.period_distribution(m1);
            // Power-law components' densities jump at their limits, so the integral has edges
            // there.
            let mut edges: Vec<f64> = periods.component_limits().collect();
            edges.sort_by(f64::total_cmp);
            edges.dedup();
            let upto = |x: f64| {
                edges.windows(2).fold(0.0, |sum, pair| {
                    let (a, b) = (pair[0], pair[1].min(x));
                    if b > a {
                        sum + integral(|t| periods.pdf(t), a, b)
                    } else {
                        sum
                    }
                })
            };
            let (_, hi) = periods.support();
            let total = upto(hi);
            assert!((total - 1.0).abs() < 1e-9, "∫ pdf = {total} at {m1:?}");
            for x in [0.5, 2.0, 3.5, 5.0, 8.0] {
                let partial = upto(x);
                assert!(
                    (partial - periods.cdf(x)).abs() < 1e-9,
                    "∫ pdf to {x} = {partial} against {} at {m1:?}",
                    periods.cdf(x)
                );
            }
            for p in REGIME_PERIODS {
                let law = model.mass_ratio_distribution(m1, Days::new(p));
                if law.lo() >= 1.0 {
                    continue;
                }
                // The twins' density jumps at 0.95, so the integral has an edge there.
                let edge = TWIN_MIN_MASS_RATIO.max(law.lo());
                let over =
                    |f: &dyn Fn(f64) -> f64| integral(f, law.lo(), edge) + integral(f, edge, 1.0);
                let total = over(&|q| law.pdf(q));
                assert!(
                    (total - 1.0).abs() < 1e-9,
                    "∫ pdf(q) = {total} at {m1:?}, {p} d"
                );
                let mean = over(&|q| q * law.pdf(q));
                assert!(
                    (mean - law.mean()).abs() < 1e-9,
                    "mean {mean} at {m1:?}, {p} d"
                );
                let middle = law.lo().midpoint(edge);
                let partial = integral(|q| law.pdf(q), law.lo(), middle);
                let at = law.cdf(middle);
                assert!(
                    (partial - at).abs() < 1e-9,
                    "∫ pdf(q) = {partial} against {at}"
                );
            }
        }
    }

    #[test]
    fn quantiles_invert_the_cumulative_distributions() {
        let model = model();
        for m1 in mass_sweep() {
            let periods = model.period_distribution(m1);
            let mut last = f64::NEG_INFINITY;
            for i in 1_u32..1_000 {
                let u = f64::from(i) / 1_000.0;
                let x = periods.quantile(u);
                assert!(x >= last, "the quantile falls at u = {u}, {m1:?}");
                last = x;
                let back = periods.cdf(x);
                assert!((back - u).abs() < 1e-12, "F(Q({u})) = {back} at {m1:?}");
            }
            for p in REGIME_PERIODS {
                let law = model.mass_ratio_distribution(m1, Days::new(p));
                if law.lo() >= 1.0 {
                    continue;
                }
                for i in 1_u32..200 {
                    let u = f64::from(i) / 200.0;
                    let back = law.cdf(law.quantile(u));
                    assert!(
                        (back - u).abs() < 1e-12,
                        "F(Q({u})) = {back} for q at {m1:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_sampler_draws_one_word() {
        let model = model();
        let mut s = stream(52);
        for m1 in [0.08, 0.3, 1.0, 5.0, 60.0].map(SolarMasses::new) {
            let start = s.position();
            let period = model.period_distribution(m1).sample(&mut s);
            let _ =
                model
                    .period_distribution(m1)
                    .sample_in(&mut s, Days::new(1.0), Days::new(10.0));
            let _ = model.mass_ratio_distribution(m1, period).sample(&mut s);
            let _ = model
                .eccentricity_distribution(Days::new(5.0))
                .sample(&mut s);
            let _ = model.eccentricity_distribution(period).sample(&mut s);
            assert_eq!(s.position() - start, 5, "at {m1:?}");
        }
    }

    #[test]
    fn the_same_stream_gives_the_same_draws() {
        let model = model();
        let draw = || {
            let mut s = stream(53);
            (0..200)
                .flat_map(|i| {
                    let m1 = SolarMasses::new(0.08 + 0.5 * f64::from(i));
                    let p = model.period_distribution(m1).sample(&mut s);
                    let q = model.mass_ratio_distribution(m1, p).sample(&mut s);
                    let e = model.eccentricity_distribution(p).sample(&mut s);
                    [p.value(), q, e]
                })
                .collect::<Vec<_>>()
        };
        for (a, b) in draw().into_iter().zip(draw()) {
            assert_same_bits(a, b);
        }
    }

    #[test]
    fn a_primary_at_the_stellar_floor_has_equal_mass_companions() {
        let law = model().mass_ratio_distribution(SolarMasses::new(0.08), Days::new(10.0));
        let mut s = stream(54);
        assert_same_bits(law.lo(), 1.0);
        assert_same_bits(law.sample(&mut s), 1.0);
        assert_same_bits(law.cdf(0.99), 0.0);
        assert_same_bits(law.cdf(1.0), 1.0);
        assert_same_bits(law.mean(), 1.0);
    }

    /// The period of a relative orbit of `a_au` about `total` M☉, as x = log₁₀(P ÷ 1 d).
    fn log_period_of(a_au: f64, total: f64) -> f64 {
        math::log10((a_au * a_au * a_au / total).sqrt() * 365.25)
    }

    /// The period anchors' peaks and shares reproduce the figures they were set from (see
    /// `PERIOD_ANCHORS`).
    #[test]
    fn the_period_anchors_reproduce_their_sources() {
        let model = model();
        let at = |m: f64| model.period_distribution(SolarMasses::new(m));
        let mode = |m: f64| {
            let periods = at(m);
            (0_u32..=12_000)
                .map(|i| -1.0 + f64::from(i) * 0.001)
                .max_by(|&a, &b| periods.pdf(a).total_cmp(&periods.pdf(b)))
                .unwrap()
        };
        // Peak separations: 4.5 au for VLM stars and 5.3 au for M dwarfs (Duchêne and Kraus,
        // Table 1), at the anchors' mean system masses.
        let vlm = mode(0.09);
        let m_dwarf = mode(0.25);
        println!("VLM mode 10^{vlm:.3} d, M dwarf mode 10^{m_dwarf:.3} d");
        assert!((vlm - log_period_of(4.5, 0.175)).abs() < 0.01, "{vlm}");
        assert!(
            (m_dwarf - log_period_of(5.3, 0.421)).abs() < 0.02,
            "{m_dwarf}"
        );
        // A stars: 30–45% with a spectroscopic companion (P < 10³ d) and 40 ± 4% with a visual
        // one over 50–2000 au (Duchêne and Kraus §3.4.2), at a companion frequency of 1.
        let a_star = at(2.7);
        let spectroscopic = a_star.cdf(3.0);
        let visual =
            a_star.cdf(log_period_of(2000.0, 3.78)) - a_star.cdf(log_period_of(50.0, 3.78));
        println!("A stars: {spectroscopic:.3} spectroscopic, {visual:.3} visual companions");
        assert!((0.30..=0.45).contains(&spectroscopic), "{spectroscopic}");
        assert!((visual - 0.40).abs() <= 0.04, "{visual}");
        // O stars, counted as the surveys count, q ≥ 0.1: 0.69 companions inside 10^3.5 d (Sana
        // et al. 2012), 30% of stars inside 10 d and 45 ± 5% visual companions over two decades
        // of separation (Duchêne and Kraus §3.5.2–3).
        let o_mass = SolarMasses::new(30.0);
        let o_star = at(30.0);
        let frequency = model.companion_frequency(o_mass);
        let above = |regime| {
            1.0 - model
                .mass_ratio_in(o_mass, regime)
                .cdf(SURVEY_MIN_MASS_RATIO)
        };
        let close_above = above(PeriodRegime::Close);
        let sana = frequency * o_star.cdf(CLOSE_MAX_LOG_PERIOD) * close_above;
        let inside_ten_days = frequency * o_star.cdf(1.0) * close_above;
        let two_decades = frequency
            * (o_star.cdf(log_period_of(3_000.0, 40.5)) - o_star.cdf(log_period_of(30.0, 40.5)))
            * above(PeriodRegime::Wide);
        println!(
            "O stars, q ≥ 0.1: {sana:.3} within 10^3.5 d, {inside_ten_days:.3} within 10 d, \
             {two_decades:.3} over 30–3000 au"
        );
        assert!((sana - O_STAR_CLOSE_SURVEYED).abs() < 1e-12, "{sana}");
        assert!((inside_ten_days - 0.30).abs() < 0.02, "{inside_ten_days}");
        assert!((two_decades - 0.45).abs() <= 0.05, "{two_decades}");
    }

    /// The O stars' shares above q = 0.1 are the ones their literals hold, and the wide count is
    /// Duchêne and Kraus's 1.3 less Sana et al.'s 0.69.
    #[test]
    fn the_o_star_shares_above_a_tenth_are_their_laws() {
        let model = model();
        let o_mass = SolarMasses::new(30.0);
        let above = |regime| {
            1.0 - model
                .mass_ratio_in(o_mass, regime)
                .cdf(SURVEY_MIN_MASS_RATIO)
        };
        assert_same_bits(above(PeriodRegime::Close), O_STAR_CLOSE_SHARE_SURVEYED);
        assert_same_bits(above(PeriodRegime::Wide), O_STAR_WIDE_SHARE_SURVEYED);
        assert!((1.3 - O_STAR_CLOSE_SURVEYED - O_STAR_WIDE_SURVEYED).abs() < 1e-15);
    }

    #[test]
    fn a_restricted_draw_outside_the_support_gives_the_nearer_end() {
        let periods = model().period_distribution(SolarMasses::new(30.0));
        let (lo, hi) = periods.support();
        let mut s = stream(55);
        let below = periods.sample_in(&mut s, Days::new(0.1), Days::new(1.0));
        assert!((math::log10(below.value()) - lo).abs() < 1e-12, "{below:?}");
        let above = periods.sample_in(&mut s, Days::new(1e9), Days::new(1e10));
        assert!((math::log10(above.value()) - hi).abs() < 1e-12, "{above:?}");
        let point = periods.sample_in(&mut s, Days::new(100.0), Days::new(100.0));
        assert!((point.value() - 100.0).abs() < 1e-9, "{point:?}");
    }

    #[test]
    #[should_panic(expected = "sample_in needs 0 < lo ≤ hi")]
    fn a_restricted_draw_needs_an_ordered_range() {
        let periods = model().period_distribution(SolarMasses::new(1.0));
        let _ = periods.sample_in(&mut stream(56), Days::new(10.0), Days::new(1.0));
    }

    #[test]
    #[should_panic(expected = "a mass-ratio law needs a positive primary mass")]
    fn a_mass_ratio_law_needs_a_positive_mass() {
        let _ = model().mass_ratio_distribution(SolarMasses::new(-1.0), Days::new(10.0));
    }

    /// Twins against Raghavan et al.'s 27 like-mass pairs (q > 0.95) of about 250 (§5.3.5), and
    /// against Duchêne and Kraus's excess at q ≥ 0.98 and P ≤ 43 d, 2–3% of spectroscopic
    /// binaries (§5.3), here those under 10⁴ days.
    #[test]
    fn sun_like_twins_match_the_surveys() {
        let model = model();
        let sun = SolarMasses::new(1.0);
        let like_mass = 1.0 - model.companion_mass_ratio_cdf(sun, TWIN_MIN_MASS_RATIO);
        let periods = model.period_distribution(sun);
        let twin_share = model
            .mass_ratio_distribution(sun, Days::new(10.0))
            .twin_share();
        let excess = twin_share * (1.0 - 0.98) / (1.0 - TWIN_MIN_MASS_RATIO)
            * periods.cdf(math::log10(43.0))
            / periods.cdf(4.0);
        println!(
            "Sun-like: {like_mass:.3} of pairs like-mass, twin excess {excess:.4} of binaries \
             under 10⁴ d"
        );
        // 27 of about 248, with a Poisson error of ±5.2 pairs at 2σ.
        assert!((0.067..=0.151).contains(&like_mass), "{like_mass}");
        assert!((0.02..=0.03).contains(&excess), "{excess}");
    }

    #[test]
    fn the_marginal_mass_ratio_law_mixes_the_regimes() {
        let model = model();
        for m1 in mass_sweep() {
            let regimes = model.mass_ratio_regimes(m1);
            let total = regimes.iter().fold(0.0, |sum, (share, _)| sum + share);
            assert!((total - 1.0).abs() < 1e-15, "{total} at {m1:?}");
            assert!(regimes.iter().all(|(share, _)| *share >= 0.0));
            let lo = regimes[0].1.lo();
            assert!(model.companion_mass_ratio_cdf(m1, lo * 0.999).abs() < 1e-15);
            assert_same_bits(model.companion_mass_ratio_cdf(m1, 1.0), 1.0);
            let mean = model.mean_companion_mass_ratio(m1);
            assert!((lo..=1.0).contains(&mean), "{mean} at {m1:?}");
        }
    }

    #[test]
    fn the_substellar_floor_is_thirteen_jupiter_masses() {
        let jupiters = crate::units::JupiterMasses::from(MIN_SUBSTELLAR_COMPANION_MASS).value();
        assert!((jupiters - 13.0).abs() < 1e-12, "{jupiters}");
        assert!((MIN_SUBSTELLAR_COMPANION_MASS.value() - 0.012_41).abs() < 1e-5);
        assert_same_bits(MIN_COMPANION_MASS.value(), 0.08);
    }
}
