//! The medium's optics at one wavelength: density profiles, phase functions, their sampling, and
//! the scattering matrices of the Stokes mode.
//!
//! Sources:
//!
//! - Rayleigh scattering with depolarisation: Hansen and Travis 1974 (Space Sci. Rev. 16, 527),
//!   eqs. (2.15)–(2.16), p. 541 (their δ is ρ here). The matrix is Δ times the Rayleigh matrix plus
//!   (1 − Δ) times an isotropic, depolarising one, with Δ = (1 − ρ) ÷ (1 + ρ/2), and its (4,4)
//!   element carries Δ′ = (1 − 2ρ) ÷ (1 − ρ) besides; its (1,1) element is Chandrasekhar 1950's
//!   (3 ÷ (4(1 + 2γ)))((1 + 3γ) + (1 − γ) cos²Θ), γ = ρ ÷ (2 − ρ).
//! - Cornette and Shanks 1992 (Appl. Opt. 31, 3152; doi:10.1364/AO.31.003152; the same function is
//!   Draine 2003, ApJ 598, 1017, eq. 5 at α = 1), sampled by rejection from Henyey and Greenstein
//!   1941 (ApJ 93, 70), whose inverse CDF is Witt 1977 (ApJS 35, 1), eq. 19. Its mean cosine,
//!   3g(4 + g²) ÷ (5(2 + g²)), is derived from the function and checked numerically here; it is
//!   also Draine 2003's eq. A2 at α = 1, as R08.T12.c's science check read it.
//! - Tabulated phase functions (plan R08, R08.T12.c): the client's `PhaseTable`
//!   (`view/atmosphere/medium.ts`), a₁ and the elements beside it linear in u = √(Θ ÷ π) between
//!   entries, as the client's `phaseAt` reads them, and drawn from exactly, by the inverse of their
//!   cumulative distribution across the entries and rejection within one.
//! - Legendre series, p(cos Θ) = (1 ÷ 4π) Σ βₗ Pₗ(cos Θ) with β₀ = 1, the form in which Garcia and
//!   Siewert 1985 (Transport Theory Stat. Phys. 14, 437) give Haze L and Cloud C1, Pₗ by Bonnet's
//!   recurrence. Their directions are drawn from a fine table of the series and weighted by the
//!   series over the table, so the series is followed exactly wherever it is positive. It must be
//!   positive at every entry of that table and midway between each two; should a truncated series
//!   still dip below zero between those, a path that draws there ends, and the series is not
//!   followed exactly there.
//!
//! Every phase function here is normalised to 1 over the sphere, per steradian. A scattering
//! matrix is block-diagonal, the form of a macroscopically isotropic and mirror-symmetric medium
//! (Hovenier, van der Mee and Domke 2004):
//!
//! ```text
//! | a₁  b₁  0   0  |
//! | b₁  a₂  0   0  |
//! | 0   0   a₃  b₂ |
//! | 0   0  −b₂  a₄ |
//! ```
//!
//! acting on (I, Q, U, V) in the scattering plane's frame, so that b₁ < 0 polarises singly
//! scattered light perpendicular to that plane, and V takes the sign convention of the case's b₂.

use core::f64::consts::PI;

use hyperion_sim::galaxy::quad::gl16;
use hyperion_sim::math;

use super::case::{DensityProfile, PhaseFunction, PhaseMatrix};
use crate::tasks::displaced_forms::births::Draws;

impl DensityProfile {
    /// The relative density at `height_m` above the ground; a negative height reads as the
    /// ground, as the client's `densityAt` has it.
    #[must_use]
    pub(crate) fn at(&self, height_m: f64) -> f64 {
        let h = height_m.max(0.0);
        match self {
            Self::Exponential { scale_height_m } => math::exp(-h / scale_height_m),
            Self::Tent {
                bottom_m,
                peak_m,
                top_m,
            } => {
                if h <= *bottom_m || h >= *top_m {
                    0.0
                } else if h <= *peak_m {
                    (h - bottom_m) / (peak_m - bottom_m)
                } else {
                    (top_m - h) / (top_m - peak_m)
                }
            }
            Self::Tabulated {
                altitudes_m,
                relative,
            } => {
                let last = altitudes_m.len() - 1;
                let above = altitudes_m.partition_point(|&a| a <= h);
                if above == 0 {
                    relative[0]
                } else if above > last {
                    relative[last]
                } else {
                    let (a0, a1) = (altitudes_m[above - 1], altitudes_m[above]);
                    let (r0, r1) = (relative[above - 1], relative[above]);
                    r0 + (r1 - r0) * (h - a0) / (a1 - a0)
                }
            }
        }
    }

    /// An upper bound of [`at`](Self::at) over the heights `[low_m, high_m]`, exact for every
    /// kind: the larger end, or a peak or node between them.
    #[must_use]
    pub(crate) fn max_over(&self, low_m: f64, high_m: f64) -> f64 {
        let ends = self.at(low_m).max(self.at(high_m));
        match self {
            Self::Exponential { .. } => ends,
            Self::Tent { peak_m, .. } => {
                if (low_m..=high_m).contains(peak_m) {
                    1.0
                } else {
                    ends
                }
            }
            Self::Tabulated {
                altitudes_m,
                relative,
            } => altitudes_m
                .iter()
                .zip(relative)
                .filter(|(a, _)| (low_m..=high_m).contains(*a))
                .fold(ends, |m, (_, &r)| m.max(r)),
        }
    }

