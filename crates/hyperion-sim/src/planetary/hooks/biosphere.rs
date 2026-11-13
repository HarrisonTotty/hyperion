//! The biosphere hook (plan 14, P14.T53.d; decision-composition §1.6): a biosphere's net surface
//! fluxes ([`Biosphere`]), the chemistry that takes them as sources ([`BioticAir`]), and the
//! palette's cover with its `biological` flag ([`Cover`]).
//!
//! # No rule here makes life
//!
//! A record's surface section holds its biosphere as a member that stays
//! [`Modelled::NotModelled`](crate::planetary::record::Modelled::NotModelled) until a life plan
//! exists (the galaxy brainstorm: life "keeps its habitability hook and … nothing more"), so every
//! generated body is abiotic and its member absent from the wire. Nothing here is called by the
//! record: the module is pure, for the life plan and its tests.
//!
//! # The fluxes
//!
//! Each flux is a gas's net surface flux in units of Earth's present biotic flux of it, as a mean
//! over the whole surface: the unit is Earth's global flux over Earth's area
//! ([`EARTH_SURFACE_AREA_M2`]), so that a world's own area never enters. They are finite and of
//! either sign, negative for a net sink:
//!
//! - **O₂** ([`EARTH_OXYGEN_FLUX_MOL_PER_YR`]): the O₂ the burial of organic carbon leaves behind,
//!   photosynthesis's net of respiration;
//! - **CH₄** ([`EARTH_METHANE_FLUX_MOL_PER_YR`]);
//! - **N₂O** ([`EARTH_NITROUS_OXIDE_FLUX_MOL_PER_YR`]).
//!
//! # The chemistry with fluxes given ([`BioticAir::derive`])
//!
//! The fluxes are sources into a world's abiotic air, its record's surface conditions, each against
//! a sink that is stated, and a flux whose sink no input states is refused
//! ([`DeriveBioticAirError::NoSink`], [`DeriveBioticAirError::SinkOutOfRange`]):
//!
//! - **O₂'s steady state against its sinks** ([`OxygenSinks`]), which the world states, since its
//!   geology sets them: the reductants its interior outgasses, which consume O₂ whatever its level,
//!   and the oxidative weathering of its crust's reduced carbon, which goes as the O₂ level's square
//!   root (COPSE's law). An O₂ flux with no sinks stated is refused; one at or below the reductants
//!   leaves the air anoxic (Claire, Catling and Zahnle 2006: oxygen rises when the reducing gases'
//!   flux "falls below the flux of organic carbon burial that produces O₂").
//! - **O₃ from O₂ by P14.T24.c's rule** ([`ozone_column`]): `N_⊕` F(r) U(star), r the air's O₂
//!   column over Earth's present one.
//! - **CH₄'s lifetime in an oxic air** ([`methane_lifetime`]): Earth's present lifetime scaled by
//!   Cooke et al. 2022's ratios against the O₂ level, from 10⁻³ to 1.5 PAL. A CH₄ flux into an air
//!   outside that range, an anoxic one among them, is refused: its sink there, photolysis against
//!   hydrogen escape, is not stated here.
//! - **N₂O's lifetime** ([`nitrous_oxide_lifetime`]): Earth's present, near Earth's O₂ level alone.
//!
//! Each gas's steady-state column is its own, added to whatever of it the abiotic air holds, which
//! is kept and not re-equilibrated with the biotic air (so a world's abiotic O₂, a desiccated
//! world's, is added to the regulated level, not drawn into it: the life brainstorm's lean). The
//! air keeps its abiotic column of every other gas, and its surface pressure rises by the added
//! gases' weight.
//!
//! # What is not modelled
//!
//! - O₂'s bistability (Goldblatt, Lenton and Watson 2006): the ozone shield's feedback on O₂'s
//!   lifetime is not in the sinks, so the steady state is one root, rising from 0 as the flux passes
//!   the reductants.
//! - The pyrite cycle's O₂, and the weathering's dependence on uplift, land and climate: a world's
//!   sinks state them only through their two terms.
//! - CH₄'s lifetime's feedback on itself (more CH₄, less OH): Cooke et al.'s ratios were taken at a
//!   fixed 0.8 ppmv of surface CH₄, and are valid only near it. A flux that holds far more is not
//!   refused, its lifetime extrapolated (Earth's own holds about 1.5 ppm): recorded.
//! - N₂O's lifetime away from Earth's O₂ level and about other stars (Segura et al. 2005), and any
//!   anoxic steady state: those fluxes are refused.
//! - The seasons: each flux is an annual mean.
//!
//! # The cover
//!
//! The palette's [`PaletteRole::Cover`](hyperion_surface::field::PaletteRole::Cover) role holds a
//! cover over the ground that is not its rock: an organic cover, P14.T51.d's tholin, or one a life
//! plan makes. [`Cover`] carries the `biological` flag beside the role's substance on the server
//! alone: the surface payload's layout froze at generator version 24's first send, and the flag
//! would state a biosphere to the client outright (the life brainstorm's request 3), so the
//! palette's entries are unchanged.
//!
//! # Determinism
//!
//! Closed forms, every transcendental through [`math`], every table read in its order: the same
//! bits on every target.

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::hooks::aerosols::{AVOGADRO, PRESENT_OXYGEN_COLUMN, log_log, ozone_column};
use crate::substance::{self, SubstanceId};
use crate::units::{Kelvin, MetresPerSecondSquared, Pascals, Years, consts};

