//! A fluid's reference equation of state, as the residual part of its dimensionless Helmholtz
//! energy (plan 14, P14.T49.h): what a liquid's isothermal compressibility, and so its scattering,
//! is computed from (decision-r11-palette-liquids §1.8).
//!
//! A multiparameter reference equation (IAPWS-95; Setzmann and Wagner 1991; Span et al. 2000; and
//! the rest) writes the Helmholtz energy f of the fluid as φ(δ, τ) = f ÷ (RT) = φ°(δ, τ) + φʳ(δ,
//! τ), in the reduced density δ = ρ ÷ `ρ_r` and the inverse reduced temperature τ = `T_r` ÷ T. Its
//! residual part φʳ is a sum of terms of four published kinds ([`HelmholtzResidual`]): power and
//! exponential terms, Gaussian bell-shaped terms, non-analytic critical terms and Gao et al. 2023's
//! associating terms, each with its coefficients as printed. Every property follows from its
//! derivatives (IAPWS R6-95(2018), Table 3); a liquid's record needs only the pressure's derivative
//! along an isotherm, so only `φʳ_δ` and `φʳ_δδ` are evaluated here:
//!
//! - p = `ρRT` (1 + `δφʳ_δ`);
//! - `κ_T` = ρ⁻¹ (∂ρ ÷ ∂p)_T = 1 ÷ [`ρRT` (1 + `2δφʳ_δ` + `δ²φʳ_δδ`)].
//!
//! The terms are summed in their published order, each power through the crate's deterministic
//! `math` (sim-determinism): an integer power of δ by [`math::powi`], a real power by
//! [`math::powf`], each exponential by [`math::exp`].

use crate::math;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

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
        let delta = density.value() / self.reducing_density.value();
        let tau = self.reducing_temperature.value() / temperature.value();
        let mut first = 0.0;
        let mut second = 0.0;
        for term in self.power {
            let d = f64::from(term.d);
            let tau_t = math::powf(tau, term.t);
            if term.c == 0 {
                first += term.n * d * math::powi(delta, term.d - 1) * tau_t;
                second += term.n * d * (d - 1.0) * math::powi(delta, term.d - 2) * tau_t;
            } else {
                let c = f64::from(term.c);
                let delta_c = math::powi(delta, term.c);
                let e = math::exp(-delta_c);
                let g = d - c * delta_c;
                first += term.n * e * math::powi(delta, term.d - 1) * tau_t * g;
                second += term.n
                    * e
                    * math::powi(delta, term.d - 2)
                    * tau_t
                    * (g * (d - 1.0 - c * delta_c) - c * c * delta_c);
            }
        }
        for term in self.gaussian {
            let d = f64::from(term.d);
            let dd = delta - term.epsilon;
            let tt = tau - term.gamma;
            let e = math::exp(-term.alpha * dd * dd - term.beta * tt * tt);
            let tau_t = math::powf(tau, term.t);
            let delta_d = math::powi(delta, term.d);
            first += term.n * delta_d * tau_t * e * (d / delta - 2.0 * term.alpha * dd);
            second += term.n
                * tau_t
                * e
                * (-2.0 * term.alpha * delta_d + 4.0 * term.alpha * term.alpha * delta_d * dd * dd
                    - 4.0 * d * term.alpha * math::powi(delta, term.d - 1) * dd
                    + d * (d - 1.0) * math::powi(delta, term.d - 2));
        }
        for term in self.non_analytic {
            let (first_term, second_term) = term.density_derivatives(delta, tau);
            first += first_term;
            second += second_term;
        }
        for term in self.associating {
            let d = f64::from(term.d);
            let dd = delta - term.epsilon;
            let tt = tau - term.gamma;
            let e = math::exp(-term.eta * dd * dd + 1.0 / (term.beta * tt * tt + term.b));
            let tau_t = math::powf(tau, term.t);
            let delta_d = math::powi(delta, term.d);
            first += term.n * delta_d * tau_t * e * (d / delta - 2.0 * term.eta * dd);
            second += term.n
                * tau_t
                * e
                * (-2.0 * term.eta * delta_d + 4.0 * term.eta * term.eta * delta_d * dd * dd
                    - 4.0 * d * term.eta * math::powi(delta, term.d - 1) * dd
                    + d * (d - 1.0) * math::powi(delta, term.d - 2));
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
        let half_inverse_beta = 1.0 / (2.0 * self.beta);
        let theta = (1.0 - tau) + self.big_a * math::powf(dm2, half_inverse_beta);
        let distance = theta * theta + self.big_b * math::powf(dm2, self.a);
        let psi = math::exp(-self.big_c * dm2 - self.big_d * tm * tm);
        let psi_d = -2.0 * self.big_c * dm * psi;
        let psi_dd = (2.0 * self.big_c * dm2 - 1.0) * 2.0 * self.big_c * psi;
        let (distance_d, distance_dd) = if dm2 > 0.0 {
            let d1 = dm
                * (self.big_a * theta * 2.0 / self.beta * math::powf(dm2, half_inverse_beta - 1.0)
                    + 2.0 * self.big_b * self.a * math::powf(dm2, self.a - 1.0));
            let inner = math::powf(dm2, half_inverse_beta - 1.0);
            let d2 = d1 / dm
                + dm2
                    * (4.0 * self.big_b * self.a * (self.a - 1.0) * math::powf(dm2, self.a - 2.0)
                        + 2.0 * self.big_a * self.big_a / (self.beta * self.beta) * inner * inner
                        + self.big_a * theta * 4.0 / self.beta
                            * (half_inverse_beta - 1.0)
                            * math::powf(dm2, half_inverse_beta - 2.0));
            (d1, d2)
        } else {
            (0.0, 0.0)
        };
        let power = math::powf(distance, self.b);
        let power_d = self.b * math::powf(distance, self.b - 1.0) * distance_d;
        let power_dd = self.b
            * (math::powf(distance, self.b - 1.0) * distance_dd
                + (self.b - 1.0) * math::powf(distance, self.b - 2.0) * distance_d * distance_d);
        let first = self.n * (power * (psi + delta * psi_d) + power_d * delta * psi);
        let second = self.n
            * (power * (2.0 * psi_d + delta * psi_dd)
                + 2.0 * power_d * (psi + delta * psi_d)
                + power_dd * delta * psi);
        (first, second)
    }
}
