//! The phase diagram: each condensable's saturation curve over its solid and its liquid, its
//! melting curve, and the densities and latent heats of its phases (plan 14, P14.T49.b's second
//! step; decision-composition §1.1 and §5; science-p14-t24f §6.3).
//!
//! A condensable row carries a [`PhaseDiagram`], built from its published reference equations and
//! evaluated verbatim:
//!
//! - **the liquid branch**, from the triple point to the critical point, is the reference equation
//!   of state's ancillary vapour-pressure equation, ln(p ÷ `p_c`) = (`T_c` ÷ T) Σ aᵢ θ^tᵢ with
//!   θ = 1 − T ÷ `T_c` ([`WagnerEquation`]): never a Clausius–Clapeyron form, whose constant or
//!   temperature-dependent enthalpy runs 10–50% off near the critical point (science-p14-t24f
//!   §6.3);
//! - **the solid branch**, below the triple point, is a published sublimation equation for each
//!   solid phase ([`SublimationPiece`]), its constant term moved by an anchor so that each meets
//!   the branch above it: the first meets the liquid branch at the triple temperature, and each
//!   colder one the warmer at the transition between the two solids. The anchors are the fitted
//!   table `tables::saturation`, which `hyperion-fit`'s `saturation_curves` computes from these
//!   equations (they need the transcendental functions, which no `const` can call). Below the
//!   coldest equation's range the branch continues at constant enthalpy, the tangent of its
//!   Clausius–Clapeyron line at the range's end;
//! - **the melting curve** ([`MeltingCurve`]) is the reference's melting equation where it has
//!   one, so that a liquid under pressure freezes and water's ice melts under pressure. Without
//!   one the melting line stands at the triple temperature, as the held columns
//!   ([`PhaseColumns`](super::PhaseColumns)) take every melting line;
//! - **the densities**: the saturated liquid's `ρ_l`(T) is the same reference's ancillary density
//!   equation ([`DensityEquation`]), and each solid phase's `ρ_s`(T) ([`SolidPhase`]) its measured
//!   density, as a fit of its lattice or bulk density in temperature where one was read, or one
//!   cell at a stated temperature, and not modelled where none was read;
//! - **the latent heats** at the triple point: the reference's h″ − h′ for vaporisation, and the
//!   enthalpy of fusion where a measurement was read; and the curve's own Clausius–Clapeyron
//!   enthalpy R T² d ln p ÷ dT at any temperature ([`PhaseDiagram::clausius_clapeyron_enthalpy_j_per_mol`]),
//!   which a reader integrating Clausius–Clapeyron takes so that it reproduces the curve.
//!
//! Generator version 22 publishes none of it: the record still reads the held columns, and
//! P14.T54.a points its callers at this module's [`saturation_pressure`], [`phase_at`] and
//! [`condensables`], which carry the held functions' names, at the 22 → 23 bump
//! (decision-bump-plan-after-22 §1.2).
//!
//! The arithmetic is the crate's deterministic `math`, in fixed term order, and every inversion a
//! fixed number of bisections, so that a value is the same bits on every target (sim-determinism).

use super::phase::{MOLAR_GAS_CONSTANT, Phase, ice_ih_density};
use super::{Source, SubstanceId, ids};
use crate::math;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// The Avogadro constant, mol⁻¹ (exact in the 2019 SI).
pub(super) const AVOGADRO_PER_MOL: f64 = 6.022_140_76e23;

/// Bisections of [`PhaseDiagram::saturation_temperature`]: 64 halve a bracket of a thousand
/// kelvin to under 10⁻¹⁶ K, past the last bit of a temperature.
const BISECTIONS: u32 = 64;

/// A Wagner-type equation, ln(p ÷ `p_r`) = (`T_r` ÷ T) Σ aᵢ θ^tᵢ with θ = 1 − T ÷ `T_r`: the reference
/// equations' ancillary vapour-pressure equations (`p_r` and `T_r` their critical point), and the
/// sublimation equations of Span and Wagner 1996 and Tegeler et al. 1999 (`p_r` and `T_r` their triple
/// point).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WagnerEquation {
    pub(super) reducing_temperature: Kelvin,
    pub(super) reducing_pressure: Pascals,
    pub(super) terms: &'static [(f64, f64)],
}

impl WagnerEquation {
    /// The reducing temperature `T_r`.
    #[must_use]
    pub const fn reducing_temperature(&self) -> Kelvin {
        self.reducing_temperature
    }

    /// The reducing pressure `p_r`.
    #[must_use]
    pub const fn reducing_pressure(&self) -> Pascals {
        self.reducing_pressure
    }

    /// The terms (aᵢ, tᵢ), in the published order.
    #[must_use]
    pub const fn terms(&self) -> &'static [(f64, f64)] {
        self.terms
    }

    /// ln(p ÷ 1 Pa) at `temperature`, for 0 < T ≤ `T_r`; outside that it is not defined (`NaN` or
    /// infinite).
    #[must_use]
    pub fn ln_pressure(&self, temperature: Kelvin) -> f64 {
        let reducing = self.reducing_temperature.value();
        let t = temperature.value();
        let theta = 1.0 - t / reducing;
        let mut sum = 0.0;
        for &(a, exponent) in self.terms {
            sum += a * math::powf(theta, exponent);
        }
        math::ln(self.reducing_pressure.value()) + reducing / t * sum
    }

    /// d ln p ÷ d(1 ÷ T) at `temperature`, kelvin: `T_r` Σ aᵢ θ^tᵢ + T Σ aᵢ tᵢ θ^(tᵢ − 1), for
    /// 0 < T < `T_r`.
    #[must_use]
    pub fn slope_k(&self, temperature: Kelvin) -> f64 {
        let reducing = self.reducing_temperature.value();
        let t = temperature.value();
        let theta = 1.0 - t / reducing;
        let (mut sum, mut derivative) = (0.0, 0.0);
        for &(a, exponent) in self.terms {
            sum += a * math::powf(theta, exponent);
            derivative += a * exponent * math::powf(theta, exponent - 1.0);
        }
        reducing * sum + t * derivative
    }
}

/// A published sublimation equation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SublimationEquation {
    /// ln(p ÷ `unit`) = A₀ + Σᵢ Aᵢ ÷ Tⁱ, the coefficients A₀, A₁, … in order: Fray and Schmitt
    /// 2009's form, with p in bar, and Gao et al. 2023's ammonia, in megapascals.
    InversePowers {
        /// The pressure unit of the published equation.
        unit: Pascals,
        /// A₀, A₁, …, Aᵢ in K^i.
        coefficients: &'static [f64],
    },
    /// The Wagner form about the triple point ([`WagnerEquation`], `T_r` and `p_r` the triple point's).
    Reduced(WagnerEquation),
    /// ln(p ÷ `p_t`) = (`T_t` ÷ T) Σ aᵢ (T ÷ `T_t`)^bᵢ, the terms (aᵢ, bᵢ): IAPWS R14-08's ice Ih.
    ScaledPowers {
        /// The reducing temperature `T_t`.
        temperature: Kelvin,
        /// The reducing pressure `p_t`.
        pressure: Pascals,
        /// The terms (aᵢ, bᵢ).
        terms: &'static [(f64, f64)],
    },
}

impl SublimationEquation {
    /// ln(p ÷ 1 Pa) at `temperature` (positive), as published, before its anchor.
    #[must_use]
    pub fn ln_pressure(&self, temperature: Kelvin) -> f64 {
        let t = temperature.value();
        match *self {
            Self::InversePowers { unit, coefficients } => {
                let inverse = 1.0 / t;
                let mut sum = 0.0;
                for &a in coefficients.iter().rev() {
                    sum = sum * inverse + a;
                }
                math::ln(unit.value()) + sum
            }
            Self::Reduced(equation) => equation.ln_pressure(temperature),
            Self::ScaledPowers {
                temperature: reducing,
                pressure,
                terms,
            } => {
                let reducing = reducing.value();
                let ratio = t / reducing;
                let mut sum = 0.0;
                for &(a, exponent) in terms {
                    sum += a * math::powf(ratio, exponent);
                }
                math::ln(pressure.value()) + reducing / t * sum
            }
        }
    }

    /// d ln p ÷ d(1 ÷ T) at `temperature`, kelvin.
    #[must_use]
    pub fn slope_k(&self, temperature: Kelvin) -> f64 {
        let t = temperature.value();
        match *self {
            Self::InversePowers { coefficients, .. } => {
                let inverse = 1.0 / t;
                let mut slope = 0.0;
                // One past the highest power, counted down to each coefficient's.
                let mut power = coefficients.iter().fold(0.0, |count, _| count + 1.0);
                for &a in coefficients.iter().skip(1).rev() {
                    power -= 1.0;
                    slope = slope * inverse + power * a;
                }
                slope
            }
            Self::Reduced(equation) => equation.slope_k(temperature),
            Self::ScaledPowers {
                temperature: reducing,
                terms,
                ..
            } => {
                let ratio = t / reducing.value();
                let mut sum = 0.0;
                for &(a, exponent) in terms {
                    sum += a * (exponent - 1.0) * math::powf(ratio, exponent - 1.0);
                }
                -t * sum
            }
        }
    }
}

/// One solid phase's sublimation equation, from its lower bound `lower` up to the bound of the
/// piece above it (the triple temperature for the first).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SublimationPiece {
    pub(super) phase: &'static str,
    pub(super) lower: Kelvin,
    pub(super) equation: SublimationEquation,
}

