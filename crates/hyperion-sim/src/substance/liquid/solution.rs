//! Solutions' liquid records (P14.T49.h's step 3; decision-p14-t49b-condensed §1.4 and §2.5;
//! decision-p14-t49h-step3-composition): a solution's components, each a registry row that is a
//! neutral species, its reference composition by mole fraction, and the mixing model that makes its
//! record from its components' at the record's temperature. The models are crate-private: rows
//! build them, and nothing outside the crate does.
//!
//! - **An ideal mixture of pure liquids** ([`MixingModel::IdealLiquids`], Titan's seas): every
//!   component a row with liquid columns of its own, at any mole fractions.
//! - **An aqueous electrolyte** ([`MixingModel::AqueousElectrolyte`], the chloride and sulphate
//!   brines): water's own record with a salt's empirical terms, from pure water to the density
//!   model's highest mass fraction.
//! - **An aqueous acid** ([`MixingModel::AqueousAcid`], `H2SO4`): the whole liquid's empirical
//!   functions of its acid's mass fraction and the temperature, between two measured compositions'
//!   optics, from 25 to 85 wt%.
//! - **One stated composition** ([`MixingModel::ReferenceOnly`], ammonia–water): the components'
//!   records mixed at the row's stated density, evaluated only at the reference composition.
//!
//! A component below its own triple point but inside a solution's liquid range (Titan's seas hold
//! liquid down to 71.15 K, where methane's reference equation starts at 90.694 K and ethane's at
//! 90.368 K; a brine's eutectic lies below water's 273.16 K) takes decision-p14-t49b-condensed
//! §2.5's continuation, and never its ancillary or a correlation outside its stated range.
//!
//! Every sum over components runs over the model's own component list, in registry order, each
//! fraction read from the composition (0 where it is absent), whatever order the composition names
//! them in (decision-r11-palette-liquids §1.7; decision-p14-t49h-step3-composition §1.5).

use hyperion_surface::field::{LIQUID_BINS, LiquidBasis, LiquidProperties};

use super::super::diagram::{AVOGADRO_PER_MOL, PhaseDiagram};
use super::super::element::Element;
use super::super::{Substance, SubstanceId};
use super::{
    LIQUID_BIN_EDGES_NM, LiquidColumns, LiquidComposition, LiquidMember, ResolveLiquidError,
    carry_index, density_derivative_of_index_squared, depolarisation_from_structure,
    extended_liquid_density, from_lorentz_lorenz, lorentz_lorenz, mean_inverse_fourth, pure_record,
    record_density, state_density,
};
use crate::math;
use crate::units::consts::BOLTZMANN_CONSTANT;
use crate::units::{Kelvin, KilogramsPerCubicMetre};

/// The slack, dimensionless, a given mole fraction may lie outside its model's range by and still
/// be evaluated: [`LiquidComposition::new`]'s 10⁻⁹, the tolerance a composition's sum is held to,
/// so that a composition converted from the model's own reference by other arithmetic resolves
/// ([`SolutionColumns::mole_fraction_range`]).
pub(crate) const MOLE_FRACTION_SLACK: f64 = 1e-9;

/// A solution's liquid columns: its components, its reference composition, and the model that
/// mixes them (decision-p14-t49b-condensed §1.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SolutionColumns {
    /// The components a given composition may name, species rows in registry order: the order
    /// every mixing rule sums them in.
    pub(crate) components: &'static [SubstanceId],
    /// The reference composition by mole fraction, in the components' order, summing to 1 within
    /// `LiquidComposition::new`'s 10⁻⁹, as a test holds.
    pub(crate) reference: &'static [(SubstanceId, f64)],
    /// The model that makes the record from the components'.
    pub(crate) model: MixingModel,
}

impl SolutionColumns {
    /// The mole fractions of `component` the model evaluates, inclusive, for
    /// [`ResolveLiquidError::CompositionOutOfRange`]; a given fraction within
    /// [`MOLE_FRACTION_SLACK`] of them is evaluated too:
    ///
    /// - an ideal mixture's, every fraction;
    /// - an aqueous electrolyte's, the salt's from 0 to the density model's highest mass fraction
    ///   converted with the rows' molar masses, and water's from 1 less that to 1;
    /// - an aqueous acid's, the acid's from its lowest to its highest mass fraction converted so,
    ///   and water's their complements;
    /// - a model of one composition's, the reference's own fraction alone.
    ///
    /// # Errors
    ///
    /// [`ResolveLiquidError::NotModelled`] for the density where a component the range is
    /// converted with has no molar mass, which a registry test holds never happens.
    pub(crate) fn mole_fraction_range(
        &self,
        component: SubstanceId,
    ) -> Result<(f64, f64), ResolveLiquidError> {
        Ok(match self.model {
            MixingModel::IdealLiquids(_) => (0.0, 1.0),
            MixingModel::AqueousElectrolyte(model) => {
                let highest = model.highest_solute_mole_fraction()?;
                if component == model.solute {
                    (0.0, highest)
                } else {
                    (1.0 - highest, 1.0)
                }
            }
            MixingModel::AqueousAcid(model) => {
                let lowest =
                    solute_mole_fraction(model.solvent, model.solute, model.lowest_mass_fraction)?;
                let highest =
                    solute_mole_fraction(model.solvent, model.solute, model.highest_mass_fraction)?;
                if component == model.solute {
                    (lowest, highest)
                } else {
                    (1.0 - highest, 1.0 - lowest)
                }
            }
            MixingModel::ReferenceOnly => {
                let x = self
                    .reference
                    .iter()
                    .find(|&&(id, _)| id == component)
                    .map_or(0.0, |&(_, x)| x);
                (x, x)
            }
        })
    }
}

