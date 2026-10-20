//! The gases' equilibrium constants by temperature, from NASA's CEA data (plan 14, P14.T49.d;
//! decision-composition §5): what P14.T52.a's speciation solves with.
//!
//! - [`log10_formation_constant`] gives a gas row's log₁₀ `K_f(T)`, the equilibrium constant of its
//!   formation at 1 bar from the elements in their reference states, and [`formation_range`] the
//!   temperatures it holds over.
//! - [`Reaction`] is a balanced reaction among gas rows, checked once when it is built, and
//!   [`Reaction::log10_equilibrium_constant`] its log₁₀ K(T), from the species' own standard Gibbs
//!   energies.
//!
//! # The data
//!
//! `tables::thermo`, written by `hyperion-fit`'s `thermo_equilibrium`: NASA Glenn's
//! 9-coefficient records (McBride, Zehe and Gordon 2002, NASA/TP-2002-211556), as NASA's CEA ships
//! them in `data/thermo.inp` under Apache 2.0, re-emitted interval by interval. Each interval holds
//! a₁…a₇, b₁ and b₂ of
//!
//! ```text
//! H ÷ RT = −a₁ T⁻² + a₂ ln(T) ÷ T + a₃ + a₄ T ÷ 2 + a₅ T² ÷ 3 + a₆ T³ ÷ 4 + a₇ T⁴ ÷ 5 + b₁ ÷ T
//! S ÷ R  = −a₁ T⁻² ÷ 2 − a₂ T⁻¹ + a₃ ln T + a₄ T + a₅ T² ÷ 2 + a₆ T³ ÷ 3 + a₇ T⁴ ÷ 4 + b₂
//! ```
//!
//! (their eqs. (2) and (3), p. 2), H being the assigned enthalpy (zero for each element's reference
//! state at 298.15 K) and S the entropy at 1 bar, so G ÷ RT = H ÷ RT − S ÷ R and, as their eq. (9)
//! (p. 3) gives an equilibrium constant,
//!
//! ```text
//! log₁₀ K_f = −(G ÷ RT − Σ nₑ G_ref,e ÷ RT) ÷ ln 10,
//! log₁₀ K   = −Σ νᵢ Gᵢ ÷ RT ÷ ln 10.
//! ```
//!
//! The polynomials are the source's, not a fit to it; the records' intervals stop at 6,000 K, and
//! none is extrapolated below its lowest bound (200, 298.15 or 300 K). A value here is CEA's
//! records evaluated with NIST-JANAF's reference states (below), not one CEA prints: CEA keeps a
//! condensed element condensed over its whole range (McBride, Zehe and Gordon 2002, p. 2).
//!
//! # The reference states
//!
//! NIST-JANAF's convention (Chase 1998, Introduction §5, pp. 13–14): each element's reference state
//! is its stable form at 1 bar, from CEA's own records.
//!
//! - **H, N, O, Cl and the noble gases** are their gases, H₂, N₂, O₂, Cl₂, He, Ne, Ar, Kr and Xe,
//!   at every temperature.
//! - **Carbon** is graphite throughout.
//! - **Every other element** is its condensed phases (CEA's Ref-Elm records: iron's α, γ and δ and
//!   its liquid) up to the temperature where their G per atom meets its gas form's at 1 bar, and
//!   that gas above it: the monatomic gas for the metals, S₂ for sulphur and P₂ for phosphorus, as
//!   JANAF's reference tables hold them. The switch is the table's `<element>_BOILING_K`, found by
//!   bisection on the records; each is within 15 K of JANAF's (vanadium's 3,704.9 K against
//!   3,690.1, the farthest; sodium's 1,170.43 K against 1,170.525). `S_BOILING_K` and
//!   `P_BOILING_K` are not boiling points: each is where S₂'s or P₂'s fugacity over the condensed
//!   element reaches 1 bar, not where all of the element's vapour species together reach it.
//! - **The electron gas** `e-` is the reference for charge, so an ion's `K_f` takes its electrons'
//!   Gibbs energy: `K_f(Na⁺)` is that of Na(ref) → Na⁺ + e⁻.
//!
//! So `K_f` is continuous in temperature, and holds over the species' record and its elements'
//! references together: from 300 K for most rows (200 K for H₂, H₂O, NH₃, N₂, O₂, Ar, H and O;
//! 298.15 K for e⁻, H⁻ and the ions of Mg, Si and Ca) to 6,000 K.
//!
//! # What is not modelled
//!
//! `FeH` has no CEA record ([`NOT_MODELLED`]; a closed set, for the composition audit), so P14.T52.d
//! takes its thermochemistry from elsewhere or labels it. OH, the P and Cl atoms and their ions,
//! which P14.T52.a's closed form may want, are not registry rows: CEA holds them, and a row added
//! later takes its record by a refit.
//!
//! # A second thermochemistry
//!
//! P14.T49.b's [`thermo`](super::thermo) sums each gas row's heat capacity over its molecular
//! constants, a statistical sum; CEA's polynomials are the other. Its tests already hold the sums to
//! NASA Glenn's `c_p`, and this module's tests compare the two on the table's own records.
//!
//! Nothing on the record reads these functions at generator version 22: P14.T52.a is their first
//! reader, and P14.T54.b restamps the table at 24 (decision-bump-plan-after-22 §1.3).

use std::error::Error;
use std::fmt;

use super::{Element, SubstanceId, SubstanceKey};
use crate::math;
use crate::tables::thermo as table;
use crate::units::Kelvin;

/// One NASA 9-coefficient interval as the table holds it: its lower and upper bounds, K, then a₁…a₇,
/// b₁ and b₂.
type Interval = [f64; 11];

/// The gas rows of the registry that have no record, each with why: a closed set, for the
/// composition audit (P14.T52.d's).
pub const NOT_MODELLED: [(&str, &str); 1] = [(
    "FeH",
    "NASA's CEA library holds no FeH record, nor NIST-JANAF a table: P14.T52.d's iron hydride takes \
     its thermochemistry from elsewhere or stays labelled",
)];

/// Each gas row's record, by key, in registry order: every gas row but [`NOT_MODELLED`]'s.
const RECORDS: [(&str, &[Interval]); 57] = [
    ("H2", &table::H2),
    ("He", &table::HE),
    ("H2O", &table::H2O),
    ("CH4", &table::CH4),
    ("NH3", &table::NH3),
    ("N2", &table::N2),
    ("O2", &table::O2),
    ("CO2", &table::CO2),
    ("Ar", &table::AR),
    ("Ne", &table::NE),
    ("Kr", &table::KR),
    ("Xe", &table::XE),
    ("N2O", &table::N2O),
    ("CO", &table::CO),
    ("SO2", &table::SO2),
    ("H2S", &table::H2S),
    ("O3", &table::O3),
    ("HCN", &table::HCN),
    ("C2H2", &table::C2H2),
    ("C2H4", &table::C2H4),
    ("C2H6", &table::C2H6),
    ("PH3", &table::PH3),
    ("CH3OH", &table::CH3OH),
    ("H", &table::H),
    ("O", &table::O),
    ("e-", &table::ELECTRON),
    ("H-", &table::H_MINUS),
    ("Na", &table::NA),
    ("K", &table::K),
    ("Fe", &table::FE),
    ("Mg", &table::MG),
    ("Si", &table::SI),
    ("Ca", &table::CA),
    ("Ti", &table::TI),
    ("SiO", &table::SIO),
    ("TiO", &table::TIO),
    ("VO", &table::VO),
    ("Al", &table::AL),
    ("V", &table::V),
    ("Mn", &table::MN),
    ("Ni", &table::NI),
    ("Zn", &table::ZN),
    ("Na+", &table::NA_PLUS),
    ("Mg+", &table::MG_PLUS),
    ("Al+", &table::AL_PLUS),
    ("Si+", &table::SI_PLUS),
    ("K+", &table::K_PLUS),
    ("Ca+", &table::CA_PLUS),
    ("Ti+", &table::TI_PLUS),
    ("V+", &table::V_PLUS),
    ("Cr+", &table::CR_PLUS),
    ("Mn+", &table::MN_PLUS),
    ("Fe+", &table::FE_PLUS),
    ("Ni+", &table::NI_PLUS),
    ("Zn+", &table::ZN_PLUS),
    ("HCl", &table::HCL),
    ("Cr", &table::CR),
];