impl SublimationPiece {
    /// The solid phase it is published for (`ice Ih`, `β`, …).
    #[must_use]
    pub const fn phase(&self) -> &'static str {
        self.phase
    }

    /// The temperature it holds down to: its range's end, or the transition to the phase below.
    #[must_use]
    pub const fn lower(&self) -> Kelvin {
        self.lower
    }

    /// Its equation, as published.
    #[must_use]
    pub const fn equation(&self) -> &SublimationEquation {
        &self.equation
    }
}

/// How an ancillary density equation's sum Σ bᵢ θ^tᵢ gives ρ′ ÷ `ρ_r`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DensityForm {
    /// ρ′ ÷ `ρ_r` = 1 + Σ bᵢ θ^tᵢ.
    Linear,
    /// ln(ρ′ ÷ `ρ_r`) = Σ bᵢ θ^tᵢ.
    Logarithmic,
}

/// A saturated-liquid density ancillary equation, in θ = 1 − T ÷ `T_r`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DensityEquation {
    pub(super) form: DensityForm,
    pub(super) reducing_temperature: Kelvin,
    pub(super) reducing_density: KilogramsPerCubicMetre,
    pub(super) terms: &'static [(f64, f64)],
}

impl DensityEquation {
    /// The saturated liquid's density at `temperature`, for 0 < T ≤ `T_r`.
    #[must_use]
    pub fn density(&self, temperature: Kelvin) -> KilogramsPerCubicMetre {
        let theta = 1.0 - temperature.value() / self.reducing_temperature.value();
        let mut sum = 0.0;
        for &(b, exponent) in self.terms {
            sum += b * math::powf(theta, exponent);
        }
        let ratio = match self.form {
            DensityForm::Linear => 1.0 + sum,
            DensityForm::Logarithmic => math::exp(sum),
        };
        self.reducing_density * ratio
    }
}

/// A published melting equation, `p_m`(T).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeltingEquation {
    /// p ÷ `p_r` = 1 + Σ aᵢ ((T ÷ `T_r`)^tᵢ − 1): Simon's form, as Setzmann and Wagner 1991, Span et
    /// al. 2000 and Tegeler et al. 1999 write it.
    Simon {
        /// The reducing temperature `T_r`.
        temperature: Kelvin,
        /// The reducing pressure `p_r`.
        pressure: Pascals,
        /// The terms (aᵢ, tᵢ).
        terms: &'static [(f64, f64)],
    },
    /// p = `p_r` + Σ Aᵢ ((T ÷ `T_r`)^tᵢ − 1), Aᵢ in pascals: the Simon–Glatzel form of Gao et al.
    /// 2023's ammonia.
    SimonGlatzel {
        /// The reducing temperature `T_r`.
        temperature: Kelvin,
        /// The reducing pressure `p_r`.
        pressure: Pascals,
        /// The terms (Aᵢ in pascals, tᵢ).
        terms: &'static [(f64, f64)],
    },
    /// p ÷ `p_r` = 1 + Σ aᵢ (T ÷ `T_r` − 1)^tᵢ: Span and Wagner 1996's carbon dioxide and Bücker and
    /// Wagner 2006's ethane.
    Polynomial {
        /// The reducing temperature `T_r`.
        temperature: Kelvin,
        /// The reducing pressure `p_r`.
        pressure: Pascals,
        /// The terms (aᵢ, tᵢ).
        terms: &'static [(f64, f64)],
    },
    /// p ÷ `p_t` = 1 + Σ aᵢ (1 − (T ÷ `T_t`)^bᵢ): IAPWS R14-08's ice Ih, which falls from the triple
    /// point.
    IceIh {
        /// The triple temperature.
        temperature: Kelvin,
        /// The triple pressure.
        pressure: Pascals,
        /// The terms (aᵢ, bᵢ).
        terms: &'static [(f64, f64)],
    },
    /// ln(p ÷ `p_r`) = Σ aᵢ (1 − (T ÷ `T_r`)^bᵢ): IAPWS R14-08's ice VII.
    Logarithmic {
        /// The reducing temperature `T_r`.
        temperature: Kelvin,
        /// The reducing pressure `p_r`.
        pressure: Pascals,
        /// The terms (aᵢ, bᵢ).
        terms: &'static [(f64, f64)],
    },
}

impl MeltingEquation {
    /// The melting pressure at `temperature`.
    ///
    /// Each form holds over its own piece of the curve: `Simon`, `SimonGlatzel`, `Polynomial` and
    /// `Logarithmic` from their reducing temperature up, and `IceIh` from its lowest temperature
    /// up to the triple point. `Polynomial` is `NaN` below its reducing temperature, where its
    /// fractional powers of a negative number are undefined; the others there give the curve's
    /// analytic continuation, which [`PhaseDiagram`] never reads.
    #[must_use]
    pub fn pressure(&self, temperature: Kelvin) -> Pascals {
        let t = temperature.value();
        match *self {
            Self::Simon {
                temperature: reducing,
                pressure,
                terms,
            } => {
                let ratio = t / reducing.value();
                let mut sum = 0.0;
                for &(a, exponent) in terms {
                    sum += a * (math::powf(ratio, exponent) - 1.0);
                }
                pressure * (1.0 + sum)
            }
            Self::SimonGlatzel {
                temperature: reducing,
                pressure,
                terms,
            } => {
                let ratio = t / reducing.value();
                let mut sum = pressure.value();
                for &(a, exponent) in terms {
                    sum += a * (math::powf(ratio, exponent) - 1.0);
                }
                Pascals::new(sum)
            }
            Self::Polynomial {
                temperature: reducing,
                pressure,
                terms,
            } => {
                let excess = t / reducing.value() - 1.0;
                let mut sum = 0.0;
                for &(a, exponent) in terms {
                    sum += a * math::powf(excess, exponent);
                }
                pressure * (1.0 + sum)
            }
            Self::IceIh {
                temperature: reducing,
                pressure,
                terms,
            } => {
                let ratio = t / reducing.value();
                let mut sum = 0.0;
                for &(a, exponent) in terms {
                    sum += a * (1.0 - math::powf(ratio, exponent));
                }
                pressure * (1.0 + sum)
            }
            Self::Logarithmic {
                temperature: reducing,
                pressure,
                terms,
            } => {
                let ratio = t / reducing.value();
                let mut sum = 0.0;
                for &(a, exponent) in terms {
                    sum += a * (1.0 - math::powf(ratio, exponent));
                }
                pressure * math::exp(sum)
            }
        }
    }
}

/// One stretch of a melting curve, from `start` up to the next piece's start, or to `end` for the
/// last piece of a curve whose equation must not be extrapolated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeltingPiece {
    pub(super) start: Kelvin,
    pub(super) end: Option<Kelvin>,
    pub(super) equation: MeltingEquation,
}

impl MeltingPiece {
    /// The temperature it starts at.
    #[must_use]
    pub const fn start(&self) -> Kelvin {
        self.start
    }

    /// The temperature beyond which it is not extrapolated, where the modelled curve ends (water's
    /// ice VII at 715 K, the highest measurement, beyond which IAPWS R14-08's Eq. 5 turns over);
    /// `None` for an equation extrapolated without end.
    #[must_use]
    pub const fn end(&self) -> Option<Kelvin> {
        self.end
    }

    /// Its equation.
    #[must_use]
    pub const fn equation(&self) -> &MeltingEquation {
        &self.equation
    }
}

/// Where the solid and the liquid meet under pressure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeltingCurve {
    /// Rising from the triple point, as for nearly every substance.
    ///
    /// The pieces are in order of temperature, the first from the triple temperature, the last
    /// extrapolated beyond its published range. A fluid at or above the curve's pressure is solid.
    Rising(&'static [MeltingPiece]),
    /// Falling from the triple point to `lowest`, as water's ice Ih does, then rising from there.
    ///
    /// The rising branch runs through the solids denser than the liquid, as water's ices III, V,
    /// VI and VII. On the falling branch the liquid is the denser phase, so pressure melts the
    /// solid; on the rising one a fluid at or above its pressure is solid again. Below `lowest`
    /// (water's triple point of ice Ih, ice III and the liquid) no liquid exists at any pressure.
    Falling {
        /// The lowest temperature of the falling branch, where the rising one starts.
        lowest: Kelvin,
        /// The falling branch's equation.
        equation: MeltingEquation,
        /// The rising branch's pieces in order of temperature, the first from `lowest`, the last
        /// extrapolated beyond its published range.
        rising: &'static [MeltingPiece],
    },
}

/// How a solid phase's density is given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SolidDensity {
    /// Ice Ih at one atmosphere, IAPWS R10-06(2009)'s Gibbs function as [`ice_ih_density`].
    IceIh,
    /// A polynomial in temperature, ρ = Σ cᵢ Tⁱ kg m⁻³, held at its value at `held_below` below it
    /// and extrapolated above its published range.
    Polynomial {
        /// c₀, c₁, …, in kg m⁻³ K⁻ⁱ.
        coefficients: &'static [f64],
        /// The fit's lowest temperature, below which its value there holds.
        held_below: Kelvin,
    },
    /// A fit of the unit cell's volume, V = Σ cᵢ Tⁱ cubic ångströms, holding `formula_units`
    /// molecules: ρ = Z M ÷ (`N_A` V).
    CellVolume {
        /// Z, the formula units in the cell.
        formula_units: f64,
        /// M, g mol⁻¹: the row's molar mass.
        molar_mass_g_per_mol: f64,
        /// c₀, c₁, …, in Å³ K⁻ⁱ.
        volume_cubic_angstrom: &'static [f64],
    },
    /// One measured density, at a stated temperature where the source gives one.
    Fixed {
        /// The density.
        density: KilogramsPerCubicMetre,
        /// The temperature it was measured at, if stated.
        at: Option<Kelvin>,
    },
    /// Measured densities at rising temperatures, interpolated linearly and held beyond the first
    /// and the last.
    Measured(&'static [(Kelvin, KilogramsPerCubicMetre)]),
}