    /// The heights at which the profile needs a shell boundary below `top_m`: every kink, and
    /// for an exponential one every scale height, so that across a shell its density changes by
    /// at most a factor e. Past [`EXPONENTIAL_SHELL_HEIGHTS`] scale heights the term is below
    /// 10⁻¹⁵ of its ground value and needs no more.
    pub(crate) fn shell_heights(&self, top_m: f64, out: &mut Vec<f64>) {
        match self {
            Self::Exponential { scale_height_m } => {
                let mut k = 1.0;
                while k <= EXPONENTIAL_SHELL_HEIGHTS && k * scale_height_m < top_m {
                    out.push(k * scale_height_m);
                    k += 1.0;
                }
            }
            Self::Tent {
                bottom_m,
                peak_m,
                top_m: tent_top_m,
            } => out.extend([*bottom_m, *peak_m, *tent_top_m]),
            Self::Tabulated { altitudes_m, .. } => out.extend(altitudes_m),
        }
    }
}

/// How many scale heights of an exponential term are given shell boundaries: e^(−36) is
/// 2.3 × 10⁻¹⁶.
pub(crate) const EXPONENTIAL_SHELL_HEIGHTS: f64 = 36.0;

/// How far a table's or series' normalisation and mean cosine may stand from the source's: the
/// plan's 10⁻⁴ (R08.T12.c).
pub(crate) const PHASE_CHECK_TOLERANCE: f64 = 1e-4;

/// How far past a₁ another element of a tabulated matrix may stand, relatively, before the table
/// is refused: rounding only, since |a₂|, |a₃|, |a₄|, |b₁| and |b₂| ≤ a₁ for every matrix.
pub(crate) const MATRIX_BOUND_ROUNDING: f64 = 1e-9;

/// The entries of the table a Legendre series' directions are drawn from, even in u = √(Θ ÷ π):
/// the first past the forward direction stands at 1.9 × 10⁻⁷ rad, and a peak as narrow as Cloud
/// C1's (some 300 terms, a width of a few tenths of a degree) holds hundreds of entries.
pub(crate) const LEGENDRE_PROPOSAL_ENTRIES: usize = 4097;

/// The most terms a Legendre series may have.
pub(crate) const MAX_LEGENDRE_TERMS: usize = 10_000;

impl PhaseFunction {
    /// Whether the term has a scattering matrix of its own; a term without one is traced in the
    /// Stokes mode only if the case marks it `depolarising`, as a total depolariser.
    #[must_use]
    pub(crate) fn has_matrix(&self) -> bool {
        match self {
            Self::Rayleigh { .. } | Self::Isotropic {} | Self::None {} => true,
            Self::Tabulated { matrix, .. } => matrix.is_some(),
            Self::CornetteShanks { .. } | Self::Legendre { .. } => false,
        }
    }
}

/// 1 ÷ (4π), sr⁻¹.
const ONE_OVER_FOUR_PI: f64 = 0.25 / PI;

/// 1 − cos Θ at u = √(Θ ÷ π), as 2 sin²(πu² ÷ 2), which keeps its digits near the forward
/// direction.
fn one_minus_cos_at(u: f64) -> f64 {
    let half = math::sin(0.5 * PI * u * u);
    2.0 * half * half
}

/// u = √(Θ ÷ π) at cos Θ, as the client's `phaseAt` takes it.
fn u_at(cos_theta: f64) -> f64 {
    (math::acos(cos_theta.clamp(-1.0, 1.0)) / PI).sqrt()
}

/// A phase function tabulated on u = √(Θ ÷ π) at one wavelength, with the rest of its scattering
/// matrix where its source gives one: the client's `PhaseTable` for one channel.
///
/// Every element is linear in u between neighbouring entries, the client's `phaseAt` rule, and
/// is divided by the table's own integral, so that what the tracer draws from and what it weights
/// with are the same normalised function; [`TabulatedPhase::normalisation`] keeps the integral as
/// given, for the case's check against its source.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TabulatedPhase {
    /// u at each entry, from exactly 0 (forward) to exactly 1 (backward).
    u: Vec<f64>,
    /// 1 − cos Θ at each entry, from 0 to 2.
    one_minus_cos: Vec<f64>,
    /// a₁ at each entry, sr⁻¹, normalised.
    a1: Vec<f64>,
    /// a₂, a₃, a₄, b₁ and b₂ at each entry, scaled as `a1`; `None` where the source gives none.
    matrix: Option<[Vec<f64>; 5]>,
    /// The probability of scattering into the intervals below each entry: 0 at the first, 1 at
    /// the last.
    cumulative: Vec<f64>,
    /// The last interval of positive probability: where a draw of ξ = 1, which rounding allows,
    /// lands, so that it never lands where the table is zero at both ends.
    last_interval: usize,
    /// ∫ a₁ dΩ of the table as given.
    normalisation: f64,
    /// ∫ a₁ cos Θ dΩ ÷ ∫ a₁ dΩ.
    mean_cosine: f64,
}

