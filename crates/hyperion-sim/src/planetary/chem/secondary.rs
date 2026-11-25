//! The equilibrium of a heavy secondary air (plan 14, P14.T55.a.a; decision-composition §1.3):
//! from a rocky world's air's element budget of H, C, N and O, with He and Ar inert, at one
//! temperature and pressure, the partial pressures of H₂, H₂O, CH₄, CO, CO₂, O₂, N₂ and NH₃,
//! and the carbon's graphite activity.
//!
//! P14.T52.a's [`equilibrium`](super::equilibrium) solves a hydrogen-dominated gas; this module
//! solves the air of a world whose hydrogen is one element among the others, which Woitke et al.
//! (2021, A&A 646, A43, "Coexistence of CH₄, CO₂, and H₂O in exoplanet atmospheres"; recalled,
//! as the composition audit §2.2 reads it) sort by the element budget alone below about 600 K at
//! 1 bar:
//!
//! - **type A**, H₂O, CH₄, NH₃, and H₂ or N₂, where the hydrogen exceeds what the oxygen and
//!   carbon take as H₂O and CH₄ (H ≥ 2 O + 4 C, by atoms);
//! - **type B**, O₂, H₂O, CO₂ and N₂, where the oxygen exceeds what the hydrogen and carbon take
//!   as H₂O and CO₂ (O ≥ H ÷ 2 + 2 C);
//! - **type C**, H₂O, CO₂, CH₄ and N₂ between them, its graphite saturated over most of it and
//!   without bound at its carbon-rich edge, the line from CH₄ to CO₂ in the H–C–O triangle: in the
//!   low-temperature limit `a_C` ≥ 1 over 93% of its area at 300 K and 68% at 600 K, all but a
//!   band along its water-rich side (CH₄ + CO₂ ⇌ 2 C + 2 H₂O, K 135 and 7.6 by the CEA records).
//!
//! Each type's three C–H–O species are fixed by the three element balances, the low-temperature
//! limit ([`AirElements::low_temperature_limit`]), and [`AirElements::woitke_type`] names the
//! region an element budget lies in. Nothing on the record reads the module: P14.T55.a.c's
//! route (research-leans-58) publishes the vent's quenched gas at generator version 36, and this
//! solve names each air's type in the census beside it, as the element budget's low-temperature
//! reference, and is the hot branch's solver above the quench (runaways, steam envelopes and
//! magma oceans) for a later follow-up.
//!
//! # The solution
//!
//! The gas is ideal at the total pressure P given, its pressures in bar. Each species' partial
//! pressure is its formation constant from the elements' reference states
//! ([`log10_formation_constant`]: NASA's CEA records with NIST-JANAF's references, P14.T49.d)
//! times the reference states' pressures: p(H₂O) = `K_w` p(H₂) p(O₂)^½, p(CH₄) = `K_m` `a_C`
//! p(H₂)², p(CO) = `K_o` `a_C` p(O₂)^½, p(CO₂) = `K_d` `a_C` p(O₂) and p(NH₃) = `K_a` p(N₂)^½
//! p(H₂)^{3/2}, `a_C` the carbon's activity against graphite. With the budget scaled to one mole
//! of atoms and λ the pressure a mole of the gas's atoms of one element exerts, its atoms' partial
//! pressure, each element's atoms in the gas are its share of the budget times λ.
//!
//! - **The oxygen potential** is the one bracketed root. The unknown is ω = ln p(O₂) − ln(P −
//!   p(O₂)), a monotone map of ln p(O₂) onto every real number, so that its bracket
//!   [−[`BRACKET`], [`BRACKET`]] holds p(O₂) from about 10⁻³⁰⁴ P to the whole of P; its residual
//!   is the oxygen's balance, ln(O in the gas ÷ (λ `b_O`)), which rises with ω (the dual of the
//!   Gibbs energy's minimum is concave in each element's potential).
//! - **At each trial ω** the other balances close: the carbon's in closed form, `a_C` = λ `b_C`
//!   ÷ (`K_m` p(H₂)² + `K_o` p(O₂)^½ + `K_d` p(O₂)), linear; the nitrogen's and the total
//!   pressure's together in closed form, a quadratic in p(N₂)^½ given p(H₂) (the pressure left
//!   to the carbon, nitrogen and inert gases fixes λ, and the N₂–NH₃ split the rest); and the
//!   hydrogen's, which the carbon's CH₄ and the nitrogen's NH₃ couple to λ, by an inner root of
//!   the same kind, in ψ, the logit of the hydrogen's share φ = (1 + p(H₂O) ÷ p(H₂)) p(H₂) ÷ (P
//!   − p(O₂)) of the pressure the O₂ leaves.
//! - **Each root** takes Newton's steps on its exact derivative (carried through the closed forms
//!   as a dual number), each kept inside the bracket that the residual's signs so far allow and
//!   the bracket bisected where a step would leave it, at most [`OUTER_STEPS`] or [`INNER_STEPS`]
//!   evaluations, stopping after a step of at most [`STEP_TOLERANCE`]. The outer root's slope
//!   is the oxygen residual's total derivative along the inner root, and each inner root starts
//!   from the last one moved along that root's own slope.
//! - **The start** is the low-temperature limit's potentials: the type's majors fix two ratios,
//!   p(H₂O) ÷ p(H₂) and p(CO₂) ÷ p(CH₄) (or p(O₂) itself, or p(CO₂) ÷ p(CO)), so that at 300 K
//!   the roots start next to their solutions and a hot air's within a few units of its logits.
//! - **The check**: a solution whose hydrogen or oxygen balance misses by over [`CONVERGED`]
//!   relative is refused ([`SolveSecondaryError::NotConverged`]) rather than returned; over the
//!   tests' domain none is.
//!
//! Every step is a pure function of the last, every sum is taken in registry order (the species'
//! [`SPECIES`] order), and every transcendental goes through [`math`], so the solution is the same
//! bits on every target (decision-composition §1.3's determinism rules).
//!
//! The one-dimensional root follows Heng and Tsai's (2016, ApJ 829, 104) way of closing a C–H–O–N
//! network on its element balances in closed form around one unknown; theirs is CO's (a decic, in
//! practice its quintic), with H₂ held at the total pressure, which a heavy air does not allow. A
//! closed form in p(H₂) at each trial (the brief's sketch) does not exist once λ is unknown: CH₄'s
//! and NH₃'s hydrogen make the hydrogen balance a root of its own (a lean, 2026-11-25).
//!
//! # Its range
//!
//! - **Below [`FLOOR`]**, 300 K, where graphite's record starts (the carbon species' formation
//!   constants hold from there), the composition is the floor's at the same pressure, held
//!   ([`Regime::HeldAtFloor`]), as P14.T52.a's is. The low-temperature limit holds there within 1%
//!   in the H, C and O carriers at any pressure, and in type A's NH₃ at 1 bar and above where H₂
//!   is left over (the Haber equilibrium gives up NH₃ as the pressure falls: at 0.01 bar it holds
//!   63% of the nitrogen).
//! - **Above [`CEILING`]**, 2,000 K, nothing is solved ([`SolveSecondaryError::AboveRange`]): the
//!   species not carried take a growing share. From the registry's own records at the solution
//!   (the tests' `the_species_not_carried_stay_minor`, over the types' interiors, steam, a
//!   hydrogen-poor oxidised air and an Earth-like one at 1 and 100 bar), each holds under 1% of
//!   any element to 1,500 K, OH the most, 0.74% of the hydrogen-poor air's hydrogen; at 2,000 K OH
//!   holds 19% of that air's hydrogen and the H atom 0.27%, and every other under 0.1%. C₂H₆ and
//!   the other hydrocarbons grow as `a_C`², so they hold over 1% of the carbon only where graphite
//!   is far past saturation, beyond the CH₄–CO₂ line. NO, which is no registry row, is not
//!   counted.
//! - **The pressure** may be any; the gas is ideal. H₂'s fugacity is within about 10% of its
//!   pressure to about 100 bar at 300 K (its compressibility is 1.47 at 700 bar and 288 K;
//!   Leachman et al. 2009, J. Phys. Chem. Ref. Data 38, 721, recalled), and to a few hundred bar
//!   at 1,000 K.
//! - **Condensation** is not this module's: water and CO₂ may be supersaturated, as graphite may
//!   (a type A air at 300 K and 1 bar holds 0.44 bar of H₂O, past its saturation's 0.035 bar,
//!   IAPWS-95's; Wagner and Pruß 2002, J. Phys. Chem. Ref. Data 31, 387, recalled);
//!   the partition at the ground is P14.T51.a's.
//!
//! # Graphite
//!
//! `a_C` is reported ([`SecondaryEquilibrium::graphite_activity`]) and graphite flagged where it
//! is at least 1 ([`SecondaryEquilibrium::graphite_saturated`]): type C's soot, over most of type C
//! at low temperature, P14.T55.d's to condense. The solution stays the gas's own, supersaturated
//! there, a census figure only (research-leans-58). The carbon has no carrier of its own: CH₄
//! takes at most a quarter of the hydrogen's atoms and CO one of the oxygen's, so a budget with C
//! ≥ H ÷ 4 + O has no gas solution and is refused ([`SolveSecondaryError::CarbonBeyondGas`]).

