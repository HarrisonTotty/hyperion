//! The liquid columns: what a row is to light and to waves as a liquid on a body's surface (plan
//! 14, P14.T49.h; decision-r11-palette-liquids §1 and §4), resolved into the surface crate's
//! [`LiquidProperties`] by [`liquid_properties`] for P14.T54.a's palette.
//!
//! A row that can be a liquid carries [`LiquidColumns`] ([`Substance::liquid`]), a group of its
//! own beside the condensed columns, whose density it reads:
//!
//! - **the index and the absorption** come from rendering plan R08's committed liquid file, the
//!   one source of a liquid's n and k (decision-composition §1.1), reduced to R08's fifteen bake
//!   bins by `hyperion-fit`'s `liquid_optics` (`tables::liquid_optics`). The index is carried from
//!   the file's state to the record's temperature by Lorentz–Lorenz at constant specific
//!   refraction, (n² − 1) ÷ (n² + 2) ∝ ρ, through the row's own liquid density at both; the
//!   absorption is the file's, at the file's temperature (R08.T5.g's "one temperature per
//!   liquid"). A liquid's absorption is read only where its file's k is a measurement or a stated
//!   bound: a k the file holds at zero for want of one is not modelled, and the record leaves it
//!   absent (§1.4);
//! - **the scattering** is the pure liquid's, by Einstein–Smoluchowski–Cabannes from the row's
//!   reference equation of state ([`HelmholtzResidual`]), as Zhang, Hu and He 2009 compute it for
//!   water at zero salinity ([`pure_liquid_scattering_per_m`]);
//! - **the surface tension** is a reference correlation in Mulero, Cachadiña and Parra 2012's form
//!   ([`SurfaceTensionEquation`]), and **the viscosity** a reference correlation
//!   ([`ViscosityCorrelation`]);
//! - **the density** is the condensed columns' (P14.T49.b): the saturated liquid's ancillary
//!   equation for a row with a phase diagram, a stated value otherwise.
//!
//! A member with no source is absent from the record, never 0 (decision-r11-palette-liquids
//! §1.4), and the record's [`LiquidBasis`] is the least direct of its members' (§1.1).
//!
//! # What is built
//!
//! P14.T49.h's first step: the pure liquids with an R08 file (water, methane, ammonia, nitrogen,
//! ethane and the 75 wt% sulphuric acid), with water's scattering, surface tension and viscosity.
//! Its later steps add the other liquids' transport and scattering, the rows with no file (their
//! index from the gas's refractivity), the solutions' mixing rules and the melts
//! (`docs/agent/plans/galaxy-generation/14-planetary-systems.md`, P14.T49.h's record). Until then
//! [`liquid_properties`] answers [`ResolveLiquidError::NotALiquid`] for those, and a given
//! composition is [`ResolveLiquidError::CompositionNotModelled`].
//!
//! Generator version 22 publishes none of it: nothing on the record reads these columns until
//! P14.T54.a resolves the palette (decision-bump-plan-after-22 §1.2).

use std::error::Error;
use std::fmt;

use hyperion_surface::field::{LIQUID_BINS, LiquidBasis, LiquidProperties};
use hyperion_surface::substance_key::SubstanceKey;

use super::condensed::{CondensedPhase, GrainDensity};
use super::diagram::{DensityEquation, DensityForm, PhaseDiagram};
use super::helmholtz::HelmholtzResidual;
use super::rows::liquids::LIQUIDS;
use super::{Source, Substance, SubstanceId};
use crate::math;
use crate::units::consts::BOLTZMANN_CONSTANT;
use crate::units::{Kelvin, KilogramsPerCubicMetre};

/// The edges of the liquid bins, nm: 380 + 25.33 k for k = 0 to 15, R08's bake bins (Design note
/// 5), whose centres are `hyperion_surface::field::LIQUID_BIN_CENTRES_NM`.
pub const LIQUID_BIN_EDGES_NM: [f64; LIQUID_BINS + 1] = bin_edges();

/// [`LIQUID_BIN_EDGES_NM`]: 380 nm, then 380 ÷ 15 = 76 ÷ 3 nm a bin.
#[must_use]
const fn bin_edges() -> [f64; LIQUID_BINS + 1] {
    let mut edges = [0.0; LIQUID_BINS + 1];
    let mut k = 0;
    let mut step = 0.0;
    while k <= LIQUID_BINS {
        edges[k] = 380.0 + 76.0 * step / 3.0;
        k += 1;
        step += 1.0;
    }
    edges
}