impl SolidDensity {
    /// The density at `temperature`.
    #[must_use]
    pub fn at(&self, temperature: Kelvin) -> KilogramsPerCubicMetre {
        let t = temperature.value();
        match *self {
            Self::IceIh => ice_ih_density(temperature),
            Self::Polynomial {
                coefficients,
                held_below,
            } => {
                let t = t.max(held_below.value());
                let mut sum = 0.0;
                for &c in coefficients.iter().rev() {
                    sum = sum * t + c;
                }
                KilogramsPerCubicMetre::new(sum)
            }
            Self::CellVolume {
                formula_units,
                molar_mass_g_per_mol,
                volume_cubic_angstrom,
            } => {
                let mut volume = 0.0;
                for &c in volume_cubic_angstrom.iter().rev() {
                    volume = volume * t + c;
                }
                // g mol⁻¹ to kg mol⁻¹ is 10⁻³, and Å³ to m³ 10⁻³⁰.
                KilogramsPerCubicMetre::new(
                    formula_units * molar_mass_g_per_mol * 1e27 / (AVOGADRO_PER_MOL * volume),
                )
            }
            Self::Fixed { density, .. } => density,
            Self::Measured(points) => {
                let Some((&(t0, rho0), rest)) = points.split_first() else {
                    return KilogramsPerCubicMetre::new(f64::NAN);
                };
                let (mut low_t, mut low_rho) = (t0.value(), rho0.value());
                if t <= low_t {
                    return KilogramsPerCubicMetre::new(low_rho);
                }
                for &(high, high_density) in rest {
                    let (high_t, high_rho) = (high.value(), high_density.value());
                    if t <= high_t {
                        let f = (t - low_t) / (high_t - low_t);
                        return KilogramsPerCubicMetre::new(low_rho + f * (high_rho - low_rho));
                    }
                    (low_t, low_rho) = (high_t, high_rho);
                }
                KilogramsPerCubicMetre::new(low_rho)
            }
        }
    }
}

/// A solid phase, from `start` up to the next phase's start (the triple temperature for the
/// warmest), with its density where one was read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidPhase {
    pub(super) name: &'static str,
    pub(super) start: Kelvin,
    pub(super) density: Option<SolidDensity>,
}

impl SolidPhase {
    /// The phase's name (`ice Ih`, `α`, …).
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The temperature it starts at.
    #[must_use]
    pub const fn start(&self) -> Kelvin {
        self.start
    }

    /// How its density is given, or `None` where no density was read.
    #[must_use]
    pub const fn density(&self) -> Option<&SolidDensity> {
        self.density.as_ref()
    }
}

/// A condensable substance's phase diagram, built from its published reference equations (the
/// [module documentation](self)), each value named with its sources in
/// [`sources`](Self::sources).
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseDiagram {
    pub(super) triple_temperature: Kelvin,
    pub(super) triple_pressure: Pascals,
    pub(super) critical_temperature: Kelvin,
    pub(super) critical_pressure: Pascals,
    pub(super) vapour_pressure: WagnerEquation,
    pub(super) liquid_density: DensityEquation,
    pub(super) sublimation: &'static [SublimationPiece],
    pub(super) anchors: &'static [f64],
    pub(super) melting: Option<MeltingCurve>,
    pub(super) solids: &'static [SolidPhase],
    pub(super) triple_vaporisation_enthalpy_j_per_mol: f64,
    pub(super) triple_fusion_enthalpy_j_per_mol: Option<f64>,
    pub(super) sources: &'static [Source],
}

impl PhaseDiagram {
    /// The triple point's temperature, the reference's.
    #[must_use]
    pub const fn triple_temperature(&self) -> Kelvin {
        self.triple_temperature
    }

    /// The triple point's pressure, the reference's: its selected value, which its vapour-pressure
    /// equation meets within that equation's error.
    #[must_use]
    pub const fn triple_pressure(&self) -> Pascals {
        self.triple_pressure
    }

    /// The critical temperature, above which the substance never condenses to a liquid.
    #[must_use]
    pub const fn critical_temperature(&self) -> Kelvin {
        self.critical_temperature
    }

    /// The critical pressure, above which, at or above the critical temperature, the fluid is
    /// supercritical.
    #[must_use]
    pub const fn critical_pressure(&self) -> Pascals {
        self.critical_pressure
    }

    /// The liquid branch's equation: the reference's ancillary vapour-pressure equation.
    #[must_use]
    pub const fn vapour_pressure(&self) -> &WagnerEquation {
        &self.vapour_pressure
    }

    /// The saturated liquid's density equation: the reference's ancillary.
    #[must_use]
    pub const fn liquid_density_equation(&self) -> &DensityEquation {
        &self.liquid_density
    }

