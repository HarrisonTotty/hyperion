//! The gases' thermochemistry (plan 14, P14.T49.d): NASA's CEA records of every substance-registry
//! gas row and of the elements' reference states, re-emitted for
//! `hyperion_sim::substance::formation`, which gives each row's equilibrium constant of formation.
//!
//! # The data
//!
//! NASA Glenn's 9-coefficient records (McBride, Zehe and Gordon 2002, NASA/TP-2002-211556; the
//! record layout is their Appendix C, Table C1, p. 43) in `data/thermo.inp` of NASA's CEA, the
//! dataset `nasa_cea_thermo`: the subset its `reduce.py` extracts verbatim, under Apache 2.0
//! (decision-composition §5). Each interval of a record holds a₁…a₇ of
//!
//! ```text
//! c_p ÷ R = a₁ T⁻² + a₂ T⁻¹ + a₃ + a₄ T + a₅ T² + a₆ T³ + a₇ T⁴
//! H ÷ RT  = −a₁ T⁻² + a₂ ln(T) ÷ T + a₃ + a₄ T ÷ 2 + a₅ T² ÷ 3 + a₆ T³ ÷ 4 + a₇ T⁴ ÷ 5 + b₁ ÷ T
//! S ÷ R   = −a₁ T⁻² ÷ 2 − a₂ T⁻¹ + a₃ ln T + a₄ T + a₅ T² ÷ 2 + a₆ T³ ÷ 3 + a₇ T⁴ ÷ 4 + b₂
//! ```
//!
//! (their eqs. (1)–(3), p. 2), with H the assigned enthalpy (zero for each element's reference
//! state at 298.15 K) and S the entropy at 1 bar, so G ÷ RT = H ÷ RT − S ÷ R.
//!
//! # Why the polynomials are re-emitted, not fitted
//!
//! The plan offers Chebyshev coefficients or the NASA polynomials re-emitted. The polynomials are
//! re-emitted: they reproduce the source exactly, where any refit would add its own error, and a
//! Chebyshev fit would be piecewise anyway, at the records' own interval bounds (1,000 K for most
//! gases) and at every phase change of an element's reference state (iron's four solids and its
//! liquid). The table therefore holds each record's intervals as `[T_lo, T_hi, a₁, …, a₇, b₁, b₂]`,
//! the coefficients being the file's decimals read to the nearest `f64`.
//!
//! - **The domain stops at 6,000 K** ([`UPPER_TEMPERATURE_K`]), the plan's "100–6,000 K where the
//!   data reach": an interval from 6,000 to 20,000 K is not emitted. Below, the records start at
//!   200, 298.15 or 300 K, and nothing is extrapolated below them.
//! - **An empty interval is dropped.** The file's 2021 update set most records' lowest bound to
//!   300 K (its header: "data is only fit to 300K"), which leaves `Si(cr)`'s and `Ca(a)`'s first
//!   intervals running from 300 down to 298.15 K, holding no temperature; their second intervals
//!   start at 298.15 K.
//!
//! # The reference states
//!
//! NIST-JANAF's convention (Chase 1998, Introduction §5, pp. 13–14): each element's reference
//! state is its stable form at 1 bar ([`reference`]). H, N, O, Cl and the noble gases are their
//! gases at every temperature, carbon is graphite throughout, and every other element is its
//! condensed Ref-Elm records up to the temperature where their G per atom meets its gas form's at
//! 1 bar, then that gas: the monatomic gas for the metals, S₂ for sulphur and P₂ for phosphorus,
//! as JANAF's reference tables (S-001, P-001; their heat capacity above the switch is the
//! diatomic's) hold them. CEA itself keeps a condensed element condensed over its whole range
//! (McBride, Zehe and Gordon 2002, p. 2), so the values the sim derives are CEA's records
//! evaluated with JANAF's reference states, not values CEA prints. The task finds each switch by
//! bisection on the records ([`JANAF_GAS_FROM_K`] holds JANAF's for the check). Sulphur's and
//! phosphorus's switches are not boiling points: each is where its diatomic gas's fugacity over
//! the condensed element reaches 1 bar. The electron gas `e-` is the reference for charge, as in
//! both.
//!
//! # The cross-checks
//!
//! The acceptance line recomputes the registry's two transitions that were reduced from JANAF
//! (decision-p14-t49b-condensed §3.1(5)) from these records: `MgSO4`'s open-system decomposition
//! to `MgO` with SO₃, SO₂ and O₂ at 1 bar in all, and graphite's summed C₁–C₅ vapour at 1 bar.

use std::fmt::{self, Write as _};
use std::num::NonZeroUsize;

use hyperion_sim::math;
pub use hyperion_sim::substance::formation::NOT_MODELLED;
use hyperion_sim::substance::{Element, by_key, ids};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The dataset the task reads.
pub const DATASET: &str = "nasa_cea_thermo";

/// The library's file in the dataset.
pub const LIBRARY: &str = "thermo.inp";

/// The top of the table's domain, K: the plan's 6,000 K.
///
/// An interval starting here or above is not emitted.
pub const UPPER_TEMPERATURE_K: f64 = 6_000.0;

/// The bisection's steps for a reference state's switch, more than the 2⁻⁶⁰ of a 6,000 K bracket
/// an `f64` resolves; it stops sooner once the bracket's ends are adjacent.
const SWITCH_STEPS: u32 = 200;

/// The registry's gas rows the task emits, in registry order: each row's key and its CEA record's
/// name.
///
/// CEA writes `AL`, `AL+` and `HCL` in its capitals, and acetylene's record is `C2H2,acetylene`
/// (not vinylidene); HCN is `HCN` (not `HNC`). The rows without a record are the sim's
/// [`NOT_MODELLED`].
pub const ROWS: [(&str, &str); 57] = [
    ("H2", "H2"),
    ("He", "He"),
    ("H2O", "H2O"),
    ("CH4", "CH4"),
    ("NH3", "NH3"),
    ("N2", "N2"),
    ("O2", "O2"),
    ("CO2", "CO2"),
    ("Ar", "Ar"),
    ("Ne", "Ne"),
    ("Kr", "Kr"),
    ("Xe", "Xe"),
    ("N2O", "N2O"),
    ("CO", "CO"),
    ("SO2", "SO2"),
    ("H2S", "H2S"),
    ("O3", "O3"),
    ("HCN", "HCN"),
    ("C2H2", "C2H2,acetylene"),
    ("C2H4", "C2H4"),
    ("C2H6", "C2H6"),
    ("PH3", "PH3"),
    ("CH3OH", "CH3OH"),
    ("H", "H"),
    ("O", "O"),
    ("e-", "e-"),
    ("H-", "H-"),
    ("Na", "Na"),
    ("K", "K"),
    ("Fe", "Fe"),
    ("Mg", "Mg"),
    ("Si", "Si"),
    ("Ca", "Ca"),
    ("Ti", "Ti"),
    ("SiO", "SiO"),
    ("TiO", "TiO"),
    ("VO", "VO"),
    ("Al", "AL"),
    ("V", "V"),
    ("Mn", "Mn"),
    ("Ni", "Ni"),
    ("Zn", "Zn"),
    ("Na+", "Na+"),
    ("Mg+", "Mg+"),
    ("Al+", "AL+"),
    ("Si+", "Si+"),
    ("K+", "K+"),
    ("Ca+", "Ca+"),
    ("Ti+", "Ti+"),
    ("V+", "V+"),
    ("Cr+", "Cr+"),
    ("Mn+", "Mn+"),
    ("Fe+", "Fe+"),
    ("Ni+", "Ni+"),
    ("Zn+", "Zn+"),
    ("HCl", "HCL"),
    ("Cr", "Cr"),
];

/// The reference gases that are not registry rows: chlorine's, sulphur's and phosphorus's.
pub const REFERENCE_GASES: [(&str, &str); 3] = [("CL2", "CL2"), ("S2", "S2"), ("P2", "P2")];

/// Each element's reference state, by the table's items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reference {
    /// The element's gas at every temperature: the item and its atoms per formula unit.
    Gas {
        /// The gas's item.
        item: &'static str,
        /// Its atoms per formula unit.
        atoms: u8,
    },
    /// Graphite at every temperature (`C_GRAPHITE`).
    Graphite,
    /// The condensed phases up to the switch, then the gas form.
    Condensed {
        /// The condensed phases' item, and the stem of the switch's `<stem>_BOILING_K`.
        item: &'static str,
        /// The gas form's item.
        gas: &'static str,
        /// The gas form's atoms per formula unit.
        atoms: u8,
    },
}