/// Grams per kilogram.
const GRAMS_PER_KILOGRAM: f64 = 1_000.0;

/// Earth's surface area, m²: 4π R⊕², R⊕ its volumetric mean radius
/// ([`consts::EARTH_RADIUS_M`]), 5.1006 × 10¹⁴. A flux in Earth's units is Earth's global flux over
/// it.
pub const EARTH_SURFACE_AREA_M2: f64 =
    4.0 * core::f64::consts::PI * consts::EARTH_RADIUS_M * consts::EARTH_RADIUS_M;

/// Earth's present net biotic O₂ flux, mol yr⁻¹: 9.0 × 10¹², the O₂ the burial of organic carbon
/// leaves behind, COPSE's present burial on land and in the sea, 4.5 × 10¹² mol C yr⁻¹ each
/// (Bergman, Lenton and Watson 2004, Am. J. Sci. 304, 397; from memory, not re-read: recorded).
/// The unit of a biosphere's O₂ flux, and of [`OxygenSinks`].
pub const EARTH_OXYGEN_FLUX_MOL_PER_YR: f64 = 9.0e12;

/// Earth's present biotic CH₄ flux, mol yr⁻¹: 3.0 × 10¹³, Thompson et al. 2022's "about 30
/// Tmol/yr" of emissions, "predominantly produced directly by life (including anthropogenic
/// sources)" (PNAS 119, e2117933119; the life brainstorm's reading, verified in the source's own
/// text). The unit of a biosphere's CH₄ flux.
pub const EARTH_METHANE_FLUX_MOL_PER_YR: f64 = 3.0e13;

/// Earth's present biotic N₂O flux, mol yr⁻¹: 6.07 × 10¹¹, Tian et al. 2020's 17.0 Tg N yr⁻¹, their
/// bottom-up mean for 2007–2016, most of it microbial, of natural and agricultural soils and the
/// ocean (Nature 586, 248; from memory, not re-read: recorded), at 28.013 g of nitrogen a mole of
/// N₂O. The unit of a biosphere's N₂O flux.
pub const EARTH_NITROUS_OXIDE_FLUX_MOL_PER_YR: f64 = 17.0e12 / 28.013;

/// The exponent of the oxidative weathering's dependence on the O₂ level, a: 0.5, COPSE's
/// (O ÷ O₀)^½ for the weathering of organic carbon (Bergman, Lenton and Watson 2004; from memory,
/// not re-read: recorded).
pub const OXIDATIVE_WEATHERING_EXPONENT: f64 = 0.5;

/// CH₄'s present lifetime in Earth's air: 9.1 years, its total atmospheric lifetime, against OH
/// chiefly, soils and the stratosphere besides (Prather, Holmes and Hsu 2012, Geophys. Res. Lett.
/// 39, L09803: 9.1 ± 0.9 years; from memory, not re-read: recorded).
pub const PRESENT_METHANE_LIFETIME: Years = Years::new(9.1);

/// CH₄'s lifetime against the O₂ level (PAL), over its value at 1 PAL: Cooke et al. 2022's surface
/// lifetimes from their three-dimensional runs (R. Soc. Open Sci. 9, 211165, Table 2; the life
/// brainstorm's reading, verified in the source's own text), each over their pre-industrial
/// 1 PAL's, taken at a fixed 0.8 ppmv of surface CH₄. Thinner ozone lets more ultraviolet make OH,
/// so less O₂ shortens methane's life. Linear in ln ratio against ln level between the nodes; the
/// lifetime is stated from the first node to the last alone ([`methane_lifetime`]). The ratios of a
/// pre-industrial lifetime against OH scale the present total one ([`PRESENT_METHANE_LIFETIME`]):
/// a stated approximation.
pub const METHANE_LIFETIME_BY_OXYGEN: [(f64, f64); 8] = [
    (1e-3, 0.010),
    (5e-3, 0.033),
    (1e-2, 0.067),
    (5e-2, 0.52),
    (0.1, 0.70),
    (0.5, 0.903),
    (1.0, 1.0),
    (1.5, 1.061),
];