impl TabulatedPhase {
    /// The table of `a1` (and `matrix`'s a₂, a₃, a₄, b₁ and b₂, in that order) at the entries
    /// `u`, whose shape the caller has checked: at least two entries ascending strictly from
    /// exactly 0 to exactly 1, every array of their length, a₁ finite and not negative and not
    /// zero everywhere.
    #[must_use]
    pub(crate) fn new(u: Vec<f64>, a1: &[f64], matrix: Option<[&[f64]; 5]>) -> Self {
        // The integrals over each interval, of a₁ and of a₁ cos Θ, in u: dΩ = 4π² u sin(πu²) du.
        // The integrand is a line times a smooth function across an interval, which the 16-point
        // rule integrates to rounding.
        let line = |i: usize, x: f64| {
            let f = (x - u[i]) / (u[i + 1] - u[i]);
            a1[i] + (a1[i + 1] - a1[i]) * f
        };
        let mut cumulative = Vec::with_capacity(u.len());
        cumulative.push(0.0);
        let (mut total, mut first_moment) = (0.0, 0.0);
        for i in 0..u.len() - 1 {
            let measure = |x: f64| 4.0 * PI * PI * x * math::sin(PI * x * x);
            total += gl16(|x| line(i, x) * measure(x), u[i], u[i + 1]);
            first_moment += gl16(
                |x| line(i, x) * measure(x) * math::cos(PI * x * x),
                u[i],
                u[i + 1],
            );
            cumulative.push(total);
        }
        for c in &mut cumulative {
            *c /= total;
        }
        let last = cumulative.len() - 1;
        cumulative[last] = 1.0;
        let last_interval = (0..last)
            .rev()
            .find(|&i| cumulative[i + 1] > cumulative[i])
            .expect("a table that is not zero everywhere has an interval of positive probability");
        let scale = |values: &[f64]| values.iter().map(|v| v / total).collect::<Vec<f64>>();
        Self {
            one_minus_cos: u.iter().map(|&x| one_minus_cos_at(x)).collect(),
            u,
            a1: scale(a1),
            matrix: matrix.map(|elements| elements.map(scale)),
            cumulative,
            last_interval,
            normalisation: total,
            mean_cosine: first_moment / total,
        }
    }

    /// ∫ a₁ dΩ of the table as given, which the source's normalisation makes 1.
    #[must_use]
    pub(crate) fn normalisation(&self) -> f64 {
        self.normalisation
    }

    /// The table's mean cosine, its asymmetry parameter g.
    #[must_use]
    pub(crate) fn mean_cosine(&self) -> f64 {
        self.mean_cosine
    }

    /// The entry below `cos_theta` and the fraction of the way to the next, by the client's
    /// `phaseAt` rule.
    fn locate(&self, cos_theta: f64) -> (usize, f64) {
        let u = u_at(cos_theta);
        let low = self
            .u
            .partition_point(|&x| x <= u)
            .saturating_sub(1)
            .min(self.u.len() - 2);
        let f = ((u - self.u[low]) / (self.u[low + 1] - self.u[low])).clamp(0.0, 1.0);
        (low, f)
    }

    /// a₁ at cos Θ, sr⁻¹.
    #[must_use]
    pub(crate) fn value(&self, cos_theta: f64) -> f64 {
        let (i, f) = self.locate(cos_theta);
        self.a1[i] + (self.a1[i + 1] - self.a1[i]) * f
    }

    /// The scattering matrix at cos Θ, if the source gives one.
    #[must_use]
    pub(crate) fn matrix(&self, cos_theta: f64) -> Option<ScatteringMatrix> {
        let elements = self.matrix.as_ref()?;
        let (i, f) = self.locate(cos_theta);
        let at = |values: &[f64]| values[i] + (values[i + 1] - values[i]) * f;
        Some(ScatteringMatrix {
            a1: at(&self.a1),
            a2: at(&elements[0]),
            a3: at(&elements[1]),
            a4: at(&elements[2]),
            b1: at(&elements[3]),
            b2: at(&elements[4]),
        })
    }

    /// Draws cos Θ from the table: an interval by the inverse of the cumulative distribution,
    /// then within it a direction uniform in solid angle, kept with probability a₁ ÷ the
    /// interval's larger end.
    pub(crate) fn sample(&self, draws: &mut Draws) -> f64 {
        let xi = draws.uniform();
        // The interval [i, i + 1] whose cumulative range holds ξ, which has positive probability:
        // ξ < 1 lands only in such an interval, and ξ = 1 in the last of them.
        let i = (self
            .cumulative
            .partition_point(|&c| c <= xi)
            .clamp(1, self.u.len() - 1)
            - 1)
        .min(self.last_interval);
        let (p0, p1) = (self.a1[i], self.a1[i + 1]);
        let top = p0.max(p1);
        let (s0, s1) = (self.one_minus_cos[i], self.one_minus_cos[i + 1]);
        let (u0, u1) = (self.u[i], self.u[i + 1]);
        loop {
            // 1 − cos Θ uniform across the interval is uniform in solid angle there.
            let s = s0 + draws.uniform() * (s1 - s0);
            let u = (2.0 * math::asin((0.5 * s).clamp(0.0, 1.0).sqrt()) / PI).sqrt();
            let f = ((u - u0) / (u1 - u0)).clamp(0.0, 1.0);
            if draws.uniform() * top < p0 + (p1 - p0) * f {
                return 1.0 - s;
            }
        }
    }
}

/// A Legendre series at one wavelength, p(cos Θ) = (1 ÷ 4π) Σ βₗ Pₗ(cos Θ), normalised.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LegendreSeries {
    /// βₗ ÷ (4π β₀).
    coefficients: Vec<f64>,
    /// The series at [`LEGENDRE_PROPOSAL_ENTRIES`] entries even in u: the table its directions
    /// are drawn from.
    proposal: TabulatedPhase,
}

/// Why a Legendre series cannot be traced.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub(crate) enum BuildLegendreError {
    /// The series is not positive at this cos Θ, an entry of the proposal or a midpoint between
    /// two.
    #[error("the series is not positive at cos Θ = {cos_theta}")]
    NotPositive { cos_theta: f64 },
}