/// A row's index and absorption, from one of R08's liquid files.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidOptics {
    pub(crate) index: &'static [f64; LIQUID_BINS],
    pub(crate) absorption: Option<(&'static [f64; LIQUID_BINS], LiquidBasis)>,
    pub(crate) state: Kelvin,
    pub(crate) index_basis: LiquidBasis,
    pub(crate) index_checked: Option<(Kelvin, Kelvin)>,
    pub(crate) absorption_checked: Option<(Kelvin, Kelvin)>,
}

impl LiquidOptics {
    /// The file's real index by bin, at [`state`](Self::state).
    #[must_use]
    pub const fn index(&self) -> &'static [f64; LIQUID_BINS] {
        self.index
    }

    /// The file's absorption coefficient a = 4πk ÷ λ by bin, m⁻¹, at its state, with its basis;
    /// `None` where the file's k is not modelled, held at zero for want of a measurement or a
    /// stated bound (decision-r11-palette-liquids §1.4: absent, never 0).
    #[must_use]
    pub const fn absorption_per_m(&self) -> Option<(&'static [f64; LIQUID_BINS], LiquidBasis)> {
        self.absorption
    }

    /// The temperature the file's constants were measured or derived at: its reduction's, which
    /// its `temperatureK` label rounds to the kelvin.
    #[must_use]
    pub const fn state(&self) -> Kelvin {
        self.state
    }

    /// The index's basis at the file's state, by decision-r11-palette-liquids §1.1's definitions
    /// applied to what the file says it is (decision-r08-t5g §3), not by its provenance field's
    /// name.
    #[must_use]
    pub const fn index_basis(&self) -> LiquidBasis {
        self.index_basis
    }

    /// The temperatures over which the carried index was checked against a reference
    /// formulation and keeps [`index_basis`](Self::index_basis), or `None` where none was: outside
    /// them the carry is an extrapolation, and the index is `Derived`.
    #[must_use]
    pub const fn index_checked(&self) -> Option<(Kelvin, Kelvin)> {
        self.index_checked
    }

    /// The temperatures over which the file's absorption, measured at one temperature, keeps
    /// its basis, or `None` where no window is stated: outside them the absorption is held from
    /// another temperature, and is `Derived` (decision-p14-t49h-liquids §1).
    #[must_use]
    pub const fn absorption_checked(&self) -> Option<(Kelvin, Kelvin)> {
        self.absorption_checked
    }
}

/// A row's scattering inputs: its reference equation of state, for `κ_T`, and its depolarisation
/// ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidScattering {
    pub(crate) equation: &'static HelmholtzResidual,
    pub(crate) depolarisation_ratio: f64,
    pub(crate) basis: LiquidBasis,
}

impl LiquidScattering {
    /// The reference equation of state the isothermal compressibility comes from.
    #[must_use]
    pub const fn equation(&self) -> &'static HelmholtzResidual {
        self.equation
    }

    /// The liquid's depolarisation ratio δ of light scattered at 90°, dimensionless.
    #[must_use]
    pub const fn depolarisation_ratio(&self) -> f64 {
        self.depolarisation_ratio
    }

    /// The basis of the equation of state and the ratio.
    #[must_use]
    pub const fn basis(&self) -> LiquidBasis {
        self.basis
    }
}

/// A surface tension in Mulero, Cachadiña and Parra 2012's form, σ = Σ σᵢ (1 − T ÷ `T_c`)^nᵢ,
/// N m⁻¹, with the range its source fits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceTensionEquation {
    pub(crate) critical_temperature: Kelvin,
    pub(crate) terms: &'static [(f64, f64)],
    pub(crate) valid_from: Kelvin,
    pub(crate) valid_to: Kelvin,
    pub(crate) basis: LiquidBasis,
}

impl SurfaceTensionEquation {
    /// The surface tension at `temperature`, N m⁻¹, the terms summed in their order; 0 at and
    /// above the critical temperature.
    #[must_use]
    pub fn surface_tension_n_per_m(&self, temperature: Kelvin) -> f64 {
        let reduced = 1.0 - temperature.value() / self.critical_temperature.value();
        if reduced <= 0.0 {
            return 0.0;
        }
        let mut sigma = 0.0;
        for &(coefficient, exponent) in self.terms {
            sigma += coefficient * math::powf(reduced, exponent);
        }
        sigma
    }