/// N₂O's present lifetime in Earth's air: 116 years, against photolysis and O(¹D) in the
/// stratosphere (Prather et al. 2015, J. Geophys. Res. Atmos. 120, 5693: 116 ± 9 years; from
/// memory, not re-read: recorded).
pub const PRESENT_NITROUS_OXIDE_LIFETIME: Years = Years::new(116.0);

/// The O₂ levels (PAL) over which N₂O's present lifetime is taken: 0.5 to 1.5, where Cooke et al.'s
/// CH₄ lifetime ([`METHANE_LIFETIME_BY_OXYGEN`]) stays within 10% of its present one, so that the
/// air's ultraviolet and oxidants are taken as Earth's (the builder's choice, stated).
pub const NITROUS_OXIDE_OXYGEN_RANGE: (f64, f64) = (0.5, 1.5);

/// The registry row whose key is `key`, a row this module names.
///
/// # Panics
///
/// For a key the registry does not hold, which no caller names.
#[must_use]
fn row(key: &str) -> SubstanceId {
    substance::by_key(key).unwrap_or_else(|| panic!("{key} is a registry row"))
}

/// A biosphere's net surface fluxes (P14.T53.d): each a gas's, in units of Earth's present biotic
/// flux of it ([module](self)), finite and of either sign, each gas once, in the order of their
/// keys. No rule in plan 14 makes one: a record's member is not modelled until a life plan exists.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::hooks::biosphere::{BuildBiosphereError, Biosphere};
/// use hyperion_sim::substance::by_key;
///
/// let gas = |key| by_key(key).expect("a registry gas");
/// // Earth's, in Earth's units.
/// let earth = Biosphere::new([(gas("O2"), 1.0), (gas("CH4"), 1.0), (gas("N2O"), 1.0)])?;
/// assert!((earth.flux(gas("O2")) - 1.0).abs() < 1e-12);
/// // A gas it does not exchange has no flux.
/// assert!(earth.flux(gas("NH3")).abs() < 1e-300);
/// # Ok::<(), BuildBiosphereError>(())
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Biosphere {
    fluxes: Vec<(SubstanceId, f64)>,
}

/// Why [`Biosphere::new`] refused its fluxes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildBiosphereError {
    /// The species is not a registry row that can be a gas with a molar mass.
    NotAGas(SubstanceId),
    /// The flux is not finite.
    NotFinite {
        /// The gas.
        species: SubstanceId,
        /// Its flux as given, in Earth's units.
        flux_earth: f64,
    },
    /// The gas is given twice.
    Repeated(SubstanceId),
}

impl fmt::Display for BuildBiosphereError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAGas(species) => write!(
                f,
                "{} is not a gas with a molar mass",
                species.substance().key()
            ),
            Self::NotFinite {
                species,
                flux_earth,
            } => write!(
                f,
                "the flux of {} ({flux_earth} of Earth's) is not finite",
                species.substance().key()
            ),
            Self::Repeated(species) => {
                write!(
                    f,
                    "the flux of {} is given twice",
                    species.substance().key()
                )
            }
        }
    }
}

impl Error for BuildBiosphereError {}

impl Biosphere {
    /// The biosphere of the net surface fluxes `fluxes`, each a gas's in units of Earth's present
    /// biotic flux of it, kept in the order of their keys (a −0 as +0).
    ///
    /// # Errors
    ///
    /// [`BuildBiosphereError::NotAGas`] for a species that is not a registry row that can be a gas
    /// with a molar mass, [`BuildBiosphereError::NotFinite`] for a flux that is not finite, and
    /// [`BuildBiosphereError::Repeated`] for a gas given twice.
    pub fn new(
        fluxes: impl IntoIterator<Item = (SubstanceId, f64)>,
    ) -> Result<Self, BuildBiosphereError> {
        let mut checked: Vec<(SubstanceId, f64)> = Vec::new();
        for (species, flux_earth) in fluxes {
            let row = species.substance();
            if row.gas().is_none() || row.molar_mass_g_per_mol().is_none() {
                return Err(BuildBiosphereError::NotAGas(species));
            }
            if !flux_earth.is_finite() {
                return Err(BuildBiosphereError::NotFinite {
                    species,
                    flux_earth,
                });
            }
            if checked.iter().any(|&(id, _)| id == species) {
                return Err(BuildBiosphereError::Repeated(species));
            }
            checked.push((species, if flux_earth == 0.0 { 0.0 } else { flux_earth }));
        }
        checked.sort_by_key(|&(id, _)| id.substance().key());
        Ok(Self { fluxes: checked })
    }