/// How a solution's record is made from its components' at its temperature
/// (decision-p14-t49b-condensed §1.4): the mixing rules take mole fractions xᵢ, and Beer's
/// additivity of absorption takes volume fractions at the record's temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MixingModel {
    /// An ideal mixture of pure liquids, each component a row with its own liquid columns, Mᵢ its
    /// molar mass and ρᵢ its liquid's density at the record's temperature, at any mole fractions:
    ///
    /// - **the density** by volume additivity, ρ = Σ xᵢ Mᵢ ÷ `V_m` with the molar volume `V_m` =
    ///   Σ xᵢ Vᵢ, Vᵢ = Mᵢ ÷ ρᵢ (decision-p14-t49b-condensed §1.2: the basis Titan's composition is
    ///   defined on);
    /// - **the index** by Lorentz–Lorenz at that molar volume, in each bin L = (n² − 1) ÷ (n² + 2)
    ///   = Σ xᵢ Rᵢ ÷ `V_m` with each component's molar refraction Rᵢ = Lᵢ Vᵢ, Lᵢ its own
    ///   Lorentz–Lorenz function at the temperature ([`mixed_lorentz_lorenz`]);
    /// - **the surface tension** by the Macleod–Sugden parachor in Weinaug and Katz's mixture form
    ///   with the vapour neglected ([`mixed_surface_tension`]);
    /// - **the viscosity** by Grunberg and Nissan 1949 (Nature 164, 799, doi:10.1038/164799b0), ln
    ///   μ = Σ xᵢ ln μᵢ + ½ ΣΣ xᵢ xⱼ Gᵢⱼ, the Gᵢⱼ the mixture's
    ///   ([`IdealLiquidMixing::grunberg_nissan`]);
    /// - **the absorption** by Beer's additivity in the volume fractions φᵢ = xᵢ Vᵢ ÷ `V_m`, where
    ///   every component present has one (decision-r11-palette-liquids §1.4);
    /// - **the scattering** by Einstein–Smoluchowski–Cabannes with the ideal mixture's `κ_T` and an
    ///   ideal-solution concentration term ([`mixture_scattering`]), where every component present
    ///   has a `κ_T`.
    IdealLiquids(&'static IdealLiquidMixing),
    /// An aqueous solution of one salt, water's record with the salt's empirical terms
    /// ([`AqueousElectrolyte`]), from pure water to the density model's highest mass fraction.
    AqueousElectrolyte(&'static AqueousElectrolyte),
    /// An aqueous acid whose every member is an empirical function of the acid's mass fraction
    /// and the temperature ([`AqueousAcid`]), between its lowest and highest mass fractions.
    AqueousAcid(&'static AqueousAcid),
    /// A mixture modelled at its reference composition alone, whose density is the row's stated
    /// value (its condensed columns' liquid, `Derived` away from the temperature it was given at):
    /// the index by Lorentz–Lorenz from the components' molar refractions and the surface tension
    /// by the parachor, both at that density's molar volume `V_m` = Σ xᵢ Mᵢ ÷ ρ; no viscosity,
    /// absorption or scattering. Any other composition is
    /// [`ResolveLiquidError::CompositionOutOfRange`] (`nh3_h2o`, whose composition laws, Croft,
    /// Lunine and Kargel 1988 and Kargel et al. 1991, are unread).
    ReferenceOnly,
}

/// An ideal mixture's own constants: Grunberg and Nissan's interaction parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IdealLiquidMixing {
    /// Grunberg and Nissan's interaction parameter Gᵢⱼ for each pair of the solution's
    /// components, dimensionless, each pair once (Gⱼᵢ = Gᵢⱼ and Gᵢᵢ = 0): G = 0 makes the pair's
    /// ln μ the mole-fraction mean of its components'. A test holds that every pair is listed.
    pub(crate) grunberg_nissan: &'static [(SubstanceId, SubstanceId, f64)],
}

impl IdealLiquidMixing {
    /// Gᵢⱼ for `first` and `second`, in either order: the table's, or 0 for a pair it does not
    /// list.
    #[must_use]
    fn interaction(&self, first: SubstanceId, second: SubstanceId) -> f64 {
        self.grunberg_nissan
            .iter()
            .find(|&&(a, b, _)| (a == first && b == second) || (a == second && b == first))
            .map_or(0.0, |&(_, _, g)| g)
    }
}

/// An aqueous solution of one salt (decision-r11-palette-liquids §1.8's brine row;
/// decision-p14-t49h-step3-composition §2.3 and §4), each member water's own record at the
/// temperature with the salt's empirical term, so that a composition of water alone is water's
/// record (its §5, item 4). With `w_w` and `w_s` the water's and the salt's mass fractions, `ρ_w`
/// water's density, t the temperature in °C:
///
/// - **the density** Laliberté and Cooper 2004's, ρ = 1 ÷ (`w_w` ÷ `ρ_w` + `w_s` `v_s`), with the
///   salt's apparent specific volume `v_s` ([`LaliberteCooperDensity`]) and the sim's own water,
///   its saturated-liquid ancillary, below 273.16 K decision-p14-t49b-condensed §2.5's line, in
///   place of their water equation; never scaled to a stated value (ruling §4);
/// - **the index** by additivity of specific refraction by mass, (n² − 1) ÷ (n² + 2) = ρ (`w_w`
///   `r_w` + `w_s` `r_s`), in each bin, `r_w` = `L_w` ÷ `ρ_w` from water's carried index at its
///   state and `r_s` the salt's
///   ([`specific_refraction_m3_per_kg`](Self::specific_refraction_m3_per_kg) with its
///   [`dispersion`](Self::dispersion));
/// - **the absorption** water's by Beer's additivity at its volume fraction `φ_w` = `w_w` ρ ÷
///   `ρ_w`, the salt transparent, where that is cited ([`SoluteAbsorption`]);
/// - **the viscosity** Laliberté 2007's, ln μ = `w_w` ln `μ_w` + `w_s` ln `μ_s`
///   ([`LaliberteViscosity`]), with the sim's own water viscosity, IAPWS R12-08's, in place of
///   theirs, and below water's triple point [`supercooled_water_viscosity_pa_s`];
/// - **no surface tension:** Dutcher, Wexler and Clegg 2010 (J. Phys. Chem. A 114, 12216), the
///   source decision-r11-palette-liquids §1.8 names, is closed, and a reduction of its authors'
///   E-AIM calculator would be an offline fit outside `hyperion-fit`;
/// - **no scattering:** the solution's `κ_T` is not sourced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AqueousElectrolyte {
    /// The solvent, water's row.
    pub(crate) solvent: SubstanceId,
    /// The salt's row.
    pub(crate) solute: SubstanceId,
    /// The density model.
    pub(crate) density: LaliberteCooperDensity,
    /// The viscosity model.
    pub(crate) viscosity: LaliberteViscosity,
    /// The salt's specific refraction at 589.3 nm, m³ kg⁻¹, relative to vacuum: (n² − 1) ÷ (n² +
    /// 2) per unit density of the salt's share, from a measured solution index.
    pub(crate) specific_refraction_m3_per_kg: f64,
    /// The salt's specific refraction's dispersion, a stand-in: (wavelength in vacuum, nm; its
    /// factor on [`specific_refraction_m3_per_kg`](Self::specific_refraction_m3_per_kg)) in
    /// increasing wavelength, linear between them, each bin the uniform mean over its edges.
    pub(crate) dispersion: &'static [(f64, f64)],
    /// What the salt adds to the absorption.
    pub(crate) absorption: SoluteAbsorption,
}

impl AqueousElectrolyte {
    /// The salt's highest mole fraction, the density model's highest mass fraction converted with
    /// the rows' molar masses: x = (w ÷ `M_s`) ÷ (w ÷ `M_s` + (1 − w) ÷ `M_w`).
    ///
    /// # Errors
    ///
    /// [`ResolveLiquidError::NotModelled`] for the density where a row has no molar mass.
    fn highest_solute_mole_fraction(&self) -> Result<f64, ResolveLiquidError> {
        solute_mole_fraction(
            self.solvent,
            self.solute,
            self.density.highest_mass_fraction,
        )
    }
}

/// The mole fraction of `solute` in a binary solution with `solvent` at the solute's mass
/// fraction `w`, with the rows' molar masses: x = (w ÷ `M_s`) ÷ (w ÷ `M_s` + (1 − w) ÷ `M_w`).
///
/// # Errors
///
/// [`ResolveLiquidError::NotModelled`] for the density where a row has no molar mass.
fn solute_mole_fraction(
    solvent: SubstanceId,
    solute: SubstanceId,
    w: f64,
) -> Result<f64, ResolveLiquidError> {
    let salt = w / molar_mass_kg_per_mol(solute)?;
    let water = (1.0 - w) / molar_mass_kg_per_mol(solvent)?;
    Ok(salt / (salt + water))
}

/// Laliberté and Cooper 2004's density of an aqueous solution of one salt (J. Chem. Eng. Data 49,
/// 1141, doi:10.1021/je0498659; the paper closed, its form and coefficients read in its supporting
/// information): the salt's apparent specific volume `v_s` = (w + c₂ + c₃ t) ÷ [(c₀ w + c₁)
/// exp(10⁻⁶ (t + c₄)²)], m³ kg⁻¹, w the salt's mass fraction and t the temperature in °C, as the
/// workbooks print it ("App vol"), the reciprocal of their apparent density.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LaliberteCooperDensity {
    /// c₀ to c₄: c₀ and c₁ kg m⁻³, c₂ dimensionless, c₃ °C⁻¹, c₄ °C.
    pub(crate) coefficients: [f64; 5],
    /// The temperatures the fit's data span; the density is `Derived` outside them.
    pub(crate) valid_from: Kelvin,
    /// See [`valid_from`](Self::valid_from).
    pub(crate) valid_to: Kelvin,
    /// The highest salt mass fraction among the fit's data, dimensionless: the composition range's
    /// upper end.
    pub(crate) highest_mass_fraction: f64,
}

impl LaliberteCooperDensity {
    /// The salt's apparent specific volume at `celsius` °C and salt mass fraction `salt`, m³
    /// kg⁻¹; negative at low concentration for a salt that contracts the water (`MgSO4`).
    #[must_use]
    fn apparent_volume_m3_per_kg(&self, celsius: f64, salt: f64) -> f64 {
        let [c0, c1, c2, c3, c4] = self.coefficients;
        let shifted = celsius + c4;
        (salt + c2 + c3 * celsius) / ((c0 * salt + c1) * math::exp(1e-6 * shifted * shifted))
    }

    /// The basis at `temperature`: `Measured` inside the fit's data, `Derived` outside.
    #[must_use]
    fn basis_at(&self, temperature: Kelvin) -> LiquidBasis {
        if temperature >= self.valid_from && temperature <= self.valid_to {
            LiquidBasis::Measured
        } else {
            LiquidBasis::Derived
        }
    }
}