    /// The basis at `temperature`: the source's inside its range, `Derived` outside it.
    #[must_use]
    pub fn basis_at(&self, temperature: Kelvin) -> LiquidBasis {
        if temperature >= self.valid_from && temperature <= self.valid_to {
            self.basis
        } else {
            self.basis.max(LiquidBasis::Derived)
        }
    }
}

/// A reference correlation for a liquid's dynamic viscosity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViscosityCorrelation {
    /// IAPWS R12-08's formulation for water, μ = μ₀(T̄) μ₁(T̄, ρ̄), with the critical
    /// enhancement μ₂ set to 1, as its §2.8 allows outside the critical region.
    Iapws2008(&'static Iapws2008Viscosity),
}

/// IAPWS R12-08's coefficients (IAPWS 2008, "Release on the IAPWS Formulation 2008 for the
/// Viscosity of Ordinary Water Substance", Berlin; Huber et al. 2009, J. Phys. Chem. Ref. Data 38,
/// 101).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Iapws2008Viscosity {
    pub(crate) reference_temperature: Kelvin,
    pub(crate) reference_density: KilogramsPerCubicMetre,
    pub(crate) reference_viscosity_pa_s: f64,
    pub(crate) dilute: [f64; 4],
    pub(crate) residual: &'static [(i32, i32, f64)],
    pub(crate) valid_from: Kelvin,
    pub(crate) valid_to: Kelvin,
    pub(crate) critical_region: (
        Kelvin,
        Kelvin,
        KilogramsPerCubicMetre,
        KilogramsPerCubicMetre,
    ),
}

impl Iapws2008Viscosity {
    /// The viscosity at `temperature` and `density`, Pa s: μ* × μ₀ × μ₁, with μ₀ = 100 √T̄ ÷ Σ Hᵢ ÷
    /// T̄ⁱ (Eq. 11) and μ₁ = exp[ρ̄ Σᵢ (1 ÷ T̄ − 1)ⁱ Σⱼ Hᵢⱼ (ρ̄ − 1)ʲ] (Eq. 12), the sums in Table
    /// 1's and Table 2's order.
    #[must_use]
    pub fn viscosity_pa_s(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> f64 {
        let t = temperature.value() / self.reference_temperature.value();
        let rho = density.value() / self.reference_density.value();
        let mut denominator = 0.0;
        for (power, &h) in (0..).zip(self.dilute.iter()) {
            denominator += h / math::powi(t, power);
        }
        let mu0 = 100.0 * t.sqrt() / denominator;
        let mut sum = 0.0;
        for &(i, j, h) in self.residual {
            sum += math::powi(1.0 / t - 1.0, i) * h * math::powi(rho - 1.0, j);
        }
        let mu1 = math::exp(rho * sum);
        self.reference_viscosity_pa_s * mu0 * mu1
    }

    /// The basis at `temperature` and `density`: `Measured` inside the release's range of
    /// validity at stable states and outside the critical region its Eq. (13) bounds, where the
    /// enhancement this omits stays under 2%; `Derived` elsewhere.
    #[must_use]
    pub fn basis_at(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> LiquidBasis {
        let (t_low, t_high, rho_low, rho_high) = self.critical_region;
        let critical =
            temperature > t_low && temperature < t_high && density > rho_low && density < rho_high;
        if temperature >= self.valid_from && temperature <= self.valid_to && !critical {
            LiquidBasis::Measured
        } else {
            LiquidBasis::Derived
        }
    }
}

impl ViscosityCorrelation {
    /// The viscosity at `temperature` and `density`, Pa s, with its basis there.
    #[must_use]
    pub fn viscosity_pa_s(
        &self,
        temperature: Kelvin,
        density: KilogramsPerCubicMetre,
    ) -> (f64, LiquidBasis) {
        match self {
            Self::Iapws2008(c) => (
                c.viscosity_pa_s(temperature, density),
                c.basis_at(temperature, density),
            ),
        }
    }
}

/// A row's liquid columns (decision-r11-palette-liquids §4), each `None` where not modelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidColumns {
    pub(crate) optics: Option<LiquidOptics>,
    pub(crate) scattering: Option<LiquidScattering>,
    pub(crate) surface_tension: Option<SurfaceTensionEquation>,
    pub(crate) viscosity: Option<ViscosityCorrelation>,
    pub(crate) sources: &'static [Source],
}

impl LiquidColumns {
    /// The index and absorption, from the row's R08 file.
    #[must_use]
    pub const fn optics(&self) -> Option<&LiquidOptics> {
        self.optics.as_ref()
    }

