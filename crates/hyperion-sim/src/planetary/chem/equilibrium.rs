//! Equilibrium speciation of a hydrogen-dominated gas (plan 14, P14.T52.a; decision-composition
//! §1.3): from a gas's element abundances, its temperature and its pressure, the mole fraction of
//! every registry gas that carries them, on the Surface path.
//!
//! A [`Network`] holds the registry's gas rows that have a thermochemical record (P14.T49.d's
//! [`substance::formation`](crate::substance::formation)), and [`Network::solve`] gives their
//! [`GasEquilibrium`] at one level: the 1-bar level, the quench level or the photosphere of an
//! envelope (P14.T52.d chooses them), or a thin H₂ air once P14.T53.a keeps one. Nothing on the
//! record reads it until P14.T54.b, so no generated output moves with it.
//!
//! # The network
//!
//! Every gas row of the registry with a CEA record is a species, in registry order: nothing is
//! chosen by name, so a row added to the registry with its record joins the network. The
//! elements fall in two sets.
//!
//! - **The major elements**, H, C, N, O and S ([`MAJOR_ELEMENTS`]), each with a basis species,
//!   H₂, CH₄, NH₃, H₂O and H₂S, and the charge with the free electron. Each species' partial
//!   pressure is K × Π `p_b^ν` over the basis, K its formation from the basis from the species' own
//!   Gibbs energies ([`Reaction`]). These are Heng and Tsai's (2016, ApJ 829, 104, §2.1, their
//!   eq. 1) nine C–H–O–N molecules and their six reactions, with H₂S carrying the sulphur
//!   (Visscher, Lodders and Fegley 2006, ApJ 648, 1181: "Hydrogen sulfide (H₂S) is the dominant
//!   S-bearing gas throughout substellar atmospheres") in its field (the reach, below), and every
//!   other registry gas of these elements: H, H⁻, O, O₂, O₃, N₂O, SO₂, CH₃OH and C₂H₆.
//! - **The trace elements**, every other element, each in species that hold one atom of it and no
//!   other trace element: each metal as its neutral atom and its first ion, with SiO, TiO and VO;
//!   phosphorus as PH₃, chlorine as HCl, the noble gases as their atoms. Given the major
//!   elements' pressures, a trace element's species share its abundance in closed form, each in
//!   proportion to its K × Π `p_b^ν`, so that each element's species sum to its abundance exactly.
//!   Neither second ions nor H⁺ is carried: second ions hold under 10⁻³ of the first below
//!   5,000 K at 10 mbar or more (plan 14), though Ca²⁺ reaches 1.3% of Ca⁺ at 6,000 K and 10 mbar
//!   and 14% at 10⁻⁴ bar (Saha, Ca II's 11.87 eV); H⁺ holds 2.0 × 10⁻⁷ of the hydrogen at 4,600 K
//!   and 0.05 bar, but 1.0 × 10⁻⁵ at 5,000 K and 10 mbar, and gives most of the electrons above
//!   about 5,000 K at 10 mbar or less (the reach, below; P14.T52.e). A row that held two
//!   trace atoms would break the closed form, and [`Network::registry`] refuses it
//!   ([`BuildNetworkError::TwoTraceAtoms`]); P14.T52.e's P₂ and chlorides will need P and Cl among
//!   the solved elements.
//!
//! The charge is balanced over the metals' first ions, the electrons and H⁻: each ion's K is that
//! of M ⇌ M⁺ + e⁻ from CEA's records, the Saha equation with the species' own partition
//! functions.
//!
//! # The solution
//!
//! The gas is ideal, at the total pressure given. The unknowns are the natural logarithms of the
//! basis species' pressures and of the hydrogen nuclei's (N, the pressure the hydrogen would exert
//! as atoms), in bar; the equations are each major element's balance against its abundance times
//! N, the charge's balance and the total pressure, each a difference of logarithms (held against
//! wanted), so that a balance carried by one species is linear in the unknowns.
//!
//! - **The start** is the closed form of each network's leading pair at the hydrogen's pressure
//!   (Heng, Lyons and Tsai 2016, ApJ 816, 96, §4; Heng and Tsai 2016, §3): H₂ against H over the
//!   helium and the trace elements, a quadratic in √p(H₂); CO against CH₄ with H₂O and O, a
//!   quadratic in p(CO), whatever the C/O; the carbon left over shared between CH₄ and C₂H₂, and
//!   NH₃ against N₂, each a quadratic; H₂S holding all the sulphur; and the electrons by Saha's
//!   closed form, every ion a minor share of its element.
//! - **The refinement**: Newton's method on all the equations at once, at most [`NEWTON_STEPS`]
//!   steps, each with its exact Jacobian (the trace elements' closed forms differentiated with
//!   them), each at most [`STEP_LIMIT`] in any logarithm, none taking a major species above the
//!   total pressure, and each halved up to [`BACKTRACKS`] times until it lowers the sum of the
//!   squared residuals. A step that lowers it at none leaves the unknowns as they are, and so would
//!   every later step: the solution stops there, which is the fixed count's result to the bit.
//!   From a fixed start, with every sum taken in the network's order and every transcendental
//!   through [`math`], the solution is the same bits on every target (decision-composition §1.3's
//!   determinism rules).
//! - **The check**: a solution whose largest residual exceeds [`CONVERGED`] is refused
//!   ([`SolveEquilibriumError::NotConverged`]) rather than returned; over the tests' domain none
//!   is.
//!
//! Heng and Tsai solve their network at the H₂ pressure taken as the total, with all the hydrogen
//! molecular and no helium (their §3: "if we make the simplification that `2n_H2 = n_H`"); their
//! decic in CO, or in practice its quintic (§5.1), is "accurate at the ∼1% level for temperatures
//! from 500 to 3000 K" (their abstract). The refinement is added (plan 14's rule: "only if the
//! asserted values below fail the stated tolerance") because that solution alone conserves no
//! element to the plan's 10⁻¹², nor carries
//! the dissociated hydrogen, the helium's share of the pressure or the oxygen in SiO, TiO and VO;
//! with it, the result is the network's exact equilibrium, and Heng and Tsai's closed forms its
//! start.
//!
//! Below the records' floor ([`Network::floor`], 300 K, where every species' record holds) the
//! composition is the floor's equilibrium at the same pressure, held ([`Regime::HeldAtFloor`]), so
//! that it is continuous at the floor. At a bar or more the floor's major carriers are all but
//! all-in, the plan's limit below 500 K (CH₄ holds over 0.999 999 of the carbon and NH₃ over 0.999
//! of the nitrogen at 300 K and 1 bar), and lower still the equilibrium would be more so; at low
//! pressure the floor keeps some N₂ (9% of the nitrogen at 10⁻⁴ bar in FastChem's run), which
//! colder equilibrium would turn to NH₃. There the composition is the quench level's in any case,
//! deeper and hotter (P14.T52.b), and the cold traps (P14.T52.c) remove the metals far below. Above
//! the records' ceiling, 6,000 K, nothing is extrapolated: the solution refuses
//! ([`SolveEquilibriumError::AboveRecords`]).
//!
//! # Validation
//!
//! Against FastChem 4.0.3 (Stock et al. 2018, MNRAS 479, 865; Stock, Kitzmann and Patzer 2022,
//! MNRAS 517, 4070; Kitzmann, Stock and Patzer 2024, MNRAS 527, 7263), run by hand outside the
//! repository and asserted only (decision-composition §5; the tests' documentation gives the
//! version, the inputs and the command):
//!
//! - **In Heng and Tsai's domain** (500–3,000 K, 10⁻²–100 bar; C, H, O, N and He at one, ten and
//!   a hundred times protosolar and at C/O 1 and 1.5, FastChem's full network), every species
//!   holding at least 1% of an element agrees within 7% at C/O 1 or less: N₂ −6.0% at 500 K and
//!   1 bar (ten times protosolar, where it holds 4.8% of the nitrogen), H₂O +6.5% at 2,500 K and
//!   10 mbar (where species not carried hold 3.3% of the oxygen), CO₂ +6.6% at 2,500 K and 10 mbar
//!   (a hundred times protosolar), CO and NH₃ within 3.4%. Minor species differ more, by the two
//!   thermochemistries: CEA's HCN 15–45% above FastChem's, C₂H₂ 15–29% below, CH₄ beside CO
//!   10–36% above at 2,000–2,500 K. At C/O 1.5 and 2,000–2,500 K, species not carried (C₂H above
//!   all) hold 2–11% of an element, and CH₄ and C₂H₂ miss by up to 63% and 50%.
//! - **Network-matched** (FastChem given this network's species alone, every element but Ni, P
//!   and Cl, 300–6,000 K × 10⁻⁶–10⁴ bar at a hundredth to a hundred times protosolar and C/O 1
//!   and 1.5, at the points where FastChem's atoms of S, N and C, not carried here, hold under
//!   0.3% of their element), every carrier holding at least half of its element agrees within 1%
//!   but vanadium and the edges of the domain (CH₄ 8.8% and CO 6.7% at 10⁴ bar and
//!   2,500–3,000 K; the water-starved Ti at C/O 1 and 1.5): with one network, one set of
//!   abundances and an ideal gas, the equilibrium is unique, so what differs is the
//!   thermochemistry. Vanadium differs most: CEA's VO record (Gurvich 1982) is about 19 kJ mol⁻¹
//!   less bound than FastChem's JANAF fit, so its K is 0.10–0.41 of FastChem's at 1,000–2,500 K,
//!   VO misses by up to −42% where it holds most of the vanadium, and the V atom is 1.1–10 times
//!   FastChem's.
//!
//! The tests assert ten such cases: in Heng and Tsai's domain every carrier within 2.5% (CO at
//! 1,100 K and 1 bar the farthest, 1.9%); in the network-matched ones each carrier holding most of
//! its element within 1% (2% for H₂O and CO at 2,500 K and 100 bar) but vanadium, and the rest
//! within their thermochemistries' difference, stated entry by entry (5% for minor CO, CO₂, CH₄,
//! NH₃ and TiO, HCN 20%, CH₄ beside CO 30% at 2,500 K, Ti 25%, VO 20–80% and the V atom up to 4.5
//! times). The gas is ideal: the solution's arithmetic holds to 10⁴ bar, but its physics only to
//! about 1 kbar, where H₂'s compressibility reaches 1.2 at 1,000 K (Leachman et al. 2009, through
//! NIST's WebBook) and FastChem's own tests stop.
//!
//! # Where it stops
//!
//! A gas is hydrogen-dominated while its all-in hydrides would take at most
//! [`HYDROGEN_DOMINANCE_BOUND`] of its hydrogen; beyond, [`Network::solve`] refuses it
//! ([`SolveEquilibriumError::NotHydrogenDominated`]), and the record says its speciation is not
//! modelled (P14.T52.d); P14.T53.a's hybrid airs take such gases. At protosolar ratios the hydrides
//! take 2.58 × 10⁻³ of the hydrogen, so the bound is an enrichment of about 194 (metals 0.24 per H
//! atom), which P14.T50.b's E(M) = 2.65 (M ÷ `M_J`)^−0.986 reaches near 4.1–4.7 M⊕ for a host of
//! solar metallicity (E about 194 to 170, by the formation zone's C/O), and at roughly twice the
//! mass for a host at \[Fe/H\] = +0.3 (decision-p14-t50b-envelope §5: "Below about 5 M⊕ an
//! envelope's metals exceed 0.2 per H atom").
//!
//! The network is the registry's, so a species that is not a row is not carried ([`NOT_CARRIED`],
//! a closed set, for the composition audit), and its element is held by the carriers that are.
//! From FastChem's gas-only grid at protosolar abundances:
//!
//! - **Sulphur**: H₂S holds the sulphur in Visscher, Lodders and Fegley's H₂S field (§3.1: SH
//!   takes over where log p < 6.82 − 17,570 ÷ T, their eq. 4, and S where log p < 4.80 −
//!   14,522 ÷ T, eq. 7, and below the triple point at about 1,509 K where log p < 5.81 − 16,046 ÷
//!   T, eq. 10): to about 1,990 K at 10 mbar and 2,580 K at 1 bar. Beyond it the network has no
//!   S or SH, and H₂S holds what they would.
//! - **Nitrogen and carbon**: N₂ and CO hold their element to 3,000 K at 10 mbar or more; above,
//!   the atoms N and C, not carried, take over (N₂ holds 26% of the nitrogen at 4,000 K and
//!   10 mbar, CO 4.9% of the carbon at 5,000 K).
//! - **Oxygen**: OH, not carried, holds up to 21% of it (6,000 K, 100 bar), 15% at 4,000 K and
//!   1 bar, 12% at 3,000 K and 10 mbar; H₂O and O hold it here.
//! - **Phosphorus and chlorine**: PH₃ is the carrier only where FastChem gives it half the
//!   phosphorus or more, at 1 bar or more to 1,200 K and at 100 bar to 1,500 K; HCl roughly from
//!   1,200 to 3,000 K at the middle pressures. Elsewhere P₂, PO, PH₂ and P, and Cl and the
//!   chloride gases, carry them, and PH₃ and HCl are published beyond their validity, a stated
//!   miss for trace elements.
//! - **Titanium and vanadium**: TiO₂ and VO₂, not carried, hold 27–42% of the titanium and
//!   53–68% of the vanadium at 1,500 K, and most of both below, under P14.T52.c's CaTiO₃ trap; TiO
//!   and VO are overstated by up to about 2× at 1,500 K.
//! - **The electrons and H⁻** are valid where H⁺ and the ions of the non-metals give under 10% of
//!   the electrons: on the grid to 4,000 K at 10⁻⁴ bar or more, to 5,000 K at 10 mbar or more,
//!   and at 6,000 K only at 100 bar. Above about 5,000 K at 10 mbar or less H⁺ gives most of the
//!   electrons, and they are low (about 6× at 6,000 K and 10 mbar). Plan 14's "H⁺ is under 10⁻⁶ of
//!   H" holds at 4,600 K and 0.05 bar (2.0 × 10⁻⁷ there).
//! - **Gas only**: the network does not condense; P14.T52.c's cold traps remove each condensate's
//!   limiting element above its base. Nor does it form graphite, which a gas richer in carbon than
//!   oxygen would (Heng and Tsai 2016, §5.2: "we urge caution when applying these formulae to C/O >
//!   1 situations"); graphite is not modelled here.
//! - **Carbon-rich gases**: [`NOT_CARRIED`] is closed over protosolar abundances. At C/O 1.5 and
//!   10 mbar, C₂H holds 9% of the carbon at 2,500 K and 13% at 3,000 K, CN 7% of the nitrogen and
//!   HNC 3–5% (FastChem's C, H, O, N and He run), none of them rows (P14.T52.e).
//!
//! The network being every gas row with a record, a gas row added later moves this output: after
//! P14.T54.b first reads it, that is a `GENERATOR_VERSION` change, which the goldens and P14.T49.d's
//! fingerprint of the rows catch.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use crate::math;
use crate::substance::{
    BalanceReactionError, Element, Reaction, SubstanceId, SubstanceKey, by_key, formation_range,
    ids,
};
use crate::units::{Kelvin, Pascals};