    /// The net surface fluxes, each a gas's in units of Earth's present biotic flux of it, in the
    /// order of their keys.
    #[must_use]
    pub fn fluxes(&self) -> &[(SubstanceId, f64)] {
        &self.fluxes
    }

    /// The net surface flux of `species`, in units of Earth's present biotic flux of it; 0 for a
    /// gas the biosphere does not exchange.
    #[must_use]
    pub fn flux(&self, species: SubstanceId) -> f64 {
        self.fluxes
            .iter()
            .find(|&&(id, _)| id == species)
            .map_or(0.0, |&(_, flux)| flux)
    }
}

/// A world's O₂ sinks (P14.T53.d), which its geology sets, each in units of Earth's present net
/// biotic O₂ flux ([`EARTH_OXYGEN_FLUX_MOL_PER_YR`]) over the whole surface: the reductants its
/// interior outgasses, which consume O₂ whatever its level while the air is oxic, and the oxidative
/// weathering of its crust's reduced carbon at Earth's present O₂ level, which goes as the level's
/// [`OXIDATIVE_WEATHERING_EXPONENT`] power.
///
/// The level is the air's O₂ column over Earth's present one, PAL, as COPSE's O ÷ O₀ is the
/// reservoir's and P14.T24.c's ozone rule reads it; on Earth it is the partial pressure's.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct OxygenSinks {
    reductants: f64,
    weathering: f64,
}

/// Why [`OxygenSinks::new`] refused its sinks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildOxygenSinksError {
    /// The reductants' sink is not finite and at least 0.
    Reductants(f64),
    /// The weathering's sink is not finite and positive.
    Weathering(f64),
}

impl fmt::Display for BuildOxygenSinksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reductants(sink) => {
                write!(
                    f,
                    "the reductants' sink {sink} is not finite and at least 0"
                )
            }
            Self::Weathering(sink) => {
                write!(f, "the weathering's sink {sink} is not finite and positive")
            }
        }
    }
}

impl Error for BuildOxygenSinksError {}

impl OxygenSinks {
    /// Earth's present O₂ sinks, COPSE's (Bergman, Lenton and Watson 2004; from memory, not
    /// re-read: recorded), which balance its present burial of organic carbon: the degassing of
    /// reduced carbon, 1.25 × 10¹² mol yr⁻¹, as the reductants, and the oxidative weathering of
    /// organic carbon, 7.75 × 10¹² mol yr⁻¹ at the present level, each over
    /// [`EARTH_OXYGEN_FLUX_MOL_PER_YR`].
    pub const EARTH: Self = Self {
        reductants: 1.25e12 / EARTH_OXYGEN_FLUX_MOL_PER_YR,
        weathering: 7.75e12 / EARTH_OXYGEN_FLUX_MOL_PER_YR,
    };

    /// The sinks of a world whose reductants consume `reductants` and whose weathering consumes
    /// `weathering` at Earth's present O₂ level, each in units of Earth's present net biotic O₂
    /// flux (a −0 as +0).
    ///
    /// # Errors
    ///
    /// [`BuildOxygenSinksError::Reductants`] for reductants that are not finite and at least 0, and
    /// [`BuildOxygenSinksError::Weathering`] for a weathering sink that is not finite and positive.
    pub fn new(reductants: f64, weathering: f64) -> Result<Self, BuildOxygenSinksError> {
        if !(reductants.is_finite() && reductants >= 0.0) {
            return Err(BuildOxygenSinksError::Reductants(reductants));
        }
        if !(weathering.is_finite() && weathering > 0.0) {
            return Err(BuildOxygenSinksError::Weathering(weathering));
        }
        Ok(Self {
            reductants: if reductants == 0.0 { 0.0 } else { reductants },
            weathering,
        })
    }

    /// The reductants' sink, in units of Earth's present net biotic O₂ flux, at least 0.
    #[must_use]
    pub const fn reductants(&self) -> f64 {
        self.reductants
    }

    /// The weathering's sink at Earth's present O₂ level, in units of Earth's present net biotic O₂
    /// flux, positive.
    #[must_use]
    pub const fn weathering(&self) -> f64 {
        self.weathering
    }

