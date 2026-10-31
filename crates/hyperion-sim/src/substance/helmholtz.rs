//! A fluid's reference equation of state, as the residual part of its dimensionless Helmholtz
//! energy (plan 14, P14.T49.h): what a liquid's isothermal compressibility, and so its scattering,
//! is computed from (decision-r11-palette-liquids §1.8); with the ideal-gas part's second
//! τ-derivative, its saturated liquid's specific heat (rendering plan R09, R09.T13.i;
//! decision-r09-t13i-cl-licence §6).
//!
//! A multiparameter reference equation (IAPWS-95; Setzmann and Wagner 1991; Span et al. 2000; and
//! the rest) writes the Helmholtz energy f of the fluid as φ(δ, τ) = f ÷ (RT) = φ°(δ, τ) + φʳ(δ,
//! τ), in the reduced density δ = ρ ÷ `ρ_r` and the inverse reduced temperature τ = `T_r` ÷ T. Its
//! residual part φʳ is a sum of terms of four published kinds ([`HelmholtzResidual`]): power and
//! exponential terms, Gaussian bell-shaped terms, non-analytic critical terms and Gao et al. 2023's
//! associating terms, each with its coefficients as printed. Every property follows from its
//! derivatives (IAPWS R6-95(2018), Table 3). A liquid's record needs the pressure's derivative
//! along an isotherm, from `φʳ_δ` and `φʳ_δδ`:
//!
//! - p = `ρRT` (1 + `δφʳ_δ`);
//! - `κ_T` = ρ⁻¹ (∂ρ ÷ ∂p)_T = 1 ÷ [`ρRT` (1 + `2δφʳ_δ` + `δ²φʳ_δδ`)].
//!
//! Its specific heats need `φʳ_ττ` and `φʳ_δτ` too, and the ideal-gas part's `φ°_ττ`
//! ([`HelmholtzIdeal`]), whose other terms (ln δ and the terms linear in τ) drop out of them:
//!
//! - `c_v` ÷ R = −τ² (`φ°_ττ` + `φʳ_ττ`);
//! - `c_p` ÷ R = −τ² (`φ°_ττ` + `φʳ_ττ`) + (1 + `δφʳ_δ` − `δτφʳ_δτ`)² ÷ (1 + `2δφʳ_δ` +
//!   `δ²φʳ_δδ`).
//!
//! The terms are summed in their published order, each power through the crate's deterministic
//! `math` (sim-determinism): an integer power of δ by [`math::powi`], a real power by
//! [`math::powf`], each exponential by [`math::exp`]; a density solved at a pressure takes a fixed
//! number of Newton steps along one isotherm, which takes τ's powers once for all of them. Nothing
//! here is cached between calls; a partition keeps its last compression in a local slot of its
//! own (`liquid::CompressionSlot`).

use crate::math;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// A polynomial or exponential term of a residual Helmholtz energy: n δ^d τ^t, times exp(−δ^c)
/// when c is not 0 (IAPWS R6-95(2018), Eq. (6), its terms 1–51).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PowerTerm {
    pub(crate) n: f64,
    pub(crate) d: i32,
    pub(crate) t: f64,
    pub(crate) c: i32,
}

impl PowerTerm {
    /// The term n δ^d τ^t, times exp(−δ^c) when c is not 0.
    #[must_use]
    pub(crate) const fn new(c: i32, d: i32, t: f64, n: f64) -> Self {
        Self { n, d, t, c }
    }
}

/// A Gaussian bell-shaped term: n δ^d τ^t exp(−α(δ − ε)² − β(τ − γ)²) (IAPWS R6-95(2018), Eq. (6),
/// its terms 52–54; Setzmann and Wagner 1991's and Span et al. 2000's alike).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GaussianTerm {
    pub(crate) n: f64,
    pub(crate) d: i32,
    pub(crate) t: f64,
    pub(crate) alpha: f64,
    pub(crate) beta: f64,
    pub(crate) gamma: f64,
    pub(crate) epsilon: f64,
}

impl GaussianTerm {
    /// The term n δ^d τ^t exp(−α(δ − ε)² − β(τ − γ)²).
    #[must_use]
    pub(crate) const fn new(
        d: i32,
        t: f64,
        n: f64,
        alpha: f64,
        beta: f64,
        gamma: f64,
        epsilon: f64,
    ) -> Self {
        Self {
            n,
            d,
            t,
            alpha,
            beta,
            gamma,
            epsilon,
        }
    }
}

/// An associating term: n δ^d τ^t exp(−η(δ − ε)² + 1 ÷ [β(τ − γ)² + b]) (Gao, Wu, Bell, Harvey
/// and Lemmon 2023, J. Phys. Chem. Ref. Data 52, 013102, the term of their Sec. 2.4 and Table 5's
/// terms 19–20, for ammonia). It has a Gaussian term's shape in δ, so its δ-derivatives are a
/// Gaussian term's with exp(1 ÷ [β(τ − γ)² + b]) in place of exp(−β(τ − γ)²).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AssociatingTerm {
    pub(crate) n: f64,
    pub(crate) d: i32,
    pub(crate) t: f64,
    pub(crate) eta: f64,
    pub(crate) beta: f64,
    pub(crate) gamma: f64,
    pub(crate) epsilon: f64,
    pub(crate) b: f64,
}

impl AssociatingTerm {
    /// The term n δ^d τ^t exp(−η(δ − ε)² + 1 ÷ [β(τ − γ)² + b]): d, t and n first, as
    /// [`GaussianTerm::new`] takes them, then η, β, γ, ε and b in the order Gao et al.'s Table 5
    /// prints them.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the term's eight published parameters, the last five in the order its source \
                  prints them"
    )]
    pub(crate) const fn new(
        d: i32,
        t: f64,
        n: f64,
        eta: f64,
        beta: f64,
        gamma: f64,
        epsilon: f64,
        b: f64,
    ) -> Self {
        Self {
            n,
            d,
            t,
            eta,
            beta,
            gamma,
            epsilon,
            b,
        }
    }
}

/// A non-analytic term for the critical region: n Δ^b δ ψ, with Δ = θ² + B[(δ − 1)²]^a,
/// θ = (1 − τ) + A[(δ − 1)²]^(1 ÷ 2β) and ψ = exp(−C(δ − 1)² − D(τ − 1)²) (IAPWS R6-95(2018),
/// Eq. (6), its terms 55–56; Span and Wagner 1996's for carbon dioxide alike).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NonAnalyticTerm {
    pub(crate) n: f64,
    pub(crate) a: f64,
    pub(crate) b: f64,
    pub(crate) big_b: f64,
    pub(crate) big_c: f64,
    pub(crate) big_d: f64,
    pub(crate) big_a: f64,
    pub(crate) beta: f64,
}

