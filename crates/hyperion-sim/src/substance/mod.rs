//! The substance registry: what a body can be made of, one row per substance (plan 14, Phase L;
//! decision-composition §1.1).
//!
//! A gas, an ice, a liquid, a mineral, a rock or a haze is a row of one registry, and a body is
//! described by which rows it holds, in which forms and amounts. Plan 14 decides those from
//! physics, and every reader takes the substance by its row, so that the generator is open to any
//! composition its physics produces and no reader works from a closed list of the Solar System's.
//!
//! # Identity
//!
//! - **Inside a process a row is a [`SubstanceId`]**, its index. The order is append-only and a
//!   row is never renumbered, since sums and loops in registry order are output
//!   (sim-determinism). An id never leaves the process.
//! - **Everywhere else a row is its [`SubstanceKey`]**, a string: a definite species by its
//!   formula in chemical case (`H2O`, `NH4SH`, `H-`), `e-` for the electron, and any other
//!   material by a lowercase name (`basalt`, `tholin`). A key is never renamed or removed, and one
//!   row serves every phase of its substance, so a phase is never part of a key. The key type lives
//!   in `hyperion-surface`, whose field header carries keys; the rows here use it.
//! - **The registry's public mirror** is `packages/protocol/fixtures/substances.json` (index, key,
//!   kind, molar mass, `c_p` ÷ R and the client's three optics flags), which the client's tests
//!   read, and `crates/hyperion-sim/tests/golden/substance/registry.golden`
//!   pins the rows' values. A test writes both and holds each to its committed rows as a prefix.
//!
//! # Columns
//!
//! A row carries its identity (key, [`SubstanceKind`], stoichiometry, charge, molar mass) and then
//! groups of columns, each `None` where the substance has no such data: so far the gas columns
//! ([`GasColumns`], with each gas's [`MolecularConstants`], over which [`thermo`] sums its heat
//! capacity, enthalpy and entropy by temperature), the phase diagrams ([`PhaseDiagram`], whose
//! functions are [`diagram`]'s, re-exported here), the condensed columns
//! ([`CondensedColumns`]: a solid's or a liquid's grain density, its transition and its mechanics
//! family) and the liquid columns ([`LiquidColumns`]: a surface liquid's optics and transport,
//! which [`liquid_properties`] resolves into a palette's liquid record). [`formation`] gives each
//! gas row's equilibrium constant of formation by temperature, from NASA's CEA records (P14.T49.d).
//! Every value names its sources and their licence basis ([`Source`], [`Basis`];
//! decision-composition §5). P14.T49.b adds the later rows, the gases' molecular constants, the
//! phase diagrams and the condensed columns; P14.T49.c–e the condensed optics, the thermochemistry
//! and the aerosols' extinction; P14.T49.h the liquid columns.
//!
//! # Rows
//!
//! Rows 0–8 are `H2`, `He`, `H2O`, `CH4`, `NH3`, `N2`, `O2`, `CO2` and `Ar`: the nine gases of
//! P14.T13, in [`Gas::ALL`]'s order, so that every sum over them keeps its bits.
//! [`Gas`] stays the typed view of these nine for escape and the greenhouse ([`Gas::substance`],
//! [`SubstanceId::gas`]). Seven of them carry a phase diagram, in registry order water, methane,
//! ammonia, nitrogen, oxygen, carbon dioxide and argon, and seven later rows do ([`condensables`]:
//! fourteen in all).
//!
//! P14.T49.b appends, in its plan's order: the gases R08 measures (`Ne`, `Kr`, `Xe`, `N2O`); the
//! gases of the chemistry, atoms and their first ions among them (`CO` … `FeH`, `Al` … `Zn`,
//! `Na+` … `Zn+`, `HCl`); the condensates aloft (`NH4SH` … `graphite`); the hazes and dusts; and
//! the solutions, lithologies, salts, organics and undifferentiated bodies' materials. A row no
//! rule yet produces is reachable data, labelled so where a reader meets it.

pub mod aerosol;
pub mod condensed;
pub mod diagram;
pub mod element;
pub mod formation;
pub mod helmholtz;
pub mod liquid;
pub mod mix;
pub mod optics;
pub mod phase;
mod rows;
pub mod thermal;
pub mod thermo;