    /// The O₂ level (PAL, by column) at which these sinks balance a net biotic O₂ flux of
    /// `flux_earth` (in Earth's units): r = ((F − R) ÷ W)^(1/a), R the reductants, W the
    /// weathering and a [`OXIDATIVE_WEATHERING_EXPONENT`]; 0, an anoxic air, for a flux at or below
    /// the reductants, and for a NaN.
    #[must_use]
    pub fn steady_level(&self, flux_earth: f64) -> f64 {
        let excess = flux_earth - self.reductants;
        if excess.is_nan() || excess <= 0.0 {
            return 0.0;
        }
        math::powf(
            excess / self.weathering,
            1.0 / OXIDATIVE_WEATHERING_EXPONENT,
        )
    }
}

/// CH₄'s lifetime in an air of O₂ level `level` (PAL, by column): [`PRESENT_METHANE_LIFETIME`]
/// times [`METHANE_LIFETIME_BY_OXYGEN`]'s ratio there, linear in ln ratio against ln level between
/// its nodes; `None`, not stated, outside the table's 10⁻³ to 1.5 PAL, an anoxic air's among them,
/// and for a NaN.
#[must_use]
pub fn methane_lifetime(level: f64) -> Option<Years> {
    let nodes = &METHANE_LIFETIME_BY_OXYGEN;
    let (first, last) = (nodes[0], nodes[nodes.len() - 1]);
    if !(level >= first.0 && level <= last.0) {
        return None;
    }
    Some(Years::new(
        PRESENT_METHANE_LIFETIME.value() * log_log(nodes, level),
    ))
}

/// N₂O's lifetime in an air of O₂ level `level` (PAL, by column): [`PRESENT_NITROUS_OXIDE_LIFETIME`]
/// within [`NITROUS_OXIDE_OXYGEN_RANGE`]; `None`, not stated, outside it, and for a NaN.
#[must_use]
pub fn nitrous_oxide_lifetime(level: f64) -> Option<Years> {
    let (low, high) = NITROUS_OXIDE_OXYGEN_RANGE;
    (level >= low && level <= high).then_some(PRESENT_NITROUS_OXIDE_LIFETIME)
}

/// The abiotic air a biosphere's fluxes enter, and the sinks its world states, as plain values
/// (P14.T53.d): a record's surface conditions', its bulk section's gravity and its host's
/// temperature.
///
/// The fields are public, as P14.T24.c's inputs are; [`BioticAir::derive`] checks every one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BioticAirInputs<'a> {
    /// The air's surface pressure, finite and positive: a biosphere's gases need an air to enter.
    pub surface_pressure: Pascals,
    /// Its gases at the ground as mole fractions (P14.T24.a's), each finite and within 0–1.
    pub gases: &'a [(SubstanceId, f64)],
    /// Its mean molar mass, g mol⁻¹, finite and positive.
    pub mean_molar_mass_g_per_mol: f64,
    /// The surface gravity, finite and positive.
    pub gravity: MetresPerSecondSquared,
    /// The host star's effective temperature, which P14.T24.c's ozone rule reads, finite and
    /// positive; `None` for an unlit body, which makes no ozone.
    pub star_temperature: Option<Kelvin>,
    /// The world's O₂ sinks; `None` where it states none, so that an O₂ flux is refused.
    pub oxygen_sinks: Option<OxygenSinks>,
}

/// Why [`BioticAir::derive`] refused its biosphere or its air.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveBioticAirError {
    /// The surface pressure is not finite and positive.
    SurfacePressure(Pascals),
    /// The mean molar mass is not finite and positive.
    MolarMass {
        /// The molar mass as given, g mol⁻¹.
        molar_mass_g_per_mol: f64,
    },
    /// The gravity is not finite and positive.
    Gravity(MetresPerSecondSquared),
    /// The star's temperature is not finite and positive.
    StarTemperature(Kelvin),
    /// A gas's mole fraction is not finite and within 0–1.
    MoleFraction {
        /// The gas.
        species: SubstanceId,
        /// Its mole fraction as given.
        fraction: f64,
    },
    /// A gas's flux has no sink stated: an O₂ flux with no [`OxygenSinks`] given, or a flux of a
    /// gas this chemistry states no sink of (any but O₂, CH₄ and N₂O).
    NoSink {
        /// The gas.
        species: SubstanceId,
    },
    /// A gas's sink is stated only over a range of O₂ levels ([`methane_lifetime`],
    /// [`nitrous_oxide_lifetime`]), and the air's level lies outside it: an anoxic air's, among
    /// others.
    SinkOutOfRange {
        /// The gas.
        species: SubstanceId,
        /// The air's O₂ level, PAL by column.
        oxygen_level: f64,
    },
    /// A net sink (a negative flux) of a gas whose only stated sink is its lifetime, which no
    /// steady state with a source this chemistry knows holds.
    NetSink {
        /// The gas.
        species: SubstanceId,
        /// Its flux as given, in Earth's units.
        flux_earth: f64,
    },
}

