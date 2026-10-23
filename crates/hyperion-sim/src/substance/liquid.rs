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
//!   water at zero salinity ([`pure_liquid_scattering_per_m`]), and `Derived` near a critical
//!   point, where the correlation length ([`CorrelationLength`]) passes
//!   [`CRITICAL_SCATTERING_LIMIT`];
//! - **the surface tension** is a reference correlation in Mulero, Cachadiña and Parra 2012's form
//!   ([`SurfaceTensionEquation`]), and **the viscosity** a reference correlation
//!   ([`ViscosityCorrelation`]);
//! - **the density** is the condensed columns' (P14.T49.b): the saturated liquid's ancillary
//!   equation for a row with a phase diagram, a stated value otherwise. A pure liquid with a
//!   reference equation of state is compressed to a pressure by it
//!   (`Substance::compressed_liquid_density`, P14.T49.n), which the record's readers take from
//!   generator version 24 (`COMPRESSED_LIQUIDS_VERSION`), its body's surface pressure; every
//!   other liquid is incompressible;
//! - **the specific heat** of the saturated liquid, `c_l`(T), J kg⁻¹ K⁻¹, is the row's reference
//!   equation of state's `c_p` at its own saturated liquid over a range, held at its ends
//!   ([`LiquidSpecificHeat`]; decision-r09-t13i §2.1, for the climate's sea of another liquid;
//!   decision-r09-t13i-cl-licence §6), for water, methane, ethane, nitrogen, ammonia and carbon
//!   dioxide, and a solution's its components' mass-weighted mean at its reference composition
//!   ([`Substance::liquid_specific_heat_j_per_kg_k`]). It is internal: the public mirror
//!   `substances.json` does not carry it.
//!
//! A solution's record is its components' instead ([`LiquidColumns::components`]): each component
//! a neutral species row with liquid columns of its own, mixed at the reference composition
//! ([`LiquidColumns::reference_composition`]) or a given one by the solution's crate-private mixing
//! model at the record's temperature (decision-p14-t49b-condensed §1.4;
//! decision-p14-t49h-step3-composition).
//!
//! A lithology's silicate melt's record is its major oxides' (decision-r11-palette-liquids §1.8's
//! "Molten silicate" row; decision-p14-t49h-step4-melts), by crate-private models of the melt at
//! the record's temperature and one bar: the viscosity by Giordano, Russell and Dingwell 2008, or,
//! for an alkali-poor, aluminium-rich melt, by Giordano and Russell 2007's CaO–MgO–Al₂O₃–SiO₂
//! model; the density by Lange and Carmichael's partial molar volumes; the index and absorption
//! R08's basaltic glass's, held; the surface tension a stated constant; no scattering. Its
//! composition is its oxides, which are not registry rows, so it has no components.
//!
//! A member with no source is absent from the record, never 0 (decision-r11-palette-liquids
//! §1.4), and the record's [`LiquidBasis`] is the least direct of its members' (§1.1).
//!
//! # What is built
//!
//! P14.T49.h's first step: the pure liquids with an R08 file (water, methane, ammonia, nitrogen and
//! ethane), with water's scattering, surface tension and viscosity. Its second: methane's,
//! nitrogen's, ethane's, ammonia's and carbon dioxide's equations of state, scattering, surface
//! tensions and viscosities, with methane's and ethane's viscosity continued below their triple
//! points ([`SupercooledViscosity`]). Its third, the solutions: a given composition
//! ([`Composition::Given`], decision-p14-t49h-step3-composition), decision-p14-t49b-condensed
//! §2.5's rule for a component below its triple point, and
//!
//! - Titan's seas (`ch4_c2h6_n2`), an ideal mixture of liquid methane, nitrogen and ethane, with
//!   its scattering from the mixture's density and concentration fluctuations;
//! - the chloride and sulphate brines (`brine_nacl`, `brine_mgso4`), water's record with
//!   Laliberté's empirical salt terms;
//! - the perchlorate brines (`brine_naclo4`, `brine_mgclo4`), whose solutes have no rows yet, at
//!   their stated densities with no index;
//! - ammonia–water (`nh3_h2o`) at its eutectic composition alone;
//! - aqueous sulphuric acid, `H2SO4`'s liquid, from 25 to 85 wt%: Myhre et al. 1998's density,
//!   its index and absorption between Palmer and Williams's 75 and 84.5 wt% files, Vehkamäki et
//!   al. 2002's surface tension and Williams and Long 1995's viscosity.
//!
//! Its fourth, the silicate melts of eleven lithologies (decision-p14-t49h-step4-melts; the rows
//! and their oxides are `rows/melts.rs`'s): `basalt` and `lava_glass` (Alae's tholeiite),
//! `basalt_fe`, `mars_dust` and `ferric` (Apollo 15 basalt 15597), `komatiite`, `basalt_reduced`
//! (Mercury's G2), `granite` (G-1), `ultramafic` (pyrolite), `anorthosite` and `feldspathic` (the
//! Apollo 16 soil). A melt takes Giordano and Russell 2007's viscosity where its oxides hold
//! `Al₂O₃` of at least 15 wt%, `Na₂O` + K₂O of at most 1 wt% and at most 3 wt% of the oxides that
//! model's mapping drops (all but `SiO₂`, `Al₂O₃`, `MgO`, `CaO`, `FeO`, `Fe₂O₃` and `MnO`), today
//! `anorthosite` and `feldspathic`; any other melt Giordano, Russell and Dingwell 2008's; a melt
//! that meets the first two conditions but not the third has no viscosity until a ruling (§2.3).
//! Every melt record is `StandIn`, from its solidus with no upper end. The lithologies with no melt
//! record (the Ca–Al residue, Fe–Ni metal, the phyllosilicate rock, the chondrites, the cometary
//! refractory) and the minerals that decompose or melt to a salt answer
//! [`ResolveLiquidError::NotALiquid`], each for its reason in `rows/melts.rs`. Molten sulphur
//! (`S8`, Io's flows) is left out: its liquid density waits for P14.T49.b's last piece.
//!
//! Its step 2b, the nine rows R08.T5.k gives liquid files (decision-p14-t49h-gas-index §1.2):
//! carbon dioxide's index joins the columns its second step built, and oxygen, argon, krypton,
//! xenon, nitrous oxide, carbon monoxide, sulphur dioxide and hydrogen sulphide take their files'
//! index, carried; oxygen its file's collision-induced absorption; all but sulphur dioxide, whose
//! depolarisation is unread, their reference equations of state's scattering, the atoms with their
//! interaction anisotropy alone; and Mulero et al.'s surface tensions. Their viscosity is absent,
//! none being ruled. Argon's, krypton's and xenon's measured indices keep their basis only over the
//! temperatures where Sinnock and Smith's warmer tables check the carry. Molten sulphur still waits
//! for P14.T49.b's last piece.
//!
//! Generator version 22 publishes none of it: nothing on the record reads these columns until
//! P14.T54.a resolves the palette (decision-bump-plan-after-22 §1.2).

use std::error::Error;
use std::fmt;

use hyperion_surface::field::{LIQUID_BINS, LiquidBasis, LiquidProperties};
use hyperion_surface::substance_key::{SubstanceKey, SubstanceKeyForm};

use super::condensed::{CondensedPhase, GrainDensity};
use super::diagram::{AVOGADRO_PER_MOL, DensityEquation, DensityForm, PhaseDiagram};
use super::helmholtz::{COMPRESSION_CEILING, HelmholtzIdeal, HelmholtzResidual};
use super::rows::liquids::LIQUIDS;
use super::{Source, Substance, SubstanceId};
use crate::math;
use crate::units::consts::BOLTZMANN_CONSTANT;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};
use crate::{GENERATOR_VERSION, GeneratorVersion};

mod melt;
mod solution;

pub(crate) use melt::{HeldOptics, Oxide, SilicateMelt};
pub(crate) use solution::{
    AcidOptics, AqueousAcid, AqueousElectrolyte, IdealLiquidMixing, LaliberteCooperDensity,
    LaliberteViscosity, MixingModel, MyhreDensity, RowlessSolute, SoluteAbsorption,
    SolutionColumns, VehkamakiSurfaceTension, WilliamsLongViscosity,
};

/// The generator version from which the record's liquids are compressed to their body's surface
/// pressure (P14.T49.n): 24, the 23 → 24 batch's bump, P14.T54.b
/// (decision-p14-t54a-water-consistency, its Batch B follow-up of 2026-10-20, F.1 and C;
/// decision-bump-plan-after-22, "Version-24 lanes after 23", §6's holds).
///
/// Below it [`CompressedLiquidsRules::in_force`] is [`CompressedLiquidsRules::Saturated`]: every
/// reader of a liquid's density on the record, the partition's layers, its seas' and its other
/// liquids' areas and its own-ocean weight (`planetary::chem::partition`), the palette's liquid
/// entry with its record (`planetary::surface::palette`), and the photometry's liquid record
/// (`planetary::derive::photometry::area_mix`), reads the saturated liquid's
/// ([`Substance::liquid_density_at_pressure`], [`liquid_properties_at_pressure`]), so that every
/// golden keeps 23's bits. The five move together, so that a liquid's layer, its area, its palette
/// entry and its Bond albedo's liquid read one `ρ_l`(T, p) (decision-p14-t54a-water-consistency's
/// W4). P14.T54.b deletes this hold, [`CompressedLiquidsRules`] with its `Saturated` arm,
/// [`with_compressed_liquids`] (and the switch in every test that sets it) and its tripwires
/// (`the_compressed_liquids_are_held_only_until_the_24_bump` and
/// `a_panic_inside_the_compressed_liquids_switch_sets_the_rules_back`).
pub(crate) const COMPRESSED_LIQUIDS_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether the record's liquids are compressed to their body's surface pressure (the hold,
/// [`COMPRESSED_LIQUIDS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CompressedLiquidsRules {
    /// Versions 23's and before: every reader takes the saturated liquid's density, under its own
    /// vapour's pressure, at its temperature.
    Saturated,
    /// The 23 → 24 batch's: every reader takes its liquid compressed to the body's surface
    /// pressure ([`Substance::compressed_liquid_density`]).
    Compressed,
}