/// The reference state of `element` (NIST-JANAF's convention, Chase 1998, Introduction §5, pp.
/// 13–14).
#[must_use]
pub const fn reference(element: Element) -> Reference {
    const fn gas(item: &'static str, atoms: u8) -> Reference {
        Reference::Gas { item, atoms }
    }
    const fn condensed(item: &'static str, gas: &'static str, atoms: u8) -> Reference {
        Reference::Condensed { item, gas, atoms }
    }
    match element {
        Element::H => gas("H2", 2),
        Element::He => gas("HE", 1),
        Element::C => Reference::Graphite,
        Element::N => gas("N2", 2),
        Element::O => gas("O2", 2),
        Element::Ne => gas("NE", 1),
        Element::Na => condensed("NA", "NA", 1),
        Element::Mg => condensed("MG", "MG", 1),
        Element::Al => condensed("AL", "AL", 1),
        Element::Si => condensed("SI", "SI", 1),
        Element::P => condensed("P", "P2", 2),
        Element::S => condensed("S", "S2", 2),
        Element::Cl => gas("CL2", 2),
        Element::Ar => gas("AR", 1),
        Element::K => condensed("K", "K", 1),
        Element::Ca => condensed("CA", "CA", 1),
        Element::Ti => condensed("TI", "TI", 1),
        Element::V => condensed("V", "V", 1),
        Element::Cr => condensed("CR", "CR", 1),
        Element::Mn => condensed("MN", "MN", 1),
        Element::Fe => condensed("FE", "FE", 1),
        Element::Ni => condensed("NI", "NI", 1),
        Element::Zn => condensed("ZN", "ZN", 1),
        Element::Kr => gas("KR", 1),
        Element::Xe => gas("XE", 1),
    }
}

/// The condensed reference states' CEA records, in [`Element::ALL`]'s order: graphite, then each
/// element's Ref-Elm phases.
///
/// `Cr(cr)`, `Fe(a)` and `Ni(cr)` each name two records, the phase below and above its lambda
/// transition.
pub const CONDENSED: [(Element, &[&str]); 16] = [
    (Element::C, &["C(gr)"]),
    (Element::Na, &["Na(cr)", "Na(L)"]),
    (Element::Mg, &["Mg(cr)", "Mg(L)"]),
    (Element::Al, &["AL(cr)", "AL(L)"]),
    (Element::Si, &["Si(cr)", "Si(L)"]),
    (Element::P, &["P(cr)", "P(L)"]),
    (Element::S, &["S(a)", "S(b)", "S(L)"]),
    (Element::K, &["K(cr)", "K(L)"]),
    (Element::Ca, &["Ca(a)", "Ca(b)", "Ca(L)"]),
    (Element::Ti, &["Ti(a)", "Ti(b)", "Ti(L)"]),
    (Element::V, &["V(cr)", "V(L)"]),
    (Element::Cr, &["Cr(cr)", "Cr(L)"]),
    (Element::Mn, &["Mn(a)", "Mn(b)", "Mn(c)", "Mn(d)", "Mn(L)"]),
    (Element::Fe, &["Fe(a)", "Fe(c)", "Fe(d)", "Fe(L)"]),
    (Element::Ni, &["Ni(cr)", "Ni(L)"]),
    (Element::Zn, &["Zn(cr)", "Zn(L)"]),
];

/// The cross-checks' records that are neither rows nor references: each item, its phase, and its
/// CEA records.
pub const CROSS_CHECKS: [(&str, Phase, &[&str]); 8] = [
    ("SO3", Phase::Gas, &["SO3"]),
    ("C", Phase::Gas, &["C"]),
    ("C2", Phase::Gas, &["C2"]),
    ("C3", Phase::Gas, &["C3"]),
    ("C4", Phase::Gas, &["C4"]),
    ("C5", Phase::Gas, &["C5"]),
    ("MGO_CRYSTAL", Phase::Condensed, &["MgO(cr)"]),
    (
        "MGSO4_CRYSTAL",
        Phase::Condensed,
        &["MgSO4(II)", "MgSO4(I)"],
    ),
];

/// NIST-JANAF's switch of each condensed reference state to its gas at 1 bar, K, and its table.
///
/// Chase 1998, NIST SRD 13, read at janaf.nist.gov 2026-10-19: each element's reference table's
/// "LIQUID <--> IDEAL GAS, FUGACITY = 1 bar" row; one transition per element, a fact,
/// decision-p14-t49b-condensed §3.1(1). The check, not a source: the task's switches are CEA's.
/// Sulphur's and phosphorus's are where S₂'s and P₂'s fugacity reaches 1 bar, not boiling points.
pub const JANAF_GAS_FROM_K: [(Element, f64, &str); 15] = [
    (Element::Na, 1_170.525, "Na-001"),
    (Element::Mg, 1_366.104, "Mg-001"),
    (Element::Al, 2_790.812, "Al-001"),
    (Element::Si, 3_504.616, "Si-001"),
    (Element::P, 1_180.008, "P-001"),
    (Element::S, 882.117, "S-001"),
    (Element::K, 1_039.540, "K-001"),
    (Element::Ca, 1_773.658, "Ca-001"),
    (Element::Ti, 3_630.956, "Ti-001"),
    (Element::V, 3_690.080, "V-001"),
    (Element::Cr, 2_952.078, "Cr-001"),
    (Element::Mn, 2_334.526, "Mn-001"),
    (Element::Fe, 3_133.345, "Fe-003"),
    (Element::Ni, 3_156.584, "Ni-001"),
    (Element::Zn, 1_180.173, "Zn-001"),
];

/// The table's items, in order: the rows in registry order, the three other reference gases, the
/// condensed references in [`Element::ALL`]'s order each with its switch, then the cross-checks'
/// records.
pub const ITEMS: [&str; 99] = [
    "H2",
    "HE",
    "H2O",
    "CH4",
    "NH3",
    "N2",
    "O2",
    "CO2",
    "AR",
    "NE",
    "KR",
    "XE",
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
    "ELECTRON",
    "H_MINUS",
    "NA",
    "K",
    "FE",
    "MG",
    "SI",
    "CA",
    "TI",
    "SIO",
    "TIO",
    "VO",
    "AL",
    "V",
    "MN",
    "NI",
    "ZN",
    "NA_PLUS",
    "MG_PLUS",
    "AL_PLUS",
    "SI_PLUS",
    "K_PLUS",
    "CA_PLUS",
    "TI_PLUS",
    "V_PLUS",
    "CR_PLUS",
    "MN_PLUS",
    "FE_PLUS",
    "NI_PLUS",
    "ZN_PLUS",
    "HCL",
    "CR",
    "CL2",
    "S2",
    "P2",
    "C_GRAPHITE",
    "NA_CONDENSED",
    "NA_BOILING_K",
    "MG_CONDENSED",
    "MG_BOILING_K",
    "AL_CONDENSED",
    "AL_BOILING_K",
    "SI_CONDENSED",
    "SI_BOILING_K",
    "P_CONDENSED",
    "P_BOILING_K",
    "S_CONDENSED",
    "S_BOILING_K",
    "K_CONDENSED",
    "K_BOILING_K",
    "CA_CONDENSED",
    "CA_BOILING_K",
    "TI_CONDENSED",
    "TI_BOILING_K",
    "V_CONDENSED",
    "V_BOILING_K",
    "CR_CONDENSED",
    "CR_BOILING_K",
    "MN_CONDENSED",
    "MN_BOILING_K",
    "FE_CONDENSED",
    "FE_BOILING_K",
    "NI_CONDENSED",
    "NI_BOILING_K",
    "ZN_CONDENSED",
    "ZN_BOILING_K",
    "SO3",
    "C",
    "C2",
    "C3",
    "C4",
    "C5",
    "MGO_CRYSTAL",
    "MGSO4_CRYSTAL",
];

/// The table item of the row whose key is `key`: the key in capitals, `+` as `_PLUS` and `-` as
/// `_MINUS`, and the electron's `ELECTRON`.
#[must_use]
pub fn item_name(key: &str) -> String {
    if key == "e-" {
        return "ELECTRON".to_owned();
    }
    key.to_ascii_uppercase()
        .replace('+', "_PLUS")
        .replace('-', "_MINUS")
}