/// G ÷ RT of piecewise `intervals` at `t` kelvin, from the first interval that holds it (the lower
/// at a shared bound), or `None` where none does.
#[must_use]
fn gibbs_over_rt(intervals: &[Interval], t: f64) -> Option<f64> {
    let &[_, _, a1, a2, a3, a4, a5, a6, a7, b1, b2] =
        intervals.iter().find(|i| i[0] <= t && t <= i[1])?;
    let ln_t = math::ln(t);
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let h = -a1 / t2
        + a2 * ln_t / t
        + a3
        + a4 * t / 2.0
        + a5 * t2 / 3.0
        + a6 * t3 / 4.0
        + a7 * t4 / 5.0
        + b1 / t;
    let s = -a1 / (2.0 * t2) - a2 / t
        + a3 * ln_t
        + a4 * t
        + a5 * t2 / 2.0
        + a6 * t3 / 3.0
        + a7 * t4 / 4.0
        + b2;
    Some(h - s)
}

/// The bounds of piecewise `intervals`, K: the first's lower and the last's upper (the table's
/// intervals join end to end).
#[must_use]
fn bounds(intervals: &[Interval]) -> (f64, f64) {
    match (intervals.first(), intervals.last()) {
        (Some(first), Some(last)) => (first[0], last[1]),
        // The table emits no empty record; an empty one holds no temperature.
        _ => (f64::INFINITY, f64::NEG_INFINITY),
    }
}

/// An element's reference state at 1 bar.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Reference {
    /// Its gas at every temperature, of `atoms` atoms per formula unit.
    Gas {
        record: &'static [Interval],
        atoms: f64,
    },
    /// Graphite at every temperature.
    Graphite,
    /// Its condensed `phases` up to `boiling_k`, then its gas form of `atoms` atoms.
    Condensed {
        phases: &'static [Interval],
        boiling_k: f64,
        gas: &'static [Interval],
        atoms: f64,
    },
}

impl Reference {
    /// The reference state of `element` (the module's documentation).
    #[must_use]
    const fn of(element: Element) -> Self {
        const fn gas(record: &'static [Interval], atoms: f64) -> Reference {
            Reference::Gas { record, atoms }
        }
        const fn condensed(
            phases: &'static [Interval],
            boiling_k: f64,
            gas: &'static [Interval],
            atoms: f64,
        ) -> Reference {
            Reference::Condensed {
                phases,
                boiling_k,
                gas,
                atoms,
            }
        }
        match element {
            Element::H => gas(&table::H2, 2.0),
            Element::He => gas(&table::HE, 1.0),
            Element::C => Self::Graphite,
            Element::N => gas(&table::N2, 2.0),
            Element::O => gas(&table::O2, 2.0),
            Element::Ne => gas(&table::NE, 1.0),
            Element::Na => condensed(&table::NA_CONDENSED, table::NA_BOILING_K, &table::NA, 1.0),
            Element::Mg => condensed(&table::MG_CONDENSED, table::MG_BOILING_K, &table::MG, 1.0),
            Element::Al => condensed(&table::AL_CONDENSED, table::AL_BOILING_K, &table::AL, 1.0),
            Element::Si => condensed(&table::SI_CONDENSED, table::SI_BOILING_K, &table::SI, 1.0),
            Element::P => condensed(&table::P_CONDENSED, table::P_BOILING_K, &table::P2, 2.0),
            Element::S => condensed(&table::S_CONDENSED, table::S_BOILING_K, &table::S2, 2.0),
            Element::Cl => gas(&table::CL2, 2.0),
            Element::Ar => gas(&table::AR, 1.0),
            Element::K => condensed(&table::K_CONDENSED, table::K_BOILING_K, &table::K, 1.0),
            Element::Ca => condensed(&table::CA_CONDENSED, table::CA_BOILING_K, &table::CA, 1.0),
            Element::Ti => condensed(&table::TI_CONDENSED, table::TI_BOILING_K, &table::TI, 1.0),
            Element::V => condensed(&table::V_CONDENSED, table::V_BOILING_K, &table::V, 1.0),
            Element::Cr => condensed(&table::CR_CONDENSED, table::CR_BOILING_K, &table::CR, 1.0),
            Element::Mn => condensed(&table::MN_CONDENSED, table::MN_BOILING_K, &table::MN, 1.0),
            Element::Fe => condensed(&table::FE_CONDENSED, table::FE_BOILING_K, &table::FE, 1.0),
            Element::Ni => condensed(&table::NI_CONDENSED, table::NI_BOILING_K, &table::NI, 1.0),
            Element::Zn => condensed(&table::ZN_CONDENSED, table::ZN_BOILING_K, &table::ZN, 1.0),
            Element::Kr => gas(&table::KR, 1.0),
            Element::Xe => gas(&table::XE, 1.0),
        }
    }

    /// The temperatures it holds over, K: its gas's, graphite's, or its condensed phases' foot to
    /// its gas's top.
    #[must_use]
    fn bounds(self) -> (f64, f64) {
        match self {
            Self::Gas { record, .. } => bounds(record),
            Self::Graphite => bounds(&table::C_GRAPHITE),
            Self::Condensed { phases, gas, .. } => (bounds(phases).0, bounds(gas).1),
        }
    }

    /// G ÷ RT per atom at `t` kelvin, or `None` outside its records.
    #[must_use]
    fn gibbs_over_rt(self, t: f64) -> Option<f64> {
        match self {
            Self::Gas { record, atoms } => Some(gibbs_over_rt(record, t)? / atoms),
            Self::Graphite => gibbs_over_rt(&table::C_GRAPHITE, t),
            Self::Condensed {
                phases,
                boiling_k,
                gas,
                atoms,
            } => {
                if t <= boiling_k {
                    gibbs_over_rt(phases, t)
                } else {
                    Some(gibbs_over_rt(gas, t)? / atoms)
                }
            }
        }
    }
}

/// A record's place in [`RECORDS`] fits the `u8` that a [`Reaction`]'s terms hold it in.
const _: () = assert!(RECORDS.len() <= 1 << u8::BITS);

/// The place in [`RECORDS`] of row `id`'s record, or `None` for a row with none.
#[must_use]
fn record_index(id: SubstanceId) -> Option<u8> {
    let key = id.substance().key();
    let place = RECORDS.iter().position(|(k, _)| *k == key.as_str())?;
    Some(u8::try_from(place).expect("RECORDS holds at most 256 records (the assertion above)"))
}

/// The record of row `id`, or `None` for a row with none.
#[must_use]
fn record(id: SubstanceId) -> Option<&'static [Interval]> {
    record_index(id).map(|i| RECORDS[usize::from(i)].1)
}

/// Where a species of `record`, `elements` and `charge` has a `K_f`, K: its record's range and its
/// elements' references' (and the electron gas's, for an ion) together, or `None` if they do not
/// meet.
#[must_use]
fn formation_bounds(
    record: &[Interval],
    elements: &[(Element, u16)],
    charge: i8,
) -> Option<(f64, f64)> {
    let (mut lower, mut upper) = bounds(record);
    for &(element, _) in elements {
        let (l, u) = Reference::of(element).bounds();
        lower = lower.max(l);
        upper = upper.min(u);
    }
    if charge != 0 {
        let (l, u) = bounds(&table::ELECTRON);
        lower = lower.max(l);
        upper = upper.min(u);
    }
    (lower <= upper).then_some((lower, upper))
}

/// log₁₀ `K_f` at `t` kelvin of a species of `record`, `elements` and `charge`, or `None` outside
/// [`formation_bounds`].
#[must_use]
fn formation_constant(
    record: &[Interval],
    elements: &[(Element, u16)],
    charge: i8,
    t: f64,
) -> Option<f64> {
    let (lower, upper) = formation_bounds(record, elements, charge)?;
    if !(lower <= t && t <= upper) {
        return None;
    }
    let mut g = gibbs_over_rt(record, t)?;
    for &(element, count) in elements {
        g -= f64::from(count) * Reference::of(element).gibbs_over_rt(t)?;
    }
    if charge != 0 {
        // An ion's formula holds −charge electrons, each the electron gas's G.
        g += f64::from(charge) * gibbs_over_rt(&table::ELECTRON, t)?;
    }
    Some(-g / core::f64::consts::LN_10)
}