impl CompressedLiquidsRules {
    /// The rules this generator version gives: the compression from
    /// [`COMPRESSED_LIQUIDS_VERSION`], the saturated liquid below it; in the tests, the rules a
    /// test's switch sets on its thread ([`with_compressed_liquids`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = COMPRESSED_LIQUIDS_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= COMPRESSED_LIQUIDS_VERSION {
            Self::Compressed
        } else {
            Self::Saturated
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The compressed liquids' rules a test sets on its thread, `None` for the version's.
    static COMPRESSED_LIQUIDS_RULES: core::cell::Cell<Option<CompressedLiquidsRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the compressed liquids' rules `rules` on this thread, for the tests to
/// reach 24's liquids ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. A record derives lazily, so a test reads it inside the switch. P14.T54.b deletes it
/// with the hold.
#[cfg(test)]
pub(crate) fn with_compressed_liquids<R>(
    rules: CompressedLiquidsRules,
    run: impl FnOnce() -> R,
) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<CompressedLiquidsRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            COMPRESSED_LIQUIDS_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(COMPRESSED_LIQUIDS_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

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
    /// its `temperatureK` label rounds to the kelvin for R08.T5.g's and T5.h's liquids and states
    /// to 0.01 K for T5.k's nine (decision-p14-t49h-gas-index §2.3).
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

/// A row's scattering inputs: its reference equation of state, for `κ_T`, its anisotropy ratio,
/// from which its depolarisation ratio follows at each state, and the amplitudes of its
/// correlation length near the critical point, for where Einstein–Smoluchowski–Cabannes holds.
///
/// **The depolarisation** (decision-p14-t49h-gas-index §3 and §4.2). In a liquid the scattering's
/// isotropic part, the density fluctuations', is suppressed by the structure factor's zero-angle
/// limit `S_0` = `ρ_N` `k_B` T `κ_T`, while its anisotropic part, the molecules' own anisotropy and
/// what their interactions induce, is not. With the anisotropy ratio x = `γ_eff`² ÷ ᾱ² and one
/// local-field factor on both parts, the vertically polarised light's ratio is `ρ_v` = (x ÷ 15) ÷
/// (`S_0` + 4x ÷ 45) and natural light's δ = `ρ_u` = 2`ρ_v` ÷ (1 + `ρ_v`), which ES–Cabannes takes.
/// So δ falls as `S_0` grows towards the critical point, and a fixed δ would misstate b across a
/// liquid's range. `S_0` here is ρ R T `κ_T` with the equation's own specific gas constant, 1 ÷ (1
/// + `2δφʳ_δ` + `δ²φʳ_δδ`) exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidScattering {
    pub(crate) equation: &'static HelmholtzResidual,
    pub(crate) anisotropy_ratio: f64,
    pub(crate) basis: LiquidBasis,
    pub(crate) fluctuations: CorrelationLength,
}

impl LiquidScattering {
    /// The reference equation of state the isothermal compressibility comes from.
    #[must_use]
    pub const fn equation(&self) -> &'static HelmholtzResidual {
        self.equation
    }

    /// The liquid's anisotropy ratio x = `γ_eff`² ÷ ᾱ², dimensionless, at least 0 and independent
    /// of the state.
    #[must_use]
    pub const fn anisotropy_ratio(&self) -> f64 {
        self.anisotropy_ratio
    }

    /// The structure factor's zero-angle limit `S_0` = ρ R T `κ_T` at `density` and `temperature`,
    /// dimensionless, from the equation of state.
    #[must_use]
    pub fn structure_factor(&self, density: KilogramsPerCubicMetre, temperature: Kelvin) -> f64 {
        density.value()
            * self.equation.gas_constant_j_per_kg_k()
            * temperature.value()
            * self
                .equation
                .isothermal_compressibility_per_pa(density, temperature)
    }

    /// The liquid's depolarisation ratio δ of natural light scattered at 90° at `density` and
    /// `temperature`, dimensionless: `ρ_u` = 2`ρ_v` ÷ (1 + `ρ_v`), `ρ_v` = (x ÷ 15) ÷ (`S_0` + 4x ÷
    /// 45).
    #[must_use]
    pub fn depolarisation_ratio(
        &self,
        density: KilogramsPerCubicMetre,
        temperature: Kelvin,
    ) -> f64 {
        depolarisation_from_structure(
            self.anisotropy_ratio,
            self.structure_factor(density, temperature),
        )
    }

    /// The basis of the equation of state and the ratio.
    #[must_use]
    pub const fn basis(&self) -> LiquidBasis {
        self.basis
    }

    /// The correlation length's amplitudes, for the critical-point rule
    /// ([`CRITICAL_SCATTERING_LIMIT`]).
    #[must_use]
    pub const fn fluctuations(&self) -> &CorrelationLength {
        &self.fluctuations
    }

    /// The scattering's basis at `density` and `temperature`, for a record whose index in the
    /// shortest bin is `shortest_bin_index`: [`basis`](Self::basis), and at least `Derived` where
    /// `q_max` ξ passes [`CRITICAL_SCATTERING_LIMIT`], `q_max` = 4π n ÷ 380 nm
    /// (decision-p14-t49h-liquids §7(c)).
    #[must_use]
    pub fn basis_at(
        &self,
        shortest_bin_index: f64,
        density: KilogramsPerCubicMetre,
        temperature: Kelvin,
    ) -> LiquidBasis {
        let xi = self
            .fluctuations
            .correlation_length_m(self.equation, density, temperature);
        let q_max =
            4.0 * std::f64::consts::PI * shortest_bin_index / (LIQUID_BIN_EDGES_NM[0] * 1e-9);
        if q_max * xi > CRITICAL_SCATTERING_LIMIT {
            self.basis.max(LiquidBasis::Derived)
        } else {
            self.basis
        }
    }
}

/// The depolarisation ratio δ of natural light scattered at 90° by a liquid of anisotropy ratio
/// `anisotropy_ratio` x and isotropic structure factor `structure_factor` S, dimensionless: `ρ_u` =
/// 2`ρ_v` ÷ (1 + `ρ_v`), `ρ_v` = (x ÷ 15) ÷ (S + 4x ÷ 45) ([`LiquidScattering`]; a mixture's S
/// counts its concentration fluctuations too).
#[must_use]
fn depolarisation_from_structure(anisotropy_ratio: f64, structure_factor: f64) -> f64 {
    let x = anisotropy_ratio;
    let vertical = (x / 15.0) / (structure_factor + 4.0 * x / 45.0);
    2.0 * vertical / (1.0 + vertical)
}

/// The limit on q ξ beyond which a liquid's Einstein–Smoluchowski–Cabannes scattering is
/// `Derived` (decision-p14-t49h-liquids §7(c)): ES–Cabannes takes the density fluctuations as
/// uncorrelated over a wavelength, which fails as the correlation length ξ grows near the critical
/// point; at q ξ = 0.1 the Ornstein–Zernike factor 1 ÷ (1 + q² ξ²), which it omits, is 0.99. The
/// scattering vector's largest magnitude is backscattering's in the shortest bin, `q_max` = 4π n ÷
/// 380 nm, with n the record's index in that bin.
pub const CRITICAL_SCATTERING_LIMIT: f64 = 0.1;

/// The amplitudes of a fluid's correlation length near its critical point, in the simplified
/// crossover form of Olchowy and Sengers 1989 that IAPWS R12-08 writes for water (its Eqs.
/// (20)–(21a)) and Huber, Perkins and Lemmon 2024 for nitrogen (their Eqs. 20–21): ξ = `ξ_0` (Δχ̃ ÷
/// `Γ_0`)^(ν ÷ γ), with Δχ̃ = χ̃(T, ρ) − χ̃(`T_R`, ρ) `T_R` ÷ T and χ̃ = (`p_c` ÷ `ρ_c`²) ρ (∂ρ ÷
/// ∂p)_T = `p_c` ρ² `κ_T` ÷ `ρ_c`², taken as 0 where Δχ̃ is negative, the compressibilities from
/// the row's equation of state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorrelationLength {
    pub(crate) amplitude_m: f64,
    pub(crate) susceptibility_amplitude: f64,
    pub(crate) nu: f64,
    pub(crate) gamma: f64,
    pub(crate) reference_temperature: Kelvin,
    pub(crate) critical_pressure: Pascals,
    pub(crate) critical_density: KilogramsPerCubicMetre,
}