use core::fmt;
use core::ops::{Add, Div, Mul, Sub};
use std::error::Error;

use super::equilibrium::Regime;
use crate::math;
use crate::planetary::derive::atmosphere::Gas;
use crate::substance::{Element, SubstanceId, by_key, log10_formation_constant};
use crate::units::{Kelvin, Pascals};

#[cfg(test)]
mod tests;

/// The species, by registry key, in registry order: rows 0–8 ([`Gas::ALL`]) and CO.
///
/// Every sum over them is taken in this order.
pub const SPECIES: [&str; 10] = [
    "H2", "He", "H2O", "CH4", "NH3", "N2", "O2", "CO2", "Ar", "CO",
];

/// Each species' place in [`SPECIES`].
const H2: usize = 0;
const HE: usize = 1;
const H2O: usize = 2;
const CH4: usize = 3;
const NH3: usize = 4;
const N2: usize = 5;
const O2: usize = 6;
const CO2: usize = 7;
const AR: usize = 8;
const CO: usize = 9;

/// The lowest temperature solved at, K: 300, where CEA's graphite record starts.
///
/// The formation constants of CH₄, CO and CO₂ start there with it (the other species' hold from
/// 200 K). Below it the floor's composition is held ([`Regime::HeldAtFloor`]).
pub const FLOOR: Kelvin = Kelvin::new(300.0);

/// The highest temperature solved at, K: 2,000 (research-leans-58: the hot branch's solver to
/// about 2,000 K), below the records' 6,000 K, where the species not carried stay minor (the
/// [module](self)'s documentation).
pub const CEILING: Kelvin = Kelvin::new(2_000.0);

/// Each root's bracket in its logit, ±700: e⁻⁷⁰⁰ ≈ 10⁻³⁰⁴, just inside the smallest normal
/// `f64`, so that every value at the bracket's ends is finite.
pub const BRACKET: f64 = 700.0;

/// The outer root's (the oxygen's) largest number of evaluations, each with its inner root: 80,
/// past the 54 bisections that narrow [`BRACKET`]'s width to 10⁻¹³.
pub const OUTER_STEPS: u32 = 80;

/// The inner root's (the hydrogen's) largest number of evaluations at one trial of the outer: 80,
/// as [`OUTER_STEPS`].
pub const INNER_STEPS: u32 = 80;

/// The Newton step in a logit after which a root stops, its step taken: 10⁻⁹, past which the
/// next step's change, quadratic in it, is below the `f64` resolution of the logit.
pub const STEP_TOLERANCE: f64 = 1e-9;

/// The largest relative residual of the hydrogen's and the oxygen's balances a solution may keep:
/// 10⁻¹³.
///
/// The carbon's and the nitrogen's close in closed form.
pub const CONVERGED: f64 = 1e-13;

/// Pascals per bar, the formation constants' pressure unit.
const PASCALS_PER_BAR: f64 = 1e5;

/// The share of the total pressure a species the low-temperature limit leaves out is given in
/// the start's ratios: 10⁻¹², so that a boundary's zero has a logarithm.
const START_TRACE: f64 = 1e-12;