/// log₁₀ `K_f`, the equilibrium constant of gas row `id`'s formation at 1 bar from the elements in
/// their reference states, at `temperature` (the module's documentation: NASA's CEA records,
/// NIST-JANAF's reference states).
///
/// `K_f` is dimensionless, the pressures in bar: for `H2O`, `K_f` = p(H₂O) ÷ (p(H₂) p(O₂)^½). An
/// element's own reference has log₁₀ `K_f` = 0 (`H2`, `N2`, the noble gases, `e-`, and a metal's atom
/// above its switch to the gas).
///
/// `None` for a row with no record (not a gas row, or [`NOT_MODELLED`]'s), and outside
/// [`formation_range`]: nothing is extrapolated.
///
/// # Examples
///
/// Water forms strongly at Earth's temperatures and dissociates on an ultra-hot dayside:
///
/// ```
/// use hyperion_sim::substance::{by_key, log10_formation_constant};
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let water = by_key("H2O")?;
/// let at_1000 = log10_formation_constant(water, Kelvin::new(1_000.0))?;
/// assert!((at_1000 - 10.059).abs() < 1e-3); // NIST-JANAF's table H-064: 10.060
/// let at_5000 = log10_formation_constant(water, Kelvin::new(5_000.0))?;
/// assert!(at_5000 < 0.0);
/// // Below the records, nothing is extrapolated.
/// assert_eq!(log10_formation_constant(water, Kelvin::new(150.0)), None);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn log10_formation_constant(id: SubstanceId, temperature: Kelvin) -> Option<f64> {
    let row = id.substance();
    formation_constant(
        record(id)?,
        row.elements(),
        row.charge(),
        temperature.value(),
    )
}

/// The temperatures over which gas row `id` has a [`log10_formation_constant`], K, its bounds
/// included.
///
/// They are its record's range and its elements' reference states' together (and the electron
/// gas's, for an ion). `None` for a row with no record.
///
/// # Examples
///
/// ```
/// use hyperion_sim::substance::{by_key, formation_range};
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// // Sodium's ion needs its crystal's record, from 300 K, and the electron gas's.
/// let (lower, upper) = formation_range(by_key("Na+")?)?;
/// assert!((lower.value() - 300.0).abs() < 1e-12 && (upper.value() - 6_000.0).abs() < 1e-12);
/// // CEA holds no FeH.
/// assert_eq!(formation_range(by_key("FeH")?), None);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn formation_range(id: SubstanceId) -> Option<(Kelvin, Kelvin)> {
    let row = id.substance();
    formation_bounds(record(id)?, row.elements(), row.charge())
        .map(|(lower, upper)| (Kelvin::new(lower), Kelvin::new(upper)))
}

/// A balanced reaction among gas rows, Σ νᵢ Xᵢ = 0, each species with its stoichiometric
/// coefficient ν, negative for a reactant and positive for a product.
///
/// [`Reaction::new`] finds each species' record and checks the balance once, so
/// [`Reaction::log10_equilibrium_constant`] only evaluates. The species keep the order they are
/// given in, and that order is part of the output: the sum −Σ νᵢ Gᵢ ÷ RT is taken in it, and a
/// float sum's bits depend on its order, so a caller builds each reaction from a fixed, declared
/// order.
///
/// # Examples
///
/// Whether carbon is in methane or carbon monoxide: CO + 3 H₂ = CH₄ + H₂O, which favours methane
/// in cool, dense hydrogen and CO in hot (Heng and Tsai 2016 solve their C–H–O network on it):
///
/// ```
/// use hyperion_sim::substance::{BalanceReactionError, Element, Reaction, by_key};
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let [co, h2, ch4, h2o] = ["CO", "H2", "CH4", "H2O"].map(by_key);
/// let species = [(co?, -1), (h2?, -3), (ch4?, 1), (h2o?, 1)];
/// let methanation = Reaction::new(&species).ok()?;
/// assert!(methanation.log10_equilibrium_constant(Kelvin::new(500.0))? > 10.0);
/// assert!(methanation.log10_equilibrium_constant(Kelvin::new(1_500.0))? < -5.0);
/// // Without the water it balances neither hydrogen nor oxygen, and is refused when it is built,
/// // naming the first element in `Element::ALL`'s order.
/// assert_eq!(
///     Reaction::new(&species[..3]),
///     Err(BalanceReactionError::Unbalanced { element: Element::H })
/// );
/// # Some(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reaction {
    /// Each species, its ν, and its record's place in [`RECORDS`], in the order given. The place
    /// follows from the species and is a `u8`, not a `usize`, so that what the derived `Hash`
    /// writes of it has the same width on every target.
    terms: Vec<(SubstanceId, i32, u8)>,
}

impl Reaction {
    /// The reaction of `species`, each `(row, ν)`, in that order.
    ///
    /// A species may appear more than once, and an empty reaction is balanced (its K is 1).
    ///
    /// # Errors
    ///
    /// - [`BalanceReactionError::NoRecord`] for the first species, in order, that has no record:
    ///   a row that is not a gas row, or one of [`NOT_MODELLED`]'s.
    /// - [`BalanceReactionError::Unbalanced`] for the first element, in [`Element::ALL`]'s order,
    ///   that the reaction does not conserve.
    /// - [`BalanceReactionError::Charge`] if it conserves every element but not the charge.
    pub fn new(species: &[(SubstanceId, i32)]) -> Result<Self, BalanceReactionError> {
        let mut terms = Vec::with_capacity(species.len());
        for &(id, nu) in species {
            let record = record_index(id).ok_or(BalanceReactionError::NoRecord {
                key: id.substance().key(),
            })?;
            terms.push((id, nu, record));
        }
        // Each term is under 2³¹ × 2¹⁶ = 2⁴⁷ in size, and no slice holds the 2⁸⁰ terms that would
        // take the sum past 2¹²⁷, so the sums cannot overflow.
        let balance = |count: &dyn Fn(SubstanceId) -> i128| -> i128 {
            species
                .iter()
                .map(|&(id, nu)| i128::from(nu) * count(id))
                .sum()
        };
        for element in Element::ALL {
            let atoms = balance(&|id| {
                id.substance()
                    .elements()
                    .iter()
                    .find(|&&(e, _)| e == element)
                    .map_or(0, |&(_, n)| i128::from(n))
            });
            if atoms != 0 {
                return Err(BalanceReactionError::Unbalanced { element });
            }
        }
        if balance(&|id| i128::from(id.substance().charge())) != 0 {
            return Err(BalanceReactionError::Charge);
        }
        Ok(Self { terms })
    }

    /// Each species and its ν, in the order given.
    pub fn species(&self) -> impl ExactSizeIterator<Item = (SubstanceId, i32)> + '_ {
        self.terms.iter().map(|&(id, nu, _)| (id, nu))
    }

    /// log₁₀ K, the reaction's equilibrium constant at 1 bar at `temperature`: −Σ νᵢ Gᵢ ÷ (RT ln
    /// 10), from the species' own standard Gibbs energies, summed in the reaction's order.
    ///
    /// K is in bar to the power Σ νᵢ. It needs no reference state, so it holds wherever every
    /// species' record does, and equals Σ νᵢ log₁₀ `K_f,i` where those are defined. `None` if a
    /// species' record has no interval holding the temperature: nothing is extrapolated.
    #[must_use]
    pub fn log10_equilibrium_constant(&self, temperature: Kelvin) -> Option<f64> {
        let t = temperature.value();
        let mut sum = 0.0;
        for &(_, nu, record) in &self.terms {
            sum += f64::from(nu) * gibbs_over_rt(RECORDS[usize::from(record)].1, t)?;
        }
        Some(-sum / core::f64::consts::LN_10)
    }
}

