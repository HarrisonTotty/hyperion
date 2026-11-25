//! The vent's gas by the mantle's redox (plan 14, P14.T55.a.b, split from P14.T55.a): the species
//! a body's volcanic vent outgasses its carbon, hydrogen, nitrogen and sulphur as, at the oxygen
//! fugacity its melt buffers, and the element budget it hands the air.
//!
//! A pure module of plain quantities. P14.T55.a.c's carriers read it on the record from generator
//! version 36 (`OUTGASSING_SPECIATION_VERSION`, under `OutgassingRules`): by the route ruling
//! (research-leans-58), a secondary air publishes this gas, quenched as it erupts, not
//! re-equilibrated at a cool surface; its shares are the vent's, for what the vent outgasses,
//! never a conversion of the whole water inventory, whose water stays water.
//!
//! # The oxygen
//!
//! The melt sets the gas's oxygen fugacity, log f(O₂) = IW(T) + ΔIW, where IW is O'Neill's
//! iron–wüstite buffer ([`iw_log_fugacity`]) at the vent's temperature and ΔIW the mantle's
//! ([`ElementInventory::mantle_delta_iw`], P14.T50.a). The buffer is taken at 1 bar: its
//! pressure term, from the solids' volume change, is left out.
//!
//! # The ratios
//!
//! Each follows from an equilibrium constant of `substance::formation` ([`Reaction`]: NASA's CEA
//! records), the gas ideal, each species' fugacity its partial pressure:
//!
//! - H₂ + ½ O₂ ⇌ H₂O and CO + ½ O₂ ⇌ CO₂ give H₂O/H₂ and CO₂/CO, each K(T) f(O₂)^½, which do not
//!   depend on the pressure. So CO₂/CO measures the mantle's oxygen fugacity ("a powerful
//!   diagnostic", Tian and Heng 2024, ApJ 963, 157, arXiv:2301.10217, read). At 1 bar H₂ and CO
//!   lead below IW and H₂O and CO₂ above about IW+1 (Ortenzi et al. 2020, Sci. Rep. 10, 10907,
//!   Fig. 1, read; here CO₂ from IW+0.95 at 1,473 K and IW+1.3 at their 2,000 K). From IW−2 down CO carries over 95% of the carbon: below IW "almost all degassed carbon
//!   takes the form of CO" (Guimond et al. 2021, Phys. Earth Planet. Inter. 320, 106788,
//!   arXiv:2108.13438, abstract, read), which nearer IW also takes the melt's low carbonate
//!   solubility, not modelled here.
//! - CO + 3 H₂ ⇌ CH₄ + H₂O gives CH₄/CO = K(T) p(H₂)³ ÷ p(H₂O), which grows as the square of the
//!   pressure: methane needs "atmospheric surface pressures exceeding ∼10 bar, a reduced (poorly
//!   oxidised) mantle and diminished magma temperatures" (Tian and Heng 2024, read).
//! - H₂S + 3⁄2 O₂ ⇌ SO₂ + H₂O gives SO₂/H₂S = K(T) f(O₂)^(3⁄2) ÷ p(H₂O), which falls with the
//!   pressure: "at pressures exceeding a few tens of bar, sulphur is degassed as H₂S, whereas at
//!   atmospheric pressure, SO₂ is the dominant species" (Gaillard, Scaillet and Arndt 2011,
//!   Nature 478, 229, read); "low pressure promotes sulfur-rich gases" (Gaillard and Scaillet 2014,
//!   EPSL 403, 307, read).
//! - Nitrogen is N₂ throughout, an inert share of the gas (NH₃, below, is not modelled).
//!
//! # The element budget
//!
//! The vent outgasses the carbon, hydrogen, nitrogen and sulphur it is given ([`VentElements`]),
//! whose ratios set its shares: by default the exchangeable carriers'
//! ([`VentElements::of_exchangeable`]), whose sulphur is held at zero, as the inventory holds it
//! (P14.T50.a). Their oxygen is what the melt's buffer leaves them, so the gas's moles close the
//! balances by construction, and its partial pressures sum to the vent's pressure. One bracketed
//! root in p(H₂) solves them ([`VentGas::speciate`]): at a trial p(H₂) the carbon and sulphur
//! species follow in closed form, the hydrogen left for H₂ and H₂O sets their moles, and the root
//! is where that p(H₂) is the gas's own. The free H₂ and H₂O are then read from the balance that
//! holds them well: the hydrogen's where they lead the gas, the pressure's where they are a small
//! remainder of what CH₄ and H₂S take. The gas's oxygen against that of the carriers drawn as
//! H₂O and CO₂ is the surplus or deficit the melt gave or took
//! ([`VentBudget::oxygen_against_carriers_mol`]).
//!
//! # The graphite cap
//!
//! Graphite saturates the vent where p(CO) reaches `K_f(CO)` f(O₂)^½, `K_f` that of C(graphite) +
//! ½ O₂ ⇌ CO. This module does not apply it: [`VentGas::carbon_monoxide_at_graphite`] gives the
//! cap, and P14.T55.a.c takes the smaller of it and p(CO), CO₂ and CH₄ following by this gas's
//! ratios (research-leans-58).
//!
//! # The inputs (leans, 2026-11-25)
//!
//! - **The vent's temperature** is its crust's eruption temperature ([`vent_temperature`]), or
//!   [`DEFAULT_VENT_TEMPERATURE`] for a body with none.
//! - **Its pressure** is the surface pressure, as Tian and Heng (2024) take it ("the surface
//!   pressure of the atmosphere (Gaillard & Scaillet 2014)"), given by the caller. An ice shell's
//!   silicate vent lies on its seafloor, so its caller gives the seafloor's pressure, and its gas
//!   reaches the air only through the ocean, which is not modelled (for P14.T55.a.c).
//! - **The mantle's own carbon and water** stay held at zero (P14.T50.a): this speciates the
//!   exchangeable carriers only.
//!
//! # Not modelled
//!
//! - **The gas's own redox on its way up.** Gaillard and Scaillet (2014) find that "regardless of
//!   the redox state of their source regions, degassing basalts tend to reach fO2 similar to
//!   FMQ-2±0.5 in the pressure range 10-0.01 bar"; here the gas keeps its mantle's ΔIW.
//! - **S₂**, which the registry has no row for: a reduced, sulphur-rich gas's sulphur is H₂S
//!   here. SO₂ and S₂ share a very low pressure's gas (Gaillard and Scaillet 2014).
//! - **NH₃, COS and the melt's solubilities**: the vent outgasses all it is given, its nitrogen as
//!   N₂. NH₃/N₂ is about 10⁻³ at the default vent and 1 bar (NIST-JANAF's K of N₂ + 3 H₂ ⇌ 2 NH₃),
//!   but 0.2–0.6 at a cool, reduced vent of 100 bar (1,000–1,200 K at IW−4).
//! - **Non-ideal fugacities**, which matter at the kilobar pressures of a deep vent.
//!
//! # Determinism
//!
//! Pure functions of their arguments: a bisection of a fixed number of steps from a bracket that
//! depends on the arguments alone, sums in [`VentSpecies::ALL`]'s order, and every
//! transcendental through the crate's [`math`].