pub use aerosol::{
    AerosolOptics, BuildSizeDistributionError, ExtinctionBasis, FileVariant, ResolveAerosolError,
    SizeDistribution, SizeDistributionKind, aerosol_optics,
};
pub use condensed::{
    CondensedColumns, CondensedPhase, DensityAt, GrainDensity, Transition, TransitionKind,
};
pub use diagram::{
    DensityEquation, DensityForm, MeltingCurve, MeltingEquation, MeltingPiece, PhaseDiagram,
    SolidDensity, SolidPhase, SublimationEquation, SublimationPiece, WagnerEquation, condensables,
    phase_at, saturation_pressure,
};
pub use element::Element;
pub use formation::{BalanceReactionError, Reaction, formation_range, log10_formation_constant};
pub use helmholtz::HelmholtzResidual;
pub use hyperion_surface::substance_key::{SubstanceKey, SubstanceKeyForm};
pub use liquid::{
    BuildLiquidCompositionError, Composition, LIQUID_BIN_EDGES_NM, LiquidColumns,
    LiquidComposition, LiquidMember, ResolveLiquidError, liquid_properties,
    pure_liquid_scattering_per_m,
};
pub use mix::{GasColumns, HeatCapacityClass, heat_capacity_over_r, mean_molar_mass_g_per_mol};
pub use optics::{CondensedOptics, ReferenceForm, ResolvePaletteEntryError, resolve_palette_entry};
pub use phase::{MOLAR_GAS_CONSTANT, Phase, ice_ih_density};
pub use rows::EQUILIBRIUM_HYDROGEN;
pub use thermal::{RegolithForm, RegolithInertia, ThermalColumns, regolith_thermal_inertia};
pub use thermo::{
    Level, LevelSet, MolecularConstants, NuclearSpin, Rotation, RovibrationalTerms, Vibration,
    enthalpy_over_r, entropy_over_r, heat_capacity_over_r_at, mixture_enthalpy_over_r,
    mixture_entropy_over_r, mixture_heat_capacity_over_r_at,
};

use crate::planetary::derive::atmosphere::Gas;
use rows::ROWS;

/// A row of the substance registry: its index, append-only and never renumbered.
///
/// It never appears on a wire or in a payload, where a substance is its [`SubstanceKey`]; the
/// registry's fixture and golden pin key and index together. Ids order as the registry does.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::{self, SubstanceId};
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let water = substance::by_key("H2O")?;
/// assert_eq!(water, Gas::Water.substance());
/// assert_eq!(water.gas(), Some(Gas::Water));
/// assert_eq!(water.substance().key().as_str(), "H2O");
/// assert_eq!(SubstanceId::from_index(water.index()), Some(water));
/// # Some(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubstanceId(u16);

impl SubstanceId {
    /// The row at `index`, or `None` past the last row.
    #[must_use]
    pub fn from_index(index: u16) -> Option<Self> {
        (usize::from(index) < ROWS.len()).then_some(Self(index))
    }

    /// The row's index.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.0
    }

    /// The row.
    #[must_use]
    pub fn substance(self) -> &'static Substance {
        substance(self)
    }

    /// The row's typed escape view: the [`Gas`] it is, for rows 0–8, and `None` for every other.
    #[must_use]
    pub fn gas(self) -> Option<Gas> {
        Gas::ALL.get(usize::from(self.0)).copied()
    }
}

impl Gas {
    /// The gas's row of the substance registry: rows 0–8 are [`Gas::ALL`] in its order.
    #[must_use]
    pub const fn substance(self) -> SubstanceId {
        SubstanceId(match self {
            Self::Hydrogen => 0,
            Self::Helium => 1,
            Self::Water => 2,
            Self::Methane => 3,
            Self::Ammonia => 4,
            Self::Nitrogen => 5,
            Self::Oxygen => 6,
            Self::CarbonDioxide => 7,
            Self::Argon => 8,
        })
    }
}

/// What kind of thing a substance is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SubstanceKind {
    /// A neutral molecule (`H2O`, `CO2`).
    Molecule,
    /// A neutral atom (`He`, `Ar`, `Na`).
    Atom,
    /// A charged species (`H-`, `e-`).
    Ion,
    /// A mineral of one composition (`Mg2SiO4`, `graphite`).
    Mineral,
    /// A rock: an assemblage of minerals with no single formula (`basalt`).
    Lithology,
    /// A solution (`brine_nacl`, `ch4_c2h6_n2`).
    Solution,
    /// An organic material with no single formula (`tholin`, `soot`).
    Organic,
}

impl SubstanceKind {
    /// The kind's name on every wire and in the registry's fixture.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Molecule => "molecule",
            Self::Atom => "atom",
            Self::Ion => "ion",
            Self::Mineral => "mineral",
            Self::Lithology => "lithology",
            Self::Solution => "solution",
            Self::Organic => "organic",
        }
    }
}

/// What a value's source lets the registry hold of it (decision-r08-licences's rule, as
/// decision-composition §5 extends it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Basis {
    /// A fact, stated with its citation: a measured constant, a reference equation's value, an
    /// atomic weight.
    Fact,
    /// Reduced from cited facts by stated arithmetic (a mean, a sum, a unit change).
    Reduced,
    /// Derived by a fit to cited data, offline (`hyperion-fit`).
    Fitted,
    /// The generator's own value, adopted where no source gives exactly what its model needs (an
    /// effective constant, a calibration), stated beside the cited values it was chosen against.
    /// It is HYPERION's number, which no licence restricts.
    Adopted,
    /// Raw data, committed under the permissive licence named.
    Raw {
        /// The licence, by its SPDX identifier or name.
        licence: &'static str,
    },
}

/// Where some of a row's values come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Source {
    covers: &'static str,
    citation: &'static str,
    basis: Basis,
}