/// The rows of [`SPECIES`], by id.
///
/// # Panics
///
/// Never: CO is a row of the registry (a test holds it).
#[must_use]
fn species_ids() -> [SubstanceId; SPECIES.len()] {
    let co = by_key("CO").expect("CO is a registry row");
    let [h2, he, h2o, ch4, nh3, n2, o2, co2, ar] = Gas::ALL.map(Gas::substance);
    [h2, he, h2o, ch4, nh3, n2, o2, co2, ar, co]
}

/// An air's element budget: its moles of H, C, N and O atoms and of the inert He and Ar, in any
/// one unit (only their ratios are solved), each finite and non-negative, not all zero.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct AirElements {
    hydrogen: f64,
    helium: f64,
    carbon: f64,
    nitrogen: f64,
    oxygen: f64,
    argon: f64,
}

/// [`AirElements::new`] could not take a budget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildAirElementsError {
    /// The element is not one the solve carries: H, He, C, N, O or Ar.
    NotCarried(Element),
    /// An element is given twice.
    Repeated(Element),
    /// An element's moles are not finite and non-negative.
    Moles {
        /// The element.
        element: Element,
        /// Its moles as given.
        value: f64,
    },
    /// Every element's moles are zero, or none is given.
    Empty,
}

impl fmt::Display for BuildAirElementsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCarried(element) => write!(
                f,
                "{} is not carried by the heavy-secondary solve",
                element.symbol()
            ),
            Self::Repeated(element) => write!(f, "{} is given twice", element.symbol()),
            Self::Moles { element, value } => write!(
                f,
                "moles {value} of {} are not finite and non-negative",
                element.symbol()
            ),
            Self::Empty => f.write_str("the budget holds no atoms"),
        }
    }
}

impl Error for BuildAirElementsError {}

/// Woitke et al.'s (2021) type of an element budget: the region of the H–C–O triangle it lies
/// in, which below about 600 K sets its major species ([`AirElements::woitke_type`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WoitkeType {
    /// H₂O, CH₄, NH₃, and H₂ or N₂: H ≥ 2 O + 4 C.
    A,
    /// O₂, H₂O, CO₂ and N₂: O ≥ H ÷ 2 + 2 C (and not type A).
    B,
    /// H₂O, CO₂, CH₄ and N₂, between them, its graphite saturated over most of it at low
    /// temperature, and the carbon-rich corner beyond the CH₄–CO₂ line, where graphite would hold
    /// the carbon its gas cannot.
    C,
}

impl AirElements {
    /// The budget of `moles`, each element with its moles of atoms.
    ///
    /// # Errors
    ///
    /// [`BuildAirElementsError`] for an element other than H, He, C, N, O and Ar, one given twice,
    /// moles that are not finite and non-negative, or no atoms at all.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::secondary::{AirElements, WoitkeType};
    /// use hyperion_sim::substance::Element;
    ///
    /// // Hydrogen beyond what the oxygen and carbon take: type A.
    /// let reduced = AirElements::new([(Element::H, 6.0), (Element::C, 0.5), (Element::O, 1.0)])?;
    /// assert_eq!(reduced.woitke_type(), WoitkeType::A);
    /// # Ok::<(), hyperion_sim::planetary::chem::secondary::BuildAirElementsError>(())
    /// ```
    pub fn new(
        moles: impl IntoIterator<Item = (Element, f64)>,
    ) -> Result<Self, BuildAirElementsError> {
        let mut budget = Self {
            hydrogen: 0.0,
            helium: 0.0,
            carbon: 0.0,
            nitrogen: 0.0,
            oxygen: 0.0,
            argon: 0.0,
        };
        let mut given = [false; 6];
        for (element, value) in moles {
            let (slot, field) = match element {
                Element::H => (0, &mut budget.hydrogen),
                Element::He => (1, &mut budget.helium),
                Element::C => (2, &mut budget.carbon),
                Element::N => (3, &mut budget.nitrogen),
                Element::O => (4, &mut budget.oxygen),
                Element::Ar => (5, &mut budget.argon),
                Element::Ne
                | Element::Na
                | Element::Mg
                | Element::Al
                | Element::Si
                | Element::P
                | Element::S
                | Element::Cl
                | Element::K
                | Element::Ca
                | Element::Ti
                | Element::V
                | Element::Cr
                | Element::Mn
                | Element::Fe
                | Element::Ni
                | Element::Zn
                | Element::Kr
                | Element::Xe => return Err(BuildAirElementsError::NotCarried(element)),
            };
            if given[slot] {
                return Err(BuildAirElementsError::Repeated(element));
            }
            if !(value.is_finite() && value >= 0.0) {
                return Err(BuildAirElementsError::Moles { element, value });
            }
            given[slot] = true;
            // −0.0 is stored as +0.0, so that no later `min` ties two signed zeros.
            *field = value + 0.0;
        }
        if budget.total() > 0.0 {
            Ok(budget)
        } else {
            Err(BuildAirElementsError::Empty)
        }
    }

    /// The moles of `element`'s atoms, zero for an element the budget does not carry.
    #[must_use]
    pub const fn moles(&self, element: Element) -> f64 {
        match element {
            Element::H => self.hydrogen,
            Element::He => self.helium,
            Element::C => self.carbon,
            Element::N => self.nitrogen,
            Element::O => self.oxygen,
            Element::Ar => self.argon,
            Element::Ne
            | Element::Na
            | Element::Mg
            | Element::Al
            | Element::Si
            | Element::P
            | Element::S
            | Element::Cl
            | Element::K
            | Element::Ca
            | Element::Ti
            | Element::V
            | Element::Cr
            | Element::Mn
            | Element::Fe
            | Element::Ni
            | Element::Zn
            | Element::Kr
            | Element::Xe => 0.0,
        }
    }

    /// Every atom of the budget, in [`Element::ALL`]'s order.
    #[must_use]
    fn total(&self) -> f64 {
        self.hydrogen + self.helium + self.carbon + self.nitrogen + self.oxygen + self.argon
    }