/// Laliberté 2007's viscosity of an aqueous solution of one salt (J. Chem. Eng. Data 52, 321,
/// doi:10.1021/je0604075; the paper closed, its form and coefficients read in its supporting
/// information): ln μ = `w_w` ln `μ_w` + `w_s` ln `μ_s`, the salt's term `μ_s` = exp[(v₁ w^v₂ + v₃)
/// ÷ (v₄ t + 1)] ÷ (v₅ w^v₆ + 1), mPa s, w the salt's mass fraction and t the temperature in °C
/// ("Mixing rule: mu=Product of mui^wi"; "App visc model", the workbooks' 'Viscosity' sheet).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LaliberteViscosity {
    /// v₁ to v₆: v₄ °C⁻¹, the rest dimensionless.
    pub(crate) coefficients: [f64; 6],
    /// The temperatures the fit's data span; the viscosity is `Derived` outside them.
    pub(crate) valid_from: Kelvin,
    /// See [`valid_from`](Self::valid_from).
    pub(crate) valid_to: Kelvin,
    /// The highest salt mass fraction among the fit's data, dimensionless; `Derived` above it.
    pub(crate) highest_mass_fraction: f64,
}

impl LaliberteViscosity {
    /// The salt's term `μ_s` at `celsius` °C and salt mass fraction `salt`, mPa s.
    #[must_use]
    fn solute_viscosity_mpa_s(&self, celsius: f64, salt: f64) -> f64 {
        let [v1, v2, v3, v4, v5, v6] = self.coefficients;
        math::exp((v1 * math::powf(salt, v2) + v3) / (v4 * celsius + 1.0))
            / (v5 * math::powf(salt, v6) + 1.0)
    }

    /// The basis at `temperature` and salt mass fraction `salt`: `Measured` inside the fit's
    /// data, `Derived` outside.
    #[must_use]
    fn basis_at(&self, temperature: Kelvin, salt: f64) -> LiquidBasis {
        if temperature >= self.valid_from
            && temperature <= self.valid_to
            && salt <= self.highest_mass_fraction
        {
            LiquidBasis::Measured
        } else {
            LiquidBasis::Derived
        }
    }
}

/// What a dissolved salt adds to its solution's visible absorption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SoluteAbsorption {
    /// Nothing within measurement: a cited measurement finds the salt transparent across the
    /// bins, so the solution's absorption is its water's by volume fraction.
    Transparent,
    /// No measurement was read: the solution's absorption is absent.
    NotModelled,
}

/// Water's viscosity below its triple point, Pa s, for an aqueous solution whose range reaches
/// there, where water's own correlation, IAPWS R12-08, is not evaluated
/// (decision-p14-t49b-condensed §2.5) and water's row carries no supercooled form: Laliberté's
/// water equation, μ(t) = (t + 246) ÷ [(0.055 94 t + 5.2842) t + 137.37] mPa s with t in °C (as
/// `thermo`'s `Laliberte_viscosity_w` writes it, citing Laliberté 2009, J. Chem. Eng. Data 54,
/// 1725; CalebBell/thermo at 2bb466e9, `thermo/electrochem.py`, SHA-256 632ed681…; the paper
/// unread; within 3.4 × 10⁻⁵ of Laliberté 2007's workbooks' water column), scaled by R12-08's value
/// over its own at the triple point, so that it is continuous there: μ(T) = `μ_R12`(`T_t`) `μ_L`(T)
/// ÷ `μ_L`(`T_t`). The scale is 1.000 67. It extrapolates Laliberté's equation, fitted above 0 °C,
/// into supercooled water, unchecked (brines.json's finding 5), and is `Derived`. `None` at and
/// above the triple point, and where `solvent` has no diagram or correlation.
#[must_use]
fn supercooled_water_viscosity_pa_s(solvent: SubstanceId, temperature: Kelvin) -> Option<f64> {
    let laliberte = |kelvin: f64| {
        let t = kelvin - 273.15;
        (t + 246.0) / ((0.055_94 * t + 5.2842) * t + 137.37)
    };
    let row = solvent.substance();
    let diagram = row.phase_diagram()?;
    let triple = diagram.triple_temperature();
    if temperature >= triple {
        return None;
    }
    let at_triple = diagram.liquid_density_equation().density(triple);
    row.liquid()?.viscosity.map(|correlation| {
        let (anchor, _) = correlation.viscosity_pa_s(triple, at_triple);
        anchor * laliberte(temperature.value()) / laliberte(triple.value())
    })
}

/// Aqueous sulphuric acid (decision-r11-palette-liquids §1.8's acid row;
/// decision-p14-t49h-step3-composition §1, item 4: `H2SO4`'s row is the molecule, and its liquid
/// columns are the aqueous acid's, its components water and itself): every member an empirical
/// function of the acid's mass fraction w and the temperature, from
/// [`lowest_mass_fraction`](Self::lowest_mass_fraction) to
/// [`highest_mass_fraction`](Self::highest_mass_fraction), with no component's own record read.
/// With the files' mass fractions `w₁` < `w₂` and s = (w − `w₁`) ÷ (`w₂` − `w₁`):
///
/// - **the density** [`MyhreDensity`]'s at (w, T);
/// - **the index** by specific refraction r = (n² − 1) ÷ [(n² + 2) ρ] linear in w between the two
///   files, each file's r its bins' at the same density function's value at the file's own state,
///   so that both files are reproduced at their states; then n at the record's ρ(w, T), in each
///   bin ([`acid_index`]): ideal additivity by mass of the two end solutions, which mixed give w
///   and r both linear in s, and Lorentz–Lorenz at constant r in temperature. Outside the files'
///   mass fractions it extrapolates in w;
/// - **the absorption** by Beer's additivity by mass of the two files' mass absorption
///   coefficients, α = ρ(w, T) [(1 − s) `α₁` ÷ `ρ₁` + s `α₂` ÷ `ρ₂`] for s in [0, 1], `ρᵢ` each
///   file's density at its state: both files' k are upper bounds, so a positive combination of
///   them is one; outside the files' fractions by more than [`MOLE_FRACTION_SLACK`], where an
///   extrapolation would weight one bound negatively, the larger of the two mass coefficients in
///   each bin, held ([`acid_absorption`]);
/// - **the surface tension** [`VehkamakiSurfaceTension`]'s, absent where it gives none;
/// - **the viscosity** [`WilliamsLongViscosity`]'s;
/// - **no scattering:** the acid's `κ_T` is not sourced (Valleau 1965, J. Acoust. Soc. Am. 38, 669,
///   doi:10.1121/1.1909778, its sound speeds and adiabatic compressibilities, is closed).
///
/// The record is `Derived`: the density's own form is a truncation (its doc).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AqueousAcid {
    /// The solvent, water's row.
    pub(crate) solvent: SubstanceId,
    /// The acid's row, `H2SO4`, whose liquid columns these are.
    pub(crate) solute: SubstanceId,
    /// The lowest acid mass fraction evaluated, dimensionless.
    pub(crate) lowest_mass_fraction: f64,
    /// The highest acid mass fraction evaluated, dimensionless.
    pub(crate) highest_mass_fraction: f64,
    /// The density.
    pub(crate) density: MyhreDensity,
    /// The two measured compositions' optics, in increasing mass fraction.
    pub(crate) files: [AcidOptics; 2],
    /// The surface tension.
    pub(crate) surface_tension: VehkamakiSurfaceTension,
    /// The viscosity.
    pub(crate) viscosity: WilliamsLongViscosity,
}

/// One measured composition's optics: R08's file reduced to the bins (`tables::liquid_optics`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AcidOptics {
    /// The acid's mass fraction, dimensionless.
    pub(crate) mass_fraction: f64,
    /// The real index by bin, at [`state`](Self::state).
    pub(crate) index: &'static [f64; LIQUID_BINS],
    /// The absorption coefficient by bin, m⁻¹, at its state: an upper bound in the visible (the
    /// shortest measured k, held to the blue).
    pub(crate) absorption: &'static [f64; LIQUID_BINS],
    /// The temperature the file was measured at.
    pub(crate) state: Kelvin,
}