impl Source {
    /// Which of the row's values this source gives, such as `"T_t, p_t"`.
    #[must_use]
    pub const fn covers(&self) -> &'static str {
        self.covers
    }

    /// The citation, with what was taken from it.
    #[must_use]
    pub const fn citation(&self) -> &'static str {
        self.citation
    }

    /// What the source lets the registry hold.
    #[must_use]
    pub const fn basis(&self) -> Basis {
        self.basis
    }
}

/// Which of the client's optics the sim may ask of a row (decision-composition §1.1): the
/// registry's fixture carries them, and the client's parity check (rendering plan R08's R08.T19)
/// holds each flagged row to an entry, an estimate or a stated stand-in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientOptics {
    rayleigh: bool,
    absorber: bool,
    aerosol_material: bool,
}

impl ClientOptics {
    /// Whether the row may be a well-mixed gas the client scatters by Rayleigh's law: every gas row
    /// but the electron.
    #[must_use]
    pub const fn rayleigh(&self) -> bool {
        self.rayleigh
    }

    /// Whether the row may absorb in the client's visible channels: a band, a continuum or a
    /// curve of growth the client draws.
    #[must_use]
    pub const fn absorber(&self) -> bool {
        self.absorber
    }

    /// Whether the row may be the material of a cloud or haze the client draws by Mie: a
    /// condensate aloft, with a refractive-index file or a stated stand-in.
    #[must_use]
    pub const fn aerosol_material(&self) -> bool {
        self.aerosol_material
    }
}

/// A row of the substance registry: one substance in every phase.
#[derive(Debug, Clone, PartialEq)]
pub struct Substance {
    key: SubstanceKey,
    kind: SubstanceKind,
    elements: &'static [(Element, u16)],
    charge: i8,
    molar_mass_g_per_mol: Option<f64>,
    gas: Option<GasColumns>,
    diagram: Option<PhaseDiagram>,
    condensed: Option<CondensedColumns>,
    optics: ClientOptics,
    sources: &'static [Source],
}

impl Substance {
    /// The row's key, its name on every wire.
    #[must_use]
    pub const fn key(&self) -> SubstanceKey {
        self.key
    }

    /// What kind of thing it is.
    #[must_use]
    pub const fn kind(&self) -> SubstanceKind {
        self.kind
    }

    /// Its stoichiometry, each element once with its count per formula unit; empty for a material
    /// with no single formula, and for the electron.
    #[must_use]
    pub const fn elements(&self) -> &'static [(Element, u16)] {
        self.elements
    }

    /// Its charge, in elementary charges: 0 but for an ion.
    #[must_use]
    pub const fn charge(&self) -> i8 {
        self.charge
    }

    /// Its molar mass, g mol⁻¹, for a species of one formula; `None` for a material with none.
    #[must_use]
    pub const fn molar_mass_g_per_mol(&self) -> Option<f64> {
        self.molar_mass_g_per_mol
    }

    /// Its gas columns, for a row that can be a gas.
    #[must_use]
    pub const fn gas(&self) -> Option<&GasColumns> {
        self.gas.as_ref()
    }

    /// Its phase diagram, for a row whose reference equations of state were read (P14.T49.b):
    /// where it condenses, and in which phase it lies.
    #[must_use]
    pub const fn phase_diagram(&self) -> Option<&PhaseDiagram> {
        self.diagram.as_ref()
    }

    /// Which of the client's optics the sim may ask of it.
    #[must_use]
    pub const fn optics(&self) -> ClientOptics {
        self.optics
    }

    /// Where its identity's values (the stoichiometry and the molar mass) come from; each column
    /// group names its own.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }
}

/// The row `id`.
#[must_use]
pub fn substance(id: SubstanceId) -> &'static Substance {
    // An id is only made for an index below the row count (`from_index`, `by_key`, `ids`, and
    // `Gas::substance`, whose nine rows a test holds), so the index is in range.
    &ROWS[usize::from(id.0)]
}

/// The row whose key is `key`, or `None` for a key the registry does not hold (a newer build's,
/// or no substance's).
///
/// # Examples
///
/// ```
/// use hyperion_sim::substance::by_key;
///
/// assert!(by_key("CO2").is_some());
/// // Case is chemistry: "co2" is no key of this registry.
/// assert_eq!(by_key("co2"), None);
/// ```
#[must_use]
pub fn by_key(key: &str) -> Option<SubstanceId> {
    ids().find(|id| id.substance().key.as_str() == key)
}

/// Every row, in registry order.
///
/// # Panics
///
/// Never: the registry holds far fewer than 65,536 rows, so each index fits a `u16`.
pub fn ids() -> impl ExactSizeIterator<Item = SubstanceId> {
    (0..ROWS.len())
        .map(|i| SubstanceId(u16::try_from(i).expect("the registry holds fewer than 65,536 rows")))
}

#[cfg(test)]
mod tests {
    use hyperion_surface::substance_key::ParseSubstanceKeyError;
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::units::Kelvin;