    /// The scattering's inputs.
    #[must_use]
    pub const fn scattering(&self) -> Option<&LiquidScattering> {
        self.scattering.as_ref()
    }

    /// The surface tension's correlation.
    #[must_use]
    pub const fn surface_tension(&self) -> Option<&SurfaceTensionEquation> {
        self.surface_tension.as_ref()
    }

    /// The viscosity's correlation.
    #[must_use]
    pub const fn viscosity(&self) -> Option<&ViscosityCorrelation> {
        self.viscosity.as_ref()
    }

    /// Where every value comes from, with its licence basis.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }
}

impl Substance {
    /// Its liquid columns, for a row the registry models as a liquid on a surface; `None` where
    /// they are not modelled.
    #[must_use]
    pub fn liquid(&self) -> Option<&'static LiquidColumns> {
        LIQUIDS
            .iter()
            .find(|(key, _)| key.as_str() == self.key().as_str())
            .map(|(_, columns)| columns)
    }
}

/// A solution's composition by mole fraction, each component a row (decision-p14-t49b-condensed
/// §1.4: the composition's invariant is its moles, and the mixing rules take mole fractions).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidComposition<'a> {
    mole_fractions: &'a [(SubstanceId, f64)],
}

impl<'a> LiquidComposition<'a> {
    /// The composition of `mole_fractions`: one or more components, each once, each fraction in
    /// (0, 1], summing to 1 within 10⁻⁹.
    ///
    /// # Errors
    ///
    /// [`BuildLiquidCompositionError`] naming the first rule broken.
    pub fn new(
        mole_fractions: &'a [(SubstanceId, f64)],
    ) -> Result<Self, BuildLiquidCompositionError> {
        if mole_fractions.is_empty() {
            return Err(BuildLiquidCompositionError::Empty);
        }
        let mut total = 0.0;
        for (k, &(id, x)) in mole_fractions.iter().enumerate() {
            if x.is_nan() || x <= 0.0 || x > 1.0 {
                return Err(BuildLiquidCompositionError::FractionOutOfRange { component: id });
            }
            if mole_fractions[..k].iter().any(|&(other, _)| other == id) {
                return Err(BuildLiquidCompositionError::Repeated { component: id });
            }
            total += x;
        }
        if (total - 1.0).abs() > 1e-9 {
            return Err(BuildLiquidCompositionError::NotWhole { total });
        }
        Ok(Self { mole_fractions })
    }

    /// The components and their mole fractions, in the given order.
    #[must_use]
    pub const fn mole_fractions(&self) -> &'a [(SubstanceId, f64)] {
        self.mole_fractions
    }
}

/// A liquid's composition could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildLiquidCompositionError {
    /// It names no component.
    Empty,
    /// A component's mole fraction is not in (0, 1].
    FractionOutOfRange {
        /// The component.
        component: SubstanceId,
    },
    /// A component is named twice.
    Repeated {
        /// The component.
        component: SubstanceId,
    },
    /// The fractions do not sum to 1 within 10⁻⁹.
    NotWhole {
        /// Their sum.
        total: f64,
    },
}

impl fmt::Display for BuildLiquidCompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a liquid's composition names no component"),
            Self::FractionOutOfRange { component } => write!(
                f,
                "{}'s mole fraction lies outside (0, 1]",
                component.substance().key()
            ),
            Self::Repeated { component } => {
                write!(f, "{} is named twice", component.substance().key())
            }
            Self::NotWhole { total } => write!(f, "the mole fractions sum to {total}, not 1"),
        }
    }
}

impl Error for BuildLiquidCompositionError {}

/// Which composition a liquid's record is resolved at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Composition<'a> {
    /// The row's own: a pure liquid's, or a solution's stated reference composition
    /// (decision-r11-palette-liquids §1.7; decision-p14-t49b-condensed §1.4 and §2.6).
    Reference,
    /// A given one, by mole fraction, as P14.T51.a's condensate entry will carry it.
    Given(LiquidComposition<'a>),
}