/// A record's phase: a gas, or a condensed phase (the library's phase field set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    /// A gas (phase field blank or 0).
    Gas,
    /// A condensed phase (any other phase field).
    Condensed,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Gas => "gas",
            Self::Condensed => "condensed",
        })
    }
}

/// One interval of a NASA 9-coefficient record: its bounds, K, and its coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    lower_k: f64,
    upper_k: f64,
    a: [f64; 7],
    b: [f64; 2],
}

impl Interval {
    /// Its lower bound, K.
    #[must_use]
    pub const fn lower_k(&self) -> f64 {
        self.lower_k
    }

    /// Its upper bound, K.
    #[must_use]
    pub const fn upper_k(&self) -> f64 {
        self.upper_k
    }

    /// Whether `t_k` kelvin lies in it, its bounds included.
    #[must_use]
    pub fn holds(&self, t_k: f64) -> bool {
        self.lower_k <= t_k && t_k <= self.upper_k
    }

    /// The table's row: `[T_lo, T_hi, a₁, …, a₇, b₁, b₂]`.
    #[must_use]
    pub fn row(&self) -> Vec<f64> {
        let mut row = Vec::with_capacity(11);
        row.extend_from_slice(&[self.lower_k, self.upper_k]);
        row.extend_from_slice(&self.a);
        row.extend_from_slice(&self.b);
        row
    }

    /// G ÷ RT at `t_k` kelvin (McBride, Zehe and Gordon 2002, eqs. (2) and (3), p. 2), the sum's
    /// terms in the order the sim's `substance::formation` adds them.
    #[must_use]
    pub fn gibbs_over_rt(&self, t_k: f64) -> f64 {
        let [a1, a2, a3, a4, a5, a6, a7] = self.a;
        let [b1, b2] = self.b;
        let t = t_k;
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
        h - s
    }

    /// `c_p` ÷ R at `t_k` kelvin (eq. (1), p. 2).
    #[must_use]
    pub fn heat_capacity_over_r(&self, t_k: f64) -> f64 {
        let [a1, a2, a3, a4, a5, a6, a7] = self.a;
        let t = t_k;
        let t2 = t * t;
        a1 / t2 + a2 / t + a3 + a4 * t + a5 * t2 + a6 * t2 * t + a7 * t2 * t2
    }
}

/// One species record of a CEA library.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    name: String,
    comment: String,
    code: String,
    formula: Vec<(String, f64)>,
    phase: Phase,
    intervals: Vec<Interval>,
}

impl Record {
    /// Its name, as the library writes it (`H2O`, `Fe(a)`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its comment, the record's sources.
    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }

    /// Its reference-date code (`g 8/89`, `tpis89`, `j12/67`).
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Its formula: each element in the library's capitals with its count, the electron as `E`,
    /// its count −1 for a cation and +1 for an anion.
    #[must_use]
    pub fn formula(&self) -> &[(String, f64)] {
        &self.formula
    }

    /// Its phase.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Its intervals, as the library writes them.
    #[must_use]
    pub fn intervals(&self) -> &[Interval] {
        &self.intervals
    }
}

/// A CEA library could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseThermoError {
    /// The library is not ASCII text.
    #[error("the library is not ASCII text")]
    NotAscii,
    /// It has no `thermo` line, or nothing after it.
    #[error("the library has no `thermo` line and temperature line")]
    NoHeader,
    /// A record is malformed.
    #[error("line {line}: {why}")]
    Malformed {
        /// The line, from 1.
        line: usize,
        /// What is wrong.
        why: &'static str,
    },
}

/// A field of the library's fixed columns: `line[from..to]`, or the empty string past its end.
#[must_use]
fn field(line: &str, from: usize, to: usize) -> &str {
    let to = to.min(line.len());
    line.get(from.min(to)..to).unwrap_or("")
}

/// A Fortran number (`1.13D+05`, `6197.428`), or 0 for a blank field.
#[must_use]
fn number(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return Some(0.0);
    }
    // The parse error carries nothing the caller's `None` loses: every caller turns `None` into
    // `Malformed` with the line and "a field is not a number".
    text.replace(['D', 'd'], "E").parse().ok()
}

/// The exponents every interval of the library uses, as its interval line writes them.
const EXPONENTS: [f64; 8] = [-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 0.0];

/// The library's lines, read by line index (from 0) with errors that name the line (from 1).
struct Lines<'a> {
    lines: Vec<&'a str>,
}

impl<'a> Lines<'a> {
    /// The error at line index `i`.
    #[must_use]
    fn bad(i: usize, why: &'static str) -> ParseThermoError {
        ParseThermoError::Malformed { line: i + 1, why }
    }

    /// Line `i`, or `Malformed` if the library ends before it.
    fn at(&self, i: usize) -> Result<&'a str, ParseThermoError> {
        self.lines
            .get(i)
            .copied()
            .ok_or_else(|| Self::bad(i, "the record ends early"))
    }

    /// The number in columns `from..to` of line `i`, which the caller has already read with
    /// [`Lines::at`].
    fn number(&self, i: usize, from: usize, to: usize) -> Result<f64, ParseThermoError> {
        number(field(self.at(i)?, from, to)).ok_or_else(|| Self::bad(i, "a field is not a number"))
    }

    /// The interval whose three lines start at line `j`.
    fn interval(&self, j: usize) -> Result<Interval, ParseThermoError> {
        let head = self.at(j)?;
        self.at(j + 2)?;
        if field(head, 22, 23) != "7" {
            return Err(Self::bad(j, "an interval of other than seven coefficients"));
        }
        for (k, &expected) in EXPONENTS.iter().enumerate() {
            let exponent = self.number(j, 23 + 5 * k, 28 + 5 * k)?;
            if exponent.total_cmp(&expected).is_ne() {
                return Err(Self::bad(j, "an interval's exponents are not -2 to 4"));
            }
        }
        let mut a = [0.0; 7];
        for (k, slot) in a.iter_mut().take(5).enumerate() {
            *slot = self.number(j + 1, 16 * k, 16 * k + 16)?;
        }
        a[5] = self.number(j + 2, 0, 16)?;
        a[6] = self.number(j + 2, 16, 32)?;
        Ok(Interval {
            lower_k: self.number(j, 0, 11)?,
            upper_k: self.number(j, 11, 22)?,
            a,
            b: [self.number(j + 2, 48, 64)?, self.number(j + 2, 64, 80)?],
        })
    }

    /// The record whose name line is line `i`, and the index of the line after it.
    fn record(&self, i: usize) -> Result<(Record, usize), ParseThermoError> {
        let mut parts = self.at(i)?.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or_default().to_owned();
        let comment = parts.next().unwrap_or_default().trim().to_owned();
        let second = self.at(i + 1)?;
        let count: usize = field(second, 0, 2)
            .trim()
            .parse()
            .map_err(|_| Self::bad(i + 1, "the interval count is not a number"))?;
        if count > 9 {
            return Err(Self::bad(i + 1, "more than nine intervals"));
        }
        let code = field(second, 3, 9).to_owned();
        let mut formula = Vec::new();
        for k in 0..5 {
            let element = field(second, 10 + 8 * k, 12 + 8 * k).trim();
            let n = self.number(i + 1, 12 + 8 * k, 18 + 8 * k)?;
            // A zero count, of either sign, names no element.
            if !element.is_empty() && n.abs().total_cmp(&0.0).is_gt() {
                formula.push((element.to_ascii_uppercase(), n));
            }
        }
        let phase = match field(second, 50, 52).trim() {
            "" | "0" => Phase::Gas,
            _ => Phase::Condensed,
        };
        let mut intervals = Vec::with_capacity(count);
        let mut j = i + 2;
        if count == 0 {
            self.at(j)?;
            j += 1;
        }
        for _ in 0..count {
            intervals.push(self.interval(j)?);
            j += 3;
        }
        let record = Record {
            name,
            comment,
            code,
            formula,
            phase,
            intervals,
        };
        Ok((record, j))
    }
}