    /// A formula key's elements and their counts, merged by element in the order each first
    /// appears, and its charge: `CH3OH` is C 1, H 4, O 1 and 0; `H-` is H 1 and −1.
    fn parse_formula(key: &str) -> (Vec<(Element, u16)>, i8) {
        let bytes = key.as_bytes();
        let (mut counts, mut charge, mut i) = (Vec::<(Element, u16)>::new(), 0, 0);
        while i < bytes.len() {
            match bytes[i] {
                b'+' => charge = 1,
                b'-' => charge = -1,
                _ => {
                    let start = i;
                    i += 1;
                    if i < bytes.len() && bytes[i].is_ascii_lowercase() {
                        i += 1;
                    }
                    let element = Element::from_symbol(&key[start..i])
                        .unwrap_or_else(|| panic!("{key}: {} is an element", &key[start..i]));
                    let digits = i;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    // The grammar writes a count of 2 or more, with no leading zero, and leaves a
                    // count of 1 unwritten, so that a species has one spelling.
                    let count = if i > digits {
                        let written = &key[digits..i];
                        assert!(
                            !written.starts_with('0'),
                            "{key}: {written} has a leading zero"
                        );
                        let count = written.parse().unwrap();
                        assert!(count >= 2, "{key}: a count of {count} is never written");
                        count
                    } else {
                        1
                    };
                    match counts.iter_mut().find(|(e, _)| *e == element) {
                        Some((_, n)) => *n += count,
                        None => counts.push((element, count)),
                    }
                    continue;
                }
            }
            i += 1;
        }
        (counts, charge)
    }

    /// Rows 0–8 are P14.T13's gases in `Gas::ALL`'s order, and their molar masses are P14.T13's to
    /// the bit (the values `Gas::molar_mass_g_per_mol` returned before it read the registry).
    #[test]
    fn substance_rows_0_to_8_are_the_gases_in_order() {
        let t13 = [
            2.016, 4.003, 18.015, 16.043, 17.031, 28.014, 31.998, 44.009, 39.95,
        ];
        let keys = ["H2", "He", "H2O", "CH4", "NH3", "N2", "O2", "CO2", "Ar"];
        for (i, gas) in Gas::ALL.into_iter().enumerate() {
            let index = u16::try_from(i).unwrap();
            let id = gas.substance();
            assert_eq!(id.index(), index);
            assert_eq!(SubstanceId::from_index(index), Some(id));
            assert_eq!(id.gas(), Some(gas));
            assert_eq!(id.substance().key().as_str(), keys[i]);
            assert_same_bits(gas.molar_mass_g_per_mol(), t13[i]);
            assert_same_bits(id.substance().molar_mass_g_per_mol().unwrap(), t13[i]);
        }
        assert_eq!(ids().len(), 112);
        assert_eq!(SubstanceId::from_index(112), None);
        assert_eq!(SubstanceId::from_index(9).and_then(SubstanceId::gas), None);
    }

    /// The keys of P14.T49.b's rows 9–111, in its plan's order: the gases R08 measures, the
    /// gases of the chemistry, the atoms of T50.b's metals, their first ions and `HCl`, the
    /// condensates aloft, the hazes and dusts, then the solutions, lithologies, salts, organics
    /// and undifferentiated bodies' materials, in the order this builder lists them.
    const APPENDED: [&str; 103] = [
        "Ne",
        "Kr",
        "Xe",
        "N2O",
        "CO",
        "SO2",
        "H2S",
        "O3",
        "HCN",
        "C2H2",
        "C2H4",
        "C2H6",
        "PH3",
        "CH3OH",
        "H",
        "O",
        "e-",
        "H-",
        "Na",
        "K",
        "Fe",
        "Mg",
        "Si",
        "Ca",
        "Ti",
        "SiO",
        "TiO",
        "VO",
        "FeH",
        "Al",
        "V",
        "Mn",
        "Ni",
        "Zn",
        "Na+",
        "Mg+",
        "Al+",
        "Si+",
        "K+",
        "Ca+",
        "Ti+",
        "V+",
        "Cr+",
        "Mn+",
        "Fe+",
        "Ni+",
        "Zn+",
        "HCl",
        "NH4SH",
        "H2SO4",
        "S8",
        "KCl",
        "NaCl",
        "ZnS",
        "Na2S",
        "MnS",
        "Cr",
        "FeS",
        "MgSiO3",
        "Mg2SiO4",
        "SiO2",
        "Al2O3",
        "CaTiO3",
        "TiO2",
        "graphite",
        "tholin",
        "soot",
        "chromophore",
        "ice_giant_haze",
        "mars_dust",
        "acid_uv_absorber",
        "brine_nacl",
        "brine_mgso4",
        "brine_naclo4",
        "brine_mgclo4",
        "nh3_h2o",
        "ch4_c2h6_n2",
        "anorthosite",
        "feldspathic",
        "basalt",
        "basalt_fe",
        "komatiite",
        "basalt_reduced",
        "granite",
        "ultramafic",
        "ferric",
        "phyllosilicate",
        "lava_glass",
        "ca_al_residue",
        "iron_nickel",
        "SiC",
        "MgSO4",
        "Na2CO3",
        "CaCO3",
        "carbonaceous",
        "chondrite_ci",
        "chondrite_cm",
        "chondrite_cv",
        "chondrite_h",
        "chondrite_l",
        "chondrite_ll",
        "chondrite_e",
        "cometary",
    ];