/// A required member of the record that P14.T49.h has not yet modelled for a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LiquidMember {
    /// The real refractive index.
    RefractiveIndex,
    /// The density.
    Density,
}

impl fmt::Display for LiquidMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RefractiveIndex => "refractive index",
            Self::Density => "density",
        })
    }
}

/// A liquid's record could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolveLiquidError {
    /// The row has no liquid columns: the registry does not model it as a surface liquid.
    NotALiquid {
        /// The row.
        key: SubstanceKey,
    },
    /// A member the record requires is not yet modelled for the row.
    NotModelled {
        /// The row.
        key: SubstanceKey,
        /// The member.
        member: LiquidMember,
    },
    /// The temperature lies outside the row's modelled liquid range, from its first melt to its
    /// critical point: P14.T54.a clamps a record's temperature into it first
    /// (decision-r11-palette-liquids §1.7; decision-p14-t49b-condensed §2).
    OutOfRange {
        /// The row.
        key: SubstanceKey,
        /// The temperature asked for.
        temperature: Kelvin,
        /// The range's lowest temperature, the row's first melt.
        lowest: Kelvin,
        /// The temperature the range ends below, its critical point, or `None` where the row
        /// states none.
        below: Option<Kelvin>,
    },
    /// A composition was given, and solutions' mixing rules are not yet modelled.
    CompositionNotModelled {
        /// The row.
        key: SubstanceKey,
    },
}

impl fmt::Display for ResolveLiquidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotALiquid { key } => write!(f, "{key} is not modelled as a liquid"),
            Self::NotModelled { key, member } => {
                write!(f, "{key}'s liquid {member} is not modelled")
            }
            Self::OutOfRange {
                key, temperature, ..
            } => write!(
                f,
                "{} K lies outside {key}'s liquid range",
                temperature.value()
            ),
            Self::CompositionNotModelled { key } => {
                write!(f, "{key}'s liquid at a given composition is not modelled")
            }
        }
    }
}

impl Error for ResolveLiquidError {}

/// The derivative in T of an ancillary density equation's ρ, kg m⁻³ K⁻¹: dρ ÷ dT = −(`ρ_r` ÷ `T_r`)
/// Σ bᵢ tᵢ θ^(tᵢ − 1) for the linear form and −(ρ ÷ `T_r`) Σ bᵢ tᵢ θ^(tᵢ − 1) for the logarithmic,
/// θ = 1 − T ÷ `T_r`.
#[must_use]
fn density_slope(equation: &DensityEquation, temperature: Kelvin) -> f64 {
    let theta = 1.0 - temperature.value() / equation.reducing_temperature.value();
    let mut sum = 0.0;
    for &(b, exponent) in equation.terms {
        sum += b * exponent * math::powf(theta, exponent - 1.0);
    }
    let scale = match equation.form {
        DensityForm::Linear => equation.reducing_density.value(),
        DensityForm::Logarithmic => equation.density(temperature).value(),
    };
    -scale * sum / equation.reducing_temperature.value()
}

/// A diagram row's liquid density at `temperature`, kg m⁻³, extended below its triple point: its
/// ancillary equation from the triple point up, and below it a line at the equation's slope there
/// (constant expansivity), never the equation itself out of its range
/// (decision-p14-t49b-condensed §2.5, which ethane's ancillary, turning over below its triple
/// point, makes a rule). The second value is whether the line was taken.
#[must_use]
fn extended_liquid_density(
    diagram: &PhaseDiagram,
    temperature: Kelvin,
) -> (KilogramsPerCubicMetre, bool) {
    let triple = diagram.triple_temperature();
    if temperature >= triple {
        (
            diagram.liquid_density_equation().density(temperature),
            false,
        )
    } else {
        let equation = diagram.liquid_density_equation();
        let at_triple = equation.density(triple).value();
        let slope = density_slope(equation, triple);
        (
            KilogramsPerCubicMetre::new(at_triple + slope * (temperature.value() - triple.value())),
            true,
        )
    }
}