impl fmt::Display for DeriveBioticAirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SurfacePressure(p) => write!(
                f,
                "surface pressure {} Pa is not finite and positive",
                p.value()
            ),
            Self::MolarMass {
                molar_mass_g_per_mol,
            } => write!(
                f,
                "mean molar mass {molar_mass_g_per_mol} g/mol is not finite and positive"
            ),
            Self::Gravity(g) => write!(f, "gravity {} m/s² is not finite and positive", g.value()),
            Self::StarTemperature(t) => write!(
                f,
                "star temperature {} K is not finite and positive",
                t.value()
            ),
            Self::MoleFraction { species, fraction } => write!(
                f,
                "the mole fraction {fraction} of {} is not within 0 to 1",
                species.substance().key()
            ),
            Self::NoSink { species } => write!(
                f,
                "the flux of {} has no sink stated",
                species.substance().key()
            ),
            Self::SinkOutOfRange {
                species,
                oxygen_level,
            } => write!(
                f,
                "the sink of {} is not stated at an O₂ level of {oxygen_level} PAL",
                species.substance().key()
            ),
            Self::NetSink {
                species,
                flux_earth,
            } => write!(
                f,
                "the net sink of {} ({flux_earth} of Earth's flux) has no source to draw on",
                species.substance().key()
            ),
        }
    }
}

impl Error for DeriveBioticAirError {}

/// A world's air with its biosphere's fluxes in it (P14.T53.d): its gases at the ground, its surface
/// pressure and mean molar mass, its O₂ level and its ozone column, each gas's column its abiotic
/// air's with the biosphere's steady state added ([module](self)).
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct BioticAir {
    gases: Vec<(SubstanceId, f64)>,
    surface_pressure: Pascals,
    mean_molar_mass_g_per_mol: f64,
    oxygen_level: f64,
    ozone_column_per_m2: f64,
}