use core::fmt;
use std::error::Error;

use crate::planetary::chem::inventory::{ElementInventory, Reservoir};
use crate::planetary::chem::redox::iw_log_fugacity;
use crate::planetary::hooks::crust::{BASALTIC_ERUPTION_TEMPERATURE, CrustLithology};
use crate::substance::{Element, Reaction, SubstanceId, log10_formation_constant};
use crate::units::{Dex, Kelvin, Pascals};
use crate::{GeneratorVersion, math};

#[cfg(test)]
mod tests;

/// Pascals in a bar.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// The generator version from which a secondary air's carriers are its vent's quenched gas
/// (P14.T55.a.c, [`OutgassingRules::Speciated`]): 36, a planetary hold (research-leans-58). Below
/// it the carbon's carrier is P14.T24.f's insolation rule alone, so that no output moves at 33,
/// 34 or 35.
pub(crate) const OUTGASSING_SPECIATION_VERSION: GeneratorVersion = GeneratorVersion::new(36);

/// Whether a secondary air's carriers are its vent's quenched gas (P14.T55.a.c; their hold,
/// [`OUTGASSING_SPECIATION_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum OutgassingRules {
    /// Version 35's and before: P14.T24.f's carrier, carbon dioxide or its methane arm, the
    /// inventory's carbon whole.
    Held,
    /// Version 36's: the vent's carbon shares, graphite's cap at the vent, carbon monoxide shifted
    /// to carbon dioxide where the air holds water, and its hydrogen to escape, on every air that
    /// is not hot and not on P14.T24.f's methane arm (`derive::atmosphere`'s `vented`).
    Speciated,
}

impl OutgassingRules {
    /// The rules in force: the version's ([`of_version`](Self::of_version)); in the tests, the
    /// rules a test's switch sets on its thread ([`with_outgassing_speciation`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = OUTGASSING_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        Self::of_version(crate::GENERATOR_VERSION)
    }

    /// The rules of the generator version `version`: speciated from
    /// [`OUTGASSING_SPECIATION_VERSION`], held below it.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        if version.get() >= OUTGASSING_SPECIATION_VERSION.get() {
            Self::Speciated
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The outgassing speciation's rules a test sets on its thread, `None` for the version's.
    static OUTGASSING_RULES: core::cell::Cell<Option<OutgassingRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the outgassing speciation's rules `rules` on this thread, for the tests to