    /// P14.T49.b's rows follow rows 0–8 in [`APPENDED`]'s order.
    #[test]
    fn substance_rows_9_onward_are_the_plan_s_in_order() {
        let keys: Vec<SubstanceKey> = ids().skip(9).map(|id| id.substance().key()).collect();
        let keys: Vec<&str> = keys.iter().map(SubstanceKey::as_str).collect();
        assert_eq!(keys, APPENDED);
        // A material of no single formula has neither a stoichiometry nor a molar mass.
        for id in ids() {
            let row = id.substance();
            let named = row.key().form() == SubstanceKeyForm::Name;
            let formless = row.elements().is_empty() && row.key().as_str() != "e-";
            assert_eq!(
                formless,
                row.molar_mass_g_per_mol().is_none(),
                "{:?}",
                row.key()
            );
            if formless {
                assert!(named, "{:?}", row.key());
            }
        }
    }

    /// The client-optics flags of P14.T49.b's rows (decision-r08-t4b-t12c, follow-ups 2 and 4):
    /// every gas row absorbs but the noble gases and the fifteen rows
    /// science-r08-unowned-absorbers adds, whose flag waits for R08's answers, and every gas row
    /// but the electron, H⁻ and the ions scatters; the condensates aloft and the hazes are aerosol materials and nothing else;
    /// a surface material is none of the three. Rows 0–8 keep generator version 22's flags until
    /// the bump, which turns on `absorber` for H₂, He, N₂ and CO₂ (P14.T54.a).
    #[test]
    fn substance_client_flags_follow_the_ruling() {
        for id in ids().skip(9) {
            let row = id.substance();
            let key = row.key();
            let key = key.as_str();
            let optics = row.optics();
            if row.gas().is_some() {
                let noble = matches!(key, "Ne" | "Kr" | "Xe");
                // The rows science-r08-unowned-absorbers adds wait for the client's answers.
                let awaiting = matches!(
                    key,
                    "Al" | "V"
                        | "Mn"
                        | "Ni"
                        | "Zn"
                        | "HCl"
                        | "Na+"
                        | "Al+"
                        | "Si+"
                        | "K+"
                        | "V+"
                        | "Cr+"
                        | "Mn+"
                        | "Ni+"
                        | "Zn+"
                );
                assert_eq!(optics.absorber(), !noble && !awaiting, "{key}");
                assert_eq!(optics.rayleigh(), row.kind() != SubstanceKind::Ion, "{key}");
            } else {
                assert!(!optics.rayleigh() && !optics.absorber(), "{key}");
            }
        }
        for key in [
            "NH4SH",
            "H2SO4",
            "S8",
            "KCl",
            "NaCl",
            "ZnS",
            "Na2S",
            "MnS",
            "Cr",
            "FeS",
            "MgSiO3",
            "Mg2SiO4",
            "SiO2",
            "Al2O3",
            "CaTiO3",
            "TiO2",
            "graphite",
            "tholin",
            "soot",
            "chromophore",
            "ice_giant_haze",
            "mars_dust",
            "acid_uv_absorber",
            "Fe",
            "CO",
            "H2S",
            "C2H6",
            "basalt",
            "basalt_fe",
            "lava_glass",
        ] {
            assert!(
                by_key(key).unwrap().substance().optics().aerosol_material(),
                "{key}"
            );
        }
        // A gas whose condensate no index file serves yet is no aerosol material until R08 adds
        // one.
        for key in ["SO2", "HCN", "C2H2", "N2O", "Ne"] {
            assert!(
                !by_key(key).unwrap().substance().optics().aerosol_material(),
                "{key}"
            );
        }
        for key in [
            "granite",
            "brine_nacl",
            "chondrite_ci",
            "MgSO4",
            "SiC",
            "carbonaceous",
        ] {
            assert_eq!(
                by_key(key).unwrap().substance().optics(),
                ClientOptics::default(),
                "{key}"
            );
        }
        for gas in [
            Gas::Hydrogen,
            Gas::Helium,
            Gas::Nitrogen,
            Gas::CarbonDioxide,
        ] {
            assert!(!gas.substance().substance().optics().absorber(), "{gas:?}");
        }
    }

    #[test]
    fn substance_keys_are_unique_and_grammatical() {
        for id in ids() {
            let row = id.substance();
            let key = row.key();
            assert_eq!(by_key(key.as_str()), Some(id), "{key} names its own row");
            assert_eq!(key.as_str().parse::<SubstanceKey>(), Ok(key));
            assert_eq!(
                ids().filter(|other| other.substance().key() == key).count(),
                1,
                "{key} is unique"
            );
            let form = key.form();
            match row.kind() {
                SubstanceKind::Molecule | SubstanceKind::Atom => {
                    assert_eq!(form, SubstanceKeyForm::Formula, "{key}");
                }
                SubstanceKind::Ion => assert_ne!(form, SubstanceKeyForm::Name, "{key}"),
                SubstanceKind::Mineral
                | SubstanceKind::Lithology
                | SubstanceKind::Solution
                | SubstanceKind::Organic => {}
            }
        }
        assert_eq!(by_key("h2o"), None);
        assert_eq!(by_key("Co"), None);
        assert_eq!(by_key(""), None);
        // One spelling: carbon dioxide is `CO2`, never `C1O2` or `CO02`, which are no keys.
        for (other, at) in [("C1O2", 1), ("CO02", 2), ("H2O1", 3)] {
            assert_eq!(
                SubstanceKey::new(other),
                Err(ParseSubstanceKeyError::Malformed { at }),
                "{other}"
            );
            assert_eq!(by_key(other), None);
        }
    }