impl BioticAir {
    /// The air of `inputs` with the fluxes of `biosphere` as sources against their stated sinks
    /// ([module](self)): O₂ at its sinks' steady level, CH₄ and N₂O each at its flux times its
    /// lifetime at the air's O₂ level, each column added to the abiotic air's; and the ozone column
    /// of the air's O₂ by P14.T24.c's rule. A flux of 0 needs no sink.
    ///
    /// # Errors
    ///
    /// - [`DeriveBioticAirError::SurfacePressure`], [`DeriveBioticAirError::MolarMass`],
    ///   [`DeriveBioticAirError::Gravity`], [`DeriveBioticAirError::StarTemperature`] and
    ///   [`DeriveBioticAirError::MoleFraction`] for an input out of its range
    ///   ([`BioticAirInputs`]);
    /// - [`DeriveBioticAirError::NoSink`] for an O₂ flux with no [`OxygenSinks`] given, and for a
    ///   flux of any gas but O₂, CH₄ and N₂O;
    /// - [`DeriveBioticAirError::SinkOutOfRange`] for a CH₄ or N₂O flux into an air whose O₂ level
    ///   lies outside the range its lifetime is stated over;
    /// - [`DeriveBioticAirError::NetSink`] for a negative CH₄ or N₂O flux.
    ///
    /// # Panics
    ///
    /// Never: O₂, CH₄ and N₂O are registry rows with molar masses.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::hooks::biosphere::{
    ///     Biosphere, BioticAir, BioticAirInputs, DeriveBioticAirError, OxygenSinks,
    /// };
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
    ///
    /// let gas = |key| by_key(key).expect("a registry gas");
    /// // An abiotic Earth-like air of nitrogen alone, about a solar twin.
    /// let gases = [(gas("N2"), 1.0)];
    /// let mut inputs = BioticAirInputs {
    ///     surface_pressure: Pascals::new(8.0e4),
    ///     gases: &gases,
    ///     mean_molar_mass_g_per_mol: 28.013,
    ///     gravity: MetresPerSecondSquared::new(9.81),
    ///     star_temperature: Some(Kelvin::new(5_772.0)),
    ///     oxygen_sinks: Some(OxygenSinks::EARTH),
    /// };
    /// let earth = Biosphere::new([(gas("O2"), 1.0), (gas("CH4"), 1.0)]).expect("Earth's fluxes");
    /// let air = BioticAir::derive(&earth, &inputs)?;
    /// // Earth's flux against Earth's sinks holds Earth's present O₂.
    /// assert!((air.oxygen_level() - 1.0).abs() < 1e-12);
    /// assert!(air.fraction(gas("CH4")) > 0.0);
    ///
    /// // The same O₂ with no sinks stated is refused.
    /// inputs.oxygen_sinks = None;
    /// assert_eq!(
    ///     BioticAir::derive(&earth, &inputs),
    ///     Err(DeriveBioticAirError::NoSink { species: gas("O2") })
    /// );
    /// # Ok::<(), DeriveBioticAirError>(())
    /// ```
    pub fn derive(
        biosphere: &Biosphere,
        inputs: &BioticAirInputs<'_>,
    ) -> Result<Self, DeriveBioticAirError> {
        let (p_s, mu, g) = inputs.check()?;
        let (oxygen, methane, nitrous_oxide) = (row("O2"), row("CH4"), row("N2O"));
        check_sinks(biosphere, inputs)?;

        // The abiotic air's column, molecules m⁻², and each gas's share of it.
        let air_column = p_s * AVOGADRO / (mu / GRAMS_PER_KILOGRAM * g);
        let mut columns: Vec<(SubstanceId, f64)> = inputs
            .gases
            .iter()
            .map(|&(id, x)| (id, x * air_column))
            .collect();
        let own = |species: SubstanceId| {
            columns
                .iter()
                .find(|&&(id, _)| id == species)
                .map_or(0.0, |&(_, column)| column)
        };

        let biotic_oxygen = inputs.oxygen_sinks.map_or(0.0, |sinks| {
            sinks.steady_level(biosphere.flux(oxygen)) * PRESENT_OXYGEN_COLUMN
        });
        let oxygen_level = (own(oxygen) + biotic_oxygen) / PRESENT_OXYGEN_COLUMN;
        // Each lifetime-limited gas's steady column, its flux (mol m⁻² yr⁻¹) times its lifetime
        // (yr), in molecules m⁻².
        let steady = |species: SubstanceId, lifetime: Option<Years>, earth_flux: f64| {
            let flux_earth = biosphere.flux(species);
            if flux_earth == 0.0 {
                return Ok(0.0);
            }
            let lifetime = lifetime.ok_or(DeriveBioticAirError::SinkOutOfRange {
                species,
                oxygen_level,
            })?;
            Ok(flux_earth * earth_flux / EARTH_SURFACE_AREA_M2 * lifetime.value() * AVOGADRO)
        };
        let added = [
            (oxygen, biotic_oxygen),
            (
                methane,
                steady(
                    methane,
                    methane_lifetime(oxygen_level),
                    EARTH_METHANE_FLUX_MOL_PER_YR,
                )?,
            ),
            (
                nitrous_oxide,
                steady(
                    nitrous_oxide,
                    nitrous_oxide_lifetime(oxygen_level),
                    EARTH_NITROUS_OXIDE_FLUX_MOL_PER_YR,
                )?,
            ),
        ];

        // The added gases' weight, kg m⁻², and their molecules.
        let (mut weight, mut molecules) = (0.0, 0.0);
        for (species, column) in added {
            if column <= 0.0 {
                continue;
            }
            let molar_mass = species
                .substance()
                .molar_mass_g_per_mol()
                .expect("O₂, CH₄ and N₂O have molar masses");
            weight += column / AVOGADRO * molar_mass / GRAMS_PER_KILOGRAM;
            molecules += column;
            match columns.iter_mut().find(|(id, _)| *id == species) {
                Some((_, own)) => *own += column,
                None => columns.push((species, column)),
            }
        }
        let total = air_column + molecules;
        let mean_molar_mass_g_per_mol =
            (air_column * mu + weight * GRAMS_PER_KILOGRAM * AVOGADRO) / total;
        let gases = columns
            .into_iter()
            .map(|(id, column)| (id, column / total))
            .collect();
        let ozone_column_per_m2 = inputs
            .star_temperature
            .map_or(0.0, |star| ozone_column(oxygen_level, star));
        Ok(Self {
            gases,
            surface_pressure: Pascals::new(p_s + weight * g),
            mean_molar_mass_g_per_mol,
            oxygen_level,
            ozone_column_per_m2,
        })
    }

    /// The gases at the ground as mole fractions: the abiotic air's, in its order, then each gas
    /// the biosphere adds that it lacked, O₂, CH₄ and N₂O in that order.
    #[must_use]
    pub fn gases(&self) -> &[(SubstanceId, f64)] {
        &self.gases
    }

    /// The mole fraction of `species` at the ground; 0 for a gas the air lacks.
    #[must_use]
    pub fn fraction(&self, species: SubstanceId) -> f64 {
        self.gases
            .iter()
            .find(|&&(id, _)| id == species)
            .map_or(0.0, |&(_, x)| x)
    }