impl CorrelationLength {
    /// The correlation length at `density` and `temperature`, m, from `equation`'s `κ_T` there and
    /// at the reference temperature; `equation` is the row's own, its scattering's.
    ///
    /// # Examples
    ///
    /// Nitrogen's within 0.02 K of its critical point, where fluctuations span nanometres, and at
    /// its normal boiling point, where they do not:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::{Kelvin, KilogramsPerCubicMetre};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let nitrogen = by_key("N2").ok_or("the registry holds nitrogen")?;
    /// let scattering = nitrogen
    ///     .substance()
    ///     .liquid()
    ///     .and_then(|columns| columns.scattering())
    ///     .ok_or("nitrogen scatters")?;
    /// let xi = |rho: f64, t: f64| {
    ///     scattering.fluctuations().correlation_length_m(
    ///         scattering.equation(),
    ///         KilogramsPerCubicMetre::new(rho),
    ///         Kelvin::new(t),
    ///     )
    /// };
    /// // Huber, Perkins and Lemmon 2024's Table 8: 13.96 nm at 126.212 K and 333 kg m⁻³.
    /// assert!((xi(333.0, 126.212) * 1e9 - 13.96).abs() < 0.01);
    /// assert!(xi(806.1, 77.355) < 1e-10);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn correlation_length_m(
        &self,
        equation: &HelmholtzResidual,
        density: KilogramsPerCubicMetre,
        temperature: Kelvin,
    ) -> f64 {
        let scale = self.critical_pressure.value() * density.value() * density.value()
            / (self.critical_density.value() * self.critical_density.value());
        let chi = scale * equation.isothermal_compressibility_per_pa(density, temperature);
        let reference =
            scale * equation.isothermal_compressibility_per_pa(density, self.reference_temperature);
        let difference = chi - reference * self.reference_temperature.value() / temperature.value();
        if difference > 0.0 {
            self.amplitude_m
                * math::powf(
                    difference / self.susceptibility_amplitude,
                    self.nu / self.gamma,
                )
        } else {
            0.0
        }
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

/// A reference correlation for a liquid's dynamic viscosity (decision-p14-t49h-liquids §6: the
/// newest reference correlation readable in full), each evaluated at the record's temperature and
/// density without its critical enhancement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViscosityCorrelation {
    /// IAPWS R12-08's formulation for water, μ = μ₀(T̄) μ₁(T̄, ρ̄), with the critical
    /// enhancement μ₂ set to 1, as its §2.8 allows outside the critical region.
    Iapws2008(&'static Iapws2008Viscosity),
    /// Huber, Perkins and Lemmon 2024's for nitrogen.
    Huber2024(&'static Huber2024Viscosity),
    /// Sotiriadou, Antoniadis, Assael, Martinek and Huber 2026's for methane.
    Sotiriadou2026(&'static Sotiriadou2026Viscosity),
    /// Laesecke and Muzny 2017's for carbon dioxide.
    LaeseckeMuzny2017(&'static LaeseckeMuzny2017Viscosity),
    /// Monogenidou, Assael and Huber 2018's for ammonia.
    Monogenidou2018(&'static Monogenidou2018Viscosity),
    /// Friend, Ingham and Ely 1991's for ethane.
    Friend1991(&'static Friend1991Viscosity),
}

/// A box about a fluid's critical point, in temperature and density, inside which a viscosity
/// correlation's omitted critical enhancement passes the bound its source states.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CriticalRegion {
    pub(crate) temperatures: (Kelvin, Kelvin),
    pub(crate) densities: (KilogramsPerCubicMetre, KilogramsPerCubicMetre),
}

impl CriticalRegion {
    /// The box from `t_low` to `t_high` and from `rho_low` to `rho_high`, open at its edges.
    #[must_use]
    pub(crate) const fn new(
        temperatures: (Kelvin, Kelvin),
        densities: (KilogramsPerCubicMetre, KilogramsPerCubicMetre),
    ) -> Self {
        Self {
            temperatures,
            densities,
        }
    }

    /// Whether (`temperature`, `density`) lies inside the box.
    #[must_use]
    pub fn contains(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> bool {
        temperature > self.temperatures.0
            && temperature < self.temperatures.1
            && density > self.densities.0
            && density < self.densities.1
    }
}

/// Where a viscosity correlation keeps its source's basis: its stated range of temperature, and
/// outside the box about the critical point where the enhancement it omits passes its stated
/// bound (`Measured` there, `Derived` elsewhere: an extrapolation, or a value missing its
/// enhancement).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViscosityRange {
    pub(crate) valid_from: Kelvin,
    pub(crate) valid_to: Kelvin,
    pub(crate) critical_region: Option<CriticalRegion>,
}

impl ViscosityRange {
    /// The basis at `temperature` and `density`.
    #[must_use]
    pub fn basis_at(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> LiquidBasis {
        let inside = temperature >= self.valid_from && temperature <= self.valid_to;
        let critical = self
            .critical_region
            .is_some_and(|region| region.contains(temperature, density));
        if inside && !critical {
            LiquidBasis::Measured
        } else {
            LiquidBasis::Derived
        }
    }

    /// The lowest temperature of the stated range.
    #[must_use]
    pub const fn valid_from(&self) -> Kelvin {
        self.valid_from
    }

    /// The highest temperature of the stated range.
    #[must_use]
    pub const fn valid_to(&self) -> Kelvin {
        self.valid_to
    }
}

/// The reduced second viscosity virial coefficient of Rainwater–Friend theory, B*_η(T*) = Σ bᵢ
/// T*^(−tᵢ), as Vogel, Küchenmeister, Bich and Laesecke 1998 (J. Phys. Chem. Ref. Data 27, 947)
/// fitted it and Sotiriadou et al. 2026 (their Eq. 5), Laesecke and Muzny 2017 (Eq. 6) and
/// Monogenidou et al. 2018 (Eq. 7 and Table 2) each print it, the terms in that order (Sotiriadou's
/// Table 4 prints b₃ with a trailing 1, 2471.012 51).
const RAINWATER_FRIEND: [(f64, f64); 9] = [
    (-19.572_881, 0.0),
    (219.739_99, 0.25),
    (-1_015.322_6, 0.5),
    (2_471.012_5, 0.75),
    (-3_375.171_7, 1.0),
    (2_491.659_7, 1.25),
    (-787.260_86, 1.5),
    (14.085_455, 2.5),
    (-0.346_641_58, 5.5),
];

/// [`RAINWATER_FRIEND`]'s B*_η at the reduced temperature `t_star` = `k_B` T ÷ ε.
#[must_use]
fn rainwater_friend_b_star(t_star: f64) -> f64 {
    let mut sum = 0.0;
    for &(b, exponent) in &RAINWATER_FRIEND {
        sum += b * math::powf(t_star, -exponent);
    }
    sum
}

/// The second viscosity virial coefficient `B_η` = B*_η `N_A` σ³ ÷ M, m³ kg⁻¹, at `temperature`,
/// for the scaling parameters σ (`sigma_m`, m) and ε ÷ `k_B` (`epsilon_k`, K) and the molar mass
/// `molar_mass_kg_per_mol`; the initial-density viscosity is η₀ `B_η` ρ.
#[must_use]
fn viscosity_virial_m3_per_kg(
    temperature: Kelvin,
    sigma_m: f64,
    epsilon_k: f64,
    molar_mass_kg_per_mol: f64,
) -> f64 {
    rainwater_friend_b_star(temperature.value() / epsilon_k)
        * AVOGADRO_PER_MOL
        * math::powi(sigma_m, 3)
        / molar_mass_kg_per_mol
}

/// The dilute gas's viscosity in Xiao et al. 2020's form, μPa s: η₀(T) = η₀(298.15 K) exp[Σᵢ aᵢ
/// (ln (T ÷ 298.15 K))ⁱ], i from 1, the terms in order.
#[must_use]
fn xiao_dilute_micro_pa_s(at_reference: f64, coefficients: &[f64], temperature: Kelvin) -> f64 {
    let x = math::ln(temperature.value() / 298.15);
    let mut sum = 0.0;
    for (power, &a) in (1..).zip(coefficients) {
        sum += a * math::powi(x, power);
    }
    at_reference * math::exp(sum)
}

/// Huber, Perkins and Lemmon 2024's correlation for nitrogen (Int. J. Thermophys. 45:146, its Eqs.
/// 1, 3 and 5 with `Δη_c` = 1): η = η₀(T) + Σᵢ Nᵢ `T_r`^tᵢ `ρ_r`^dᵢ, μPa s, the dilute gas Xiao et
/// al. 2020's (their Table 3) with Berg and Moldover 2012's η₀(298.15 K) = 17.7494 μPa s, and the
/// residual their Table 4, `T_r` = T ÷ 126.192 K and `ρ_r` = ρ ÷ 11.1839 mol dm⁻³.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Huber2024Viscosity {
    pub(crate) dilute_at_reference_micro_pa_s: f64,
    pub(crate) dilute: &'static [f64],
    pub(crate) critical_temperature: Kelvin,
    pub(crate) critical_density: KilogramsPerCubicMetre,
    pub(crate) residual: &'static [(f64, f64, f64)],
    pub(crate) range: ViscosityRange,
}

impl Huber2024Viscosity {
    /// Where the correlation keeps its source's basis.
    #[must_use]
    pub const fn range(&self) -> &ViscosityRange {
        &self.range
    }

    /// The viscosity at `temperature` and `density`, Pa s.
    #[must_use]
    pub fn viscosity_pa_s(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> f64 {
        let dilute = xiao_dilute_micro_pa_s(
            self.dilute_at_reference_micro_pa_s,
            self.dilute,
            temperature,
        );
        let t = temperature.value() / self.critical_temperature.value();
        let rho = density.value() / self.critical_density.value();
        let mut residual = 0.0;
        for &(n, power_t, power_rho) in self.residual {
            residual += n * math::powf(t, power_t) * math::powf(rho, power_rho);
        }
        (dilute + residual) * 1e-6
    }
}

/// Sotiriadou, Antoniadis, Assael, Martinek and Huber 2026's correlation for methane (Int. J.
/// Thermophys. 47:18, its Eqs. 1–6 with the critical term zero, as they set it): η = η₀ + η₀ `B_η`
/// ρ + Δη, μPa s, with η₀ Xiao et al.'s form at η₀(298.15 K) = 11.0631 μPa s (their Table 3, a₃ =
/// 3.29256 × 10⁻⁴), `B_η` Rainwater–Friend's at σ = 0.4115 nm and ε ÷ `k_B` = 112.2 K, and Δη =
/// `ρ_r`^(2/3) `T_r`^(1/2) {(f₁ `ρ_r`² + f₂ `ρ_r`³) ÷ (f₃ + `T_r` − `ρ_r`) + (f₄ + `T_r`^f₅)(f₆
/// `ρ_r` + f₇ `ρ_r`² + f₈ `ρ_r`³ + f₉ `ρ_r`⁴)}, `T_r` = T ÷ 190.564 K and `ρ_r` = ρ ÷ 162.66 kg
/// m⁻³.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sotiriadou2026Viscosity {
    pub(crate) dilute_at_reference_micro_pa_s: f64,
    pub(crate) dilute: &'static [f64],
    pub(crate) sigma_m: f64,
    pub(crate) epsilon_k: f64,
    pub(crate) molar_mass_kg_per_mol: f64,
    pub(crate) critical_temperature: Kelvin,
    pub(crate) critical_density: KilogramsPerCubicMetre,
    pub(crate) residual: [f64; 9],
    pub(crate) range: ViscosityRange,
}

impl Sotiriadou2026Viscosity {
    /// Where the correlation keeps its source's basis.
    #[must_use]
    pub const fn range(&self) -> &ViscosityRange {
        &self.range
    }

    /// The viscosity at `temperature` and `density`, Pa s.
    #[must_use]
    pub fn viscosity_pa_s(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> f64 {
        let dilute = xiao_dilute_micro_pa_s(
            self.dilute_at_reference_micro_pa_s,
            self.dilute,
            temperature,
        );
        let initial = dilute
            * viscosity_virial_m3_per_kg(
                temperature,
                self.sigma_m,
                self.epsilon_k,
                self.molar_mass_kg_per_mol,
            )
            * density.value();
        let t = temperature.value() / self.critical_temperature.value();
        let rho = density.value() / self.critical_density.value();
        let [f1, f2, f3, f4, f5, f6, f7, f8, f9] = self.residual;
        let rho2 = rho * rho;
        let rho3 = rho2 * rho;
        let residual = math::powf(rho, 2.0 / 3.0)
            * t.sqrt()
            * ((f1 * rho2 + f2 * rho3) / (f3 + t - rho)
                + (f4 + math::powf(t, f5)) * (f6 * rho + f7 * rho2 + f8 * rho3 + f9 * rho2 * rho2));
        (dilute + initial + residual) * 1e-6
    }
}

/// Laesecke and Muzny 2017's correlation for carbon dioxide (J. Phys. Chem. Ref. Data 46, 013107,
/// its Eqs. 1, 4–6, 8 and 9 without `Δη_c`): η = η₀ + η₀ `B_η` ρ + Δη, mPa s, with η₀ = 1.0055 √T ÷
/// [a₀ + a₁ T^(1/6) + a₂ exp(a₃ T^(1/3)) + (a₄ + a₅ T^(1/3)) ÷ exp(T^(1/3)) + a₆ √T] (T in K; their
/// Table 2), `B_η` Rainwater–Friend's at σ = 0.378 421 nm and ε ÷ `k_B` = 200.760 K with M =
/// 44.0095 g mol⁻¹, and Δη = `η_tL` [c₁ `T_r` `ρ_r`³ + (`ρ_r`² + `ρ_r`^γ) ÷ (`T_r` − c₂)], `T_r` =
/// T ÷ 216.592 K and `ρ_r` = ρ ÷ 1178.53 kg m⁻³, the triple point's liquid, and `η_tL` =
/// `ρ_tL`^(2/3) √(R `T_t`) ÷ (M^(1/6) `N_A`^(1/3)) with R = 8.314 459 8 J mol⁻¹ K⁻¹ (their Eq. 9;
/// Table 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaeseckeMuzny2017Viscosity {
    pub(crate) dilute: [f64; 7],
    pub(crate) sigma_m: f64,
    pub(crate) epsilon_k: f64,
    pub(crate) molar_mass_kg_per_mol: f64,
    pub(crate) gas_constant_j_per_mol_k: f64,
    pub(crate) triple_temperature: Kelvin,
    pub(crate) triple_liquid_density: KilogramsPerCubicMetre,
    pub(crate) residual: (f64, f64, f64),
    pub(crate) range: ViscosityRange,
}

impl LaeseckeMuzny2017Viscosity {
    /// Where the correlation keeps its source's basis.
    #[must_use]
    pub const fn range(&self) -> &ViscosityRange {
        &self.range
    }

    /// The viscosity at `temperature` and `density`, Pa s.
    #[must_use]
    pub fn viscosity_pa_s(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> f64 {
        let t = temperature.value();
        let cube_root = math::cbrt(t);
        let a = self.dilute;
        let dilute_milli = 1.0055 * t.sqrt()
            / (a[0]
                + a[1] * math::powf(t, 1.0 / 6.0)
                + a[2] * math::exp(a[3] * cube_root)
                + (a[4] + a[5] * cube_root) / math::exp(cube_root)
                + a[6] * t.sqrt());
        let initial = dilute_milli
            * viscosity_virial_m3_per_kg(
                temperature,
                self.sigma_m,
                self.epsilon_k,
                self.molar_mass_kg_per_mol,
            )
            * density.value();
        let triple = self.triple_temperature.value();
        let rho_triple = self.triple_liquid_density.value();
        let scale_milli = 1e3
            * math::powf(rho_triple, 2.0 / 3.0)
            * (self.gas_constant_j_per_mol_k * triple).sqrt()
            / (math::powf(self.molar_mass_kg_per_mol, 1.0 / 6.0) * math::cbrt(AVOGADRO_PER_MOL));
        let (gamma, c1, c2) = self.residual;
        let tr = t / triple;
        let rho = density.value() / rho_triple;
        let residual = scale_milli
            * (c1 * tr * math::powi(rho, 3) + (rho * rho + math::powf(rho, gamma)) / (tr - c2));
        (dilute_milli + initial + residual) * 1e-3
    }
}

/// Monogenidou, Assael and Huber 2018's correlation for ammonia (J. Phys. Chem. Ref. Data 47,
/// 023102, its Eqs. 1–8 and Tables 2 and 3): η = η₀ + η₀ `B_η` ρ + Δη, μPa s, with η₀ = 0.021357
/// √(M T) ÷ (σ² S*_η), ln S*_η = Σᵢ αᵢ (ln T*)ⁱ, T* = T ÷ 386.0 K, σ = 0.2957 nm and M = 17.030 52
/// g mol⁻¹; `B_η` Rainwater–Friend's; and Δη = `ρ_r`^(2/3) `T_r`^(1/2) {c₀ + c₁ `ρ_r` + c₂ `ρ_r`⁴ +
/// c₃ `ρ_r`⁸ ÷ `T_r`⁴ + c₄ `T_r` `ρ_r`²}, `T_r` = T ÷ 405.56 K and `ρ_r` = ρ ÷ 233.25 kg m⁻³.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monogenidou2018Viscosity {
    pub(crate) molar_mass_g_per_mol: f64,
    pub(crate) sigma_nm: f64,
    pub(crate) epsilon_k: f64,
    pub(crate) cross_section: [f64; 5],
    pub(crate) critical_temperature: Kelvin,
    pub(crate) critical_density: KilogramsPerCubicMetre,
    pub(crate) residual: [f64; 5],
    pub(crate) range: ViscosityRange,
}

impl Monogenidou2018Viscosity {
    /// Where the correlation keeps its source's basis.
    #[must_use]
    pub const fn range(&self) -> &ViscosityRange {
        &self.range
    }

    /// The viscosity at `temperature` and `density`, Pa s.
    #[must_use]
    pub fn viscosity_pa_s(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> f64 {
        let t = temperature.value();
        let ln_t_star = math::ln(t / self.epsilon_k);
        let mut ln_s = 0.0;
        for (power, &alpha) in (0..).zip(&self.cross_section) {
            ln_s += alpha * math::powi(ln_t_star, power);
        }
        let dilute = 0.021_357 * (self.molar_mass_g_per_mol * t).sqrt()
            / (self.sigma_nm * self.sigma_nm * math::exp(ln_s));
        let initial = dilute
            * viscosity_virial_m3_per_kg(
                temperature,
                self.sigma_nm * 1e-9,
                self.epsilon_k,
                self.molar_mass_g_per_mol * 1e-3,
            )
            * density.value();
        let tr = t / self.critical_temperature.value();
        let rho = density.value() / self.critical_density.value();
        let [c0, c1, c2, c3, c4] = self.residual;
        let rho2 = rho * rho;
        let rho4 = rho2 * rho2;
        let residual = math::powf(rho, 2.0 / 3.0)
            * tr.sqrt()
            * (c0 + c1 * rho + c2 * rho4 + c3 * rho4 * rho4 / math::powi(tr, 4) + c4 * tr * rho2);
        (dilute + initial + residual) * 1e-6
    }
}

/// Friend, Ingham and Ely 1991's correlation for ethane (J. Phys. Chem. Ref. Data 20, 275, its
/// Eqs. 8, 10a, 12 and 15a with Tables 1, 8 and 9): η = η₀ + `η_ex`, μPa s, with η₀ = 12.0085 √t
/// Σᵢ Cᵢ t^((i − 1)/3 − 1), t = T ÷ 245.0 K, and `η_ex` = 15.977 Σ₁⁹ gᵢ δ^rᵢ τ^sᵢ ÷ (1 + Σ₁₀¹¹ gᵢ
/// δ^rᵢ τ^sᵢ), δ = ρ ÷ 6.87 mol dm⁻³ at M = 30.070 g mol⁻¹ and τ = 305.33 K ÷ T (their Table 1's
/// critical point, on IPTS-68).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Friend1991Viscosity {
    pub(crate) epsilon_k: f64,
    pub(crate) collision: [f64; 9],
    pub(crate) critical_temperature: Kelvin,
    pub(crate) critical_density: KilogramsPerCubicMetre,
    pub(crate) numerator: &'static [(f64, i32, f64)],
    pub(crate) denominator: &'static [(f64, i32, f64)],
    pub(crate) range: ViscosityRange,
}

impl Friend1991Viscosity {
    /// Where the correlation keeps its source's basis.
    #[must_use]
    pub const fn range(&self) -> &ViscosityRange {
        &self.range
    }

    /// The viscosity at `temperature` and `density`, Pa s.
    #[must_use]
    pub fn viscosity_pa_s(&self, temperature: Kelvin, density: KilogramsPerCubicMetre) -> f64 {
        let t_star = temperature.value() / self.epsilon_k;
        let mut sum = 0.0;
        for (i, &c) in (0..).zip(&self.collision) {
            sum += c * math::powf(t_star, f64::from(i) / 3.0 - 1.0);
        }
        let dilute = 12.008_5 * t_star.sqrt() * sum;
        let delta = density.value() / self.critical_density.value();
        let tau = self.critical_temperature.value() / temperature.value();
        let term = |&(g, r, s): &(f64, i32, f64)| g * math::powi(delta, r) * math::powf(tau, s);
        let mut numerator = 0.0;
        for t in self.numerator {
            numerator += term(t);
        }
        let mut denominator = 1.0;
        for t in self.denominator {
            denominator += term(t);
        }
        (dilute + 15.977 * numerator / denominator) * 1e-6
    }
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
            Self::Huber2024(c) => (
                c.viscosity_pa_s(temperature, density),
                c.range.basis_at(temperature, density),
            ),
            Self::Sotiriadou2026(c) => (
                c.viscosity_pa_s(temperature, density),
                c.range.basis_at(temperature, density),
            ),
            Self::LaeseckeMuzny2017(c) => (
                c.viscosity_pa_s(temperature, density),
                c.range.basis_at(temperature, density),
            ),
            Self::Monogenidou2018(c) => (
                c.viscosity_pa_s(temperature, density),
                c.range.basis_at(temperature, density),
            ),
            Self::Friend1991(c) => (
                c.viscosity_pa_s(temperature, density),
                c.range.basis_at(temperature, density),
            ),
        }
    }
}

/// A Vogel–Fulcher–Tammann continuation of a row's viscosity below its triple point, for a
/// solution whose first melt lies below its component's (decision-p14-t49b-condensed §2.5): ln
/// μ(T) = ln μ(`T_t`) + B [1 ÷ (T − `T_0`) − 1 ÷ (`T_t` − `T_0`)], anchored at the reference
/// correlation's value at the triple point, so continuous there, with B and `T_0` fitted by
/// `hyperion-fit`'s `liquid_optics` to the correlation along the saturated liquid from the triple
/// point to the normal boiling point (decision-r11-palette-liquids §4), `T_0` held in [0, `T_t`):
/// the supercooled liquid's form, evaluated as fitted, and `Derived`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupercooledViscosity {
    pub(crate) b_kelvin: f64,
    pub(crate) divergence: Kelvin,
    pub(crate) triple: Kelvin,
}

impl SupercooledViscosity {
    /// B, K.
    #[must_use]
    pub const fn b_kelvin(&self) -> f64 {
        self.b_kelvin
    }

    /// `T_0`, the temperature at which the form diverges, K.
    #[must_use]
    pub const fn divergence_temperature(&self) -> Kelvin {
        self.divergence
    }

    /// `T_t`, the row's triple point, where the form is anchored and was fitted from.
    #[must_use]
    pub const fn triple_temperature(&self) -> Kelvin {
        self.triple
    }

    /// The viscosity at `temperature`, Pa s, from `at_triple_pa_s`, the row's own correlation's
    /// value at its triple point (its record's there). It holds for `temperature` in (`T_0`,
    /// `T_t`], where it rises steadily as the liquid cools; at `T_t` it is `at_triple_pa_s` itself.
    ///
    /// # Examples
    ///
    /// Methane's viscosity at Titan's first melt, 71.15 K, for a sea that holds it there:
    ///
    /// ```
    /// use hyperion_sim::substance::{Composition, by_key, liquid_properties};
    /// use hyperion_sim::units::Kelvin;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let methane = by_key("CH4").ok_or("the registry holds methane")?; let columns =
    /// methane.substance().liquid().ok_or("methane is a liquid")?; let form =
    /// columns.supercooled_viscosity().ok_or("methane's continues below T_t")?; let at_triple =
    /// liquid_properties(methane, form.triple_temperature(), Composition::Reference)?
    ///     .viscosity_pa_s
    ///     .ok_or("methane has a viscosity")?;
    /// let titan = form.viscosity_pa_s(at_triple, Kelvin::new(71.15));
    /// // About 0.54 mPa s, against 0.21 at its triple point.
    /// assert!(titan > 2.0 * at_triple && titan < 3.5 * at_triple);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn viscosity_pa_s(&self, at_triple_pa_s: f64, temperature: Kelvin) -> f64 {
        let t0 = self.divergence.value();
        at_triple_pa_s
            * math::exp(
                self.b_kelvin
                    * (1.0 / (temperature.value() - t0) - 1.0 / (self.triple.value() - t0)),
            )
    }
}

/// A saturated liquid's specific isobaric heat capacity `c_l`(T), J kg⁻¹ K⁻¹, from its reference
/// equation of state over a range (decision-r09-t13i §2.1; decision-r09-t13i-cl-licence §6),
/// held at the range's ends.
///
/// At a temperature T in the range it is the equation's `c_p`
/// ([`HelmholtzResidual::isobaric_heat_capacity_j_per_kg_k`]) at the equation's own saturated
/// liquid on the row's phase diagram: the density four Newton steps take from the diagram's
/// saturated-liquid density ([`PhaseDiagram::liquid_density`]) to the diagram's vapour pressure
/// ([`PhaseDiagram::saturation_pressure`]) along the isotherm
/// ([`HelmholtzResidual::density_at_pressure`]). Outside the range it is the nearer end's value, to
/// the bit. The source is the row's [`LiquidColumns::sources`] entry that covers `c_l`; the
/// equation is the row's scattering's, as a test holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidSpecificHeat {
    pub(crate) equation: &'static HelmholtzResidual,
    pub(crate) ideal: &'static HelmholtzIdeal,
    pub(crate) low: Kelvin,
    pub(crate) high: Kelvin,
}