    /// The solid branch's equations, from the triple point down.
    #[must_use]
    pub const fn sublimation(&self) -> &'static [SublimationPiece] {
        self.sublimation
    }

    /// Each sublimation piece's anchor, the amount added to its ln p (pressure in pascals) so that
    /// it meets the branch above it: `tables::saturation`'s, fitted by `hyperion-fit`.
    #[must_use]
    pub const fn anchors(&self) -> &'static [f64] {
        self.anchors
    }

    /// The melting curve, or `None` where the reference gives none and the melting line stands at
    /// the triple temperature.
    #[must_use]
    pub const fn melting(&self) -> Option<&MeltingCurve> {
        self.melting.as_ref()
    }

    /// The solid phases from 0 K up, each with its density where one was read.
    #[must_use]
    pub const fn solid_phases(&self) -> &'static [SolidPhase] {
        self.solids
    }

    /// The enthalpy of vaporisation at the triple point, h″ − h′ of the reference, J mol⁻¹.
    #[must_use]
    pub const fn triple_vaporisation_enthalpy_j_per_mol(&self) -> f64 {
        self.triple_vaporisation_enthalpy_j_per_mol
    }

    /// The enthalpy of fusion at the triple point, J mol⁻¹, where a measurement was read.
    #[must_use]
    pub const fn triple_fusion_enthalpy_j_per_mol(&self) -> Option<f64> {
        self.triple_fusion_enthalpy_j_per_mol
    }

    /// The enthalpy of sublimation at the triple point, vaporisation's plus fusion's, J mol⁻¹, where
    /// both are known.
    #[must_use]
    pub fn triple_sublimation_enthalpy_j_per_mol(&self) -> Option<f64> {
        self.triple_fusion_enthalpy_j_per_mol
            .map(|fusion| self.triple_vaporisation_enthalpy_j_per_mol + fusion)
    }

    /// Where each value comes from.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }

    /// The solid branch's ln(p ÷ 1 Pa) at `temperature` (positive): the piece whose range holds
    /// it, with its anchor, or below the coldest piece the tangent of its Clausius–Clapeyron line
    /// at that piece's lower bound.
    #[must_use]
    fn solid_ln_pressure(&self, temperature: Kelvin) -> f64 {
        for (piece, anchor) in self.sublimation.iter().zip(self.anchors) {
            if temperature >= piece.lower {
                return piece.equation.ln_pressure(temperature) + anchor;
            }
        }
        let (Some(piece), Some(anchor)) = (self.sublimation.last(), self.anchors.last()) else {
            return f64::NAN;
        };
        let lower = piece.lower;
        piece.equation.ln_pressure(lower)
            + anchor
            + piece.equation.slope_k(lower) * (1.0 / temperature.value() - 1.0 / lower.value())
    }

    /// d ln p ÷ d(1 ÷ T) of the solid branch at `temperature` (positive), kelvin.
    #[must_use]
    fn solid_slope_k(&self, temperature: Kelvin) -> f64 {
        for piece in self.sublimation {
            if temperature >= piece.lower {
                return piece.equation.slope_k(temperature);
            }
        }
        self.sublimation
            .last()
            .map_or(f64::NAN, |piece| piece.equation.slope_k(piece.lower))
    }

    /// ln(`p_sat` ÷ 1 Pa) at `temperature`: the solid branch below the triple temperature and the
    /// liquid branch from it.
    ///
    /// `None` at or above the critical temperature, where the substance never condenses, at or
    /// below 0 K, and for a temperature that is not a number.
    #[must_use]
    pub fn ln_saturation_pressure(&self, temperature: Kelvin) -> Option<f64> {
        let t = temperature.value();
        if t.is_nan() || t <= 0.0 || temperature >= self.critical_temperature {
            return None;
        }
        Some(if temperature < self.triple_temperature {
            self.solid_ln_pressure(temperature)
        } else {
            self.vapour_pressure.ln_pressure(temperature)
        })
    }

    /// The saturation vapour pressure at `temperature`, over the solid below the triple point and
    /// the liquid above.
    ///
    /// Infinite at and above the critical temperature, where the substance never condenses, and
    /// zero at or below 0 K and for a temperature that is not a number, as the held
    /// [`PhaseColumns::saturation_pressure`](super::PhaseColumns::saturation_pressure) answers.
    #[must_use]
    pub fn saturation_pressure(&self, temperature: Kelvin) -> Pascals {
        if temperature >= self.critical_temperature {
            return Pascals::new(f64::INFINITY);
        }
        self.ln_saturation_pressure(temperature)
            .map_or(Pascals::ZERO, |ln_p| Pascals::new(math::exp(ln_p)))
    }

    /// The curve's own Clausius–Clapeyron enthalpy at `temperature`, R T² d ln `p_sat` ÷ dT,
    /// J mol⁻¹: of sublimation below the triple point and of vaporisation from it.
    ///
    /// It is the enthalpy with which an ideal-gas Clausius–Clapeyron step follows the curve, and exceeds
    /// the true latent heat by the factor 1 ÷ (z″ − z′) of the vapour's non-ideality: a few per
    /// cent at most rows' triple points (10% at carbon dioxide's, 5.2 bar), growing without bound
    /// toward the critical point. `None` as
    /// [`ln_saturation_pressure`](Self::ln_saturation_pressure) is.
    #[must_use]
    pub fn clausius_clapeyron_enthalpy_j_per_mol(&self, temperature: Kelvin) -> Option<f64> {
        self.ln_saturation_pressure(temperature)?;
        let slope = if temperature < self.triple_temperature {
            self.solid_slope_k(temperature)
        } else {
            self.vapour_pressure.slope_k(temperature)
        };
        Some(-MOLAR_GAS_CONSTANT * slope)
    }

    /// The temperature at which the saturation pressure is `pressure`: the frost point below the
    /// triple pressure and the boiling point above it.
    ///
    /// `None` at or above the liquid branch's pressure at the critical temperature, where the
    /// fluid no longer boils, and for a pressure that is not positive or not a number. Below the
    /// coldest sublimation piece the branch is a straight line in 1 ÷ T, solved exactly; above
    /// it, 64 bisections of the curve, which rises monotonically.
    ///
    /// # Examples
    ///
    /// Methane boils at 111.668 K under one atmosphere (Setzmann and Wagner 1991, p. 1064), and
    /// its frost point under a pascal lies far below its triple point, 90.6941 K:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::Pascals;
    ///
    /// # fn main() {
    /// #     assert!(example().is_some(), "every value the example reads is there");
    /// # }
    /// # fn example() -> Option<()> {
    /// let methane = by_key("CH4")?.substance().phase_diagram()?;
    /// let boiling = methane.saturation_temperature(Pascals::new(101_325.0))?;
    /// assert!((boiling.value() - 111.668).abs() < 0.01);
    /// let frost = methane.saturation_temperature(Pascals::new(1.0))?;
    /// assert!(frost.value() < 60.0);
    /// // Above the critical point the fluid no longer boils.
    /// assert_eq!(methane.saturation_temperature(Pascals::new(1e7)), None);
    /// # Some(())
    /// # }
    /// ```
    #[must_use]
    pub fn saturation_temperature(&self, pressure: Pascals) -> Option<Kelvin> {
        let p = pressure.value();
        if p.is_nan() || p <= 0.0 {
            return None;
        }
        let target = math::ln(p);
        let critical = self.critical_temperature;
        if target >= self.vapour_pressure.ln_pressure(critical) {
            return None;
        }
        let coldest = self.sublimation.last()?;
        let lowest = coldest.lower;
        let at_lowest = self.solid_ln_pressure(lowest);
        if target < at_lowest {
            let slope = coldest.equation.slope_k(lowest);
            return Some(Kelvin::new(
                1.0 / (1.0 / lowest.value() + (target - at_lowest) / slope),
            ));
        }
        let (mut cold, mut warm) = (lowest.value(), critical.value());
        for _ in 0..BISECTIONS {
            let middle = f64::midpoint(cold, warm);
            let ln_p = if middle < self.triple_temperature.value() {
                self.solid_ln_pressure(Kelvin::new(middle))
            } else {
                self.vapour_pressure.ln_pressure(Kelvin::new(middle))
            };
            if ln_p < target {
                cold = middle;
            } else {
                warm = middle;
            }
        }
        Some(Kelvin::new(f64::midpoint(cold, warm)))
    }

    /// The saturated liquid's density at `temperature`, from the triple to the critical
    /// temperature; `None` outside them.
    #[must_use]
    pub fn liquid_density(&self, temperature: Kelvin) -> Option<KilogramsPerCubicMetre> {
        (temperature >= self.triple_temperature && temperature <= self.critical_temperature)
            .then(|| self.liquid_density.density(temperature))
    }

    /// The saturated liquid's density at the triple point: what a body's liquid inventory occupies
    /// where the held columns' [`triple_liquid_density`](super::PhaseColumns::triple_liquid_density)
    /// is read now.
    #[must_use]
    pub fn triple_liquid_density(&self) -> KilogramsPerCubicMetre {
        self.liquid_density.density(self.triple_temperature)
    }

    /// The solid phase at `temperature`, from 0 K to the triple temperature; `None` outside them
    /// or for a temperature that is not a number.
    #[must_use]
    pub fn solid_phase_at(&self, temperature: Kelvin) -> Option<&'static SolidPhase> {
        let t = temperature.value();
        if t.is_nan() || t < 0.0 || temperature > self.triple_temperature {
            return None;
        }
        self.solids
            .iter()
            .rev()
            .find(|phase| temperature >= phase.start)
    }

    /// The solid's density at `temperature` (at one atmosphere or its vapour's pressure, which
    /// differ by less than the measurements' errors), from 0 K to the triple temperature; `None`
    /// outside them and in a phase whose density was not read.
    #[must_use]
    pub fn solid_density(&self, temperature: Kelvin) -> Option<KilogramsPerCubicMetre> {
        self.solid_phase_at(temperature)?
            .density
            .map(|density| density.at(temperature))
    }

    /// The melting pressure at `temperature` on the curve's rising branch, at or above which a
    /// fluid is solid.
    ///
    /// The branch starts at the triple temperature on a rising curve, and at water's lowest liquid
    /// temperature, through its ices III, V, VI and VII; `None` below it, beyond a curve's end
    /// (water's above 715 K) and where the reference gives no melting curve.
    ///
    /// # Examples
    ///
    /// Water's two branches at 260 K (IAPWS R14-08, Table 3): pressure melts ice Ih above 138.3
    /// megapascals, and the liquid freezes again to ice V above 402.6 megapascals:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::Kelvin;
    ///
    /// # fn main() {
    /// #     assert!(example().is_some(), "every value the example reads is there");
    /// # }
    /// # fn example() -> Option<()> {
    /// let water = by_key("H2O")?.substance().phase_diagram()?;
    /// let t = Kelvin::new(260.0);
    /// let melts = water.falling_melting_pressure(t)?.value();
    /// let freezes = water.melting_pressure(t)?.value();
    /// assert!((melts / 138.268e6 - 1.0).abs() < 1e-5);
    /// assert!((freezes / 402.58e6 - 1.0).abs() < 1e-4);
    /// # Some(())
    /// # }
    /// ```
    #[must_use]
    pub fn melting_pressure(&self, temperature: Kelvin) -> Option<Pascals> {
        let pieces = match self.melting? {
            MeltingCurve::Rising(pieces) | MeltingCurve::Falling { rising: pieces, .. } => pieces,
        };
        pieces
            .iter()
            .rev()
            .find(|piece| temperature >= piece.start)
            .filter(|piece| piece.end.is_none_or(|end| temperature <= end))
            .map(|piece| piece.equation.pressure(temperature))
    }

    /// The melting pressure at `temperature` on a falling branch (water's ice Ih), at or above
    /// which pressure melts the solid.
    ///
    /// The branch runs from its lowest temperature to the triple temperature; `None` elsewhere and
    /// for a curve with no falling branch ([`melting_pressure`](Self::melting_pressure) shows
    /// both).
    #[must_use]
    pub fn falling_melting_pressure(&self, temperature: Kelvin) -> Option<Pascals> {
        match self.melting? {
            MeltingCurve::Falling {
                lowest, equation, ..
            } => (temperature >= lowest && temperature <= self.triple_temperature)
                .then(|| equation.pressure(temperature)),
            MeltingCurve::Rising(_) => None,
        }
    }

    /// The phase at `temperature` and `pressure`.
    ///
    /// `None` for a temperature or pressure that is negative or not a number. Otherwise:
    ///
    /// - At or above the melting curve's rising branch: solid, above the critical temperature
    ///   too.
    /// - At or above the critical temperature otherwise: supercritical at or above the critical
    ///   pressure, and vapour below it.
    /// - Below the saturation pressure: vapour. On the curve itself the substance is condensed.
    /// - Otherwise liquid from the triple temperature, and below it solid, but liquid at or above
    ///   a falling branch's pressure (water's ice Ih melting under pressure), down to its lowest
    ///   temperature. Where the reference has no melting curve the melting line stands at the
    ///   triple temperature.
    ///
    /// Each rising branch is extrapolated beyond its published range but water's, which ends at
    /// ice VII's 715 K: above it water reads fluid at any pressure. A solid–solid transition under
    /// pressure that the melting curve does not mark (each solid's own high-pressure phases but
    /// water's) is not modelled.
    #[must_use]
    pub fn phase_at(&self, temperature: Kelvin, pressure: Pascals) -> Option<Phase> {
        let (t, p) = (temperature.value(), pressure.value());
        if t.is_nan() || p.is_nan() || t < 0.0 || p < 0.0 {
            return None;
        }
        if let Some(melting) = self.melting_pressure(temperature)
            && pressure >= melting
        {
            return Some(Phase::Solid);
        }
        if temperature >= self.critical_temperature {
            return Some(if pressure >= self.critical_pressure {
                Phase::Supercritical
            } else {
                Phase::Vapour
            });
        }
        if pressure < self.saturation_pressure(temperature) {
            return Some(Phase::Vapour);
        }
        if temperature >= self.triple_temperature {
            return Some(Phase::Liquid);
        }
        if let Some(melting) = self.falling_melting_pressure(temperature)
            && pressure >= melting
        {
            return Some(Phase::Liquid);
        }
        Some(Phase::Solid)
    }
}