/// reach version 36's ahead of its bump; the version's rules are set back after, a panic in `run`
/// included. A record derives lazily, so a test reads it inside the switch. 36's bump deletes it
/// with the hold.
#[cfg(test)]
pub(crate) fn with_outgassing_speciation<R>(rules: OutgassingRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<OutgassingRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OUTGASSING_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(OUTGASSING_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The vent's temperature on a body whose crust is not known: 1,473.15 K, a basaltic lava's
/// eruption temperature ([`BASALTIC_ERUPTION_TEMPERATURE`], Keszthelyi et al. 2007; a lean,
/// 2026-11-25).
pub const DEFAULT_VENT_TEMPERATURE: Kelvin = BASALTIC_ERUPTION_TEMPERATURE;

/// The temperature of the vent of a body whose lid is `lid`: its lid's
/// [`eruption_temperature`](CrustLithology::eruption_temperature), but for a lid that is not a
/// lava, whose vent erupts the mantle's melt beneath it at [`DEFAULT_VENT_TEMPERATURE`] (a lean,
/// 2026-11-25): an ice shell, and the flotation crusts [`CrustLithology::Graphite`] and
/// [`CrustLithology::Feldspathic`], whose own melts lie at 3,973 and 1,533.15 K, while their vents
/// erupt the mantle's melt (Mercury's volcanic plains, the lunar maria).
///
/// # Examples
///
/// Io's ultramafic lava erupts hotter than a basalt, and an icy moon's silicate vent and
/// Mercury's under its graphite are taken at a basalt's:
///
/// ```
/// use hyperion_sim::planetary::chem::outgassing::{DEFAULT_VENT_TEMPERATURE, vent_temperature};
/// use hyperion_sim::planetary::hooks::crust::CrustLithology;
///
/// assert!(vent_temperature(CrustLithology::Komatiite) > DEFAULT_VENT_TEMPERATURE);
/// assert_eq!(vent_temperature(CrustLithology::Water), DEFAULT_VENT_TEMPERATURE);
/// assert_eq!(vent_temperature(CrustLithology::Graphite), DEFAULT_VENT_TEMPERATURE);
/// ```
#[must_use]
pub fn vent_temperature(lid: CrustLithology) -> Kelvin {
    match lid {
        CrustLithology::Water | CrustLithology::Graphite | CrustLithology::Feldspathic => {
            DEFAULT_VENT_TEMPERATURE
        }
        CrustLithology::Basalt
        | CrustLithology::IronRichBasalt
        | CrustLithology::Komatiite
        | CrustLithology::ReducedBasalt
        | CrustLithology::Granite
        | CrustLithology::Ultramafic
        | CrustLithology::Phyllosilicate
        | CrustLithology::ChondriteCi
        | CrustLithology::ChondriteCm
        | CrustLithology::ChondriteCv
        | CrustLithology::ChondriteH
        | CrustLithology::ChondriteL
        | CrustLithology::ChondriteLl
        | CrustLithology::ChondriteE
        | CrustLithology::Cometary => lid.eruption_temperature(),
    }
}

/// A species of the vent's gas, in registry order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VentSpecies {
    /// H₂.
    Hydrogen,
    /// H₂O.
    Water,
    /// CH₄.
    Methane,
    /// N₂.
    Nitrogen,
    /// O₂, at the melt's fugacity.
    Oxygen,
    /// CO₂.
    CarbonDioxide,
    /// CO.
    CarbonMonoxide,
    /// SO₂.
    SulphurDioxide,
    /// H₂S.
    HydrogenSulphide,
}

/// The number of [`VentSpecies`].
const SPECIES: usize = 9;

impl VentSpecies {
    /// Every species, in registry order, the order in which the gas's sums are taken.
    pub const ALL: [Self; SPECIES] = [
        Self::Hydrogen,
        Self::Water,
        Self::Methane,
        Self::Nitrogen,
        Self::Oxygen,
        Self::CarbonDioxide,
        Self::CarbonMonoxide,
        Self::SulphurDioxide,
        Self::HydrogenSulphide,
    ];

    /// Its registry row's key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Hydrogen => "H2",
            Self::Water => "H2O",
            Self::Methane => "CH4",
            Self::Nitrogen => "N2",
            Self::Oxygen => "O2",
            Self::CarbonDioxide => "CO2",
            Self::CarbonMonoxide => "CO",
            Self::SulphurDioxide => "SO2",
            Self::HydrogenSulphide => "H2S",
        }
    }

    /// Its registry row.
    ///
    /// # Panics
    ///
    /// Never: every species is a registry row, as the module's tests hold.
    #[must_use]
    pub fn substance(self) -> SubstanceId {
        SubstanceId::from_index(self.registry_index())
            .expect("every vent species is a registry row, as the module's tests hold")
    }

    /// Its row's index in the registry, which is append-only, so that a row's index never
    /// changes; the module's tests hold each to its key.
    #[must_use]
    const fn registry_index(self) -> u16 {
        match self {
            Self::Hydrogen => 0,
            Self::Water => 2,
            Self::Methane => 3,
            Self::Nitrogen => 5,
            Self::Oxygen => 6,
            Self::CarbonDioxide => 7,
            Self::CarbonMonoxide => 13,
            Self::SulphurDioxide => 14,
            Self::HydrogenSulphide => 15,
        }
    }

    /// The atoms of `element` in one of its molecules, from its row's stoichiometry.
    #[must_use]
    pub fn atoms(self, element: Element) -> f64 {
        self.substance()
            .substance()
            .elements()
            .iter()
            .find(|&&(e, _)| e == element)
            .map_or(0.0, |&(_, n)| f64::from(n))
    }

    /// Its place in [`ALL`](Self::ALL), an index into the gas's arrays alone.
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::Hydrogen => 0,
            Self::Water => 1,
            Self::Methane => 2,
            Self::Nitrogen => 3,
            Self::Oxygen => 4,
            Self::CarbonDioxide => 5,
            Self::CarbonMonoxide => 6,
            Self::SulphurDioxide => 7,
            Self::HydrogenSulphide => 8,
        }
    }
}

/// The hydrogen, carbon, nitrogen and sulphur a vent outgasses, mol of atoms, each finite and at
/// least 0.
///
/// The scale is the caller's: the gas's shares and ratios depend on the elements' ratios alone.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct VentElements {
    hydrogen: f64,
    carbon: f64,
    nitrogen: f64,
    sulphur: f64,
}

impl VentElements {
    /// The elements given, mol of atoms, in the element registry's order; a value that is
    /// negative or not finite is taken as none, as
    /// [`VolatileInventory::new`](crate::planetary::derive::atmosphere::VolatileInventory::new)
    /// takes its masses.
    #[must_use]
    pub fn new(hydrogen_mol: f64, carbon_mol: f64, nitrogen_mol: f64, sulphur_mol: f64) -> Self {
        let held = |x: f64| if x.is_finite() && x > 0.0 { x } else { 0.0 };
        Self {
            hydrogen: held(hydrogen_mol),
            carbon: held(carbon_mol),
            nitrogen: held(nitrogen_mol),
            sulphur: held(sulphur_mol),
        }
    }

