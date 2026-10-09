//! The medium's optics at one wavelength: density profiles, phase functions, their sampling, and
//! the Rayleigh scattering matrix of the Stokes mode.
//!
//! Sources:
//!
//! - Rayleigh scattering with depolarisation: Hansen and Travis 1974 (Space Sci. Rev. 16, 527),
//!   eq. 2.15, whose matrix is Δ times the Rayleigh matrix plus (1 − Δ) times an isotropic,
//!   depolarising one, with Δ = (1 − ρ) ÷ (1 + ρ/2) and Δ′ = (1 − 2ρ) ÷ (1 − ρ); its (1,1)
//!   element is Chandrasekhar 1950's (3 ÷ (4(1 + 2γ)))((1 + 3γ) + (1 − γ) cos²Θ), γ = ρ ÷ (2 − ρ).
//! - Cornette and Shanks 1992 (Appl. Opt. 31, 3152), eq. 8, sampled by rejection from
//!   Henyey and Greenstein 1941 (ApJ 93, 70), whose inverse CDF is the textbook one (for example
//!   Witt 1977, ApJS 35, 1, eq. 13).
//!
//! Every phase function here is normalised to 1 over the sphere, per steradian.

use core::f64::consts::PI;

use hyperion_sim::math;

use super::case::{DensityProfile, PhaseFunction};
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

impl PhaseFunction {
    /// Whether the Stokes mode has this phase function's scattering matrix.
    #[must_use]
    pub(crate) fn has_matrix(&self) -> bool {
        match self {
            Self::Rayleigh { .. } | Self::Isotropic {} | Self::None {} => true,
            Self::CornetteShanks { .. } => false,
        }
    }
}

/// A term's phase function at one wavelength.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Phase {
    /// Rayleigh with depolarisation factor ρ.
    Rayleigh { depolarisation: f64 },
    /// Cornette and Shanks with parameter g.
    CornetteShanks { asymmetry: f64 },
    /// Isotropic and fully depolarising.
    Isotropic,
    /// No scattering.
    None,
}

/// The elements of a scattering matrix of the block form the Rayleigh and isotropic matrices
/// share, per steradian, on the Stokes vector (I, Q, U) with Q and U in the scattering plane's
/// frame (Hovenier, van der Mee and Domke 2004's a₁, a₂, a₃ and b₁; V decouples, and no source
/// here makes any).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ScatteringMatrix {
    /// I to I: the phase function.
    pub(crate) a1: f64,
    /// Q to Q.
    pub(crate) a2: f64,
    /// U to U.
    pub(crate) a3: f64,
    /// I to Q and Q to I.
    pub(crate) b1: f64,
}

impl ScatteringMatrix {
    /// `self + weight × other`.
    pub(crate) fn add_scaled(&mut self, weight: f64, other: &Self) {
        self.a1 += weight * other.a1;
        self.a2 += weight * other.a2;
        self.a3 += weight * other.a3;
        self.b1 += weight * other.b1;
    }
}

/// 1 ÷ (4π), sr⁻¹.
const ONE_OVER_FOUR_PI: f64 = 0.25 / PI;