/// Myhre, Nielsen and Saastad 1998's density of aqueous sulphuric acid (J. Chem. Eng. Data 43, 617,
/// doi:10.1021/je980013g, its Eq. 2; the paper closed) without its printed T⁴ column: ρ = Σᵢ Σⱼ
/// `kᵢⱼ` wⁱ tʲ kg m⁻³, i from 0 to 10 and j from 0 to 3, w the acid's mass fraction and t the
/// temperature in °C, 32 non-zero terms ([`coefficients`](Self::coefficients)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MyhreDensity {
    /// `kᵢⱼ`, row i the power of w and column j the power of t, kg m⁻³ °C⁻ʲ.
    pub(crate) coefficients: [[f64; 4]; 11],
}

impl MyhreDensity {
    /// The density at acid mass fraction `w` and `temperature`, kg m⁻³: each row's cubic in t by
    /// Horner's rule, then the rows' polynomial in w by Horner's rule, from the highest power down.
    #[must_use]
    fn density_kg_m3(&self, w: f64, temperature: Kelvin) -> f64 {
        let t = temperature.value() - 273.15;
        let mut density = 0.0;
        for row in self.coefficients.iter().rev() {
            let in_t = row[0] + t * (row[1] + t * (row[2] + t * row[3]));
            density = density * w + in_t;
        }
        density
    }
}

/// Vehkamäki et al. 2002's surface tension of aqueous sulphuric acid (J. Geophys. Res. 107 (D22),
/// 4622, doi:10.1029/2002JD002184, its Eq. 9): σ = a(w) + T b(w), N m⁻¹, a and b quintic in the
/// acid's mass fraction w and T in K, fitted over 233–323 K "for all sulfuric acid mass fractions
/// where the solution is liquid" to Sabinina and Terpugow 1935, Morgan and Davies 1916, Suggitt et
/// al. 1949, Hoffmann and Seeman 1960 and Myhre et al. 1998's data (all but Myhre's 0.291 series).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VehkamakiSurfaceTension {
    /// a's coefficients by power of w from 0, N m⁻¹.
    pub(crate) a: [f64; 6],
    /// b's coefficients by power of w from 0, N m⁻¹ K⁻¹.
    pub(crate) b: [f64; 6],
}

impl VehkamakiSurfaceTension {
    /// The surface tension at acid mass fraction `w` and `temperature`, N m⁻¹, each polynomial by
    /// Horner's rule from the highest power down.
    #[must_use]
    fn surface_tension_n_per_m(&self, w: f64, temperature: Kelvin) -> f64 {
        let horner = |c: &[f64; 6]| c.iter().rev().fold(0.0, |sum, &k| sum * w + k);
        horner(&self.a) + temperature.value() * horner(&self.b)
    }
}

/// Williams and Long 1995's viscosity of aqueous sulphuric acid (J. Phys. Chem. 99, 3748,
/// doi:10.1021/j100011a050; closed), as Shi, Jayne, Kolb, Worsnop and Davidovits 2001 (J. Geophys.
/// Res. 106 (D20), 24259; closed) reproduce it and three open model codes print it to every digit:
/// μ = A T^−1.43 exp[448 K ÷ (T − `T_0`)] mPa s, with A and `T_0` (K) cubic in W, the acid's mass
/// percentage; stated for 200–300 K and 30–80 wt%. W is held at the nearer end of that range
/// outside it ([`lowest_mass_percent`](Self::lowest_mass_percent) and
/// [`highest_mass_percent`](Self::highest_mass_percent)), as UKCA holds it at 80 above.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WilliamsLongViscosity {
    /// A's coefficients by power of W from 0, mPa s K^1.43.
    pub(crate) amplitude: [f64; 4],
    /// `T_0`'s coefficients by power of W from 0, K.
    pub(crate) divergence: [f64; 4],
    /// The power of T, dimensionless.
    pub(crate) exponent: f64,
    /// The activation temperature, K.
    pub(crate) activation_k: f64,
    /// The lowest mass percentage of the stated range.
    pub(crate) lowest_mass_percent: f64,
    /// The highest mass percentage of the stated range.
    pub(crate) highest_mass_percent: f64,
}

impl WilliamsLongViscosity {
    /// The viscosity at acid mass fraction `w` and `temperature`, Pa s, W = 100 w clamped to the
    /// stated range and each cubic by Horner's rule from the highest power down; it holds for
    /// `temperature` above `T_0`, which is under 175 K at every W of the range.
    #[must_use]
    fn viscosity_pa_s(&self, w: f64, temperature: Kelvin) -> f64 {
        let percent = (100.0 * w).clamp(self.lowest_mass_percent, self.highest_mass_percent);
        let horner = |c: &[f64; 4]| c.iter().rev().fold(0.0, |sum, &k| sum * percent + k);
        let t = temperature.value();
        horner(&self.amplitude)
            * math::powf(t, -self.exponent)
            * math::exp(self.activation_k / (t - horner(&self.divergence)))
            * 1e-3
    }
}

/// The acid's index in one bin at the record's density `density` and mass-fraction weight `s` =
/// (w − `w₁`) ÷ (`w₂` − `w₁`), from the two files' indices `low` and `high` there at their own
/// states' densities `low_density` and `high_density`: the specific refractions
/// r = (n² − 1) ÷ [(n² + 2) ρ] mixed as (1 − s) `r₁` + s `r₂`, then n at r ρ ([`AqueousAcid`]).
#[must_use]
fn acid_index(
    (low, low_density): (f64, KilogramsPerCubicMetre),
    (high, high_density): (f64, KilogramsPerCubicMetre),
    s: f64,
    density: KilogramsPerCubicMetre,
) -> f64 {
    let refraction = (1.0 - s) * lorentz_lorenz(low) / low_density.value()
        + s * lorentz_lorenz(high) / high_density.value();
    from_lorentz_lorenz(refraction * density.value())
}

/// Where a composition's acid mass fraction lies against the acid's two files, for
/// [`acid_absorption`].
#[derive(Debug, Clone, Copy, PartialEq)]
enum AcidSpan {
    /// Between the files' mass fractions, within [`MOLE_FRACTION_SLACK`]: the mass-fraction weight
    /// s = (w − `w₁`) ÷ (`w₂` − `w₁`), held to [0, 1].
    Between(f64),
    /// Outside them by more than the slack.
    Outside,
}

/// The acid's absorption coefficient in one bin, m⁻¹, at the record's density `density`, from
/// the two files' coefficients `low` and `high` (m⁻¹) at their own states' densities
/// `low_density` and `high_density`: by mass, (1 − s) `α₁` ÷ `ρ₁` + s `α₂` ÷ `ρ₂` at
/// [`AcidSpan::Between`]`(s)`, between the files; and the larger of the two at
/// [`AcidSpan::Outside`]; times ρ ([`AqueousAcid`]).
#[must_use]
fn acid_absorption(
    (low, low_density): (f64, KilogramsPerCubicMetre),
    (high, high_density): (f64, KilogramsPerCubicMetre),
    span: AcidSpan,
    density: KilogramsPerCubicMetre,
) -> f64 {
    let (per_kg_low, per_kg_high) = (low / low_density.value(), high / high_density.value());
    let per_kg = match span {
        AcidSpan::Between(s) => (1.0 - s) * per_kg_low + s * per_kg_high,
        AcidSpan::Outside => per_kg_low.max(per_kg_high),
    };
    per_kg * density.value()
}

/// A brine's solute that has no registry row yet (decision-p14-t49h-step3-composition §1.3): its
/// brine resolves only at its reference, from its own columns, and its `Given` is
/// [`ResolveLiquidError::CompositionNotModelled`] until the row is appended (§6), when the brine's
/// components gain it and this constant is held to the row's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RowlessSolute {
    /// The solvent, water's row, whose critical point the brine's range ends below (§3: the
    /// brines' salts have no phase diagram).
    pub(crate) solvent: SubstanceId,
    /// Its elements and their counts per formula unit.
    pub(crate) elements: &'static [(Element, u16)],
    /// Its molar mass, g mol⁻¹: its elements' IUPAC 2021 abridged standard atomic weights summed
    /// (Prohaska et al. 2022, Pure Appl. Chem. 94, 573), the registry's own arithmetic, to their
    /// last place.
    pub(crate) molar_mass_g_per_mol: f64,
}