    /// The budget's Woitke type (Woitke et al. 2021, recalled): A where H ≥ 2 O + 4 C, else B
    /// where O ≥ H ÷ 2 + 2 C, else C.
    ///
    /// Only H, C and O decide it; a budget with none of them, nitrogen and inert gas alone, is
    /// type A.
    #[must_use]
    pub fn woitke_type(&self) -> WoitkeType {
        let (h, c, o) = (self.hydrogen, self.carbon, self.oxygen);
        if h >= 2.0 * o + 4.0 * c {
            WoitkeType::A
        } else if o >= 0.5 * h + 2.0 * c {
            WoitkeType::B
        } else {
            WoitkeType::C
        }
    }

    /// The gas the budget tends to as its temperature falls, with no graphite: its atoms in the
    /// low-temperature limit's species, as moles in [`SPECIES`]' order (the [module](self)'s
    /// types):
    ///
    /// - type A: the oxygen as H₂O, the carbon as CH₄, then the nitrogen as NH₃ while hydrogen is
    ///   left, the rest of the hydrogen as H₂ and of the nitrogen as N₂;
    /// - type B: the hydrogen as H₂O, the carbon as CO₂, the nitrogen as N₂ and the rest of the
    ///   oxygen as O₂;
    /// - type C: H₂O, CH₄ and CO₂ as the three balances fix them, and the nitrogen as N₂;
    /// - beyond the CH₄–CO₂ line (O < 2 C − H ÷ 2), where graphite would condense: the hydrogen
    ///   as CH₄ and the rest of the carbon as CO₂ and CO on the oxygen.
    ///
    /// # Errors
    ///
    /// [`SolveSecondaryError::CarbonBeyondGas`] where C ≥ H ÷ 4 + O, past what CH₄ and CO can
    /// carry.
    ///
    /// # Examples
    ///
    /// Type C's three species are fixed by the three balances:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::secondary::{AirElements, SPECIES, WoitkeType};
    /// use hyperion_sim::substance::Element;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let air = AirElements::new([(Element::H, 4.0), (Element::C, 1.5), (Element::O, 2.5)])?;
    /// assert_eq!(air.woitke_type(), WoitkeType::C);
    /// let limit = air.low_temperature_limit()?;
    /// let moles = |key: &str| SPECIES.iter().position(|&k| k == key).map(|i| limit[i]);
    /// let near = |key: &str, want: f64| moles(key).is_some_and(|m| (m - want).abs() < 1e-12);
    /// // CH₄ = (H ÷ 2 + 2 C − O) ÷ 4, CO₂ = C − CH₄, H₂O = H ÷ 2 − 2 CH₄.
    /// assert!(near("CH4", 0.625));
    /// assert!(near("CO2", 0.875));
    /// assert!(near("H2O", 0.75));
    /// # Ok(())
    /// # }
    /// ```
    pub fn low_temperature_limit(&self) -> Result<[f64; SPECIES.len()], SolveSecondaryError> {
        let (h, c, n, o) = (self.hydrogen, self.carbon, self.nitrogen, self.oxygen);
        self.check_carbon()?;
        let mut moles = [0.0; SPECIES.len()];
        moles[HE] = self.helium;
        moles[AR] = self.argon;
        match self.woitke_type() {
            WoitkeType::A => {
                let rest = h - 2.0 * o - 4.0 * c;
                let ammonia = n.min(rest / 3.0);
                moles[H2O] = o;
                moles[CH4] = c;
                moles[NH3] = ammonia;
                // Where the nitrogen takes all the hydrogen left, a rounding's negative is none.
                let hydrogen = 0.5 * (rest - 3.0 * ammonia);
                moles[H2] = if hydrogen > 0.0 { hydrogen } else { 0.0 };
                moles[N2] = 0.5 * (n - ammonia);
            }
            WoitkeType::B => {
                moles[H2O] = 0.5 * h;
                moles[CO2] = c;
                moles[O2] = 0.5 * (o - 0.5 * h - 2.0 * c);
                moles[N2] = 0.5 * n;
            }
            WoitkeType::C if o >= 2.0 * c - 0.5 * h => {
                let methane = 0.25 * (0.5 * h + 2.0 * c - o);
                moles[H2O] = 0.5 * h - 2.0 * methane;
                moles[CH4] = methane;
                moles[CO2] = c - methane;
                moles[N2] = 0.5 * n;
            }
            WoitkeType::C => {
                let left = c - 0.25 * h;
                moles[CH4] = 0.25 * h;
                moles[CO2] = o - left;
                moles[CO] = 2.0 * left - o;
                moles[N2] = 0.5 * n;
            }
        }
        Ok(moles)
    }

    /// Whether the gas can carry the carbon: C < H ÷ 4 + O, or no carbon.
    fn check_carbon(&self) -> Result<(), SolveSecondaryError> {
        let carriers = 0.25 * self.hydrogen + self.oxygen;
        if self.carbon > 0.0 && self.carbon >= carriers {
            Err(SolveSecondaryError::CarbonBeyondGas {
                carbon: self.carbon,
                carriers,
            })
        } else {
            Ok(())
        }
    }
}

/// [`solve`] could not speciate the air.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SolveSecondaryError {
    /// The temperature is not finite and positive.
    Temperature(Kelvin),
    /// The temperature is above [`CEILING`].
    AboveRange(Kelvin),
    /// The pressure is not finite and positive, or so small (under about 10⁻³⁰³ Pa) that it is
    /// not a normal float in bar.
    Pressure(Pascals),
    /// The budget's carbon is at least what its gas can carry, CH₄ on a quarter of its hydrogen
    /// and CO on its oxygen: only graphite could hold the rest.
    CarbonBeyondGas {
        /// The carbon's moles.
        carbon: f64,
        /// H ÷ 4 + O, the carbon the gas could carry, in the same moles.
        carriers: f64,
    },
    /// A root did not close its balance to [`CONVERGED`].
    NotConverged {
        /// The largest relative residual reached.
        residual: f64,
    },
}

impl fmt::Display for SolveSecondaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temperature(t) => {
                write!(f, "temperature {} K is not finite and positive", t.value())
            }
            Self::AboveRange(t) => write!(
                f,
                "temperature {} K is above the heavy-secondary solve's {} K",
                t.value(),
                CEILING.value()
            ),
            Self::Pressure(p) => write!(f, "pressure {} Pa is not finite and positive", p.value()),
            Self::CarbonBeyondGas { carbon, carriers } => write!(
                f,
                "carbon {carbon} is at least the {carriers} its gas can carry as CH4 and CO"
            ),
            Self::NotConverged { residual } => {
                write!(f, "the solution did not converge (residual {residual})")
            }
        }
    }
}