    /// The exchangeable hydrogen, carbon, nitrogen and sulphur of `inventory`, mol of atoms: its
    /// carriers' water, carbon dioxide and nitrogen, by the element registry's weights, and its
    /// exchangeable sulphur, which P14.T50.a holds at zero (the mantle's own is not the
    /// carriers').
    ///
    /// Their ratios are the vent's; what the vent outgasses of them is the caller's.
    #[must_use]
    pub fn of_exchangeable(inventory: &ElementInventory) -> Self {
        let mol = |element: Element| {
            inventory.mass(element, Reservoir::Exchangeable).value() * 1e3
                / element.standard_atomic_weight_g_per_mol()
        };
        Self::new(
            mol(Element::H),
            mol(Element::C),
            mol(Element::N),
            mol(Element::S),
        )
    }

    /// Its hydrogen, mol of atoms.
    #[must_use]
    pub const fn hydrogen_mol(&self) -> f64 {
        self.hydrogen
    }

    /// Its carbon, mol of atoms.
    #[must_use]
    pub const fn carbon_mol(&self) -> f64 {
        self.carbon
    }

    /// Its nitrogen, mol of atoms.
    #[must_use]
    pub const fn nitrogen_mol(&self) -> f64 {
        self.nitrogen
    }

    /// Its sulphur, mol of atoms.
    #[must_use]
    pub const fn sulphur_mol(&self) -> f64 {
        self.sulphur
    }
}

/// Where a vent's gas is speciated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VentInputs {
    /// The mantle's ΔIW, log₁₀ units, finite ([`ElementInventory::mantle_delta_iw`]).
    pub delta_iw: Dex,
    /// The vent's temperature, K, within the species' records, 300–6,000 K
    /// ([`vent_temperature`], or [`DEFAULT_VENT_TEMPERATURE`]).
    pub temperature: Kelvin,
    /// The vent's pressure, Pa, finite and positive: the surface pressure (the module's leans).
    pub pressure: Pascals,
}

/// A vent's gas could not be speciated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeciateVentGasError {
    /// The mantle's ΔIW is not finite.
    DeltaIw(f64),
    /// The vent's temperature, K, is not finite or lies outside the species' records.
    Temperature(f64),
    /// The vent's pressure, Pa, is not finite and positive.
    Pressure(f64),
    /// The melt's oxygen fugacity alone reaches the vent's pressure.
    OxygenFugacity {
        /// The melt's f(O₂), bar.
        fugacity_bar: f64,
        /// The vent's pressure, bar.
        pressure_bar: f64,
    },
    /// The vent is given no hydrogen, carbon, nitrogen or sulphur.
    NoVolatiles,
    /// ΔIW or the pressure lies so far out, ΔIW some hundreds of log units from IW, that a
    /// constant or a share of the gas is not a finite float.
    Overflow,
}

impl fmt::Display for SpeciateVentGasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeltaIw(x) => write!(f, "mantle ΔIW {x} is not finite"),
            Self::Temperature(t) => {
                write!(f, "vent temperature {t} K is outside the species' records")
            }
            Self::Pressure(p) => write!(f, "vent pressure {p} Pa is not finite and positive"),
            Self::OxygenFugacity {
                fugacity_bar,
                pressure_bar,
            } => write!(
                f,
                "the melt's oxygen fugacity {fugacity_bar} bar reaches the vent's pressure \
                 {pressure_bar} bar"
            ),
            Self::NoVolatiles => {
                f.write_str("the vent is given no hydrogen, carbon, nitrogen or sulphur")
            }
            Self::Overflow => f.write_str("the vent's gas overflows the floats"),
        }
    }
}

impl Error for SpeciateVentGasError {}

/// The elements a vent's gas hands the air, mol of atoms: its hydrogen, carbon, nitrogen and
/// sulphur as given, and the oxygen the melt's buffer left them, each summed over the species in
/// [`VentSpecies::ALL`]'s order.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct VentBudget {
    hydrogen: f64,
    carbon: f64,
    nitrogen: f64,
    oxygen: f64,
    sulphur: f64,
}

impl VentBudget {
    /// Its hydrogen, mol of atoms.
    #[must_use]
    pub const fn hydrogen_mol(&self) -> f64 {
        self.hydrogen
    }

    /// Its carbon, mol of atoms.
    #[must_use]
    pub const fn carbon_mol(&self) -> f64 {
        self.carbon
    }

    /// Its nitrogen, mol of atoms.
    #[must_use]
    pub const fn nitrogen_mol(&self) -> f64 {
        self.nitrogen
    }

    /// Its oxygen, mol of atoms, the O₂ at the melt's fugacity among it.
    #[must_use]
    pub const fn oxygen_mol(&self) -> f64 {
        self.oxygen
    }

    /// Its sulphur, mol of atoms.
    #[must_use]
    pub const fn sulphur_mol(&self) -> f64 {
        self.sulphur
    }

    /// Its oxygen less that of its hydrogen and carbon as H₂O and CO₂, the carriers as drawn, mol
    /// of atoms: negative where the melt kept oxygen (a reduced gas's H₂, CO and CH₄), positive
    /// where it gave some (O₂ and SO₂'s, sulphur being drawn with none).
    #[must_use]
    pub fn oxygen_against_carriers_mol(&self) -> f64 {
        self.oxygen - (0.5 * self.hydrogen + 2.0 * self.carbon)
    }
}