/// Pascals per bar, the records' standard pressure.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// The major elements, each with its basis species' key, in the order of the unknowns.
pub const MAJOR_ELEMENTS: [(Element, &str); 5] = [
    (Element::H, "H2"),
    (Element::C, "CH4"),
    (Element::N, "NH3"),
    (Element::O, "H2O"),
    (Element::S, "H2S"),
];

/// The free electron's key, the charge's basis.
const ELECTRON: &str = "e-";

/// The number of unknowns: the five basis pressures, the electrons' and the hydrogen nuclei's.
const UNKNOWNS: usize = 7;

/// The place of the electrons' logarithm among the unknowns, and of the charge's equation.
const CHARGE: usize = 5;

/// The place of the hydrogen nuclei's logarithm among the unknowns, and of the total pressure's
/// equation.
const NUCLEI: usize = 6;

/// The Newton steps every solution takes, from the closed-form start, at most: it stops early only
/// where a step would leave the unknowns as they are, so that every later one would too.
pub const NEWTON_STEPS: usize = 60;

/// The largest change a Newton step makes in any unknown's natural logarithm: a factor of e⁸.
pub const STEP_LIMIT: f64 = 8.0;

/// The halvings a Newton step may take until it lowers the sum of the squared residuals; a step
/// that lowers it at none leaves the unknowns as they are.
pub const BACKTRACKS: usize = 12;

/// The largest residual of a returned solution, each a difference of natural logarithms and so a
/// relative miss: over each major element's balance, the charge's and the total pressure's.
pub const CONVERGED: f64 = 1e-13;

/// The largest share of a gas's hydrogen its all-in hydrides may take (CH₄, NH₃, H₂O, H₂S, PH₃
/// and HCl) for it to be solved as hydrogen-dominated: a half, so that H₂ holds at least half of
/// the hydrogen in the cold limit.
pub const HYDROGEN_DOMINANCE_BOUND: f64 = 0.5;

/// How far above the total pressure a Newton step may take a major species' pressure, in its
/// natural logarithm: none can exceed the total at the solution.
const HEADROOM: f64 = 2.0;

/// The largest natural logarithm the start's quadratics are taken at, so that their squares stay
/// finite (e³⁰⁰ ≈ 10¹³⁰); beyond it the pair is all in its stronger member anyway.
const START_LOG_BOUND: f64 = 300.0;

/// A gas species the network does not carry, since it is not a registry row: an entry of
/// [`NOT_CARRIED`].
///
/// The fields are plain data with no invariant between them, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotCarried {
    /// Its Hill formula: C first and then H, the other elements alphabetically after them, or every
    /// element alphabetically where it holds no carbon (`HO` for OH, `ClNa` for NaCl).
    pub formula: &'static str,
    /// Its isomer, as FastChem's species list names it, where the formula has more than one.
    pub isomer: Option<&'static str>,
    /// Its charge, in elementary charges.
    pub charge: i8,
    /// The element it holds the largest share of.
    pub element: Element,
    /// That share of the element's atoms, at its largest over the grid.
    pub largest_share: f64,
    /// The grid's temperature there.
    pub temperature: Kelvin,
    /// The grid's total pressure there.
    pub pressure: Pascals,
    /// Whether FastChem's equilibrium condensation holds at least half of the element condensed
    /// there: a share of the gas-only run that P14.T52.c's cold traps remove.
    pub condensed: bool,
}