    /// A formula key's symbols are elements and its element counts equal its row's stoichiometry,
    /// each written once as 2 or more with no leading zero, or left unwritten for 1, so that a
    /// species has one spelling; its sign is its charge. The stoichiometry lists each element
    /// once, with a count of at least one.
    #[test]
    fn substance_formula_keys_match_their_stoichiometry() {
        for id in ids() {
            let row = id.substance();
            let elements = row.elements();
            for (i, (element, count)) in elements.iter().enumerate() {
                assert!(*count >= 1, "{:?}", row.key());
                assert!(
                    elements[..i].iter().all(|(other, _)| other != element),
                    "{:?} lists {element:?} once",
                    row.key()
                );
            }
            if row.key().form() != SubstanceKeyForm::Formula {
                continue;
            }
            let (counts, charge) = parse_formula(row.key().as_str());
            let mut parsed = counts;
            let mut listed = elements.to_vec();
            parsed.sort_unstable();
            listed.sort_unstable();
            assert_eq!(parsed, listed, "{:?}", row.key());
            assert_eq!(charge, row.charge(), "{:?}", row.key());
            match row.kind() {
                SubstanceKind::Atom => {
                    assert_eq!(elements.len(), 1, "{:?}", row.key());
                    assert_eq!(elements[0].1, 1, "{:?}", row.key());
                    assert_eq!(row.charge(), 0);
                }
                SubstanceKind::Molecule => assert_eq!(row.charge(), 0, "{:?}", row.key()),
                SubstanceKind::Ion => assert_ne!(row.charge(), 0, "{:?}", row.key()),
                SubstanceKind::Mineral
                | SubstanceKind::Lithology
                | SubstanceKind::Solution
                | SubstanceKind::Organic => {}
            }
        }
        assert_eq!(
            parse_formula("CH3OH").0,
            [(Element::C, 1), (Element::H, 4), (Element::O, 1)]
        );
        assert_eq!(parse_formula("H-"), (vec![(Element::H, 1)], -1));
    }

    /// Each molar mass is its formula's sum of IUPAC 2021's abridged standard atomic weights
    /// (Prohaska et al. 2022, Pure Appl. Chem. 94, 573, as CIAAW prints them) to their last place,
    /// an ion's less its charge in electrons.
    #[test]
    fn substance_molar_masses_are_their_formulae_s() {
        // P14.T50.a's standard atomic weights, CIAAW's abridged values, the element registry's.
        // The electron's molar mass, g mol⁻¹ (CODATA 2018), which an ion's charge takes away.
        let electron = 5.485_799_088_8e-4;
        for id in ids() {
            let row = id.substance();
            let Some(molar_mass) = row.molar_mass_g_per_mol() else {
                continue;
            };
            let sum: f64 = row
                .elements()
                .iter()
                .map(|&(e, n)| f64::from(n) * e.standard_atomic_weight_g_per_mol())
                .sum::<f64>()
                - f64::from(row.charge()) * electron;
            assert!(
                (molar_mass - sum).abs() < 5e-4,
                "{:?}: {molar_mass} against {sum}",
                row.key()
            );
        }
    }

    /// Every value names its sources: the identity, every gas column and every condensed column,
    /// with each condensed value covered by name in some source's comma-separated `covers` (the
    /// phase diagrams' own test, `diagram_values_name_their_sources`, holds theirs).
    #[test]
    fn substance_values_name_their_sources() {
        let named = |sources: &[Source]| {
            assert_ne!(sources, []);
            for s in sources {
                assert!(!s.covers().is_empty() && !s.citation().is_empty(), "{s:?}");
            }
        };
        for id in ids() {
            let row = id.substance();
            named(row.sources());
            if let Some(gas) = row.gas() {
                named(gas.sources());
            }
            if let Some(condensed) = row.condensed() {
                named(condensed.sources());
                for value in ["grain density", "transition", "mechanics"] {
                    assert!(
                        condensed
                            .sources()
                            .iter()
                            .any(|s| s.covers().split(", ").any(|covered| covered == value)),
                        "{:?}'s {value} names a source",
                        row.key()
                    );
                }
            }
        }
    }