/// The steps of [`VentGas::speciate`]'s bisection: the first steps fall by [`FIRST_STEP`] until
/// the bracket's foot leaves zero, then each halves its logarithm, so that 96 resolve p(H₂) to the
/// last bit from a foot as low as 10⁻¹²⁰ of the bracket's top.
const BISECTION_STEPS: u32 = 96;

/// How far below the bracket's top [`VentGas::speciate`]'s first trial lies, while the bracket's
/// foot is zero: 10⁻⁶.
const FIRST_STEP: f64 = 1e-6;

/// A vent's gas: its species by mole fraction and moles at the melt's oxygen fugacity, and the
/// element budget it hands the air (the [module](self)'s model).
///
/// Built only by [`VentGas::speciate`], which guarantees every figure finite and at least 0, the
/// fractions summing to 1 and the moles closing the elements given.
#[derive(Debug, Clone, PartialEq)]
pub struct VentGas {
    temperature: Kelvin,
    pressure: Pascals,
    log10_oxygen_fugacity: Dex,
    log10_carbon_monoxide_formation_constant: f64,
    water_to_hydrogen: f64,
    carbon_dioxide_to_monoxide: f64,
    methane_to_monoxide: f64,
    sulphur_dioxide_to_hydrogen_sulphide: Option<f64>,
    moles: [f64; SPECIES],
    fractions: [f64; SPECIES],
    budget: VentBudget,
}

/// 2 H₂ + O₂ ⇌ 2 H₂O, each species with its ν.
const WATER_FORMATION: [(VentSpecies, i32); 3] = [
    (VentSpecies::Hydrogen, -2),
    (VentSpecies::Oxygen, -1),
    (VentSpecies::Water, 2),
];

/// 2 CO + O₂ ⇌ 2 CO₂.
const CARBON_DIOXIDE_FORMATION: [(VentSpecies, i32); 3] = [
    (VentSpecies::CarbonMonoxide, -2),
    (VentSpecies::Oxygen, -1),
    (VentSpecies::CarbonDioxide, 2),
];

/// CO + 3 H₂ ⇌ CH₄ + H₂O.
const METHANATION: [(VentSpecies, i32); 4] = [
    (VentSpecies::CarbonMonoxide, -1),
    (VentSpecies::Hydrogen, -3),
    (VentSpecies::Methane, 1),
    (VentSpecies::Water, 1),
];

/// 2 H₂S + 3 O₂ ⇌ 2 SO₂ + 2 H₂O.
const SULPHUR_OXIDATION: [(VentSpecies, i32); 4] = [
    (VentSpecies::HydrogenSulphide, -2),
    (VentSpecies::Oxygen, -3),
    (VentSpecies::SulphurDioxide, 2),
    (VentSpecies::Water, 2),
];

/// log₁₀ K of the reaction `species`, each `(species, ν)`, at `temperature`, or `None` outside
/// its records.
#[must_use]
fn log10_k<const N: usize>(species: &[(VentSpecies, i32); N], temperature: Kelvin) -> Option<f64> {
    let terms = species.map(|(x, nu)| (x.substance(), nu));
    Reaction::new(&terms)
        .expect("each vent reaction is balanced, as the module's tests hold")
        .log10_equilibrium_constant(temperature)
}

/// The constants of a vent's gas at one temperature and oxygen fugacity, f(O₂) in bar.
#[derive(Debug, Clone, Copy)]
struct Constants {
    /// H₂O/H₂, K f(O₂)^½.
    water_to_hydrogen: f64,
    /// CO₂/CO, K f(O₂)^½.
    carbon_dioxide_to_monoxide: f64,
    /// CH₄/CO ÷ p(H₂)², bar⁻²: K of CO + 3 H₂ ⇌ CH₄ + H₂O over H₂O/H₂.
    methane: f64,
    /// SO₂/H₂S × p(H₂), bar: K^½ of 2 H₂S + 3 O₂ ⇌ 2 SO₂ + 2 H₂O times f(O₂)^(3⁄2), over H₂O/H₂.
    sulphur: f64,
}

/// The moles of each species of a gas of `elements` under `constants` at a trial p(H₂) of
/// `hydrogen_bar`, in [`VentSpecies::ALL`]'s order, but H₂, H₂O and O₂, which are 0: the carbon,
/// nitrogen and sulphur in closed form.
#[must_use]
fn others_at(elements: &VentElements, constants: &Constants, hydrogen_bar: f64) -> [f64; SPECIES] {
    let methane_to_monoxide = constants.methane * hydrogen_bar * hydrogen_bar;
    let monoxide =
        elements.carbon / (1.0 + constants.carbon_dioxide_to_monoxide + methane_to_monoxide);
    let dioxide = constants.carbon_dioxide_to_monoxide * monoxide;
    let methane = methane_to_monoxide * monoxide;
    let sulphur_share = hydrogen_bar + constants.sulphur;
    // Both terms are zero only with no hydrogen and an SO₂ ratio below the smallest float, where
    // the sulphur can only be SO₂.
    let (hydrogen_sulphide, sulphur_dioxide) = if sulphur_share > 0.0 {
        (
            elements.sulphur * hydrogen_bar / sulphur_share,
            elements.sulphur * constants.sulphur / sulphur_share,
        )
    } else {
        (0.0, elements.sulphur)
    };
    [
        0.0,
        0.0,
        methane,
        0.5 * elements.nitrogen,
        0.0,
        dioxide,
        monoxide,
        sulphur_dioxide,
        hydrogen_sulphide,
    ]
}