/// A [`Reaction`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BalanceReactionError {
    /// A species has no record: it is not a gas row, or it is one of [`NOT_MODELLED`]'s.
    NoRecord {
        /// The row.
        key: SubstanceKey,
    },
    /// The reaction does not conserve an element.
    Unbalanced {
        /// The first element, in [`Element::ALL`]'s order, that it does not conserve.
        element: Element,
    },
    /// The reaction conserves every element but not the charge.
    Charge,
}

impl fmt::Display for BalanceReactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRecord { key } => write!(f, "{key} has no thermochemical record"),
            Self::Unbalanced { element } => {
                write!(f, "the reaction does not conserve {}", element.symbol())
            }
            Self::Charge => f.write_str("the reaction does not conserve charge"),
        }
    }
}

impl Error for BalanceReactionError {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::thermal::tests::{
        before_inline_tests, names, out_of_line_test_modules, rust_files,
    };
    use super::*;
    use crate::substance::{EQUILIBRIUM_HYDROGEN, by_key, heat_capacity_over_r_at, ids};

    /// The files outside this module and its table whose reads of its API sit behind a named
    /// hold, each with it: none yet. **P14.T52.a or P14.T54.b deletes this list** with the
    /// tripwire below.
    const HELD: [(&str, &str); 0] = [];

    /// The forms by which other code would read the formation constants or their table: the
    /// module's calls, its types and its path, and the table's path (never a bare `thermo`:
    /// `substance::thermo` is another module).
    const API_PATTERNS: [&str; 7] = [
        "log10_formation_constant(",
        "formation_range(",
        "log10_equilibrium_constant(",
        "Reaction",
        "BalanceReactionError",
        "substance::formation",
        "tables::thermo",
    ];

    /// The lines that declare this module and its table and re-export the module, each by its file
    /// and the start of its line: `substance/mod.rs`'s module line and its `pub use formation::…;`
    /// statement, whatever lines that takes, and `tables/mod.rs`'s module line and the table's
    /// entry in the `MANIFEST`, which `hyperion-fit` writes.
    const DECLARATIONS: [(&str, &str); 4] = [
        ("substance/mod.rs", "pub mod formation;"),
        ("substance/mod.rs", "pub use formation::"),
        ("tables/mod.rs", "pub mod thermo;"),
        ("tables/mod.rs", "name: \"thermo_equilibrium\""),
    ];

    /// Whether the file at `relative` (under the crate's `src/`) is this module's or its table.
    fn own(relative: &str) -> bool {
        relative == "substance/formation.rs" || relative == "tables/thermo.rs"
    }

    /// `line` without its text in backticks, an unclosed span's to the line's end: Rust's code holds
    /// no backtick, so that text is prose, a doc string's or a citation's (two of the condensed
    /// rows' citations name `substance::formation`, whose figures it reduced).
    fn outside_backticks(line: &str) -> String {
        line.split('`').step_by(2).collect()
    }

    /// Whether the code of `line` names `pattern` as a whole name: as `names` reads it, and, for a
    /// pattern that ends in an identifier's character, not as the start of a longer name, so that
    /// `Reaction` is not `Reactions`, nor `tables::thermo` `tables::thermo_equilibrium`. Its text
    /// in backticks is left out ([`outside_backticks`]).
    fn reads(line: &str, pattern: &str) -> bool {
        let code = outside_backticks(line);
        let continues = |c: char| c.is_alphanumeric() || c == '_';
        if !pattern.ends_with(continues) {
            return names(&code, pattern);
        }
        code.match_indices(pattern).any(|(at, _)| {
            code[..at].chars().next_back().is_none_or(|c| !continues(c))
                && code[at + pattern.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !continues(c))
        })
    }

    /// The `(line, pattern)` pairs, 1-based, by which the code of the file at `relative`, of text
    /// `text`, names [`API_PATTERNS`] before its inline test module: its comment lines and its
    /// [`DECLARATIONS`] (a statement to its `;`) left out, each pattern as a whole name (`reads`).
    fn readers(relative: &str, text: &str) -> Vec<(usize, &'static str)> {
        let mut found = Vec::new();
        let mut in_declaration = false;
        for (n, line) in before_inline_tests(text).iter().enumerate() {
            let code = line.trim_start();
            if in_declaration {
                in_declaration = !code.trim_end().ends_with(';');
                continue;
            }
            if DECLARATIONS
                .iter()
                .any(|(file, start)| *file == relative && code.starts_with(start))
            {
                in_declaration = !code.trim_end().ends_with(';') && !code.ends_with(',');
                continue;
            }
            if code.starts_with("//") {
                continue;
            }
            for pattern in API_PATTERNS {
                if reads(line, pattern) {
                    found.push((n + 1, pattern));
                }
            }
        }
        found
    }

    /// The scan reads the code alone: it finds a call, a path and a type, leaves out comments,
    /// prose in backticks, the declarations (a re-export over several lines whole) and the inline
    /// test module, and reads whole names only, at both ends.
    #[test]
    fn substance_formation_the_tripwire_s_scan_reads_the_code_alone() {
        let text = "// log10_formation_constant(id, t)\n\
                    let k = substance::formation::log10_formation_constant(id, t)?;\n\
                    let r: Reaction = build();\n\
                    let n = Reactions::new(); let m = tables::thermo_equilibrium::X;\n\
                    \"through `substance::formation`'s log K_f\",\n\
                    #[cfg(test)]\nmod tests {\n    fn f() { formation_range(id); }\n}\n";
        assert_eq!(
            readers("planetary/x.rs", text),
            [
                (2, "log10_formation_constant("),
                (2, "substance::formation"),
                (3, "Reaction")
            ]
        );
        let declarations = "pub mod formation;\npub use formation::{\n    BalanceReactionError, \
                            Reaction,\n};\npub use thermo::NasaPolynomial;\n\
                            fn f(e: BalanceReactionError) {}\n";
        assert_eq!(
            readers("substance/mod.rs", declarations),
            [(6, "BalanceReactionError")]
        );
        let tables = "pub mod thermo;\n    TableInfo {\n        name: \"thermo_equilibrium\",\n\
                      let t = crate::tables::thermo::C_GRAPHITE;\n";
        assert_eq!(readers("tables/mod.rs", tables), [(4, "tables::thermo")]);
        assert!(own("tables/thermo.rs") && !own("substance/thermo.rs"));
    }