/// The saturation vapour pressure of the substance `id` at `temperature` on its phase diagram
/// ([`PhaseDiagram::saturation_pressure`]), or `None` for a row with none.
///
/// P14.T54.a re-exports it at `substance::` in place of the held
/// [`substance::saturation_pressure`](super::saturation_pressure).
///
/// # Examples
///
/// Methane boils at 111.668 K under one atmosphere (Setzmann and Wagner 1991, p. 1064), and
/// carbon dioxide at 600 Pa freezes out below about 148 K:
///
/// ```
/// use hyperion_sim::substance::{by_key, diagram};
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let methane = diagram::saturation_pressure(by_key("CH4")?, Kelvin::new(111.668))?;
/// assert!((methane.value() / 101_325.0 - 1.0).abs() < 1e-4);
/// let frost = diagram::saturation_pressure(by_key("CO2")?, Kelvin::new(148.0))?;
/// assert!((600.0..700.0).contains(&frost.value()));
/// // Helium holds no phase diagram.
/// assert_eq!(diagram::saturation_pressure(by_key("He")?, Kelvin::new(3.0)), None);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn saturation_pressure(id: SubstanceId, temperature: Kelvin) -> Option<Pascals> {
    id.substance()
        .phase_diagram()
        .map(|diagram| diagram.saturation_pressure(temperature))
}

/// The phase of the substance `id` at `temperature` and `pressure` on its phase diagram
/// ([`PhaseDiagram::phase_at`]).
///
/// `None` for a row with none, or a temperature or pressure that is negative or not a number.
/// P14.T54.a re-exports it at `substance::` in place of the held
/// [`substance::phase_at`](super::phase_at).
///
/// # Examples
///
/// Water under 500 bar stays liquid at 270 K, where ice Ih melts under pressure:
///
/// ```
/// use hyperion_sim::substance::{Phase, by_key, diagram};
/// use hyperion_sim::units::{Kelvin, Pascals};
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let water = by_key("H2O")?;
/// let deep = diagram::phase_at(water, Kelvin::new(270.0), Pascals::new(5e7));
/// assert_eq!(deep, Some(Phase::Liquid));
/// let shallow = diagram::phase_at(water, Kelvin::new(270.0), Pascals::new(1e5));
/// assert_eq!(shallow, Some(Phase::Solid));
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn phase_at(id: SubstanceId, temperature: Kelvin, pressure: Pascals) -> Option<Phase> {
    id.substance()
        .phase_diagram()
        .and_then(|diagram| diagram.phase_at(temperature, pressure))
}