/// Σ cₗ Pₗ(x) by Bonnet's recurrence, (l + 1) Pₗ₊₁ = (2l + 1) x Pₗ − l Pₗ₋₁.
fn legendre_sum(coefficients: &[f64], x: f64) -> f64 {
    let (mut below, mut at) = (1.0, x);
    let mut sum = coefficients[0];
    let mut l = 1.0;
    for &c in &coefficients[1..] {
        sum += c * at;
        let above = ((2.0 * l + 1.0) * x * at - l * below) / (l + 1.0);
        below = at;
        at = above;
        l += 1.0;
    }
    sum
}

impl LegendreSeries {
    /// The series of `beta` (β₀, β₁, …; β₀ positive), whose length and normalisation the caller
    /// has checked.
    ///
    /// # Errors
    ///
    /// [`BuildLegendreError::NotPositive`] where the series is not positive at an entry of the
    /// table its directions are drawn from, or midway between two.
    pub(crate) fn new(beta: &[f64]) -> Result<Self, BuildLegendreError> {
        let coefficients: Vec<f64> = beta
            .iter()
            .map(|b| b / beta[0] * ONE_OVER_FOUR_PI)
            .collect();
        let last = LEGENDRE_PROPOSAL_ENTRIES - 1;
        let u: Vec<f64> = (0..=last)
            .map(|k| {
                let k = u32::try_from(k).expect("the proposal has fewer than 2³² entries");
                let last = u32::try_from(last).expect("the proposal has fewer than 2³² entries");
                f64::from(k) / f64::from(last)
            })
            .collect();
        let positive = |x: f64| {
            let cos_theta = 1.0 - one_minus_cos_at(x);
            let p = legendre_sum(&coefficients, cos_theta);
            if p > 0.0 && p.is_finite() {
                Ok(p)
            } else {
                Err(BuildLegendreError::NotPositive { cos_theta })
            }
        };
        let mut values = Vec::with_capacity(u.len());
        for (k, &x) in u.iter().enumerate() {
            values.push(positive(x)?);
            if k + 1 < u.len() {
                positive(f64::midpoint(x, u[k + 1]))?;
            }
        }
        Ok(Self {
            proposal: TabulatedPhase::new(u, &values, None),
            coefficients,
        })
    }

    /// p(cos Θ), sr⁻¹.
    #[must_use]
    pub(crate) fn value(&self, cos_theta: f64) -> f64 {
        legendre_sum(&self.coefficients, cos_theta.clamp(-1.0, 1.0))
    }

    /// Draws cos Θ from the proposal table, and the weight p ÷ the table's density there that
    /// makes the draw the series' own.
    pub(crate) fn sample(&self, draws: &mut Draws) -> PhaseDraw {
        let cos_theta = self.proposal.sample(draws);
        PhaseDraw {
            cos_theta,
            weight: self.value(cos_theta) / self.proposal.value(cos_theta),
        }
    }
}

/// A term's phase function at one wavelength.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Phase {
    /// Rayleigh with depolarisation factor ρ.
    Rayleigh { depolarisation: f64 },
    /// Cornette and Shanks with parameter g.
    CornetteShanks { asymmetry: f64 },
    /// Isotropic and fully depolarising.
    Isotropic,
    /// No scattering.
    None,
    /// A table, with its matrix or without.
    Tabulated(Box<TabulatedPhase>),
    /// A Legendre series, without a matrix.
    Legendre(Box<LegendreSeries>),
}

/// The elements of a block-diagonal scattering matrix (module documentation), per steradian.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ScatteringMatrix {
    /// I to I: the phase function.
    pub(crate) a1: f64,
    /// Q to Q.
    pub(crate) a2: f64,
    /// U to U.
    pub(crate) a3: f64,
    /// V to V.
    pub(crate) a4: f64,
    /// I to Q and Q to I.
    pub(crate) b1: f64,
    /// V to U, and minus U to V.
    pub(crate) b2: f64,
}

impl ScatteringMatrix {
    /// A total depolariser of phase function `a1`: a₁ alone.
    #[must_use]
    pub(crate) fn depolarising(a1: f64) -> Self {
        Self {
            a1,
            ..Self::default()
        }
    }

    /// `self + weight × other`.
    pub(crate) fn add_scaled(&mut self, weight: f64, other: &Self) {
        self.a1 += weight * other.a1;
        self.a2 += weight * other.a2;
        self.a3 += weight * other.a3;
        self.a4 += weight * other.a4;
        self.b1 += weight * other.b1;
        self.b2 += weight * other.b2;
    }

    /// The matrix divided by its phase function, which must be positive: what a path carries
    /// when its direction is drawn from that phase function.
    #[must_use]
    pub(crate) fn per_phase(&self) -> Self {
        Self {
            a1: 1.0,
            a2: self.a2 / self.a1,
            a3: self.a3 / self.a1,
            a4: self.a4 / self.a1,
            b1: self.b1 / self.a1,
            b2: self.b2 / self.a1,
        }
    }
}

/// A drawn scattering direction's cos Θ, and the weight the path takes with it: 1 but for a draw
/// from a proposal (a Legendre series').
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PhaseDraw {
    pub(crate) cos_theta: f64,
    pub(crate) weight: f64,
}

impl Phase {
    /// The phase function of `phase` at wavelength `wavelength`, from a case that has been
    /// validated.
    ///
    /// # Panics
    ///
    /// For a Legendre series that is not positive, which the case's validation refuses.
    #[must_use]
    pub(crate) fn of(phase: &PhaseFunction, wavelength: usize) -> Self {
        match phase {
            PhaseFunction::Rayleigh { depolarisation } => Self::Rayleigh {
                depolarisation: depolarisation[wavelength],
            },
            PhaseFunction::CornetteShanks { asymmetry } => Self::CornetteShanks {
                asymmetry: *asymmetry,
            },
            PhaseFunction::Isotropic {} => Self::Isotropic,
            PhaseFunction::None {} => Self::None,
            PhaseFunction::Tabulated { u, a1, matrix, .. } => Self::Tabulated(Box::new(tabulated(
                u,
                &a1[wavelength],
                matrix.as_ref(),
                wavelength,
            ))),
            PhaseFunction::Legendre { coefficients } => Self::Legendre(Box::new(
                LegendreSeries::new(&coefficients[wavelength])
                    .expect("the case's validation refuses a series that is not positive"),
            )),
        }
    }