/// A solution's component at a temperature, as the mixing rules read it: its own record's
/// members, or below its triple point decision-p14-t49b-condensed §2.5's continuation of them.
#[derive(Debug, Clone, Copy)]
struct ComponentState {
    /// The liquid's density.
    density: KilogramsPerCubicMetre,
    /// The real index by bin.
    refractive_index: [f64; LIQUID_BINS],
    /// The absorption coefficient by bin, m⁻¹, where modelled.
    absorption_per_m: Option<[f64; LIQUID_BINS]>,
    /// The scattering's inputs, where the row has an equation of state and its `κ_T` is positive.
    fluctuations: Option<Fluctuations>,
    /// The surface tension, N m⁻¹, where modelled.
    surface_tension_n_per_m: Option<f64>,
    /// The dynamic viscosity, Pa s, where modelled.
    viscosity_pa_s: Option<f64>,
    /// The least direct of the members' bases, its own critical-point rule for the scattering
    /// among them (its record's, [`super::LiquidScattering::basis_at`]).
    basis: LiquidBasis,
}

/// A component's inputs to its mixture's scattering.
#[derive(Debug, Clone, Copy)]
struct Fluctuations {
    /// The isothermal compressibility `κ_T`, Pa⁻¹, finite and positive, from the row's equation
    /// of state at the record's density.
    compressibility_per_pa: f64,
    /// The liquid's anisotropy ratio x, dimensionless.
    anisotropy_ratio: f64,
    /// The equation's specific gas constant, J kg⁻¹ K⁻¹, which its `S_0` = ρ R T `κ_T` takes.
    gas_constant_j_per_kg_k: f64,
}

/// The scattering's inputs from `columns`' equation of state at `density` and `temperature`,
/// where the row has one and its `κ_T` is finite and positive, as the record's scattering takes
/// it.
#[must_use]
fn fluctuations(
    columns: &LiquidColumns,
    density: KilogramsPerCubicMetre,
    temperature: Kelvin,
) -> Option<Fluctuations> {
    columns.scattering.and_then(|scattering| {
        let kappa = scattering
            .equation
            .isothermal_compressibility_per_pa(density, temperature);
        (kappa.is_finite() && kappa > 0.0).then_some(Fluctuations {
            compressibility_per_pa: kappa,
            anisotropy_ratio: scattering.anisotropy_ratio,
            gas_constant_j_per_kg_k: scattering.equation.gas_constant_j_per_kg_k(),
        })
    })
}

/// Row `id`'s molar mass, kg mol⁻¹.
///
/// # Errors
///
/// [`ResolveLiquidError::NotModelled`] for the density, keyed by the row, where it has none: a
/// component is a neutral formula-key species, which a registry test holds has one.
fn molar_mass_kg_per_mol(id: SubstanceId) -> Result<f64, ResolveLiquidError> {
    let row = id.substance();
    row.molar_mass_g_per_mol()
        .map(|grams| grams * 1e-3)
        .ok_or(ResolveLiquidError::NotModelled {
            key: row.key(),
            member: LiquidMember::Density,
        })
}

/// Row `id` as a solution's component at `temperature`: from its triple point up, its own record
/// ([`pure_record`]) with `κ_T` at the record's density; below it, [`supercooled_state`].
///
/// # Errors
///
/// The component's own refusals, keyed by the component: [`ResolveLiquidError::NotALiquid`] for a
/// row with no liquid columns, [`ResolveLiquidError::OutOfRange`] outside its range above the
/// triple point, and [`ResolveLiquidError::NotModelled`] for a missing index or density.
fn component_state(
    id: SubstanceId,
    temperature: Kelvin,
) -> Result<ComponentState, ResolveLiquidError> {
    let row = id.substance();
    let columns = row
        .liquid()
        .ok_or(ResolveLiquidError::NotALiquid { key: row.key() })?;
    match row.phase_diagram() {
        Some(diagram) if temperature < diagram.triple_temperature() => {
            supercooled_state(row, columns, diagram, temperature)
        }
        _ => {
            let record = pure_record(row, columns, temperature)?;
            let density = KilogramsPerCubicMetre::new(record.density_kg_m3);
            Ok(ComponentState {
                density,
                refractive_index: record.refractive_index,
                absorption_per_m: record.absorption_per_m,
                fluctuations: fluctuations(columns, density, temperature),
                surface_tension_n_per_m: record.surface_tension_n_per_m,
                viscosity_pa_s: record.viscosity_pa_s,
                basis: record.basis,
            })
        }
    }
}

/// A component below its triple point `T_t`, by decision-p14-t49b-condensed §2.5's rule, every
/// member `Derived`:
///
/// - the density linear in T from `T_t` at the ancillary's own slope there
///   ([`extended_liquid_density`], constant expansivity);
/// - the index the row's file's carried by Lorentz–Lorenz to that density ([`carry_index`]);
/// - `κ_T` held at its value at `T_t` and the ancillary's density there;
/// - the viscosity the row's fitted Vogel–Fulcher–Tammann form, anchored at its correlation's
///   value at `T_t` ([`super::SupercooledViscosity`]), absent where the row has no form;
/// - the surface tension the row's correlation's form at T, Mulero et al. 2012's being smooth
///   through the triple point, absent where it gives none;
/// - the absorption the row's file's, held.
///
/// Each is continuous at `T_t` with the row's own record there, as a test holds.
///
/// # Errors
///
/// [`ResolveLiquidError::NotModelled`], keyed by the component, where its index or its density at
/// its file's state is not modelled.
fn supercooled_state(
    row: &Substance,
    columns: &LiquidColumns,
    diagram: &PhaseDiagram,
    temperature: Kelvin,
) -> Result<ComponentState, ResolveLiquidError> {
    let key = row.key();
    let triple = diagram.triple_temperature();
    let at_triple = diagram.liquid_density_equation().density(triple);
    let (density, _) = extended_liquid_density(diagram, temperature);
    let optics = columns.optics.ok_or(ResolveLiquidError::NotModelled {
        key,
        member: LiquidMember::RefractiveIndex,
    })?;
    let (state, _) = state_density(row, optics.state).ok_or(ResolveLiquidError::NotModelled {
        key,
        member: LiquidMember::Density,
    })?;
    let mut basis = optics.index_basis.max(LiquidBasis::Derived);
    let absorption_per_m = optics.absorption.map(|(absorption, absorption_basis)| {
        basis = basis.max(absorption_basis);
        *absorption
    });
    let held = fluctuations(columns, at_triple, triple);
    if let (Some(scattering), Some(_)) = (columns.scattering, held) {
        basis = basis.max(scattering.basis);
    }
    let surface_tension_n_per_m = columns.surface_tension.and_then(|equation| {
        let sigma = equation.surface_tension_n_per_m(temperature);
        (sigma > 0.0).then(|| {
            basis = basis.max(equation.basis_at(temperature));
            sigma
        })
    });
    let viscosity_pa_s =
        columns
            .viscosity
            .zip(columns.supercooled_viscosity)
            .map(|(correlation, form)| {
                let (at_triple_pa_s, _) = correlation.viscosity_pa_s(triple, at_triple);
                form.viscosity_pa_s(at_triple_pa_s, temperature)
            });
    Ok(ComponentState {
        density,
        refractive_index: carry_index(optics.index, state, density),
        absorption_per_m,
        fluctuations: held,
        surface_tension_n_per_m,
        viscosity_pa_s,
        basis,
    })
}

/// A component the composition holds, as the mixing rules take it.
struct Part {
    /// The component's row.
    id: SubstanceId,
    /// Its mole fraction over the composition's sum.
    fraction: f64,
    /// Its molar mass, kg mol⁻¹.
    molar_mass_kg_per_mol: f64,
    /// Its molar volume at the record's state, Mᵢ ÷ ρᵢ, m³ mol⁻¹.
    molar_volume_m3_per_mol: f64,
    /// Its state at the record's temperature.
    state: ComponentState,
}