    /// The partial pressure of `species` at the ground, its mole fraction times the surface
    /// pressure (Dalton's law).
    #[must_use]
    pub fn partial_pressure(&self, species: SubstanceId) -> Pascals {
        Pascals::new(self.fraction(species) * self.surface_pressure.value())
    }

    /// The surface pressure: the abiotic air's, with the added gases' weight.
    #[must_use]
    pub const fn surface_pressure(&self) -> Pascals {
        self.surface_pressure
    }

    /// The mean molar mass, g mol⁻¹.
    #[must_use]
    pub const fn mean_molar_mass_g_per_mol(&self) -> f64 {
        self.mean_molar_mass_g_per_mol
    }

    /// The O₂ level, PAL: the air's O₂ column over Earth's present one
    /// ([`PRESENT_OXYGEN_COLUMN`]), its abiotic O₂ with its biosphere's.
    #[must_use]
    pub const fn oxygen_level(&self) -> f64 {
        self.oxygen_level
    }

    /// The ozone column, molecules m⁻², by P14.T24.c's rule ([`ozone_column`]) at the air's O₂
    /// level; 0 with no O₂ or no star.
    #[must_use]
    pub const fn ozone_column_per_m2(&self) -> f64 {
        self.ozone_column_per_m2
    }
}

/// Checks that every non-zero flux of `biosphere` has a sink `inputs` state or this chemistry
/// states ([`BioticAir::derive`]'s `NoSink` and `NetSink`; its `SinkOutOfRange` needs the air's
/// O₂ level, and is its own).
fn check_sinks(
    biosphere: &Biosphere,
    inputs: &BioticAirInputs<'_>,
) -> Result<(), DeriveBioticAirError> {
    let (oxygen, methane, nitrous_oxide) = (row("O2"), row("CH4"), row("N2O"));
    for &(species, flux_earth) in biosphere.fluxes() {
        if flux_earth == 0.0 {
            continue;
        }
        if species == oxygen {
            if inputs.oxygen_sinks.is_none() {
                return Err(DeriveBioticAirError::NoSink { species });
            }
        } else if species == methane || species == nitrous_oxide {
            if flux_earth < 0.0 {
                return Err(DeriveBioticAirError::NetSink {
                    species,
                    flux_earth,
                });
            }
        } else {
            return Err(DeriveBioticAirError::NoSink { species });
        }
    }
    Ok(())
}

impl BioticAirInputs<'_> {
    /// Checks every field, giving the surface pressure in Pa, the mean molar mass in g mol⁻¹ and
    /// the gravity in m s⁻².
    fn check(&self) -> Result<(f64, f64, f64), DeriveBioticAirError> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        let p_s = self.surface_pressure.value();
        if !positive(p_s) {
            return Err(DeriveBioticAirError::SurfacePressure(self.surface_pressure));
        }
        let mu = self.mean_molar_mass_g_per_mol;
        if !positive(mu) {
            return Err(DeriveBioticAirError::MolarMass {
                molar_mass_g_per_mol: mu,
            });
        }
        let g = self.gravity.value();
        if !positive(g) {
            return Err(DeriveBioticAirError::Gravity(self.gravity));
        }
        if let Some(star) = self.star_temperature
            && !positive(star.value())
        {
            return Err(DeriveBioticAirError::StarTemperature(star));
        }
        for &(species, fraction) in self.gases {
            if !(fraction.is_finite() && (0.0..=1.0).contains(&fraction)) {
                return Err(DeriveBioticAirError::MoleFraction { species, fraction });
            }
        }
        Ok((p_s, mu, g))
    }
}

/// A cover over a body's ground that is not its rock, as the palette's
/// [`PaletteRole::Cover`](hyperion_surface::field::PaletteRole::Cover) role holds it
/// (decision-composition §1.6): its substance and its `biological` flag, on the server alone
/// ([module](self)'s "The cover").
///
/// Every cover the generator makes is abiotic, P14.T51.d's organic cover of `tholin`; a biological
/// one is a life plan's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cover {
    substance: SubstanceId,
    biological: bool,
}

impl Cover {
    /// An abiotic cover of `substance`, as the organic cover's `tholin`.
    #[must_use]
    pub const fn abiotic(substance: SubstanceId) -> Self {
        Self {
            substance,
            biological: false,
        }
    }

    /// A biological cover of `substance`, which only a life plan makes.
    #[must_use]
    pub const fn living(substance: SubstanceId) -> Self {
        Self {
            substance,
            biological: true,
        }
    }

    /// The cover's substance, its palette entry's.
    #[must_use]
    pub const fn substance(self) -> SubstanceId {
        self.substance
    }

    /// Whether the cover is biological: the `biological` flag.
    #[must_use]
    pub const fn is_biological(self) -> bool {
        self.biological
    }
}