/// The moles of each species of a gas of `elements` under `constants` at a trial p(H₂) of
/// `hydrogen_bar`, O₂'s 0, by the hydrogen balance: [`others_at`], and the hydrogen that CH₄ and
/// H₂S leave in H₂ and H₂O, none where they take it all.
#[must_use]
fn moles_at(elements: &VentElements, constants: &Constants, hydrogen_bar: f64) -> [f64; SPECIES] {
    let mut moles = others_at(elements, constants, hydrogen_bar);
    let methane = moles[VentSpecies::Methane.index()];
    let hydrogen_sulphide = moles[VentSpecies::HydrogenSulphide.index()];
    let left = elements.hydrogen - 4.0 * methane - 2.0 * hydrogen_sulphide;
    let hydrogen = if left > 0.0 {
        left / (2.0 * (1.0 + constants.water_to_hydrogen))
    } else {
        0.0
    };
    moles[VentSpecies::Hydrogen.index()] = hydrogen;
    moles[VentSpecies::Water.index()] = constants.water_to_hydrogen * hydrogen;
    moles
}

/// The moles of each species of a gas of `elements` under `constants` at p(H₂) `hydrogen_bar`,
/// its volatiles at `volatile_bar`, O₂'s 0, by the pressure balance: [`others_at`], and H₂ and
/// H₂O at p(H₂) and p(H₂O) over the bar per mole that the other species' share of the pressure
/// gives. `rest_mol`, the moles of those species (the carbon, the N₂ and the sulphur), is
/// positive, and H₂ and H₂O hold at most half of `volatile_bar`, so that the share is not a small
/// difference.
#[must_use]
fn moles_by_pressure(
    elements: &VentElements,
    constants: &Constants,
    hydrogen_bar: f64,
    volatile_bar: f64,
    rest_mol: f64,
) -> [f64; SPECIES] {
    let mut moles = others_at(elements, constants, hydrogen_bar);
    let held_bar = (1.0 + constants.water_to_hydrogen) * hydrogen_bar;
    let bar_per_mol = (volatile_bar - held_bar) / rest_mol;
    let hydrogen = hydrogen_bar / bar_per_mol;
    moles[VentSpecies::Hydrogen.index()] = hydrogen;
    moles[VentSpecies::Water.index()] = constants.water_to_hydrogen * hydrogen;
    moles
}

/// The sum of `moles`, in [`VentSpecies::ALL`]'s order.
#[must_use]
fn total(moles: &[f64; SPECIES]) -> f64 {
    moles.iter().fold(0.0, |sum, &n| sum + n)
}