/// The row's liquid density at `temperature` with its basis, or `None` where it has none: a
/// diagram row's saturated liquid inside [triple, critical], `Measured`; a stated value at any
/// temperature, `Measured` at its own and `Derived` elsewhere.
#[must_use]
fn record_density(
    row: &Substance,
    temperature: Kelvin,
) -> Option<(KilogramsPerCubicMetre, LiquidBasis)> {
    match row.condensed()?.grain_density() {
        GrainDensity::OfDiagram => row
            .grain_density(CondensedPhase::Liquid, temperature)
            .map(|density| (density, LiquidBasis::Measured)),
        GrainDensity::Fixed { liquid, .. } => liquid.map(|given| {
            let exact = (given.at().value() - temperature.value()).abs() < 1e-9;
            let basis = if exact {
                LiquidBasis::Measured
            } else {
                LiquidBasis::Derived
            };
            (given.density(), basis)
        }),
    }
}

/// The row's liquid density at its optics file's state, kg m⁻³, and whether it is a stated
/// extension (a line below the triple point, or a stated value away from its temperature).
#[must_use]
fn state_density(row: &Substance, state: Kelvin) -> Option<(KilogramsPerCubicMetre, bool)> {
    match row.condensed()?.grain_density() {
        GrainDensity::OfDiagram => row
            .phase_diagram()
            .map(|diagram| extended_liquid_density(diagram, state)),
        GrainDensity::Fixed { liquid, .. } => liquid.map(|given| {
            (
                given.density(),
                (given.at().value() - state.value()).abs() >= 1e-9,
            )
        }),
    }
}

/// The Lorentz–Lorenz function (n² − 1) ÷ (n² + 2).
#[must_use]
fn lorentz_lorenz(n: f64) -> f64 {
    let n2 = n * n;
    (n2 - 1.0) / (n2 + 2.0)
}

/// The index whose Lorentz–Lorenz function is `l`: √[(1 + 2l) ÷ (1 − l)].
#[must_use]
fn from_lorentz_lorenz(l: f64) -> f64 {
    ((1.0 + 2.0 * l) / (1.0 - l)).sqrt()
}

/// `index`, a liquid's real index by bin at density `from`, carried to density `to` at constant
/// specific refraction: (n² − 1) ÷ (n² + 2) times `to` ÷ `from`, each bin alone.
#[must_use]
pub(crate) fn carry_index(
    index: &[f64; LIQUID_BINS],
    from: KilogramsPerCubicMetre,
    to: KilogramsPerCubicMetre,
) -> [f64; LIQUID_BINS] {
    let ratio = to.value() / from.value();
    index.map(|n| from_lorentz_lorenz(lorentz_lorenz(n) * ratio))
}

/// ρ (∂n² ÷ ∂ρ)_T by the PMH model (Proutière, Megnassan and Hucteau 1992, J. Phys. Chem. 96,
/// 3485, doi:10.1021/j100187a058, not read), as Zhang, Hu and He 2009's published code writes it:
/// (n² − 1) [1 + ⅔ (n² + 2) (n ÷ 3 − 1 ÷ 3n)²].
#[must_use]
fn density_derivative_of_index_squared(n: f64) -> f64 {
    let n2 = n * n;
    let bracket = n / 3.0 - 1.0 / (3.0 * n);
    (n2 - 1.0) * (1.0 + 2.0 / 3.0 * (n2 + 2.0) * bracket * bracket)
}

/// The uniform mean of λ⁻⁴ over the bin [λ₁, λ₂], m⁻⁴: (λ₁⁻³ − λ₂⁻³) ÷ [3 (λ₂ − λ₁)].
#[must_use]
fn mean_inverse_fourth(low_nm: f64, high_nm: f64) -> f64 {
    let low = low_nm * 1e-9;
    let high = high_nm * 1e-9;
    (math::powi(low, -3) - math::powi(high, -3)) / (3.0 * (high - low))
}