/// The gas species a hydrogen-dominated gas holds that are not registry rows, and so are not
/// carried: a closed set, for the composition audit (P14.T33.c's census).
///
/// It is sorted by formula, charge and isomer. P14.T52.e adds rows from it, and each entry goes
/// as its row joins.
///
/// The set is closed by a rule, not by hand (decision-p14-t52a-rows): every gas species of
/// FastChem 4.0.3's full network (its `logK.dat`, gas phase only, the 25 elements at the
/// registry's protosolar abundances, Lodders 2010) that holds at least 1% of any element at any
/// point of a grid of 300, 500, 800, 1,000, 1,200, 1,500, 2,000, 2,500, 3,000, 4,000, 5,000 and
/// 6,000 K by 10⁻⁶, 10⁻⁴, 10⁻², 1 and 100 bar, and is not a registry row; its scope is
/// protosolar, and the carriers of carbon-rich gases (C₂H, CN, HNC) are the [module](self)'s
/// reach's. Each entry's
/// `condensed` reads the same grid run with FastChem's equilibrium condensation (its
/// `logK_condensates.dat`). FastChem was run by hand outside the repository, as for the asserted
/// cases (the tests' documentation). FeH, a registry row with no record, is
/// [`formation::NOT_MODELLED`](crate::substance::formation::NOT_MODELLED)'s, not this set's; it
/// holds at most 2.8% of the iron (5,000 K, 100 bar).
pub const NOT_CARRIED: [NotCarried; 65] = [
    NotCarried {
        formula: "Al2O",
        isomer: None,
        charge: 0,
        element: Element::Al,
        largest_share: 0.75,
        temperature: Kelvin::new(500.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "AlCl",
        isomer: None,
        charge: 0,
        element: Element::Cl,
        largest_share: 0.2,
        temperature: Kelvin::new(1200.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "AlH",
        isomer: None,
        charge: 0,
        element: Element::Al,
        largest_share: 0.43,
        temperature: Kelvin::new(2500.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "AlHO",
        isomer: Some("aloh"),
        charge: 0,
        element: Element::Al,
        largest_share: 0.63,
        temperature: Kelvin::new(1500.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "AlHO2",
        isomer: None,
        charge: 0,
        element: Element::Al,
        largest_share: 0.98,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e5),
        condensed: true,
    },
    NotCarried {
        formula: "Ar",
        isomer: None,
        charge: 1,
        element: Element::Ar,
        largest_share: 0.011,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "C",
        isomer: None,
        charge: 0,
        element: Element::C,
        largest_share: 0.98,
        temperature: Kelvin::new(4000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "C",
        isomer: None,
        charge: 1,
        element: Element::C,
        largest_share: 0.88,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "CH",
        isomer: None,
        charge: 0,
        element: Element::C,
        largest_share: 0.044,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "CHP",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 0.075,
        temperature: Kelvin::new(1200.0),
        pressure: Pascals::new(1e5),
        condensed: false,
    },
    NotCarried {
        formula: "CaCl",
        isomer: None,
        charge: 0,
        element: Element::Cl,
        largest_share: 0.14,
        temperature: Kelvin::new(1200.0),
        pressure: Pascals::new(1e3),
        condensed: false,
    },
    NotCarried {
        formula: "CaCl2",
        isomer: None,
        charge: 0,
        element: Element::Cl,
        largest_share: 0.77,
        temperature: Kelvin::new(1000.0),
        pressure: Pascals::new(1e1),
        condensed: false,
    },
    NotCarried {
        formula: "CaH",
        isomer: None,
        charge: 0,
        element: Element::Ca,
        largest_share: 0.11,
        temperature: Kelvin::new(4000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "CaH2O2",
        isomer: None,
        charge: 0,
        element: Element::Ca,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "CaHO",
        isomer: None,
        charge: 0,
        element: Element::Ca,
        largest_share: 0.18,
        temperature: Kelvin::new(2000.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "Cl",
        isomer: None,
        charge: 0,
        element: Element::Cl,
        largest_share: 1.0,
        temperature: Kelvin::new(3000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "Cl",
        isomer: None,
        charge: 1,
        element: Element::Cl,
        largest_share: 0.41,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "Cl2K2",
        isomer: None,
        charge: 0,
        element: Element::K,
        largest_share: 0.45,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e1),
        condensed: true,
    },
    NotCarried {
        formula: "Cl2Na2",
        isomer: None,
        charge: 0,
        element: Element::Cl,
        largest_share: 0.8,
        temperature: Kelvin::new(800.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "ClK",
        isomer: None,
        charge: 0,
        element: Element::K,
        largest_share: 0.44,
        temperature: Kelvin::new(800.0),
        pressure: Pascals::new(1e3),
        condensed: true,
    },
    NotCarried {
        formula: "ClNa",
        isomer: None,
        charge: 0,
        element: Element::Cl,
        largest_share: 0.76,
        temperature: Kelvin::new(1200.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "CrH",
        isomer: None,
        charge: 0,
        element: Element::Cr,
        largest_share: 0.058,
        temperature: Kelvin::new(4000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "CrS",
        isomer: None,
        charge: 0,
        element: Element::Cr,
        largest_share: 0.69,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "FeH2O2",
        isomer: None,
        charge: 0,
        element: Element::Fe,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "H",
        isomer: None,
        charge: 1,
        element: Element::H,
        largest_share: 0.062,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "H2K2O2",
        isomer: None,
        charge: 0,
        element: Element::K,
        largest_share: 0.68,
        temperature: Kelvin::new(500.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "H2MgO2",
        isomer: None,
        charge: 0,
        element: Element::Mg,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "H2N",
        isomer: None,
        charge: 0,
        element: Element::N,
        largest_share: 0.02,
        temperature: Kelvin::new(5000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "H2Na2O2",
        isomer: None,
        charge: 0,
        element: Element::Na,
        largest_share: 0.94,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "H2P",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 0.66,
        temperature: Kelvin::new(2500.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "H3Si",
        isomer: None,
        charge: 0,
        element: Element::Si,
        largest_share: 0.011,
        temperature: Kelvin::new(2500.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "H4Si",
        isomer: None,
        charge: 0,
        element: Element::Si,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "HK",
        isomer: None,
        charge: 0,
        element: Element::K,
        largest_share: 0.051,
        temperature: Kelvin::new(2500.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HKO",
        isomer: None,
        charge: 0,
        element: Element::K,
        largest_share: 0.42,
        temperature: Kelvin::new(800.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "HMg",
        isomer: None,
        charge: 0,
        element: Element::Mg,
        largest_share: 0.043,
        temperature: Kelvin::new(5000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HMgO",
        isomer: None,
        charge: 0,
        element: Element::Mg,
        largest_share: 0.031,
        temperature: Kelvin::new(1200.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "HMn",
        isomer: None,
        charge: 0,
        element: Element::Mn,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "HN",
        isomer: None,
        charge: 0,
        element: Element::N,
        largest_share: 0.12,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HNa",
        isomer: None,
        charge: 0,
        element: Element::Na,
        largest_share: 0.05,
        temperature: Kelvin::new(3000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HNaO",
        isomer: None,
        charge: 0,
        element: Element::Na,
        largest_share: 0.072,
        temperature: Kelvin::new(1000.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "HNi",
        isomer: None,
        charge: 0,
        element: Element::Ni,
        largest_share: 0.73,
        temperature: Kelvin::new(500.0),
        pressure: Pascals::new(1e7),
        condensed: true,
    },
    NotCarried {
        formula: "HO",
        isomer: None,
        charge: 0,
        element: Element::O,
        largest_share: 0.21,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HP",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 0.37,
        temperature: Kelvin::new(4000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HS",
        isomer: None,
        charge: 0,
        element: Element::S,
        largest_share: 0.48,
        temperature: Kelvin::new(4000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HSi",
        isomer: None,
        charge: 0,
        element: Element::Si,
        largest_share: 0.17,
        temperature: Kelvin::new(5000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HTi",
        isomer: None,
        charge: 0,
        element: Element::Ti,
        largest_share: 0.043,
        temperature: Kelvin::new(4000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "HZn",
        isomer: None,
        charge: 0,
        element: Element::Zn,
        largest_share: 0.017,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e7),
        condensed: false,
    },
    NotCarried {
        formula: "N",
        isomer: None,
        charge: 0,
        element: Element::N,
        largest_share: 1.0,
        temperature: Kelvin::new(4000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "N",
        isomer: None,
        charge: 1,
        element: Element::N,
        largest_share: 0.045,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "NP",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 0.013,
        temperature: Kelvin::new(2000.0),
        pressure: Pascals::new(1e5),
        condensed: false,
    },
    NotCarried {
        formula: "NiS",
        isomer: None,
        charge: 0,
        element: Element::Ni,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e1),
        condensed: true,
    },
    NotCarried {
        formula: "O",
        isomer: None,
        charge: 1,
        element: Element::O,
        largest_share: 0.054,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "O10P4",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "O2Ti",
        isomer: None,
        charge: 0,
        element: Element::Ti,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "O2V",
        isomer: None,
        charge: 0,
        element: Element::V,
        largest_share: 1.0,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "OP",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 0.78,
        temperature: Kelvin::new(1500.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "P",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 1.0,
        temperature: Kelvin::new(3000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "P",
        isomer: None,
        charge: 1,
        element: Element::P,
        largest_share: 0.99,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "P2",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 1.0,
        temperature: Kelvin::new(800.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "PS",
        isomer: None,
        charge: 0,
        element: Element::P,
        largest_share: 0.38,
        temperature: Kelvin::new(1200.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "S",
        isomer: None,
        charge: 0,
        element: Element::S,
        largest_share: 1.0,
        temperature: Kelvin::new(3000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "S",
        isomer: None,
        charge: 1,
        element: Element::S,
        largest_share: 0.97,
        temperature: Kelvin::new(6000.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "S2",
        isomer: None,
        charge: 0,
        element: Element::S,
        largest_share: 0.059,
        temperature: Kelvin::new(1200.0),
        pressure: Pascals::new(1e-1),
        condensed: false,
    },
    NotCarried {
        formula: "SSi",
        isomer: None,
        charge: 0,
        element: Element::S,
        largest_share: 0.82,
        temperature: Kelvin::new(300.0),
        pressure: Pascals::new(1e-1),
        condensed: true,
    },
    NotCarried {
        formula: "STi",
        isomer: None,
        charge: 0,
        element: Element::Ti,
        largest_share: 0.01,
        temperature: Kelvin::new(2000.0),
        pressure: Pascals::new(1e5),
        condensed: false,
    },
];

/// Which composition a [`GasEquilibrium`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Regime {
    /// The network's equilibrium at the temperature, solved.
    Equilibrium,
    /// Below the records' floor: the floor's equilibrium at the same pressure, held (the
    /// [module](self)'s documentation), the major carriers all but all-in at a bar or more.
    HeldAtFloor,
}

/// One species of the network.
#[derive(Debug, Clone, PartialEq)]
struct Species {
    id: SubstanceId,
    /// Its formation from the basis, and one over the multiple of the species it is written for
    /// (1, or ½ where the reaction is doubled to keep H₂'s coefficient whole); `None` for a basis
    /// species, whose K is 1.
    formation: Option<(Reaction, f64)>,
    /// Its ν over the unknowns' basis: H₂, CH₄, NH₃, H₂O, H₂S and the electron.
    nu: [f64; CHARGE + 1],
    /// Twice each ν, whole numbers, which say which unknowns its pressure reads.
    doubled: [i32; CHARGE + 1],
    /// Its atoms of each major element, in [`MAJOR_ELEMENTS`]' order.
    counts: [u16; 5],
    charge: i8,
    /// The trace element it holds, by its place in the network's traces.
    trace: Option<usize>,
}

/// One trace element of the network and its species.
#[derive(Debug, Clone, PartialEq)]
struct Trace {
    element: Element,
    /// Its species, by place in the network, in registry order.
    species: Vec<usize>,
    /// The hydrogen atoms of its basis species: its all-in hydride's.
    hydride_hydrogen: f64,
}

/// The places in the network of the species the start reads, where the registry has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StartSpecies {
    hydrogen_atom: Option<usize>,
    hydride: Option<usize>,
    carbon_monoxide: Option<usize>,
    oxygen_atom: Option<usize>,
    nitrogen: Option<usize>,
    acetylene: Option<usize>,
}

/// The equilibrium network: every gas row of the registry with a thermochemical record (the
/// [module](self)'s documentation).
///
/// Build it once with [`Network::registry`] and solve each level with [`Network::solve`].
#[derive(Debug, Clone, PartialEq)]
pub struct Network {
    species: Vec<Species>,
    traces: Vec<Trace>,
    /// The basis species' places, in [`MAJOR_ELEMENTS`]' order, then the electron's.
    basis: [usize; CHARGE + 1],
    start: StartSpecies,
    /// The lowest and highest temperatures at which every species' record holds.
    floor: Kelvin,
    ceiling: Kelvin,
}

/// [`Network::registry`] could not build the network from the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildNetworkError {
    /// A basis species is not a gas row with a record.
    MissingBasis {
        /// Its key.
        key: &'static str,
    },
    /// A species holds two atoms of trace elements, which the closed form cannot share.
    TwoTraceAtoms {
        /// The row.
        key: SubstanceKey,
    },
    /// A trace element has no neutral species to be its basis.
    NoNeutralSpecies {
        /// The element.
        element: Element,
    },
    /// A species' formation from the basis could not be built.
    Reaction {
        /// The row.
        key: SubstanceKey,
        /// Why.
        error: BalanceReactionError,
    },
    /// The species' records share no temperature.
    NoCommonRange,
}

impl fmt::Display for BuildNetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBasis { key } => write!(f, "the basis species {key} has no record"),
            Self::TwoTraceAtoms { key } => {
                write!(f, "{key} holds two atoms of trace elements")
            }
            Self::NoNeutralSpecies { element } => {
                write!(f, "{} has no neutral species", element.symbol())
            }
            Self::Reaction { key, error } => {
                write!(f, "the formation of {key} could not be built: {error}")
            }
            Self::NoCommonRange => f.write_str("the species' records share no temperature"),
        }
    }
}

impl Error for BuildNetworkError {}

/// [`Network::solve`] could not speciate the gas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SolveEquilibriumError {
    /// The temperature is not finite and positive.
    Temperature(Kelvin),
    /// The temperature is above the records' ceiling ([`Network::ceiling`]); nothing is
    /// extrapolated.
    AboveRecords(Kelvin),
    /// The pressure is not finite and positive.
    Pressure(Pascals),
    /// An element's abundance is not finite and non-negative.
    Abundance {
        /// The element.
        element: Element,
        /// Its abundance as given.
        value: f64,
    },
    /// An element is given twice.
    Repeated(Element),
    /// The gas holds no hydrogen.
    NoHydrogen,
    /// The gas is not hydrogen-dominated: its all-in hydrides would take more than
    /// [`HYDROGEN_DOMINANCE_BOUND`] of its hydrogen.
    NotHydrogenDominated {
        /// The share they would take.
        share: f64,
    },
    /// A species has no record at the temperature, which the network's range should exclude.
    NoRecord {
        /// The row.
        key: SubstanceKey,
    },
    /// The Newton steps did not reach [`CONVERGED`].
    NotConverged {
        /// The largest relative residual reached.
        residual: f64,
    },
}

impl fmt::Display for SolveEquilibriumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temperature(t) => {
                write!(f, "temperature {} K is not finite and positive", t.value())
            }
            Self::AboveRecords(t) => {
                write!(f, "temperature {} K is above the records", t.value())
            }
            Self::Pressure(p) => write!(f, "pressure {} Pa is not finite and positive", p.value()),
            Self::Abundance { element, value } => write!(
                f,
                "abundance {value} of {} is not finite and non-negative",
                element.symbol()
            ),
            Self::Repeated(element) => write!(f, "{} is given twice", element.symbol()),
            Self::NoHydrogen => f.write_str("the gas holds no hydrogen"),
            Self::NotHydrogenDominated { share } => write!(
                f,
                "the gas is not hydrogen-dominated: its hydrides would take {share} of its \
                 hydrogen"
            ),
            Self::NoRecord { key } => write!(f, "{key} has no record at the temperature"),
            Self::NotConverged { residual } => {
                write!(f, "the solution did not converge (residual {residual})")
            }
        }
    }
}

impl Error for SolveEquilibriumError {}

/// A gas's speciation at one temperature and pressure ([`Network::solve`]): the mole fraction of
/// each species of the network.
#[derive(Debug, Clone, PartialEq)]
pub struct GasEquilibrium {
    temperature: Kelvin,
    pressure: Pascals,
    regime: Regime,
    fractions: Vec<(SubstanceId, f64)>,
}

impl GasEquilibrium {
    /// The temperature solved at.
    #[must_use]
    pub const fn temperature(&self) -> Kelvin {
        self.temperature
    }

    /// The total pressure solved at.
    #[must_use]
    pub const fn pressure(&self) -> Pascals {
        self.pressure
    }

    /// Whether the composition is the equilibrium's at the temperature or the floor's, held.
    #[must_use]
    pub const fn regime(&self) -> Regime {
        self.regime
    }

    /// Each species of the network and its mole fraction, n ÷ Σ n, in registry order: zero for a
    /// species of an element the gas does not hold.
    pub fn mole_fractions(&self) -> impl ExactSizeIterator<Item = (SubstanceId, f64)> + '_ {
        self.fractions.iter().copied()
    }

    /// The mole fraction of `id`, or `None` if the network does not carry it.
    #[must_use]
    pub fn mole_fraction(&self, id: SubstanceId) -> Option<f64> {
        self.fractions
            .iter()
            .find(|&&(s, _)| s == id)
            .map(|&(_, x)| x)
    }

    /// The partial pressure of `id`, its mole fraction times the total, or `None` if the network
    /// does not carry it.
    #[must_use]
    pub fn partial_pressure(&self, id: SubstanceId) -> Option<Pascals> {
        self.mole_fraction(id).map(|x| self.pressure * x)
    }

    /// The element's atoms in the gas per hydrogen atom, from the mole fractions: the abundance
    /// solved for, to the solution's precision.
    #[must_use]
    pub fn per_hydrogen(&self, element: Element) -> f64 {
        let mut held = 0.0;
        let mut hydrogen = 0.0;
        for &(id, x) in &self.fractions {
            held += f64::from(atoms(id, element)) * x;
            hydrogen += f64::from(atoms(id, Element::H)) * x;
        }
        held / hydrogen
    }

    /// The charge's imbalance over its sum: (Σ positive − Σ negative) ÷ (Σ positive + Σ
    /// negative) of the species' charges times their mole fractions, or zero where nothing is
    /// charged.
    #[must_use]
    pub fn charge_imbalance(&self) -> f64 {
        let mut positive = 0.0;
        let mut negative = 0.0;
        for &(id, x) in &self.fractions {
            let q = f64::from(id.substance().charge());
            if q > 0.0 {
                positive += q * x;
            } else {
                negative -= q * x;
            }
        }
        let total = positive + negative;
        if total > 0.0 {
            (positive - negative) / total
        } else {
            0.0
        }
    }
}

/// The atoms of `element` in `id`.
#[must_use]
fn atoms(id: SubstanceId, element: Element) -> u16 {
    id.substance()
        .elements()
        .iter()
        .find(|&&(e, _)| e == element)
        .map_or(0, |&(_, n)| n)
}

/// Whether `element` is a major element.
#[must_use]
fn is_major(element: Element) -> bool {
    MAJOR_ELEMENTS.iter().any(|&(e, _)| e == element)
}

/// The trace elements of the network's `rows`, in [`Element::ALL`]'s order, each with its species
/// and the place of its basis: its neutral species of fewest atoms, the first in registry order on
/// a tie.
fn trace_elements(rows: &[SubstanceId]) -> Result<Vec<(Trace, usize)>, BuildNetworkError> {
    let size = |id: SubstanceId| -> u32 {
        id.substance()
            .elements()
            .iter()
            .map(|&(_, n)| u32::from(n))
            .sum()
    };
    let mut traces = Vec::new();
    for element in Element::ALL {
        if is_major(element) {
            continue;
        }
        let species: Vec<usize> = (0..rows.len())
            .filter(|&i| atoms(rows[i], element) > 0)
            .collect();
        if species.is_empty() {
            continue;
        }
        let base = species
            .iter()
            .copied()
            .filter(|&i| rows[i].substance().charge() == 0)
            .min_by_key(|&i| (size(rows[i]), i))
            .ok_or(BuildNetworkError::NoNeutralSpecies { element })?;
        let hydride_hydrogen = f64::from(atoms(rows[base], Element::H));
        traces.push((
            Trace {
                element,
                species,
                hydride_hydrogen,
            },
            base,
        ));
    }
    Ok(traces)
}

/// Species `i` of the network's `rows`: its formation from the basis (the major elements' `basis`
/// and, for a trace species, its element's in `traces`), its ν over the unknowns and its atoms of
/// the major elements.
fn species_of(
    i: usize,
    rows: &[SubstanceId],
    basis: &[usize; CHARGE + 1],
    traces: &[(Trace, usize)],
) -> Result<Species, BuildNetworkError> {
    let id = rows[i];
    let row = id.substance();
    let mut trace = None;
    for (t, (tr, _)) in traces.iter().enumerate() {
        let n = atoms(id, tr.element);
        if n == 0 {
            continue;
        }
        if n > 1 || trace.is_some() {
            return Err(BuildNetworkError::TwoTraceAtoms { key: row.key() });
        }
        trace = Some(t);
    }
    let counts = MAJOR_ELEMENTS.map(|(e, _)| i32::from(atoms(id, e)));
    // The trace basis' own atoms of the major elements, taken out of ν.
    let base = trace.map(|t| traces[t].1);
    let base_counts = base.map_or([0; 5], |b| {
        MAJOR_ELEMENTS.map(|(e, _)| i32::from(atoms(rows[b], e)))
    });
    let own: [i32; 5] = core::array::from_fn(|k| counts[k] - base_counts[k]);
    // ν over CH₄, NH₃, H₂O and H₂S is the species' own atoms of C, N, O and S; over H₂, the hydrogen
    // left, halved; over the electron, −q.
    let twice_hydrogen = own[0] - 4 * own[1] - 3 * own[2] - 2 * own[3] - 2 * own[4];
    let charge = i32::from(row.charge());
    let twice = [
        twice_hydrogen,
        2 * own[1],
        2 * own[2],
        2 * own[3],
        2 * own[4],
        -2 * charge,
    ];
    let (formation, doubled) = if let Some(k) = basis.iter().position(|&b| b == i) {
        // A major basis species is its own unknown.
        let mut unit = [0; CHARGE + 1];
        unit[k] = 2;
        (None, unit)
    } else if base == Some(i) {
        (None, [0; CHARGE + 1])
    } else {
        // Written for two of the species where H₂'s ν is a half, so that every ν is whole.
        let multiple: i32 = if twice_hydrogen % 2 == 0 { 1 } else { 2 };
        let mut terms = vec![(id, multiple)];
        for (&slot, &t) in basis.iter().zip(&twice) {
            if t != 0 {
                terms.push((rows[slot], -(multiple * t) / 2));
            }
        }
        if let Some(b) = base {
            terms.push((rows[b], -multiple));
        }
        let reaction = Reaction::new(&terms).map_err(|error| BuildNetworkError::Reaction {
            key: row.key(),
            error,
        })?;
        (Some((reaction, 1.0 / f64::from(multiple))), twice)
    };
    Ok(Species {
        id,
        formation,
        nu: doubled.map(|t| 0.5 * f64::from(t)),
        doubled,
        counts: MAJOR_ELEMENTS.map(|(e, _)| atoms(id, e)),
        charge: row.charge(),
        trace,
    })
}

/// The temperatures over which every record of `rows` holds.
fn common_range(rows: &[SubstanceId]) -> Result<(Kelvin, Kelvin), BuildNetworkError> {
    let mut floor = f64::NEG_INFINITY;
    let mut ceiling = f64::INFINITY;
    for &id in rows {
        let Some((lower, upper)) = formation_range(id) else {
            return Err(BuildNetworkError::NoCommonRange);
        };
        floor = floor.max(lower.value());
        ceiling = ceiling.min(upper.value());
    }
    if floor > ceiling {
        return Err(BuildNetworkError::NoCommonRange);
    }
    Ok((Kelvin::new(floor), Kelvin::new(ceiling)))
}

impl Network {
    /// The network of the registry's gas rows with a thermochemical record.
    ///
    /// # Errors
    ///
    /// [`BuildNetworkError`] if a basis species has no record, a species holds two trace atoms, a
    /// trace element has no neutral species, a formation does not balance, or the records share
    /// no temperature. None occurs with the registry as built; its tests hold it so.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::equilibrium::Network;
    ///
    /// let network = Network::registry()?;
    /// assert!(network.species().len() > 50);
    /// assert!((network.floor().value() - 300.0).abs() < 1e-9);
    /// assert!((network.ceiling().value() - 6_000.0).abs() < 1e-9);
    /// # Ok::<(), hyperion_sim::planetary::chem::equilibrium::BuildNetworkError>(())
    /// ```
    pub fn registry() -> Result<Self, BuildNetworkError> {
        let rows: Vec<SubstanceId> = ids().filter(|&id| formation_range(id).is_some()).collect();
        let place = |key: &'static str| -> Result<usize, BuildNetworkError> {
            by_key(key)
                .and_then(|id| rows.iter().position(|&r| r == id))
                .ok_or(BuildNetworkError::MissingBasis { key })
        };
        let mut basis = [0; CHARGE + 1];
        for (slot, &(_, key)) in basis.iter_mut().zip(MAJOR_ELEMENTS.iter()) {
            *slot = place(key)?;
        }
        basis[CHARGE] = place(ELECTRON)?;
        let traces = trace_elements(&rows)?;
        let species = (0..rows.len())
            .map(|i| species_of(i, &rows, &basis, &traces))
            .collect::<Result<Vec<Species>, BuildNetworkError>>()?;
        let (floor, ceiling) = common_range(&rows)?;
        let find = |key: &str| by_key(key).and_then(|id| rows.iter().position(|&r| r == id));
        let start = StartSpecies {
            hydrogen_atom: find("H"),
            hydride: find("H-"),
            carbon_monoxide: find("CO"),
            oxygen_atom: find("O"),
            nitrogen: find("N2"),
            acetylene: find("C2H2"),
        };
        Ok(Self {
            species,
            traces: traces.into_iter().map(|(t, _)| t).collect(),
            basis,
            start,
            floor,
            ceiling,
        })
    }

    /// The network's species, in registry order.
    pub fn species(&self) -> impl ExactSizeIterator<Item = SubstanceId> + '_ {
        self.species.iter().map(|s| s.id)
    }

    /// The lowest temperature at which every species' record holds: below it the floor's
    /// composition is held ([`Regime::HeldAtFloor`]).
    #[must_use]
    pub const fn floor(&self) -> Kelvin {
        self.floor
    }

    /// The highest temperature at which every species' record holds: above it nothing is solved.
    #[must_use]
    pub const fn ceiling(&self) -> Kelvin {
        self.ceiling
    }

    /// The gas of element abundances `abundances`, each element by number relative to hydrogen
    /// (normalised by hydrogen's own value; an element not given is absent), at `temperature`
    /// and total pressure `pressure`, speciated over the network (the [module](self)'s model).
    ///
    /// # Errors
    ///
    /// [`SolveEquilibriumError`] for a temperature that is not finite and positive or is above
    /// [`ceiling`](Self::ceiling), a pressure that is not finite and positive, an abundance that
    /// is not finite and non-negative (also after its division by hydrogen's) or is given twice,
    /// no hydrogen, a gas that is not hydrogen-dominated, a species with no record at the
    /// temperature (which the network's range excludes), or a solution that did not converge
    /// (which the tests hold never happens over the domain).
    ///
    /// # Examples
    ///
    /// Jupiter-like gas at 1,500 K and 1 bar keeps its carbon as CO, and at 800 K as CH₄:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::equilibrium::Network;
    /// use hyperion_sim::substance::{Element, by_key};
    /// use hyperion_sim::units::{Kelvin, Pascals};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let network = Network::registry()?;
    /// let protosolar = Element::ALL.map(|e| (e, e.protosolar_abundance_per_hydrogen()));
    /// let [co, ch4] = ["CO", "CH4"].map(|k| by_key(k).ok_or("a row"));
    /// let (co, ch4) = (co?, ch4?);
    /// let hot = network.solve(protosolar, Kelvin::new(1_500.0), Pascals::new(1e5))?;
    /// assert!(hot.mole_fraction(co) > hot.mole_fraction(ch4));
    /// let cool = network.solve(protosolar, Kelvin::new(800.0), Pascals::new(1e5))?;
    /// assert!(cool.mole_fraction(ch4) > cool.mole_fraction(co));
    /// # Ok(())
    /// # }
    /// ```
    pub fn solve(
        &self,
        abundances: impl IntoIterator<Item = (Element, f64)>,
        temperature: Kelvin,
        pressure: Pascals,
    ) -> Result<GasEquilibrium, SolveEquilibriumError> {
        let t = temperature.value();
        if !(t.is_finite() && t > 0.0) {
            return Err(SolveEquilibriumError::Temperature(temperature));
        }
        if t > self.ceiling.value() {
            return Err(SolveEquilibriumError::AboveRecords(temperature));
        }
        let p = pressure.value();
        if !(p.is_finite() && p > 0.0) {
            return Err(SolveEquilibriumError::Pressure(pressure));
        }
        let mut per_hydrogen = [0.0; Element::ALL.len()];
        let mut given = [false; Element::ALL.len()];
        for (element, value) in abundances {
            let slot = element_index(element);
            if given[slot] {
                return Err(SolveEquilibriumError::Repeated(element));
            }
            if !(value.is_finite() && value >= 0.0) {
                return Err(SolveEquilibriumError::Abundance { element, value });
            }
            given[slot] = true;
            per_hydrogen[slot] = value;
        }
        let hydrogen = per_hydrogen[element_index(Element::H)];
        if hydrogen <= 0.0 {
            return Err(SolveEquilibriumError::NoHydrogen);
        }
        let mut relative = [0.0; Element::ALL.len()];
        for ((slot, &value), element) in relative.iter_mut().zip(&per_hydrogen).zip(Element::ALL) {
            *slot = value / hydrogen;
            // A tiny hydrogen can take another element's share past every finite value.
            if !slot.is_finite() {
                return Err(SolveEquilibriumError::Abundance { element, value });
            }
        }
        let gas = Gas::new(self, &relative);
        if gas.hydride_share > HYDROGEN_DOMINANCE_BOUND {
            return Err(SolveEquilibriumError::NotHydrogenDominated {
                share: gas.hydride_share,
            });
        }
        let bar = p / PASCALS_PER_BAR;
        let (regime, pressures) = if t < self.floor.value() {
            (
                Regime::HeldAtFloor,
                self.equilibrium(&gas, self.floor, bar)?,
            )
        } else {
            (
                Regime::Equilibrium,
                self.equilibrium(&gas, temperature, bar)?,
            )
        };
        let total: f64 = pressures.iter().sum();
        if !(total.is_finite() && total > 0.0) || pressures.iter().any(|q| !q.is_finite()) {
            return Err(SolveEquilibriumError::NotConverged {
                residual: f64::INFINITY,
            });
        }
        Ok(GasEquilibrium {
            temperature,
            pressure,
            regime,
            fractions: self
                .species
                .iter()
                .zip(&pressures)
                .map(|(s, &q)| (s.id, q / total))
                .collect(),
        })
    }

    /// The basis species' place of `trace`: its species whose ν is all zero.
    #[cfg(test)]
    #[must_use]
    fn trace_basis(&self, trace: &Trace) -> usize {
        trace
            .species
            .iter()
            .copied()
            .find(|&i| self.species[i].formation.is_none())
            .expect("a trace element has its basis among its species")
    }

    /// The equilibrium's partial pressures, bar, at `temperature` and `bar` bar in total.
    fn equilibrium(
        &self,
        gas: &Gas,
        temperature: Kelvin,
        bar: f64,
    ) -> Result<Vec<f64>, SolveEquilibriumError> {
        let mut log_k = Vec::with_capacity(self.species.len());
        for s in &self.species {
            log_k.push(match &s.formation {
                None => 0.0,
                Some((reaction, per)) => {
                    let log10 = reaction.log10_equilibrium_constant(temperature).ok_or(
                        SolveEquilibriumError::NoRecord {
                            key: s.id.substance().key(),
                        },
                    )?;
                    log10 * core::f64::consts::LN_10 * per
                }
            });
        }
        let state = State::new(self, gas, &log_k, bar);
        let (y, residual) = state.refine(state.start(), Stop::AtFixedPoint);
        // A residual that is not finite is the worst of all; `f64::max` alone would pass it over.
        let worst = residual.iter().fold(0.0_f64, |m, r| {
            if r.is_finite() {
                m.max(r.abs())
            } else {
                f64::INFINITY
            }
        });
        let pressures = state.pressures(&y);
        if worst > CONVERGED || pressures.iter().any(|p| !p.is_finite()) {
            return Err(SolveEquilibriumError::NotConverged { residual: worst });
        }
        Ok(pressures)
    }
}

/// The place of `element` in [`Element::ALL`].
#[must_use]
fn element_index(element: Element) -> usize {
    Element::ALL
        .iter()
        .position(|&e| e == element)
        .expect("every element is in Element::ALL")
}

/// A gas's abundances as the network reads them.
#[derive(Debug, Clone, PartialEq)]
struct Gas {
    /// Each major element per hydrogen, in [`MAJOR_ELEMENTS`]' order (hydrogen's 1).
    majors: [f64; 5],
    /// Each trace element per hydrogen, in the network's order.
    traces: Vec<f64>,
    /// The share of the hydrogen the all-in hydrides take.
    hydride_share: f64,
}

impl Gas {
    #[must_use]
    fn new(network: &Network, per_hydrogen: &[f64; Element::ALL.len()]) -> Self {
        let majors = MAJOR_ELEMENTS.map(|(e, _)| per_hydrogen[element_index(e)]);
        let traces: Vec<f64> = network
            .traces
            .iter()
            .map(|t| per_hydrogen[element_index(t.element)])
            .collect();
        let mut hydride_share = 0.0;
        for (&slot, &n) in network.basis[1..5].iter().zip(&majors[1..]) {
            hydride_share += f64::from(network.species[slot].counts[0]) * n;
        }
        for (t, trace) in network.traces.iter().enumerate() {
            hydride_share += trace.hydride_hydrogen * traces[t];
        }
        Self {
            majors,
            traces,
            hydride_share,
        }
    }
}

/// When [`State::refine`]'s Newton steps end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// At the first step that no halving lets lower the residuals: the solution's rule.
    AtFixedPoint,
    /// After every one of [`NEWTON_STEPS`]: the fixed count, which the tests compare it with.
    #[cfg(test)]
    FullCount,
}

/// One solution's fixed data: the network, the gas, each species' ln K and which unknowns and
/// species are active.
#[derive(Debug)]
struct State<'a> {
    network: &'a Network,
    gas: &'a Gas,
    log_k: &'a [f64],
    /// The total pressure, bar, and its natural logarithm.
    bar: f64,
    log_bar: f64,
    /// The natural logarithm of each major element's abundance per hydrogen (−∞ for one the gas
    /// does not hold, whose balance is not solved).
    log_majors: [f64; 5],
    /// Whether each unknown is solved for: a major element the gas holds, the charge where a
    /// cation can form, and always the hydrogen nuclei.
    active: [bool; UNKNOWNS],
    /// Whether each species is present: all its elements held, and the charge active if it is
    /// charged.
    present: Vec<bool>,
}

/// Each trace element's sums at one point: the largest exponent of its species, the sum of
/// their weights relative to it, and each unknown's mean ν over them.
#[derive(Debug, Clone, Copy)]
struct TraceSums {
    largest: f64,
    weight: f64,
    mean_nu: [f64; CHARGE + 1],
}

impl<'a> State<'a> {
    #[must_use]
    fn new(network: &'a Network, gas: &'a Gas, log_k: &'a [f64], bar: f64) -> Self {
        let mut active = [false; UNKNOWNS];
        for (a, &n) in active.iter_mut().zip(&gas.majors) {
            *a = n > 0.0;
        }
        active[NUCLEI] = true;
        let held = active;
        let holds = |s: &Species| {
            s.counts
                .iter()
                .zip(&held)
                .all(|(&count, &is_held)| count == 0 || is_held)
                && s.trace.is_none_or(|t| gas.traces[t] > 0.0)
        };
        let charged = network.species.iter().any(|s| s.charge > 0 && holds(s));
        active[CHARGE] = charged;
        let present = network
            .species
            .iter()
            .map(|s| holds(s) && (s.charge == 0 || charged))
            .collect();
        Self {
            network,
            gas,
            log_k,
            bar,
            log_bar: math::ln(bar),
            log_majors: gas.majors.map(math::ln),
            active,
            present,
        }
    }

    /// The exponent of species `i` at `y` over the basis: ln K + Σ ν y (for a trace species,
    /// relative to its element's basis).
    #[must_use]
    fn exponent(&self, i: usize, y: &[f64; UNKNOWNS]) -> f64 {
        let s = &self.network.species[i];
        let mut e = self.log_k[i];
        for ((&nu, &doubled), &yk) in s.nu.iter().zip(&s.doubled).zip(y) {
            if doubled != 0 {
                e += nu * yk;
            }
        }
        e
    }

    /// Each trace element's sums at `y`.
    #[must_use]
    fn trace_sums(&self, y: &[f64; UNKNOWNS]) -> Vec<TraceSums> {
        self.network
            .traces
            .iter()
            .map(|trace| {
                let mut largest = f64::NEG_INFINITY;
                for &i in &trace.species {
                    if self.present[i] {
                        largest = largest.max(self.exponent(i, y));
                    }
                }
                let mut weight = 0.0;
                let mut mean_nu = [0.0; CHARGE + 1];
                for &i in &trace.species {
                    if self.present[i] {
                        let w = math::exp(self.exponent(i, y) - largest);
                        weight += w;
                        for (m, &nu) in mean_nu.iter_mut().zip(&self.network.species[i].nu) {
                            *m += w * nu;
                        }
                    }
                }
                if weight > 0.0 {
                    for m in &mut mean_nu {
                        *m /= weight;
                    }
                }
                TraceSums {
                    largest,
                    weight,
                    mean_nu,
                }
            })
            .collect()
    }

    /// Species `i`'s natural log pressure at `y`, given the trace sums, or `None` if it is not
    /// present.
    #[must_use]
    fn log_pressure(&self, i: usize, y: &[f64; UNKNOWNS], sums: &[TraceSums]) -> Option<f64> {
        if !self.present[i] {
            return None;
        }
        let s = &self.network.species[i];
        Some(match s.trace {
            None => self.exponent(i, y),
            Some(t) => {
                let sum = &sums[t];
                math::ln(self.gas.traces[t]) + y[NUCLEI] + self.exponent(i, y)
                    - sum.largest
                    - math::ln(sum.weight)
            }
        })
    }

    /// The species' partial pressures at `y`, bar, in the network's order.
    #[must_use]
    fn pressures(&self, y: &[f64; UNKNOWNS]) -> Vec<f64> {
        let sums = self.trace_sums(y);
        (0..self.network.species.len())
            .map(|i| self.log_pressure(i, y, &sums).map_or(0.0, math::exp))
            .collect()
    }

    /// The residuals at `y` (each major element's balance, the charge's, the total pressure's; zero
    /// for an inactive one) and their Jacobian in the unknowns, an inactive unknown's row the unit
    /// row.
    #[must_use]
    fn evaluate(&self, y: &[f64; UNKNOWNS]) -> ([f64; UNKNOWNS], [[f64; UNKNOWNS]; UNKNOWNS]) {
        let sums = self.trace_sums(y);
        let mut held = [0.0; 5];
        let mut d_held = [[0.0; UNKNOWNS]; 5];
        let mut total = 0.0;
        let mut d_total = [0.0; UNKNOWNS];
        let (mut positive, mut negative) = (0.0, 0.0);
        let mut d_positive = [0.0; UNKNOWNS];
        let mut d_negative = [0.0; UNKNOWNS];
        for (i, s) in self.network.species.iter().enumerate() {
            let Some(log_p) = self.log_pressure(i, y, &sums) else {
                continue;
            };
            let p = math::exp(log_p);
            // ∂ ln p ÷ ∂ y: ν for a major species; ν less its element's mean, and 1 in ln N, for a
            // trace species.
            let mut g = [0.0; UNKNOWNS];
            match s.trace {
                None => g[..=CHARGE].copy_from_slice(&s.nu),
                Some(t) => {
                    for ((gk, &nu), &mean) in g.iter_mut().zip(&s.nu).zip(&sums[t].mean_nu) {
                        *gk = nu - mean;
                    }
                    g[NUCLEI] = 1.0;
                }
            }
            for (k, &count) in s.counts.iter().enumerate() {
                if count > 0 {
                    let n = f64::from(count);
                    held[k] += n * p;
                    for (d, &gk) in d_held[k].iter_mut().zip(&g) {
                        *d += n * p * gk;
                    }
                }
            }
            total += p;
            for (d, &gk) in d_total.iter_mut().zip(&g) {
                *d += p * gk;
            }
            let q = f64::from(s.charge);
            match s.charge.cmp(&0) {
                Ordering::Greater => {
                    positive += q * p;
                    for (d, &gk) in d_positive.iter_mut().zip(&g) {
                        *d += q * p * gk;
                    }
                }
                Ordering::Less => {
                    negative -= q * p;
                    for (d, &gk) in d_negative.iter_mut().zip(&g) {
                        *d -= q * p * gk;
                    }
                }
                Ordering::Equal => {}
            }
        }
        // Each residual is a difference of logarithms, held against wanted, so that a balance
        // carried by one species is linear in the unknowns and Newton's step lands on it.
        let mut residual = [0.0; UNKNOWNS];
        let mut jacobian = [[0.0; UNKNOWNS]; UNKNOWNS];
        for k in 0..5 {
            if !self.active[k] {
                jacobian[k][k] = 1.0;
                continue;
            }
            residual[k] = math::ln(held[k]) - (self.log_majors[k] + y[NUCLEI]);
            for (entry, &d) in jacobian[k].iter_mut().zip(&d_held[k]) {
                *entry = d / held[k];
            }
            jacobian[k][NUCLEI] -= 1.0;
        }
        if self.active[CHARGE] {
            residual[CHARGE] = math::ln(positive) - math::ln(negative);
            for ((entry, &dp), &dn) in jacobian[CHARGE]
                .iter_mut()
                .zip(&d_positive)
                .zip(&d_negative)
            {
                *entry = dp / positive - dn / negative;
            }
        } else {
            jacobian[CHARGE][CHARGE] = 1.0;
        }
        residual[NUCLEI] = math::ln(total) - self.log_bar;
        for (entry, &d) in jacobian[NUCLEI].iter_mut().zip(&d_total) {
            *entry = d / total;
        }
        // An inactive unknown's column is left out of every active row, so that its unit row fixes
        // its step at zero.
        for (j, &active) in self.active.iter().enumerate() {
            if !active {
                for (k, row) in jacobian.iter_mut().enumerate() {
                    if k != j {
                        row[j] = 0.0;
                    }
                }
            }
        }
        (residual, jacobian)
    }

    /// The largest scale of `step` from `y` at which no unknown moves more than [`STEP_LIMIT`] and
    /// no major species' pressure rises above the total by more than [`HEADROOM`] (in the natural
    /// logarithm), at most 1; zero for a step that is not finite.
    #[must_use]
    fn scale(&self, y: &[f64; UNKNOWNS], step: &[f64; UNKNOWNS]) -> f64 {
        let largest = step.iter().fold(0.0_f64, |m, s| {
            if s.is_finite() {
                m.max(s.abs())
            } else {
                f64::INFINITY
            }
        });
        if !largest.is_finite() {
            return 0.0;
        }
        let mut scale = if largest > STEP_LIMIT {
            STEP_LIMIT / largest
        } else {
            1.0
        };
        let ceiling = self.log_bar + HEADROOM;
        for (i, s) in self.network.species.iter().enumerate() {
            if !self.present[i] || s.trace.is_some() {
                continue;
            }
            let now = self.exponent(i, y);
            let rise: f64 = (0..=CHARGE).map(|k| s.nu[k] * step[k]).sum();
            if rise > 0.0 && now < ceiling && now + scale * rise > ceiling {
                scale = (ceiling - now) / rise;
            }
        }
        let now = y[NUCLEI];
        let nuclei_ceiling = self.log_bar + core::f64::consts::LN_2 + HEADROOM;
        if step[NUCLEI] > 0.0 && now < nuclei_ceiling && now + scale * step[NUCLEI] > nuclei_ceiling
        {
            scale = (nuclei_ceiling - now) / step[NUCLEI];
        }
        scale
    }

    /// The unknowns and their residuals after Newton's steps from `y` (the [module](self)'s
    /// documentation): at most [`NEWTON_STEPS`], each halved up to [`BACKTRACKS`] times until it
    /// lowers the squared residuals, a step that lowers them at none leaving the unknowns as they
    /// are. Under `Stop::AtFixedPoint` the steps end there; under the tests' `Stop::FullCount` they
    /// run on, and give the same bits, since a step reads only the unknowns, their residuals and
    /// their Jacobian, which a refused step leaves as they were.
    #[must_use]
    fn refine(&self, mut y: [f64; UNKNOWNS], rule: Stop) -> ([f64; UNKNOWNS], [f64; UNKNOWNS]) {
        let (mut residual, mut jacobian) = self.evaluate(&y);
        for _ in 0..NEWTON_STEPS {
            let step = solve_linear(jacobian, residual.map(|r| -r));
            let mut scale = self.scale(&y, &step);
            let before = merit(&residual);
            let mut accepted = false;
            for _ in 0..BACKTRACKS {
                let trial: [f64; UNKNOWNS] = core::array::from_fn(|k| y[k] + scale * step[k]);
                let (r, j) = self.evaluate(&trial);
                if merit(&r) < before {
                    (y, residual, jacobian) = (trial, r, j);
                    accepted = true;
                    break;
                }
                scale *= 0.5;
            }
            if !accepted && rule == Stop::AtFixedPoint {
                break;
            }
        }
        (y, residual)
    }

    /// The natural log of the K of the species at `slot`, where the registry has it.
    #[must_use]
    fn log_k_of(&self, slot: Option<usize>) -> Option<f64> {
        slot.map(|i| self.log_k[i])
    }

    /// The closed-form start (the [module](self)'s documentation).
    #[must_use]
    fn start(&self) -> [f64; UNKNOWNS] {
        let (log_hydrogen, nuclei) = self.start_hydrogen();
        let mut y = [0.0; UNKNOWNS];
        y[0] = log_hydrogen;
        y[NUCLEI] = math::ln(nuclei);
        let (methane, water) = self.start_carbon_and_oxygen(log_hydrogen, nuclei);
        if self.active[1] {
            y[1] = math::ln(methane);
        }
        if self.active[3] {
            y[3] = math::ln(water);
        }
        if self.active[2] {
            // NH₃ against N₂: N = a + 2 κ a², κ = K ÷ p(H₂)³.
            let kappa = self
                .log_k_of(self.network.start.nitrogen)
                .map_or(0.0, |lk| bounded_exp(lk - 3.0 * log_hydrogen));
            y[2] = math::ln(pair_root(self.gas.majors[2] * nuclei, kappa));
        }
        if self.active[4] {
            y[4] = math::ln(self.gas.majors[4] * nuclei);
        }
        if self.active[CHARGE] {
            y[CHARGE] = self.start_electrons(&y, nuclei);
        }
        y
    }

    /// H₂ against H over the helium and the trace elements, each trace atom one particle:
    /// P = s² (1 + 2ε) + K s (1 + ε) and N = 2 s² + K s, with s = √p(H₂) and K the H atom's.
    /// The natural log of p(H₂) and N, bar.
    #[must_use]
    fn start_hydrogen(&self) -> (f64, f64) {
        let inert: f64 = self.gas.traces.iter().sum();
        let atom = self
            .log_k_of(self.network.start.hydrogen_atom)
            .map_or(0.0, math::exp);
        let square = 1.0 + 2.0 * inert;
        let linear = atom * (1.0 + inert);
        let root = 2.0 * self.bar / (linear + (linear * linear + 4.0 * square * self.bar).sqrt());
        (2.0 * math::ln(root), 2.0 * root * root + atom * root)
    }

    /// CO against CH₄ and H₂O at p(H₂) = e^`log_hydrogen` and hydrogen nuclei `nuclei`, bar:
    /// p(CO) = κ p(CH₄) p(H₂O)', with p(H₂O)' the oxygen in water and atoms (p(O) = λ p(H₂O),
    /// λ = K ÷ p(H₂)) and κ = K ÷ (p(H₂)³ (1 + λ)), whatever the C/O; then the carbon not in CO
    /// shared between CH₄ and C₂H₂, e = m + 2 κ m² with κ = K ÷ p(H₂)³, which holds a carbon-rich
    /// gas's excess where the hydrogen is dissociated. p(CH₄) and p(H₂O), bar.
    #[must_use]
    fn start_carbon_and_oxygen(&self, log_hydrogen: f64, nuclei: f64) -> (f64, f64) {
        let start = &self.network.start;
        let carbon = self.gas.majors[1] * nuclei;
        let oxygen = self.gas.majors[3] * nuclei;
        let lambda = self.log_k_of(start.oxygen_atom).map_or(0.0, |lk| {
            math::exp((lk - log_hydrogen).min(START_LOG_BOUND))
        });
        let (methane, water) = if self.active[1] && self.active[3] {
            let kappa = self.log_k_of(start.carbon_monoxide).map_or(0.0, |lk| {
                bounded_exp(lk - 3.0 * log_hydrogen - math::ln_1p(lambda))
            });
            let sum = carbon + oxygen;
            let difference = carbon - oxygen;
            // The product p(CH₄) p(H₂O)' = p(CO) ÷ κ, the quadratic's stable root.
            let product = 2.0 * carbon * oxygen
                / (1.0
                    + kappa * sum
                    + (1.0 + 2.0 * kappa * sum + kappa * kappa * difference * difference).sqrt());
            let larger = |excess: f64| excess.midpoint((excess * excess + 4.0 * product).sqrt());
            let (left_carbon, left_oxygen) = if difference >= 0.0 {
                let more = larger(difference);
                (more, product / more)
            } else {
                let more = larger(-difference);
                (product / more, more)
            };
            (left_carbon, left_oxygen / (1.0 + lambda))
        } else {
            (carbon, oxygen / (1.0 + lambda))
        };
        let kappa = self
            .log_k_of(start.acetylene)
            .map_or(0.0, |lk| bounded_exp(lk - 3.0 * log_hydrogen));
        (pair_root(methane, kappa), water)
    }

    /// The electrons by Saha's closed form, each ion a minor share of its element: p(e⁻)² (1 + K'
    /// √p(H₂)) = Σ ε N R, R each trace element's cations over its neutral species at p(e⁻) = 1
    /// bar, and at most every trace atom ionised. The natural log of p(e⁻), bar, at the start's
    /// other unknowns `y` and hydrogen nuclei `nuclei`.
    #[must_use]
    fn start_electrons(&self, y: &[f64; UNKNOWNS], nuclei: f64) -> f64 {
        let network = self.network;
        let mut probe = *y;
        probe[CHARGE] = 0.0;
        let mut cations = 0.0;
        for (trace, &abundance) in network.traces.iter().zip(&self.gas.traces) {
            let largest = trace
                .species
                .iter()
                .filter(|&&i| self.present[i])
                .fold(f64::NEG_INFINITY, |most, &i| {
                    most.max(self.exponent(i, &probe))
                });
            let (mut ions, mut neutral) = (0.0, 0.0);
            for &i in trace.species.iter().filter(|&&i| self.present[i]) {
                let weight = math::exp(self.exponent(i, &probe) - largest);
                if network.species[i].charge > 0 {
                    ions += weight;
                } else {
                    neutral += weight;
                }
            }
            if neutral > 0.0 {
                cations += abundance * nuclei * ions / neutral;
            }
        }
        let hydride = self
            .log_k_of(network.start.hydride)
            .map_or(0.0, |lk| math::exp(lk + 0.5 * y[0]));
        let all_ionised = self.gas.traces.iter().sum::<f64>() * nuclei;
        let electrons = (cations / (1.0 + hydride))
            .sqrt()
            .min(all_ionised)
            .max(f64::MIN_POSITIVE);
        math::ln(electrons)
    }
}

/// e^`x`, with `x` held within ±[`START_LOG_BOUND`] so that the start's quadratics stay finite.
#[must_use]
fn bounded_exp(x: f64) -> f64 {
    math::exp(x.clamp(-START_LOG_BOUND, START_LOG_BOUND))
}

/// The smaller member's pressure of a pair sharing `total` of an element, the larger holding two
/// of its atoms at κ times the square: `total` = a + 2 κ a², by the stable root.
#[must_use]
fn pair_root(total: f64, kappa: f64) -> f64 {
    2.0 * total / (1.0 + (1.0 + 8.0 * kappa * total).sqrt())
}

/// The sum of the squared residuals, the measure a Newton step must lower; infinite where one is
/// not finite.
#[must_use]
fn merit(residual: &[f64; UNKNOWNS]) -> f64 {
    let mut sum = 0.0;
    for r in residual {
        if !r.is_finite() {
            return f64::INFINITY;
        }
        sum += r * r;
    }
    sum
}

/// The solution x of a x = b by Gaussian elimination with partial pivoting, the pivot the first
/// largest in magnitude; a zero pivot leaves its unknown's step zero.
#[must_use]
fn solve_linear(mut a: [[f64; UNKNOWNS]; UNKNOWNS], mut b: [f64; UNKNOWNS]) -> [f64; UNKNOWNS] {
    for col in 0..UNKNOWNS {
        let mut pivot = col;
        for row in col + 1..UNKNOWNS {
            if a[row][col].abs() > a[pivot][col].abs() {
                pivot = row;
            }
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let diagonal = a[col][col];
        // Exactly zero, and only that, is the sentinel of a column with no pivot: the matrix is
        // singular there, and the unknown's step stays zero rather than infinite.
        if diagonal == 0.0 {
            continue;
        }
        let pivot_row = a[col];
        for row in col + 1..UNKNOWNS {
            let factor = a[row][col] / diagonal;
            // A row with nothing to eliminate is left as it is, bit for bit.
            if factor != 0.0 {
                for (entry, &above) in a[row][col..].iter_mut().zip(&pivot_row[col..]) {
                    *entry -= factor * above;
                }
                b[row] -= factor * b[col];
            }
        }
    }
    let mut x = [0.0; UNKNOWNS];
    for col in (0..UNKNOWNS).rev() {
        let diagonal = a[col][col];
        // The column with no pivot, as above.
        if diagonal == 0.0 {
            continue;
        }
        let mut sum = b[col];
        for (&coefficient, &xj) in a[col][col + 1..].iter().zip(&x[col + 1..]) {
            sum -= coefficient * xj;
        }
        x[col] = sum / diagonal;
    }
    x
}

#[cfg(test)]
mod tests;