/// The solution `row`'s record at `temperature` and `composition`, which
/// [`super::liquid_properties`] has checked against the solution's components and its model's
/// range: its reference composition, or a given one.
///
/// Each component of the model's list is read at its fraction in `composition`, 0 where the
/// composition does not name it, in the list's order; a component at 0 is absent from the mixture.
///
/// # Errors
///
/// [`ResolveLiquidError::OutOfRange`] outside [the solution's first melt, the lowest critical
/// point among the components the composition holds); a component's own refusal where its index
/// or density is not modelled.
pub(super) fn solution_record(
    row: &Substance,
    solution: &SolutionColumns,
    composition: LiquidComposition<'_>,
    temperature: Kelvin,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let key = row.key();
    let mut present = Vec::with_capacity(solution.components.len());
    for &id in solution.components {
        let fraction = composition
            .mole_fractions()
            .iter()
            .find(|&&(named, _)| named == id)
            .map_or(0.0, |&(_, x)| x);
        if fraction > 0.0 {
            present.push((id, fraction));
        }
    }
    let condensed = row.condensed().ok_or(ResolveLiquidError::NotModelled {
        key,
        member: LiquidMember::Density,
    })?;
    let lowest = condensed.transition().temperature();
    let mut below: Option<Kelvin> = None;
    for &(id, _) in &present {
        if let Some(diagram) = id.substance().phase_diagram() {
            let critical = diagram.critical_temperature();
            below = Some(below.map_or(critical, |b| if critical < b { critical } else { b }));
        }
    }
    let finite = temperature.value().is_finite();
    if !finite || temperature < lowest || below.is_some_and(|critical| temperature >= critical) {
        return Err(ResolveLiquidError::OutOfRange {
            key,
            temperature,
            lowest,
            below,
        });
    }
    let mut total = 0.0;
    for &(_, x) in &present {
        total += x;
    }
    match solution.model {
        MixingModel::IdealLiquids(mixing) => {
            let parts = parts(&present, total, temperature)?;
            Ok(ideal_mixture(&parts, mixing, temperature))
        }
        MixingModel::AqueousElectrolyte(model) => {
            electrolyte_record(model, &present, total, temperature)
        }
        MixingModel::AqueousAcid(model) => acid_record(model, &present, total, temperature),
        MixingModel::ReferenceOnly => {
            let parts = parts(&present, total, temperature)?;
            reference_only_record(row, &parts, temperature)
        }
    }
}

/// The components `present`, each with its fraction over `total` and its state at `temperature`,
/// in their order.
///
/// # Errors
///
/// A component's own refusal ([`component_state`]), or [`ResolveLiquidError::NotModelled`] where
/// it has no molar mass.
fn parts(
    present: &[(SubstanceId, f64)],
    total: f64,
    temperature: Kelvin,
) -> Result<Vec<Part>, ResolveLiquidError> {
    let mut parts = Vec::with_capacity(present.len());
    for &(id, x) in present {
        let state = component_state(id, temperature)?;
        let molar_mass_kg_per_mol = molar_mass_kg_per_mol(id)?;
        parts.push(Part {
            id,
            fraction: x / total,
            molar_mass_kg_per_mol,
            molar_volume_m3_per_mol: molar_mass_kg_per_mol / state.density.value(),
            state,
        });
    }
    Ok(parts)
}

/// The mixture's Lorentz–Lorenz function in each bin, L = Σ xᵢ Lᵢ Vᵢ ÷ `molar_volume_m3_per_mol`
/// (Lorentz–Lorenz from the components' molar refractions Rᵢ = Lᵢ Vᵢ at the mixture's molar volume
/// `V_m`), summed in the parts' order.
#[must_use]
fn mixed_lorentz_lorenz(parts: &[Part], molar_volume_m3_per_mol: f64) -> [f64; LIQUID_BINS] {
    let mut lorentz = [0.0; LIQUID_BINS];
    for (k, l) in lorentz.iter_mut().enumerate() {
        let mut refraction = 0.0;
        for part in parts {
            refraction += part.fraction
                * lorentz_lorenz(part.state.refractive_index[k])
                * part.molar_volume_m3_per_mol;
        }
        *l = refraction / molar_volume_m3_per_mol;
    }
    lorentz
}

/// The mixture's surface tension, N m⁻¹, by the Macleod–Sugden parachor in Weinaug and Katz's
/// mixture form with the vapour neglected, σ^(1/4) = (ρ ÷ M) Σ xᵢ Pᵢ = Σ xᵢ Pᵢ ÷ `V_m`, with each
/// component's parachor Pᵢ = σᵢ^(1/4) Vᵢ from its own tension and molar volume at the temperature
/// (Macleod 1923, Trans. Faraday Soc. 19, 38, doi:10.1039/tf9231900038; Sugden 1924, J. Chem. Soc.,
/// Trans. 125, 1177, doi:10.1039/ct9242501177; Weinaug and Katz 1943, Ind. Eng. Chem. 35, 239,
/// doi:10.1021/ie50398a028), `molar_volume_m3_per_mol` the mixture's `V_m`; `None` where a part
/// has no tension.
#[must_use]
fn mixed_surface_tension(parts: &[Part], molar_volume_m3_per_mol: f64) -> Option<f64> {
    let mut parachors = Some(0.0);
    for part in parts {
        parachors = parachors
            .zip(part.state.surface_tension_n_per_m)
            .map(|(sum, sigma)| {
                sum + part.fraction * sigma.sqrt().sqrt() * part.molar_volume_m3_per_mol
            });
    }
    parachors.map(|sum| math::powi(sum / molar_volume_m3_per_mol, 4))
}

/// [`MixingModel::IdealLiquids`]' record of `parts` at `temperature`, each sum in the parts'
/// order; a member is absent where any part lacks it, and the basis is at least `Derived`.
#[must_use]
fn ideal_mixture(
    parts: &[Part],
    mixing: &IdealLiquidMixing,
    temperature: Kelvin,
) -> LiquidProperties {
    let mut molar_mass = 0.0;
    let mut molar_volume = 0.0;
    let mut basis = LiquidBasis::Derived;
    for part in parts {
        molar_mass += part.fraction * part.molar_mass_kg_per_mol;
        molar_volume += part.fraction * part.molar_volume_m3_per_mol;
        basis = basis.max(part.state.basis);
    }
    let lorentz = mixed_lorentz_lorenz(parts, molar_volume);
    let refractive_index = lorentz.map(from_lorentz_lorenz);
    let mut absorption = Some([0.0; LIQUID_BINS]);
    for part in parts {
        let volume_fraction = part.fraction * part.molar_volume_m3_per_mol / molar_volume;
        absorption = absorption
            .zip(part.state.absorption_per_m)
            .map(|(mut sum, own)| {
                for (total, a) in sum.iter_mut().zip(&own) {
                    *total += volume_fraction * a;
                }
                sum
            });
    }
    let surface_tension_n_per_m = mixed_surface_tension(parts, molar_volume);
    let mut ln_viscosity = Some(0.0);
    for part in parts {
        ln_viscosity = ln_viscosity
            .zip(part.state.viscosity_pa_s)
            .map(|(sum, mu)| sum + part.fraction * math::ln(mu));
    }
    let viscosity_pa_s = ln_viscosity.map(|mut sum| {
        for (i, first) in parts.iter().enumerate() {
            for second in &parts[i + 1..] {
                sum += first.fraction * second.fraction * mixing.interaction(first.id, second.id);
            }
        }
        math::exp(sum)
    });
    let scattering_per_m = mixture_scattering(
        parts,
        molar_mass,
        molar_volume,
        &lorentz,
        &refractive_index,
        temperature,
    )
    .map(|scattering| scattering.total);
    LiquidProperties {
        temperature,
        density_kg_m3: molar_mass / molar_volume,
        basis,
        refractive_index,
        absorption_per_m: absorption,
        scattering_per_m,
        surface_tension_n_per_m,
        viscosity_pa_s,
    }
}

/// An ideal mixture's scattering coefficient by bin, m⁻¹ over 4π, and its two parts.
#[derive(Debug, Clone, Copy)]
struct MixtureScattering {
    /// b, the sum of the two parts.
    total: [f64; LIQUID_BINS],
    /// The density fluctuations' part.
    density_term: [f64; LIQUID_BINS],
    /// The concentration fluctuations' part.
    concentration_term: [f64; LIQUID_BINS],
}