    /// The phase function at scattering angle Θ, `cos_theta` = cos Θ, sr⁻¹.
    #[must_use]
    pub(crate) fn value(&self, cos_theta: f64) -> f64 {
        match self {
            Self::Rayleigh {
                depolarisation: rho,
            } => {
                // (3 ÷ 16π) × 2((1 + ρ) + (1 − ρ) cos²Θ) ÷ (2 + ρ).
                3.0 / (8.0 * PI) * ((1.0 + rho) + (1.0 - rho) * cos_theta * cos_theta) / (2.0 + rho)
            }
            Self::CornetteShanks { asymmetry: g } => {
                let g2 = g * g;
                let x = 1.0 + g2 - 2.0 * g * cos_theta;
                3.0 / (8.0 * PI) * (1.0 - g2) * (1.0 + cos_theta * cos_theta)
                    / ((2.0 + g2) * x * x.sqrt())
            }
            Self::Isotropic => ONE_OVER_FOUR_PI,
            Self::None => 0.0,
            Self::Tabulated(table) => table.value(cos_theta),
            Self::Legendre(series) => series.value(cos_theta),
        }
    }

    /// The density, sr⁻¹, that [`sample`](Self::sample)'s draws have at cos Θ: the phase function,
    /// but for a Legendre series, whose draws come from its proposal table.
    #[must_use]
    pub(crate) fn sampling_density(&self, cos_theta: f64) -> f64 {
        match self {
            Self::Legendre(series) => series.proposal.value(cos_theta),
            Self::Rayleigh { .. }
            | Self::CornetteShanks { .. }
            | Self::Isotropic
            | Self::None
            | Self::Tabulated(_) => self.value(cos_theta),
        }
    }

    /// The scattering matrix at cos Θ, per steradian, or `None` for a phase function whose
    /// source gives none (Cornette and Shanks's, a Legendre series, a table without a matrix).
    #[must_use]
    pub(crate) fn matrix(&self, cos_theta: f64) -> Option<ScatteringMatrix> {
        match self {
            Self::Rayleigh {
                depolarisation: rho,
            } => {
                let delta = (1.0 - rho) / (1.0 + 0.5 * rho);
                let delta_prime = (1.0 - 2.0 * rho) / (1.0 - rho);
                let c2 = cos_theta * cos_theta;
                let polarised = 0.75 * delta * ONE_OVER_FOUR_PI;
                Some(ScatteringMatrix {
                    a1: polarised * (1.0 + c2) + (1.0 - delta) * ONE_OVER_FOUR_PI,
                    a2: polarised * (1.0 + c2),
                    a3: polarised * 2.0 * cos_theta,
                    a4: polarised * 2.0 * cos_theta * delta_prime,
                    b1: -polarised * (1.0 - c2),
                    b2: 0.0,
                })
            }
            Self::Isotropic => Some(ScatteringMatrix::depolarising(ONE_OVER_FOUR_PI)),
            Self::None => Some(ScatteringMatrix::default()),
            Self::Tabulated(table) => table.matrix(cos_theta),
            Self::CornetteShanks { .. } | Self::Legendre(_) => None,
        }
    }

    /// Draws cos Θ from the phase function, with the weight the draw carries.
    ///
    /// # Panics
    ///
    /// For `None`, which never scatters and so is never drawn from.
    pub(crate) fn sample(&self, draws: &mut Draws) -> PhaseDraw {
        let exact = |cos_theta| PhaseDraw {
            cos_theta,
            weight: 1.0,
        };
        match self {
            Self::Rayleigh { depolarisation } => {
                exact(rayleigh_cos(*depolarisation, draws.uniform()))
            }
            Self::CornetteShanks { asymmetry } => loop {
                // Henyey–Greenstein's pdf times (3 ÷ 2)(1 + μ²) ÷ (2 + g²) is Cornette and
                // Shanks's, and (1 + μ²) ÷ 2 is at most 1.
                let mu = henyey_greenstein_cos(*asymmetry, draws.uniform());
                if 2.0 * draws.uniform() < 1.0 + mu * mu {
                    break exact(mu);
                }
            },
            Self::Isotropic => exact(2.0 * draws.uniform() - 1.0),
            Self::Tabulated(table) => exact(table.sample(draws)),
            Self::Legendre(series) => series.sample(draws),
            Self::None => panic!("an absorbing-only term never scatters"),
        }
    }
}

/// The table of a case's `tabulated` phase at one wavelength.
#[must_use]
pub(crate) fn tabulated(
    u: &[f64],
    a1: &[f64],
    matrix: Option<&PhaseMatrix>,
    wavelength: usize,
) -> TabulatedPhase {
    TabulatedPhase::new(
        u.to_vec(),
        a1,
        matrix.map(|m| {
            [
                m.a2[wavelength].as_slice(),
                m.a3[wavelength].as_slice(),
                m.a4[wavelength].as_slice(),
                m.b1[wavelength].as_slice(),
                m.b2[wavelength].as_slice(),
            ]
        }),
    )
}