/// A pure liquid's scattering coefficient b by bin, m⁻¹, over 4π, from its density fluctuations:
/// Einstein–Smoluchowski with Cabannes's factor, as Zhang, Hu and He 2009 (Opt. Express 17, 5698)
/// write it for water at zero salinity in their published code (`betasw_ZHH2009.m`, its density
/// term):
///
/// - β(90°) = (π² ÷ 2) λ⁻⁴ `k_B` T `κ_T` [ρ (∂n² ÷ ∂ρ)_T]² (6 + 6δ) ÷ (6 − 7δ);
/// - b = (8π ÷ 3) β(90°) (2 + δ) ÷ (1 + δ);
///
/// with ρ (∂n² ÷ ∂ρ)_T by the PMH model, `refractive_index` the bins' real index, `temperature`
/// in K, `isothermal_compressibility_per_pa` `κ_T` in Pa⁻¹ and `depolarisation_ratio` δ, each bin
/// the uniform mean of λ⁻⁴ over its edges ([`LIQUID_BIN_EDGES_NM`], vacuum wavelengths) times the
/// rest at the bin's index: the index's variation inside a bin moves b by under 5 × 10⁻⁴
/// relative (4.4 × 10⁻⁴ for water in the 380–405 nm bin).
/// The phase function is (1 + cos²θ (1 − δ) ÷ (1 + δ)), symmetric about 90°, so the
/// backscattering is b ÷ 2.
///
/// # Examples
///
/// A record's scattering is this, at the record's index and temperature, with `κ_T` from the row's
/// equation of state at the record's density and the row's depolarisation ratio:
///
/// ```
/// use hyperion_sim::substance::{
///     Composition, by_key, liquid_properties, pure_liquid_scattering_per_m,
/// };
/// use hyperion_sim::units::{Kelvin, KilogramsPerCubicMetre};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let water = by_key("H2O").ok_or("the registry holds water")?;
/// let t = Kelvin::new(288.15);
/// let sea = liquid_properties(water, t, Composition::Reference)?;
/// let inputs = water
///     .substance()
///     .liquid()
///     .and_then(|columns| columns.scattering())
///     .ok_or("water scatters")?;
/// let density = KilogramsPerCubicMetre::new(sea.density_kg_m3);
/// let kappa = inputs.equation().isothermal_compressibility_per_pa(density, t);
/// let delta = inputs.depolarisation_ratio();
/// let b = pure_liquid_scattering_per_m(&sea.refractive_index, t, kappa, delta);
/// assert_eq!(Some(b), sea.scattering_per_m);
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn pure_liquid_scattering_per_m(
    refractive_index: &[f64; LIQUID_BINS],
    temperature: Kelvin,
    isothermal_compressibility_per_pa: f64,
    depolarisation_ratio: f64,
) -> [f64; LIQUID_BINS] {
    let delta = depolarisation_ratio;
    let cabannes = (6.0 + 6.0 * delta) / (6.0 - 7.0 * delta);
    let total = 8.0 * std::f64::consts::PI / 3.0 * (2.0 + delta) / (1.0 + delta);
    let fluctuation = BOLTZMANN_CONSTANT * temperature.value() * isothermal_compressibility_per_pa;
    let pi2_half = std::f64::consts::PI * std::f64::consts::PI / 2.0;
    let mut b = [0.0; LIQUID_BINS];
    for (k, (value, &n)) in b.iter_mut().zip(refractive_index).enumerate() {
        let derivative = density_derivative_of_index_squared(n);
        let beta_90 = pi2_half
            * mean_inverse_fourth(LIQUID_BIN_EDGES_NM[k], LIQUID_BIN_EDGES_NM[k + 1])
            * fluctuation
            * derivative
            * derivative
            * cabannes;
        *value = total * beta_90;
    }
    b
}