impl LiquidSpecificHeat {
    /// The reference equation of state's residual part.
    #[must_use]
    pub const fn equation(&self) -> &'static HelmholtzResidual {
        self.equation
    }

    /// The same equation's ideal-gas part.
    #[must_use]
    pub const fn ideal(&self) -> &'static HelmholtzIdeal {
        self.ideal
    }

    /// The lowest temperature of its range, above the row's triple point.
    #[must_use]
    pub const fn low(&self) -> Kelvin {
        self.low
    }

    /// The highest temperature of its range, below the row's critical point.
    #[must_use]
    pub const fn high(&self) -> Kelvin {
        self.high
    }

    /// Its range, from [`low`](Self::low) to [`high`](Self::high).
    #[must_use]
    pub const fn range(&self) -> (Kelvin, Kelvin) {
        (self.low, self.high)
    }

    /// Its value at `temperature` on `diagram`, the row's own phase diagram, J kg⁻¹ K⁻¹: the
    /// equation's `c_p` at its saturated liquid at `temperature` held to the range, so that outside
    /// it the nearer end's value holds to the bit; a NaN gives a NaN. `None` where the diagram has
    /// no liquid density there, which no row's range reaches, as a test holds.
    #[must_use]
    pub(crate) fn at(&self, diagram: &PhaseDiagram, temperature: Kelvin) -> Option<f64> {
        if temperature.value().is_nan() {
            return Some(f64::NAN);
        }
        let held = Kelvin::new(
            temperature
                .value()
                .clamp(self.low.value(), self.high.value()),
        );
        let start = diagram.liquid_density(held)?;
        let density =
            self.equation
                .density_at_pressure(start, diagram.saturation_pressure(held), held);
        Some(
            self.equation
                .isobaric_heat_capacity_j_per_kg_k(self.ideal, density, held),
        )
    }
}