    /// Every row that is no gas has condensed columns, as has every row with a phase diagram, whose
    /// columns are its diagram's; every other row's are a fixed density, finite and positive, of a
    /// stated phase at a stated temperature, and every transition is finite and positive, a
    /// eutectic's composition a mass fraction.
    #[test]
    fn substance_condensed_columns_cover_the_condensed_rows() {
        let mut count = 0;
        for id in ids() {
            let row = id.substance();
            let key = row.key();
            if row.gas().is_none() || row.phase_diagram().is_some() {
                assert!(row.condensed().is_some(), "{key} has condensed columns");
            }
            let Some(condensed) = row.condensed() else {
                continue;
            };
            count += 1;
            let transition = condensed.transition();
            let t = transition.temperature().value();
            assert!(t.is_finite() && t > 0.0, "{key}: transition {t} K");
            match (condensed.grain_density(), row.phase_diagram()) {
                (GrainDensity::OfDiagram, Some(diagram)) => {
                    assert_eq!(transition.kind(), TransitionKind::TriplePoint, "{key}");
                    assert_same_bits(t, diagram.triple_temperature().value());
                }
                (GrainDensity::OfDiagram, None) => panic!("{key} has no diagram to read"),
                (GrainDensity::Fixed { solid, liquid }, diagram) => {
                    assert!(
                        diagram.is_none(),
                        "{key} has a diagram, whose densities it takes"
                    );
                    assert!(solid.is_some() || liquid.is_some(), "{key} gives a phase");
                    for given in solid.iter().chain(&liquid) {
                        let rho = given.density().value();
                        assert!(rho.is_finite() && rho > 0.0, "{key}: {rho} kg m⁻³");
                        assert!(given.at().value() > 0.0, "{key}: {:?}", given.at());
                    }
                    assert_ne!(transition.kind(), TransitionKind::TriplePoint, "{key}");
                }
            }
            if let TransitionKind::Eutectic {
                solute_mass_fraction,
            } = transition.kind()
            {
                assert!(
                    solute_mass_fraction > 0.0 && solute_mass_fraction < 1.0,
                    "{key}: {solute_mass_fraction}"
                );
                // `H2SO4` is a molecule whose condensed columns are its 75 wt% aqueous acid's.
                let expected = if key.as_str() == "H2SO4" {
                    SubstanceKind::Molecule
                } else {
                    SubstanceKind::Solution
                };
                assert_eq!(row.kind(), expected, "{key}");
            }
        }
        assert_eq!(count, 71);
    }

    /// The first melt is every row's transition (decision-p14-t49b-condensed §2): the frozen
    /// 75 wt% acid, and the absorber it carries, first melt at the dihydrate–monohydrate
    /// eutectic: at its upper bound, the dihydrate's 233.67 K, below the 75 wt% acid's liquidus,
    /// with the eutectic's 0.738 `H2SO4` by mass, between the dihydrate's composition and the
    /// row's own.
    #[test]
    fn substance_the_acid_first_melts_at_its_hydrates_eutectic() {
        for key in ["H2SO4", "acid_uv_absorber"] {
            let transition = by_key(key)
                .unwrap()
                .substance()
                .condensed()
                .unwrap()
                .transition();
            let TransitionKind::Eutectic {
                solute_mass_fraction,
            } = transition.kind()
            else {
                panic!("{key}: {:?} is no eutectic", transition.kind());
            };
            let t = transition.temperature().value();
            assert_same_bits(t, 233.67);
            assert_same_bits(solute_mass_fraction, 0.738);
            assert!(
                (0.7313..0.75).contains(&solute_mass_fraction),
                "{key}: {solute_mass_fraction} lies between the dihydrate's and the row's own"
            );
            let liquidus_lower_k = 242.0 - 2.0;
            assert!(
                t < liquidus_lower_k,
                "{key}: the first melt, {t} K, lies below the 75 wt% acid's liquidus"
            );
        }
    }

    /// A row resolves its grain density as its columns give it: a diagram row through its
    /// diagram, ice Ih's to the bit; a fixed row its one value for its own phase at any
    /// temperature, and none for the other; a row with no columns none.
    #[test]
    fn substance_grain_density_resolves_each_form() {
        let water = by_key("H2O").unwrap().substance();
        let diagram = water.phase_diagram().unwrap();
        for t in [20.0, 100.0, 250.0] {
            let at = Kelvin::new(t);
            assert_same_bits(
                water
                    .grain_density(CondensedPhase::Solid, at)
                    .unwrap()
                    .value(),
                ice_ih_density(at).value(),
            );
        }
        let warm = Kelvin::new(300.0);
        assert_same_bits(
            water
                .grain_density(CondensedPhase::Liquid, warm)
                .unwrap()
                .value(),
            diagram.liquid_density(warm).unwrap().value(),
        );
        // No ice above the triple point at low pressure, and no saturated liquid below it.
        assert_eq!(water.grain_density(CondensedPhase::Solid, warm), None);
        assert_eq!(
            water.grain_density(CondensedPhase::Liquid, Kelvin::new(250.0)),
            None
        );
        // Argon's solid density was not read: not modelled.
        let argon = Gas::Argon.substance().substance();
        assert_eq!(
            argon.grain_density(CondensedPhase::Solid, Kelvin::new(60.0)),
            None
        );
        let brine = by_key("brine_nacl").unwrap().substance();
        for t in [252.0, 273.15, 300.0] {
            let rho = brine.grain_density(CondensedPhase::Liquid, Kelvin::new(t));
            assert_same_bits(rho.unwrap().value(), 1_184.1);
        }
        assert_eq!(
            brine.grain_density(CondensedPhase::Solid, Kelvin::new(240.0)),
            None
        );
        let helium = Gas::Helium.substance().substance();
        assert_eq!(helium.condensed(), None);
        assert_eq!(
            helium.grain_density(CondensedPhase::Liquid, Kelvin::new(3.0)),
            None
        );
    }