impl NonAnalyticTerm {
    /// The term n Δ^b δ ψ, its parameters in the order the release's Table 2 prints them: a, b, B,
    /// n, C, D, A, β.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the term's eight published parameters, in the order its source prints them"
    )]
    pub(crate) const fn new(
        a: f64,
        b: f64,
        big_b: f64,
        n: f64,
        big_c: f64,
        big_d: f64,
        big_a: f64,
        beta: f64,
    ) -> Self {
        Self {
            n,
            a,
            b,
            big_b,
            big_c,
            big_d,
            big_a,
            beta,
        }
    }
}

/// A power term of an ideal-gas part: n τ^t (Span et al. 2000's nitrogen, its terms in τ⁻¹, τ⁻²
/// and τ⁻³; CoolProp's `IdealGasHelmholtzPower`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IdealPowerTerm {
    pub(crate) n: f64,
    pub(crate) t: f64,
}

impl IdealPowerTerm {
    /// The term n τ^t.
    #[must_use]
    pub(crate) const fn new(n: f64, t: f64) -> Self {
        Self { n, t }
    }
}

/// A Planck–Einstein term of an ideal-gas part: n ln(1 − exp(−θτ)), θ the term's characteristic
/// temperature reduced by the equation's `T_r` (IAPWS R6-95(2018), Eq. (5), its terms 4–8, n°ᵢ
/// and γ°ᵢ; CoolProp's `IdealGasHelmholtzPlanckEinstein`'s n and t, or its
/// `IdealGasHelmholtzPlanckEinsteinFunctionT`'s v in kelvin divided by its `Tcrit`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlanckEinsteinTerm {
    pub(crate) n: f64,
    pub(crate) theta: f64,
}

impl PlanckEinsteinTerm {
    /// The term n ln(1 − exp(−θτ)), θ reduced.
    #[must_use]
    pub(crate) const fn new(n: f64, theta: f64) -> Self {
        Self { n, theta }
    }
}

/// The ideal-gas part of a fluid's dimensionless Helmholtz energy, φ°(δ, τ), as far as its second
/// τ-derivative `φ°_ττ` needs it.
///
/// It holds the coefficient of its ln τ term, its power terms in τ and its
/// Planck–Einstein terms, each as printed, in the τ of the residual it belongs to (each equation's
/// two parts share their reducing temperature). Its ln δ term, its constant and its term linear
/// in τ, the last two fixing only the zeros of energy and entropy, have no second τ-derivative and
/// are not held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelmholtzIdeal {
    pub(crate) log_tau: f64,
    pub(crate) power: &'static [IdealPowerTerm],
    pub(crate) planck_einstein: &'static [PlanckEinsteinTerm],
}

impl HelmholtzIdeal {
    /// `φ°_ττ` at the inverse reduced temperature `tau` (IAPWS R6-95(2018), Table 4): −a ÷ τ² for
    /// the ln τ coefficient a, n t (t − 1) τ^(t − 2) for each power term and −n θ² e^(−θτ) ÷ (1 −
    /// e^(−θτ))² for each Planck–Einstein term, summed in that order, the equations' published
    /// one.
    #[must_use]
    pub(crate) fn second_tau_derivative(&self, tau: f64) -> f64 {
        let mut sum = -self.log_tau / (tau * tau);
        for term in self.power {
            sum += term.n * term.t * (term.t - 1.0) * math::powf(tau, term.t - 2.0);
        }
        for term in self.planck_einstein {
            let e = math::exp(-term.theta * tau);
            let complement = 1.0 - e;
            sum -= term.n * term.theta * term.theta * e / (complement * complement);
        }
        sum
    }
}

/// The Newton steps [`HelmholtzResidual::density_at_pressure`] takes, a fixed count
/// (decision-r09-t13i-cl-licence §6.1). From a saturated liquid's ancillary density, within 10⁻⁴
/// to a few 10⁻³ of the equation's own (4.2 × 10⁻⁴ at most at the six columns' ends), Newton's
/// quadratic convergence along the dense liquid's isotherm meets the pressure to rounding in
/// four, as a test holds.
const NEWTON_STEPS: usize = 4;

/// The Newton steps [`Isotherm::compressed_density`] takes from the saturated liquid, a
/// fixed count (P14.T49.n), which a test holds to meet the pressure to rounding over every row's
/// liquid range up to [`COMPRESSION_CEILING`]: the most any of its cases needs is 10, a part in a
/// thousand below a critical point under the ceiling, where the liquid is about three times its
/// saturated density.
pub(crate) const COMPRESSION_STEPS: usize = 16;

/// The generator version from which a liquid's compression settles, 26.
///
/// From it the compression is [`CompressionRules::Settled`]'s: a planetary bump after 25
/// (P14.T54.b.l's cost lever; perf-24's handoff, the compression's Newton). Below it
/// [`CompressionRules::in_force`] is [`CompressionRules::Fixed`], and every compression is version
/// 24's bit for bit. 26's bump deletes this hold and the tests' switch (`with_compression`), keeps
/// the fixed count as the tests' oracle of the settled one, and deletes the tripwire
/// (`the_settled_compression_is_held_only_until_the_26_bump`).
pub(crate) const SETTLED_COMPRESSION_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// The share of its density at or under which a step of the settled compression ends it: 2⁻⁴⁰,
/// 9.1 × 10⁻¹³ ([`CompressionRules::Settled`]).
///
/// Newton's convergence is quadratic along the isotherm, so the step after one this small would
/// move the density by about its square, under rounding: the stop ends the steps where
/// [`iterate`]'s rounding cycles begin.
const COMPRESSION_TOLERANCE: f64 = 1.0 / 1_099_511_627_776.0;

/// How a liquid's compression steps ([`SETTLED_COMPRESSION_VERSION`]'s hold).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CompressionRules {
    /// Version 25's and before: [`COMPRESSION_STEPS`] from the equation's saturated liquid, ended
    /// where the density repeats ([`iterate`]).
    Fixed,
    /// Version 26's: at most [`COMPRESSION_STEPS`] from the equation's saturated liquid, ended by
    /// the first step that moves the density by at most [`COMPRESSION_TOLERANCE`] of itself.
    Settled,
}