/// What a row's liquid is made of, and so which compositions its record resolves at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LiquidConstitution {
    /// A pure liquid, the row's own species, its columns its own: its one component is itself
    /// (`components`, the row alone), and its reference composition itself at a mole fraction of
    /// 1 (`reference`).
    Pure {
        /// The row alone.
        components: &'static [SubstanceId],
        /// The row at 1.
        reference: &'static [(SubstanceId, f64)],
    },
    /// A solution of registry rows, its record made from its components' compositions by its
    /// mixing model; its own optics, scattering and transport columns are `None`.
    Solution(SolutionColumns),
    /// A solution whose solute has no registry row yet, so that only its reference resolves, from
    /// its own columns, and it has no components until the row is appended
    /// (decision-p14-t49h-step3-composition §1.3 and §6): `brine_naclo4` and `brine_mgclo4`, with
    /// their solutes' constants.
    AwaitingSolute(&'static RowlessSolute),
    /// A lithology's silicate melt, its record made from the row's major oxides by its own models
    /// at the record's temperature (decision-p14-t49h-step4-melts): its composition is its oxides,
    /// which are not registry rows, so it has no components and its given composition is not
    /// modelled (decision-p14-t49h-step3-composition §2.3); its own optics, scattering and
    /// transport columns are `None`.
    Melt(&'static SilicateMelt),
}

/// A row's liquid columns (decision-r11-palette-liquids §4), each `None` where not modelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidColumns {
    pub(crate) optics: Option<LiquidOptics>,
    pub(crate) scattering: Option<LiquidScattering>,
    pub(crate) surface_tension: Option<SurfaceTensionEquation>,
    pub(crate) viscosity: Option<ViscosityCorrelation>,
    pub(crate) supercooled_viscosity: Option<SupercooledViscosity>,
    pub(crate) specific_heat: Option<LiquidSpecificHeat>,
    pub(crate) constitution: LiquidConstitution,
    pub(crate) sources: &'static [Source],
}

impl LiquidColumns {
    /// The components a [`Composition::Given`] may name, in registry order: a pure liquid's is
    /// itself, a solution's its model's (the aqueous acid's, water and `H2SO4` itself); empty
    /// where a given composition is not modelled (`brine_naclo4` and `brine_mgclo4` until their
    /// solutes have rows, and the melts) (decision-p14-t49h-step3-composition §2.2).
    ///
    /// # Examples
    ///
    /// Titan's seas take methane, nitrogen and ethane, in the registry's order:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let titan = by_key("ch4_c2h6_n2").ok_or("the registry holds Titan's seas")?;
    /// let columns = titan.substance().liquid().ok_or("Titan's seas are a liquid")?;
    /// let rows: Vec<_> = ["CH4", "N2", "C2H6"].into_iter().filter_map(by_key).collect();
    /// assert_eq!(columns.components(), rows.as_slice());
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub const fn components(&self) -> &'static [SubstanceId] {
        match &self.constitution {
            LiquidConstitution::Pure { components, .. } => components,
            LiquidConstitution::Solution(solution) => solution.components,
            LiquidConstitution::AwaitingSolute(_) | LiquidConstitution::Melt(_) => &[],
        }
    }

    /// The reference composition by mole fraction, in registry order: a pure liquid's is itself
    /// at 1, a solution's its stated reference (decision-p14-t49b-condensed §1.4 and §2.6);
    /// `None` where [`components`](Self::components) is empty.
    ///
    /// [`Composition::Reference`] resolves as this composition given, to the bit.
    #[must_use]
    pub const fn reference_composition(&self) -> Option<LiquidComposition<'static>> {
        match &self.constitution {
            LiquidConstitution::Pure { reference, .. } => {
                Some(LiquidComposition::of_reference(reference))
            }
            LiquidConstitution::Solution(solution) => {
                Some(LiquidComposition::of_reference(solution.reference))
            }
            LiquidConstitution::AwaitingSolute(_) | LiquidConstitution::Melt(_) => None,
        }
    }

    /// A melt row's total iron as `FeO`, FeO(T), as a mass fraction of its analysis: its major
    /// oxides' FeO(T) in weight per cent over 100 (P14.T54.b.c; decision-p14-t51d-weathering
    /// §1.2), not over the analysis's own total, which lies within a weight per cent of 100.
    /// `None` for a liquid that is not a lithology's melt.
    #[must_use]
    pub(crate) fn melt_feo_mass_fraction(&self) -> Option<f64> {
        match &self.constitution {
            LiquidConstitution::Melt(melt) => {
                Some(melt::total_iron_as_feo_wt_pct(melt.oxides) / 100.0)
            }
            LiquidConstitution::Pure { .. }
            | LiquidConstitution::Solution(_)
            | LiquidConstitution::AwaitingSolute(_) => None,
        }
    }

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

    /// The viscosity's continuation below the triple point, for a row that is a solution's
    /// component there (decision-p14-t49b-condensed §2.5).
    #[must_use]
    pub const fn supercooled_viscosity(&self) -> Option<&SupercooledViscosity> {
        self.supercooled_viscosity.as_ref()
    }

    /// The saturated liquid's own specific heat, for the six pure liquids decision-r09-t13i §2.1
    /// gives one, from their reference equations of state; `None` for every other row, a
    /// solution's being its components' ([`Substance::liquid_specific_heat_j_per_kg_k`]).
    #[must_use]
    pub const fn specific_heat(&self) -> Option<&LiquidSpecificHeat> {
        self.specific_heat.as_ref()
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

    /// Its saturated liquid's specific heat `c_l` at `temperature`, J kg⁻¹ K⁻¹
    /// (decision-r09-t13i §2.1), each column's end value held outside its range
    /// ([`LiquidSpecificHeat`]); a NaN gives a NaN:
    ///
    /// - a pure liquid's is its own column's: its reference equation of state's `c_p` at the
    ///   equation's saturated liquid on the row's phase diagram (decision-r09-t13i-cl-licence §6);
    /// - a solution's is its components' mean weighted by mass at its reference composition,
    ///   Σ wᵢ `c_l,i`(T) with wᵢ = xᵢ Mᵢ ÷ Σ xⱼ Mⱼ from the mole fractions xᵢ and the rows' molar
    ///   masses Mᵢ, each component's own column at T, summed in the components' order. That is
    ///   ideal mixing, with no excess heat capacity, and of low confidence, as the ruling states;
    ///   Titan's seas and ammonia–water take it today.
    ///
    /// `None` where the row has no liquid columns, or no column of its own (a pure liquid the
    /// ruling gives none), where any component of a solution lacks one or a molar mass (the
    /// chloride and sulphate brines, whose salts have none, and the aqueous acid), for a solution
    /// whose solute has no row yet and for a silicate melt.
    ///
    /// # Examples
    ///
    /// Titan's seas at the Huygens landing site's 93.65 K hold about seven tenths of the heat per
    /// kilogram per kelvin of Earth's water at 285 K:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::Kelvin;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let heat = |key: &str, t: f64| {
    ///     by_key(key)
    ///         .and_then(|id| id.substance().liquid_specific_heat_j_per_kg_k(Kelvin::new(t)))
    ///         .ok_or("the row's liquid has a specific heat")
    /// };
    /// let ratio = heat("ch4_c2h6_n2", 93.65)? / heat("H2O", 285.0)?;
    /// assert!((0.6..0.8).contains(&ratio), "{ratio}");
    /// // Sodium chloride's liquid has no column, so its brine has none.
    /// assert!(heat("brine_nacl", 285.0).is_err());
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn liquid_specific_heat_j_per_kg_k(&self, temperature: Kelvin) -> Option<f64> {
        let columns = self.liquid()?;
        match &columns.constitution {
            LiquidConstitution::Pure { .. } => self.own_liquid_specific_heat(temperature),
            LiquidConstitution::Solution(solution) => {
                mixed_specific_heat_j_per_kg_k(solution, temperature)
            }
            LiquidConstitution::AwaitingSolute(_) | LiquidConstitution::Melt(_) => None,
        }
    }

    /// The range of its pure liquid's own specific heat column ([`LiquidSpecificHeat::range`]);
    /// `None` where [`liquid_specific_heat_j_per_kg_k`](Self::liquid_specific_heat_j_per_kg_k)
    /// is, and for a solution, whose components' columns each hold their own ends.
    #[must_use]
    pub fn liquid_specific_heat_range(&self) -> Option<(Kelvin, Kelvin)> {
        let columns = self.liquid()?;
        match &columns.constitution {
            LiquidConstitution::Pure { .. } => {
                columns.specific_heat().map(LiquidSpecificHeat::range)
            }
            LiquidConstitution::Solution(_)
            | LiquidConstitution::AwaitingSolute(_)
            | LiquidConstitution::Melt(_) => None,
        }
    }

    /// Its own specific heat column's value at `temperature` on its own phase diagram, J kg⁻¹
    /// K⁻¹; `None` for a row with no column or no diagram.
    #[must_use]
    fn own_liquid_specific_heat(&self, temperature: Kelvin) -> Option<f64> {
        self.liquid()?
            .specific_heat()?
            .at(self.phase_diagram()?, temperature)
    }
}