impl VentGas {
    /// The gas a vent at `inputs` outgasses `elements` as.
    ///
    /// # Errors
    ///
    /// The first, in this order, of:
    ///
    /// - [`SpeciateVentGasError::DeltaIw`] for a ΔIW that is not finite;
    /// - [`SpeciateVentGasError::Temperature`] for a temperature outside the species' records;
    /// - [`SpeciateVentGasError::Pressure`] for a pressure that is not finite and positive;
    /// - [`SpeciateVentGasError::OxygenFugacity`] where the melt's f(O₂) reaches the pressure;
    /// - [`SpeciateVentGasError::NoVolatiles`] for elements that are all zero;
    /// - [`SpeciateVentGasError::Overflow`] where the gas is not finite.
    ///
    /// # Panics
    ///
    /// Never: every species is a registry row with a record, and every reaction balances, as the
    /// module's tests hold.
    ///
    /// # Examples
    ///
    /// Earth's upper mantle, near IW+3.6, outgasses water and carbon dioxide; a mantle at IW−2
    /// outgasses the same carbon as carbon monoxide:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::outgassing::{
    ///     DEFAULT_VENT_TEMPERATURE, VentElements, VentGas, VentInputs, VentSpecies,
    /// };
    /// use hyperion_sim::units::{Dex, Pascals};
    ///
    /// let elements = VentElements::new(2.0, 0.1, 0.01, 0.0);
    /// let vent = |delta_iw: f64| VentInputs {
    ///     delta_iw: Dex::new(delta_iw),
    ///     temperature: DEFAULT_VENT_TEMPERATURE,
    ///     pressure: Pascals::new(1.0e5),
    /// };
    /// let earth = VentGas::speciate(&vent(3.6), &elements)?;
    /// assert!(earth.water_to_hydrogen() > 10.0 && earth.carbon_dioxide_to_monoxide() > 10.0);
    /// let reduced = VentGas::speciate(&vent(-2.0), &elements)?;
    /// let (co, co2) = (VentSpecies::CarbonMonoxide, VentSpecies::CarbonDioxide);
    /// assert!(reduced.fraction(co) > reduced.fraction(co2));
    /// // The melt kept the oxygen the carriers drew.
    /// assert!(reduced.budget().oxygen_against_carriers_mol() < 0.0);
    /// # Ok::<(), hyperion_sim::planetary::chem::outgassing::SpeciateVentGasError>(())
    /// ```
    pub fn speciate(
        inputs: &VentInputs,
        elements: &VentElements,
    ) -> Result<Self, SpeciateVentGasError> {
        let delta_iw = inputs.delta_iw.value();
        if !delta_iw.is_finite() {
            return Err(SpeciateVentGasError::DeltaIw(delta_iw));
        }
        let t = inputs.temperature;
        let out_of_range = || SpeciateVentGasError::Temperature(t.value());
        if !t.value().is_finite() {
            return Err(out_of_range());
        }
        let water = log10_k(&WATER_FORMATION, t).ok_or_else(out_of_range)?;
        let carbon = log10_k(&CARBON_DIOXIDE_FORMATION, t).ok_or_else(out_of_range)?;
        let methane = log10_k(&METHANATION, t).ok_or_else(out_of_range)?;
        let sulphur = log10_k(&SULPHUR_OXIDATION, t).ok_or_else(out_of_range)?;
        let monoxide_formation =
            log10_formation_constant(VentSpecies::CarbonMonoxide.substance(), t)
                .ok_or_else(out_of_range)?;
        let pressure_pa = inputs.pressure.value();
        if !(pressure_pa.is_finite() && pressure_pa > 0.0) {
            return Err(SpeciateVentGasError::Pressure(pressure_pa));
        }

        let log10_oxygen_fugacity = iw_log_fugacity(t) + inputs.delta_iw;
        let log_f = log10_oxygen_fugacity.value();
        let fugacity_bar = math::exp10(log_f);
        let pressure_bar = pressure_pa / PASCALS_PER_BAR;
        if fugacity_bar >= pressure_bar {
            return Err(SpeciateVentGasError::OxygenFugacity {
                fugacity_bar,
                pressure_bar,
            });
        }
        let given = elements.hydrogen + elements.carbon + elements.nitrogen + elements.sulphur;
        if given <= 0.0 {
            return Err(SpeciateVentGasError::NoVolatiles);
        }

        let log_water = 0.5 * water + 0.5 * log_f;
        let constants = Constants {
            water_to_hydrogen: math::exp10(log_water),
            carbon_dioxide_to_monoxide: math::exp10(0.5 * carbon + 0.5 * log_f),
            methane: math::exp10(methane - log_water),
            sulphur: math::exp10(0.5 * sulphur + 1.5 * log_f - log_water),
        };
        let finite = [
            constants.water_to_hydrogen,
            constants.carbon_dioxide_to_monoxide,
            constants.methane,
            constants.sulphur,
        ];
        if !finite.iter().all(|x| x.is_finite()) {
            return Err(SpeciateVentGasError::Overflow);
        }
        // The volatiles' share of the pressure; the rest is the melt's O₂.
        let volatile_bar = pressure_bar - fugacity_bar;
        let hydrogen_bar = if elements.hydrogen > 0.0 {
            Self::hydrogen_root(elements, &constants, volatile_bar)
        } else {
            0.0
        };
        // Each balance gives the free H₂ and H₂O well where the other does not: the hydrogen's
        // where they hold most of the gas, and the pressure's where they hold little of it, as a
        // small remainder of hydrogen that CH₄ or H₂S take nearly all of.
        let rest_mol = elements.carbon + 0.5 * elements.nitrogen + elements.sulphur;
        let held_bar = (1.0 + constants.water_to_hydrogen) * hydrogen_bar;
        let mut moles = if rest_mol > 0.0 && held_bar <= 0.5 * volatile_bar {
            moles_by_pressure(elements, &constants, hydrogen_bar, volatile_bar, rest_mol)
        } else {
            moles_at(elements, &constants, hydrogen_bar)
        };
        let bar_per_mol = volatile_bar / total(&moles);
        moles[VentSpecies::Oxygen.index()] = fugacity_bar / bar_per_mol;
        let all = total(&moles);
        let fractions = moles.map(|n| n / all);
        if !(moles.iter().chain(&fractions).all(|x| x.is_finite())) {
            return Err(SpeciateVentGasError::Overflow);
        }
        Ok(Self {
            temperature: t,
            pressure: inputs.pressure,
            log10_oxygen_fugacity,
            log10_carbon_monoxide_formation_constant: monoxide_formation,
            water_to_hydrogen: constants.water_to_hydrogen,
            carbon_dioxide_to_monoxide: constants.carbon_dioxide_to_monoxide,
            methane_to_monoxide: constants.methane * hydrogen_bar * hydrogen_bar,
            sulphur_dioxide_to_hydrogen_sulphide: (hydrogen_bar > 0.0)
                .then(|| constants.sulphur / hydrogen_bar),
            moles,
            fractions,
            budget: Self::budget_of(&moles),
        })
    }

    /// p(H₂), bar, of a gas of `elements`, which holds hydrogen, under `constants`, its volatiles
    /// at `volatile_bar`: the root of g(a) = a − P x(H₂)(a), where x(H₂)(a) is the mole fraction
    /// of H₂ of [`moles_at`] a trial a.
    ///
    /// x(H₂)(a) falls as a rises, since the carbon and sulphur take more of the hydrogen, so g
    /// rises, from below zero at a = 0 to at least zero at the bracket's top, x(H₂)(0) P. The
    /// bisection keeps its foot where g is negative, and returns it, so that the hydrogen left
    /// for H₂ and H₂O there is positive.
    #[must_use]
    fn hydrogen_root(elements: &VentElements, constants: &Constants, volatile_bar: f64) -> f64 {
        let excess = |a: f64| {
            let moles = moles_at(elements, constants, a);
            a - volatile_bar * moles[VentSpecies::Hydrogen.index()] / total(&moles)
        };
        let (mut foot, mut top): (f64, f64) = (0.0, -excess(0.0));
        for _ in 0..BISECTION_STEPS {
            let trial = if foot > 0.0 {
                foot.sqrt() * top.sqrt()
            } else {
                top * FIRST_STEP
            };
            if excess(trial) < 0.0 {
                foot = trial;
            } else {
                top = trial;
            }
        }
        foot
    }