/// The cos Θ at which Rayleigh's phase function of depolarisation ρ has cumulative probability
/// `xi`, from cos Θ = −1.
///
/// The CDF is 3((1 + ρ)(μ + 1) + (1 − ρ)(μ³ + 1) ÷ 3) ÷ (4(2 + ρ)), so μ is the one real root of
/// μ³ + pμ + q = 0 with p = 3(1 + ρ) ÷ (1 − ρ) > 0 and q = (4 + 2ρ)(1 − 2ξ) ÷ (1 − ρ), by
/// Cardano's formula taken in the form without cancellation.
#[must_use]
pub(crate) fn rayleigh_cos(depolarisation: f64, xi: f64) -> f64 {
    let rho = depolarisation;
    let p = 3.0 * (1.0 + rho) / (1.0 - rho);
    let q = (4.0 + 2.0 * rho) * (1.0 - 2.0 * xi) / (1.0 - rho);
    let root = (0.25 * q * q + p * p * p / 27.0).sqrt();
    let a = math::cbrt(-0.5 * q - root.copysign(q));
    let mu = if a.abs() > 0.0 {
        a - p / (3.0 * a)
    } else {
        0.0
    };
    mu.clamp(-1.0, 1.0)
}

/// The cos Θ at which Henyey and Greenstein's phase function of asymmetry g has cumulative
/// probability `xi` (Witt 1977, ApJS 35, 1, eq. 19).
#[must_use]
pub(crate) fn henyey_greenstein_cos(asymmetry: f64, xi: f64) -> f64 {
    let g = asymmetry;
    if g.abs() < 1e-6 {
        return 2.0 * xi - 1.0;
    }
    let s = (1.0 - g * g) / (1.0 - g + 2.0 * g * xi);
    ((1.0 + g * g - s * s) / (2.0 * g)).clamp(-1.0, 1.0)
}

#[cfg(test)]
pub(crate) mod tests {
    use hyperion_sim::galaxy::quad::gl_panels;

    use super::*;

    /// ∫ p dΩ over the sphere, by panels in cos Θ fine enough for a forward peak.
    fn norm(phase: &Phase) -> f64 {
        let edges: Vec<f64> = (0..=64).map(|i| -1.0 + f64::from(i) / 32.0).collect();
        2.0 * PI * gl_panels(|mu| phase.value(mu), &edges)
    }

    #[test]
    fn atmosphere_phase_functions_are_normalised() {
        for phase in [
            Phase::Rayleigh {
                depolarisation: 0.0,
            },
            Phase::Rayleigh {
                depolarisation: 0.0279,
            },
            Phase::Rayleigh {
                depolarisation: 0.4,
            },
            Phase::CornetteShanks { asymmetry: 0.0 },
            Phase::CornetteShanks { asymmetry: 0.8 },
            Phase::CornetteShanks { asymmetry: -0.5 },
            Phase::Isotropic,
        ] {
            assert!(
                (norm(&phase) - 1.0).abs() < 1e-12,
                "{phase:?}: {}",
                norm(&phase)
            );
        }
        assert!(norm(&Phase::None).abs() < f64::EPSILON);
    }

    #[test]
    fn atmosphere_rayleigh_matrix_agrees_with_its_phase_function_and_depolarisation() {
        for rho in [0.0, 0.0279, 0.1] {
            let phase = Phase::Rayleigh {
                depolarisation: rho,
            };
            for mu in [-1.0, -0.4, 0.0, 0.3, 1.0] {
                let m = phase.matrix(mu).unwrap();
                assert!((m.a1 - phase.value(mu)).abs() < 1e-15, "{rho} {mu}");
                // Hansen and Travis 1974, eq. (2.15): a₄ = Δ′ a₃, with Δ′ of eq. (2.16).
                let delta_prime = (1.0 - 2.0 * rho) / (1.0 - rho);
                assert!((m.a4 - delta_prime * m.a3).abs() < 1e-17, "{rho} {mu}");
                assert!(m.b2.abs() < f64::MIN_POSITIVE);
            }
            // At 90° unpolarised light scatters with depolarisation ratio I_l ÷ I_r = ρ.
            let m = phase.matrix(0.0).unwrap();
            let (parallel, perpendicular) = (m.a1 + m.b1, m.a1 - m.b1);
            assert!((parallel / perpendicular - rho).abs() < 1e-14, "{rho}");
        }
    }

    #[test]
    fn atmosphere_rayleigh_sampling_inverts_its_cdf() {
        for rho in [0.0, 0.0279, 0.3] {
            let cdf = |mu: f64| {
                3.0 * ((1.0 + rho) * (mu + 1.0) + (1.0 - rho) * (mu * mu * mu + 1.0) / 3.0)
                    / (4.0 * (2.0 + rho))
            };
            for xi in [1e-9, 0.01, 0.25, 0.5, 0.75, 0.99, 1.0 - 1e-9] {
                let mu = rayleigh_cos(rho, xi);
                assert!((cdf(mu) - xi).abs() < 1e-13, "{rho} {xi}: {mu}");
            }
        }
    }

    /// The mean of `f(cos Θ)` and its standard error over `n` draws of `phase`, each weighted
    /// by its draw's weight, and the mean weight.
    fn sampled_mean(phase: &Phase, n: u32, f: impl Fn(f64) -> f64) -> (f64, f64, f64) {
        let mut draws = Draws::new(3, 0, 0);
        let (mut sum, mut sum2, mut weights) = (0.0, 0.0, 0.0);
        for _ in 0..n {
            let draw = phase.sample(&mut draws);
            let x = draw.weight * f(draw.cos_theta);
            sum += x;
            sum2 += x * x;
            weights += draw.weight;
        }
        let n = f64::from(n);
        let mean = sum / n;
        (mean, ((sum2 / n - mean * mean) / n).sqrt(), weights / n)
    }