/// Every product record of a CEA library in the TP's record layout (McBride, Zehe and Gordon
/// 2002, Appendix C, Table C1, p. 43): the records between the temperature line after `thermo`
/// and `END PRODUCTS`.
///
/// # Errors
///
/// [`ParseThermoError`] for text that is not ASCII, a missing header, or a malformed record: a
/// field that is not a number, an interval count above 9, an interval whose exponents are not the
/// 7-term set `-2 … 4`, or a record cut short by the library's end.
pub fn parse(text: &str) -> Result<Vec<Record>, ParseThermoError> {
    if !text.is_ascii() {
        return Err(ParseThermoError::NotAscii);
    }
    let lines = Lines {
        lines: text.lines().map(|l| l.trim_end_matches('\r')).collect(),
    };
    let start = lines
        .lines
        .iter()
        .position(|l| l.starts_with("thermo"))
        .ok_or(ParseThermoError::NoHeader)?
        + 2;
    if start > lines.lines.len() {
        return Err(ParseThermoError::NoHeader);
    }
    let mut records = Vec::new();
    let mut i = start;
    while let Some(&line) = lines.lines.get(i) {
        if line.starts_with("END PRODUCTS") {
            break;
        }
        if line.starts_with('!') || line.trim().is_empty() {
            i += 1;
            continue;
        }
        let (record, next) = lines.record(i)?;
        records.push(record);
        i = next;
    }
    Ok(records)
}

/// The fit could not be made from the library.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FitThermoError {
    /// The dataset holds no library file of this name.
    #[error("the dataset holds no `{0}`")]
    NoLibrary(&'static str),
    /// The library has no record of this name and phase.
    #[error("the library holds no {phase} record named `{name}`")]
    Missing {
        /// The record's name.
        name: String,
        /// The phase sought.
        phase: Phase,
    },
    /// The library names a gas twice.
    #[error("the library holds two gas records named `{0}`")]
    Duplicate(String),
    /// The registry's gas rows are not the task's.
    #[error("the registry's gas rows are {found:?}, not the task's and its unmodelled ones")]
    Rows {
        /// The registry's gas rows, by key.
        found: Vec<String>,
    },
    /// A record's formula is not its row's stoichiometry and charge.
    #[error("`{key}`'s record has the formula {found:?}, not the registry's {expected:?}")]
    Formula {
        /// The row.
        key: String,
        /// The record's formula.
        found: Vec<(String, f64)>,
        /// The row's, in the library's terms.
        expected: Vec<(String, f64)>,
    },
    /// An item's intervals do not join end to end.
    #[error("`{item}`'s intervals do not join at {at_k} K")]
    Gap {
        /// The item.
        item: String,
        /// Where the gap or the overlap begins, K.
        at_k: f64,
    },
    /// An item has no interval below 6,000 K.
    #[error("`{0}` has no interval below 6,000 K")]
    Empty(String),
    /// A condensed reference state does not meet its gas inside both ranges.
    #[error("{0:?}'s condensed phases do not meet its gas form inside their common range")]
    NoSwitch(Element),
    /// The fit's items are not the task's [`ITEMS`].
    #[error("the fit's items are {found:?}, not the task's")]
    Items {
        /// The fit's items, in order.
        found: Vec<String>,
    },
}

/// An item's intervals, from one or more records: in temperature order, an empty interval and any
/// starting at [`UPPER_TEMPERATURE_K`] or above dropped (each named in `dropped`), each joining the
/// next exactly.
fn pieces(
    item: &str,
    records: &[&Record],
    dropped: &mut Vec<String>,
) -> Result<Vec<Interval>, FitThermoError> {
    let mut kept = Vec::new();
    for record in records {
        for interval in &record.intervals {
            if interval.lower_k >= UPPER_TEMPERATURE_K {
                continue;
            }
            if interval.lower_k >= interval.upper_k {
                dropped.push(format!(
                    "`{}` {}–{} K",
                    record.name, interval.lower_k, interval.upper_k
                ));
                continue;
            }
            kept.push(*interval);
        }
    }
    kept.sort_by(|a, b| a.lower_k.total_cmp(&b.lower_k));
    for pair in kept.windows(2) {
        if pair[0].upper_k.total_cmp(&pair[1].lower_k).is_ne() {
            return Err(FitThermoError::Gap {
                item: item.to_owned(),
                at_k: pair[0].upper_k,
            });
        }
    }
    if kept.is_empty() {
        return Err(FitThermoError::Empty(item.to_owned()));
    }
    Ok(kept)
}

/// G ÷ RT of piecewise `intervals` at `t_k` kelvin, from the first interval that holds it (the
/// lower at a shared bound), or `None` outside them all.
#[must_use]
pub fn gibbs_over_rt(intervals: &[Interval], t_k: f64) -> Option<f64> {
    intervals
        .iter()
        .find(|i| i.holds(t_k))
        .map(|i| i.gibbs_over_rt(t_k))
}

/// One item of the table, before rendering.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A record or a set of phases: its name, doc and intervals.
    Record {
        /// The item's name.
        name: String,
        /// Its doc lines.
        doc: Vec<String>,
        /// Its intervals.
        intervals: Vec<Interval>,
    },
    /// A reference state's switch to its gas.
    Switch {
        /// The item's name.
        name: String,
        /// The element.
        element: Element,
        /// The temperature, K.
        value_k: f64,
    },
}

impl Item {
    /// The item's name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Record { name, .. } | Self::Switch { name, .. } => name,
        }
    }
}

/// The fit: the table's items in order, and the intervals dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    items: Vec<Item>,
    dropped: Vec<String>,
}

impl Fit {
    /// The items, in the table's order.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The empty intervals dropped, by record.
    #[must_use]
    pub fn dropped(&self) -> &[String] {
        &self.dropped
    }

    /// The intervals of item `name`, if it is a record.
    #[must_use]
    pub fn intervals(&self, name: &str) -> Option<&[Interval]> {
        self.items.iter().find_map(|item| match item {
            Item::Record {
                name: n, intervals, ..
            } if n == name => Some(intervals.as_slice()),
            Item::Record { .. } | Item::Switch { .. } => None,
        })
    }

    /// The switch of `element`'s reference state to its gas, K, if it has one.
    #[must_use]
    pub fn switch_k(&self, element: Element) -> Option<f64> {
        self.items.iter().find_map(|item| match item {
            Item::Switch {
                element: e,
                value_k,
                ..
            } if *e == element => Some(*value_k),
            Item::Record { .. } | Item::Switch { .. } => None,
        })
    }

    /// G ÷ RT per atom of `element`'s reference state at `t_k` kelvin, or `None` outside its
    /// range.
    #[must_use]
    pub fn reference_gibbs_over_rt(&self, element: Element, t_k: f64) -> Option<f64> {
        match reference(element) {
            Reference::Gas { item, atoms } => {
                Some(gibbs_over_rt(self.intervals(item)?, t_k)? / f64::from(atoms))
            }
            Reference::Graphite => gibbs_over_rt(self.intervals("C_GRAPHITE")?, t_k),
            Reference::Condensed { item, gas, atoms } => {
                if t_k <= self.switch_k(element)? {
                    gibbs_over_rt(self.intervals(&format!("{item}_CONDENSED"))?, t_k)
                } else {
                    Some(gibbs_over_rt(self.intervals(gas)?, t_k)? / f64::from(atoms))
                }
            }
        }
    }

    /// log₁₀ `K_f` at 1 bar of item `name`, whose formula is `formula` in the library's terms, at
    /// `t_k` kelvin.
    ///
    /// It is −(G − Σ n `G_ref`) ÷ (RT ln 10), the electron gas taking an ion's charge (McBride,
    /// Zehe and Gordon 2002, eq. (9), p. 3). `None` where the item or a reference has no interval
    /// holding `t_k`.
    #[must_use]
    pub fn log10_formation_constant(
        &self,
        name: &str,
        formula: &[(String, f64)],
        t_k: f64,
    ) -> Option<f64> {
        let mut g = gibbs_over_rt(self.intervals(name)?, t_k)?;
        for (symbol, count) in formula {
            let reference = if symbol == "E" {
                gibbs_over_rt(self.intervals("ELECTRON")?, t_k)?
            } else {
                self.reference_gibbs_over_rt(element_of(symbol)?, t_k)?
            };
            g -= count * reference;
        }
        Some(-g / std::f64::consts::LN_10)
    }
}

/// The element a library symbol names (`MG`, `CL`, `AL`).
#[must_use]
fn element_of(symbol: &str) -> Option<Element> {
    Element::ALL
        .into_iter()
        .find(|e| e.symbol().eq_ignore_ascii_case(symbol))
}