/// The liquid's record for row `id` at `temperature` and `composition`
/// (decision-r11-palette-liquids §1.7 and §4), for P14.T54.a's palette.
///
/// - **The temperature** must lie in the row's liquid range, from its first melt (its
///   condensed columns' transition, decision-p14-t49b-condensed §2) to below its critical point;
///   P14.T54.a clamps a body's temperature into it first.
/// - **The index** is the row's R08 file's, carried by Lorentz–Lorenz to the record's density;
///   **the density** is the condensed columns'; both are required.
/// - **The absorption** is the file's where it is modelled; **the scattering** is
///   [`pure_liquid_scattering_per_m`] at the record's index, temperature and the equation of
///   state's `κ_T` at the record's density; **the surface tension and viscosity** are the row's
///   correlations at the record's state. Each is absent where the row has no source.
/// - **The basis** is the least direct of the members'.
///
/// # Errors
///
/// [`ResolveLiquidError::NotALiquid`] for a row with no liquid columns;
/// [`ResolveLiquidError::CompositionNotModelled`] for a [`Composition::Given`], until the
/// solutions' step; [`ResolveLiquidError::OutOfRange`] for a temperature outside the liquid range;
/// [`ResolveLiquidError::NotModelled`] where the row's index or density is not modelled.
///
/// # Examples
///
/// Water at 15 °C, every member measured:
///
/// ```
/// use hyperion_sim::substance::{Composition, by_key, liquid_properties};
/// use hyperion_sim::units::Kelvin;
/// use hyperion_surface::field::LiquidBasis;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let water = by_key("H2O").ok_or("the registry holds water")?;
/// let sea = liquid_properties(water, Kelvin::new(288.15), Composition::Reference)?;
/// assert_eq!(sea.basis, LiquidBasis::Measured);
/// // About 1.335 in the bin across 545 nm, and blue light scatters most.
/// assert!((sea.refractive_index[6] - 1.335).abs() < 0.001);
/// let b = sea.scattering_per_m.ok_or("water scatters")?;
/// assert!(b[0] > b[14]);
/// # Ok(())
/// # }
/// ```
pub fn liquid_properties(
    id: SubstanceId,
    temperature: Kelvin,
    composition: Composition<'_>,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let row = id.substance();
    let key = row.key();
    let columns = row.liquid().ok_or(ResolveLiquidError::NotALiquid { key })?;
    match composition {
        Composition::Reference => {}
        Composition::Given(_) => return Err(ResolveLiquidError::CompositionNotModelled { key }),
    }
    let condensed = row.condensed().ok_or(ResolveLiquidError::NotModelled {
        key,
        member: LiquidMember::Density,
    })?;
    let lowest = condensed.transition().temperature();
    let below = row.phase_diagram().map(PhaseDiagram::critical_temperature);
    let finite = temperature.value().is_finite();
    if !finite || temperature < lowest || below.is_some_and(|critical| temperature >= critical) {
        return Err(ResolveLiquidError::OutOfRange {
            key,
            temperature,
            lowest,
            below,
        });
    }
    let (density, density_basis) =
        record_density(row, temperature).ok_or(ResolveLiquidError::NotModelled {
            key,
            member: LiquidMember::Density,
        })?;
    let optics = columns.optics.ok_or(ResolveLiquidError::NotModelled {
        key,
        member: LiquidMember::RefractiveIndex,
    })?;
    let (state, extended) =
        state_density(row, optics.state).ok_or(ResolveLiquidError::NotModelled {
            key,
            member: LiquidMember::Density,
        })?;
    let refractive_index = carry_index(optics.index, state, density);
    let mut index_basis = optics.index_basis.max(density_basis);
    let unchecked = optics
        .index_checked
        .is_some_and(|(low, high)| temperature < low || temperature > high);
    if extended || unchecked {
        index_basis = index_basis.max(LiquidBasis::Derived);
    }
    let mut basis = index_basis;
    let absorption_unchecked = optics
        .absorption_checked
        .is_some_and(|(low, high)| temperature < low || temperature > high);
    let absorption_per_m = optics.absorption.map(|(absorption, absorption_basis)| {
        basis = basis.max(absorption_basis);
        if absorption_unchecked {
            basis = basis.max(LiquidBasis::Derived);
        }
        *absorption
    });
    // An equation of state gives a positive κ_T at every stable liquid state; at a state its
    // ancillary density puts inside the spinodal it would not, and the scattering is then not
    // modelled rather than negative.
    let scattering_per_m = columns.scattering.and_then(|scattering| {
        let kappa = scattering
            .equation
            .isothermal_compressibility_per_pa(density, temperature);
        (kappa.is_finite() && kappa > 0.0).then(|| {
            basis = basis.max(scattering.basis);
            pure_liquid_scattering_per_m(
                &refractive_index,
                temperature,
                kappa,
                scattering.depolarisation_ratio,
            )
        })
    });
    let surface_tension_n_per_m = columns.surface_tension.map(|equation| {
        basis = basis.max(equation.basis_at(temperature));
        equation.surface_tension_n_per_m(temperature)
    });
    let viscosity_pa_s = columns.viscosity.map(|correlation| {
        let (mu, viscosity_basis) = correlation.viscosity_pa_s(temperature, density);
        basis = basis.max(viscosity_basis);
        mu
    });
    Ok(LiquidProperties {
        temperature,
        density_kg_m3: density.value(),
        basis,
        refractive_index,
        absorption_per_m,
        scattering_per_m,
        surface_tension_n_per_m,
        viscosity_pa_s,
    })
}

#[cfg(test)]
mod tests;
