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
use crate::galaxy::quad::gl16;
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

/// The period of the eccentricity envelope, P₀ = 2 days: Moe and Di Stefano's (2017, ApJS 230,
/// 15, eq. 3) `e_max(P) = 1 − (P ÷ 2 d)^(−2/3)`, which keeps a pair's Roche-lobe fill factors
/// under about 70% at periastron.
///
/// Every orbit of `P ≥ P₀` therefore has its periastron at or beyond the separation of a
/// circular orbit of 2 days about the same masses. Orbits under [`CIRCULARISATION_PERIOD`] are
/// circular whatever the envelope allows (ruling 37).
pub const ECCENTRICITY_ENVELOPE_PERIOD: Days = Days::new(2.0);

/// The mass ratio above which a pair counts as a twin: Moe and Di Stefano's (2017, §2) excess
/// population lies on q = 0.95–1, as Raghavan et al.'s (2010, §5.3.5) "like-mass pairs (M₂ ÷ M₁
/// > 0.95)" do.
pub(super) const TWIN_MIN_MASS_RATIO: f64 = 0.95;

/// The mass ratio above which Moe and Di Stefano (2017, §2) count the companions that their
/// excess twin fraction is a share of.
const TWIN_REFERENCE_MASS_RATIO: f64 = 0.3;