    #[test]
    fn atmosphere_cornette_shanks_sampling_has_its_mean_cosine() {
        for g in [0.0, 0.5, 0.8, -0.3] {
            let phase = Phase::CornetteShanks { asymmetry: g };
            let (mean, error, _) = sampled_mean(&phase, 200_000, |mu| mu);
            // The mean cosine, derived from the function (module documentation) and checked
            // against its quadrature here: 3g(4 + g²) ÷ (5(2 + g²)).
            let expected = 3.0 * g * (4.0 + g * g) / (5.0 * (2.0 + g * g));
            let edges: Vec<f64> = (0..=64).map(|i| -1.0 + f64::from(i) / 32.0).collect();
            let quadrature = 2.0 * PI * gl_panels(|mu| mu * phase.value(mu), &edges);
            assert!((quadrature - expected).abs() < 1e-12, "{g}: {quadrature}");
            assert!(
                (mean - expected).abs() < 4.0 * error,
                "{g}: {mean} {expected} ± {error}"
            );
        }
    }

    /// Henyey and Greenstein's phase function of asymmetry g, sr⁻¹.
    pub(crate) fn henyey_greenstein(g: f64, cos_theta: f64) -> f64 {
        let x = 1.0 + g * g - 2.0 * g * cos_theta;
        (1.0 - g * g) / (4.0 * PI * x * x.sqrt())
    }

    /// `n` entries even in u from 0 to 1.
    pub(crate) fn even_u(n: u32) -> Vec<f64> {
        (0..n).map(|k| f64::from(k) / f64::from(n - 1)).collect()
    }

    #[test]
    fn atmosphere_tabulated_phase_reads_as_the_client_does_and_is_normalised() {
        // Henyey–Greenstein at g = 0.7 on 256 entries even in u, as the client's tables are.
        let g = 0.7;
        let u = even_u(256);
        let a1: Vec<f64> = u
            .iter()
            .map(|&x| henyey_greenstein(g, math::cos(PI * x * x)))
            .collect();
        let table = TabulatedPhase::new(u.clone(), &a1, None);
        // Sampled at its entries and read as a line in u, the function's integral and mean cosine
        // move by 6.5 × 10⁻⁵ and −4.6 × 10⁻⁶ (measured), inside the plan's 10⁻⁴. A sharper peak
        // moves the integral further, 1.1 × 10⁻⁴ at g = 0.8, 2.3 × 10⁻⁴ at 0.9 and 4.8 × 10⁻⁴ at
        // 0.95 on 256 entries (a quarter of that on 512), while the mean cosine stays within
        // 1.3 × 10⁻⁵: so a table's builder normalises its entries under this rule, as the client's
        // `PhaseTable` asks of R08.T5.c, and the case's check holds it to that.
        assert!(
            (table.normalisation() - 1.0).abs() < 1e-4,
            "{}",
            table.normalisation()
        );
        assert!(
            (table.mean_cosine() - g).abs() < 1e-5,
            "{}",
            table.mean_cosine()
        );
        // At the entries the normalised table is the function over its integral; between them, a
        // line in u, as `phaseAt` reads it.
        let n = table.normalisation();
        for k in [0, 1, 17, 128, 255] {
            let c = math::cos(PI * u[k] * u[k]);
            assert!((table.value(c) * n / a1[k] - 1.0).abs() < 1e-12, "{k}");
        }
        let half = f64::midpoint(u[40], u[41]);
        let between = table.value(math::cos(PI * half * half)) * n;
        assert!((between / f64::midpoint(a1[40], a1[41]) - 1.0).abs() < 1e-12);
        assert!(table.matrix(0.3).is_none());
    }