    /// Titan's sea liquid's density is its components' rows' saturated liquids mixed ideally by
    /// volume at its reference composition, volume fractions CH₄ 0.71, C₂H₆ 0.12 and N₂ 0.17
    /// (Mastrogiuseppe et al. 2016, §V.B), and 93.65 K: the pinned figure to its last place.
    ///
    /// The mole fractions its density source derives, 0.7391, 0.0965 and 0.1644, follow from the
    /// volume fractions by the same densities and the rows' molar masses, each rounded to four
    /// places from its own exact value, so the three need not sum to 1
    /// (decision-p14-t49b-condensed §1.2).
    #[test]
    fn substance_titan_liquid_is_its_components_mixed_ideally() {
        let at = Kelvin::new(93.65);
        let mut mixed = 0.0;
        let mut moles = [0.0; 3];
        let components = [
            ("CH4", 0.71, 0.7391),
            ("C2H6", 0.12, 0.0965),
            ("N2", 0.17, 0.1644),
        ];
        for ((key, phi, _), mol) in components.iter().zip(&mut moles) {
            let row = by_key(key).unwrap().substance();
            let rho = row
                .phase_diagram()
                .unwrap()
                .liquid_density(at)
                .unwrap()
                .value();
            mixed += phi * rho;
            *mol = phi * rho / row.molar_mass_g_per_mol().unwrap();
        }
        let total: f64 = moles.iter().sum();
        for ((key, _, stated), mol) in components.iter().zip(moles) {
            let x = mol / total;
            assert!(
                (x - stated).abs() <= 5e-5,
                "{key}: mole fraction {x} rounds to four places as {stated}"
            );
        }
        let titan = by_key("ch4_c2h6_n2").unwrap().substance();
        let GrainDensity::Fixed {
            solid: None,
            liquid: Some(given),
        } = titan.condensed().unwrap().grain_density()
        else {
            panic!("Titan's liquid is a liquid's fixed density");
        };
        assert_eq!(given.at(), at);
        let density = given.density().value();
        assert!((density - mixed).abs() < 0.05, "{density} against {mixed}");
    }

    /// The condensables are the rows with a phase diagram, in registry order (the diagrams' own
    /// test, `diagram_condensables_are_the_fourteen_in_registry_order`, names them), and the
    /// functions re-exported here are the diagrams': each row's triple point lies below its
    /// critical point, its latent heats are positive and its sublimation, where its enthalpy of
    /// fusion was read, takes more than its vaporisation.
    #[test]
    fn substance_condensables_are_the_diagram_rows_in_registry_order() {
        let found: Vec<_> = condensables().collect();
        let diagrams: Vec<_> = ids()
            .filter(|id| id.substance().phase_diagram().is_some())
            .collect();
        assert_eq!(found, diagrams);
        assert_eq!(found.len(), 14);
        assert!(found.windows(2).all(|pair| pair[0] < pair[1]));
        for id in condensables() {
            let d = id.substance().phase_diagram().unwrap();
            assert!(d.triple_temperature() < d.critical_temperature());
            assert!(d.triple_pressure() < d.critical_pressure());
            let vaporisation = d.triple_vaporisation_enthalpy_j_per_mol();
            assert!(vaporisation > 0.0);
            if let Some(sublimation) = d.triple_sublimation_enthalpy_j_per_mol() {
                assert!(sublimation > vaporisation);
            }
            assert!(d.triple_liquid_density().value() > 0.0);
            let t = Kelvin::new(f64::midpoint(
                d.triple_temperature().value(),
                d.critical_temperature().value(),
            ));
            assert_eq!(saturation_pressure(id, t), Some(d.saturation_pressure(t)));
            let p = d.saturation_pressure(t) * 2.0;
            assert_eq!(phase_at(id, t, p), d.phase_at(t, p));
        }
    }

    /// Every one of P14.T13's gases is a gas row the client may scatter, holding generator
    /// version 22's class; every later gas row has its molecular constants and no class; and the
    /// kinds name themselves.
    #[test]
    fn substance_gas_rows_carry_their_gas_columns() {
        for gas in Gas::ALL {
            let row = gas.substance().substance();
            assert!(row.gas().is_some(), "{gas:?}");
            assert!(row.optics().rayleigh(), "{gas:?}");
            assert!(row.gas().unwrap().heat_capacity().is_some(), "{gas:?}");
        }
        for id in ids().skip(9) {
            if let Some(gas) = id.substance().gas() {
                assert_eq!(gas.heat_capacity(), None, "{:?}", id.substance().key());
                assert_eq!(gas.heat_capacity_over_r(), None);
            }
        }
        assert_eq!(SubstanceKind::Molecule.name(), "molecule");
        assert_eq!(SubstanceKind::Atom.name(), "atom");
    }
}