/// The period, as x = log₁₀(P ÷ 1 d), that divides an O star's close companions from its wide
/// ones in the period law: 3.5, about 3,000 days, the upper edge of the range Sana et al. (2012)
/// fitted, whose sample reaches "up to about nine years".
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
    /// A normal in x of this mean and standard deviation, truncated to `[lo, hi]`.
    LogNormal {
        mean: f64,
        sigma: f64,
        lo: f64,
        hi: f64,
    },
    /// A density ∝ x^`exponent` on `[lo, hi]`, 0 < lo: Sana et al.'s (2012) form for O stars,
    /// and Öpik's law, flat in x, at an exponent of 0.
    LogPowerLaw { exponent: f64, lo: f64, hi: f64 },
    /// A density linear in x between knots `(x, density)`, the density unnormalised: Moe and Di
    /// Stefano's (2017) piecewise fit.
    PiecewiseLinear { knots: &'static [(f64, f64)] },
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

/// The share of the O stars' close companions above q = 0.1, under Moe and Di Stefano's laws at
/// 30 M☉ averaged over Sana et al.'s component, twins included: 0.861. The literal is that
/// share, which a test recomputes.
const O_STAR_CLOSE_SHARE_SURVEYED: f64 = 0.860_919_369_285_465_5;

/// The share of the O stars' wide companions above q = 0.1, under Moe and Di Stefano's laws at
/// 30 M☉ averaged over the Öpik component: 0.607. The literal is that share, which a test
/// recomputes.
const O_STAR_WIDE_SHARE_SURVEYED: f64 = 0.607_158_916_861_864_7;

/// The close companions per O star of every mass ratio the model draws, down to
/// 0.08 M☉ ÷ 30 M☉: the surveyed 0.69 over its law's share above q = 0.1, 0.80.
pub(super) const O_STAR_CLOSE_FREQUENCY: f64 = O_STAR_CLOSE_SURVEYED / O_STAR_CLOSE_SHARE_SURVEYED;

/// The wide companions per O star of every mass ratio the model draws: the surveyed 0.61 over its
/// law's share above q = 0.1, 1.00.
pub(super) const O_STAR_WIDE_FREQUENCY: f64 = O_STAR_WIDE_SURVEYED / O_STAR_WIDE_SHARE_SURVEYED;

/// The share of an O star's companions in Sana et al.'s close component, 0.80 ÷ 1.81 = 0.44.
const O_STAR_CLOSE_WEIGHT: f64 =
    O_STAR_CLOSE_FREQUENCY / (O_STAR_CLOSE_FREQUENCY + O_STAR_WIDE_FREQUENCY);

/// De Rosa et al.'s (2014, MNRAS 437, 1216, §6.4) weighted frequency of spectroscopic companions
/// to A stars, 35.1 ± 6.5%: Abt (1965) for normal A stars, Carquillat and Prieur (2007) for Am
/// stars and Carrier et al. (2002) for Ap stars, weighted by their shares of the VAST sample.
const A_STAR_SPECTROSCOPIC: f64 = 0.351;

/// x = log₁₀(P ÷ 1 d) of a projected separation of 30 au, the inner limit of the VAST survey: the
/// boundary between the A stars' spectroscopic and visual companions. A projected separation is
/// deprojected by +0.13 dex, Duquennoy and Mayor's (1991) statistical relation `log a = log ρ +
/// 0.13`, which Raghavan et al. (2010, §5.3.3) apply to their visual pairs, and turned into a
/// period at the anchor's mean system mass, 3.78 M☉ (2.7 M☉ times 1 plus the mean mass ratio of
/// γ = −0.5 on [0.08 ÷ 2.7, 1], 0.40). The literal is that period, which a test recomputes.
const A_STAR_IMAGING_LOG_PERIOD: f64 = 4.684_526_206_767_216;

/// The A stars' visual companions per star beyond a projected 30 au: De Rosa et al.'s log-normal
/// in the log of the projected separation (peak 2.59 ± 0.13, 387 au; σ 0.79 ± 0.12 dex; §6.2)
/// integrates to 33.8 ± 2.6% over 30–10⁴ au, 0.883 of it, so the whole of it holds 0.383 and the
/// part beyond 30 au 0.352. The literal is that count, which a test recomputes.
const A_STAR_VISUAL: f64 = 0.352_212_663_390_216_9;

/// x = log₁₀(P ÷ 1 d) of De Rosa et al.'s peak, a projected 387 au (10^2.59), deprojected and at
/// 3.78 M☉ as [`A_STAR_IMAGING_LOG_PERIOD`] is. The literal is that period, which a test
/// recomputes.
const A_STAR_VISUAL_LOG_PERIOD: f64 = 6.353_844_324_687_722;

/// The width of De Rosa et al.'s log-normal in x = log₁₀(P ÷ 1 d): 1.5 × 0.79 dex, since P ∝ a^1.5
/// at a fixed mass.
const A_STAR_VISUAL_SIGMA: f64 = 1.5 * 0.79;

/// The shape of the A stars' spectroscopic companions in x = log₁₀(P ÷ 1 d): Moe and Di Stefano's
/// (2017, eqs. 20–23) companion frequency per decade, `f_logP;q>0.3`, at M₁ = 2.7 M☉, from their
/// shortest period, x = 0.2, to [`A_STAR_IMAGING_LOG_PERIOD`]. At 2.7 M☉ their eqs. 20–22 give
/// 0.0503 below x = 1, 0.0711 at 2.7 and 0.0639 at 5.5, and eq. 23 joins them with a rise of 0.018
/// per decade across 2.7 ± 0.7. The literals are those knots, which a test recomputes.
const A_STAR_SPECTROSCOPIC_SHAPE: [(f64, f64); 5] = [
    (0.2, 0.050_279_779_358_418_24),
    (1.0, 0.050_279_779_358_418_24),
    (2.0, 0.058_456_210_461_423_23),
    (3.4, 0.083_656_210_461_423_23),
    (A_STAR_IMAGING_LOG_PERIOD, 0.071_556_334_002_249_08),
];

/// The share of an A star's companions that are spectroscopic, 0.351 ÷ 0.703 = 0.499.
const A_STAR_CLOSE_WEIGHT: f64 = A_STAR_SPECTROSCOPIC / (A_STAR_SPECTROSCOPIC + A_STAR_VISUAL);

/// The period anchors (Design note 3), each a primary mass and its distribution of
/// x = log₁₀(P ÷ 1 d); between two anchors the distribution is their mixture, weighted linearly in
/// ln m, and outside it is the end anchor's. A mixture is the one blend that stays continuous in
/// mass across anchors of different forms (ruling 37), and between two unimodal anchors whose
/// peaks lie apart it is bimodal. Separations are turned into periods by Kepler's third law at
/// the anchor's mean system mass, `m₁ (1 + E[q])`, with the mean mass ratio of
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
/// - **2.7 M☉**: the A stars' two populations as De Rosa et al.'s VAST survey (2014, MNRAS 437,
///   1216) counts them, which is Duchêne and Kraus's bimodal distribution (Table 1 and §3.4.3).
///   Their visual companions, beyond a projected 30 au, follow the survey's log-normal in
///   separation (§6.2: peak 387 au, σ 0.79 dex), deprojected by +0.13 dex and truncated at
///   30 au, 0.352 per star ([`A_STAR_VISUAL`]). Their spectroscopic companions, 0.351 per star
///   (§6.4), lie inside it with the shape of Moe and Di Stefano's (2017, ApJS 230, 15) measured
///   companion frequency per decade at 2.7 M☉ ([`A_STAR_SPECTROSCOPIC_SHAPE`]). The weights are
///   the two counts' shares, 0.499 and 0.501; the anchor's companion frequency stays Duchêne and
///   Kraus's. So 0.55 of the spectroscopic companions lie under 10³ days, there are 0.19
///   companions per star at 1–10 au (0.14 in the survey's own count, whose total is 0.70), where
///   Duchêne and Kraus (§5.1.2) find about the solar-type 10–15%, and the visual part reproduces
///   the survey's 21.9 ± 2.6% over 30–800 au and 33.8 ± 2.6% over 30–10⁴ au.
/// - **30 M☉**: Sana et al.'s (2012) close component, a density ∝ x^−0.55 on x = 0.15–3.5
///   (π = −0.55 ± 0.22) holding 0.69 of the 1.3 companions of q ≥ 0.1 per star, and a wide one
///   flat in x (Öpik's law) from 3.5 to 7.76, 10⁴ au at 40.5 M☉, holding the remaining 0.61:
///   Duchêne and Kraus (§3.5.3) combine "short period binaries (log P ≲ 1, 30% of all high-mass
///   stars) and a power law period distribution extending out to ≳ 10⁴ AU". The weights are
///   those counts extended to every mass ratio the model draws, [`O_STAR_CLOSE_FREQUENCY`] and
///   [`O_STAR_WIDE_FREQUENCY`], 0.44 and 0.56. Counted as the surveys count, q ≥ 0.1, this gives
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
                lo: LOG_PERIOD_MIN,
                hi: LOG_PERIOD_MAX,
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
                lo: LOG_PERIOD_MIN,
                hi: LOG_PERIOD_MAX,
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
                lo: LOG_PERIOD_MIN,
                hi: LOG_PERIOD_MAX,
            },
        )],
    },
    PeriodAnchor {
        mass: 2.7,
        components: &[
            (
                A_STAR_CLOSE_WEIGHT,
                PeriodShape::PiecewiseLinear {
                    knots: &A_STAR_SPECTROSCOPIC_SHAPE,
                },
            ),
            (
                1.0 - A_STAR_CLOSE_WEIGHT,
                PeriodShape::LogNormal {
                    mean: A_STAR_VISUAL_LOG_PERIOD,
                    sigma: A_STAR_VISUAL_SIGMA,
                    lo: A_STAR_IMAGING_LOG_PERIOD,
                    hi: LOG_PERIOD_MAX,
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

/// One anchor of the mass-ratio law below Moe and Di Stefano's range: a primary mass and the
/// exponent γ of `q^γ`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MassRatioAnchor {
    /// The primary mass, M☉.
    mass: f64,
    /// The exponent γ.
    gamma: f64,
}

impl MassRatioAnchor {
    /// The primary mass, M☉.
    #[must_use]
    pub(super) fn mass(&self) -> f64 {
        self.mass
    }
}

/// The mass-ratio anchors below [`MOE_DI_STEFANO_MIN_MASS`] (Design note 3; ruling 41):
/// `f(q) ∝ q^γ` on `[0.08 M☉ ÷ m₁, 1]`, with γ from Duchêne and Kraus (2013, Table 1),
/// re-checked, interpolated linearly in ln m and held below the first:
///
/// | Anchor (M☉) | γ   | Table 1 (and the text)                            |
/// | ----------- | --- | ------------------------------------------------- |
/// | 0.09        | 4.2 | 4.2 ± 1.0 (Burgasser et al. 2006; §3.3.4)         |
/// | 0.25        | 0.4 | 0.4 ± 0.2 (0.39 ± 0.23, Delfosse et al.; §3.2.4)  |
/// | 1.0         | 0.3 | 0.3 ± 0.1 (0.28 ± 0.05, Raghavan et al.; §3.1.4)  |
///
/// The 1 M☉ anchor only shapes the interpolation up to 0.8 M☉, where Moe and Di Stefano's laws
/// take over. No twin excess is added below 0.8 M☉, since no cited measurement gives one there.
pub(super) const MASS_RATIO_ANCHORS: [MassRatioAnchor; 3] = [
    MassRatioAnchor {
        mass: 0.09,
        gamma: 4.2,
    },
    MassRatioAnchor {
        mass: 0.25,
        gamma: 0.4,
    },
    MassRatioAnchor {
        mass: 1.0,
        gamma: 0.3,
    },
];

/// The lowest primary mass, M☉, of Moe and Di Stefano's (2017, ApJS 230, 15) mass-ratio set: the
/// lower edge of their solar-type interval, 0.8–1.2 M☉ (§9.1). From it up the model takes their
/// whole set, both slopes of the broken power law and the excess twins; below it Duchêne and
/// Kraus's single slope stands, with no excess (ruling 41).
pub(super) const MOE_DI_STEFANO_MIN_MASS: f64 = 0.8;

/// The mass ratios where Moe and Di Stefano's power law breaks (§2): `γ_smallq` below 0.3 and
/// `γ_largeq` above; and 0.1, the lowest ratio they measure, below which the model continues with
/// `max(γ_smallq, 0)`.
const MASS_RATIO_BREAKS: [f64; 2] = [0.1, TWIN_REFERENCE_MASS_RATIO];

/// Primary masses, M☉, where Moe and Di Stefano's laws have a kink or a jump: the lower edge of
/// their range; 1.2, 3.5 and 6.0, between which their slopes are interpolated (§9.1); 6.5, where
/// the longest twin period stops falling (eq. 7); and 100 M☉, above which their eq. 6 would fall
/// below zero and is held at zero.
pub(super) const MOE_DI_STEFANO_MASS_KINKS: [f64; 6] =
    [MOE_DI_STEFANO_MIN_MASS, 1.2, 3.5, 6.0, 6.5, 100.0];

/// Periods, as x = log₁₀(P ÷ 1 d), where Moe and Di Stefano's slopes have kinks (eqs. 9–11 and
/// 13–15), with the limits of their range, 0.2 and 8.0, outside which each is held.
pub(super) const MOE_DI_STEFANO_PERIOD_KINKS: [f64; 12] =
    [0.2, 1.0, 2.0, 2.5, 3.0, 4.0, 4.5, 5.0, 5.5, 5.6, 6.5, 8.0];

/// A value interpolated between Moe and Di Stefano's solar-type, A/late-B (3.5 M☉) and early-type
/// (over 6 M☉) fits, linearly in M₁ across 1.2–3.5 and 3.5–6.0 M☉, as their §9.1 does.
#[must_use]
fn by_primary_mass(m: f64, solar: f64, a_late_b: f64, early: f64) -> f64 {
    if m <= 1.2 {
        solar
    } else if m <= 3.5 {
        lerp(solar, a_late_b, (m - 1.2) / 2.3)
    } else if m <= 6.0 {
        lerp(a_late_b, early, (m - 3.5) / 2.5)
    } else {
        early
    }
}

/// Moe and Di Stefano's (2017, eqs. 9–11) slope `γ_largeq` of the mass-ratio law over
/// q = 0.3–1, for a primary of `m` M☉ (0.8 or more) at x = log₁₀(P ÷ 1 d), held outside x =
/// 0.2–8, re-checked against the paper:
///
/// - solar type: −0.5 up to x = 5, then −0.5 − 0.3 (x − 5) (eq. 9);
/// - 3.5 M☉: −0.5 up to 1, −0.5 − 0.2 (x − 1) to 4.5, −1.2 − 0.4 (x − 4.5) to 6.5, then −2.0
///   (eq. 10);
/// - over 6 M☉: −0.5 up to 1, −0.5 − 0.9 (x − 1) to 2, −1.4 − 0.3 (x − 2) to 4, then −2.0 (eq. 11).
///
/// Their 1σ is 0.3 everywhere (eq. 12).
#[must_use]
fn gamma_large(m: f64, log_period: f64) -> f64 {
    let x = log_period.clamp(0.2, 8.0);
    let solar = if x < 5.0 {
        -0.5
    } else {
        -0.5 - 0.3 * (x - 5.0)
    };
    let a_late_b = if x < 1.0 {
        -0.5
    } else if x < 4.5 {
        -0.5 - 0.2 * (x - 1.0)
    } else if x < 6.5 {
        -1.2 - 0.4 * (x - 4.5)
    } else {
        -2.0
    };
    let early = if x < 1.0 {
        -0.5
    } else if x < 2.0 {
        -0.5 - 0.9 * (x - 1.0)
    } else if x < 4.0 {
        -1.4 - 0.3 * (x - 2.0)
    } else {
        -2.0
    };
    by_primary_mass(m, solar, a_late_b, early)
}

/// Moe and Di Stefano's (2017, eqs. 13–15) slope `γ_smallq` of the mass-ratio law over
/// q = 0.1–0.3, for a primary of `m` M☉ (0.8 or more) at x = log₁₀(P ÷ 1 d), held outside x =
/// 0.2–8, re-checked against the paper:
///
/// - solar type: 0.3 (eq. 13);
/// - 3.5 M☉: 0.2 up to 2.5, 0.2 − 0.3 (x − 2.5) to 5.5, then −0.7 − 0.2 (x − 5.5) (eq. 14);
/// - over 6 M☉: 0.1 up to 1, 0.1 − 0.15 (x − 1) to 3, −0.2 − 0.5 (x − 3) to 5.6, then −1.5
///   (eq. 15).
///
/// Their 1σ runs from 0.4 to 0.6 (eq. 16).
#[must_use]
fn gamma_small(m: f64, log_period: f64) -> f64 {
    let x = log_period.clamp(0.2, 8.0);
    let a_late_b = if x < 2.5 {
        0.2
    } else if x < 5.5 {
        0.2 - 0.3 * (x - 2.5)
    } else {
        -0.7 - 0.2 * (x - 5.5)
    };
    let early = if x < 1.0 {
        0.1
    } else if x < 3.0 {
        0.1 - 0.15 * (x - 1.0)
    } else if x < 5.6 {
        -0.2 - 0.5 * (x - 3.0)
    } else {
        -1.5
    };
    by_primary_mass(m, 0.3, a_late_b, early)
}

/// The longest period, as x = log₁₀(P ÷ 1 d), at which a primary of `m1` has excess twins: Moe
/// and Di Stefano's (2017, eq. 7) `8 − M₁ ÷ M☉` up to 6.5 M☉, and 1.5 above; below
/// [`MOE_DI_STEFANO_MIN_MASS`], where there are none, the value there.
#[must_use]
pub(super) fn longest_twin_log_period(m1: SolarMasses) -> f64 {
    let m = m1.value().max(MOE_DI_STEFANO_MIN_MASS);
    if m <= 6.5 { 8.0 - m } else { 1.5 }
}

/// Moe and Di Stefano's (2017, ApJS 230, 15, eqs. 5–7) excess twin fraction `F_twin(M₁, P)`: the
/// share of the companions with q > 0.3 that make an excess population uniform on q = 0.95–1,
/// over their own broken power law beneath it (§2), at x = `log_period` = log₁₀(P ÷ 1 d),
/// re-checked against the paper.
///
/// Below x = 1 it is `F₀ = 0.30 − 0.15 log₁₀(M₁ ÷ M☉)` (eq. 6); it falls linearly in x to zero at
/// [`longest_twin_log_period`] (eq. 5), `8 − M₁ ÷ M☉` up to 6.5 M☉ and 1.5 above (eq. 7), and is
/// zero beyond. Their 1σ uncertainty is max(0.03, 0.3 F) (eq. 8). So a Sun-like pair under
/// 10 days has 0.30, falling to none at 10⁷ days, and an O star's (28 M☉) 0.08, gone by 30 days,
/// against Sana et al.'s (2012) finding of no separate twin population. It is zero below
/// [`MOE_DI_STEFANO_MIN_MASS`], outside their range (ruling 41), and F₀, which their slope would
/// take below zero above 100 M☉, is held at zero.
#[must_use]
pub(super) fn excess_twin_fraction(m1: SolarMasses, log_period: f64) -> f64 {
    let m = m1.value();
    if m < MOE_DI_STEFANO_MIN_MASS {
        return 0.0;
    }
    let short = (0.30 - 0.15 * math::log10(m)).max(0.0);
    let longest = longest_twin_log_period(m1);
    if log_period < 1.0 {
        short
    } else if log_period < longest {
        short * (1.0 - (log_period - 1.0) / (longest - 1.0))
    } else {
        0.0
    }
}

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

/// A density linear in x between knots, normalised over them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Polyline {
    /// The knots `(x, density)`, x strictly increasing, densities unnormalised and not negative.
    knots: &'static [(f64, f64)],
    /// The area under the knots.
    total: f64,
}

impl Polyline {
    #[must_use]
    fn new(knots: &'static [(f64, f64)]) -> Self {
        debug_assert!(knots.len() >= 2, "a polyline needs two knots");
        let total = knots.windows(2).fold(0.0, |sum, pair| {
            let ((x0, f0), (x1, f1)) = (pair[0], pair[1]);
            sum + f0.midpoint(f1) * (x1 - x0)
        });
        Self { knots, total }
    }

    #[must_use]
    fn support(&self) -> (f64, f64) {
        (self.knots[0].0, self.knots[self.knots.len() - 1].0)
    }

    #[must_use]
    fn cdf_pdf(&self, x: f64) -> (f64, f64) {
        let (lo, hi) = self.support();
        if x < lo {
            return (0.0, 0.0);
        }
        if x > hi {
            return (1.0, 0.0);
        }
        let mut below = 0.0;
        for pair in self.knots.windows(2) {
            let ((x0, f0), (x1, f1)) = (pair[0], pair[1]);
            if x <= x1 {
                let f = f0 + (f1 - f0) * (x - x0) / (x1 - x0);
                let area = below + f0.midpoint(f) * (x - x0);
                return ((area / self.total).clamp(0.0, 1.0), f / self.total);
            }
            below += f0.midpoint(f1) * (x1 - x0);
        }
        (1.0, 0.0)
    }

    /// The inverse of the cumulative distribution: the segment that holds the share `u` of the
    /// area, then, for a share `v` of that segment's area, `t = v (f₀ + f₁) ÷ (f₀ + √(f₀² +
    /// v (f₁² − f₀²)))` of its width, the rationalised root of its quadratic cumulative
    /// distribution (as [`PiecewiseLinear`](crate::rng::PiecewiseLinear) samples).
    #[must_use]
    fn quantile(&self, u: f64) -> f64 {
        let target = u.clamp(0.0, 1.0) * self.total;
        let mut below = 0.0;
        let last = self.knots.len() - 2;
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
        self.support().1
    }
}

/// One built component of a [`PeriodDistribution`].
#[derive(Debug, Clone, Copy, PartialEq)]
enum Component {
    Normal(TruncatedNormal),
    Power(PowerLaw),
    Linear(Polyline),
}

impl Component {
    #[must_use]
    fn new(shape: PeriodShape) -> Self {
        match shape {
            PeriodShape::LogNormal {
                mean,
                sigma,
                lo,
                hi,
            } => Self::Normal(TruncatedNormal::new(mean, sigma, lo, hi)),
            PeriodShape::LogPowerLaw { exponent, lo, hi } => Self::Power(
                PowerLaw::new(-exponent, lo, hi).expect("the anchors' power laws are valid"),
            ),
            PeriodShape::PiecewiseLinear { knots } => Self::Linear(Polyline::new(knots)),
        }
    }

    #[must_use]
    fn cdf_pdf(&self, x: f64) -> (f64, f64) {
        match self {
            Self::Normal(n) => n.cdf_pdf(x),
            Self::Power(p) => (p.cdf(x), p.pdf(x)),
            Self::Linear(l) => l.cdf_pdf(x),
        }
    }

    #[must_use]
    fn quantile(&self, u: f64) -> f64 {
        match self {
            Self::Normal(n) => n.quantile(u),
            Self::Power(p) => p.quantile(u),
            Self::Linear(l) => l.quantile(u),
        }
    }

    #[must_use]
    fn support(&self) -> (f64, f64) {
        match self {
            Self::Normal(n) => (n.lo, n.hi),
            Self::Power(p) => (p.lo(), p.hi()),
            Self::Linear(l) => l.support(),
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

    /// Every point where a component's density jumps or has a kink, in x = log₁₀(P ÷ 1 d): each
    /// component's limits and a piecewise-linear one's knots, for quadratures that put panel
    /// edges there.
    #[must_use]
    pub(super) fn component_limits(&self) -> Vec<f64> {
        let mut limits = Vec::with_capacity(4 * MAX_PERIOD_COMPONENTS);
        for (_, component) in self.used() {
            match component {
                Component::Linear(line) => limits.extend(line.knots.iter().map(|&(x, _)| x)),
                Component::Normal(_) | Component::Power(_) => {
                    let (lo, hi) = component.support();
                    limits.extend([lo, hi]);
                }
            }
        }
        limits
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
        self.quantile_in(stream.uniform_open(), lo, hi)
    }

    /// The period below which a share `u` of the distribution restricted to `[lo, hi]` lies, for
    /// `u` in [0, 1]: the inverse transform [`sample_in`](Self::sample_in) applies to its uniform,
    /// with the same clamping, for a caller that holds its own variate (the hierarchy draw, whose
    /// one mark picks a node and a period together).
    ///
    /// # Panics
    ///
    /// Unless `0 < lo ≤ hi`, both finite.
    pub(super) fn quantile_in(&self, u: f64, lo: Days, hi: Days) -> Days {
        assert!(
            lo.value() > 0.0 && lo <= hi && hi.value().is_finite(),
            "quantile_in needs 0 < lo ≤ hi, got {lo:?} and {hi:?}"
        );
        let (s_lo, s_hi) = self.support();
        let a = math::log10(lo.value()).clamp(s_lo, s_hi);
        let b = math::log10(hi.value()).clamp(s_lo, s_hi);
        let (f_a, f_b) = (self.cdf(a), self.cdf(b));
        let p = f_a + u * (f_b - f_a);
        Days::new(math::exp10(self.quantile_between(p, a, b)))
    }

    /// The share of the distribution inside `[lo, hi]` after the clamping that
    /// [`sample_in`](Self::sample_in) applies: its weight as one window of a mixture of
    /// restricted draws. Zero for a range outside the support or empty after clamping.
    ///
    /// # Panics
    ///
    /// Unless `0 < lo ≤ hi`, both finite.
    pub(super) fn share_in(&self, lo: Days, hi: Days) -> f64 {
        assert!(
            lo.value() > 0.0 && lo <= hi && hi.value().is_finite(),
            "share_in needs 0 < lo ≤ hi, got {lo:?} and {hi:?}"
        );
        let (s_lo, s_hi) = self.support();
        let a = math::log10(lo.value()).clamp(s_lo, s_hi);
        let b = math::log10(hi.value()).clamp(s_lo, s_hi);
        if b > a {
            (self.cdf(b) - self.cdf(a)).max(0.0)
        } else {
            0.0
        }
    }
}

/// `∫ f(x) dx` over x = log₁₀(P ÷ 1 d) from `lo` to `hi`, by 16-point Gauss–Legendre on panels
/// between `lo`, `hi`, every limit of a component of `periods` (where its density may jump) and
/// `kinks`, each split into equal parts no wider than `max_panel`. Fewer than one panel gives 0.
#[must_use]
pub(super) fn integrate_log_period(
    periods: &PeriodDistribution,
    lo: f64,
    hi: f64,
    kinks: &[f64],
    max_panel: f64,
    mut f: impl FnMut(f64) -> f64,
) -> f64 {
    if hi <= lo {
        return 0.0;
    }
    let mut edges: Vec<f64> = periods
        .component_limits()
        .into_iter()
        .chain(kinks.iter().copied())
        .filter(|&x| x > lo && x < hi)
        .collect();
    edges.push(lo);
    edges.push(hi);
    edges.sort_by(f64::total_cmp);
    edges.dedup();
    let mut sum = 0.0;
    for pair in edges.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let mut pieces = 1_u32;
        while (end - start) / f64::from(pieces) > max_panel {
            pieces += 1;
        }
        let step = (end - start) / f64::from(pieces);
        for i in 0..pieces {
            let a = start + step * f64::from(i);
            let b = if i + 1 == pieces { end } else { a + step };
            sum += gl16(&mut f, a, b);
        }
    }
    sum
}

/// `∫ qᵏ dq` over `[a, b]`, for `0 < a ≤ b`: `b^(k+1) (1 − (a ÷ b)^(k+1)) ÷ (k + 1)`, or
/// `ln(b ÷ a)` at k = −1.
#[must_use]
fn power_integral(k: f64, a: f64, b: f64) -> f64 {
    let g = k + 1.0;
    let log_ratio = math::ln(a / b);
    if g.abs() < 1e-8 {
        -log_ratio
    } else {
        -math::powf(b, g) * math::exp_m1(g * log_ratio) / g
    }
}

/// The most segments of a mass-ratio law's broken power law: below 0.1, 0.1–0.3 and above 0.3.
const MAX_MASS_RATIO_SEGMENTS: usize = 3;

/// One segment of a broken power law in q: `c q^γ` on `[lo, hi]`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Segment {
    /// The density `q^γ` normalised on the segment.
    power: PowerLaw,
    /// The segment's share of the whole law's area.
    share: f64,
}

/// The distribution of a companion's mass ratio q = m₂ ÷ m₁ for one primary mass and period
/// ([`MultiplicityModel::mass_ratio_distribution`]): a power law on `[0.08 M☉ ÷ m₁, 1]`, continuous
/// and broken at q = 0.1 and 0.3 for primaries of 0.8 M☉ or more, with a share of twins uniform on
/// [0.95, 1] there.
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
#[derive(Debug, Clone, PartialEq)]
pub struct MassRatioDistribution {
    /// The lowest mass ratio, 0.08 M☉ ÷ m₁, or 1 when that is at least 1.
    lo: f64,
    /// The smooth law's segments in ascending q; the first `len` are in use, none when the range
    /// is empty.
    segments: [Segment; MAX_MASS_RATIO_SEGMENTS],
    len: usize,
    /// The weight of the twins in the mixture.
    twin_share: f64,
    /// The twins' lowest mass ratio, `max(0.95, lo)`.
    twin_lo: f64,
}

impl MassRatioDistribution {
    /// The smooth law on `[lo, 1]`, without twins: `q^γᵢ` on successive segments ending at each
    /// `(end, γᵢ)` of `pieces` (the last ending at 1), continuous at every break; segments that
    /// end at or below `lo` are left out.
    #[must_use]
    fn smooth(lo: f64, pieces: &[(f64, f64)]) -> Self {
        let placeholder = Segment {
            power: PowerLaw::new(0.0, 0.5, 1.0).expect("a flat law on [0.5, 1] is valid"),
            share: 0.0,
        };
        let mut segments = [placeholder; MAX_MASS_RATIO_SEGMENTS];
        if lo >= 1.0 {
            return Self {
                lo: 1.0,
                segments,
                len: 0,
                twin_share: 0.0,
                twin_lo: 1.0,
            };
        }
        let mut len = 0;
        let mut start = lo;
        // The density's coefficient on the current segment, and its areas, for continuity.
        let mut coefficient = 1.0;
        let mut areas = [0.0; MAX_MASS_RATIO_SEGMENTS];
        let mut previous: Option<f64> = None;
        for &(end, gamma) in pieces {
            if end <= start {
                continue;
            }
            if let Some(before) = previous {
                // c₁ start^γ₀ = c₂ start^γ₁ at the break.
                coefficient *= math::powf(start, before - gamma);
            }
            let power =
                PowerLaw::new(-gamma, start, end).expect("q^γ on a segment of [lo, 1] is valid");
            areas[len] = coefficient * power.integral();
            segments[len] = Segment { power, share: 0.0 };
            len += 1;
            previous = Some(gamma);
            start = end;
        }
        let total = areas[..len].iter().fold(0.0, |sum, a| sum + a);
        for (segment, area) in segments[..len].iter_mut().zip(areas) {
            segment.share = area / total;
        }
        Self {
            lo,
            segments,
            len,
            twin_share: 0.0,
            twin_lo: TWIN_MIN_MASS_RATIO.max(lo),
        }
    }

    fn used(&self) -> &[Segment] {
        &self.segments[..self.len]
    }

    /// The smooth law's cumulative distribution and density at `q`, without the twins.
    #[must_use]
    fn smooth_cdf_pdf(&self, q: f64) -> (f64, f64) {
        let mut below = 0.0;
        for segment in self.used() {
            if q <= segment.power.hi() {
                let cdf = below + segment.share * segment.power.cdf(q);
                return (cdf.clamp(0.0, 1.0), segment.share * segment.power.pdf(q));
            }
            below += segment.share;
        }
        (1.0, 0.0)
    }

    /// The smooth law's quantile of `u`, without the twins.
    #[must_use]
    fn smooth_quantile(&self, u: f64) -> f64 {
        let mut below = 0.0;
        let last = self.len.saturating_sub(1);
        for (i, segment) in self.used().iter().enumerate() {
            if u <= below + segment.share || i == last {
                let v = if segment.share > 0.0 {
                    ((u - below) / segment.share).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                return segment.power.quantile(v);
            }
            below += segment.share;
        }
        1.0
    }

    /// The weight in this law's mixture of Moe and Di Stefano's excess twin fraction `excess`,
    /// which they count among companions with q > 0.3: the w with `w ÷ (w + (1 − w) S) =
    /// excess`, where S is the smooth law's share above 0.3, so `w = excess S ÷ (1 − excess +
    /// excess S)`. For a primary of 0.08 M☉ or less there are no twins to weigh.
    #[must_use]
    pub(super) fn twin_weight(&self, excess: f64) -> f64 {
        if self.len == 0 {
            return 0.0;
        }
        let above = 1.0 - self.smooth_cdf_pdf(TWIN_REFERENCE_MASS_RATIO).0;
        excess * above / (1.0 - excess + excess * above)
    }

    /// This law with twins of Moe and Di Stefano's excess fraction `excess` added.
    #[must_use]
    fn with_twins(mut self, excess: f64) -> Self {
        self.twin_share = self.twin_weight(excess);
        self
    }

    /// The lowest mass ratio: 0.08 M☉ ÷ m₁, or 1 for a primary of 0.08 M☉ or less.
    #[must_use]
    pub fn lo(&self) -> f64 {
        self.lo
    }

    /// The weight of the twins, uniform on [0.95, 1], in the law's mixture: Moe and Di Stefano's
    /// excess twin fraction for this primary and period, reweighed from companions of q > 0.3 to
    /// all the law's companions; 0 below 0.8 M☉.
    #[must_use]
    pub fn twin_share(&self) -> f64 {
        self.twin_share
    }

    /// The same law without its twins.
    #[cfg(test)]
    #[must_use]
    pub(super) fn smooth_part(&self) -> Self {
        Self {
            twin_share: 0.0,
            ..self.clone()
        }
    }

    /// The twins' part of the density at `q`, before their weight.
    #[must_use]
    fn twin_pdf(&self, q: f64) -> f64 {
        if self.twin_lo < 1.0 && (self.twin_lo..=1.0).contains(&q) {
            1.0 / (1.0 - self.twin_lo)
        } else {
            0.0
        }
    }

    /// The twins' part of the cumulative distribution at `q`, before their weight.
    #[must_use]
    fn twin_cdf(&self, q: f64) -> f64 {
        if self.twin_lo >= 1.0 {
            return if q >= 1.0 { 1.0 } else { 0.0 };
        }
        ((q - self.twin_lo) / (1.0 - self.twin_lo)).clamp(0.0, 1.0)
    }

    /// The density at `q`; 0 everywhere for a primary of 0.08 M☉ or less, whose companions all
    /// have q = 1.
    #[must_use]
    pub fn pdf(&self, q: f64) -> f64 {
        if self.len == 0 {
            return 0.0;
        }
        let w = self.twin_share;
        (1.0 - w) * self.smooth_cdf_pdf(q).1 + w * self.twin_pdf(q)
    }

    /// The probability that the mass ratio is at most `q`.
    #[must_use]
    pub fn cdf(&self, q: f64) -> f64 {
        if self.len == 0 {
            return if q >= 1.0 { 1.0 } else { 0.0 };
        }
        let w = self.twin_share;
        ((1.0 - w) * self.smooth_cdf_pdf(q).0 + w * self.twin_cdf(q)).clamp(0.0, 1.0)
    }

    /// The mean mass ratio.
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.len == 0 {
            return 1.0;
        }
        let smooth = self.used().iter().fold(0.0, |sum, segment| {
            let (a, b) = (segment.power.lo(), segment.power.hi());
            let gamma = -segment.power.exponent();
            let mean = power_integral(gamma + 1.0, a, b) / power_integral(gamma, a, b);
            sum + segment.share * mean
        });
        let w = self.twin_share;
        (1.0 - w) * smooth + w * self.twin_lo.midpoint(1.0)
    }

    /// The mass ratio below which a share `u` of companions lies, for `u` in [0, 1]: the inverse
    /// of [`cdf`](Self::cdf), in closed form below the twins and by a fixed number of Newton steps
    /// among them.
    #[must_use]
    pub fn quantile(&self, u: f64) -> f64 {
        if self.len == 0 {
            return 1.0;
        }
        let u = u.clamp(0.0, 1.0);
        let w = self.twin_share;
        if w <= 0.0 {
            return self.smooth_quantile(u);
        }
        let below_twins = (1.0 - w) * self.smooth_cdf_pdf(self.twin_lo).0;
        if u <= below_twins {
            return self.smooth_quantile(u / (1.0 - w)).min(self.twin_lo);
        }
        let eval = |q: f64| {
            let (cdf, pdf) = self.smooth_cdf_pdf(q);
            (
                (1.0 - w) * cdf + w * self.twin_cdf(q),
                (1.0 - w) * pdf + w * self.twin_pdf(q),
            )
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
/// law. The cap is Moe and Di Stefano's (2017, ApJS 230, 15, eq. 3) envelope, `e_max = 1 −
/// (P ÷ 2 d)^(−2/3)`, which keeps the pair's Roche-lobe fill factors under about 70% at
/// periastron and so the periastron `a (1 − e)` at or beyond the separation of a circular 2-day
/// orbit about the same masses ([`ECCENTRICITY_ENVELOPE_PERIOD`]; ruling 37). It is the rising
/// upper envelope of the period–eccentricity plane that Duchêne and Kraus describe, and at
/// 12 days it already allows 0.70. Under the circularisation period every orbit is circular,
/// which is a separate, tidal statement.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::multiplicity::MultiplicityModel;
/// use hyperion_sim::units::Days;
///
/// let model = MultiplicityModel::default_v1();
/// assert_eq!(model.eccentricity_distribution(Days::new(5.0)).e_max(), 0.0);
/// // Eight times the envelope's period: the separation is 4 times a 2-day orbit's.
/// let wide = model.eccentricity_distribution(Days::new(16.0));
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
        let ratio = period / ECCENTRICITY_ENVELOPE_PERIOD;
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
    /// | 2.7    | 0.50 × Moe and Di Stefano's shape on 0.2–4.68 + 0.50 × normal (6.35, 1.19)   |
    /// | 30     | 0.44 × x^−0.55 on 0.15–3.5 + 0.56 × flat on 3.5–7.76                        |
    ///
    /// The first three normals are truncated to [`LOG_PERIOD_MIN`]–[`LOG_PERIOD_MAX`], the A
    /// stars' visual one to 4.68–[`LOG_PERIOD_MAX`]. The sources and the corrections to the plan's
    /// figures are in this module's anchor table.
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

    /// The smooth mass-ratio law for a primary of `m1` at x = `log_period` = log₁₀(P ÷ 1 d),
    /// without twins: below [`MOE_DI_STEFANO_MIN_MASS`], Duchêne and Kraus's `q^γ` with γ from
    /// [`MASS_RATIO_ANCHORS`]; from it up, Moe and Di Stefano's broken power law, `γ_smallq` on
    /// q = 0.1–0.3 and `γ_largeq` above (their eqs. 9–11 and 13–15; [`gamma_small`],
    /// [`gamma_large`]), continued below 0.1 with `max(γ_smallq, 0)`.
    ///
    /// Their laws are measured down to q = 0.1. Below it the model neither lets their negative
    /// small-q slopes rise towards extreme ratios nor stops: a slope that rose over the 1.5 decades
    /// down to 0.08 M☉ ÷ 30 M☉ would, with the surveyed counts above q = 0.1 held, give an O star
    /// more companions than [`MAX_COMPANIONS`](super::MAX_COMPANIONS) allows, and Duchêne and Kraus
    /// (§5.1.3, §5.4) find a deficit of extreme mass ratios, not an excess.
    ///
    /// # Panics
    ///
    /// If `m1` is not positive and finite.
    #[must_use]
    pub(super) fn smooth_mass_ratio(
        &self,
        m1: SolarMasses,
        log_period: f64,
    ) -> MassRatioDistribution {
        assert!(
            m1.value() > 0.0 && m1.value().is_finite(),
            "a mass-ratio law needs a positive primary mass, got {m1:?}"
        );
        let lo = MIN_COMPANION_MASS / m1;
        let m = m1.value();
        if m < MOE_DI_STEFANO_MIN_MASS {
            let anchors = self.mass_ratio_anchors();
            let (i, t) = blend(anchors, |a| a.mass, m);
            let gamma = lerp(anchors[i].gamma, anchors[i + 1].gamma, t);
            return MassRatioDistribution::smooth(lo, &[(1.0, gamma)]);
        }
        let small = gamma_small(m, log_period);
        let large = gamma_large(m, log_period);
        let [extreme_end, small_end] = MASS_RATIO_BREAKS;
        MassRatioDistribution::smooth(
            lo,
            &[
                (extreme_end, small.max(0.0)),
                (small_end, small),
                (1.0, large),
            ],
        )
    }

    /// The mass-ratio law with its twins at x = log₁₀(P ÷ 1 d).
    #[must_use]
    pub(super) fn mass_ratio_at(&self, m1: SolarMasses, log_period: f64) -> MassRatioDistribution {
        self.smooth_mass_ratio(m1, log_period)
            .with_twins(excess_twin_fraction(m1, log_period))
    }

    /// The distribution of the mass ratio q = m₂ ÷ m₁ of a companion of a primary of initial mass
    /// `m1` on an orbit of `period`, on `[0.08 M☉ ÷ m₁, 1]`: below 0.8 M☉ Duchêne and Kraus's
    /// (2013, Table 1) `q^γ`; from 0.8 M☉ up Moe and Di Stefano's (2017) whole mass-ratio set,
    /// their broken power law by mass and period and their excess twins on [0.95, 1]
    /// (`excess_twin_fraction`), so that twins are counted once (ruling 41).
    ///
    /// # Panics
    ///
    /// If `m1` is not positive and finite.
    #[must_use]
    pub fn mass_ratio_distribution(&self, m1: SolarMasses, period: Days) -> MassRatioDistribution {
        self.mass_ratio_at(m1, math::log10(period.value()))
    }

    /// The periods, as x = log₁₀(P ÷ 1 d), where a primary of `m1`'s mass-ratio law has a kink:
    /// Moe and Di Stefano's slopes' and their twins' (x = 1 and the longest twin period).
    #[must_use]
    pub(super) fn mass_ratio_period_kinks(m1: SolarMasses) -> [f64; 13] {
        let mut kinks = [0.0; 13];
        kinks[..12].copy_from_slice(&MOE_DI_STEFANO_PERIOD_KINKS);
        kinks[12] = longest_twin_log_period(m1);
        kinks
    }

    /// `∫ f_P(x) g(law at x) dx ÷ ∫ f_P(x) dx` over the periods of a primary of `m1`, the
    /// mass-ratio law taken at each period, by [`integrate_log_period`] on panels no wider than a
    /// decade with edges at the law's kinks; the ratio makes a constant `g` exact. Below 0.8 M☉,
    /// where the law does not depend on the period, it is `g` of that law.
    #[must_use]
    fn over_periods(&self, m1: SolarMasses, g: impl Fn(&MassRatioDistribution) -> f64) -> f64 {
        if m1.value() < MOE_DI_STEFANO_MIN_MASS {
            return g(&self.mass_ratio_at(m1, 0.0));
        }
        let periods = self.period_distribution(m1);
        let (lo, hi) = periods.support();
        let kinks = Self::mass_ratio_period_kinks(m1);
        let value = integrate_log_period(&periods, lo, hi, &kinks, 1.0, |x| {
            periods.pdf(x) * g(&self.mass_ratio_at(m1, x))
        });
        let total = integrate_log_period(&periods, lo, hi, &kinks, 1.0, |x| periods.pdf(x));
        value / total
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
        if q >= 1.0 {
            return 1.0;
        }
        if q < MIN_COMPANION_MASS / m1 {
            return 0.0;
        }
        self.over_periods(m1, |law| law.cdf(q)).clamp(0.0, 1.0)
    }

    /// The mean mass ratio of a companion of a primary of initial mass `m1`, over every period.
    ///
    /// # Panics
    ///
    /// If `m1` is not positive and finite.
    #[must_use]
    pub fn mean_companion_mass_ratio(&self, m1: SolarMasses) -> f64 {
        self.over_periods(m1, MassRatioDistribution::mean)
    }

    /// The distribution of a companion orbit's eccentricity at `period`, the same for every
    /// primary mass (Duchêne and Kraus 2013, §5.1.4: "remarkably little dependency on primary
    /// mass").
    #[must_use]
    pub fn eccentricity_distribution(&self, period: Days) -> EccentricityDistribution {
        EccentricityDistribution::for_period(period)
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

    /// Duquennoy and Mayor's (1991) deprojection of a projected separation, dex.
    const DEPROJECTION_DEX: f64 = 0.13;

    /// The A-star anchor's mean system mass, M☉.
    const A_STAR_SYSTEM_MASS: f64 = 3.78;

    /// x = log₁₀(P ÷ 1 d) of a relative orbit of `a_au` about the A-star anchor's system mass.
    fn a_star_log_period(a_au: f64) -> f64 {
        log_period_of(a_au, A_STAR_SYSTEM_MASS)
    }

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
    fn eccentric_orbits_keep_periastron_outside_a_two_day_orbit() {
        let model = model();
        let mut s = stream(51);
        for i in 0_u32..=80 {
            let period = Days::new(math::exp10(-1.0 + f64::from(i) * 0.15));
            let law = model.eccentricity_distribution(period);
            let floor = if period < CIRCULARISATION_PERIOD {
                1.0
            } else {
                math::powf(ECCENTRICITY_ENVELOPE_PERIOD / period, 2.0 / 3.0)
            };
            for _ in 0..500 {
                let e = law.sample(&mut s);
                assert!((0.0..1.0).contains(&e), "e = {e} at {period:?}");
                // a (1 − e) ≥ a(2 d), with a ∝ P^(2/3) at fixed masses.
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
            let mut edges = periods.component_limits();
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
                // The twins' density jumps at 0.95 and the power law breaks at 0.1 and 0.3, so the
                // integral has edges there.
                let edge = TWIN_MIN_MASS_RATIO.max(law.lo());
                let mut edges: Vec<f64> = [0.1, 0.3, edge]
                    .into_iter()
                    .filter(|&q| q > law.lo() && q < 1.0)
                    .collect();
                edges.push(law.lo());
                edges.push(1.0);
                edges.sort_by(f64::total_cmp);
                edges.dedup();
                let over = |f: &dyn Fn(f64) -> f64| {
                    edges
                        .windows(2)
                        .fold(0.0, |sum, pair| sum + integral(f, pair[0], pair[1]))
                };
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
                let middle = law.lo().midpoint(edges[1]);
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
        // A stars, in De Rosa et al.'s (2014) own count: 35.1% spectroscopic companions inside
        // a projected 30 au, and 21.9 ± 2.6% and 33.8 ± 2.6% visual ones over 30–800 and 30–10⁴ au
        // (Table 8); the 1–10 au share, which Duchêne and Kraus (§5.1.2) expect near the
        // solar-type 10–15%, is recorded.
        let a_star = at(2.7);
        let counted = A_STAR_SPECTROSCOPIC + A_STAR_VISUAL;
        let x = |a: f64| a_star_log_period(a * math::exp10(DEPROJECTION_DEX));
        let inside = counted * a_star.cdf(A_STAR_IMAGING_LOG_PERIOD);
        let to_800 = counted * (a_star.cdf(x(800.0)) - a_star.cdf(x(30.0)));
        let to_10_000 = counted * (a_star.cdf(x(1e4)) - a_star.cdf(x(30.0)));
        let one_to_ten = a_star.cdf(a_star_log_period(10.0)) - a_star.cdf(a_star_log_period(1.0));
        let under_thousand_days = a_star.cdf(3.0) / a_star.cdf(A_STAR_IMAGING_LOG_PERIOD);
        println!(
            "A stars: {inside:.3} spectroscopic, {to_800:.3} over 30–800 au, {to_10_000:.3} over \
             30–10⁴ au; {one_to_ten:.3} per companion at 1–10 au ({:.3} in the survey's count); \
             {under_thousand_days:.3} of the spectroscopic ones under 10³ d",
            counted * one_to_ten
        );
        assert!((inside - A_STAR_SPECTROSCOPIC).abs() < 1e-12, "{inside}");
        assert!((to_800 - 0.219).abs() <= 0.026, "{to_800}");
        assert!((to_10_000 - 0.338).abs() <= 0.026, "{to_10_000}");
        assert!((0.18..=0.21).contains(&one_to_ten), "{one_to_ten}");
        // O stars, counted as the surveys count, q ≥ 0.1: 0.69 companions inside 10^3.5 d (Sana
        // et al. 2012), 30% of stars inside 10 d and 45 ± 5% visual companions over two decades
        // of separation (Duchêne and Kraus §3.5.2–3).
        let o_mass = SolarMasses::new(30.0);
        let o_star = at(30.0);
        let frequency = model.companion_frequency(o_mass);
        let close_above = surveyed_share(&model, o_mass, (0.15, CLOSE_MAX_LOG_PERIOD));
        let sana = frequency * o_star.cdf(CLOSE_MAX_LOG_PERIOD) * close_above;
        let inside_ten_days = frequency * o_star.cdf(1.0) * close_above;
        let two_decades = frequency
            * (o_star.cdf(log_period_of(3_000.0, 40.5)) - o_star.cdf(log_period_of(30.0, 40.5)))
            * surveyed_share(&model, o_mass, (CLOSE_MAX_LOG_PERIOD, 7.76));
        println!(
            "O stars, q ≥ 0.1: {sana:.3} within 10^3.5 d, {inside_ten_days:.3} within 10 d, \
             {two_decades:.3} over 30–3000 au"
        );
        assert!((sana - O_STAR_CLOSE_SURVEYED).abs() < 1e-12, "{sana}");
        assert!((inside_ten_days - 0.30).abs() < 0.02, "{inside_ten_days}");
        assert!((two_decades - 0.45).abs() <= 0.05, "{two_decades}");
    }

    /// The share above q = 0.1, twins included, of the companions whose periods lie in `range`
    /// of x = log₁₀(P ÷ 1 d), as the surveys of massive stars count them.
    fn surveyed_share(model: &MultiplicityModel, m1: SolarMasses, (lo, hi): (f64, f64)) -> f64 {
        let periods = model.period_distribution(m1);
        let kinks = MultiplicityModel::mass_ratio_period_kinks(m1);
        let above = integrate_log_period(&periods, lo, hi, &kinks, 1.0, |x| {
            periods.pdf(x) * (1.0 - model.mass_ratio_at(m1, x).cdf(SURVEY_MIN_MASS_RATIO))
        });
        above / integrate_log_period(&periods, lo, hi, &kinks, 1.0, |x| periods.pdf(x))
    }

    /// The O stars' shares above q = 0.1 are the ones their literals hold, and the wide count is
    /// Duchêne and Kraus's 1.3 less Sana et al.'s 0.69.
    #[test]
    fn the_o_star_shares_above_a_tenth_are_their_laws() {
        let model = model();
        let o_mass = SolarMasses::new(30.0);
        let close = surveyed_share(&model, o_mass, (0.15, CLOSE_MAX_LOG_PERIOD));
        let wide = surveyed_share(&model, o_mass, (CLOSE_MAX_LOG_PERIOD, 7.76));
        println!("O stars above q = 0.1: close {close:?}, wide {wide:?}");
        assert!(
            (close - O_STAR_CLOSE_SHARE_SURVEYED).abs() < 1e-14,
            "{close:?}"
        );
        assert!(
            (wide - O_STAR_WIDE_SHARE_SURVEYED).abs() < 1e-14,
            "{wide:?}"
        );
        assert!((1.3 - O_STAR_CLOSE_SURVEYED - O_STAR_WIDE_SURVEYED).abs() < 1e-15);
    }

    /// The A-star anchor's literals are what their documentation derives them from: De Rosa et
    /// al.'s log-normal and counts, the 30 au boundary, and Moe and Di Stefano's eqs. 20–23 at
    /// 2.7 M☉.
    #[test]
    fn the_a_star_literals_are_their_derivations() {
        let z = |log_a: f64| (log_a - 2.59) / 0.79;
        let observed = normal_cdf(z(4.0)) - normal_cdf(z(math::log10(30.0)));
        let visual = 0.338 / observed * (1.0 - normal_cdf(z(math::log10(30.0))));
        assert!((visual - A_STAR_VISUAL).abs() < 1e-14, "{visual:?}");
        let boundary = a_star_log_period(30.0 * math::exp10(DEPROJECTION_DEX));
        assert!(
            (boundary - A_STAR_IMAGING_LOG_PERIOD).abs() < 1e-12,
            "{boundary:?}"
        );
        let peak = a_star_log_period(math::exp10(2.59 + DEPROJECTION_DEX));
        assert!((peak - A_STAR_VISUAL_LOG_PERIOD).abs() < 1e-12, "{peak:?}");
        let l = math::log10(2.7);
        let short = 0.020 + 0.04 * l + 0.07 * l * l;
        let middle = 0.039 + 0.07 * l + 0.01 * l * l;
        let long = 0.078 - 0.05 * l + 0.04 * l * l;
        let (alpha, delta) = (0.018, 0.7);
        // Eq. 23 of Moe and Di Stefano (2017).
        let f = |x: f64| {
            if x < 1.0 {
                short
            } else if x < 2.7 - delta {
                short + (x - 1.0) / (1.7 - delta) * (middle - short - alpha * delta)
            } else if x < 2.7 + delta {
                middle + alpha * (x - 2.7)
            } else {
                middle
                    + alpha * delta
                    + (x - 2.7 - delta) / (2.8 - delta) * (long - middle - alpha * delta)
            }
        };
        for (x, density) in A_STAR_SPECTROSCOPIC_SHAPE {
            assert!(
                (f(x) - density).abs() < 1e-15,
                "{x}: {:?} against {density}",
                f(x)
            );
        }
        println!("A stars at 2.7 M☉: f_logP {short:.4}, {middle:.4}, {long:.4}");
    }

    /// Moe and Di Stefano's excess twin fraction at their evaluation masses (§9.1: 1, 3.5, 7, 12
    /// and 28 M☉), from their eqs. 5–7.
    #[test]
    fn the_twin_fraction_is_moe_and_di_stefanos() {
        for (m, short, longest) in [
            (1.0, 0.30, 7.0),
            (3.5, 0.218_4, 4.5),
            (7.0, 0.173_2, 1.5),
            (12.0, 0.138_1, 1.5),
            (28.0, 0.082_9, 1.5),
        ] {
            let m1 = SolarMasses::new(m);
            assert!(
                (excess_twin_fraction(m1, 0.5) - short).abs() < 1e-4,
                "{m} M☉"
            );
            assert!(
                (longest_twin_log_period(m1) - longest).abs() < 1e-12,
                "{m} M☉"
            );
            let halfway = excess_twin_fraction(m1, 1.0_f64.midpoint(longest));
            assert!((halfway - 0.5 * excess_twin_fraction(m1, 0.0)).abs() < 1e-12);
            assert_same_bits(excess_twin_fraction(m1, longest), 0.0);
        }
        // No excess below Moe and Di Stefano's range (ruling 41).
        assert_same_bits(excess_twin_fraction(SolarMasses::new(0.79), 0.0), 0.0);
        assert!((excess_twin_fraction(SolarMasses::new(0.8), 0.0) - 0.3145).abs() < 1e-4);
        assert_same_bits(excess_twin_fraction(SolarMasses::new(150.0), 0.0), 0.0);
    }

    /// Among companions with q > 0.3, the twins' weight in the law is the excess fraction, as Moe
    /// and Di Stefano define it.
    #[test]
    fn the_twin_weight_counts_among_companions_above_three_tenths() {
        let model = model();
        for m in [0.8, 1.0, 4.0, 30.0] {
            let m1 = SolarMasses::new(m);
            let law = model.mass_ratio_distribution(m1, Days::new(5.0));
            let excess = excess_twin_fraction(m1, math::log10(5.0));
            let above = 1.0 - law.smooth_part().cdf(TWIN_REFERENCE_MASS_RATIO);
            let w = law.twin_share();
            let counted = w / (w + (1.0 - w) * above);
            assert!(
                (counted - excess).abs() < 1e-14,
                "{counted} against {excess} at {m} M☉"
            );
        }
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

    /// `quantile_in` is `sample_in` with its uniform given, and `share_in` is the probability
    /// between the same clamped limits: the pair the hierarchy draw's windows rest on.
    #[test]
    fn a_window_s_share_and_quantile_clamp_as_the_restricted_draw_does() {
        let periods = model().period_distribution(SolarMasses::new(1.0));
        let (x_min, x_max) = periods.support();
        let (lo, hi) = (Days::new(10.0), Days::new(1e4));
        let share = periods.share_in(lo, hi);
        assert_same_bits(share, periods.cdf(4.0) - periods.cdf(1.0));
        let mut drawn = stream(57);
        let mut given = stream(57);
        for _ in 0..1_000 {
            let u = given.uniform_open();
            assert_same_bits(
                periods.sample_in(&mut drawn, lo, hi).value(),
                periods.quantile_in(u, lo, hi).value(),
            );
        }
        // The ends of the window are the clamped limits.
        assert!((periods.quantile_in(0.0, lo, hi).value() - 10.0).abs() < 1e-9);
        assert!((periods.quantile_in(1.0, lo, hi).value() - 1e4).abs() < 1e-6);
        // Outside the support a window holds nothing, and partly outside it is clamped.
        assert_same_bits(periods.share_in(Days::new(1e12), Days::new(1e13)), 0.0);
        assert_same_bits(periods.share_in(Days::new(1e-3), Days::new(1e-2)), 0.0);
        let whole = periods.share_in(Days::new(1e-5), Days::new(1e15));
        assert_same_bits(whole, periods.cdf(x_max) - periods.cdf(x_min));
        assert!((whole - 1.0).abs() < 1e-12);
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
    /// binaries (§5.3), here those under 10⁴ days. Both are printed; the first is held to its
    /// 2σ Poisson range.
    #[test]
    fn sun_like_twins_match_the_surveys() {
        let model = model();
        let sun = SolarMasses::new(1.0);
        let like_mass = 1.0 - model.companion_mass_ratio_cdf(sun, TWIN_MIN_MASS_RATIO);
        let periods = model.period_distribution(sun);
        let twins = integrate_log_period(
            &periods,
            LOG_PERIOD_MIN,
            math::log10(43.0),
            &MultiplicityModel::mass_ratio_period_kinks(sun),
            0.5,
            |x| periods.pdf(x) * model.mass_ratio_at(sun, x).twin_share(),
        );
        let excess = twins * (1.0 - 0.98) / (1.0 - TWIN_MIN_MASS_RATIO) / periods.cdf(4.0);
        println!(
            "Sun-like: {like_mass:.3} of pairs like-mass, twin excess {excess:.4} of binaries \
             under 10⁴ d"
        );
        // 27 of about 248, 0.109 with a Poisson error of 0.021 at 1σ: the 2σ range (ruling 41).
        assert!((0.067..=0.151).contains(&like_mass), "{like_mass}");
        assert!((0.01..=0.03).contains(&excess), "{excess}");
    }

    #[test]
    fn the_marginal_mass_ratio_law_runs_from_nothing_to_everything() {
        let model = model();
        for m1 in mass_sweep() {
            let lo = MIN_COMPANION_MASS / m1;
            if lo >= 1.0 {
                continue;
            }
            assert!(model.companion_mass_ratio_cdf(m1, lo * 0.999).abs() < 1e-15);
            assert_same_bits(model.companion_mass_ratio_cdf(m1, 1.0), 1.0);
            let mut last = 0.0;
            for i in 0_u32..=20 {
                let q = lo + (1.0 - lo) * f64::from(i) / 20.0;
                let f = model.companion_mass_ratio_cdf(m1, q);
                assert!(f >= last - 1e-15, "the marginal falls at q = {q}, {m1:?}");
                last = f;
            }
            assert!((last - 1.0).abs() < 1e-12, "{last} at {m1:?}");
            let mean = model.mean_companion_mass_ratio(m1);
            assert!((lo..=1.0).contains(&mean), "{mean} at {m1:?}");
        }
    }

    /// Moe and Di Stefano's slopes at their evaluation points (eqs. 9–11 and 13–15), with the
    /// interpolation between their solar-type, 3.5 M☉ and early-type fits.
    #[test]
    fn the_slopes_are_moe_and_di_stefanos() {
        let cases = [
            (1.0, 3.0, -0.5, 0.3),
            (1.0, 6.0, -0.8, 0.3),
            (3.5, 3.0, -0.9, 0.05),
            (3.5, 5.5, -1.6, -0.7),
            (3.5, 7.0, -2.0, -1.0),
            (10.0, 1.5, -0.95, 0.025),
            (10.0, 3.0, -1.7, -0.2),
            (10.0, 6.0, -2.0, -1.5),
        ];
        for (m, x, large, small) in cases {
            assert!(
                (gamma_large(m, x) - large).abs() < 1e-12,
                "γ_largeq at {m}, {x}"
            );
            assert!(
                (gamma_small(m, x) - small).abs() < 1e-12,
                "γ_smallq at {m}, {x}"
            );
        }
        // Halfway between the solar-type and 3.5 M☉ fits in mass.
        let halfway = gamma_large(2.35, 3.0);
        assert!((halfway - (-0.7)).abs() < 1e-12, "{halfway}");
    }

    /// A law in Moe and Di Stefano's range is continuous at its breaks and has their slopes on
    /// each side.
    #[test]
    fn the_broken_power_law_is_continuous_with_their_slopes() {
        let model = model();
        for (m, x) in [(1.0, 3.0), (4.0, 5.0), (30.0, 6.0)] {
            let law = model.smooth_mass_ratio(SolarMasses::new(m), x);
            for q in MASS_RATIO_BREAKS {
                let below = law.pdf(q * (1.0 - 1e-9));
                let above = law.pdf(q * (1.0 + 1e-9));
                assert!((below / above - 1.0).abs() < 1e-6, "a jump at {q}, {m} M☉");
            }
            let slope = |a: f64, b: f64| math::ln(law.pdf(b) / law.pdf(a)) / math::ln(b / a);
            assert!((slope(0.4, 0.8) - gamma_large(m, x)).abs() < 1e-9, "{m} M☉");
            assert!(
                (slope(0.15, 0.25) - gamma_small(m, x)).abs() < 1e-9,
                "{m} M☉"
            );
            let extreme = gamma_small(m, x).max(0.0);
            let floor = law.lo() * 1.01;
            assert!((slope(floor, 0.095) - extreme).abs() < 1e-9, "{m} M☉");
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