    #[test]
    fn atmosphere_tabulated_sampling_follows_the_table() {
        // A strongly forward table (g = 0.9) and a backward one: the draws' mean cosine and second
        // moment are the table's, by its own quadrature, to 4σ.
        for g in [0.9, -0.4] {
            let u = even_u(256);
            let a1: Vec<f64> = u
                .iter()
                .map(|&x| henyey_greenstein(g, math::cos(PI * x * x)))
                .collect();
            let table = TabulatedPhase::new(u, &a1, None);
            let phase = Phase::Tabulated(Box::new(table.clone()));
            let moment = |f: &dyn Fn(f64) -> f64| {
                let edges: Vec<f64> = (0..=2048).map(|i| -1.0 + f64::from(i) / 1024.0).collect();
                2.0 * PI * gl_panels(|mu| f(mu) * table.value(mu), &edges)
            };
            for (name, f) in [
                ("cos", &(|mu: f64| mu) as &dyn Fn(f64) -> f64),
                ("cos²", &|mu: f64| mu * mu),
            ] {
                let expected = moment(f);
                let (mean, error, weight) = sampled_mean(&phase, 200_000, f);
                assert!((weight - 1.0).abs() < f64::EPSILON, "{g}");
                assert!(
                    (mean - expected).abs() < 4.0 * error,
                    "{g} {name}: {mean} against {expected} ± {error}"
                );
            }
            assert!((moment(&|_| 1.0) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn atmosphere_legendre_series_is_followed_exactly() {
        // Henyey–Greenstein's series, βₗ = (2l + 1) gˡ, to 400 terms at g = 0.9: the series is the
        // function to 10⁻¹⁵, and weighted draws from its proposal have the mean cosine g and a
        // mean weight of 1, each to 4σ.
        let g: f64 = 0.9;
        let mut power = 1.0;
        let beta: Vec<f64> = (0..400)
            .map(|l| {
                let b = f64::from(2 * l + 1) * power;
                power *= g;
                b
            })
            .collect();
        let series = LegendreSeries::new(&beta).unwrap();
        for c in [-1.0, -0.3, 0.0, 0.5, 0.9, 0.999, 1.0] {
            let expected = henyey_greenstein(g, c);
            assert!(
                (series.value(c) / expected - 1.0).abs() < 1e-12,
                "{c}: {} {expected}",
                series.value(c)
            );
        }
        let phase = Phase::Legendre(Box::new(series));
        let (mean, error, _) = sampled_mean(&phase, 200_000, |mu| mu);
        assert!((mean - g).abs() < 4.0 * error, "{mean} ± {error}");
        let (weight, weight_error, _) = sampled_mean(&phase, 200_000, |_| 1.0);
        assert!(
            (weight - 1.0).abs() < 4.0 * weight_error.max(1e-9),
            "{weight}"
        );
        // A series that goes negative is refused.
        assert!(matches!(
            LegendreSeries::new(&[1.0, 0.0, -3.0]),
            Err(BuildLegendreError::NotPositive { .. })
        ));
    }

    /// The matrix of a sphere whose amplitudes are those of an electric and a magnetic dipole,
    /// S₁ = a + b cos Θ and S₂ = a cos Θ + b (Bohren and Huffman 1983, eq. 4.74, its first term,
    /// without its factor 3 ÷ 2), as
    /// Bohren and Huffman's S₁₁, S₁₂, S₃₃ and S₃₄ (eq. 3.16), with a₂ = a₁ and a₄ = a₃:
    /// (a₁, b₁, a₃, b₂) unnormalised. `b = 0` is a small sphere's, Rayleigh's limit.
    pub(crate) fn dipole_sphere(a: (f64, f64), b: (f64, f64), cos_theta: f64) -> [f64; 4] {
        let s1 = (a.0 + b.0 * cos_theta, a.1 + b.1 * cos_theta);
        let s2 = (a.0 * cos_theta + b.0, a.1 * cos_theta + b.1);
        let norm1 = s1.0 * s1.0 + s1.1 * s1.1;
        let norm2 = s2.0 * s2.0 + s2.1 * s2.1;
        // S₂ S₁*.
        let (re, im) = (s2.0 * s1.0 + s2.1 * s1.1, s2.1 * s1.0 - s2.0 * s1.1);
        [f64::midpoint(norm2, norm1), 0.5 * (norm2 - norm1), re, im]
    }

    #[test]
    fn atmosphere_a_small_spheres_tabulated_matrix_is_the_rayleigh_matrix() {
        // A small sphere's amplitudes, S₂ = S₁ cos Θ, tabulated on 256 entries even in u: its
        // matrix read through the table is Hansen and Travis's at ρ = 0 to the table's
        // interpolation, 10⁻⁴ of a₁ (measured 3 × 10⁻⁵).
        let u = even_u(256);
        let elements: Vec<[f64; 4]> = u
            .iter()
            .map(|&x| dipole_sphere((0.3, -1.7), (0.0, 0.0), math::cos(PI * x * x)))
            .collect();
        let column = |k: usize| elements.iter().map(|e| e[k]).collect::<Vec<f64>>();
        let (a1, b1, a3, b2) = (column(0), column(1), column(2), column(3));
        let table = TabulatedPhase::new(u, &a1, Some([&a1, &a3, &a3, &b1, &b2]));
        let rayleigh = Phase::Rayleigh {
            depolarisation: 0.0,
        };
        for k in 0..=200 {
            let c = -1.0 + f64::from(k) / 100.0;
            let (tabulated, exact) = (table.matrix(c).unwrap(), rayleigh.matrix(c).unwrap());
            for (x, y) in [
                (tabulated.a1, exact.a1),
                (tabulated.a2, exact.a2),
                (tabulated.a3, exact.a3),
                (tabulated.a4, exact.a4),
                (tabulated.b1, exact.b1),
                (tabulated.b2, exact.b2),
            ] {
                assert!(
                    (x - y).abs() < 1e-4 * exact.a1,
                    "{c}: {tabulated:?} {exact:?}"
                );
            }
        }
    }

    #[test]
    fn atmosphere_density_profiles_are_bounded_over_their_shells() {
        let profiles = [
            DensityProfile::Exponential {
                scale_height_m: 8000.0,
            },
            DensityProfile::Tent {
                bottom_m: 10e3,
                peak_m: 25e3,
                top_m: 40e3,
            },
            DensityProfile::Tabulated {
                altitudes_m: vec![0.0, 1e3, 3e3, 9e3],
                relative: vec![0.5, 1.0, 0.2, 0.4],
            },
        ];
        for profile in &profiles {
            for (lo, hi) in [(0.0, 2e3), (500.0, 12e3), (20e3, 30e3), (35e3, 60e3)] {
                let bound = profile.max_over(lo, hi);
                let worst = (0..=1000)
                    .map(|i| profile.at(lo + (hi - lo) * f64::from(i) / 1000.0))
                    .fold(0.0, f64::max);
                // A bound, and a tight one: the grid of 1,001 points may miss a node by a step.
                assert!(
                    worst <= bound && bound - worst < 1e-2,
                    "{profile:?} {lo} {hi}"
                );
            }
        }
        let tabulated = &profiles[2];
        assert!((tabulated.at(-5.0) - 0.5).abs() < f64::EPSILON);
        assert!((tabulated.at(2e3) - 0.6).abs() < 1e-15);
        assert!((tabulated.at(1e5) - 0.4).abs() < f64::EPSILON);
    }
}