/// The records of `library` named `names`, of `phase`, in the library's order.
fn find<'a>(
    library: &'a [Record],
    names: &[&str],
    phase: Phase,
) -> Result<Vec<&'a Record>, FitThermoError> {
    let mut found = Vec::new();
    for name in names {
        let hits: Vec<&Record> = library
            .iter()
            .filter(|r| r.name == *name && r.phase == phase)
            .collect();
        if hits.is_empty() {
            return Err(FitThermoError::Missing {
                name: (*name).to_owned(),
                phase,
            });
        }
        if phase == Phase::Gas && hits.len() > 1 {
            return Err(FitThermoError::Duplicate((*name).to_owned()));
        }
        found.extend(hits);
    }
    Ok(found)
}

/// The doc of a record item.
#[must_use]
fn record_doc(subject: &str, records: &[&Record]) -> Vec<String> {
    let mut doc = vec![format!(
        "{subject}: NASA Glenn's 9-coefficient {}, intervals to 6,000 K, each `[T_lo, T_hi, a₁, …, a₇, b₁, b₂]`.",
        if records.len() == 1 {
            "record"
        } else {
            "records"
        }
    )];
    for record in records {
        doc.push(format!(
            "`{}` ({}): `{}`",
            record.name,
            record.code.trim(),
            record.comment.replace('`', "'")
        ));
    }
    doc
}

/// The library's terms for `key`'s stoichiometry and charge: each element in capitals with its
/// count, then `E` at minus the charge.
#[must_use]
fn registry_formula(id: hyperion_sim::substance::SubstanceId) -> Vec<(String, f64)> {
    let row = id.substance();
    let mut formula: Vec<(String, f64)> = row
        .elements()
        .iter()
        .map(|&(e, n)| (e.symbol().to_ascii_uppercase(), f64::from(n)))
        .collect();
    if row.charge() != 0 {
        formula.push(("E".to_owned(), -f64::from(row.charge())));
    }
    formula.sort_by(|a, b| a.0.cmp(&b.0));
    formula
}

/// Whether two formulas in the library's terms are the same, element by element, the counts
/// compared by [`f64::total_cmp`].
#[must_use]
fn same_formula(a: &[(String, f64)], b: &[(String, f64)]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.0 == y.0 && x.1.total_cmp(&y.1).is_eq())
}

/// Whether gas row `key` has a record: it is not one of [`NOT_MODELLED`]'s.
#[must_use]
fn modelled(key: &str) -> bool {
    NOT_MODELLED.iter().all(|&(n, _)| n != key)
}

/// The keys of the gas rows the task emits, from `gas_keys`, every gas row's key in registry
/// order: all but [`NOT_MODELLED`]'s.
///
/// # Errors
///
/// [`FitThermoError::Rows`] if they are not [`ROWS`]' keys in order, or a row of
/// [`NOT_MODELLED`]'s is not among `gas_keys`.
fn modelled_keys(gas_keys: Vec<String>) -> Result<Vec<String>, FitThermoError> {
    let unmodelled_present = NOT_MODELLED
        .iter()
        .all(|&(n, _)| gas_keys.iter().any(|k| k == n));
    let keys: Vec<String> = gas_keys.into_iter().filter(|k| modelled(k)).collect();
    if !unmodelled_present
        || keys
            .iter()
            .map(String::as_str)
            .ne(ROWS.iter().map(|(k, _)| *k))
    {
        return Err(FitThermoError::Rows { found: keys });
    }
    Ok(keys)
}

/// The registry's gas rows' items, in registry order, each checked against its row's stoichiometry
/// and charge.
fn row_items(library: &[Record], dropped: &mut Vec<String>) -> Result<Vec<Item>, FitThermoError> {
    let gas_rows: Vec<_> = ids().filter(|id| id.substance().gas().is_some()).collect();
    modelled_keys(
        gas_rows
            .iter()
            .map(|id| id.substance().key().as_str().to_owned())
            .collect(),
    )?;
    let mut items = Vec::with_capacity(ROWS.len());
    let rows = gas_rows
        .iter()
        .filter(|id| modelled(id.substance().key().as_str()));
    for (&(key, name), id) in ROWS.iter().zip(rows) {
        let records = find(library, &[name], Phase::Gas)?;
        let mut found = records[0].formula.clone();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        let expected = registry_formula(*id);
        if !same_formula(&found, &expected) {
            return Err(FitThermoError::Formula {
                key: key.to_owned(),
                found,
                expected,
            });
        }
        let item = item_name(key);
        items.push(Item::Record {
            doc: record_doc(&format!("`{key}`"), &records),
            intervals: pieces(&item, &records, dropped)?,
            name: item,
        });
    }
    Ok(items)
}

/// The fit from the parsed `library`.
///
/// # Errors
///
/// [`FitThermoError`] if the registry's gas rows are not [`ROWS`] and [`NOT_MODELLED`], a record is
/// missing or names a gas twice, a row's record has another formula, an item's intervals do not
/// join or none lies below [`UPPER_TEMPERATURE_K`], or a condensed reference does not meet its
/// gas.
pub fn fit(library: &[Record]) -> Result<Fit, FitThermoError> {
    let mut dropped = Vec::new();
    let mut items = row_items(library, &mut dropped)?;
    for (item, name) in REFERENCE_GASES {
        let records = find(library, &[name], Phase::Gas)?;
        items.push(Item::Record {
            name: item.to_owned(),
            doc: record_doc(&format!("`{name}`, a reference gas"), &records),
            intervals: pieces(item, &records, &mut dropped)?,
        });
    }
    let mut fit = Fit {
        items,
        dropped: Vec::new(),
    };
    for (element, names) in CONDENSED {
        let records = find(library, names, Phase::Condensed)?;
        match reference(element) {
            Reference::Graphite => fit.items.push(Item::Record {
                name: "C_GRAPHITE".to_owned(),
                doc: record_doc(
                    "Carbon's reference state, graphite at every temperature",
                    &records,
                ),
                intervals: pieces("C_GRAPHITE", &records, &mut dropped)?,
            }),
            Reference::Condensed { item, gas, atoms } => {
                let name = format!("{item}_CONDENSED");
                let intervals = pieces(&name, &records, &mut dropped)?;
                let gas = fit
                    .intervals(gas)
                    .ok_or(FitThermoError::NoSwitch(element))?;
                let value_k = switch_k(&intervals, gas, f64::from(atoms))
                    .ok_or(FitThermoError::NoSwitch(element))?;
                fit.items.push(Item::Record {
                    doc: record_doc(
                        &format!(
                            "{}'s condensed reference phases, to `{item}_BOILING_K`",
                            element.symbol()
                        ),
                        &records,
                    ),
                    intervals,
                    name,
                });
                fit.items.push(Item::Switch {
                    name: format!("{item}_BOILING_K"),
                    element,
                    value_k,
                });
            }
            Reference::Gas { .. } => return Err(FitThermoError::NoSwitch(element)),
        }
    }
    for (item, phase, names) in CROSS_CHECKS {
        let records = find(library, names, phase)?;
        fit.items.push(Item::Record {
            name: item.to_owned(),
            doc: record_doc(&format!("`{item}`, for the cross-checks"), &records),
            intervals: pieces(item, &records, &mut dropped)?,
        });
    }
    fit.dropped = dropped;
    Ok(fit)
}

/// Where condensed `phases` meet their gas form `gas` of `atoms` atoms at 1 bar, K: the last
/// temperature, by bisection, at which the condensed G per atom is below the gas's, inside both
/// ranges; `None` if it is not below at the bracket's foot and above at its top.
///
/// The miss is negative at the switch and not negative at the next `f64` above it.
#[must_use]
pub fn switch_k(phases: &[Interval], gas: &[Interval], atoms: f64) -> Option<f64> {
    let foot = phases.first()?.lower_k.max(gas.first()?.lower_k);
    let top = phases.last()?.upper_k.min(gas.last()?.upper_k);
    let miss = |t_k: f64| Some(gibbs_over_rt(phases, t_k)? - gibbs_over_rt(gas, t_k)? / atoms);
    bisect(miss, foot, top).map(|(below, _)| below)
}

/// The library's formula of item `name`'s first record.
#[must_use]
fn formula_of<'a>(library: &'a [Record], name: &str) -> Option<&'a [(String, f64)]> {
    library
        .iter()
        .find(|r| r.name == name)
        .map(|r| r.formula.as_slice())
}