/// A solution's specific heat at its reference composition and `temperature`, J kg⁻¹ K⁻¹: Σ xᵢ Mᵢ
/// `c_l,i`(T) ÷ Σ xⱼ Mⱼ, which is Σ wᵢ `c_l,i`(T) with the mass fractions wᵢ = xᵢ Mᵢ ÷ Σ xⱼ Mⱼ,
/// over the solution's components in their order, each fraction read from the reference (0 where
/// it is absent) and each component's own column at T on its own phase diagram, held at its own
/// ends; `None` where any component lacks a column or a molar mass
/// ([`Substance::liquid_specific_heat_j_per_kg_k`]).
#[must_use]
fn mixed_specific_heat_j_per_kg_k(solution: &SolutionColumns, temperature: Kelvin) -> Option<f64> {
    let mut mass = 0.0;
    let mut heat = 0.0;
    for &component in solution.components {
        let x = solution
            .reference
            .iter()
            .find(|&&(id, _)| id == component)
            .map_or(0.0, |&(_, x)| x);
        let row = component.substance();
        let molar_mass = row.molar_mass_g_per_mol()?;
        let c = row.own_liquid_specific_heat(temperature)?;
        mass += x * molar_mass;
        heat += x * molar_mass * c;
    }
    Some(heat / mass)
}

/// A liquid's composition by mole fraction, each component a registry row that is a neutral
/// species named by a formula key (decision-p14-t49b-condensed §1.4: the composition's invariant
/// is its moles, and the mixing rules take mole fractions; decision-p14-t49h-step3-composition
/// §2.1: a solution, lithology or organic row, an ion or the electron is never a component).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidComposition<'a> {
    mole_fractions: &'a [(SubstanceId, f64)],
}

impl<'a> LiquidComposition<'a> {
    /// The composition of `mole_fractions`: one or more components, each a neutral species with
    /// a formula key, each once, each fraction in (0, 1], summing to 1 within 10⁻⁹.
    ///
    /// Each component is checked in the given order, its species first, then its fraction, then
    /// whether it was named before.
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
            let row = id.substance();
            if row.key().form() != SubstanceKeyForm::Formula || row.charge() != 0 {
                return Err(BuildLiquidCompositionError::NotASpecies { component: id });
            }
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