impl Phase {
    /// The phase function of `phase` at wavelength `wavelength`.
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
        }
    }

    /// The phase function at scattering angle Θ, `cos_theta` = cos Θ, sr⁻¹.
    #[must_use]
    pub(crate) fn value(self, cos_theta: f64) -> f64 {
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
        }
    }

    /// The scattering matrix at cos Θ, per steradian, for the phase functions that have one.
    ///
    /// # Panics
    ///
    /// For Cornette and Shanks's, which has none: the Stokes mode refuses such cases before it
    /// traces.
    #[must_use]
    pub(crate) fn matrix(self, cos_theta: f64) -> ScatteringMatrix {
        match self {
            Self::Rayleigh {
                depolarisation: rho,
            } => {
                let delta = (1.0 - rho) / (1.0 + 0.5 * rho);
                let c2 = cos_theta * cos_theta;
                let polarised = 0.75 * delta * ONE_OVER_FOUR_PI;
                ScatteringMatrix {
                    a1: polarised * (1.0 + c2) + (1.0 - delta) * ONE_OVER_FOUR_PI,
                    a2: polarised * (1.0 + c2),
                    a3: polarised * 2.0 * cos_theta,
                    b1: -polarised * (1.0 - c2),
                }
            }
            Self::Isotropic => ScatteringMatrix {
                a1: ONE_OVER_FOUR_PI,
                ..ScatteringMatrix::default()
            },
            Self::None => ScatteringMatrix::default(),
            Self::CornetteShanks { .. } => {
                panic!("the Stokes mode traces only Rayleigh and isotropic scattering")
            }
        }
    }

    /// Draws cos Θ from the phase function.
    ///
    /// # Panics
    ///
    /// For `None`, which never scatters and so is never drawn from.
    pub(crate) fn sample(self, draws: &mut Draws) -> f64 {
        match self {
            Self::Rayleigh { depolarisation } => rayleigh_cos(depolarisation, draws.uniform()),
            Self::CornetteShanks { asymmetry } => loop {
                // Henyey–Greenstein's pdf times (3 ÷ 2)(1 + μ²) ÷ (2 + g²) is Cornette and
                // Shanks's, and (1 + μ²) ÷ 2 is at most 1.
                let mu = henyey_greenstein_cos(asymmetry, draws.uniform());
                if 2.0 * draws.uniform() < 1.0 + mu * mu {
                    break mu;
                }
            },
            Self::Isotropic => 2.0 * draws.uniform() - 1.0,
            Self::None => panic!("an absorbing-only term never scatters"),
        }
    }
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
/// probability `xi`.
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
mod tests {
    use hyperion_sim::galaxy::quad::gl_panels;

    use super::*;

    /// ∫ p dΩ over the sphere, by panels in cos Θ fine enough for a forward peak.
    fn norm(phase: Phase) -> f64 {
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
                (norm(phase) - 1.0).abs() < 1e-12,
                "{phase:?}: {}",
                norm(phase)
            );
        }
        assert!(norm(Phase::None).abs() < f64::EPSILON);
    }

    #[test]
    fn atmosphere_rayleigh_matrix_agrees_with_its_phase_function_and_depolarisation() {
        for rho in [0.0, 0.0279, 0.1] {
            let phase = Phase::Rayleigh {
                depolarisation: rho,
            };
            for mu in [-1.0, -0.4, 0.0, 0.3, 1.0] {
                let m = phase.matrix(mu);
                assert!((m.a1 - phase.value(mu)).abs() < 1e-15, "{rho} {mu}");
            }
            // At 90° unpolarised light scatters with depolarisation ratio I_l ÷ I_r = ρ.
            let m = phase.matrix(0.0);
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

    #[test]
    fn atmosphere_cornette_shanks_sampling_has_its_mean_cosine() {
        for g in [0.0, 0.5, 0.8, -0.3] {
            let phase = Phase::CornetteShanks { asymmetry: g };
            let mut draws = Draws::new(3, 0, 0);
            let n = 200_000;
            let (mut sum, mut sum2) = (0.0, 0.0);
            for _ in 0..n {
                let mu = phase.sample(&mut draws);
                sum += mu;
                sum2 += mu * mu;
            }
            let mean = sum / f64::from(n);
            let error = ((sum2 / f64::from(n) - mean * mean) / f64::from(n)).sqrt();
            // Cornette and Shanks 1992's mean cosine, 3g(4 + g²) ÷ (5(2 + g²)).
            let expected = 3.0 * g * (4.0 + g * g) / (5.0 * (2.0 + g * g));
            assert!(
                (mean - expected).abs() < 4.0 * error,
                "{g}: {mean} {expected} ± {error}"
            );
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