    /// Nothing on the record reads the formation constants before P14.T52.a, their first reader,
    /// or P14.T54.b, which restamps their table (the module's remarks; decision-bump-plan-after-22
    /// §1.3): no code of the crate's `src/` outside this module and its table names
    /// [`API_PATTERNS`], every file read in sorted order by `substance::thermal`'s tripwire's walk,
    /// but each inline test module, every file an out-of-line test module declares,
    /// [`DECLARATIONS`] and [`HELD`]'s files (none). **P14.T52.a or P14.T54.b deletes this
    /// tripwire** when the speciation first reads them.
    #[test]
    fn substance_formation_nothing_on_the_record_reads_the_constants_yet() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&src, &src, &mut files);
        assert!(files.len() > 100, "the walk found the crate's sources");
        let read = |path: &Path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        };
        assert_eq!(
            files.iter().filter(|(relative, _)| own(relative)).count(),
            2,
            "the module and its table"
        );
        for (held, hold) in HELD {
            assert!(!hold.is_empty(), "{held} names its hold");
            assert!(
                files.iter().any(|(relative, _)| relative == held),
                "{held} is a source file"
            );
        }
        let mut tests = Vec::new();
        for (relative, path) in &files {
            tests.extend(out_of_line_test_modules(relative, &read(path)));
        }
        let mut found = Vec::new();
        for (relative, path) in &files {
            let test_file = tests
                .iter()
                .any(|t| relative == t || (t.ends_with('/') && relative.starts_with(t.as_str())));
            if test_file || own(relative) || HELD.iter().any(|(held, _)| held == relative) {
                continue;
            }
            for (line, pattern) in readers(relative, &read(path)) {
                found.push(format!("{relative}:{line}: {pattern}"));
            }
        }
        assert!(
            found.is_empty(),
            "nothing on the record reads substance::formation before P14.T52.a: {found:#?}"
        );
    }

    /// The row whose key is `key`.
    fn row(key: &str) -> SubstanceId {
        by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
    }

    /// log₁₀ `K_f` of `key` at `t` kelvin.
    fn kf(key: &str, t: f64) -> Option<f64> {
        log10_formation_constant(row(key), Kelvin::new(t))
    }

    /// Each gas row's log₁₀ `K_f` at three temperatures, K: CEA's records evaluated with
    /// NIST-JANAF's reference states, by an independent Python reading of the library, the
    /// dataset's own scripts (`crates/hyperion-fit/data/nasa_cea_thermo/`, 2026-10-19), run there
    /// as `python3 -I asserted.py kf.py cea.py thermo.inp`. `kf.py` parses the file's columns
    /// itself (`cea.py`), evaluates McBride, Zehe and Gordon's eqs. (2) and (3) in Python's `math`,
    /// and takes the reference states as this module does, its switches by its own bisection.
    /// The raw file, `data/thermo.inp` of NASA's `nasa/cea` at
    /// 2f79a647e85737742fb08a127da00d1ea1d069da (1,234,323 bytes, SHA-256
    /// fa7746572952d74e249e818a82a35c113829742fb421a308e167185528884363), gives the same output
    /// byte for byte. A metal's atom has `K_f` = 1 by definition above its switch, so it is taken
    /// below it: at 500 K, at its switch rounded down to 100 K, and midway; every other row at
    /// 500, 2,500 and 5,000 K.
    const CEA: [(&str, [f64; 3], [f64; 3]); 57] = [
        ("H2", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        ("He", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        (
            "H2O",
            [500.0, 2_500.0, 5_000.0],
            [
                22.883_071_234_206_078,
                2.226_332_234_640_565,
                -0.408_044_855_820_761_1,
            ],
        ),
        (
            "CH4",
            [500.0, 2_500.0, 5_000.0],
            [
                3.398_735_003_569_713_7,
                -3.808_617_609_331_434,
                -4.521_696_064_945_759,
            ],
        ),
        (
            "NH3",
            [500.0, 2_500.0, 5_000.0],
            [
                -0.497_713_802_569_978_7,
                -4.979_785_043_496_54,
                -5.495_290_229_253_544,
            ],
        ),
        ("N2", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        ("O2", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        (
            "CO2",
            [500.0, 2_500.0, 5_000.0],
            [
                41.256_516_219_717_8,
                8.274_173_346_692_1,
                4.064_627_140_695_035,
            ],
        ),
        ("Ar", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        ("Ne", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        ("Kr", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        ("Xe", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        (
            "N2O",
            [500.0, 2_500.0, 5_000.0],
            [
                -12.416_588_048_469_562,
                -5.449_959_292_746_669,
                -4.426_625_511_935_621_5,
            ],
        ),
        (
            "CO",
            [500.0, 2_500.0, 5_000.0],
            [
                16.237_023_030_282_447,
                6.839_840_129_596_356,
                5.456_573_728_258_677,
            ],
        ),
        (
            "SO2",
            [500.0, 2_500.0, 5_000.0],
            [
                31.426_582_423_468_19,
                3.741_908_142_658_861,
                -0.055_557_659_689_498_676,
            ],
        ),
        (
            "H2S",
            [500.0, 2_500.0, 5_000.0],
            [
                4.210_083_977_278_114,
                -0.681_651_494_361_872_1,
                -1.612_336_755_525_550_3,
            ],
        ),
        (
            "O3",
            [500.0, 2_500.0, 5_000.0],
            [
                -18.417_731_886_519_37,
                -6.436_852_802_716_316_5,
                -4.492_478_158_953_208,
            ],
        ),
        (
            "HCN",
            [500.0, 2_500.0, 5_000.0],
            [
                -12.087_675_751_747_447,
                -1.108_483_782_787_864_2,
                0.200_851_930_257_723_85,
            ],
        ),
        (
            "C2H2",
            [500.0, 2_500.0, 5_000.0],
            [
                -20.782_873_426_848_266,
                -1.963_879_763_949_441_4,
                0.279_566_005_308_061_97,
            ],
        ),
        (
            "C2H4",
            [500.0, 2_500.0, 5_000.0],
            [
                -8.458_223_309_192_269,
                -5.095_121_506_732_418,
                -4.755_182_458_050_378,
            ],
        ),
        (
            "C2H6",
            [500.0, 2_500.0, 5_000.0],
            [
                -0.628_096_743_698_987_2,
                -9.111_415_866_875_781,
                -10.152_222_596_789_779,
            ],
        ),
        (
            "PH3",
            [500.0, 2_500.0, 5_000.0],
            [
                -2.185_084_221_015_696_7,
                -4.116_718_908_217_568,
                -4.751_108_378_302_725,
            ],
        ),
        (
            "CH3OH",
            [500.0, 2_500.0, 5_000.0],
            [
                14.001_158_088_069_994,
                -3.930_822_745_455_732_3,
                -6.214_681_685_501_372,
            ],
        ),
        (
            "H",
            [500.0, 2_500.0, 5_000.0],
            [
                -20.157_464_655_210_07,
                -1.598_858_851_888_444_1,
                0.808_898_422_924_685,
            ],
        ),
        (
            "O",
            [500.0, 2_500.0, 5_000.0],
            [
                -22.935_681_113_413_51,
                -1.839_178_076_920_208_6,
                0.845_038_383_512_137_8,
            ],
        ),
        ("e-", [500.0, 2_500.0, 5_000.0], [0.0, 0.0, 0.0]),
        (
            "H-",
            [500.0, 2_500.0, 5_000.0],
            [
                -13.427_499_640_629_565,
                -2.697_955_069_725_246,
                -1.802_977_310_015_305_7,
            ],
        ),
        (
            "Na",
            [500.0, 800.0, 1_100.0],
            [
                -6.030_473_612_969_557_5,
                -2.044_588_153_488_092_5,
                -0.278_709_773_521_673_6,
            ],
        ),
        (
            "K",
            [500.0, 750.0, 1_000.0],
            [
                -4.480_293_361_731_846,
                -1.571_503_028_113_761,
                -0.157_838_445_920_522_26,
            ],
        ),
        (
            "Fe",
            [500.0, 1_800.0, 3_100.0],
            [
                -35.407_499_071_279_27,
                -4.528_675_734_976_92,
                -0.072_173_596_231_499_99,
            ],
        ),
        (
            "Mg",
            [500.0, 900.0, 1_300.0],
            [
                -9.331_118_137_022_024,
                -2.609_141_537_745_743_6,
                -0.242_576_384_667_587_32,
            ],
        ),
        (
            "Si",
            [500.0, 2_000.0, 3_500.0],
            [
                -39.215_377_638_947_61,
                -4.361_298_137_216_492,
                -0.015_366_662_756_285_666,
            ],
        ),
        (
            "Ca",
            [500.0, 1_100.0, 1_700.0],
            [
                -12.739_900_189_735_597,
                -2.828_564_827_256_054_4,
                -0.202_132_030_182_299_86,
            ],
        ),
        (
            "Ti",
            [500.0, 2_050.0, 3_600.0],
            [
                -41.613_330_919_041_31,
                -4.706_537_339_884_472_5,
                -0.056_501_438_231_830_195,
            ],
        ),
        (
            "SiO",
            [500.0, 2_500.0, 5_000.0],
            [
                15.004_602_856_629_518,
                5.797_654_556_040_739,
                2.257_972_891_593_204,
            ],
        ),
        (
            "TiO",
            [500.0, 2_500.0, 5_000.0],
            [
                0.001_462_430_565_892_361_6,
                3.297_049_818_843_102_6,
                1.429_172_136_887_748,
            ],
        ),
        (
            "VO",
            [500.0, 2_500.0, 5_000.0],
            [
                -10.383_584_855_816_183,
                1.331_832_488_750_566_8,
                0.493_982_662_212_617_3,
            ],
        ),
        (
            "Al",
            [500.0, 1_600.0, 2_700.0],
            [
                -27.380_161_545_579_33,
                -4.218_474_693_908_547,
                -0.198_937_526_485_078_45,
            ],
        ),
        (
            "V",
            [500.0, 2_100.0, 3_700.0],
            [
                -46.029_721_301_607_616,
                -5.055_394_561_992_614,
                -0.008_392_356_206_385_588,
            ],
        ),
        (
            "Mn",
            [500.0, 1_400.0, 2_300.0],
            [
                -22.140_362_769_249_364,
                -3.583_007_225_515_586_5,
                -0.055_178_701_895_205_79,
            ],
        ),
        (
            "Ni",
            [500.0, 1_800.0, 3_100.0],
            [
                -36.999_585_605_736_27,
                -4.876_839_382_384_356,
                -0.120_122_000_643_201_55,
            ],
        ),
        (
            "Zn",
            [500.0, 800.0, 1_100.0],
            [
                -7.418_130_562_634_098,
                -2.466_432_666_627_114_6,
                -0.367_407_187_124_576_5,
            ],
        ),
        (
            "Na+",
            [500.0, 2_500.0, 5_000.0],
            [
                -57.559_430_295_158_656,
                -8.342_084_218_801_334,
                -2.420_877_383_363_405,
            ],
        ),
        (
            "Mg+",
            [500.0, 2_500.0, 5_000.0],
            [
                -85.528_920_679_817_43,
                -12.794_204_276_111_362,
                -4.341_751_444_648_932,
            ],
        ),
        (
            "Al+",
            [500.0, 2_500.0, 5_000.0],
            [
                -87.832_297_014_989_21,
                -11.165_042_248_694_908,
                -3.731_189_538_594_244_4,
            ],
        ),
        (
            "Si+",
            [500.0, 2_500.0, 5_000.0],
            [
                -121.015_414_615_556_22,
                -16.630_759_050_177_005,
                -5.366_064_265_207_734,
            ],
        ),
        (
            "K+",
            [500.0, 2_500.0, 5_000.0],
            [
                -47.961_613_783_789_915,
                -6.733_492_293_927_545,
                -1.645_495_138_739_310_4,
            ],
        ),
        (
            "Ca+",
            [500.0, 2_500.0, 5_000.0],
            [
                -73.485_066_937_111_13,
                -9.703_177_244_083_918,
                -2.815_883_589_912_752,
            ],
        ),
        (
            "Ti+",
            [500.0, 2_500.0, 5_000.0],
            [
                -109.638_417_455_751_28,
                -13.835_317_110_761_281,
                -3.528_025_586_258_292,
            ],
        ),
        (
            "V+",
            [500.0, 2_500.0, 5_000.0],
            [
                -113.349_972_104_264_89,
                -14.442_482_944_478_51,
                -3.762_426_726_200_875,
            ],
        ),
        (
            "Cr+",
            [500.0, 2_500.0, 5_000.0],
            [
                -101.372_858_473_577_24,
                -12.499_017_024_899_656,
                -3.914_655_533_661_256,
            ],
        ),
        (
            "Mn+",
            [500.0, 2_500.0, 5_000.0],
            [
                -96.433_215_634_726_95,
                -12.598_852_523_229_318,
                -4.339_192_752_197_858,
            ],
        ),
        (
            "Fe+",
            [500.0, 2_500.0, 5_000.0],
            [
                -114.422_620_097_597_57,
                -14.943_854_228_472_697,
                -4.699_985_663_901_195,
            ],
        ),
        (
            "Ni+",
            [500.0, 2_500.0, 5_000.0],
            [
                -113.756_840_841_946_51,
                -15.248_287_647_125_858,
                -5.082_322_667_291_325,
            ],
        ),
        (
            "Zn+",
            [500.0, 2_500.0, 5_000.0],
            [
                -101.234_550_276_133_32,
                -16.317_902_012_966_698,
                -6.096_199_755_163_265,
            ],
        ),
        (
            "HCl",
            [500.0, 2_500.0, 5_000.0],
            [
                10.150_420_978_927_002,
                2.285_266_555_319_879_4,
                1.264_933_800_465_030_6,
            ],
        ),
        (
            "Cr",
            [500.0, 1_700.0, 2_900.0],
            [
                -33.674_243_753_674_39,
                -4.681_203_731_968_139,
                -0.107_440_118_547_725_06,
            ],
        ),
    ];

    /// Three values per species to 10⁻⁴ in log K (the plan's test): CEA's records as the
    /// independent Python evaluation gives them. Measured, every value is the Python's to the bit.
    #[test]
    fn substance_formation_reproduces_the_cea_records() {
        let mut worst = 0.0_f64;
        for (key, temperatures, values) in CEA {
            for (t, expected) in temperatures.into_iter().zip(values) {
                let actual = kf(key, t).unwrap_or_else(|| panic!("{key} at {t} K"));
                assert!(
                    (actual - expected).abs() < 1e-4,
                    "{key} at {t} K: {actual} against the records' {expected}"
                );
                worst = worst.max((actual - expected).abs());
            }
        }
        assert!(worst < 1e-10, "{worst}");
        // A metal's three values lie below its switch, where its K_f is not 1 by definition.
        for element in Element::ALL {
            if let Reference::Condensed {
                boiling_k, atoms, ..
            } = Reference::of(element)
                && atoms < 1.5
            {
                let (_, temperatures, values) = CEA
                    .iter()
                    .find(|(key, ..)| *key == element.symbol())
                    .unwrap_or_else(|| panic!("{element:?}'s atom is asserted"));
                assert!(temperatures.iter().all(|&t| t < boiling_k), "{element:?}");
                assert!(values.iter().all(|&v| v < 0.0), "{element:?}");
            }
        }
    }

    /// The gas rows P14.T52.a and P14.T52.b name: the C–H–O–N network of Heng and Tsai 2016 and
    /// H₂S, T52.a's trace metals as neutral atoms and first ions, H, H⁻ and e⁻, T52.b's quenched and
    /// photolysed gases, and T52.d's oxides and hydride.
    const NAMED: [&str; 51] = [
        "H2", "He", "H2O", "CH4", "CO", "CO2", "NH3", "N2", "HCN", "C2H2", "C2H4", "H2S", "SO2",
        "H", "H-", "e-", "Na", "K", "Ti", "V", "Fe", "Mg", "Si", "Ca", "Al", "P", "Cl", "Cr", "Mn",
        "Ni", "Zn", "Na+", "K+", "Ti+", "V+", "Fe+", "Mg+", "Si+", "Ca+", "Al+", "P+", "Cl+",
        "Cr+", "Mn+", "Ni+", "Zn+", "TiO", "VO", "SiO", "FeH", "OH",
    ];

    /// The species T52.a may want that are not registry rows: CEA holds each, and a row added
    /// later takes its record by a refit.
    const NOT_ROWS: [&str; 5] = ["P", "Cl", "P+", "Cl+", "OH"];

    /// Every gas row that T52.a or T52.b names has an entry, but `FeH`, which CEA does not hold (and
    /// the five species that are not rows); and every gas row of the registry has one but `FeH`.
    #[test]
    fn substance_formation_every_gas_row_of_the_chemistry_has_an_entry() {
        for key in NAMED {
            match by_key(key) {
                None => assert!(NOT_ROWS.contains(&key), "{key} is a row"),
                Some(id) => {
                    assert!(id.substance().gas().is_some(), "{key} is a gas row");
                    let modelled = formation_range(id).is_some();
                    assert_eq!(modelled, key != "FeH", "{key}");
                }
            }
        }
        for key in NOT_ROWS {
            assert_eq!(by_key(key), None, "{key}");
        }
        let gas_rows: Vec<String> = ids()
            .filter(|id| id.substance().gas().is_some())
            .map(|id| id.substance().key().as_str().to_owned())
            .collect();
        let entries: Vec<&str> = RECORDS.iter().map(|&(k, _)| k).collect();
        let modelled: Vec<&str> = gas_rows
            .iter()
            .map(String::as_str)
            .filter(|k| NOT_MODELLED.iter().all(|(n, _)| n != k))
            .collect();
        assert_eq!(
            entries, modelled,
            "the records are the gas rows in registry order"
        );
        assert_eq!(NOT_MODELLED.map(|(k, _)| k), ["FeH"]);
        assert!(gas_rows.iter().any(|k| k == "FeH"));
        assert_eq!(
            log10_formation_constant(row("FeH"), Kelvin::new(1_000.0)),
            None
        );
        assert_eq!(
            log10_formation_constant(row("graphite"), Kelvin::new(1_000.0)),
            None
        );
    }

    /// Nothing is extrapolated: each row has a value at both ends of its range, and none just
    /// below or above it, nor at a temperature that is not positive and finite.
    #[test]
    fn substance_formation_is_never_extrapolated() {
        for &(key, _) in &RECORDS {
            let (lower, upper) = formation_range(row(key)).unwrap();
            let (lower, upper) = (lower.value(), upper.value());
            assert!(
                lower >= 200.0 && upper <= 6_000.0,
                "{key}: {lower}–{upper} K"
            );
            assert!(
                kf(key, lower).is_some() && kf(key, upper).is_some(),
                "{key}"
            );
            assert_eq!(kf(key, lower - 1e-6), None, "{key} below {lower} K");
            assert_eq!(kf(key, upper + 1e-6), None, "{key} above {upper} K");
            for t in [0.0, -300.0, f64::NAN, f64::INFINITY] {
                assert_eq!(kf(key, t), None, "{key} at {t}");
            }
        }
        let range = |key| {
            let (l, u) = formation_range(row(key)).unwrap();
            [l.value(), u.value()]
        };
        assert_eq!(range("H2O"), [200.0, 6_000.0]);
        assert_eq!(range("CH4"), [300.0, 6_000.0]);
        assert_eq!(range("Mg+"), [298.15, 6_000.0]);
        assert_eq!(range("Na+"), [300.0, 6_000.0]);
        let water = Reaction::new(&[(row("H2"), -2), (row("O2"), -1), (row("H2O"), 2)]).unwrap();
        assert!(
            water
                .log10_equilibrium_constant(Kelvin::new(6_000.0))
                .is_some()
        );
        assert_eq!(
            water.log10_equilibrium_constant(Kelvin::new(6_000.0 + 1e-9)),
            None
        );
    }

    /// The references hold log₁₀ `K_f` = 0 exactly, a metal's atom from its switch to the gas; each
    /// condensed reference meets its gas form at the switch to 10⁻⁹ in G ÷ RT, so `K_f` is
    /// continuous through it; and each switch is CEA's crossing, within 15 K of NIST-JANAF's.
    #[test]
    fn substance_formation_the_reference_states_are_continuous() {
        for key in ["H2", "N2", "O2", "He", "Ne", "Ar", "Kr", "Xe", "e-"] {
            for t in [300.0, 1_000.0, 6_000.0] {
                assert_eq!(kf(key, t), Some(0.0), "{key} at {t} K");
            }
        }
        for element in Element::ALL {
            let Reference::Condensed {
                phases,
                boiling_k,
                gas,
                atoms,
            } = Reference::of(element)
            else {
                continue;
            };
            let miss = gibbs_over_rt(phases, boiling_k).unwrap()
                - gibbs_over_rt(gas, boiling_k).unwrap() / atoms;
            assert!(miss.abs() < 1e-9, "{element:?}: {miss}");
            let (_, upper) = Reference::of(element).bounds();
            assert!((upper - 6_000.0).abs() < 1e-9, "{element:?}");
            if atoms < 1.5 {
                let key = element.symbol();
                assert_eq!(kf(key, boiling_k + 1e-9), Some(0.0), "{key}");
                assert!(kf(key, boiling_k).unwrap().abs() < 1e-9, "{key}");
            }
        }
        // Sulphur's switch is to S₂ where its fugacity reaches 1 bar (JANAF's table S-001,
        // 882.117 K), phosphorus's to P₂ (P-001, 1,180.008 K).
        assert!((table::S_BOILING_K - 882.117).abs() < 1.0);
        assert!((table::P_BOILING_K - 1_180.008).abs() < 1.0);
    }

    /// The fit bisects each switch with its own G ÷ RT and the sim branches on it with this
    /// module's: by this module's evaluation, each condensed reference's miss (its condensed G per
    /// atom less its gas form's) is negative at its `_BOILING_K` and not negative at the next
    /// `f64` above, so the two agree on which side of the switch every temperature lies.
    #[test]
    fn substance_formation_each_switch_is_the_last_condensed_temperature() {
        let mut condensed = 0;
        for element in Element::ALL {
            let Reference::Condensed {
                phases,
                boiling_k,
                gas,
                atoms,
            } = Reference::of(element)
            else {
                continue;
            };
            let miss =
                |t: f64| gibbs_over_rt(phases, t).unwrap() - gibbs_over_rt(gas, t).unwrap() / atoms;
            assert!(miss(boiling_k) < 0.0, "{element:?} at {boiling_k} K");
            assert!(
                miss(boiling_k.next_up()) >= 0.0,
                "{element:?} above {boiling_k} K"
            );
            condensed += 1;
        }
        assert_eq!(condensed, 15);
    }

    /// A few NIST-JANAF values (Chase 1998, NIST SRD 13; read at janaf.nist.gov on 2026-10-19), as
    /// decision-composition §5 allows, each within 3 × 10⁻³ of CEA's records': water's log₁₀
    /// `K_f`, table H-064, 40.047 at 298.15 K (the records' 40.0453, −0.0017) and 10.060 at 1,000 K
    /// (10.0592, −0.0008); carbon monoxide's, C-093, 10.461 at 1,000 K (10.4619, +0.0009);
    /// methane's, C-067, −1.018 at 1,000 K (−1.0178, +0.0002): the two evaluations differ by up to
    /// 1.7 × 10⁻³, beyond the table's 10⁻³ digits, so JANAF is a cross-check and the records are
    /// asserted to 10⁻⁴ (main, 2026-10-19).
    #[test]
    fn substance_formation_meets_nist_janaf() {
        for (key, t, janaf) in [
            ("H2O", 298.15, 40.047),
            ("H2O", 1_000.0, 10.060),
            ("CO", 1_000.0, 10.461),
            ("CH4", 1_000.0, -1.018),
        ] {
            let ours = kf(key, t).unwrap();
            assert!(
                (ours - janaf).abs() < 3e-3,
                "{key} at {t} K: {ours} against JANAF's {janaf}"
            );
        }
    }

    /// The species of a cross-check that are not rows, from the table: (record, elements).
    fn cross(name: &str) -> (&'static [Interval], &'static [(Element, u16)]) {
        match name {
            "SO3" => (&table::SO3, &[(Element::S, 1), (Element::O, 3)]),
            "C" => (&table::C, &[(Element::C, 1)]),
            "C2" => (&table::C2, &[(Element::C, 2)]),
            "C3" => (&table::C3, &[(Element::C, 3)]),
            "C4" => (&table::C4, &[(Element::C, 4)]),
            "C5" => (&table::C5, &[(Element::C, 5)]),
            "MgO" => (&table::MGO_CRYSTAL, &[(Element::Mg, 1), (Element::O, 1)]),
            "MgSO4" => (
                &table::MGSO4_CRYSTAL,
                &[(Element::Mg, 1), (Element::S, 1), (Element::O, 4)],
            ),
            _ => panic!("{name} is a cross-check's species"),
        }
    }

    /// log₁₀ `K_f` of cross-check species `name` at `t` kelvin.
    fn cross_kf(name: &str, t: f64) -> f64 {
        let (record, elements) = cross(name);
        formation_constant(record, elements, 0, t).unwrap_or_else(|| panic!("{name} at {t} K"))
    }

    /// The bisection's root of `f`, negative at `below` and positive at `above`.
    fn root(f: impl Fn(f64) -> f64, mut below: f64, mut above: f64) -> f64 {
        assert!(f(below) < 0.0 && f(above) > 0.0);
        for _ in 0..200 {
            let mid = f64::midpoint(below, above);
            if mid <= below || mid >= above {
                break;
            }
            if f(mid) < 0.0 {
                below = mid;
            } else {
                above = mid;
            }
        }
        f64::midpoint(below, above)
    }

    /// The registry's two transitions that were reduced from NIST-JANAF, recomputed from this
    /// module's CEA-based log `K_f` by the rows' own arithmetic (decision-p14-t49b-condensed
    /// §3.1(5)), the roots found by bisection rather than between 100 K rows:
    ///
    /// - `MgSO4`'s open-system decomposition at 1 bar in all, 1,375.44 K (the Python evaluation's
    ///   1,375.441; interpolated linearly in 1 ÷ T between 1,300 and 1,400 K, 1,375.51), against
    ///   JANAF's 1,272 K, 103 K below: the row now holds 1,375 K;
    /// - graphite's summed C₁–C₅ vapour at 1 bar, 3,973.32 K (3,973.324; C₃ 0.72 bar of it), against
    ///   JANAF's 4,080 K, 107 K above: `graphite` and `soot` now hold 3,973 K.
    #[test]
    fn substance_formation_the_registry_s_transitions_meet_cea() {
        let mgso4 = root(
            |t| {
                let solids = cross_kf("MgO", t) - cross_kf("MgSO4", t);
                let p_so3 = math::exp10(solids + cross_kf("SO3", t));
                let log_k2 = solids + kf("SO2", t).unwrap();
                let p_so2 = math::exp10((0.5 * math::log10(2.0) + log_k2) * 2.0 / 3.0);
                math::log10(p_so3 + 1.5 * p_so2)
            },
            1_000.0,
            1_400.0,
        );
        let graphite = root(
            |t| {
                let sum: f64 = ["C", "C2", "C3", "C4", "C5"]
                    .into_iter()
                    .map(|n| math::exp10(cross_kf(n, t)))
                    .sum();
                math::log10(sum)
            },
            3_500.0,
            4_500.0,
        );
        assert!((mgso4 - 1_375.441_310_241).abs() < 1e-6, "{mgso4}");
        assert!((graphite - 3_973.323_838_499).abs() < 1e-6, "{graphite}");
        let transition = |key: &str| {
            row(key)
                .substance()
                .condensed()
                .unwrap()
                .transition()
                .temperature()
                .value()
        };
        assert!((transition("MgSO4") - mgso4).abs() < 10.0);
        assert!((transition("graphite") - graphite).abs() < 10.0);
        assert!((transition("soot") - graphite).abs() < 10.0);
    }

    /// The reactions [`substance_formation_an_equilibrium_constant_sums_its_formation_constants`]
    /// takes, which `tests/formation_golden.rs` repeats: methanation, ammonia's synthesis,
    /// sodium's ionisation (Saha's equilibrium Na = Na⁺ + e⁻) and H₂S's oxidation by water.
    const REACTIONS: [&[(&str, i32)]; 4] = [
        &[("CO", -1), ("H2", -3), ("CH4", 1), ("H2O", 1)],
        &[("N2", -1), ("H2", -3), ("NH3", 2)],
        &[("Na", -1), ("Na+", 1), ("e-", 1)],
        &[("H2S", -1), ("H2O", -2), ("SO2", 1), ("H2", 3)],
    ];

    /// The reaction of `species`, by key.
    fn reaction(species: &[(&str, i32)]) -> Result<Reaction, BalanceReactionError> {
        let ids: Vec<(SubstanceId, i32)> = species.iter().map(|&(k, n)| (row(k), n)).collect();
        Reaction::new(&ids)
    }

    /// An equilibrium constant is its species' formation constants' sum where those are defined,
    /// to rounding; its electrons balance an ionisation; and an unbalanced reaction, or a row with
    /// no record, is refused when it is built, each by its cause.
    #[test]
    fn substance_formation_an_equilibrium_constant_sums_its_formation_constants() {
        for species in REACTIONS {
            let built = reaction(species).unwrap();
            assert!(
                built
                    .species()
                    .map(|(id, nu)| (id.substance().key().as_str().to_owned(), nu))
                    .eq(species.iter().map(|&(k, n)| (k.to_owned(), n))),
                "{species:?} keeps its order"
            );
            for t in [500.0, 1_500.0, 3_000.0, 5_000.0] {
                let k = built.log10_equilibrium_constant(Kelvin::new(t)).unwrap();
                let sum: f64 = species
                    .iter()
                    .map(|&(key, nu)| f64::from(nu) * kf(key, t).unwrap())
                    .sum();
                assert!(
                    (k - sum).abs() < 1e-9 * (1.0 + k.abs()),
                    "{species:?} at {t} K"
                );
            }
        }
        assert_eq!(
            reaction(&[("H2", -1), ("H", 1)]),
            Err(BalanceReactionError::Unbalanced {
                element: Element::H
            })
        );
        assert_eq!(
            reaction(&[("Na", -1), ("Na+", 1)]),
            Err(BalanceReactionError::Charge)
        );
        let fe_h = reaction(&[("Fe", -1), ("H", -1), ("FeH", 1)]);
        assert_eq!(
            fe_h,
            Err(BalanceReactionError::NoRecord {
                key: row("FeH").substance().key()
            })
        );
        assert_eq!(
            fe_h.unwrap_err().to_string(),
            "FeH has no thermochemical record"
        );
        let graphite = reaction(&[("graphite", -1), ("O2", -1), ("CO2", 1)]);
        assert!(matches!(
            graphite,
            Err(BalanceReactionError::NoRecord { .. })
        ));
        let empty = Reaction::new(&[]).unwrap();
        assert_eq!(
            empty.log10_equilibrium_constant(Kelvin::new(1_000.0)),
            Some(0.0)
        );
        // Below 300 K CH₄'s formation has no constant (graphite's record starts there), but its
        // combustion among gases does, down to the gases' own 200 K: 167.42 at 250 K.
        let combustion = reaction(&[("CH4", -1), ("O2", -2), ("CO2", 1), ("H2O", 2)]).unwrap();
        let burnt = combustion
            .log10_equilibrium_constant(Kelvin::new(250.0))
            .unwrap();
        assert!((burnt - 167.417_334).abs() < 1e-5, "{burnt}");
        let oxygen = reaction(&[("O2", -1), ("O", 2)]).unwrap();
        assert!(
            oxygen
                .log10_equilibrium_constant(Kelvin::new(250.0))
                .is_some()
        );
        assert_eq!(kf("CH4", 250.0), None);
        for t in [0.0, -300.0, f64::NAN, f64::INFINITY] {
            assert_eq!(oxygen.log10_equilibrium_constant(Kelvin::new(t)), None);
        }
    }

    /// T49.b's statistical sums and CEA's polynomials, two sources of `c_p`, agree on the table's
    /// own records within T49.b's oracle tolerance (`the_heat_capacities_follow_nasa_glenn`): N₂,
    /// CO₂ and H₂O, which the ruling holds closest, within 1 per cent to 1,000 K; CH₄ and NH₃
    /// within 2 per cent to 700 K and 3 per cent at 1,000 K; H₂ by its equilibrium modification,
    /// CEA's (science-r08-gases-2.md §1.5).
    #[test]
    fn substance_formation_the_two_heat_capacities_agree() {
        let cea = |key: &str, t: f64| {
            let &[_, _, a1, a2, a3, a4, a5, a6, a7, _, _] = record(row(key))
                .unwrap()
                .iter()
                .find(|i| i[0] <= t && t <= i[1])
                .unwrap();
            a1 / (t * t) + a2 / t + a3 + a4 * t + a5 * t * t + a6 * t * t * t + a7 * t * t * t * t
        };
        for (key, close) in [
            ("N2", true),
            ("CO2", true),
            ("H2O", true),
            ("CH4", false),
            ("NH3", false),
        ] {
            for t in [300.0, 700.0, 1_000.0] {
                let within = match (close, t) {
                    (true, _) => 0.01,
                    (false, t) if t <= 700.0 => 0.02,
                    (false, _) => 0.03,
                };
                let sum = heat_capacity_over_r_at(row(key), Kelvin::new(t)).unwrap();
                let expected = cea(key, t);
                assert!(
                    (sum / expected - 1.0).abs() < within,
                    "{key} at {t} K: the sum's {sum} against CEA's {expected}"
                );
            }
        }
        for t in [300.0, 700.0, 1_000.0] {
            let sum = EQUILIBRIUM_HYDROGEN
                .heat_capacity_over_r(Kelvin::new(t))
                .unwrap();
            assert!((sum / cea("H2", t) - 1.0).abs() < 0.01, "H2 at {t} K");
        }
    }
}