impl Error for SolveSecondaryError {}

/// A heavy secondary air's equilibrium at one temperature and pressure ([`solve`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondaryEquilibrium {
    temperature: Kelvin,
    pressure: Pascals,
    regime: Regime,
    /// The species' rows, in [`SPECIES`]' order.
    ids: [SubstanceId; SPECIES.len()],
    /// Each species' mole fraction, in [`SPECIES`]' order.
    fractions: [f64; SPECIES.len()],
    graphite_activity: f64,
}

impl SecondaryEquilibrium {
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

    /// Whether the composition is the equilibrium's at the temperature or [`FLOOR`]'s, held.
    #[must_use]
    pub const fn regime(&self) -> Regime {
        self.regime
    }

    /// Each species and its mole fraction, n ÷ Σ n, in [`SPECIES`]' (registry) order: zero for a
    /// species of an element the budget lacks.
    pub fn mole_fractions(&self) -> impl ExactSizeIterator<Item = (SubstanceId, f64)> + '_ {
        self.ids.into_iter().zip(self.fractions)
    }

    /// The mole fraction of `id`, or `None` if the solve does not carry it.
    #[must_use]
    pub fn mole_fraction(&self, id: SubstanceId) -> Option<f64> {
        self.mole_fractions()
            .find(|&(s, _)| s == id)
            .map(|(_, x)| x)
    }

    /// The partial pressure of `id`, its mole fraction times the total, or `None` if the solve
    /// does not carry it.
    #[must_use]
    pub fn partial_pressure(&self, id: SubstanceId) -> Option<Pascals> {
        self.mole_fraction(id).map(|x| self.pressure * x)
    }

    /// The carbon's activity against graphite at the temperature, dimensionless: p(CH₄) ÷ (`K_m`
    /// p(H₂)²), with every pressure in bar; zero with no carbon.
    #[must_use]
    pub const fn graphite_activity(&self) -> f64 {
        self.graphite_activity
    }

    /// Whether graphite is saturated, its activity at least 1: type C's soot, which the gas holds
    /// supersaturated (P14.T55.d condenses it).
    #[must_use]
    pub fn graphite_saturated(&self) -> bool {
        self.graphite_activity >= 1.0
    }
}

/// The equilibrium of the air of budget `elements` at `temperature` and total pressure
/// `pressure` (the [module](self)'s solution).
///
/// # Errors
///
/// [`SolveSecondaryError`] for a temperature that is not finite and positive or is above
/// [`CEILING`], a pressure that is not finite and positive (or not a normal float in bar), a budget
/// whose carbon its gas cannot
/// carry, or a root that did not converge (which the tests hold never happens over the domain).
///
/// # Examples
///
/// An Earth-like budget, oxygen beyond its hydrogen and carbon, is type B: its carbon is CO₂ and
/// its graphite far from saturated. A reduced one is type A, its carbon CH₄:
///
/// ```
/// use hyperion_sim::planetary::chem::secondary::{AirElements, WoitkeType, solve};
/// use hyperion_sim::substance::{Element, by_key};
/// use hyperion_sim::units::{Kelvin, Pascals};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let [ch4, co2] = ["CH4", "CO2"].map(|k| by_key(k).ok_or("a row"));
/// let (ch4, co2) = (ch4?, co2?);
/// let oxidised = AirElements::new([
///     (Element::H, 0.2),
///     (Element::C, 0.1),
///     (Element::N, 0.1),
///     (Element::O, 0.6),
/// ])?;
/// let air = solve(&oxidised, Kelvin::new(400.0), Pascals::new(1e5))?;
/// assert!(air.mole_fraction(co2) > Some(0.2));
/// assert!(!air.graphite_saturated());
/// let reduced = AirElements::new([(Element::H, 6.0), (Element::C, 0.5), (Element::O, 1.0)])?;
/// assert_eq!(reduced.woitke_type(), WoitkeType::A);
/// let air = solve(&reduced, Kelvin::new(400.0), Pascals::new(1e5))?;
/// assert!(air.mole_fraction(ch4) > air.mole_fraction(co2));
/// # Ok(())
/// # }
/// ```
pub fn solve(
    elements: &AirElements,
    temperature: Kelvin,
    pressure: Pascals,
) -> Result<SecondaryEquilibrium, SolveSecondaryError> {
    let t = temperature.value();
    if !(t.is_finite() && t > 0.0) {
        return Err(SolveSecondaryError::Temperature(temperature));
    }
    if t > CEILING.value() {
        return Err(SolveSecondaryError::AboveRange(temperature));
    }
    let p = pressure.value();
    let bar = p / PASCALS_PER_BAR;
    // A pressure whose bar is not a normal float would underflow every partial pressure.
    if !(p.is_finite() && p > 0.0 && bar.is_normal()) {
        return Err(SolveSecondaryError::Pressure(pressure));
    }
    elements.check_carbon()?;
    let (regime, at) = if t < FLOOR.value() {
        (Regime::HeldAtFloor, FLOOR)
    } else {
        (Regime::Equilibrium, temperature)
    };
    let ids = species_ids();
    let constants = Constants::at(at, &ids);
    let problem = Problem::new(elements, &constants, bar);
    let (pressures_bar, graphite_activity) = problem.solve()?;
    let mut total = 0.0;
    for q in pressures_bar {
        total += q;
    }
    if !(total.is_finite() && total > 0.0) {
        return Err(SolveSecondaryError::NotConverged { residual: f64::NAN });
    }
    Ok(SecondaryEquilibrium {
        temperature,
        pressure,
        regime,
        ids,
        fractions: pressures_bar.map(|q| q / total),
        graphite_activity,
    })
}

/// The formation constants at one temperature, each K, with its natural logarithm, of H₂O, CH₄,
/// CO, CO₂ and NH₃ from the elements' reference states, pressures in bar.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Constants {
    water: Constant,
    methane: Constant,
    monoxide: Constant,
    dioxide: Constant,
    ammonia: Constant,
}