impl CompressionRules {
    /// The rules this generator version gives: the settled compression from
    /// [`SETTLED_COMPRESSION_VERSION`], the fixed count below it; in the tests, the rules a test's
    /// switch sets on its thread (`with_compression`).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = COMPRESSION.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= SETTLED_COMPRESSION_VERSION {
            Self::Settled
        } else {
            Self::Fixed
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The rules a test sets on its thread, `None` for the version's.
    static COMPRESSION: core::cell::Cell<Option<CompressionRules>> =
        const { core::cell::Cell::new(None) };
    /// The settled compressions on this thread and their steps, while a test counts them
    /// ([`with_compression_steps`]).
    static COMPRESSION_COUNT: core::cell::Cell<Option<(u64, u64)>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the compression's rules `rules` on this thread, for the tests to reach
/// 26's settled compression ahead of the bump; the version's rules are set back after, a panic in
/// `run` included. 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_compression<R>(rules: CompressionRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<CompressionRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            COMPRESSION.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(COMPRESSION.with(|cell| cell.replace(Some(rules))));
    run()
}

/// `run`'s result with the settled compressions on this thread and their steps.
///
/// The thread's count is set back after, a panic in `run` included.
#[cfg(test)]
pub(crate) fn with_compression_steps<R>(run: impl FnOnce() -> R) -> (R, (u64, u64)) {
    /// Sets the thread's count back as it drops.
    struct Restore(Option<(u64, u64)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            COMPRESSION_COUNT.with(|cell| cell.set(self.0));
        }
    }
    let restore = Restore(COMPRESSION_COUNT.with(|cell| cell.replace(Some((0, 0)))));
    let result = run();
    let count = COMPRESSION_COUNT
        .with(core::cell::Cell::get)
        .unwrap_or_default();
    drop(restore);
    (result, count)
}

/// The largest step [`Isotherm::compressed_density`] takes, relative to the density it
/// steps from.
pub(crate) const COMPRESSION_STEP_LIMIT: f64 = 0.25;

/// The highest pressure at which a liquid's compression is evaluated, Pa: 1,000 MPa, the upper
/// bound of IAPWS-95's range of validity (IAPWS R6-95(2018); Wagner and Pruß 2002, J. Phys. Chem.
/// Ref. Data 31, 387). The registry's other reference equations are evaluated up to it too, past
/// the pressures some of them were fitted to, a stated extrapolation (CoolProp's files state 80 MPa
/// for oxygen's and 50 MPa for nitrous oxide's, as their rows record); a liquid under a heavier air
/// takes its density at the ceiling, a stated hold (P14.T49.n).
pub(crate) const COMPRESSION_CEILING: Pascals = Pascals::new(1e9);

/// The residual part of a fluid's dimensionless Helmholtz energy, φʳ(δ, τ), with its reducing
/// parameters and the gas constant its coefficients were fitted with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelmholtzResidual {
    pub(crate) reducing_temperature: Kelvin,
    pub(crate) reducing_density: KilogramsPerCubicMetre,
    pub(crate) gas_constant_j_per_kg_k: f64,
    pub(crate) power: &'static [PowerTerm],
    pub(crate) gaussian: &'static [GaussianTerm],
    pub(crate) non_analytic: &'static [NonAnalyticTerm],
    pub(crate) associating: &'static [AssociatingTerm],
}

/// φʳ's first and second derivatives in δ at fixed τ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DensityDerivatives {
    /// ∂φʳ ÷ ∂δ.
    pub(crate) first: f64,
    /// ∂²φʳ ÷ ∂δ².
    pub(crate) second: f64,
}

/// φʳ's second derivative in τ at fixed δ, and its mixed derivative in δ and τ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TemperatureDerivatives {
    /// ∂²φʳ ÷ ∂τ².
    pub(crate) second: f64,
    /// ∂²φʳ ÷ ∂δ ∂τ.
    pub(crate) mixed: f64,
}

impl HelmholtzResidual {
    /// The temperature τ is reduced by, `T_r`: the fluid's critical temperature for every equation
    /// read so far.
    #[must_use]
    pub const fn reducing_temperature(&self) -> Kelvin {
        self.reducing_temperature
    }

    /// The density δ is reduced by, `ρ_r`.
    #[must_use]
    pub const fn reducing_density(&self) -> KilogramsPerCubicMetre {
        self.reducing_density
    }

    /// The specific gas constant R the equation's coefficients were fitted with, J kg⁻¹ K⁻¹, which
    /// its properties must be computed with (IAPWS R6-95(2018), §2).
    #[must_use]
    pub const fn gas_constant_j_per_kg_k(&self) -> f64 {
        self.gas_constant_j_per_kg_k
    }

    /// `φʳ_δ` and `φʳ_δδ` at `density` and `temperature` (IAPWS R6-95(2018), Table 5), the terms
    /// summed in their published order.
    #[must_use]
    pub(crate) fn density_derivatives(
        &self,
        density: KilogramsPerCubicMetre,
        temperature: Kelvin,
    ) -> DensityDerivatives {
        let tau = self.reducing_temperature.value() / temperature.value();
        self.density_derivatives_with(density, tau, |_, t| math::powf(tau, t), self.delta_top())
    }

    /// The highest power of δ the terms' derivatives in δ read: a power term's d − 1 and c, and a
    /// Gaussian or associating term's d ([`DeltaPowers`]).
    #[must_use]
    fn delta_top(&self) -> i32 {
        let power = self.power.iter().map(|term| (term.d - 1).max(term.c));
        let gaussian = self.gaussian.iter().map(|term| term.d);
        let associating = self.associating.iter().map(|term| term.d);
        power.chain(gaussian).chain(associating).fold(0, i32::max)
    }

