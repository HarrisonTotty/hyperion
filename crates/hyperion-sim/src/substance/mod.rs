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
//!   read, and `crates/hyperion-sim/tests/golden/substance/registry.golden` pins the rows' values.
//!   A test writes both and holds each to its committed rows as a prefix.
//!
//! # Columns
//!
//! A row carries its identity (key, [`SubstanceKind`], stoichiometry, charge, molar mass) and then
//! groups of columns, each `None` where the substance has no such data: so far the gas columns
//! ([`GasColumns`]) and the phase columns ([`PhaseColumns`]). Every value names its sources and
//! their licence basis ([`Source`], [`Basis`]; decision-composition §5). P14.T49.b–e add the
//! later rows and the fitted phase data, the condensed optics, the thermochemistry and the
//! aerosols' extinction.
//!
//! # Rows
//!
//! Rows 0–8 are `H2`, `He`, `H2O`, `CH4`, `NH3`, `N2`, `O2`, `CO2` and `Ar`: the nine gases of
//! P14.T13, in [`Gas::ALL`]'s order, so that every sum over them keeps its bits.
//! [`Gas`] stays the typed view of these nine for escape and the greenhouse ([`Gas::substance`],
//! [`SubstanceId::gas`]). Five carry phase data, in registry order water, methane, nitrogen,
//! carbon dioxide and argon ([`condensables`]).

pub mod element;
pub mod mix;
pub mod phase;
mod rows;

pub use element::Element;
pub use hyperion_surface::substance_key::{SubstanceKey, SubstanceKeyForm};
pub use mix::{GasColumns, HeatCapacityClass, heat_capacity_over_r, mean_molar_mass_g_per_mol};
pub use phase::{MOLAR_GAS_CONSTANT, Phase, PhaseColumns, phase_at, saturation_pressure};

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
    phase: Option<PhaseColumns>,
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

    /// Its phase columns, for a row that condenses.
    #[must_use]
    pub const fn phase(&self) -> Option<&PhaseColumns> {
        self.phase.as_ref()
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
#[must_use]
pub fn ids() -> impl ExactSizeIterator<Item = SubstanceId> {
    (0..ROWS.len())
        .map(|i| SubstanceId(u16::try_from(i).expect("the registry holds fewer than 65,536 rows")))
}

/// The rows that condense, those with phase columns, in registry order: so far water, methane,
/// nitrogen, carbon dioxide and argon. A climate loops over them in this order (P14.T48.d), so a
/// later row joins as data.
pub fn condensables() -> impl Iterator<Item = SubstanceId> {
    ids().filter(|id| id.substance().phase.is_some())
}

#[cfg(test)]
mod tests {
    use hyperion_surface::substance_key::ParseSubstanceKeyError;
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

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
        assert_eq!(ids().len(), 9);
        assert_eq!(SubstanceId::from_index(9), None);
        assert_eq!(SubstanceId::from_index(9).and_then(SubstanceId::gas), None);
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
    /// (Prohaska et al. 2022, Pure Appl. Chem. 94, 573) to their last place.
    #[test]
    fn substance_molar_masses_are_their_formulae_s() {
        let weight = |element: Element| match element {
            Element::H => 1.0080,
            Element::He => 4.0026,
            Element::C => 12.011,
            Element::N => 14.007,
            Element::O => 15.999,
            Element::Ar => 39.95,
            other => panic!("{other:?} has no weight here yet (P14.T50.a's)"),
        };
        for id in ids() {
            let row = id.substance();
            let sum: f64 = row
                .elements()
                .iter()
                .map(|&(e, n)| f64::from(n) * weight(e))
                .sum();
            let molar_mass = row.molar_mass_g_per_mol().unwrap();
            assert!(
                (molar_mass - sum).abs() < 5e-4,
                "{:?}: {molar_mass} against {sum}",
                row.key()
            );
        }
    }

    /// Every value names its sources: the identity, every gas column and every phase column, with
    /// each phase value covered by name in some source's comma-separated `covers`.
    #[test]
    fn substance_values_name_their_sources() {
        let named = |sources: &[Source]| {
            assert!(!sources.is_empty());
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
            if let Some(phase) = row.phase() {
                named(phase.sources());
                for value in ["T_t", "p_t", "T_c", "p_c", "L_sub", "L_vap", "ρ_l(T_t)"] {
                    assert!(
                        phase
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

    /// The condensables are water, methane, nitrogen, carbon dioxide and argon, in registry
    /// order, each with a triple point below its critical point and the enthalpy of sublimation
    /// above that of vaporisation.
    #[test]
    fn substance_condensables_are_in_registry_order() {
        let found: Vec<_> = condensables().collect();
        let expected = [
            Gas::Water,
            Gas::Methane,
            Gas::Nitrogen,
            Gas::CarbonDioxide,
            Gas::Argon,
        ]
        .map(Gas::substance);
        assert_eq!(found, expected);
        for id in condensables() {
            let phase = id.substance().phase().unwrap();
            assert!(phase.triple_temperature() < phase.critical_temperature());
            assert!(phase.triple_pressure() < phase.critical_pressure());
            assert!(
                phase.sublimation_enthalpy_j_per_mol() > phase.vaporisation_enthalpy_j_per_mol()
            );
            assert!(phase.vaporisation_enthalpy_j_per_mol() > 0.0);
            assert!(phase.triple_liquid_density().value() > 0.0);
        }
    }

    /// Every gas row is one the client may scatter, and its kinds name themselves.
    #[test]
    fn substance_gas_rows_carry_their_gas_columns() {
        for gas in Gas::ALL {
            let row = gas.substance().substance();
            assert!(row.gas().is_some(), "{gas:?}");
            assert!(row.optics().rayleigh(), "{gas:?}");
        }
        assert_eq!(SubstanceKind::Molecule.name(), "molecule");
        assert_eq!(SubstanceKind::Atom.name(), "atom");
    }
}