/// One formation constant, K and ln K.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Constant {
    k: f64,
    ln: f64,
}

impl Constants {
    /// The constants at `temperature`, within [`FLOOR`] and [`CEILING`], of the species `ids`
    /// ([`species_ids`]).
    ///
    /// # Panics
    ///
    /// Never: every species' record holds from [`FLOOR`] to [`CEILING`] (a test holds it).
    #[must_use]
    fn at(temperature: Kelvin, ids: &[SubstanceId; SPECIES.len()]) -> Self {
        let constant = |place: usize| {
            let log10 = log10_formation_constant(ids[place], temperature)
                .expect("every species' record holds from the floor to the ceiling");
            let ln = log10 * core::f64::consts::LN_10;
            Constant {
                k: math::exp(ln),
                ln,
            }
        };
        Self {
            water: constant(H2O),
            methane: constant(CH4),
            monoxide: constant(CO),
            dioxide: constant(CO2),
            ammonia: constant(NH3),
        }
    }
}

/// A value with its derivatives in the two unknowns: ψ, the hydrogen's logit, and ω, the
/// oxygen's.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Dual {
    value: f64,
    by_hydrogen: f64,
    by_oxygen: f64,
}

impl Dual {
    /// A value that depends on neither unknown.
    #[must_use]
    const fn constant(value: f64) -> Self {
        Self {
            value,
            by_hydrogen: 0.0,
            by_oxygen: 0.0,
        }
    }

    /// `self` times the plain number `k`.
    #[must_use]
    fn scale(self, k: f64) -> Self {
        Self {
            value: self.value * k,
            by_hydrogen: self.by_hydrogen * k,
            by_oxygen: self.by_oxygen * k,
        }
    }

    /// √self, with no slope where the value is zero (an exact zero or an underflow), where the
    /// slope's own factor is zero too.
    #[must_use]
    fn sqrt(self) -> Self {
        let root = self.value.sqrt();
        if root > 0.0 {
            let half = 0.5 / root;
            Self {
                value: root,
                by_hydrogen: self.by_hydrogen * half,
                by_oxygen: self.by_oxygen * half,
            }
        } else {
            Self::constant(root)
        }
    }

    /// ln self.
    #[must_use]
    fn ln(self) -> Self {
        Self {
            value: math::ln(self.value),
            by_hydrogen: self.by_hydrogen / self.value,
            by_oxygen: self.by_oxygen / self.value,
        }
    }
}

impl Add for Dual {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            by_hydrogen: self.by_hydrogen + other.by_hydrogen,
            by_oxygen: self.by_oxygen + other.by_oxygen,
        }
    }
}

impl Sub for Dual {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            value: self.value - other.value,
            by_hydrogen: self.by_hydrogen - other.by_hydrogen,
            by_oxygen: self.by_oxygen - other.by_oxygen,
        }
    }
}

impl Mul for Dual {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self {
            value: self.value * other.value,
            by_hydrogen: self.by_hydrogen * other.value + self.value * other.by_hydrogen,
            by_oxygen: self.by_oxygen * other.value + self.value * other.by_oxygen,
        }
    }
}

impl Div for Dual {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        let quotient = self.value / other.value;
        Self {
            value: quotient,
            by_hydrogen: (self.by_hydrogen - quotient * other.by_hydrogen) / other.value,
            by_oxygen: (self.by_oxygen - quotient * other.by_oxygen) / other.value,
        }
    }
}

/// The logistic σ(x) = 1 ÷ (1 + e⁻ˣ) of `x` (a logit) and 1 − σ(x), each from one exponential of
/// −|x|, so that neither loses its tail to rounding; their slope is σ (1 − σ).
#[must_use]
fn logistic(x: Dual) -> (Dual, Dual) {
    let tail = math::exp(-x.value.abs());
    let near = 1.0 / (1.0 + tail);
    let far = tail * near;
    let (share, rest) = if x.value >= 0.0 {
        (near, far)
    } else {
        (far, near)
    };
    let slope = share * rest;
    (
        Dual {
            value: share,
            by_hydrogen: slope * x.by_hydrogen,
            by_oxygen: slope * x.by_oxygen,
        },
        Dual {
            value: rest,
            by_hydrogen: -slope * x.by_hydrogen,
            by_oxygen: -slope * x.by_oxygen,
        },
    )
}

/// What one trial gives: each balance's residual, the species' partial pressures, bar, in
/// [`SPECIES`]' order, and the graphite activity.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Trial {
    /// ln(H in the gas ÷ λ `b_H`), zero where the hydrogen closes in closed form.
    hydrogen: Dual,
    /// ln(O in the gas ÷ λ `b_O`), zero with no oxygen.
    oxygen: Dual,
    pressures_bar: [f64; SPECIES.len()],
    graphite_activity: f64,
}

/// One solve's fixed inputs: the budget as shares of its atoms, the constants and the pressure.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Problem<'a> {
    hydrogen: f64,
    helium: f64,
    carbon: f64,
    nitrogen: f64,
    oxygen: f64,
    argon: f64,
    /// The atoms that share the pressure the hydrogen and oxygen leave: He + C + N + Ar.
    rest: f64,
    constants: &'a Constants,
    /// The total pressure, bar.
    bar: f64,
}

impl<'a> Problem<'a> {
    /// The problem of `elements` over `constants` at `bar` bar, the budget scaled to one mole of
    /// atoms.
    #[must_use]
    fn new(elements: &AirElements, constants: &'a Constants, bar: f64) -> Self {
        let total = elements.total();
        let share = |moles: f64| moles / total;
        let (carbon, nitrogen) = (share(elements.carbon), share(elements.nitrogen));
        let (helium, argon) = (share(elements.helium), share(elements.argon));
        Self {
            hydrogen: share(elements.hydrogen),
            helium,
            carbon,
            nitrogen,
            oxygen: share(elements.oxygen),
            argon,
            rest: helium + carbon + nitrogen + argon,
            constants,
            bar,
        }
    }

    /// Whether the hydrogen's balance takes the inner root: there is hydrogen, and the carbon,
    /// nitrogen or inert gas share the pressure with it (else the hydrogen and oxygen fill it, and
    /// λ follows from the hydrogen's balance).
    #[must_use]
    fn hydrogen_is_rooted(&self) -> bool {
        self.hydrogen > 0.0 && self.rest > 0.0
    }