/// The rows with a phase diagram, in registry order.
///
/// P14.T54.a re-exports it at `substance::` in place of the held
/// [`substance::condensables`](super::condensables), whose five rows it extends.
pub fn condensables() -> impl Iterator<Item = SubstanceId> {
    ids().filter(|id| id.substance().phase_diagram().is_some())
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::substance::by_key;

    fn diagram(key: &str) -> &'static PhaseDiagram {
        by_key(key)
            .unwrap_or_else(|| panic!("{key} is a row"))
            .substance()
            .phase_diagram()
            .unwrap_or_else(|| panic!("{key} has a phase diagram"))
    }

    fn relative(ours: f64, theirs: f64) -> f64 {
        (ours / theirs - 1.0).abs()
    }

    /// The fourteen condensables, in registry order: the five of P14.T13.c's held columns and
    /// every row whose reference equation of state was read.
    #[test]
    fn diagram_condensables_are_the_fourteen_in_registry_order() {
        let keys: Vec<String> = condensables()
            .map(|id| id.substance().key().as_str().to_owned())
            .collect();
        assert_eq!(
            keys,
            [
                "H2O", "CH4", "NH3", "N2", "O2", "CO2", "Ar", "Kr", "Xe", "N2O", "CO", "SO2",
                "H2S", "C2H6"
            ]
        );
        // Every held condensable has a diagram.
        for id in crate::substance::condensables() {
            assert!(id.substance().phase_diagram().is_some(), "{id:?}");
        }
    }

    /// Triple and critical points are their references' values, exactly.
    #[test]
    fn diagram_points_are_exact() {
        for (key, points) in [
            ("H2O", [273.16, 611.657, 647.096, 22.064e6]),
            ("CH4", [90.6941, 11_696.0, 190.564, 4.5992e6]),
            ("NH3", [195.49, 6_053.39, 405.56, 11.3634e6]),
            ("N2", [63.151, 12_523.0, 126.192, 3.3958e6]),
            ("O2", [54.361, 146.33, 154.581, 5.043e6]),
            ("CO2", [216.592, 517_950.0, 304.1282, 7.3773e6]),
            ("Ar", [83.8058, 68_891.0, 150.687, 4.863e6]),
            ("Kr", [115.77, 73_502.837_48, 209.48, 5.525e6]),
            ("Xe", [161.4, 81_747.799_07, 289.733, 5.842e6]),
            ("N2O", [182.33, 87_837.439_23, 309.52, 7.245e6]),
            ("CO", [68.16, 15_536.876_66, 132.86, 3.494e6]),
            ("SO2", [197.7, 1_666.144_079, 430.64, 7.886_587_6e6]),
            ("H2S", [187.7, 23_258.855_84, 373.1, 9.0e6]),
            ("C2H6", [90.368, 1.142_107_639, 305.322, 4.8722e6]),
        ] {
            let d = diagram(key);
            assert_same_bits(d.triple_temperature().value(), points[0]);
            assert_same_bits(d.triple_pressure().value(), points[1]);
            assert_same_bits(d.critical_temperature().value(), points[2]);
            assert_same_bits(d.critical_pressure().value(), points[3]);
            // The liquid branch passes the selected triple point within its error.
            let at_triple = d.saturation_pressure(d.triple_temperature()).value();
            assert!(relative(at_triple, points[1]) < 7e-4, "{key}: {at_triple}");
        }
    }

    /// The liquid branches' checks: each row's key, its ancillary's stated error (relative), and
    /// three (K, Pa) points of its equation of state; `NaN` marks none.
    type LiquidCheck = (&'static str, f64, [(f64, f64); 3]);

    const LIQUID_CHECKS: [LiquidCheck; 14] = [
        (
            "H2O",
            2.5e-4,
            [
                (275.0, 698.451_167),
                (450.0, 932_203.564),
                (625.0, 16_908_269.3),
            ],
        ),
        (
            "CH4",
            2e-4,
            [(100.0, 34_376.0), (120.0, 191_430.0), (180.0, 3_285_200.0)],
        ),
        (
            "N2",
            7.5e-4,
            [(70.0, 38_540.0), (100.0, 778_270.0), (120.0, 2_510_580.0)],
        ),
        (
            "CO2",
            1.2e-4,
            [
                (220.0, 599_130.0),
                (250.0, 1_785_000.0),
                (300.0, 6_713_100.0),
            ],
        ),
        (
            "Ar",
            4e-4,
            [
                (90.0, 133_510.0),
                (120.0, 1_213_000.0),
                (140.0, 3_168_200.0),
            ],
        ),
        (
            "O2",
            2e-4,
            [(70.0, 6_262.0), (90.0, 99_350.0), (150.0, 4_218_600.0)],
        ),
        (
            "NH3",
            2e-4,
            [(195.49, 6_053.39), (239.834, 101_325.0), (405.0, f64::NAN)],
        ),
        (
            "C2H6",
            2.4e-4,
            [
                (122.61, 505.061_90),
                (176.35, 63_552.397),
                (283.83, 3_064_196.5),
            ],
        ),
        (
            "SO2",
            8e-5,
            [
                (232.64, 20_954.523),
                (290.88, 305_363.98),
                (360.76, 2_135_686.1),
            ],
        ),
        (
            "CO",
            6.6e-4,
            [
                (77.86, 64_412.353),
                (94.04, 340_327.72),
                (126.39, 2_584_175.1),
            ],
        ),
        (
            "H2S",
            1.41e-4,
            [
                (215.51, 115_712.74),
                (261.86, 730_138.46),
                (354.56, 6_512_067.8),
            ],
        ),
        (
            "N2O",
            6.8e-5,
            [
                (201.41, 250_528.41),
                (233.21, 940_779.91),
                (296.8, 5_484_721.4),
            ],
        ),
        (
            "Kr",
            9.1e-5,
            [
                (129.83, 208_899.02),
                (153.25, 765_414.98),
                (200.11, 4_229_845.4),
            ],
        ),
        (
            "Xe",
            3.4e-5,
            [
                (180.65, 228_484.56),
                (212.73, 823_857.60),
                (276.9, 4_480_410.6),
            ],
        ),
    ];

    /// Each liquid branch at three temperatures against its reference's equation of state, within
    /// the ancillary's stated error: the paper's own saturation table where it prints one (IAPWS
    /// R6-95(2018), Table 8; Setzmann and Wagner 1991, Table 39; Span et al. 2000, pp. 1403–1405;
    /// Span and Wagner 1996, Table 34; Tegeler et al. 1999, Table 33; Stewart et al. 1991, Table
    /// 10, which states no error and prints four or five figures), Gao et al. 2023's Table 1
    /// points, and for CoolProp's fits that equation of state solved for phase equilibrium from
    /// CoolProp's coefficients (`research/saturation/data.json`'s grids, Akasaka 2008's method).
    #[test]
    fn diagram_liquid_branches_meet_their_equations_of_state() {
        for (key, within, points) in LIQUID_CHECKS {
            let d = diagram(key);
            for (t, p) in points {
                if p.is_nan() {
                    continue;
                }
                let ours = d.saturation_pressure(Kelvin::new(t)).value();
                assert!(
                    relative(ours, p) < within,
                    "{key} at {t} K: {ours} against {p}"
                );
            }
        }
        // Ammonia's critical point closes its curve.
        let ammonia = diagram("NH3");
        let below = ammonia
            .saturation_pressure(Kelvin::new(405.56 * (1.0 - 1e-12)))
            .value();
        assert!(relative(below, 11.3634e6) < 1e-9);
    }

    /// Methane within 0.1% of NIST's WebBook at 100, 150 and 180 K (science-p14-t24f §6.3), where
    /// the held Clausius–Clapeyron curve runs 9.5–11.3% high.
    #[test]
    fn diagram_methane_meets_the_webbook() {
        let d = diagram("CH4");
        for (t, p) in [
            (100.0, 34_375.892),
            (150.0, 1_039_961.3),
            (180.0, 3_285_180.7),
        ] {
            let ours = d.saturation_pressure(Kelvin::new(t)).value();
            assert!(relative(ours, p) < 1e-3, "{t} K: {ours} against {p}");
        }
    }

    /// Each sublimation piece is its published equation plus its anchor, at three temperatures in
    /// its range; the branch is continuous at the triple point and at every join; the published
    /// solid branches with a stated error (water's, carbon dioxide's and argon's) stay within it,
    /// and the anchors of the rest are the misses they close.
    #[test]
    fn diagram_solid_branches_are_their_anchored_equations() {
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            let key = id.substance().key();
            assert_eq!(d.anchors().len(), d.sublimation().len(), "{key}");
            let mut top = d.triple_temperature();
            let mut above = d.vapour_pressure().ln_pressure(top);
            for (piece, anchor) in d.sublimation().iter().zip(d.anchors()) {
                let joined = piece.equation().ln_pressure(top) + anchor;
                assert!((joined - above).abs() < 1e-12, "{key} at {top:?}");
                let lower = piece.lower().value();
                for f in [0.1, 0.5, 0.9] {
                    let t = Kelvin::new(lower + f * (top.value() - lower));
                    let ours = d.ln_saturation_pressure(t).unwrap();
                    let published = piece.equation().ln_pressure(t);
                    assert!((ours - published - anchor).abs() < 1e-12, "{key} at {t:?}");
                }
                above = piece.equation().ln_pressure(piece.lower()) + anchor;
                top = piece.lower();
            }
            // Continuous across the triple point.
            let triple = d.triple_temperature().value();
            let solid = d.solid_ln_pressure(Kelvin::new(triple));
            let liquid = d.vapour_pressure().ln_pressure(Kelvin::new(triple));
            assert!((solid - liquid).abs() < 1e-12, "{key}");
        }
        // The published branches with a stated error, within it at the triple point: IAPWS
        // R14-08's 0.0011%, Span and Wagner's 250 Pa and Tegeler et al.'s 0.05%.
        for (key, within) in [("H2O", 1.1e-5), ("CO2", 250.0 / 517_950.0), ("Ar", 5e-4)] {
            assert!(diagram(key).anchors()[0].abs() < within, "{key}");
        }
    }

    /// The solid branches against measurements and their papers' own check values: ice Ih at
    /// 270, 250, 200 and 150 K (Wagner, Riethmann, Feistel and Harvey 2011, Table 1, IAPWS-95 and
    /// the ice Ih equation in equilibrium), carbon dioxide's 101,325 Pa at 194.6855 K (Span and
    /// Wagner 1996, Sec. 3.4), carbon monoxide's 3.74 kPa at its α–β transition (Morrison,
    /// Newsham and Weir 1968) and nitrogen's 0.467–0.667 Pa at its own (Raposa et al. 2024,
    /// Table A3's spread, 35.5–35.7 K).
    #[test]
    fn diagram_solid_branches_meet_their_checks() {
        let water = diagram("H2O");
        for (t, p) in [
            (270.0, 470.076_477),
            (250.0, 76.016_722),
            (200.0, 0.162_595_324),
            (150.0, 6.095_677_59e-6),
        ] {
            let ours = water.saturation_pressure(Kelvin::new(t)).value();
            assert!(relative(ours, p) < 1e-4, "ice at {t} K: {ours}");
        }
        let frost = diagram("CO2")
            .saturation_pressure(Kelvin::new(194.6855))
            .value();
        assert!(relative(frost, 101_325.0) < 1e-4, "{frost}");
        let carbon_monoxide = diagram("CO")
            .saturation_pressure(Kelvin::new(61.585))
            .value();
        assert!(
            relative(carbon_monoxide, 3_740.0) < 0.02,
            "{carbon_monoxide}"
        );
        let nitrogen = diagram("N2")
            .saturation_pressure(Kelvin::new(35.61))
            .value();
        assert!((0.467..=0.667).contains(&nitrogen), "{nitrogen}");
    }

    /// Below the coldest piece the branch is that piece's Clausius–Clapeyron tangent, continuous
    /// with it in value and slope; at and above the critical temperature, at or below 0 K and for
    /// NaN the curve answers as the held one does.
    #[test]
    fn diagram_curves_extrapolate_and_answer_outside() {
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            let key = id.substance().key();
            let coldest = d.sublimation().last().unwrap();
            let lower = coldest.lower();
            let at = d.ln_saturation_pressure(lower).unwrap();
            let just_below = d
                .ln_saturation_pressure(Kelvin::new(lower.value() * (1.0 - 1e-9)))
                .unwrap();
            assert!((at - just_below).abs() < 1e-6, "{key}");
            let slope_at = d.clausius_clapeyron_enthalpy_j_per_mol(lower).unwrap();
            let slope_below = d
                .clausius_clapeyron_enthalpy_j_per_mol(Kelvin::new(lower.value() * 0.5))
                .unwrap();
            assert!(relative(slope_below, slope_at) < 1e-12, "{key}");
            assert!(
                d.saturation_pressure(d.critical_temperature())
                    .value()
                    .is_infinite()
            );
            assert_same_bits(d.saturation_pressure(Kelvin::ZERO).value(), 0.0);
            assert_same_bits(d.saturation_pressure(Kelvin::new(f64::NAN)).value(), 0.0);
            assert_eq!(d.ln_saturation_pressure(d.critical_temperature()), None);
        }
    }

    /// Every curve rises from 1 K to its critical temperature, and its Clausius–Clapeyron enthalpy
    /// is its slope, R T² d ln p ÷ dT, against a central difference.
    #[test]
    fn diagram_curves_rise_and_their_enthalpies_are_their_slopes() {
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            let key = id.substance().key();
            let critical = d.critical_temperature().value();
            let mut last = f64::NEG_INFINITY;
            for k in 1..=2_000_u32 {
                let t = 1.0 + (critical - 1.0) * f64::from(k) / 2_001.0;
                let ln_p = d.ln_saturation_pressure(Kelvin::new(t)).unwrap();
                assert!(ln_p > last, "{key} at {t} K");
                last = ln_p;
                if k % 97 == 0 {
                    let h = 1e-5 * t;
                    let difference = (d.ln_saturation_pressure(Kelvin::new(t + h)).unwrap()
                        - d.ln_saturation_pressure(Kelvin::new(t - h)).unwrap())
                        / (2.0 * h);
                    let enthalpy = d
                        .clausius_clapeyron_enthalpy_j_per_mol(Kelvin::new(t))
                        .unwrap();
                    let triple = d.triple_temperature().value();
                    let joins = d
                        .sublimation()
                        .iter()
                        .map(|piece| piece.lower().value())
                        .chain([triple]);
                    if joins.into_iter().all(|join| (t - join).abs() > 2.0 * h) {
                        assert!(
                            relative(enthalpy, MOLAR_GAS_CONSTANT * t * t * difference) < 1e-5,
                            "{key} at {t} K"
                        );
                    }
                }
            }
        }
    }

    /// The Clausius–Clapeyron enthalpy of vaporisation at the triple point exceeds the reference's
    /// h″ − h′ by the vapour's non-ideality there, a few per cent for most rows and up to about a
    /// tenth for carbon dioxide's 5.2 bar.
    #[test]
    fn diagram_triple_point_enthalpies_follow_the_references() {
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            let key = id.substance().key();
            let slope = d
                .clausius_clapeyron_enthalpy_j_per_mol(d.triple_temperature())
                .unwrap();
            let reference = d.triple_vaporisation_enthalpy_j_per_mol();
            let excess = slope / reference - 1.0;
            assert!(
                (-1e-3..0.12).contains(&excess),
                "{key}: {slope} against {reference}"
            );
            if let (Some(fusion), Some(sublimation)) = (
                d.triple_fusion_enthalpy_j_per_mol(),
                d.triple_sublimation_enthalpy_j_per_mol(),
            ) {
                assert_same_bits(sublimation, reference + fusion);
            }
        }
    }

    /// The saturation temperature inverts the curve over the solid, the liquid and the
    /// extrapolation, and answers `None` from the curve's end at the critical temperature.
    #[test]
    fn diagram_saturation_temperature_inverts_the_curve() {
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            let key = id.substance().key();
            let critical = d.critical_temperature().value();
            for k in 1..200_u32 {
                let t = 0.25 * d.sublimation().last().unwrap().lower().value()
                    + (critical - 0.25 * d.sublimation().last().unwrap().lower().value())
                        * f64::from(k)
                        / 200.0;
                let p = d.saturation_pressure(Kelvin::new(t));
                let back = d.saturation_temperature(p).unwrap().value();
                assert!(relative(back, t) < 1e-9, "{key}: {t} K gives {back} K");
            }
            let end = Pascals::new(
                math::exp(d.vapour_pressure().ln_pressure(d.critical_temperature()))
                    * (1.0 + 1e-12),
            );
            assert_eq!(d.saturation_temperature(end), None, "{key}");
            assert_eq!(d.saturation_temperature(Pascals::ZERO), None);
            assert_eq!(d.saturation_temperature(Pascals::new(f64::NAN)), None);
        }
    }

    /// The surfaces audit's liquid windows (§2.5), the boiling point at 0.006, 1, 1.5, 10 and 92
    /// bar: within 2 K of its Clausius–Clapeyron entries, but where its constant enthalpy misses
    /// by more, which these curves correct: water at 92 bar boils at 578.1 K (the audit's 569),
    /// ethane at 0.006 bar at 123.9 K (120) and ammonia at 92 bar at 393.7 K (390). An entry the
    /// audit marks supercritical reads `None` here, the pressure being at or above the critical.
    #[test]
    fn diagram_reproduces_the_audit_s_liquid_windows() {
        let pressures = [0.006e5, 1e5, 1.5e5, 10e5, 92e5];
        for (key, windows) in [
            (
                "H2O",
                [None, Some(373.0), Some(385.0), Some(452.0), Some(578.08)],
            ),
            ("CO2", [None, None, None, Some(235.0), None]),
            ("N2", [None, Some(77.0), Some(81.0), Some(105.0), None]),
            ("CH4", [None, Some(112.0), Some(117.0), Some(151.0), None]),
            (
                "C2H6",
                [Some(123.90), Some(184.0), Some(192.0), Some(243.0), None],
            ),
            (
                "NH3",
                [None, Some(240.0), Some(248.0), Some(298.0), Some(393.69)],
            ),
            ("SO2", [None, Some(263.0), Some(273.0), Some(329.0), None]),
            ("H2S", [None, Some(213.0), Some(221.0), Some(272.0), None]),
            ("CO", [None, Some(82.0), Some(85.0), Some(110.0), None]),
            ("Ar", [None, Some(87.0), Some(91.0), Some(118.0), None]),
        ] {
            let d = diagram(key);
            for (p, window) in pressures.into_iter().zip(windows) {
                let pressure = Pascals::new(p);
                let boiling = (pressure >= d.triple_pressure())
                    .then(|| d.saturation_temperature(pressure))
                    .flatten();
                match (boiling, window) {
                    (Some(ours), Some(audit)) => assert!(
                        (ours.value() - audit).abs() < 2.0,
                        "{key} at {p} Pa: {ours:?} against {audit}"
                    ),
                    (None, None) => {}
                    _ => panic!("{key} at {p} Pa: {boiling:?} against {window:?}"),
                }
            }
        }
    }

    /// Each condensable's four regions, as the held columns' test has them, on its own curve.
    #[test]
    fn diagram_each_condensable_has_its_four_regions() {
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            let key = id.substance().key();
            let (t_t, p_t) = (d.triple_temperature(), d.triple_pressure());
            let (t_c, p_c) = (d.critical_temperature(), d.critical_pressure());
            let mid = Kelvin::new(f64::midpoint(t_t.value(), t_c.value()));
            let cold = Kelvin::new(0.8 * t_t.value());
            let hot = Kelvin::new(1.5 * t_c.value());
            let sat = |t: Kelvin| d.saturation_pressure(t);
            for (t, p, phase) in [
                (cold, p_t, Phase::Solid),
                (cold, sat(cold) * 0.5, Phase::Vapour),
                (mid, sat(mid) * 1.5, Phase::Liquid),
                (mid, sat(mid) * 0.5, Phase::Vapour),
                (hot, p_c * 2.0, Phase::Supercritical),
                (hot, p_c * 0.5, Phase::Vapour),
                (t_c, p_c, Phase::Supercritical),
            ] {
                assert_eq!(d.phase_at(t, p), Some(phase), "{key} at {t:?}, {p:?}");
                assert_eq!(phase_at(id, t, p), Some(phase));
            }
            assert_eq!(d.phase_at(mid, sat(mid)), Some(Phase::Liquid), "{key}");
            assert_eq!(d.phase_at(cold, sat(cold)), Some(Phase::Solid), "{key}");
            for (t, p) in [
                (Kelvin::new(f64::NAN), p_t),
                (mid, Pascals::new(f64::NAN)),
                (Kelvin::new(-1.0), p_t),
                (mid, Pascals::new(-1.0)),
            ] {
                assert_eq!(d.phase_at(t, p), None);
            }
        }
    }

    /// The melting curves: each meets its triple point; a rising one freezes the liquid under
    /// pressure, above the critical temperature too. Water's falls through ice Ih, which pressure
    /// melts, to 251.165 K, below which no liquid exists, and rises from there through ices III, V,
    /// VI and VII to 715 K: each of IAPWS R14-08's Eqs. (1)–(5) gives its Table 3 value.
    #[test]
    fn diagram_melting_curves_bound_the_solid() {
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            let key = id.substance().key();
            match d.melting() {
                Some(MeltingCurve::Rising(_)) => {
                    let at_triple = d.melting_pressure(d.triple_temperature()).unwrap();
                    assert!(relative(at_triple.value(), d.triple_pressure().value()) < 0.01);
                    let warm = Kelvin::new(d.triple_temperature().value() + 5.0);
                    let melting = d.melting_pressure(warm).unwrap();
                    // The liquid's field, from its saturation pressure to its melting pressure.
                    assert!(melting > d.saturation_pressure(warm) * 10.0, "{key}");
                    assert_eq!(
                        d.phase_at(warm, melting * 1.01),
                        Some(Phase::Solid),
                        "{key}"
                    );
                    assert_eq!(
                        d.phase_at(warm, melting * 0.99),
                        Some(Phase::Liquid),
                        "{key}"
                    );
                    let hot = Kelvin::new(d.critical_temperature().value() * 1.2);
                    let squeezed = d.melting_pressure(hot).unwrap() * 1.01;
                    assert_eq!(d.phase_at(hot, squeezed), Some(Phase::Solid), "{key}");
                    assert_eq!(d.melting_pressure(Kelvin::new(1.0)), None);
                    assert_eq!(d.falling_melting_pressure(warm), None);
                }
                Some(MeltingCurve::Falling { .. }) => assert_eq!(key.as_str(), "H2O"),
                None => {
                    let warm = Kelvin::new(d.triple_temperature().value() + 1.0);
                    assert_eq!(d.phase_at(warm, Pascals::new(1e9)), Some(Phase::Liquid));
                }
            }
        }
        let water = diagram("H2O");
        let falling = |t: f64| {
            water
                .falling_melting_pressure(Kelvin::new(t))
                .unwrap()
                .value()
        };
        let rising = |t: f64| water.melting_pressure(Kelvin::new(t)).unwrap().value();
        for (ours, table) in [
            (falling(260.0), 138.268e6),
            (rising(254.0), 268.685e6),
            (rising(265.0), 479.640e6),
            (rising(320.0), 1_356.76e6),
            (rising(550.0), 6_308.71e6),
        ] {
            assert!(relative(ours, table) < 1e-5, "{ours} against {table}");
        }
        // The branches meet at Table 1's triple points.
        assert!(relative(falling(251.165), 208.566e6) < 1e-5);
        assert_same_bits(rising(251.165), 208.566e6);
        for (t, p) in [(256.164, 350.1e6), (273.31, 632.4e6), (355.0, 2_216e6)] {
            let below = rising(t * (1.0 - 1e-12));
            assert!(relative(below, p) < 2e-3, "{t} K: {below}");
        }
        assert!(relative(falling(270.0), 39.313e6) < 1e-4);
        let at = |t: f64, p: f64| water.phase_at(Kelvin::new(t), Pascals::new(p));
        assert_eq!(at(270.0, 1e5), Some(Phase::Solid));
        assert_eq!(at(270.0, 5e7), Some(Phase::Liquid));
        assert_eq!(at(255.0, 2e8), Some(Phase::Liquid));
        assert_eq!(at(255.0, 3.2e8), Some(Phase::Solid), "ice III");
        assert_eq!(at(250.0, 3e8), Some(Phase::Solid));
        assert_eq!(at(300.0, 1e8), Some(Phase::Liquid));
        assert_eq!(at(300.0, 1.2e9), Some(Phase::Solid), "ice VI");
        assert_eq!(at(600.0, 1e10), Some(Phase::Solid), "ice VII");
        assert_eq!(
            at(800.0, 1e10),
            Some(Phase::Supercritical),
            "not modelled above 715 K"
        );
        assert_eq!(water.melting_pressure(Kelvin::new(250.0)), None);
        assert_eq!(water.falling_melting_pressure(Kelvin::new(250.0)), None);
        assert_eq!(water.falling_melting_pressure(Kelvin::new(273.2)), None);
        // Ammonia's two pieces meet at the I–II–liquid point, 217.35 K and 307 MPa.
        let ammonia = diagram("NH3");
        let below = ammonia
            .melting_pressure(Kelvin::new(217.35 - 1e-9))
            .unwrap()
            .value();
        assert!(relative(below, 307e6) < 1e-3, "{below}");
    }

    /// The Solar System's surfaces lie in their phases on the new curves, as on the held ones, and
    /// the rows the held columns lack at points on either side of their curves: sulphur dioxide
    /// frost at 110 K under a millipascal, carbon monoxide ice at 38 K, ammonia ice at 150 K and
    /// 0.7 bar, and liquid ethane at Titan's surface (93.65 K and 1.467 bar; Fulchignoni et al.
    /// 2005, Nature 438, 785).
    #[test]
    fn diagram_the_solar_system_s_surfaces_lie_in_their_phases() {
        let earth = 101_325.0;
        for (key, t, p, phase) in [
            ("H2O", 288.0, earth, Phase::Liquid),
            ("H2O", 250.0, earth, Phase::Solid),
            ("H2O", 400.0, earth, Phase::Vapour),
            ("CO2", 140.0, 600.0, Phase::Solid),
            ("CO2", 210.0, 600.0, Phase::Vapour),
            ("CO2", 737.0, 9.2e6, Phase::Supercritical),
            ("CH4", 93.65, 1.467e5, Phase::Liquid),
            ("N2", 30.0, 1.0, Phase::Solid),
            ("N2", 70.0, earth, Phase::Liquid),
            ("N2", 288.0, earth, Phase::Vapour),
            ("Ar", 288.0, earth, Phase::Vapour),
            ("SO2", 110.0, 1e-3, Phase::Solid),
            ("CO", 38.0, 1.4, Phase::Solid),
            ("NH3", 150.0, 7e4, Phase::Solid),
            ("C2H6", 93.65, 1.467e5, Phase::Liquid),
        ] {
            assert_eq!(
                phase_at(by_key(key).unwrap(), Kelvin::new(t), Pascals::new(p)),
                Some(phase),
                "{key} at {t} K, {p} Pa"
            );
        }
        assert_eq!(
            phase_at(by_key("He").unwrap(), Kelvin::new(3.0), Pascals::new(1e5)),
            None
        );
        assert_eq!(
            saturation_pressure(by_key("He").unwrap(), Kelvin::new(3.0)),
            None
        );
    }

    /// The saturated liquids' densities at the triple point against their references, within each
    /// ancillary's stated error: the papers' (Wagner and Pruss 2002 and NIST's WebBook, 999.7925;
    /// Setzmann and Wagner 1991, 451.48; Span et al. 2000, 867.22; Span and Wagner 1996, 1,178.46;
    /// Tegeler et al. 1999, 1,416.77; within 0.03%), and for the others their equations of state
    /// at the triple point as CoolProp's fluid files give them, within the ancillary's error.
    #[test]
    fn diagram_liquid_densities_meet_their_references() {
        for (key, within, rho) in [
            ("H2O", 1e-4, 999.7925),
            ("CH4", 3e-4, 451.48),
            ("N2", 3e-4, 867.22),
            ("CO2", 3e-4, 1_178.46),
            ("Ar", 3e-4, 1_416.77),
            ("NH3", 4e-4, 733.848_663),
            ("O2", 2.5e-3, 1_306.076_806),
            ("C2H6", 2.7e-3, 651.529_481),
            ("SO2", 7.7e-4, 1_627.945_610),
            ("CO", 3.6e-3, 849.538_097),
            ("H2S", 1.3e-3, 992.310_041),
            ("N2O", 6.9e-3, 1_237.350_863),
            ("Kr", 8.4e-3, 2_446.677_223),
            ("Xe", 4.5e-3, 2_966.216_416),
        ] {
            let d = diagram(key);
            let ours = d.triple_liquid_density().value();
            assert!(relative(ours, rho) < within, "{key}: {ours} against {rho}");
            assert_same_bits(
                d.liquid_density(d.triple_temperature()).unwrap().value(),
                ours,
            );
            assert_eq!(
                d.liquid_density(Kelvin::new(d.triple_temperature().value() - 1.0)),
                None
            );
            let near_critical = d
                .liquid_density(Kelvin::new(d.critical_temperature().value() * 0.999))
                .unwrap()
                .value();
            assert!(near_critical < ours, "{key}");
        }
    }

    /// Each density equation at the triple point as a second, independent transcription of the same
    /// equations evaluates it (the lane's research for this step, `research/saturation/data.json`),
    /// which checks every coefficient's reading.
    #[test]
    fn diagram_density_equations_match_a_second_transcription() {
        for (key, rho) in [
            ("H2O", 999.789_134_651_148),
            ("CH4", 451.474_672_188_057_3),
            ("N2", 867.211_748_846_544_2),
            ("CO2", 1_178.525_122_234_527),
            ("Ar", 1_416.688_277_213_608_2),
            ("O2", 1_308.241_205_340_383),
            ("NH3", 733.892_842_654_820_5),
            ("C2H6", 651.096_315_357_612),
            ("SO2", 1_627.886_169_234_220_5),
            ("CO", 848.544_069_127_820_1),
            ("H2S", 992.041_673_815_927_4),
            ("N2O", 1_240.926_188_234_640_4),
            ("Kr", 2_441.476_149_697_971_3),
            ("Xe", 2_963.402_217_761_457),
        ] {
            let ours = diagram(key).triple_liquid_density().value();
            assert!(relative(ours, rho) < 1e-9, "{key}: {ours} against {rho}");
        }
    }

    /// Ice Ih's density within 0.02 kg m⁻³ of IAPWS R10-06(2009): its Table 6 at the triple point
    /// (916.709 492 200 at 611.657 Pa) and at the melting point under one atmosphere (916.721 463
    /// 419 at 273.152 519 K), and its Gibbs function at one atmosphere at 20, 100, 160 and 273.15
    /// K (`science-composition-crusts/ice_ih_density.out`); every solid phase's density where it
    /// was read, through its phase.
    #[test]
    fn diagram_solid_densities_follow_their_measurements() {
        let water = diagram("H2O");
        for (t, rho) in [
            (273.16, 916.709_492_2),
            (273.152_519, 916.721_463_419),
            (20.0, 933.795),
            (100.0, 933.049),
            (160.0, 929.873),
            (273.15, 916.722),
        ] {
            let ours = water.solid_density(Kelvin::new(t)).unwrap().value();
            assert!((ours - rho).abs() < 0.02, "ice Ih at {t} K: {ours}");
        }
        let at = |key: &str, t: f64| {
            diagram(key)
                .solid_density(Kelvin::new(t))
                .map(KilogramsPerCubicMetre::value)
        };
        // Maynard-Casely, Hester and Brand 2020: the fits' own values at the transitions.
        assert!((at("N2", 35.0).unwrap() - 1_016.0).abs() < 2.0);
        assert!((at("N2", 35.61).unwrap() - 1_005.22).abs() < 0.05);
        assert!((at("N2", 63.151).unwrap() - 956.82).abs() < 0.05);
        assert!((at("CH4", 20.0).unwrap() - 532.38).abs() < 0.05);
        assert!((at("CH4", 20.4).unwrap() - 529.00).abs() < 0.05);
        // Mangan et al. 2017's 80 K point, held below it.
        assert!((at("CO2", 80.0).unwrap() - 1_685.3).abs() < 0.1);
        assert_same_bits(at("CO2", 40.0).unwrap(), at("CO2", 80.0).unwrap());
        // Sears and Klug's 50 K cell, and an interpolation.
        assert!((at("Xe", 50.0).unwrap() - 3_719.98).abs() < 1e-9);
        assert!((at("Xe", 52.5).unwrap() - f64::midpoint(3_719.98, 3_707.88)).abs() < 1e-9);
        // Not read: argon's, hydrogen sulphide's, ethane's, β-oxygen's and β-carbon monoxide's.
        for (key, t) in [
            ("Ar", 50.0),
            ("H2S", 150.0),
            ("C2H6", 60.0),
            ("O2", 30.0),
            ("CO", 65.0),
        ] {
            assert_eq!(at(key, t), None, "{key} at {t} K");
        }
        assert_eq!(at("SO2", 250.0), None, "above the triple point");
        assert_eq!(at("SO2", -1.0), None);
    }

    /// Every diagram names a source for each of its values, by name in some source's
    /// comma-separated `covers`.
    #[test]
    fn diagram_values_name_their_sources() {
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            let key = id.substance().key();
            let mut needed = vec![
                "T_t", "p_t", "T_c", "p_c", "p_liquid", "ρ_l", "p_solid", "anchors", "L_vap",
            ];
            if d.melting().is_some() {
                needed.push("p_melt");
            }
            if d.solid_phases()
                .iter()
                .any(|phase| phase.density().is_some())
            {
                needed.push("ρ_s");
            }
            if d.triple_fusion_enthalpy_j_per_mol().is_some() {
                needed.push("L_fus");
            }
            for value in needed {
                assert!(
                    d.sources()
                        .iter()
                        .any(|s| s.covers().split(", ").any(|covered| covered == value)),
                    "{key}'s {value} names a source"
                );
            }
            for s in d.sources() {
                assert!(!s.citation().is_empty(), "{key}");
            }
            // Each solid phase list starts at 0 K and rises below the triple point; each
            // sublimation piece's lower bound falls.
            let phases = d.solid_phases();
            if let Some(first) = phases.first() {
                assert_same_bits(first.start().value(), 0.0);
            }
            assert!(
                phases.windows(2).all(|w| w[0].start() < w[1].start()),
                "{key}"
            );
            assert!(
                d.sublimation()
                    .windows(2)
                    .all(|w| w[0].lower() > w[1].lower()),
                "{key}"
            );
            assert!(d.sublimation()[0].lower() < d.triple_temperature(), "{key}");
        }
        // A cell-volume fit's molar mass is its row's.
        for key in ["CH4", "N2"] {
            let row = by_key(key).unwrap().substance();
            for phase in row.phase_diagram().unwrap().solid_phases() {
                if let Some(SolidDensity::CellVolume {
                    molar_mass_g_per_mol,
                    ..
                }) = phase.density()
                {
                    assert_same_bits(*molar_mass_g_per_mol, row.molar_mass_g_per_mol().unwrap());
                }
            }
        }
    }
}