    /// The equation along the isotherm at `temperature`, its τ-powers taken once
    /// ([`Isotherm`]).
    #[must_use]
    pub(crate) fn isotherm(&self, temperature: Kelvin) -> Isotherm<'_> {
        let tau = self.reducing_temperature.value() / temperature.value();
        let base = math::PowBase::new(tau);
        let tau_t = self
            .power
            .iter()
            .map(|term| term.t)
            .chain(self.gaussian.iter().map(|term| term.t))
            .chain(self.associating.iter().map(|term| term.t))
            .map(|t| base.powf(t))
            .collect();
        Isotherm {
            equation: self,
            temperature,
            tau,
            tau_t,
            delta_top: self.delta_top(),
        }
    }

    /// [`density_derivatives`](Self::density_derivatives) at `density` and the inverse reduced
    /// temperature `tau`, with `tau_t(k, t)` giving τ^t of the `k`-th term counted over the power,
    /// Gaussian and associating terms in that order, `t` its exponent, and δ's powers taken up to
    /// `delta_top` ([`delta_top`](Self::delta_top)).
    #[must_use]
    fn density_derivatives_with(
        &self,
        density: KilogramsPerCubicMetre,
        tau: f64,
        tau_t: impl Fn(usize, f64) -> f64,
        delta_top: i32,
    ) -> DensityDerivatives {
        let delta = density.value() / self.reducing_density.value();
        let powers = DeltaPowers::new(delta, delta_top);
        let mut first = 0.0;
        let mut second = 0.0;
        // δ^c and exp(−δ^c) of the last exponential term, which the next term with the same c
        // reads again: the published equations list their terms by c (IAPWS-95's 44 exponential
        // terms take five values of it).
        let mut shared: Option<(i32, f64, f64)> = None;
        for (k, term) in self.power.iter().enumerate() {
            let d = f64::from(term.d);
            let tau_t = tau_t(k, term.t);
            if term.c == 0 {
                first += term.n * d * powers.get(term.d - 1) * tau_t;
                second += term.n * d * (d - 1.0) * powers.get(term.d - 2) * tau_t;
            } else {
                let c = f64::from(term.c);
                let (delta_c, e) = match shared {
                    Some((at, delta_c, e)) if at == term.c => (delta_c, e),
                    _ => {
                        let delta_c = powers.get(term.c);
                        let e = math::exp(-delta_c);
                        shared = Some((term.c, delta_c, e));
                        (delta_c, e)
                    }
                };
                let g = d - c * delta_c;
                first += term.n * e * powers.get(term.d - 1) * tau_t * g;
                second += term.n
                    * e
                    * powers.get(term.d - 2)
                    * tau_t
                    * (g * (d - 1.0 - c * delta_c) - c * c * delta_c);
            }
        }
        let gaussian_from = self.power.len();
        for (k, term) in self.gaussian.iter().enumerate() {
            let d = f64::from(term.d);
            let dd = delta - term.epsilon;
            let tt = tau - term.gamma;
            let e = math::exp(-term.alpha * dd * dd - term.beta * tt * tt);
            let tau_t = tau_t(gaussian_from + k, term.t);
            let delta_d = powers.get(term.d);
            first += term.n * delta_d * tau_t * e * (d / delta - 2.0 * term.alpha * dd);
            second += term.n
                * tau_t
                * e
                * (-2.0 * term.alpha * delta_d + 4.0 * term.alpha * term.alpha * delta_d * dd * dd
                    - 4.0 * d * term.alpha * powers.get(term.d - 1) * dd
                    + d * (d - 1.0) * powers.get(term.d - 2));
        }
        for term in self.non_analytic {
            let (first_term, second_term) = term.density_derivatives(delta, tau);
            first += first_term;
            second += second_term;
        }
        let associating_from = gaussian_from + self.gaussian.len();
        for (k, term) in self.associating.iter().enumerate() {
            let d = f64::from(term.d);
            let dd = delta - term.epsilon;
            let tt = tau - term.gamma;
            let e = math::exp(-term.eta * dd * dd + 1.0 / (term.beta * tt * tt + term.b));
            let tau_t = tau_t(associating_from + k, term.t);
            let delta_d = powers.get(term.d);
            first += term.n * delta_d * tau_t * e * (d / delta - 2.0 * term.eta * dd);
            second += term.n
                * tau_t
                * e
                * (-2.0 * term.eta * delta_d + 4.0 * term.eta * term.eta * delta_d * dd * dd
                    - 4.0 * d * term.eta * powers.get(term.d - 1) * dd
                    + d * (d - 1.0) * powers.get(term.d - 2));
        }
        DensityDerivatives { first, second }
    }

    /// The pressure at `density` and `temperature`: p = `ρRT` (1 + `δφʳ_δ`).
    #[must_use]
    pub fn pressure(&self, density: KilogramsPerCubicMetre, temperature: Kelvin) -> Pascals {
        let delta = density.value() / self.reducing_density.value();
        let derivatives = self.density_derivatives(density, temperature);
        Pascals::new(
            density.value()
                * self.gas_constant_j_per_kg_k
                * temperature.value()
                * (1.0 + delta * derivatives.first),
        )
    }

    /// The isothermal compressibility at `density` and `temperature`, Pa⁻¹: `κ_T` = 1 ÷ [`ρRT` (1 +
    /// `2δφʳ_δ` + `δ²φʳ_δδ`)] (IAPWS R6-95(2018), Table 3, from p's derivative).
    ///
    /// It is the equation's at that state, whatever pressure the equation gives there: a liquid's
    /// record evaluates it at its own density, the saturated liquid's from the row's ancillary
    /// equation, whose pressure lies within about 0.1 megapascal of the vapour's for water below
    /// 373 K.
    #[must_use]
    pub fn isothermal_compressibility_per_pa(
        &self,
        density: KilogramsPerCubicMetre,
        temperature: Kelvin,
    ) -> f64 {
        let delta = density.value() / self.reducing_density.value();
        let derivatives = self.density_derivatives(density, temperature);
        1.0 / (density.value()
            * self.gas_constant_j_per_kg_k
            * temperature.value()
            * (1.0 + 2.0 * delta * derivatives.first + delta * delta * derivatives.second))
    }

    /// `φʳ_ττ` and `φʳ_δτ` at `density` and `temperature` (IAPWS R6-95(2018), Table 5), the terms
    /// summed in their published order, as [`density_derivatives`](Self::density_derivatives)
    /// sums them. An associating term's τ factor is exp(1 ÷ u), u = β(τ − γ)² + b, whose
    /// logarithm's derivatives are −2β(τ − γ) ÷ u² and −2β ÷ u² + 8β²(τ − γ)² ÷ u³.
    #[must_use]
    pub(crate) fn temperature_derivatives(
        &self,
        density: KilogramsPerCubicMetre,
        temperature: Kelvin,
    ) -> TemperatureDerivatives {
        let delta = density.value() / self.reducing_density.value();
        let tau = self.reducing_temperature.value() / temperature.value();
        let mut second = 0.0;
        let mut mixed = 0.0;
        for term in self.power {
            let d = f64::from(term.d);
            let t = term.t;
            let tau_t2 = math::powf(tau, t - 2.0);
            let tau_t1 = math::powf(tau, t - 1.0);
            if term.c == 0 {
                second += term.n * t * (t - 1.0) * math::powi(delta, term.d) * tau_t2;
                mixed += term.n * d * t * math::powi(delta, term.d - 1) * tau_t1;
            } else {
                let c = f64::from(term.c);
                let delta_c = math::powi(delta, term.c);
                let e = math::exp(-delta_c);
                second += term.n * t * (t - 1.0) * math::powi(delta, term.d) * tau_t2 * e;
                mixed +=
                    term.n * t * math::powi(delta, term.d - 1) * tau_t1 * (d - c * delta_c) * e;
            }
        }
        for term in self.gaussian {
            let d = f64::from(term.d);
            let dd = delta - term.epsilon;
            let tt = tau - term.gamma;
            let e = math::exp(-term.alpha * dd * dd - term.beta * tt * tt);
            let f = term.n * math::powi(delta, term.d) * math::powf(tau, term.t) * e;
            let in_delta = d / delta - 2.0 * term.alpha * dd;
            let in_tau = term.t / tau - 2.0 * term.beta * tt;
            second += f * (in_tau * in_tau - term.t / (tau * tau) - 2.0 * term.beta);
            mixed += f * in_delta * in_tau;
        }
        for term in self.non_analytic {
            let (second_term, mixed_term) = term.temperature_derivatives(delta, tau);
            second += second_term;
            mixed += mixed_term;
        }
        for term in self.associating {
            let d = f64::from(term.d);
            let dd = delta - term.epsilon;
            let tt = tau - term.gamma;
            let u = term.beta * tt * tt + term.b;
            let e = math::exp(-term.eta * dd * dd + 1.0 / u);
            let f = term.n * math::powi(delta, term.d) * math::powf(tau, term.t) * e;
            let in_delta = d / delta - 2.0 * term.eta * dd;
            let in_tau = term.t / tau - 2.0 * term.beta * tt / (u * u);
            let curvature =
                -2.0 * term.beta / (u * u) + 8.0 * term.beta * term.beta * tt * tt / (u * u * u);
            second += f * (in_tau * in_tau - term.t / (tau * tau) + curvature);
            mixed += f * in_delta * in_tau;
        }
        TemperatureDerivatives { second, mixed }
    }

    /// The specific isochoric heat capacity at `density` and `temperature`, J kg⁻¹ K⁻¹, with
    /// `ideal` the same equation's ideal-gas part.
    ///
    /// `c_v` = −R τ² (`φ°_ττ` + `φʳ_ττ`) (IAPWS R6-95(2018), Table 3), with the gas constant the
    /// coefficients were fitted with. The isobaric heat capacity's example
    /// ([`isobaric_heat_capacity_j_per_kg_k`](Self::isobaric_heat_capacity_j_per_kg_k)) shows where
    /// an equation and its ideal part come from.
    #[must_use]
    pub fn isochoric_heat_capacity_j_per_kg_k(
        &self,
        ideal: &HelmholtzIdeal,
        density: KilogramsPerCubicMetre,
        temperature: Kelvin,
    ) -> f64 {
        let tau = self.reducing_temperature.value() / temperature.value();
        let residual = self.temperature_derivatives(density, temperature);
        -self.gas_constant_j_per_kg_k
            * tau
            * tau
            * (ideal.second_tau_derivative(tau) + residual.second)
    }

    /// The specific isobaric heat capacity at `density` and `temperature`, J kg⁻¹ K⁻¹, with
    /// `ideal` the same equation's ideal-gas part (IAPWS R6-95(2018), Table 3):
    ///
    /// `c_p` ÷ R = −τ² (`φ°_ττ` + `φʳ_ττ`) + (1 + `δφʳ_δ` − `δτφʳ_δτ`)² ÷ (1 + `2δφʳ_δ` +
    /// `δ²φʳ_δδ`),
    ///
    /// with the gas constant the coefficients were fitted with, as
    /// [`isothermal_compressibility_per_pa`](Self::isothermal_compressibility_per_pa) takes it.
    ///
    /// # Examples
    ///
    /// Liquid methane at Titan's surface, 93.65 K, on its saturated liquid from the phase
    /// diagram's density and vapour pressure, holds about 3.38 kJ kg⁻¹ K⁻¹:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::Kelvin;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let methane = by_key("CH4").ok_or("methane is a row")?.substance();
    /// let column = methane
    ///     .liquid()
    ///     .and_then(|columns| columns.specific_heat())
    ///     .ok_or("methane's liquid has a specific heat")?;
    /// let diagram = methane.phase_diagram().ok_or("methane has a phase diagram")?;
    /// let t = Kelvin::new(93.65);
    /// let start = diagram.liquid_density(t).ok_or("93.65 K is liquid")?;
    /// let equation = column.equation();
    /// let density = equation.density_at_pressure(start, diagram.saturation_pressure(t), t);
    /// let c_p = equation.isobaric_heat_capacity_j_per_kg_k(column.ideal(), density, t);
    /// assert!((3_370.0..3_390.0).contains(&c_p), "{c_p}");
    /// // The registry's column is this evaluation, to the bit.
    /// let column_value = methane.liquid_specific_heat_j_per_kg_k(t).map(f64::to_bits);
    /// assert_eq!(column_value, Some(c_p.to_bits()));
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn isobaric_heat_capacity_j_per_kg_k(
        &self,
        ideal: &HelmholtzIdeal,
        density: KilogramsPerCubicMetre,
        temperature: Kelvin,
    ) -> f64 {
        let delta = density.value() / self.reducing_density.value();
        let tau = self.reducing_temperature.value() / temperature.value();
        let in_delta = self.density_derivatives(density, temperature);
        let in_tau = self.temperature_derivatives(density, temperature);
        let isochoric = -tau * tau * (ideal.second_tau_derivative(tau) + in_tau.second);
        let numerator = 1.0 + delta * in_delta.first - delta * tau * in_tau.mixed;
        let denominator = 1.0 + 2.0 * delta * in_delta.first + delta * delta * in_delta.second;
        self.gas_constant_j_per_kg_k * (isochoric + numerator * numerator / denominator)
    }

    /// The density at which the equation gives `pressure` at `temperature`, from `start`.
    ///
    /// It takes a fixed four Newton steps along the isotherm, ρ ← ρ + (p − p(ρ, T)) ρ `κ_T`(ρ, T),
    /// by [`pressure`](Self::pressure) and
    /// [`isothermal_compressibility_per_pa`](Self::isothermal_compressibility_per_pa)
    /// (decision-r09-t13i-cl-licence §6.1), and stays on the branch it starts on, where (∂p ÷ ∂ρ)_T
    /// is positive. So `start` picks the phase: started from a saturated liquid's ancillary density
    /// at the vapour pressure, it is the equation's own saturated liquid.
    #[must_use]
    pub fn density_at_pressure(
        &self,
        start: KilogramsPerCubicMetre,
        pressure: Pascals,
        temperature: Kelvin,
    ) -> KilogramsPerCubicMetre {
        self.isotherm(temperature)
            .density_at_pressure(start, pressure)
    }

    /// [`Isotherm::compressed_density`] at `temperature`, for the tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn compressed_density(
        &self,
        saturated: KilogramsPerCubicMetre,
        pressure: Pascals,
        temperature: Kelvin,
    ) -> KilogramsPerCubicMetre {
        self.compressed_density_in(saturated, pressure, temperature, COMPRESSION_STEPS)
    }

    /// [`Isotherm::compressed_density`] at `temperature` in `steps` steps, for the tests that
    /// compare the count with more.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn compressed_density_in(
        &self,
        saturated: KilogramsPerCubicMetre,
        pressure: Pascals,
        temperature: Kelvin,
        steps: usize,
    ) -> KilogramsPerCubicMetre {
        self.isotherm(temperature)
            .compressed_density_in(saturated, pressure, steps)
    }
}