    /// The trial at the logits `psi` (the hydrogen's) and `omega` (the oxygen's): the closed forms
    /// of the [module](self)'s solution.
    #[must_use]
    fn trial(&self, psi: f64, omega: f64) -> Trial {
        let k = self.constants;
        let bar = Dual::constant(self.bar);
        let (oxygen, room) = if self.oxygen > 0.0 {
            let (share, rest) = logistic(Dual {
                value: omega,
                by_hydrogen: 0.0,
                by_oxygen: 1.0,
            });
            (share * bar, rest * bar)
        } else {
            (Dual::constant(0.0), bar)
        };
        let root_oxygen = oxygen.sqrt();
        // H₂O ÷ H₂, and the pressure the hydrogen's H₂ and H₂O take with it.
        let water_ratio = root_oxygen.scale(k.water.k);
        let one = Dual::constant(1.0);
        let (hydrogen, left) = if self.hydrogen <= 0.0 {
            (Dual::constant(0.0), room)
        } else if self.rest > 0.0 {
            let (share, rest) = logistic(Dual {
                value: psi,
                by_hydrogen: 1.0,
                by_oxygen: 0.0,
            });
            (share * room / (one + water_ratio), rest * room)
        } else {
            (room / (one + water_ratio), Dual::constant(0.0))
        };
        // NH₃ ÷ N₂^½.
        let ammonia_ratio = (hydrogen * hydrogen.sqrt()).scale(k.ammonia.k);
        let carbon_sum = (hydrogen * hydrogen).scale(k.methane.k)
            + oxygen.scale(k.dioxide.k)
            + root_oxygen.scale(k.monoxide.k);
        let water = water_ratio * hydrogen;
        let (root_nitrogen, scale) = if self.rest > 0.0 {
            // The nitrogen's and the pressure's balances: (2 B − b_N) z² + B a z − b_N R = 0,
            // with B the rest's atoms, a NH₃ ÷ z, R the pressure left and z = p(N₂)^½, taken
            // in the form that does not cancel; then λ = (R + z²) ÷ B.
            let z = if self.nitrogen > 0.0 {
                let b = self.rest;
                let quadratic = 2.0 * b - self.nitrogen;
                let disc = (ammonia_ratio * ammonia_ratio).scale(b * b)
                    + left.scale(4.0 * quadratic * self.nitrogen);
                left.scale(2.0 * self.nitrogen) / (ammonia_ratio.scale(b) + disc.sqrt())
            } else {
                Dual::constant(0.0)
            };
            (z, (left + z * z).scale(1.0 / self.rest))
        } else {
            // Only hydrogen and oxygen: λ from the hydrogen's balance, H₂ and H₂O alone.
            (
                Dual::constant(0.0),
                (hydrogen + water).scale(2.0 / self.hydrogen),
            )
        };
        let activity = if self.carbon > 0.0 {
            scale.scale(self.carbon) / carbon_sum
        } else {
            Dual::constant(0.0)
        };
        let methane = (activity * hydrogen * hydrogen).scale(k.methane.k);
        let ammonia = ammonia_ratio * root_nitrogen;
        let nitrogen = root_nitrogen * root_nitrogen;
        let dioxide = (activity * oxygen).scale(k.dioxide.k);
        let monoxide = (activity * root_oxygen).scale(k.monoxide.k);
        let residual_hydrogen = if self.hydrogen_is_rooted() {
            let held =
                hydrogen.scale(2.0) + water.scale(2.0) + methane.scale(4.0) + ammonia.scale(3.0);
            (held / scale.scale(self.hydrogen)).ln()
        } else {
            Dual::constant(0.0)
        };
        let residual_oxygen = if self.oxygen > 0.0 {
            let held = water + oxygen.scale(2.0) + dioxide.scale(2.0) + monoxide;
            (held / scale.scale(self.oxygen)).ln()
        } else {
            Dual::constant(0.0)
        };
        let mut pressures_bar = [0.0; SPECIES.len()];
        pressures_bar[H2] = hydrogen.value;
        pressures_bar[HE] = scale.value * self.helium;
        pressures_bar[H2O] = water.value;
        pressures_bar[CH4] = methane.value;
        pressures_bar[NH3] = ammonia.value;
        pressures_bar[N2] = nitrogen.value;
        pressures_bar[O2] = oxygen.value;
        pressures_bar[CO2] = dioxide.value;
        pressures_bar[AR] = scale.value * self.argon;
        pressures_bar[CO] = monoxide.value;
        Trial {
            hydrogen: residual_hydrogen,
            oxygen: residual_oxygen,
            pressures_bar,
            graphite_activity: activity.value,
        }
    }

    /// The hydrogen's logit at the oxygen's `omega`, from `start`: the inner root, or `start`
    /// where the hydrogen closes in closed form.
    #[must_use]
    fn hydrogen_root(&self, omega: f64, start: f64) -> f64 {
        if self.hydrogen_is_rooted() {
            root(start, INNER_STEPS, |psi| {
                let trial = self.trial(psi, omega);
                (trial.hydrogen.value, trial.hydrogen.by_hydrogen)
            })
        } else {
            start
        }
    }