    /// A row's reference composition, `mole_fractions` as its liquid columns state it, unchecked
    /// here: `each_liquid_s_components_are_its_rows` holds that [`new`](Self::new) accepts each
    /// unchanged.
    #[must_use]
    pub(crate) const fn of_reference(mole_fractions: &'a [(SubstanceId, f64)]) -> Self {
        Self { mole_fractions }
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
    /// A component is not a neutral species named by a formula key: a solution, lithology,
    /// organic or assemblage row (a name key), an ion, or the electron.
    NotASpecies {
        /// The component.
        component: SubstanceId,
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
            Self::NotASpecies { component } => write!(
                f,
                "{} is not a neutral species, so not a liquid's component",
                component.substance().key()
            ),
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
    ///
    /// - **A solution's** may name any of its components ([`LiquidColumns::components`]), each
    ///   at most once; a component it leaves out is absent from the mixture.
    /// - **A pure liquid's** names the row itself, at 1, and resolves as its reference.
    /// - Anything else names a component the row's model does not carry, and is refused.
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
    /// The temperature lies outside the row's modelled liquid range, from its first melt to below
    /// `below`: P14.T54.a clamps a record's temperature into it first
    /// (decision-r11-palette-liquids §1.7; decision-p14-t49b-condensed §2;
    /// decision-p14-t49h-step3-composition §3).
    OutOfRange {
        /// The row.
        key: SubstanceKey,
        /// The temperature asked for.
        temperature: Kelvin,
        /// The range's lowest temperature, the row's first melt.
        lowest: Kelvin,
        /// The temperature the range ends below: the row's critical point; for a solution, the
        /// lowest critical point among the components the composition holds; or `None` where
        /// none states one (a silicate melt's, whose range has no upper end).
        below: Option<Kelvin>,
    },
    /// A given composition is not modelled for the row, whose
    /// [`components`](LiquidColumns::components) are empty: a melt's, whose composition is its
    /// oxides, or a solution's whose components are not all registry rows (`brine_naclo4` and
    /// `brine_mgclo4`, whose perchlorate solutes have none yet).
    CompositionNotModelled {
        /// The row.
        key: SubstanceKey,
    },
    /// A given component is not one the row's model carries
    /// ([`LiquidColumns::components`]): for a pure liquid, any row but itself.
    ComponentNotModelled {
        /// The row.
        key: SubstanceKey,
        /// The component named.
        component: SubstanceKey,
    },
    /// A given component's mole fraction lies outside the range the row's model evaluates,
    /// `lowest` to `highest` inclusive (its source's range, or saturation), by more than 10⁻⁹, the
    /// tolerance a composition's sum is held to: a brine's salt above its density model's highest
    /// fraction, the aqueous acid outside 25–85 wt%, or `nh3_h2o` at any composition but its
    /// reference, where `lowest` and `highest` are both the reference's fraction.
    CompositionOutOfRange {
        /// The row.
        key: SubstanceKey,
        /// The component.
        component: SubstanceKey,
        /// Its mole fraction, dimensionless.
        mole_fraction: f64,
        /// The lowest mole fraction the model evaluates.
        lowest: f64,
        /// The highest mole fraction the model evaluates.
        highest: f64,
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
            Self::ComponentNotModelled { key, component } => {
                write!(
                    f,
                    "{key}'s liquid does not model {component} as a component"
                )
            }
            Self::CompositionOutOfRange {
                key,
                component,
                mole_fraction,
                lowest,
                highest,
            } => write!(
                f,
                "{component}'s mole fraction {mole_fraction} in {key}'s liquid lies outside \
                 {lowest} to {highest}"
            ),
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

impl Substance {
    /// Its liquid's grain density at `temperature` compressed to `pressure`, kg m⁻³, without pore
    /// space (P14.T49.n; decision-p14-t54a-water-consistency, its Batch B follow-up of 2026-10-20,
    /// F.1 and C):
    ///
    /// `ρ_l`(T, p) = `ρ_reg`(T) × `ρ_eos`(T, p′) ÷ `ρ_eos`(T, `p_sat`(T)), with p′ = max(p,
    /// `p_sat`(T)) held at most 1,000 MPa,
    ///
    /// for a pure liquid with a reference equation of state, its scattering's
    /// ([`LiquidScattering::equation`]), and a phase diagram: `ρ_reg`(T) is the registry's
    /// saturated liquid ([`Substance::grain_density`]), `p_sat`(T) the diagram's vapour pressure
    /// ([`PhaseDiagram::saturation_pressure`]), and `ρ_eos`(T, p) the equation's density at p on
    /// the liquid's isotherm: at `p_sat`, four Newton steps from `ρ_reg`, the equation's own
    /// saturated liquid as [`LiquidSpecificHeat`] takes it
    /// ([`HelmholtzResidual::density_at_pressure`]); at p′,
    /// [`COMPRESSION_STEPS`](super::helmholtz::COMPRESSION_STEPS) more from there
    /// ([`HelmholtzResidual::compressed_density`]). The equation's compression is carried onto the
    /// registry's saturated density, so the law is the registry's own at saturation to the bit, and
    /// it never expands a liquid: a pressure at or below `p_sat`(T) gives `ρ_reg`(T) itself, as
    /// does a pressure that is not a number, and any pressure at the critical temperature, where
    /// `p_sat` is infinite. Above 1,000 MPa, the upper bound of IAPWS-95's validity, the liquid is
    /// held at its density there, a stated hold.
    ///
    /// The equations are IAPWS-95 for water (IAPWS R6-95(2018); Wagner and Pruß 2002, J. Phys.
    /// Chem. Ref. Data 31, 387), and each other row's as its liquid columns cite it: Setzmann and
    /// Wagner 1991's methane (J. Phys. Chem. Ref. Data 20, 1061), Span et al. 2000's nitrogen
    /// (29, 1361), Bücker and Wagner 2006's ethane (35, 205), Gao et al. 2023's ammonia (52,
    /// 013102), Span and Wagner 1996's carbon dioxide (25, 1509), Schmidt and Wagner 1985's oxygen
    /// (Fluid Phase Equilib. 19, 175), Tegeler, Span and Wagner 1999's argon (J. Phys. Chem. Ref.
    /// Data 28, 779), and Lemmon and Span 2006's krypton, xenon, carbon monoxide, nitrous oxide and
    /// hydrogen sulphide (J. Chem. Eng. Data 51, 785).
    ///
    /// Every other liquid is incompressible here, its saturated or stated density at every
    /// pressure, a recorded refinement: the solutions (the brines, ammonia–water, the aqueous acid
    /// and Titan's seas), the silicate and metal melts, and a pure liquid with no equation of
    /// state (sulphur dioxide's).
    ///
    /// `None` where the row has no liquid density at `temperature` ([`Substance::grain_density`]).
    /// `water_s_compressed_liquid_meets_nist_s_isotherms` holds water at 596.41 K under a steam
    /// world's 22.5 MPa to the NIST Chemistry WebBook's 690.48 kg m⁻³ (SRD 69, its evaluation of
    /// IAPWS-95), about 4.8% denser than its saturated liquid under its vapour's 11.78 MPa.
    ///
    /// Crate-private while [`COMPRESSED_LIQUIDS_VERSION`] holds the record's liquids saturated,
    /// so that no caller outside the crate publishes 24's law at 23.
    #[must_use]
    pub(crate) fn compressed_liquid_density(
        &self,
        temperature: Kelvin,
        pressure: Pascals,
    ) -> Option<KilogramsPerCubicMetre> {
        self.grain_density(CondensedPhase::Liquid, temperature)
            .map(|saturated| compressed(self, saturated, temperature, pressure))
    }

    /// Its liquid's grain density at `temperature` under `pressure`, kg m⁻³, as the record's
    /// readers take it ([`COMPRESSED_LIQUIDS_VERSION`]'s hold): the saturated liquid's
    /// ([`Substance::grain_density`]) under [`CompressedLiquidsRules::Saturated`], 23's to the bit,
    /// and [`Substance::compressed_liquid_density`] under [`CompressedLiquidsRules::Compressed`].
    #[must_use]
    pub(crate) fn liquid_density_at_pressure(
        &self,
        temperature: Kelvin,
        pressure: Pascals,
    ) -> Option<KilogramsPerCubicMetre> {
        match CompressedLiquidsRules::in_force() {
            CompressedLiquidsRules::Saturated => {
                self.grain_density(CondensedPhase::Liquid, temperature)
            }
            CompressedLiquidsRules::Compressed => {
                self.compressed_liquid_density(temperature, pressure)
            }
        }
    }
}

/// The reference equation of state a liquid's compression reads, with its row's phase diagram:
/// a pure liquid's scattering's equation, where its grain density is its diagram's
/// ([`GrainDensity::OfDiagram`], the saturated liquid's); `None` for any other row.
#[must_use]
fn equation_of_state(row: &Substance) -> Option<(&'static HelmholtzResidual, &PhaseDiagram)> {
    let columns = row.liquid()?;
    let LiquidConstitution::Pure { .. } = columns.constitution else {
        return None;
    };
    let GrainDensity::OfDiagram = row.condensed()?.grain_density() else {
        return None;
    };
    Some((columns.scattering?.equation, row.phase_diagram()?))
}

/// `saturated`, the row `row`'s saturated liquid density at `temperature`, compressed to
/// `pressure` by its reference equation of state ([`Substance::compressed_liquid_density`]'s law);
/// `saturated` itself, to the bit, for a row with no equation and at a pressure not above the
/// vapour's.
#[must_use]
fn compressed(
    row: &Substance,
    saturated: KilogramsPerCubicMetre,
    temperature: Kelvin,
    pressure: Pascals,
) -> KilogramsPerCubicMetre {
    let Some((equation, diagram)) = equation_of_state(row) else {
        return saturated;
    };
    let vapour = diagram.saturation_pressure(temperature);
    // By comparison, so that a pressure that is not a number compresses nothing, nor any pressure
    // where the vapour's is infinite, at the critical temperature.
    let compresses = pressure > vapour;
    if !compresses {
        return saturated;
    }
    let held = if pressure < COMPRESSION_CEILING {
        pressure
    } else {
        COMPRESSION_CEILING
    };
    let at_vapour = equation.density_at_pressure(saturated, vapour, temperature);
    let at_pressure = equation.compressed_density(at_vapour, held, temperature);
    KilogramsPerCubicMetre::new(saturated.value() * (at_pressure.value() / at_vapour.value()))
}

/// The row's liquid density at `temperature` with its basis, or `None` where it has none: a
/// diagram row's saturated liquid inside [triple, critical], `Measured`; a stated value at any
/// temperature, `Measured` at its own and `Derived` elsewhere; a liquid by temperature's nodes'
/// law, `Derived` at every temperature, since a row's nodes may be a mixing rule's (the silicate
/// melts', P14.T49.n), which its grain-density source states and the record cannot tell apart.
#[must_use]
fn record_density(
    row: &Substance,
    temperature: Kelvin,
) -> Option<(KilogramsPerCubicMetre, LiquidBasis)> {
    match row.condensed()?.grain_density() {
        GrainDensity::OfDiagram => row
            .grain_density(CondensedPhase::Liquid, temperature)
            .map(|density| (density, LiquidBasis::Measured)),
        GrainDensity::MeasuredLiquid { .. } => row
            .grain_density(CondensedPhase::Liquid, temperature)
            .map(|density| (density, LiquidBasis::Derived)),
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
/// extension (a line below the triple point, a stated value away from its temperature, or a
/// liquid by temperature held beyond its first or last node).
#[must_use]
fn state_density(row: &Substance, state: Kelvin) -> Option<(KilogramsPerCubicMetre, bool)> {
    match row.condensed()?.grain_density() {
        GrainDensity::OfDiagram => row
            .phase_diagram()
            .map(|diagram| extended_liquid_density(diagram, state)),
        GrainDensity::MeasuredLiquid { liquid, .. } => {
            let (first, last) = (liquid.first()?.0, liquid.last()?.0);
            row.grain_density(CondensedPhase::Liquid, state)
                .map(|density| (density, state < first || state > last))
        }
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
/// equation of state at the record's density and the row's depolarisation ratio there:
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
/// let delta = inputs.depolarisation_ratio(density, t);
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
/// For a pure liquid:
///
/// - **The temperature** must lie in the row's liquid range, from its first melt (its
///   condensed columns' transition, decision-p14-t49b-condensed §2) to below its critical point;
///   P14.T54.a clamps a body's temperature into it first.
/// - **The index** is the row's R08 file's, carried by Lorentz–Lorenz to the record's density;
///   **the density** is the condensed columns'; both are required.
/// - **The absorption** is the file's where it is modelled; **the scattering** is
///   [`pure_liquid_scattering_per_m`] at the record's index, temperature and the equation of
///   state's `κ_T` at the record's density, `Derived` where `q_max` ξ passes
///   [`CRITICAL_SCATTERING_LIMIT`]; **the surface tension and viscosity** are the row's
///   correlations at the record's state. Each is absent where the row has no source, and the
///   surface tension where its correlation gives none.
/// - **The basis** is the least direct of the members'.
/// - **A given composition** must be the row itself at 1, and resolves as the reference.
///
/// For a solution, at its reference composition or a given one that names any of its components
/// ([`LiquidColumns::components`]):
///
/// - **The temperature** must lie from the solution's first melt to below the lowest critical
///   point among the components the composition holds (decision-p14-t49h-step3-composition §3).
/// - **Each component** is its own record at the temperature, or, below its triple point,
///   decision-p14-t49b-condensed §2.5's continuation: its density linear at its ancillary's
///   triple-point slope, its index carried by Lorentz–Lorenz to that density, its `κ_T` held at its
///   triple-point value, its viscosity its fitted Vogel–Fulcher–Tammann form
///   ([`SupercooledViscosity`], absent where it has none) and its surface tension its correlation's
///   form; all `Derived`.
/// - **The members** are the solution's mixing model's of its components', summed over the model's
///   components in registry order, each at its fraction in the composition, whatever the
///   composition's order:
///   - Titan's seas, an ideal mixture: its density by volume additivity, its index by
///     Lorentz–Lorenz, its surface tension by the Macleod–Sugden parachor, its viscosity by
///     Grunberg and Nissan, its absorption by Beer's additivity and its scattering by
///     Einstein–Smoluchowski–Cabannes with the mixture's compressibility and an ideal-solution
///     concentration term; a by-bin or scalar member is absent where any component present lacks
///     it;
///   - the chloride and sulphate brines, an aqueous electrolyte from pure water to its density
///     model's highest salt fraction: Laliberté and Cooper 2004's density and Laliberté 2007's
///     viscosity with water's own in place of theirs, the index by additivity of specific
///     refraction, the absorption water's by volume fraction where the salt's transparency is
///     cited, and no surface tension or scattering;
///   - ammonia–water at its reference composition alone: its stated density, with the index by
///     Lorentz–Lorenz and the surface tension by the parachor at that density, and no viscosity,
///     absorption or scattering;
///   - aqueous sulphuric acid, an empirical model of the whole liquid in its acid's mass fraction
///     from 25 to 85 wt%: Myhre, Nielsen and Saastad 1998's density without its printed T⁴ column;
///     the index by specific refraction linear in mass fraction between Palmer and Williams's 75
///     and 84.5 wt% files at 300 K, carried by Lorentz–Lorenz to the record's density; the
///     absorption the files' upper bounds mixed by mass, and outside 75–84.5 wt% the larger of the
///     two held; Vehkamäki et al. 2002's surface tension, standing in for Myhre's own; Williams and
///     Long 1995's viscosity, its mass fraction held in their 30–80 wt%; and no scattering.
/// - **A solution whose solute has no row** (`brine_naclo4`, `brine_mgclo4`) resolves at its
///   reference from its own columns, which hold its stated density and no index, so it is
///   [`ResolveLiquidError::NotModelled`] for the index, from its eutectic to below water's critical
///   point.
/// - **The basis** is at least `Derived` (a mixing rule, and a reference composition standing in
///   for a body's own until P14.T51.a carries one), and the least direct of the components'.
/// - Between the solution's first melt and the liquidus of its composition, unmapped for Titan's
///   ternary, the record stands in for the supercooled liquid at that composition
///   (decision-p14-t49b-condensed §2.4).
///
/// For a lithology's silicate melt (decision-p14-t49h-step4-melts), at its reference composition
/// alone, its major oxides:
///
/// - **The temperature** must lie at or above the row's first melt, its solidus, with no upper end:
///   above its models' calibrations (2,073.15 K for the viscosity, 1,873 K for the density) each
///   member extrapolates a continuous, bounded form.
/// - **The viscosity** is Giordano, Russell and Dingwell 2008's model on the oxides, or, for a melt
///   with `Al₂O₃` of at least 15 wt%, `Na₂O` + K₂O of at most 1 wt% and at most 3 wt% of other
///   minor oxides, Giordano and Russell 2007's model of the CaO–MgO–Al₂O₃–SiO₂ melts, its iron and
///   manganese as `MgO`: `Derived`, and absent for a melt the rule leaves to a ruling.
/// - **The density** is Lange and Carmichael's ideal mixing of partial molar volumes, `Derived`,
///   the compressibility neglected.
/// - **The index and the absorption** are R08's basaltic glass's at room temperature, held, and
///   **the surface tension** a stated constant (Walker and Mullins 1981's basaltic 0.36 N m⁻¹,
///   Bagdassarov, Dorfman and Dingwell 2000's granitic 0.29), each `StandIn`; **the scattering** is
///   absent.
/// - **The basis** is `StandIn`. Between the solidus and the liquidus the record is the bulk liquid
///   at the row's composition, not the crystal-laden residual melt.
///
/// [`Composition::Reference`] resolves as [`LiquidColumns::reference_composition`] given, by one
/// path, to the bit.
///
/// # Errors
///
/// In this order (decision-p14-t49h-step3-composition §2.3), each naming the first offence in the
/// given composition's order:
///
/// 1. [`ResolveLiquidError::NotALiquid`] for a row with no liquid columns;
/// 2. [`ResolveLiquidError::CompositionNotModelled`] for a [`Composition::Given`] to a row with
///    no components (the perchlorate brines and the silicate melts);
/// 3. [`ResolveLiquidError::ComponentNotModelled`] for a given component the row's model does not
///    carry;
/// 4. [`ResolveLiquidError::CompositionOutOfRange`] for a given mole fraction outside the model's
///    range (none for an ideal mixture; a brine's salt above its highest fraction; the acid
///    outside 25–85 wt%; any composition but ammonia–water's reference);
/// 5. [`ResolveLiquidError::OutOfRange`] for a temperature outside the liquid range (for a melt,
///    below its solidus);
/// 6. [`ResolveLiquidError::NotModelled`] where the row's index or density, or a component's, is
///    not modelled.
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
///
/// Titan's seas at Huygens's 93.65 K, at Ligeia Mare's composition and as a nitrogen-free
/// methane–ethane binary, lighter without the dense dissolved nitrogen:
///
/// ```
/// use hyperion_sim::substance::{Composition, LiquidComposition, by_key, liquid_properties};
/// use hyperion_sim::units::Kelvin;
/// use hyperion_surface::field::LiquidBasis;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let titan = by_key("ch4_c2h6_n2").ok_or("the registry holds Titan's seas")?;
/// let t = Kelvin::new(93.65);
/// let ligeia = liquid_properties(titan, t, Composition::Reference)?;
/// assert!((ligeia.density_kg_m3 - 518.9).abs() < 0.05);
/// assert_eq!(ligeia.basis, LiquidBasis::Derived);
/// let methane = by_key("CH4").ok_or("the registry holds methane")?;
/// let ethane = by_key("C2H6").ok_or("the registry holds ethane")?;
/// let fractions = [(methane, 0.8), (ethane, 0.2)];
/// let binary = Composition::Given(LiquidComposition::new(&fractions)?);
/// let binary = liquid_properties(titan, t, binary)?;
/// assert!(binary.density_kg_m3 < ligeia.density_kg_m3);
/// assert!(binary.refractive_index[6] > 1.28 && binary.refractive_index[6] < 1.40);
/// # Ok(())
/// # }
/// ```
///
/// A 3.5 wt% chloride brine at 25 °C, sodium chloride at a mole fraction of 0.011 06, denser and
/// more refractive than fresh water:
///
/// ```
/// use hyperion_sim::substance::{Composition, LiquidComposition, by_key, liquid_properties};
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let brine = by_key("brine_nacl").ok_or("the registry holds the chloride brine")?;
/// let water = by_key("H2O").ok_or("the registry holds water")?;
/// let salt = by_key("NaCl").ok_or("the registry holds sodium chloride")?;
/// let fractions = [(water, 0.988_94), (salt, 0.011_06)];
/// let t = Kelvin::new(298.15);
/// let sea = liquid_properties(brine, t, Composition::Given(LiquidComposition::new(&fractions)?))?;
/// let fresh = liquid_properties(water, t, Composition::Reference)?;
/// assert!(sea.density_kg_m3 > fresh.density_kg_m3 + 20.0);
/// // About 0.006 more in the bin across 545 nm.
/// let increment = sea.refractive_index[6] - fresh.refractive_index[6];
/// assert!((increment - 0.006).abs() < 3e-4);
/// # Ok(())
/// # }
/// ```
pub fn liquid_properties(
    id: SubstanceId,
    temperature: Kelvin,
    composition: Composition<'_>,
) -> Result<LiquidProperties, ResolveLiquidError> {
    // No pressure compresses a liquid beyond its saturated state: this is the saturated record.
    resolve(id, temperature, composition, Pascals::ZERO)
}

/// The liquid's record for row `id` at `temperature` and `composition` under `pressure`, as the
/// palette's liquid entry reads it ([`COMPRESSED_LIQUIDS_VERSION`]'s hold): [`liquid_properties`]
/// to the bit under [`CompressedLiquidsRules::Saturated`]; under
/// [`CompressedLiquidsRules::Compressed`], its record with a pure liquid's density compressed to
/// `pressure`, Pa ([`Substance::compressed_liquid_density`]'s law, at the record's temperature),
/// so that the members read at the record's density move with it: the index carried to it by
/// Lorentz–Lorenz, the scattering's `κ_T`, depolarisation and critical-region basis there, and the
/// viscosity there. A solution's and a melt's record is [`liquid_properties`]' under either, the
/// law holding them incompressible.
///
/// # Errors
///
/// As [`liquid_properties`].
pub(crate) fn liquid_properties_at_pressure(
    id: SubstanceId,
    temperature: Kelvin,
    composition: Composition<'_>,
    pressure: Pascals,
) -> Result<LiquidProperties, ResolveLiquidError> {
    match CompressedLiquidsRules::in_force() {
        CompressedLiquidsRules::Saturated => liquid_properties(id, temperature, composition),
        CompressedLiquidsRules::Compressed => resolve(id, temperature, composition, pressure),
    }
}

/// [`liquid_properties`] under `pressure`, a pure liquid's density compressed to it
/// ([`compressed`]); at zero pressure the saturated record.
///
/// # Errors
///
/// As [`liquid_properties`].
fn resolve(
    id: SubstanceId,
    temperature: Kelvin,
    composition: Composition<'_>,
    pressure: Pascals,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let row = id.substance();
    let key = row.key();
    let columns = row.liquid().ok_or(ResolveLiquidError::NotALiquid { key })?;
    let given = match composition {
        Composition::Reference => None,
        Composition::Given(given) => Some(checked_composition(key, columns, given)?),
    };
    match &columns.constitution {
        LiquidConstitution::Pure { .. } | LiquidConstitution::AwaitingSolute(_) => {
            pure_record(row, columns, temperature, pressure)
        }
        LiquidConstitution::Solution(solution) => {
            let composition = given.unwrap_or(LiquidComposition::of_reference(solution.reference));
            solution::solution_record(row, solution, composition, temperature)
        }
        LiquidConstitution::Melt(melt) => melt::melt_record(row, melt, temperature),
    }
}

/// `given`, checked for the row `key` with `columns` in decision-p14-t49h-step3-composition
/// §2.3's order: the row has components, each component given is one of them, and each fraction is
/// in its model's range. A pure liquid's `given` passing these is the row at 1
/// ([`LiquidComposition`] names each component once and sums to 1), its reference.
///
/// # Errors
///
/// [`ResolveLiquidError::CompositionNotModelled`], [`ResolveLiquidError::ComponentNotModelled`] and
/// [`ResolveLiquidError::CompositionOutOfRange`], each for the first offence in `given`'s order.
fn checked_composition<'a>(
    key: SubstanceKey,
    columns: &LiquidColumns,
    given: LiquidComposition<'a>,
) -> Result<LiquidComposition<'a>, ResolveLiquidError> {
    let components = columns.components();
    if components.is_empty() {
        return Err(ResolveLiquidError::CompositionNotModelled { key });
    }
    if let Some(&(stranger, _)) = given
        .mole_fractions()
        .iter()
        .find(|(id, _)| !components.contains(id))
    {
        return Err(ResolveLiquidError::ComponentNotModelled {
            key,
            component: stranger.substance().key(),
        });
    }
    if let LiquidConstitution::Solution(solution) = &columns.constitution {
        for &(id, mole_fraction) in given.mole_fractions() {
            let (lowest, highest) = solution.mole_fraction_range(id)?;
            if mole_fraction < lowest - solution::MOLE_FRACTION_SLACK
                || mole_fraction > highest + solution::MOLE_FRACTION_SLACK
            {
                return Err(ResolveLiquidError::CompositionOutOfRange {
                    key,
                    component: id.substance().key(),
                    mole_fraction,
                    lowest,
                    highest,
                });
            }
        }
    }
    Ok(given)
}

/// A liquid's record from its own columns, at `temperature` ([`liquid_properties`]): a pure
/// liquid's, its range ending below its own critical point; or a solution's whose solute has no
/// row, at its reference, its range ending below its solvent's (decision-p14-t49h-step3-composition
/// §3: the brines' salts have no phase diagram), so that above it the record is
/// [`ResolveLiquidError::OutOfRange`] before any member is [`ResolveLiquidError::NotModelled`].
/// Its density is compressed to `pressure` ([`compressed`]; none at or below the vapour's), its
/// basis the saturated density's, and every member read at the record's density reads the
/// compressed one.
fn pure_record(
    row: &Substance,
    columns: &LiquidColumns,
    temperature: Kelvin,
    pressure: Pascals,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let key = row.key();
    let condensed = row.condensed().ok_or(ResolveLiquidError::NotModelled {
        key,
        member: LiquidMember::Density,
    })?;
    let lowest = condensed.transition().temperature();
    let range_row = match &columns.constitution {
        LiquidConstitution::AwaitingSolute(solute) => solute.solvent.substance(),
        LiquidConstitution::Pure { .. }
        | LiquidConstitution::Solution(_)
        | LiquidConstitution::Melt(_) => row,
    };
    let below = range_row
        .phase_diagram()
        .map(PhaseDiagram::critical_temperature);
    let finite = temperature.value().is_finite();
    if !finite || temperature < lowest || below.is_some_and(|critical| temperature >= critical) {
        return Err(ResolveLiquidError::OutOfRange {
            key,
            temperature,
            lowest,
            below,
        });
    }
    let (saturated, density_basis) =
        record_density(row, temperature).ok_or(ResolveLiquidError::NotModelled {
            key,
            member: LiquidMember::Density,
        })?;
    let density = compressed(row, saturated, temperature, pressure);
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
            let depolarisation = scattering.depolarisation_ratio(density, temperature);
            basis = basis.max(scattering.basis_at(refractive_index[0], density, temperature));
            pure_liquid_scattering_per_m(&refractive_index, temperature, kappa, depolarisation)
        })
    });
    // A correlation can give no tension at a state: methane's Mulero sum, whose last term is
    // negative, falls below 0 within about 0.18 K of its critical point. It is absent there, never
    // 0 or negative.
    let surface_tension_n_per_m = columns.surface_tension.and_then(|equation| {
        let sigma = equation.surface_tension_n_per_m(temperature);
        (sigma > 0.0).then(|| {
            basis = basis.max(equation.basis_at(temperature));
            sigma
        })
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