/// How many powers of δ [`DeltaPowers`] holds, from δ⁻² up: to δ²¹, past every equation's
/// highest (IAPWS-95's δ¹⁴).
const DELTA_POWERS: usize = 24;

/// δ^k for every k from −2 (a Gaussian term's d − 2 at d = 0) to an equation's highest, each
/// [`math::powi`]`(δ, k)`, the call the terms made one by one: one evaluation of `φʳ_δ` and
/// `φʳ_δδ` took 111 of them for IAPWS-95's 56 terms, against 17 here. With [`iterate`]'s cycles
/// of any period, `generate` over the first 20 sample systems fell from 595 to 557 ms (pod, 8
/// vCPU, P14.T54.b.l's probe).
#[derive(Debug, Clone, Copy, PartialEq)]
struct DeltaPowers {
    delta: f64,
    top: i32,
    table: [f64; DELTA_POWERS],
}

impl DeltaPowers {
    /// δ's powers from −2 to `top`, or to δ²¹, the table's last.
    #[must_use]
    fn new(delta: f64, top: i32) -> Self {
        let mut table = [0.0; DELTA_POWERS];
        for (slot, k) in table.iter_mut().zip(-2..=top) {
            *slot = math::powi(delta, k);
        }
        Self { delta, top, table }
    }