/// The scattering of an ideal mixture of `parts` (decision-r11-palette-liquids §1.8's Titan row:
/// "ES–Cabannes with ideal-mixing `κ_T`, plus an ideal-solution concentration term"), from the
/// mixture's `molar_mass_kg_per_mol`, `molar_volume_m3_per_mol` `V_m`, Lorentz–Lorenz function
/// `lorentz` and index `index` by bin at `temperature`; `None` where a part has no `κ_T`.
///
/// For natural light the isotropic part of the volume scattering function at 90° is (π² ÷ 2) λ⁻⁴
/// ⟨(δε)²⟩ δV (Einstein's form, as Zhang, Hu and He 2009, Opt. Express 17, 5698, write it in their
/// published code, `betasw_ZHH2009.m`, SHA-256 089a2d5d…), and the fluctuations of pressure at
/// fixed composition and of composition at fixed temperature and pressure are independent, the
/// temperature's term dropped as ZHH drop it, so it is the sum of two terms. In each bin, with ε =
/// n² the mixture's and ⟨λ⁻⁴⟩ the bin's uniform mean:
///
/// - **the density term**, the pure liquid's form ([`super::pure_liquid_scattering_per_m`]) with
///   the mixture's compressibility and index: `β_d` = (π² ÷ 2) ⟨λ⁻⁴⟩ `k_B` T `κ_T,mix` [ρ (∂n² ÷
///   ∂ρ)]², ρ (∂n² ÷ ∂ρ) PMH's at the mixture's n, and `κ_T,mix` = Σ φᵢ `κ_T,i`, the components' by
///   volume fraction φᵢ = xᵢ Vᵢ ÷ `V_m`, exact for an ideal solution, whose volume is additive at
///   every pressure (not their mole-fraction mean), each `κ_T,i` its own at its record's density,
///   held at its triple point's below it (decision-p14-t49b-condensed §2.5);
/// - **the concentration term**: an ideal solution's composition fluctuations in a portion of N
///   molecules are multinomial, ⟨δxᵢ δxⱼ⟩ = (xᵢ δᵢⱼ − xᵢ xⱼ) ÷ N (`k_B` T ÷ N times the inverse
///   Hessian of the ideal free energy of mixing on the simplex; Kirkwood and Goldberg 1950, J.
///   Chem. Phys. 18, 54, and Stockmayer 1950, J. Chem. Phys. 18, 58, not read), and the portion's
///   volume is N `V_m` ÷ `N_A`, so `β_c` = (π² ÷ 2) ⟨λ⁻⁴⟩ (`V_m` ÷ `N_A`) [Σ xᵢ εᵢ'² − (Σ xᵢ εᵢ')²]
///   with εᵢ' = N ∂ε ÷ ∂Nᵢ = [(ε + 2)² ÷ 3] (Vᵢ ÷ `V_m`) (Lᵢ − L), from Lorentz–Lorenz mixing at
///   ideal volume additivity; the covariance's null vector (1, …, 1) makes the constraint Σ xᵢ = 1
///   exact. It vanishes as any xᵢ → 1, and wherever every pure liquid has the same index at T,
///   whatever their densities; for a dilute binary it is ZHH's concentration term;
/// - **the depolarisation**, the pure liquid's anisotropy-ratio form
///   ([`super::LiquidScattering::depolarisation_ratio`]) with the mixture's ratio `X_mix` = Σ xᵢ
///   xᵢ' Rᵢ² ÷ (Σ xᵢ Rᵢ)² (xᵢ' each component's ratio, Rᵢ = Vᵢ Lᵢ: the anisotropy adds by molecules
///   and the mean polarisability by molar refraction, orientational pair correlations neglected as
///   for a pure liquid) and the isotropic structure factor `S_iso` = `S_0,mix` (1 + `β_c` ÷ `β_d`),
///   `S_0,mix` = ρ `R_mix` T `κ_T,mix`, `R_mix` = Σ wᵢ Rᵢ' the components' equations' specific gas
///   constants Rᵢ' by mass fraction wᵢ = xᵢ Mᵢ ÷ M, so that a mixture of one component takes its
///   own `S_0`; one δ and one Cabannes factor on both terms;
/// - b = (8π ÷ 3) (`β_d` + `β_c`) (6 + 6δ) ÷ (6 − 7δ) × (2 + δ) ÷ (1 + δ).
///
/// The derivation and its checks (εᵢ' against a finite difference, a binary against x₁ x₂ (∂ε ÷
/// ∂x₁)², the limits) are the lane's research, `.git/rm45-scratch/p14-liquids/step3/mixtures/`
/// (`scripts/mixtures.py`, SHA-256 7f0b8304…, whose docstring writes it out, and
/// `mixtures.json`). A mixture of one component is the pure liquid's scattering within round-off.
/// The concentration term is a lower bound for a liquid that deviates positively from Raoult's
/// law, as methane–nitrogen does.
#[must_use]
fn mixture_scattering(
    parts: &[Part],
    molar_mass_kg_per_mol: f64,
    molar_volume_m3_per_mol: f64,
    lorentz: &[f64; LIQUID_BINS],
    index: &[f64; LIQUID_BINS],
    temperature: Kelvin,
) -> Option<MixtureScattering> {
    let mut inputs = Vec::with_capacity(parts.len());
    let mut compressibility = 0.0;
    let mut gas_constant = 0.0;
    for part in parts {
        let own = part.state.fluctuations?;
        compressibility += part.fraction * part.molar_volume_m3_per_mol / molar_volume_m3_per_mol
            * own.compressibility_per_pa;
        gas_constant += part.fraction * part.molar_mass_kg_per_mol / molar_mass_kg_per_mol
            * own.gas_constant_j_per_kg_k;
        inputs.push(own);
    }
    let t = temperature.value();
    let density = molar_mass_kg_per_mol / molar_volume_m3_per_mol;
    let structure_factor = density * gas_constant * t * compressibility;
    let fluctuation = BOLTZMANN_CONSTANT * t * compressibility;
    let per_molecule = molar_volume_m3_per_mol / AVOGADRO_PER_MOL;
    let pi2_half = std::f64::consts::PI * std::f64::consts::PI / 2.0;
    let mut scattering = MixtureScattering {
        total: [0.0; LIQUID_BINS],
        density_term: [0.0; LIQUID_BINS],
        concentration_term: [0.0; LIQUID_BINS],
    };
    for (k, (&n, &mixed)) in index.iter().zip(lorentz).enumerate() {
        let inverse_fourth =
            mean_inverse_fourth(LIQUID_BIN_EDGES_NM[k], LIQUID_BIN_EDGES_NM[k + 1]);
        let derivative = density_derivative_of_index_squared(n);
        let beta_density = pi2_half * inverse_fourth * fluctuation * derivative * derivative;
        let epsilon = n * n;
        let field = (epsilon + 2.0) * (epsilon + 2.0) / 3.0;
        let (mut mean, mut square, mut anisotropy, mut refraction) = (0.0, 0.0, 0.0, 0.0);
        for (part, own) in parts.iter().zip(&inputs) {
            let own_lorentz = lorentz_lorenz(part.state.refractive_index[k]);
            let slope = field * part.molar_volume_m3_per_mol / molar_volume_m3_per_mol
                * (own_lorentz - mixed);
            mean += part.fraction * slope;
            square += part.fraction * slope * slope;
            let molar_refraction = part.molar_volume_m3_per_mol * own_lorentz;
            anisotropy +=
                part.fraction * own.anisotropy_ratio * molar_refraction * molar_refraction;
            refraction += part.fraction * molar_refraction;
        }
        let beta_concentration = pi2_half * inverse_fourth * per_molecule * (square - mean * mean);
        let ratio = anisotropy / (refraction * refraction);
        let isotropic = structure_factor * (1.0 + beta_concentration / beta_density);
        let delta = depolarisation_from_structure(ratio, isotropic);
        let cabannes = (6.0 + 6.0 * delta) / (6.0 - 7.0 * delta);
        let whole = 8.0 * std::f64::consts::PI / 3.0 * (2.0 + delta) / (1.0 + delta);
        scattering.density_term[k] = whole * beta_density * cabannes;
        scattering.concentration_term[k] = whole * beta_concentration * cabannes;
        scattering.total[k] = whole * (beta_density + beta_concentration) * cabannes;
    }
    Some(scattering)
}