/// The bisection's last bracket `(below, above)` of a root of `f` on [`below`, `above`], `f`
/// negative at the foot and positive at the top: `f` is negative at `below` and not negative at
/// `above`, the two adjacent `f64`s unless [`SWITCH_STEPS`] ran out first. `None` where `f` is not
/// so signed at the ends, or not defined at a point the bisection reads.
#[must_use]
fn bisect(f: impl Fn(f64) -> Option<f64>, mut below: f64, mut above: f64) -> Option<(f64, f64)> {
    if !(f(below)? < 0.0 && f(above)? > 0.0) {
        return None;
    }
    for _ in 0..SWITCH_STEPS {
        let mid = f64::midpoint(below, above);
        if mid <= below || mid >= above {
            break;
        }
        if f(mid)? < 0.0 {
            below = mid;
        } else {
            above = mid;
        }
    }
    Some((below, above))
}

/// The bisection's root of `f` on [`below`, `above`]: the midpoint of [`bisect`]'s last bracket.
#[must_use]
fn root(f: impl Fn(f64) -> Option<f64>, below: f64, above: f64) -> Option<f64> {
    bisect(f, below, above).map(|(below, above)| f64::midpoint(below, above))
}

/// `MgSO4`'s open-system decomposition at 1 bar, K, as the registry's row reduces it
/// (decision-p14-t49b-condensed §3.1(2)), both solids at unit activity and the gas the
/// decomposition's own:
///
/// ```text
/// log₁₀ p(SO₃) = log₁₀ K_f(MgO) + log₁₀ K_f(SO₃) − log₁₀ K_f(MgSO₄)
/// p(SO₂)^1.5 ÷ √2 = K_f(MgO) K_f(SO₂) ÷ K_f(MgSO₄)    (MgSO₄ → MgO + SO₂ + ½O₂, p(O₂) = p(SO₂) ÷ 2)
/// ```
///
/// the root of the total's log₁₀ between 1,000 and 1,400 K, found by bisection on the records
/// rather than by interpolating 100 K rows.
#[must_use]
pub fn magnesium_sulphate_decomposition_k(fit: &Fit, library: &[Record]) -> Option<f64> {
    let kf = |name: &str, record: &str, t_k: f64| {
        fit.log10_formation_constant(name, formula_of(library, record)?, t_k)
    };
    let total = |t_k: f64| {
        let solids = kf("MGO_CRYSTAL", "MgO(cr)", t_k)? - kf("MGSO4_CRYSTAL", "MgSO4(II)", t_k)?;
        let p_so3 = math::exp10(solids + kf("SO3", "SO3", t_k)?);
        let log_k2 = solids + kf("SO2", "SO2", t_k)?;
        let p_so2 = math::exp10((0.5 * math::log10(2.0) + log_k2) * 2.0 / 3.0);
        Some(math::log10(p_so3 + 1.5 * p_so2))
    };
    root(total, 1_000.0, 1_400.0)
}

/// Graphite's sublimation at 1 bar, K: where the summed vapour of C₁–C₅ over graphite at unit
/// activity, each p(Cₙ) in bar its `K_f`, reaches 1 bar, by bisection between 3,500 and 4,500 K.
#[must_use]
pub fn graphite_sublimation_k(fit: &Fit, library: &[Record]) -> Option<f64> {
    let total = |t_k: f64| {
        let mut sum = 0.0;
        for name in ["C", "C2", "C3", "C4", "C5"] {
            sum +=
                math::exp10(fit.log10_formation_constant(name, formula_of(library, name)?, t_k)?);
        }
        Some(math::log10(sum))
    };
    root(total, 3_500.0, 4_500.0)
}