    /// [`math::powi`]`(δ, k)`, to the bit: the table's where it holds k, the call otherwise.
    #[must_use]
    fn get(&self, k: i32) -> f64 {
        match usize::try_from(k + 2) {
            Ok(at) if k <= self.top && at < DELTA_POWERS => self.table[at],
            _ => math::powi(self.delta, k),
        }
    }
}

/// One equation along one isotherm: τ^t of each of its terms, which every evaluation of `φʳ_δ`
/// and `φʳ_δδ` at that temperature reads, taken once.
///
/// A density solved at a pressure evaluates the equation at one temperature once a Newton step: a
/// liquid's compression (P14.T49.n) takes [`NEWTON_STEPS`] to the equation's saturated liquid and
/// [`COMPRESSION_STEPS`] more, and the partition's sweeps ask for one at every sweep of every
/// greenhouse step. Each power is [`math::PowBase`]'s, bit for bit [`math::powf`]'s, so every
/// evaluation is [`HelmholtzResidual::density_derivatives`]' to the bit.
///
/// At version 24 the compression was 54% of `generate` and `snapshot_at` over the first 20 sample
/// systems, most of it `powf` in τ. With the isotherm, one evaluation a step for
/// [`density_at_pressure`](Self::density_at_pressure), exp(−δ^c) shared by the terms of one c
/// and the steps ending where the density repeats ([`iterate`]), `generate` over them fell from
/// 1,998 to 630 ms and `snapshot_at` from 3,208 to 2,125 ms (pod, 8 vCPU, P14.T54.b.l's probe).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Isotherm<'a> {
    equation: &'a HelmholtzResidual,
    temperature: Kelvin,
    /// τ = `T_r` ÷ T.
    tau: f64,
    /// τ^t of every power term, then every Gaussian term, then every associating term, each in
    /// its published order.
    tau_t: Vec<f64>,
    /// [`HelmholtzResidual::delta_top`].
    delta_top: i32,
}