/// [`MixingModel::AqueousElectrolyte`]'s record of the components `present`, each with its
/// fraction over `total`, at `temperature` ([`AqueousElectrolyte`]), every sum water's term first,
/// in registry order; `Derived` throughout.
///
/// # Errors
///
/// Water's own refusals ([`component_state`]), and [`ResolveLiquidError::NotModelled`] for the
/// density where a row has no molar mass.
fn electrolyte_record(
    model: &AqueousElectrolyte,
    present: &[(SubstanceId, f64)],
    total: f64,
    temperature: Kelvin,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let water = component_state(model.solvent, temperature)?;
    let fraction = |id: SubstanceId| {
        present
            .iter()
            .find(|&&(named, _)| named == id)
            .map_or(0.0, |&(_, x)| x / total)
    };
    let (water_moles, salt_moles) = (fraction(model.solvent), fraction(model.solute));
    let water_molar_mass = molar_mass_kg_per_mol(model.solvent)?;
    let salt_molar_mass = molar_mass_kg_per_mol(model.solute)?;
    let mass = water_moles * water_molar_mass + salt_moles * salt_molar_mass;
    let water_share = water_moles * water_molar_mass / mass;
    let salt_share = salt_moles * salt_molar_mass / mass;
    let celsius = temperature.value() - 273.15;
    let water_density = water.density.value();
    let density = 1.0
        / (water_share / water_density
            + salt_share * model.density.apparent_volume_m3_per_kg(celsius, salt_share));
    let mut basis = LiquidBasis::Derived
        .max(water.basis)
        .max(model.density.basis_at(temperature));
    let mut refractive_index = [0.0; LIQUID_BINS];
    for (k, n) in refractive_index.iter_mut().enumerate() {
        let water_refraction = lorentz_lorenz(water.refractive_index[k]) / water_density;
        let salt_refraction = model.specific_refraction_m3_per_kg
            * piecewise_linear_mean(
                model.dispersion,
                LIQUID_BIN_EDGES_NM[k],
                LIQUID_BIN_EDGES_NM[k + 1],
            );
        *n = from_lorentz_lorenz(
            density * (water_share * water_refraction + salt_share * salt_refraction),
        );
    }
    let water_volume = water_share * density / water_density;
    let absorption_per_m = match model.absorption {
        SoluteAbsorption::Transparent => water
            .absorption_per_m
            .map(|own| own.map(|a| water_volume * a)),
        SoluteAbsorption::NotModelled => None,
    };
    let water_viscosity = water
        .viscosity_pa_s
        .or_else(|| supercooled_water_viscosity_pa_s(model.solvent, temperature));
    let viscosity_pa_s = water_viscosity.map(|mu| {
        basis = basis.max(model.viscosity.basis_at(temperature, salt_share));
        let salt = model.viscosity.solute_viscosity_mpa_s(celsius, salt_share) * 1e-3;
        math::exp(water_share * math::ln(mu) + salt_share * math::ln(salt))
    });
    Ok(LiquidProperties {
        temperature,
        density_kg_m3: density,
        basis,
        refractive_index,
        absorption_per_m,
        scattering_per_m: None,
        surface_tension_n_per_m: None,
        viscosity_pa_s,
    })
}

/// [`MixingModel::AqueousAcid`]'s record of the components `present`, each with its fraction over
/// `total`, at `temperature` ([`AqueousAcid`]): the acid's mass fraction w from the rows' molar
/// masses, water's term first in registry order, and each member at (w, T); `Derived`.
///
/// # Errors
///
/// [`ResolveLiquidError::NotModelled`] for the density where a row has no molar mass.
fn acid_record(
    model: &AqueousAcid,
    present: &[(SubstanceId, f64)],
    total: f64,
    temperature: Kelvin,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let fraction = |id: SubstanceId| {
        present
            .iter()
            .find(|&&(named, _)| named == id)
            .map_or(0.0, |&(_, x)| x / total)
    };
    let water_mass = fraction(model.solvent) * molar_mass_kg_per_mol(model.solvent)?;
    let acid_mass = fraction(model.solute) * molar_mass_kg_per_mol(model.solute)?;
    let w = acid_mass / (water_mass + acid_mass);
    let density = KilogramsPerCubicMetre::new(model.density.density_kg_m3(w, temperature));
    let [low, high] = &model.files;
    let s = (w - low.mass_fraction) / (high.mass_fraction - low.mass_fraction);
    // A composition at a file's own fraction, given by mole fraction, lands within an ulp or two
    // of it: within the composition's slack it is between the files, so that both are reproduced.
    let span = if w >= low.mass_fraction - MOLE_FRACTION_SLACK
        && w <= high.mass_fraction + MOLE_FRACTION_SLACK
    {
        AcidSpan::Between(s.clamp(0.0, 1.0))
    } else {
        AcidSpan::Outside
    };
    let low_density =
        KilogramsPerCubicMetre::new(model.density.density_kg_m3(low.mass_fraction, low.state));
    let high_density =
        KilogramsPerCubicMetre::new(model.density.density_kg_m3(high.mass_fraction, high.state));
    let mut refractive_index = [0.0; LIQUID_BINS];
    let mut absorption_per_m = [0.0; LIQUID_BINS];
    for (k, (n, a)) in refractive_index
        .iter_mut()
        .zip(absorption_per_m.iter_mut())
        .enumerate()
    {
        *n = acid_index(
            (low.index[k], low_density),
            (high.index[k], high_density),
            s,
            density,
        );
        *a = acid_absorption(
            (low.absorption[k], low_density),
            (high.absorption[k], high_density),
            span,
            density,
        );
    }
    let sigma = model
        .surface_tension
        .surface_tension_n_per_m(w, temperature);
    Ok(LiquidProperties {
        temperature,
        density_kg_m3: density.value(),
        basis: LiquidBasis::Derived,
        refractive_index,
        absorption_per_m: Some(absorption_per_m),
        scattering_per_m: None,
        surface_tension_n_per_m: (sigma > 0.0).then_some(sigma),
        viscosity_pa_s: Some(model.viscosity.viscosity_pa_s(w, temperature)),
    })
}

/// The uniform mean over [`low_nm`, `high_nm`] of the function linear between `points` (nm,
/// value), which run in increasing wavelength and span the interval: the trapezoids between its
/// ends and every point inside it, exact for that function.
#[must_use]
fn piecewise_linear_mean(points: &[(f64, f64)], low_nm: f64, high_nm: f64) -> f64 {
    let at = |wavelength: f64| {
        let mut value = points.last().map_or(1.0, |&(_, v)| v);
        for pair in points.windows(2) {
            let ((a, fa), (b, fb)) = (pair[0], pair[1]);
            if wavelength <= b {
                value = fa + (fb - fa) * (wavelength - a) / (b - a);
                break;
            }
        }
        value
    };
    let (mut from, mut value, mut area) = (low_nm, at(low_nm), 0.0);
    for &(wavelength, v) in points {
        if wavelength > low_nm && wavelength < high_nm {
            area += f64::midpoint(value, v) * (wavelength - from);
            from = wavelength;
            value = v;
        }
    }
    area += f64::midpoint(value, at(high_nm)) * (high_nm - from);
    area / (high_nm - low_nm)
}

/// [`MixingModel::ReferenceOnly`]'s record of `parts` at `temperature`, the solution `row`'s
/// stated density's, each sum in the parts' order; `Derived`.
///
/// # Errors
///
/// [`ResolveLiquidError::NotModelled`] for the density where the row states none.
fn reference_only_record(
    row: &Substance,
    parts: &[Part],
    temperature: Kelvin,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let (density, density_basis) =
        record_density(row, temperature).ok_or(ResolveLiquidError::NotModelled {
            key: row.key(),
            member: LiquidMember::Density,
        })?;
    let mut molar_mass = 0.0;
    let mut basis = LiquidBasis::Derived.max(density_basis);
    for part in parts {
        molar_mass += part.fraction * part.molar_mass_kg_per_mol;
        basis = basis.max(part.state.basis);
    }
    let molar_volume = molar_mass / density.value();
    Ok(LiquidProperties {
        temperature,
        density_kg_m3: density.value(),
        basis,
        refractive_index: mixed_lorentz_lorenz(parts, molar_volume).map(from_lorentz_lorenz),
        absorption_per_m: None,
        scattering_per_m: None,
        surface_tension_n_per_m: mixed_surface_tension(parts, molar_volume),
        viscosity_pa_s: None,
    })
}

#[cfg(test)]
mod tests;