    /// The element budget of `moles`, each element summed over [`VentSpecies::ALL`]'s order.
    #[must_use]
    fn budget_of(moles: &[f64; SPECIES]) -> VentBudget {
        let sum = |element: Element| {
            VentSpecies::ALL
                .iter()
                .zip(moles)
                .fold(0.0, |sum, (species, &n)| sum + n * species.atoms(element))
        };
        VentBudget {
            hydrogen: sum(Element::H),
            carbon: sum(Element::C),
            nitrogen: sum(Element::N),
            oxygen: sum(Element::O),
            sulphur: sum(Element::S),
        }
    }

    /// The vent's temperature, K.
    #[must_use]
    pub const fn temperature(&self) -> Kelvin {
        self.temperature
    }

    /// The vent's pressure, Pa: the sum of its species' partial pressures.
    #[must_use]
    pub const fn pressure(&self) -> Pascals {
        self.pressure
    }

    /// log₁₀ of the melt's oxygen fugacity, bar: IW(T) + ΔIW.
    #[must_use]
    pub const fn log10_oxygen_fugacity(&self) -> Dex {
        self.log10_oxygen_fugacity
    }

    /// log₁₀ `K_f(CO)` at the vent's temperature, that of C(graphite) + ½ O₂ ⇌ CO, its pressures
    /// in bar ([`log10_formation_constant`]).
    #[must_use]
    pub const fn log10_carbon_monoxide_formation_constant(&self) -> f64 {
        self.log10_carbon_monoxide_formation_constant
    }

    /// The graphite cap, Pa: the p(CO) at which graphite saturates the vent, `K_f(CO)` f(O₂)^½
    /// (the [module](self)'s cap, which P14.T55.a.c applies).
    ///
    /// # Examples
    ///
    /// A carbon-rich gas from a mantle at IW−6 passes the cap at 1 bar, so graphite saturates its
    /// vent, and the air takes the smaller of the two:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::outgassing::{
    ///     DEFAULT_VENT_TEMPERATURE, VentElements, VentGas, VentInputs, VentSpecies,
    /// };
    /// use hyperion_sim::units::{Dex, Pascals};
    ///
    /// let vent = VentGas::speciate(
    ///     &VentInputs {
    ///         delta_iw: Dex::new(-6.0),
    ///         temperature: DEFAULT_VENT_TEMPERATURE,
    ///         pressure: Pascals::new(1.0e5),
    ///     },
    ///     &VentElements::new(0.1, 1.0, 0.0, 0.0),
    /// )?;
    /// let (co, cap) = (
    ///     vent.partial_pressure(VentSpecies::CarbonMonoxide),
    ///     vent.carbon_monoxide_at_graphite(),
    /// );
    /// assert!(co > cap);
    /// let held = if co < cap { co } else { cap };
    /// assert_eq!(held, cap);
    /// # Ok::<(), hyperion_sim::planetary::chem::outgassing::SpeciateVentGasError>(())
    /// ```
    #[must_use]
    pub fn carbon_monoxide_at_graphite(&self) -> Pascals {
        Pascals::new(
            math::exp10(
                self.log10_carbon_monoxide_formation_constant
                    + 0.5 * self.log10_oxygen_fugacity.value(),
            ) * PASCALS_PER_BAR,
        )
    }

    /// H₂O/H₂ by moles, K(T) f(O₂)^½, the buffer's ratio whether or not the vent holds hydrogen.
    #[must_use]
    pub const fn water_to_hydrogen(&self) -> f64 {
        self.water_to_hydrogen
    }

    /// CO₂/CO by moles, K(T) f(O₂)^½, the buffer's ratio whether or not the vent holds carbon.
    #[must_use]
    pub const fn carbon_dioxide_to_monoxide(&self) -> f64 {
        self.carbon_dioxide_to_monoxide
    }

    /// CH₄/CO by moles, K(T) p(H₂)³ ÷ p(H₂O); 0 where the vent holds no hydrogen.
    #[must_use]
    pub const fn methane_to_monoxide(&self) -> f64 {
        self.methane_to_monoxide
    }

    /// SO₂/H₂S by moles, K(T) f(O₂)^(3⁄2) ÷ p(H₂O), whether or not the vent holds sulphur; `None`
    /// where it holds no hydrogen, which leaves its sulphur all SO₂.
    #[must_use]
    pub const fn sulphur_dioxide_to_hydrogen_sulphide(&self) -> Option<f64> {
        self.sulphur_dioxide_to_hydrogen_sulphide
    }

    /// The mole fraction of `species`, its share of the vent's gas.
    #[must_use]
    pub const fn fraction(&self, species: VentSpecies) -> f64 {
        self.fractions[species.index()]
    }

    /// The moles of `species`, on the scale of the elements given.
    #[must_use]
    pub const fn moles(&self, species: VentSpecies) -> f64 {
        self.moles[species.index()]
    }

    /// The partial pressure of `species`, Pa: its fraction of the vent's pressure.
    #[must_use]
    pub fn partial_pressure(&self, species: VentSpecies) -> Pascals {
        self.pressure * self.fraction(species)
    }

    /// Each species' row and mole fraction, in registry order.
    pub fn species(&self) -> impl ExactSizeIterator<Item = (SubstanceId, f64)> + '_ {
        VentSpecies::ALL
            .iter()
            .map(|&species| (species.substance(), self.fraction(species)))
    }

    /// The elements the gas hands the air.
    #[must_use]
    pub const fn budget(&self) -> VentBudget {
        self.budget
    }
}