impl Isotherm<'_> {
    /// [`HelmholtzResidual::density_derivatives`] at `density` on this isotherm, to the bit.
    #[must_use]
    pub(crate) fn density_derivatives(
        &self,
        density: KilogramsPerCubicMetre,
    ) -> DensityDerivatives {
        self.equation.density_derivatives_with(
            density,
            self.tau,
            |k, _| self.tau_t[k],
            self.delta_top,
        )
    }

    /// [`HelmholtzResidual::density_at_pressure`] on this isotherm: each step's pressure and
    /// `κ_T` read one evaluation of the equation, as [`HelmholtzResidual::pressure`] and
    /// [`HelmholtzResidual::isothermal_compressibility_per_pa`] would each take it, to the bit.
    #[must_use]
    pub(crate) fn density_at_pressure(
        &self,
        start: KilogramsPerCubicMetre,
        pressure: Pascals,
    ) -> KilogramsPerCubicMetre {
        let equation = self.equation;
        let temperature = self.temperature.value();
        let step = |density: f64| {
            let delta = density / equation.reducing_density.value();
            let derivatives = self.density_derivatives(KilogramsPerCubicMetre::new(density));
            let at = density * equation.gas_constant_j_per_kg_k * temperature;
            let here = at * (1.0 + delta * derivatives.first);
            let compressibility = 1.0
                / (at
                    * (1.0 + 2.0 * delta * derivatives.first + delta * delta * derivatives.second));
            let miss = pressure.value() - here;
            Some(density + miss * density * compressibility)
        };
        KilogramsPerCubicMetre::new(iterate(start.value(), NEWTON_STEPS, step))
    }

    /// The compressed liquid's density at `pressure` on this isotherm, from `saturated`, the
    /// equation's own saturated liquid there (the liquids' compression, P14.T49.n), for a
    /// `pressure` above the saturated liquid's: [`COMPRESSION_STEPS`] Newton steps along the
    /// isotherm, ρ ← ρ + (p − p(ρ, T)) ρ `κ_T`(ρ, T) as in
    /// [`HelmholtzResidual::density_at_pressure`], each held to at most
    /// [`COMPRESSION_STEP_LIMIT`] of ρ.
    ///
    /// Above the saturated liquid p(ρ) rises along the isotherm and is convex, so a step from below
    /// the root passes it, and every step from above falls back toward it without passing it, never
    /// toward the spinodal below the saturated liquid. The limit keeps the first steps from a
    /// liquid near its critical point, whose `κ_T` is large, inside the equation's dense liquid.
    /// A step that is not finite, from a stiffness ∂p ÷ ∂ρ of zero or not a number near a
    /// spinodal, ends the iteration at the density it would step from, so that a finite start
    /// never gives a density that is not a number.
    ///
    /// From generator version 26 the steps end by [`CompressionRules::Settled`]'s tolerance
    /// ([`settled_density`](Self::settled_density)), held until then
    /// ([`SETTLED_COMPRESSION_VERSION`]).
    #[must_use]
    pub(crate) fn compressed_density(
        &self,
        saturated: KilogramsPerCubicMetre,
        pressure: Pascals,
    ) -> KilogramsPerCubicMetre {
        match CompressionRules::in_force() {
            CompressionRules::Fixed => {
                self.compressed_density_in(saturated, pressure, COMPRESSION_STEPS)
            }
            CompressionRules::Settled => self.settled_density(saturated, pressure),
        }
    }

    /// [`compressed_density`](Self::compressed_density) in `steps` steps.
    #[must_use]
    pub(crate) fn compressed_density_in(
        &self,
        saturated: KilogramsPerCubicMetre,
        pressure: Pascals,
        steps: usize,
    ) -> KilogramsPerCubicMetre {
        KilogramsPerCubicMetre::new(iterate(
            saturated.value(),
            steps,
            self.compression_step(pressure),
        ))
    }

    /// The compressed liquid's density at `pressure` on this isotherm from `saturated` by version
    /// 26's rules ([`CompressionRules::Settled`]): [`compressed_density`](Self::compressed_density)'s
    /// steps, ended by the first that moves the density by at most [`COMPRESSION_TOLERANCE`] of
    /// itself, or at the most, [`COMPRESSION_STEPS`]; where a step is not finite, the density it
    /// would step from.
    #[must_use]
    pub(crate) fn settled_density(
        &self,
        saturated: KilogramsPerCubicMetre,
        pressure: Pascals,
    ) -> KilogramsPerCubicMetre {
        let step = self.compression_step(pressure);
        let mut density = saturated.value();
        #[cfg(test)]
        let mut taken = 0_u64;
        for _ in 0..COMPRESSION_STEPS {
            let Some(next) = step(density) else {
                break;
            };
            #[cfg(test)]
            {
                taken += 1;
            }
            // By comparison, so that a step that is not a number ends nothing.
            let settled = (next - density).abs() <= COMPRESSION_TOLERANCE * density;
            density = next;
            if settled {
                break;
            }
        }
        #[cfg(test)]
        COMPRESSION_COUNT.with(|cell| {
            if let Some((calls, steps)) = cell.get() {
                cell.set(Some((calls + 1, steps + taken)));
            }
        });
        KilogramsPerCubicMetre::new(density)
    }

    /// One step of the compression toward `pressure` on this isotherm
    /// ([`compressed_density`](Self::compressed_density)'s), `None` where it is not finite.
    fn compression_step(&self, pressure: Pascals) -> impl Fn(f64) -> Option<f64> + '_ {
        let equation = self.equation;
        let rt = equation.gas_constant_j_per_kg_k * self.temperature.value();
        move |density: f64| {
            let delta = density / equation.reducing_density.value();
            let derivatives = self.density_derivatives(KilogramsPerCubicMetre::new(density));
            let here = density * rt * (1.0 + delta * derivatives.first);
            let stiffness =
                rt * (1.0 + 2.0 * delta * derivatives.first + delta * delta * derivatives.second);
            // (p − p(ρ)) ρ κ_T, with κ_T = 1 ÷ (ρ ∂p ÷ ∂ρ), held to the limit either way; by
            // comparison rather than `clamp`, which would panic on a limit that is not a number.
            let step = (pressure.value() - here) / stiffness;
            // Zero or not-a-number stiffness near a spinodal: keep the last finite density.
            if !step.is_finite() {
                return None;
            }
            let limit = COMPRESSION_STEP_LIMIT * density;
            Some(
                density
                    + if step > limit {
                        limit
                    } else if step < -limit {
                        -limit
                    } else {
                        step
                    },
            )
        }
    }
}

/// How many of [`iterate`]'s densities it keeps to find a repeat in: past
/// [`COMPRESSION_STEPS`] and [`NEWTON_STEPS`], and the tests' longer runs.
const ITERATE_HISTORY: usize = 32;

/// The density after `steps` Newton steps `step` from `start`, or where a step gives `None`, the
/// density it would step from: what a loop of `steps` calls gives, to the bit.
///
/// A step reads the density alone, so once a density repeats one before it, bit for bit, the
/// sequence cycles from there and the rest of the steps are known, not taken: the cycle's period
/// and the steps left give the density the loop ends on. Near its root a compression's rounding
/// settles into cycles of one to six densities: over the first 20 sample systems the compression
/// took 12.95 steps a call when only fixed points and 2-cycles ended it, and 11.14 at any period
/// (P14.T54.b.l's count).
#[must_use]
fn iterate(start: f64, steps: usize, step: impl Fn(f64) -> Option<f64>) -> f64 {
    let mut seen = [0.0; ITERATE_HISTORY];
    seen[0] = start;
    let mut density = start;
    for taken in 0..steps {
        let Some(next) = step(density) else {
            break;
        };
        // Every density from the start to this one is kept while the history holds them.
        if taken + 1 < ITERATE_HISTORY {
            if let Some(from) = seen[..=taken]
                .iter()
                .position(|before| before.total_cmp(&next).is_eq())
            {
                let period = taken + 1 - from;
                return seen[from + (steps - from) % period];
            }
            seen[taken + 1] = next;
        }
        density = next;
    }
    density
}

impl NonAnalyticTerm {
    /// The term's `φʳ_δ` and `φʳ_δδ` at `delta` and `tau` (IAPWS R6-95(2018), Table 5 and its
    /// continuation's derivatives of Δ^b and ψ). At δ = 1 the distance function's derivatives
    /// ∂Δ ÷ ∂δ and ∂²Δ ÷ ∂δ² are taken as 0, their limit there when β < ½ and a > 1 (IAPWS-95's
    /// β = 0.3 and a = 3.5; an equation with other parameters checks this before using it); the
    /// term's own derivatives keep their Δ^b ψ parts.
    #[must_use]
    fn density_derivatives(&self, delta: f64, tau: f64) -> (f64, f64) {
        let dm = delta - 1.0;
        let tm = tau - 1.0;
        let dm2 = dm * dm;
        // Each base's powers share its logarithm, bit for bit `math::powf`'s.
        let dm2_base = math::PowBase::new(dm2);
        let half_inverse_beta = 1.0 / (2.0 * self.beta);
        let theta = (1.0 - tau) + self.big_a * dm2_base.powf(half_inverse_beta);
        let distance = theta * theta + self.big_b * dm2_base.powf(self.a);
        let psi = math::exp(-self.big_c * dm2 - self.big_d * tm * tm);
        let psi_d = -2.0 * self.big_c * dm * psi;
        let psi_dd = (2.0 * self.big_c * dm2 - 1.0) * 2.0 * self.big_c * psi;
        let (distance_d, distance_dd) = if dm2 > 0.0 {
            let inner = dm2_base.powf(half_inverse_beta - 1.0);
            let d1 = dm
                * (self.big_a * theta * 2.0 / self.beta * inner
                    + 2.0 * self.big_b * self.a * dm2_base.powf(self.a - 1.0));
            let d2 = d1 / dm
                + dm2
                    * (4.0 * self.big_b * self.a * (self.a - 1.0) * dm2_base.powf(self.a - 2.0)
                        + 2.0 * self.big_a * self.big_a / (self.beta * self.beta) * inner * inner
                        + self.big_a * theta * 4.0 / self.beta
                            * (half_inverse_beta - 1.0)
                            * dm2_base.powf(half_inverse_beta - 2.0));
            (d1, d2)
        } else {
            (0.0, 0.0)
        };
        let distance_base = math::PowBase::new(distance);
        let power = distance_base.powf(self.b);
        let below = distance_base.powf(self.b - 1.0);
        let power_d = self.b * below * distance_d;
        let power_dd = self.b
            * (below * distance_dd
                + (self.b - 1.0) * distance_base.powf(self.b - 2.0) * distance_d * distance_d);
        let first = self.n * (power * (psi + delta * psi_d) + power_d * delta * psi);
        let second = self.n
            * (power * (2.0 * psi_d + delta * psi_dd)
                + 2.0 * power_d * (psi + delta * psi_d)
                + power_dd * delta * psi);
        (first, second)
    }