/// The table's contents.
#[must_use]
pub fn render(fit: &Fit) -> RustTable {
    let items = fit
        .items
        .iter()
        .map(|item| match item {
            Item::Record {
                name,
                doc,
                intervals,
            } => TableItem::Nested {
                name: name.clone(),
                doc: doc.clone(),
                rows: intervals.iter().map(Interval::row).collect(),
            },
            Item::Switch {
                name,
                element,
                value_k,
            } => {
                let mut doc = vec![
                    format!(
                        "Where {}'s condensed phases meet its gas form at 1 bar, K: its reference state is",
                        element.symbol()
                    ),
                    "the condensed phases up to it and the gas above, found by bisection on the records."
                        .to_owned(),
                ];
                if let Reference::Condensed { gas, atoms, .. } = reference(*element)
                    && atoms > 1
                {
                    doc.push(format!(
                        "Not a boiling point: where {gas}'s fugacity over the condensed element reaches 1 bar."
                    ));
                }
                TableItem::Scalar {
                    name: name.clone(),
                    doc,
                    value: *value_k,
                }
            }
        })
        .collect();
    RustTable {
        summary: vec![
            "The gases' thermochemistry (plan 14, P14.T49.d): NASA's CEA records, which".to_owned(),
            "`substance::formation` reads.".to_owned(),
        ],
        notes: [
            "Each record is NASA Glenn's 9-coefficient polynomials (McBride, Zehe and Gordon 2002,",
            "NASA/TP-2002-211556, eqs. (1)-(3)) re-emitted, one interval a row as",
            "`[T_lo, T_hi, a1, ..., a7, b1, b2]`, K and the file's coefficients to the nearest f64, from",
            "the subset of CEA's `data/thermo.inp` in the dataset `nasa_cea_thermo` (Apache 2.0). They",
            "are re-emitted rather than refitted: they reproduce the source exactly, and their intervals",
            "and the elements' phase changes would make a Chebyshev fit piecewise anyway. Intervals from",
            "6,000 K up are not emitted, nor an empty one (`Si(cr)`'s and `Ca(a)`'s 300-298.15 K). The",
            "reference states are NIST-JANAF's convention (Chase 1998, Introduction section 5): H2, N2,",
            "O2, Cl2 and the noble gases; graphite; and the other elements' condensed phases, then their",
            "gas (S2 for sulphur, P2 for phosphorus) above each `_BOILING_K`; the electron gas for charge.",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        items,
    }
}

/// The registry row `key`'s transition temperature, K, if it has condensed columns.
#[must_use]
fn row_transition_k(key: &str) -> Option<f64> {
    Some(
        by_key(key)?
            .substance()
            .condensed()?
            .transition()
            .temperature()
            .value(),
    )
}

/// The acceptance line: what was re-emitted, the switches against JANAF's, and the two
/// cross-checks against the registry's rows as they stand.
#[must_use]
fn acceptance(fit: &Fit, library: &[Record]) -> String {
    let records: usize = fit
        .items
        .iter()
        .filter(|i| matches!(i, Item::Record { .. }))
        .count();
    let intervals: usize = fit
        .items
        .iter()
        .map(|i| match i {
            Item::Record { intervals, .. } => intervals.len(),
            Item::Switch { .. } => 0,
        })
        .sum();
    let mut switches = String::new();
    let mut worst = 0.0_f64;
    for &(element, janaf, _) in &JANAF_GAS_FROM_K {
        if let Some(t) = fit.switch_k(element) {
            worst = worst.max((t - janaf).abs());
            // Writing to a `String` cannot fail: its `fmt::Write` never returns an error.
            let _ = write!(
                switches,
                "{}{} {:.2} K ({:+.2})",
                if switches.is_empty() { "" } else { ", " },
                element.symbol(),
                t,
                t - janaf
            );
        }
    }
    // Each cross-check's root, and the rows it stands for with each row's value and its
    // difference from the root.
    let against = |root: Option<f64>, rows: &[&str]| {
        let mut text = root.map_or_else(|| "none".to_owned(), |t| format!("{t:.2} K"));
        for key in rows {
            // As above, a `String` is written infallibly.
            let _ = match (row_transition_k(key), root) {
                (Some(row), Some(t)) => {
                    write!(text, "; `{key}`'s row {row:.2} K ({:+.2})", row - t)
                }
                (Some(row), None) => write!(text, "; `{key}`'s row {row:.2} K"),
                (None, _) => write!(text, "; `{key}` has no transition"),
            };
        }
        text
    };
    let unmodelled: Vec<String> = NOT_MODELLED.iter().map(|(k, _)| format!("`{k}`")).collect();
    format!(
        "{} of the registry's {} gas rows ({} not modelled), {records} items of {intervals} intervals \
         re-emitted from the records to the bit; dropped as empty: {}; the reference states' switches to their \
         gas against NIST-JANAF's: {switches}, within {worst:.1} K; `MgSO4`'s decomposition at 1 bar {}; \
         graphite's sublimation at 1 bar {}",
        ROWS.len(),
        ROWS.len() + NOT_MODELLED.len(),
        unmodelled.join(", "),
        if fit.dropped.is_empty() {
            "none".to_owned()
        } else {
            fit.dropped.join(", ")
        },
        against(magnesium_sulphate_decomposition_k(fit, library), &["MgSO4"]),
        against(graphite_sublimation_k(fit, library), &["graphite", "soot"]),
    )
}

/// The fit's parameters as its manifest records them.
#[must_use]
fn parameters() -> toml::Table {
    let text = format!("upper_temperature_k = {UPPER_TEMPERATURE_K:?}\n");
    toml::from_str(&text).expect("the parameters' own text is TOML")
}

/// The task: P14.T49.d's records, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ThermoEquilibriumTask;

impl FitTask for ThermoEquilibriumTask {
    fn name(&self) -> &'static str {
        "thermo_equilibrium"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "thermo.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The registry's gas rows the table follows: each row's index, charge and stoichiometry, in
    /// registry order, each element by its place in [`Element::ALL`] before its count, so that a
    /// gas row added or changed asks for a rerun.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes = Vec::new();
        for id in ids().filter(|id| id.substance().gas().is_some()) {
            let row = id.substance();
            let key = row.key();
            probes.push((format!("{key} index"), f64::from(id.index())));
            probes.push((format!("{key} charge"), f64::from(row.charge())));
            for &(element, count) in row.elements() {
                let place = Element::ALL
                    .iter()
                    .position(|&e| e == element)
                    .and_then(|place| u8::try_from(place).ok())
                    .expect("every element is one of Element::ALL's 25");
                probes.push((format!("{key} element"), f64::from(place)));
                probes.push((format!("{key} {}", element.symbol()), f64::from(count)));
            }
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        manifest.expect_params(&parameters())?;
        let dataset = manifest.load_dataset(DATASET)?;
        let input = |source: Box<dyn std::error::Error + Send + Sync>| RunTaskError::Input {
            task: "thermo_equilibrium",
            source,
        };
        let bytes = dataset
            .files
            .iter()
            .find(|(name, _)| name == LIBRARY)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| input(Box::new(FitThermoError::NoLibrary(LIBRARY))))?;
        let text =
            std::str::from_utf8(bytes).map_err(|_| input(Box::new(ParseThermoError::NotAscii)))?;
        let library = parse(text).map_err(|e| input(Box::new(e)))?;
        let fitted = fit(&library).map_err(|e| input(Box::new(e)))?;
        let names: Vec<&str> = fitted.items.iter().map(Item::name).collect();
        if names != ITEMS {
            return Err(input(Box::new(FitThermoError::Items {
                found: names.into_iter().map(str::to_owned).collect(),
            })));
        }
        Ok(TaskOutput {
            source: "NASA Glenn's 9-coefficient records (McBride, Zehe and Gordon 2002, NASA/TP-2002-211556) \
                     in `data/thermo.inp` of NASA's CEA, nasa/cea at 2f79a647 (Apache 2.0), the subset of the \
                     dataset `nasa_cea_thermo`; the reference states by NIST-JANAF's convention (Chase 1998, \
                     Introduction section 5)"
                .to_owned(),
            acceptance: acceptance(&fitted, &library),
            table: render(&fitted),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Graphite's record as the library writes it (CRLF), three intervals.
    const SAMPLE: &str = "thermo\r\n    200.00   1000.00   6000.00  20000.   9/8/2021\r\n\
C(gr)             Graphite. Ref-Elm. TRC(4/83) vc,uc,tc1000-1002.\r\n \
3 n 4/83 C   1.00    0.00    0.00    0.00    0.00 1   12.0107000          0.000\r\n    \
300.000    600.0007 -2.0 -1.0  0.0  1.0  2.0  3.0  4.0  0.0         1053.500\r\n \
1.132856760D+05-1.980421677D+03 1.365384188D+01-4.636096440D-02 1.021333011D-04\r\n\
-1.082893179D-07 4.472258860D-11                 8.943859760D+03-7.295824740D+01\r\n    \
600.000   2000.0007 -2.0 -1.0  0.0  1.0  2.0  3.0  4.0  0.0         1053.500\r\n \
3.356004410D+05-2.596528368D+03 6.948841910D+00-3.484836090D-03 1.844192445D-06\r\n\
-5.055205960D-10 5.750639010D-14                 1.398412456D+04-4.477183040D+01\r\n   \
2000.000   6000.0007 -2.0 -1.0  0.0  1.0  2.0  3.0  4.0  0.0         1053.500\r\n \
2.023105106D+05-1.138235908D+03 3.700279500D+00-1.833807727D-04 6.343683250D-08\r\n\
-7.068589480D-12 3.335435980D-16                 5.848134850D+03-2.350925275D+01\r\n\
END PRODUCTS\r\nEND REACTANTS\r\n";

    /// The committed library, parsed.
    fn library() -> Vec<Record> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join(DATASET)
            .join(LIBRARY);
        parse(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// The committed library with gas record `name` changed by `change`.
    fn edited(library: &[Record], name: &str, change: impl Fn(&mut Record)) -> Vec<Record> {
        let mut edited = library.to_vec();
        let record = edited
            .iter_mut()
            .find(|r| r.name == name && r.phase == Phase::Gas)
            .unwrap_or_else(|| panic!("{name} is a gas record"));
        change(record);
        edited
    }

    #[test]
    fn thermo_equilibrium_a_record_is_read_from_its_columns() {
        let records = parse(SAMPLE).unwrap();
        assert_eq!(records.len(), 1);
        let graphite = &records[0];
        assert_eq!(graphite.name(), "C(gr)");
        assert_eq!(graphite.code(), "n 4/83");
        assert_eq!(graphite.phase(), Phase::Condensed);
        assert_eq!(graphite.formula(), [("C".to_owned(), 1.0)]);
        assert_eq!(graphite.intervals().len(), 3);
        let first = graphite.intervals()[0];
        assert_eq!(
            first.row(),
            [
                300.0,
                600.0,
                1.132_856_76e5,
                -1.980_421_677e3,
                13.653_841_88,
                -4.636_096_44e-2,
                1.021_333_011e-4,
                -1.082_893_179e-7,
                4.472_258_86e-11,
                8.943_859_76e3,
                -72.958_247_4
            ]
        );
        // Graphite's c_p ÷ R at 1,000 K, 2.599 256 (21.61 J mol⁻¹ K⁻¹), as the lane's independent
        // Python evaluation of the record gives it.
        let cp = graphite.intervals()[1].heat_capacity_over_r(1_000.0);
        assert!((cp - 2.599_256_132_1).abs() < 1e-12, "{cp}");
    }

    #[test]
    fn thermo_equilibrium_a_malformed_library_is_refused() {
        let malformed = |line, why| Err(ParseThermoError::Malformed { line, why });
        assert_eq!(parse("no header\n"), Err(ParseThermoError::NoHeader));
        assert_eq!(parse("thermo\n  é\n"), Err(ParseThermoError::NotAscii));
        let five = SAMPLE.replace("0.0007 -2.0", "0.0005 -2.0");
        assert_eq!(
            parse(&five),
            malformed(5, "an interval of other than seven coefficients")
        );
        let exponent = SAMPLE.replacen(" 4.0  0.0", " 5.0  0.0", 1);
        assert_eq!(
            parse(&exponent),
            malformed(5, "an interval's exponents are not -2 to 4")
        );
        let short = SAMPLE.lines().take(6).collect::<Vec<_>>().join("\n");
        assert_eq!(parse(&short), malformed(7, "the record ends early"));
        let ten = SAMPLE.replacen(" 3 n 4/83", "10 n 4/83", 1);
        assert_eq!(parse(&ten), malformed(4, "more than nine intervals"));
        let count = SAMPLE.replacen(" 3 n 4/83", " x n 4/83", 1);
        assert_eq!(
            parse(&count),
            malformed(4, "the interval count is not a number")
        );
        let coefficient = SAMPLE.replacen("1.132856760D+05", "1.132856760X+05", 1);
        assert_eq!(parse(&coefficient), malformed(6, "a field is not a number"));
        let atoms = SAMPLE.replacen("C   1.00", "C   1.x0", 1);
        assert_eq!(parse(&atoms), malformed(4, "a field is not a number"));
        let bound = SAMPLE.replacen("    300.000", "    300.0x0", 1);
        assert_eq!(parse(&bound), malformed(5, "a field is not a number"));
    }

    /// Every item is found in the committed library, in the task's order, and the only empty
    /// intervals are `Si(cr)`'s and `Ca(a)`'s.
    #[test]
    fn thermo_equilibrium_the_library_holds_every_item() {
        let library = library();
        assert_eq!(library.len(), 111);
        let fitted = fit(&library).unwrap();
        let names: Vec<&str> = fitted.items().iter().map(Item::name).collect();
        assert_eq!(names, ITEMS);
        assert_eq!(
            fitted.dropped(),
            ["`Si(cr)` 300–298.15 K", "`Ca(a)` 300–298.15 K"]
        );
        for item in fitted.items() {
            if let Item::Record { intervals, .. } = item {
                assert!(intervals.last().unwrap().upper_k() <= UPPER_TEMPERATURE_K);
            }
        }
        let table = render(&fitted);
        assert_eq!(table.item_names(), ITEMS);
    }

    /// Each switch is where the condensed phases' G per atom meets the gas form's, the miss
    /// negative at it and not negative at the next `f64` above, and within 15 K of NIST-JANAF's;
    /// the worst is vanadium's, 14.8 K. CEA's condensed records of V, Cr, Mn, Fe and Ni are Chase
    /// 1998's own, so their misses come from the gas atoms' enthalpies of formation: Gurvich
    /// 1982's for V, Desai 1987's for Mn and Hultgren 1973's for Fe and Ni (Cr's is Chase 1998's,
    /// −0.26 K); aluminium's crystal record and its gas's enthalpy are Cox et al. 1989's (CODATA).
    #[test]
    fn thermo_equilibrium_the_switches_meet_janaf_s() {
        let fitted = fit(&library()).unwrap();
        for &(element, janaf, table) in &JANAF_GAS_FROM_K {
            let t = fitted.switch_k(element).unwrap();
            assert!(
                (t - janaf).abs() < 15.0,
                "{element:?} at {t} K, JANAF's {table} {janaf} K"
            );
            let Reference::Condensed { item, gas, atoms } = reference(element) else {
                panic!("{element:?} has a condensed reference")
            };
            let phases = fitted.intervals(&format!("{item}_CONDENSED")).unwrap();
            let gas = fitted.intervals(gas).unwrap();
            let miss = |t: f64| {
                gibbs_over_rt(phases, t).unwrap()
                    - gibbs_over_rt(gas, t).unwrap() / f64::from(atoms)
            };
            assert!(miss(t).abs() < 1e-9, "{element:?}: {}", miss(t));
            assert!(miss(t) < 0.0, "{element:?} at its switch");
            assert!(miss(t.next_up()) >= 0.0, "{element:?} above its switch");
        }
        assert_eq!(
            CONDENSED
                .iter()
                .filter(|(e, _)| fitted.switch_k(*e).is_some())
                .count(),
            JANAF_GAS_FROM_K.len()
        );
    }

    /// The registry's two transitions reduced from NIST-JANAF, recomputed from the records: `MgSO4`'s
    /// 1,375.44 K and graphite's 3,973.32 K (the independent Python evaluation of the raw file,
    /// the dataset's `kf.py`), each now the row's within 10 K (decision-p14-t49b-condensed
    /// §3.1(5)).
    #[test]
    fn thermo_equilibrium_the_cross_checks_meet_the_rows() {
        let library = library();
        let fitted = fit(&library).unwrap();
        let mgso4 = magnesium_sulphate_decomposition_k(&fitted, &library).unwrap();
        let graphite = graphite_sublimation_k(&fitted, &library).unwrap();
        assert!((mgso4 - 1_375.441_3).abs() < 1e-3, "{mgso4}");
        assert!((graphite - 3_973.323_8).abs() < 1e-3, "{graphite}");
        let row = |key: &str| row_transition_k(key).unwrap();
        assert!((row("MgSO4") - mgso4).abs() < 10.0);
        assert!((row("graphite") - graphite).abs() < 10.0);
        assert!((row("soot") - graphite).abs() < 10.0);
    }

    /// A row whose record is missing, and a record of another formula, are refused.
    #[test]
    fn thermo_equilibrium_a_wrong_library_is_refused() {
        let library = library();
        let without: Vec<Record> = library
            .iter()
            .filter(|r| r.name != "TiO")
            .cloned()
            .collect();
        assert_eq!(
            fit(&without),
            Err(FitThermoError::Missing {
                name: "TiO".to_owned(),
                phase: Phase::Gas
            })
        );
        let swapped = edited(&library, "VO", |r| {
            r.formula = vec![("V".to_owned(), 1.0), ("O".to_owned(), 2.0)];
        });
        assert!(matches!(
            fit(&swapped),
            Err(FitThermoError::Formula { key, .. }) if key == "VO"
        ));
    }

    /// Each of the fit's other refusals, by its variant: intervals that do not join; a gas named
    /// twice; an item with no interval below 6,000 K; a condensed reference that never meets its
    /// gas; and gas rows that are not the task's.
    #[test]
    fn thermo_equilibrium_each_refusal_names_its_cause() {
        let library = library();
        // Water's second interval starting 1 K above its first's top, 1,000 K.
        let gap = edited(&library, "H2O", |r| r.intervals[1].lower_k += 1.0);
        assert_eq!(
            fit(&gap),
            Err(FitThermoError::Gap {
                item: "H2O".to_owned(),
                at_k: 1_000.0
            })
        );
        let mut twice = library.clone();
        twice.push(library.iter().find(|r| r.name == "H2").unwrap().clone());
        assert_eq!(fit(&twice), Err(FitThermoError::Duplicate("H2".to_owned())));
        let high = edited(&library, "He", |r| {
            r.intervals.retain(|i| i.lower_k() >= UPPER_TEMPERATURE_K);
        });
        assert_eq!(fit(&high), Err(FitThermoError::Empty("HE".to_owned())));
        // Sodium's gas with its S ÷ R raised by 1,000: its G lies below the crystal's everywhere.
        let entropic = edited(&library, "Na", |r| {
            for interval in &mut r.intervals {
                interval.b[1] += 1_000.0;
            }
        });
        assert_eq!(fit(&entropic), Err(FitThermoError::NoSwitch(Element::Na)));
        let rows: Vec<String> = ROWS.iter().map(|&(k, _)| k.to_owned()).collect();
        let mut gas_keys = rows.clone();
        gas_keys.push("FeH".to_owned());
        assert_eq!(modelled_keys(gas_keys.clone()), Ok(rows.clone()));
        assert_eq!(
            modelled_keys(rows.clone()),
            Err(FitThermoError::Rows {
                found: rows.clone()
            })
        );
        gas_keys.retain(|k| k != "CH4");
        let found: Vec<String> = rows.into_iter().filter(|k| k != "CH4").collect();
        assert_eq!(modelled_keys(gas_keys), Err(FitThermoError::Rows { found }));
        assert_eq!(switch_k(&[], &[], 1.0), None);
    }

    #[test]
    fn thermo_equilibrium_item_names_spell_the_keys() {
        assert_eq!(item_name("H2O"), "H2O");
        assert_eq!(item_name("Na+"), "NA_PLUS");
        assert_eq!(item_name("H-"), "H_MINUS");
        assert_eq!(item_name("e-"), "ELECTRON");
        assert_eq!(element_of("CL"), Some(Element::Cl));
        assert_eq!(element_of("AL"), Some(Element::Al));
        assert_eq!(element_of("MG"), Some(Element::Mg));
        assert_eq!(element_of("E"), None);
    }

    /// The switches of sulphur and phosphorus, which are not boiling points, say so in the table.
    #[test]
    fn thermo_equilibrium_the_diatomic_switches_are_labelled() {
        let table = render(&fit(&library()).unwrap());
        for item in &table.items {
            if let TableItem::Scalar { name, doc, .. } = item {
                let labelled = doc.iter().any(|l| l.starts_with("Not a boiling point"));
                assert_eq!(
                    labelled,
                    name == "S_BOILING_K" || name == "P_BOILING_K",
                    "{name}"
                );
            }
        }
    }
}