    /// The solution's partial pressures, bar, in [`SPECIES`]' order, and its graphite activity.
    fn solve(&self) -> Result<([f64; SPECIES.len()], f64), SolveSecondaryError> {
        if self.hydrogen <= 0.0 && self.rest <= 0.0 {
            // Oxygen alone: O₂ fills the pressure, and no balance is left to root.
            let mut pressures_bar = [0.0; SPECIES.len()];
            pressures_bar[O2] = self.bar;
            return Ok((pressures_bar, 0.0));
        }
        let (omega_start, psi_start) = self.start();
        // The last inner root, its ω, and its slope along the oxygen's logit, which start the
        // next inner root.
        let mut last = (omega_start, psi_start, 0.0);
        let omega = if self.oxygen > 0.0 {
            root(omega_start, OUTER_STEPS, |omega| {
                let (at, psi, slope) = last;
                let psi = self
                    .hydrogen_root(omega, (psi + slope * (omega - at)).clamp(-BRACKET, BRACKET));
                let trial = self.trial(psi, omega);
                let (h, o) = (trial.hydrogen, trial.oxygen);
                let (along, total) = if self.hydrogen_is_rooted() && h.by_hydrogen > 0.0 {
                    let along = -h.by_oxygen / h.by_hydrogen;
                    (along, o.by_oxygen + o.by_hydrogen * along)
                } else {
                    (0.0, o.by_oxygen)
                };
                last = (omega, psi, if along.is_finite() { along } else { 0.0 });
                (o.value, total)
            })
        } else {
            omega_start
        };
        let (at, psi, slope) = last;
        let psi = self.hydrogen_root(omega, (psi + slope * (omega - at)).clamp(-BRACKET, BRACKET));
        let trial = self.trial(psi, omega);
        let (hydrogen, oxygen) = (trial.hydrogen.value.abs(), trial.oxygen.value.abs());
        // `max` would pass a NaN over; each is tested.
        let residual = if hydrogen.is_nan() || oxygen.is_nan() {
            f64::NAN
        } else {
            hydrogen.max(oxygen)
        };
        if residual.is_nan()
            || residual > CONVERGED
            || trial
                .pressures_bar
                .iter()
                .any(|q| !(q.is_finite() && *q >= 0.0))
        {
            return Err(SolveSecondaryError::NotConverged { residual });
        }
        Ok((trial.pressures_bar, trial.graphite_activity))
    }

    /// The roots' start: the logits of the low-temperature limit's potentials (the
    /// [module](self)'s start), ω first.
    #[must_use]
    fn start(&self) -> (f64, f64) {
        let k = self.constants;
        let budget = AirElements {
            hydrogen: self.hydrogen,
            helium: self.helium,
            carbon: self.carbon,
            nitrogen: self.nitrogen,
            oxygen: self.oxygen,
            argon: self.argon,
        };
        let Ok(moles) = budget.low_temperature_limit() else {
            // `solve` refused the budget as given; its shares can still round onto C = H ÷ 4 +
            // O. The bracket's middle is a start like any other, and the final check refuses a
            // solution that misses.
            return (0.0, 0.0);
        };
        let mut total = 0.0;
        for m in moles {
            total += m;
        }
        let ln_bar = math::ln(self.bar);
        // ln of each species' pressure, bar, a trace where the limit leaves it out.
        let ln_p = |place: usize| math::ln(moles[place].max(START_TRACE * total) / total) + ln_bar;
        let (ln_h2, ln_root_o2) = match budget.woitke_type() {
            WoitkeType::A => {
                let ln_h2 = if moles[H2] > START_TRACE * total {
                    ln_p(H2)
                } else {
                    // The nitrogen's NH₃ and N₂ fix the hydrogen where its H₂ is spent.
                    (2.0 / 3.0) * (ln_p(NH3) - k.ammonia.ln - 0.5 * ln_p(N2))
                };
                (ln_h2, ln_p(H2O) - k.water.ln - ln_h2)
            }
            WoitkeType::B => {
                let ln_root_o2 = 0.5 * ln_p(O2);
                (ln_p(H2O) - k.water.ln - ln_root_o2, ln_root_o2)
            }
            WoitkeType::C if moles[H2O] > START_TRACE * total => {
                // H₂O fixes p(H₂) p(O₂)^½, and CO₂ ÷ CH₄ their ratio.
                let product = ln_p(H2O) - k.water.ln;
                let ratio = 0.5 * (ln_p(CO2) - ln_p(CH4) + k.methane.ln - k.dioxide.ln);
                (0.5 * (product - ratio), f64::midpoint(product, ratio))
            }
            WoitkeType::C => {
                // CO₂ ÷ CO fixes p(O₂)^½, CO the activity, and CH₄ then p(H₂).
                let ln_root_o2 = ln_p(CO2) - ln_p(CO) + k.monoxide.ln - k.dioxide.ln;
                let ln_activity = ln_p(CO) - k.monoxide.ln - ln_root_o2;
                (0.5 * (ln_p(CH4) - k.methane.ln - ln_activity), ln_root_o2)
            }
        };
        let o2 = math::exp(2.0 * ln_root_o2).min(self.bar * (1.0 - START_TRACE));
        let ln_room = math::ln(self.bar - o2);
        let omega = 2.0 * ln_root_o2 - ln_room;
        // φ = (1 + K_w p(O₂)^½) p(H₂) ÷ (P − p(O₂)), its logit held inside the bracket.
        let ln_water_ratio = k.water.ln + ln_root_o2;
        let ln_share = ln_h2 + softplus(ln_water_ratio) - ln_room;
        let share = math::exp(ln_share.min(0.0)).min(1.0 - 1e-15);
        let psi = ln_share - math::ln_1p(-share);
        (omega.clamp(-BRACKET, BRACKET), psi.clamp(-BRACKET, BRACKET))
    }
}

/// ln(1 + eˣ), without overflow.
#[must_use]
fn softplus(x: f64) -> f64 {
    if x > 0.0 {
        x + math::ln_1p(math::exp(-x))
    } else {
        math::ln_1p(math::exp(x))
    }
}

/// The root of `f`, an increasing function of a logit that gives its value and slope, from
/// `start`: Newton's steps, each kept inside the bracket the values' signs so far allow
/// ([`BRACKET`] at first) and the bracket bisected where a step would leave it, at most `steps`
/// evaluations, stopping after a step of at most [`STEP_TOLERANCE`] (the [module](self)'s
/// solution).
fn root(start: f64, steps: u32, mut f: impl FnMut(f64) -> (f64, f64)) -> f64 {
    let (mut low, mut high) = (-BRACKET, BRACKET);
    let mut x = start;
    for _ in 0..steps {
        let (value, slope) = f(x);
        if value < 0.0 {
            low = x;
        } else {
            high = x;
        }
        let step = -value / slope;
        let next = x + step;
        if step.is_finite() && slope > 0.0 && step.abs() <= STEP_TOLERANCE {
            return next.clamp(low, high);
        }
        x = if step.is_finite() && slope > 0.0 && low < next && next < high {
            next
        } else {
            f64::midpoint(low, high)
        };
    }
    x
}