    /// The term's `φʳ_ττ` and `φʳ_δτ` at `delta` and `tau` (IAPWS R6-95(2018), Table 5 and its
    /// continuation's derivatives of Δ^b and ψ): with ∂Δ^b ÷ ∂τ = −2θbΔ^(b − 1), ∂²Δ^b ÷ ∂τ² =
    /// 2bΔ^(b − 1) + 4θ²b(b − 1)Δ^(b − 2) and ∂²Δ^b ÷ ∂δ∂τ = −Ab(2 ÷ β)Δ^(b − 1)(δ − 1)[(δ −
    /// 1)²]^(1 ÷ 2β − 1) − 2θb(b − 1)Δ^(b − 2) ∂Δ ÷ ∂δ. At δ = 1 the derivatives in δ of the
    /// distance function, and the first part of the mixed one, are taken as 0, their limit there,
    /// as [`density_derivatives`](Self::density_derivatives) takes them.
    #[must_use]
    fn temperature_derivatives(&self, delta: f64, tau: f64) -> (f64, f64) {
        let dm = delta - 1.0;
        let tm = tau - 1.0;
        let dm2 = dm * dm;
        let half_inverse_beta = 1.0 / (2.0 * self.beta);
        let theta = (1.0 - tau) + self.big_a * math::powf(dm2, half_inverse_beta);
        let distance = theta * theta + self.big_b * math::powf(dm2, self.a);
        let psi = math::exp(-self.big_c * dm2 - self.big_d * tm * tm);
        let psi_d = -2.0 * self.big_c * dm * psi;
        let psi_tau = -2.0 * self.big_d * tm * psi;
        let psi_tau_tau = (2.0 * self.big_d * tm * tm - 1.0) * 2.0 * self.big_d * psi;
        let psi_delta_tau = 4.0 * self.big_c * self.big_d * dm * tm * psi;
        let (distance_d, mixed_first_part) = if dm2 > 0.0 {
            let d1 = dm
                * (self.big_a * theta * 2.0 / self.beta * math::powf(dm2, half_inverse_beta - 1.0)
                    + 2.0 * self.big_b * self.a * math::powf(dm2, self.a - 1.0));
            let first_part = -self.big_a * self.b * 2.0 / self.beta
                * math::powf(distance, self.b - 1.0)
                * dm
                * math::powf(dm2, half_inverse_beta - 1.0);
            (d1, first_part)
        } else {
            (0.0, 0.0)
        };
        let power = math::powf(distance, self.b);
        let power_d = self.b * math::powf(distance, self.b - 1.0) * distance_d;
        let power_tau = -2.0 * theta * self.b * math::powf(distance, self.b - 1.0);
        let power_tau_tau = 2.0 * self.b * math::powf(distance, self.b - 1.0)
            + 4.0 * theta * theta * self.b * (self.b - 1.0) * math::powf(distance, self.b - 2.0);
        let power_delta_tau = mixed_first_part
            - 2.0
                * theta
                * self.b
                * (self.b - 1.0)
                * math::powf(distance, self.b - 2.0)
                * distance_d;
        let second = self.n
            * delta
            * (power_tau_tau * psi + 2.0 * power_tau * psi_tau + power * psi_tau_tau);
        let mixed = self.n
            * (power * (psi_tau + delta * psi_delta_tau)
                + delta * power_d * psi_tau
                + power_tau * (psi + delta * psi_d)
                + power_delta_tau * delta * psi);
        (second, mixed)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// [`iterate`] is the plain loop of `steps` steps to the bit, at every count of steps to 40,
    /// past its history: over sequences of a tail of up to five densities into a cycle of one to
    /// seven, of tails that reach the cycle only past the history, one that never repeats, and one
    /// whose step ends.
    #[test]
    fn iterating_ends_where_the_plain_loop_does() {
        let plain = |start: f64, steps: usize, step: &dyn Fn(f64) -> Option<f64>| {
            let mut density = start;
            for _ in 0..steps {
                let Some(next) = step(density) else {
                    break;
                };
                density = next;
            }
            density
        };
        for tail in (0..=5_u32).chain(28..=36) {
            for period in 1..=7_u32 {
                // The densities 1, 2, …: the tail's, then the cycle's, whose last returns to its
                // first.
                let last = tail + period;
                let step = |x: f64| {
                    let next = if x.total_cmp(&f64::from(last)).is_eq() {
                        f64::from(tail + 1)
                    } else {
                        x + 1.0
                    };
                    Some(next)
                };
                for steps in 0..=40 {
                    assert_same_bits(iterate(1.0, steps, step), plain(1.0, steps, &step));
                }
            }
        }
        let ending = |x: f64| (x < 4.0).then_some(x + 1.0);
        let rising = |x: f64| Some(x + 1.0);
        for steps in 0..=40 {
            assert_same_bits(iterate(1.0, steps, ending), plain(1.0, steps, &ending));
            assert_same_bits(iterate(1.0, steps, rising), plain(1.0, steps, &rising));
        }
    }

    /// [`DeltaPowers`] gives [`math::powi`]'s bits at every exponent, inside its table and past
    /// it.
    #[test]
    fn the_powers_of_delta_are_powi_s() {
        for delta in [0.0, 1e-3, 0.37, 1.0, 1.0 + 1e-9, 2.8, 3.5] {
            let powers = DeltaPowers::new(delta, 14);
            for k in -4..=30 {
                assert_same_bits(powers.get(k), math::powi(delta, k));
            }
        }
    }
}
