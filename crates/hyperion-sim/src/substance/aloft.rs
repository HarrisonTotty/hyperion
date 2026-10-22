//! The laws aloft: where a condensate's vapour saturates in an air or an envelope, each with its
//! limiting vapour, its reaction's other vapours and its sources (plan 14, P14.T49.b's last piece,
//! built with P14.T52.c, whose loop reads them; decision-composition §1.3).
//!
//! # The forms
//!
//! A law gives the saturation vapour pressure of a condensate's limiting vapour by temperature,
//! and so the level of an air at which a vapour of known mole fraction x reaches it, the
//! condensate's cloud base (P14.T52.c), where x P = `p_sat`:
//!
//! - **The vapour law** ([`SaturationLaw::Vapour`]), the plan's form, log₁₀ `p_sat` (bar) = A −
//!   B ÷ T − C \[M\] − D log₁₀ `P_T`, with `P_T` the total pressure in bar and \[M\] the gas's
//!   metallicity in dex, for the metals' and the minerals' clouds (Morley et al. 2012, 2024;
//!   Batalha et al. 2025; Grant et al. 2023; Gao et al. 2020). The term in \[M\] carries the
//!   enrichment of the reaction's other vapours (the H₂S of a sulphide, the H₂O of a silicate)
//!   at the papers' uniform scaling, and D the reaction's dependence on the hydrogen it releases.
//!   Most papers instead publish the condensation curve 10⁴ ÷ `T_cond` = a − b log₁₀ `P_T` − c
//!   \[M\] for the vapour's solar mole fraction `x₀`; the two agree only as the pair (law, `x₀`),
//!   since b = 10⁴ (1 + D) ÷ B, c = 10⁴ (1 + C) ÷ B for one metal atom, and a = 10⁴ (A −
//!   log₁₀ `x₀`) ÷ B. Each law here holds its paper's curve at 1 bar and solar metallicity to
//!   within 10 K with its stated `x₀` (the tests).
//! - **The row's own saturation curve** ([`SaturationLaw::Diagram`]): every row with a
//!   [`PhaseDiagram`](super::PhaseDiagram) (water, methane, ammonia, hydrogen sulphide, carbon
//!   dioxide, nitrogen and the rest, [`condensables`](super::condensables)), whose vapour is the
//!   row itself, over its solid below the triple point and its liquid above, and never at or above
//!   its critical temperature. The volatile decks take it rather than a closed form: the closed
//!   forms for water and ammonia (Lodders and Fegley 2002; Lewis 1969) could not be read, and
//!   the registry's curves are the measured ones (P14.T49.b).
//! - **A product law** ([`SaturationLaw::Product`]), log₁₀ (p₁ p₂) (bar²) = A − B ÷ T, for a salt
//!   that forms from two vapours: ammonium hydrosulphide from NH₃ and H₂S (Kelley 1937, via
//!   Lodders and Fegley 2024). The base is where x₁ x₂ `P_T`² reaches it, and whichever of the two
//!   is scarcer limits it (sulphur on Jupiter, nitrogen on Uranus and Neptune).
//! - **Kulmala and Laaksonen 1990's curve** ([`SaturationLaw::KulmalaLaaksonen`]) for the
//!   sulphuric acid, whose vapour is a share `ε_v` of the sulphur dioxide below the deck
//!   ([`LawAloft::vapour_share`]; P14.T24.g).
//!
//! # The vapours
//!
//! A law's limiting vapour and its reaction's other vapours are [`Reactant`]s, each either an
//! element's whole gas ([`Vapour::Element`]: a metal, every species that carries it, as the papers
//! write its abundance) or one species ([`Vapour::Species`]: H₂O, H₂S, NH₃, Cl's HCl …), with its
//! count in one formula unit of the condensate. A cold trap (P14.T52.c) removes the limiting
//! vapour above the base and the others in proportion.
//!
//! # Not modelled
//!
//! Four condensate rows have no law ([`NOT_MODELLED`]): sodium chloride, iron sulphide and
//! graphite, each for a stated reason, and S₈, the photochemical haze P14.T52.b makes. Two laws are stated but give no deck of a gas that holds
//! the condensate that displaces them ([`LawAloft::displaced_by`]): quartz, whose silicon goes to
//! enstatite wherever enstatite can form (Visscher, Lodders and Fegley 2010, §5.1), and titanium
//! dioxide, whose titanium first condenses as calcium titanates (Lodders 2002, through Wakeford et
//! al. 2017; Lodders and Fegley 2006). A closed set, for the composition audit.
//!
//! # Where they hold
//!
//! The vapour laws were fitted over \[M/H\] −0.5 to +0.5 (Visscher et al. 2006, 2010; Morley et
//! al. 2012) or −1 to +1 (Morley et al. 2024), and over about 10⁻⁶–10⁴ bar and a few hundred to
//! 3,000 K ([`LawAloft::fitted`] states each). A sub-Neptune's envelope at 100 times solar or more is an extrapolation, which
//! P14.T52.c's decks state. Corundum is the first aluminium condensate only below about 0.1 bar
//! (Wakeford et al. 2017, §3.1) or 3 × 10⁻³ bar (Lodders and Fegley 2006, §1.5.2), and perovskite
//! the first titanium condensate only below about 0.03 bar (Lodders and Fegley 2006, §1.5.2); at
//! 1 bar hibonite and Ca₄Ti₃O₁₀ come first, within 10 K of them, so their laws stand in, a
//! recorded approximation. Kelley's ammonium hydrosulphide data span 277–318 K, so every
//! planetary base, at 150–250 K, extrapolates it in temperature.
//!
//! # Sources
//!
//! Each law names its paper's equation. The lane's research (2026-10-14, kept outside the
//! repository with every file's SHA-256) read: Visscher, Lodders and Fegley 2006 (ApJ 648, 1181);
//! Visscher, Lodders and Fegley 2010 (ApJ 716, 1060); Morley et al. 2012 (ApJ 756, 172); Morley et
//! al. 2014 (ApJL 787, L78); Morley et al. 2024 (Sonora Diamondback, ApJ, arXiv:2402.00758);
//! Batalha et al. 2025 (Virga, arXiv:2508.15102); Wakeford et al. 2017 (MNRAS 464, 4247); Grant
//! et al. 2023 (ApJL 956, L29); Gao et al. 2020 (Nature Astronomy 4, 951, its Supplementary Tables
//! 2–4); Lodders and Fegley 2006 (Astrophysics Update 2); Lodders and Fegley 2024 (Sulfur in the
//! giant planets, arXiv:2410.11138); Adams and Lodders 2025 (ApJ, arXiv:2411.11832); Ackerman and
//! Marley 2001 (ApJ 556, 872); Dai et al. 2022 (JGR Planets, arXiv:2207.10238) and Dai, Zhang and
//! Cui 2022 (MNRAS, arXiv:2207.10243), through which Kulmala and Laaksonen 1990 (J. Chem. Phys.
//! 93, 696) was read; Vehkamäki et al. 2002 (JGR 107(D22), 4622); and, checked and not used,
//! Lodders 2010, Marley et al. 2021, Visscher et al. 2005, Parmentier et al. 2018 and Burningham
//! et al. 2021.
//!
//! Nothing on the record reads these laws before P14.T54.b wires P14.T52.c's loop
//! (decision-bump-plan-after-22 §1.3), so no generated output moves with them.

use super::{Basis, Element, Source, SubstanceId, SubstanceKey, by_key, ids};
use crate::math;
use crate::units::{Kelvin, Pascals};

/// Pascals per bar, the vapour laws' unit.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// Pascals per standard atmosphere, Kulmala and Laaksonen's unit.
const PASCALS_PER_ATMOSPHERE: f64 = 101_325.0;

/// The vapour a law reads: an element's whole gas, or one species.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Vapour {
    /// Every gas species that carries the element, by its atoms: a metal's vapour, as the papers
    /// write its abundance (Mg for forsterite, Si as SiO for enstatite, Ti as TiO for perovskite).
    Element(Element),
    /// One gas species, by its key.
    Species(SubstanceKey),
}

/// A vapour of a condensate's reaction and its count in one formula unit of the condensate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reactant {
    vapour: Vapour,
    count: u16,
}

impl Reactant {
    /// The vapour.
    #[must_use]
    pub const fn vapour(&self) -> Vapour {
        self.vapour
    }

    /// Its count in one formula unit of the condensate: atoms for [`Vapour::Element`], molecules
    /// for [`Vapour::Species`].
    #[must_use]
    pub const fn count(&self) -> u16 {
        self.count
    }
}

/// A reactant of `count` atoms of `element`'s gas.
const fn element(element: Element, count: u16) -> Reactant {
    Reactant {
        vapour: Vapour::Element(element),
        count,
    }
}

/// A reactant of `count` molecules of the species `key`.
const fn species(key: &str, count: u16) -> Reactant {
    Reactant {
        vapour: Vapour::Species(SubstanceKey::new_const(key)),
        count,
    }
}

/// How a law gives the limiting vapour's saturation pressure (the [module](self)'s forms).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SaturationLaw {
    /// log₁₀ `p_sat` (bar) = a − b ÷ T − c \[M\] − d log₁₀ `P_T` (bar): the plan's form, its
    /// coefficients the papers' A, B, C and D.
    Vapour {
        /// A, dimensionless.
        a: f64,
        /// B, kelvin.
        b: f64,
        /// C, per dex of metallicity.
        c: f64,
        /// D, per dex of total pressure.
        d: f64,
    },
    /// The row's own phase diagram's saturation pressure, over its solid below the triple point
    /// and its liquid above; no condensation at or above its critical temperature.
    Diagram,
    /// log₁₀ (p₁ p₂) (bar²) = a − b ÷ T, the product of the two reactants' partial pressures.
    Product {
        /// A, dimensionless.
        a: f64,
        /// B, kelvin.
        b: f64,
    },
    /// Kulmala and Laaksonen 1990's curve over concentrated sulphuric acid: ln `p_sat` (atm) =
    /// 16.259 − 10156 ÷ T₀ + 10156 \[−1 ÷ T + 1 ÷ T₀ + 0.38 ÷ (`T_c` − T₀) (1 + ln(T₀ ÷ T) −
    /// T₀ ÷ T)\], with T₀ = 360 K and `T_c` = 905 K ([`KULMALA_LAAKSONEN`]).
    KulmalaLaaksonen,
}

/// Kulmala and Laaksonen 1990's constants (J. Chem. Phys. 93, 696; closed access, read through
/// Dai, Zhang, Shao, Bierson and Cui 2022, JGR Planets, doi:10.1029/2021JE007060, their eq. 12,
/// and Dai, Zhang and Cui 2022, MNRAS, their eq. 16; plan 14, P14.T24.g): Ayers, Gillett and Gras
/// 1980's 16.259 and 10156 K (98 wt% acid at 338–445 K, GRL 7, 433, not read), the curvature's
/// 0.38, T₀ = 360 K and `T_c` = 905 K. The pressure is in standard atmospheres: Vehkamäki et
/// al. 2002 (JGR 107(D22), 4622, their eq. 8) multiply the same exponential by 101,325 to give
/// pascals, and Ayers's fit is in atmospheres; Dai et al.'s "bar" is their relabelling (1.3% in
/// p). Vehkamäki et al.'s own refit (T₀ 360.15 K, L 11.695) is a later curve, not this one.
pub const KULMALA_LAAKSONEN: KulmalaLaaksonenConstants = KulmalaLaaksonenConstants {
    ayers_constant: 16.259,
    ayers_slope_k: 10_156.0,
    curvature: 0.38,
    reference_k: 360.0,
    critical_k: 905.0,
};

/// The constants of Kulmala and Laaksonen 1990's curve ([`KULMALA_LAAKSONEN`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KulmalaLaaksonenConstants {
    /// Ayers et al.'s ln p at 1 ÷ T = 0, p in atmospheres.
    pub ayers_constant: f64,
    /// Ayers et al.'s slope, kelvin: ΔH ÷ R.
    pub ayers_slope_k: f64,
    /// The curvature term's coefficient.
    pub curvature: f64,
    /// T₀, kelvin, where the curve meets Ayers et al.'s.
    pub reference_k: f64,
    /// `T_c`, kelvin, the acid's critical temperature as the curve takes it.
    pub critical_k: f64,
}

impl KulmalaLaaksonenConstants {
    /// ln(`p_sat` ÷ 1 atm) at `t` kelvin.
    #[must_use]
    fn ln_atm(&self, t: f64) -> f64 {
        let t0 = self.reference_k;
        let curve = self.curvature / (self.critical_k - t0) * (1.0 + math::ln(t0 / t) - t0 / t);
        self.ayers_constant - self.ayers_slope_k / t0
            + self.ayers_slope_k * (-1.0 / t + 1.0 / t0 + curve)
    }
}

/// The share of the sulphur dioxide below the acid deck that is acid vapour, `ε_v` (plan 14,
/// P14.T24.g: "The acid vapour below the deck is `ε_v` `x_SO₂`, with `ε_v` = 0.06", which gives Venus
/// 7.8 ppm against the 5–10 ppm measured and modelled under its clouds, Dai et al. 2022). P14.T24.g
/// owns it and its assertion; the law states it.
pub const SULPHURIC_ACID_VAPOUR_SHARE: f64 = 0.06;

/// A condensate's law aloft (the [module](self)'s documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LawAloft {
    condensate: SubstanceKey,
    law: SaturationLaw,
    limiting: Reactant,
    others: &'static [Reactant],
    vapour_share: f64,
    displaced_by: Option<SubstanceKey>,
    fitted: Option<Fitted>,
    reference: Option<Reference>,
    sources: &'static [Source],
}

/// The metallicities, total pressures and temperatures a law was fitted over, or its data span:
/// each read off the paper's figures where it states none, and unbounded where the law has no
/// such term (a product law's metallicity, the acid's pressure).
///
/// The fields are plain data with no invariant between them, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fitted {
    /// The lowest and highest \[M/H\], dex.
    pub metallicity_dex: (f64, f64),
    /// The lowest and highest total pressure.
    pub total_pressure: (Pascals, Pascals),
    /// The lowest and highest temperature.
    pub temperature: (Kelvin, Kelvin),
}

/// A law's check against its paper: the vapour's solar mole fraction `x₀` with which it holds the
/// paper's curve, and the curve's condensation temperature at 1 bar and solar metallicity.
///
/// The fields are plain data with no invariant between them, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reference {
    /// `x₀`: for a product law, the product of the two reactants' mole fractions.
    pub solar_mole_fraction: f64,
    /// The paper's condensation temperature at 1 bar and \[M\] = 0.
    pub condensation_temperature: Kelvin,
}

impl LawAloft {
    /// The condensate's key.
    #[must_use]
    pub const fn condensate(&self) -> SubstanceKey {
        self.condensate
    }

    /// How the law gives the saturation pressure.
    #[must_use]
    pub const fn law(&self) -> SaturationLaw {
        self.law
    }

    /// The limiting vapour, whose saturation the law gives: for a product law, the first of its
    /// two reactants (the scarcer of them limits the deck).
    #[must_use]
    pub const fn limiting(&self) -> Reactant {
        self.limiting
    }

    /// The reaction's other vapours, removed in proportion to the limiting one: for a product
    /// law, the second reactant.
    #[must_use]
    pub const fn others(&self) -> &'static [Reactant] {
        self.others
    }

    /// The share of the limiting reactant that is the condensing vapour: 1, or for the sulphuric
    /// acid `ε_v` ([`SULPHURIC_ACID_VAPOUR_SHARE`]) of the sulphur dioxide.
    #[must_use]
    pub const fn vapour_share(&self) -> f64 {
        self.vapour_share
    }

    /// The condensate whose deck takes this one's limiting vapour wherever it can form, so that
    /// this law gives no deck of a gas that also holds that one's: quartz's enstatite and titanium
    /// dioxide's perovskite (the [module](self)'s documentation).
    #[must_use]
    pub const fn displaced_by(&self) -> Option<SubstanceKey> {
        self.displaced_by
    }

    /// What the law was fitted over; `None` for the rows' own saturation curves, which hold over
    /// their diagrams.
    #[must_use]
    pub const fn fitted(&self) -> Option<Fitted> {
        self.fitted
    }

    /// The law's check against its paper; `None` where the paper gives no condensation
    /// temperature to check (titanium dioxide, the acid, the rows' own curves).
    #[must_use]
    pub const fn reference(&self) -> Option<Reference> {
        self.reference
    }

    /// The law's sources.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }

    /// ln(`p_sat` ÷ 1 Pa) of the condensing vapour at `temperature` under the total pressure
    /// `total_pressure` in a gas of metallicity `metallicity_dex` (\[M\], dex); for a product law,
    /// ln(p₁ p₂ ÷ 1 Pa²).
    ///
    /// `None` where the vapour does not condense: for a row's own curve at or above its critical
    /// temperature, and for a temperature or a pressure that is not finite and positive.
    #[must_use]
    pub fn ln_saturation_pressure(
        &self,
        temperature: Kelvin,
        total_pressure: Pascals,
        metallicity_dex: f64,
    ) -> Option<f64> {
        let t = temperature.value();
        let p = total_pressure.value();
        if !(t.is_finite() && t > 0.0 && p.is_finite() && p > 0.0 && metallicity_dex.is_finite()) {
            return None;
        }
        let ln_bar = math::ln(PASCALS_PER_BAR);
        Some(match self.law {
            SaturationLaw::Vapour { a, b, c, d } => {
                let log_total = math::log10(p / PASCALS_PER_BAR);
                (a - b / t - c * metallicity_dex - d * log_total) * core::f64::consts::LN_10
                    + ln_bar
            }
            SaturationLaw::Product { a, b } => {
                (a - b / t) * core::f64::consts::LN_10 + 2.0 * ln_bar
            }
            SaturationLaw::KulmalaLaaksonen => {
                KULMALA_LAAKSONEN.ln_atm(t) + math::ln(PASCALS_PER_ATMOSPHERE)
            }
            SaturationLaw::Diagram => by_key(self.condensate.as_str())?
                .substance()
                .phase_diagram()?
                .ln_saturation_pressure(temperature)?,
        })
    }

    /// The temperature at which a vapour of mole fraction `vapour` (for a product law, the product
    /// of the two reactants' mole fractions) saturates under the total pressure `total_pressure`
    /// in a gas of metallicity `metallicity_dex`: the condensation temperature a deck's base sits
    /// at.
    ///
    /// The vapour and product laws are solved exactly, the row's own curve by its
    /// [`saturation_temperature`](super::PhaseDiagram::saturation_temperature), and Kulmala and
    /// Laaksonen's curve, which rises monotonically, by 64 bisections over 50–905 K. `None` where
    /// no temperature saturates it: a mole fraction or a pressure that is not finite and positive,
    /// a row's curve beyond its critical point, or the acid outside 50–905 K.
    #[must_use]
    pub fn condensation_temperature(
        &self,
        total_pressure: Pascals,
        vapour: f64,
        metallicity_dex: f64,
    ) -> Option<Kelvin> {
        let p = total_pressure.value();
        if !(p.is_finite() && p > 0.0 && vapour.is_finite() && vapour > 0.0) {
            return None;
        }
        let bars = p / PASCALS_PER_BAR;
        match self.law {
            SaturationLaw::Vapour { a, b, c, d } => {
                let log_total = math::log10(bars);
                let denominator =
                    a - c * metallicity_dex - d * log_total - math::log10(vapour * bars);
                (denominator > 0.0).then(|| Kelvin::new(b / denominator))
            }
            SaturationLaw::Product { a, b } => {
                let denominator = a - math::log10(vapour * bars * bars);
                (denominator > 0.0).then(|| Kelvin::new(b / denominator))
            }
            SaturationLaw::Diagram => by_key(self.condensate.as_str())?
                .substance()
                .phase_diagram()?
                .saturation_temperature(Pascals::new(vapour * p)),
            SaturationLaw::KulmalaLaaksonen => {
                let target = math::ln(vapour * p / PASCALS_PER_ATMOSPHERE);
                let (mut cold, mut warm) = (50.0, KULMALA_LAAKSONEN.critical_k);
                if !(KULMALA_LAAKSONEN.ln_atm(cold) <= target
                    && target < KULMALA_LAAKSONEN.ln_atm(warm))
                {
                    return None;
                }
                for _ in 0..64 {
                    let middle = f64::midpoint(cold, warm);
                    if KULMALA_LAAKSONEN.ln_atm(middle) < target {
                        cold = middle;
                    } else {
                        warm = middle;
                    }
                }
                Some(Kelvin::new(f64::midpoint(cold, warm)))
            }
        }
    }
}

/// A source of `covers`, cited as `citation`, with `basis`.
const fn source(covers: &'static str, citation: &'static str, basis: Basis) -> Source {
    Source {
        covers,
        citation,
        basis,
    }
}

/// A fitted range of \[M/H\] `low`–`high` dex, `top_bar`–`bottom_bar` bar and `cold_k`–`hot_k`
/// kelvin.
const fn fitted(
    (low, high): (f64, f64),
    (top_bar, bottom_bar): (f64, f64),
    (cold_k, hot_k): (f64, f64),
) -> Fitted {
    Fitted {
        metallicity_dex: (low, high),
        total_pressure: (
            Pascals::new(top_bar * PASCALS_PER_BAR),
            Pascals::new(bottom_bar * PASCALS_PER_BAR),
        ),
        temperature: (Kelvin::new(cold_k), Kelvin::new(hot_k)),
    }
}

/// No bound.
const UNBOUNDED: (f64, f64) = (f64::NEG_INFINITY, f64::INFINITY);

/// Visscher, Lodders and Fegley 2006's Fig. 1 domain, log₁₀ `P_T` −6 to +4 and about 300–2,500 K
/// (read off the figure by the lane's research), at their \[M/H\] −0.5 to +0.5.
const VISSCHER_2006: Fitted = fitted((-0.5, 0.5), (1e-6, 1e4), (300.0, 2_500.0));

/// Visscher, Lodders and Fegley 2010's overview, log₁₀ `P_T` −4 to +4 and 800–2,500 K (read off
/// their figures), at Morley et al. 2024's \[M/H\] −1 to +1, over which Diamondback's laws are
/// fitted.
const DIAMONDBACK: Fitted = fitted((-1.0, 1.0), (1e-4, 1e4), (800.0, 2_500.0));

/// Wakeford et al. 2017's Figs. 1–2, 10⁻⁶–10² bar and 1,000–3,000 K, computed at \[M/H\] 0, 0.5
/// and 1.0.
const WAKEFORD: Fitted = fitted((0.0, 1.0), (1e-6, 1e2), (1_000.0, 3_000.0));

/// A check of `x0` against the paper's `t_k` kelvin at 1 bar.
const fn reference(x0: f64, t_k: f64) -> Reference {
    Reference {
        solar_mole_fraction: x0,
        condensation_temperature: Kelvin::new(t_k),
    }
}

/// A vapour law of coefficients `a`, `b`, `c` and `d`.
const fn vapour_law(a: f64, b: f64, c: f64, d: f64) -> SaturationLaw {
    SaturationLaw::Vapour { a, b, c, d }
}

/// The laws with a closed form, in the registry's order of their condensates.
const LAWS: [LawAloft; 14] = [
    LawAloft {
        condensate: SubstanceKey::new_const("NH4SH"),
        law: SaturationLaw::Product {
            a: 14.89,
            b: 4_732.0,
        },
        limiting: species("NH3", 1),
        others: &[species("H2S", 1)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(fitted(UNBOUNDED, (1e-4, 1e2), (277.35, 317.55))),
        // X_NH₃ X_H₂S = 10^−8.378 at solar (Lodders 2020, via Lodders and Fegley 2024), and their
        // Table 2's 203 K at 1 bar.
        reference: Some(reference(4.188e-9, 203.0)),
        sources: &[source(
            "the law, its reaction and its check",
            "Kelley 1937's dissociation pressure of NH₄SH, log₁₀(p_NH₃ p_H₂S) = 14.89 − 4732 ÷ T \
             (bar², the units not stated; atm² would move T by about 0.1 K), through K. Lodders and \
             B. Fegley, 'Sulfur in the giant planets', arXiv:2410.11138 (2024), pp. 9–10 and Table \
             2 (203 K at 1 bar and solar, 151–246 K over 10⁻⁴–10² bar, which the law holds within \
             0.5 K); NH₃ + H₂S = NH₄SH(s), Visscher, Lodders and Fegley 2006, ApJ 648, 1181, eq. \
             31, whose curve, eq. 32, gives 204.5 K. Kelley's data span 4.2–44.4 °C",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("H2SO4"),
        law: SaturationLaw::KulmalaLaaksonen,
        limiting: species("SO2", 1),
        others: &[],
        vapour_share: SULPHURIC_ACID_VAPOUR_SHARE,
        displaced_by: None,
        // Ayers, Gillett and Gras 1980's measured span, 338–445 K (not read).
        fitted: Some(fitted(UNBOUNDED, (0.0, f64::INFINITY), (338.0, 445.0))),
        reference: None,
        sources: &[
            source(
                "the curve",
                "Kulmala and Laaksonen 1990, J. Chem. Phys. 93, 696 (closed access), read through \
                 Dai, Zhang, Shao, Bierson and Cui 2022, JGR Planets, doi:10.1029/2021JE007060, eq. \
                 12, and Dai, Zhang and Cui 2022, MNRAS, eq. 16; in atmospheres, as Vehkamäki et \
                 al. 2002, JGR 107(D22), 4622, eq. 8 write it (KULMALA_LAAKSONEN)",
                Basis::Fact,
            ),
            source(
                "the vapour",
                "ε_v = 0.06 of the sulphur dioxide below the deck, plan 14's P14.T24.g, which owns \
                 the deck's assertion",
                Basis::Adopted,
            ),
        ],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("KCl"),
        law: vapour_law(7.611, 11_382.0, 0.0, 0.0),
        limiting: element(Element::K, 1),
        others: &[element(Element::Cl, 1)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(VISSCHER_2006),
        reference: Some(reference(2.55e-7, 801.35)),
        sources: &[source(
            "the law, its reaction and its check",
            "C. V. Morley, J. J. Fortney, M. S. Marley, C. Visscher, D. Saumon and S. K. Leggett, \
             ApJ 756, 172 (2012), §2.4.2: log₁₀ p_KCl = 7.611 − 11382 ÷ T (eq. 18), KCl = KCl(s) \
             (eq. 17), x₀ = 2.55 × 10⁻⁷ (Table 3), and the curve 10⁴ ÷ T = 12.479 − 0.879 log₁₀ \
             P_T − 0.879 [Fe/H] (eq. 20), 801 K at 1 bar; K is 70% of Cl in a solar gas, so K \
             limits it, and KCl must be K's dominant gas at the base (after eq. 18)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("ZnS"),
        law: vapour_law(12.812, 15_873.0, 1.0, 0.0),
        limiting: element(Element::Zn, 1),
        others: &[species("H2S", 1)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(VISSCHER_2006),
        reference: Some(reference(8.45e-8, 798.72)),
        sources: &[source(
            "the law, its reaction and its check",
            "Morley et al. 2012, ApJ 756, 172, eq. 15 (log₁₀ p_Zn = 12.812 − 15873 ÷ T − [Fe/H]) \
             with x₀ = 8.45 × 10⁻⁸ (Table 3); Zn + H₂S = ZnS(s) + H₂ and the curve 10⁴ ÷ T = 12.52 \
             − 0.63 log₁₀ P_T − 1.26 [M/H], 799 K at 1 bar (Visscher, Lodders and Fegley 2006, ApJ \
             648, 1181, §3.2.3, eqs. 29–30)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("Na2S"),
        law: vapour_law(8.55, 13_889.0, 0.5, 0.0),
        limiting: element(Element::Na, 2),
        others: &[species("H2S", 1)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(VISSCHER_2006),
        reference: Some(reference(3.97e-6, 995.02)),
        sources: &[source(
            "the law, its reaction and its check",
            "Morley et al. 2012, ApJ 756, 172, eq. 12 (log₁₀ p_Na = 8.55 − 13889 ÷ T − 0.5 [Fe/H]) \
             with x₀ = 3.97 × 10⁻⁶ (Table 3); 2Na + H₂S = Na₂S(s) + H₂ and the curve 10⁴ ÷ T = 10.05 \
             − 0.72 log₁₀ P_T − 1.08 [M/H], 995 K at 1 bar (Visscher et al. 2006, §3.2.2, eqs. \
             27–28)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("MnS"),
        law: vapour_law(11.532, 23_810.0, 1.0, 0.0),
        limiting: element(Element::Mn, 1),
        others: &[species("H2S", 1)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(VISSCHER_2006),
        reference: Some(reference(6.32e-7, 1_342.28)),
        sources: &[source(
            "the law, its reaction and its check",
            "Morley et al. 2012, ApJ 756, 172, eq. 9 (log₁₀ p_Mn = 11.532 − 23810 ÷ T − [Fe/H]) \
             with x₀ = 6.32 × 10⁻⁷ (Table 3); Mn + H₂S = MnS(s) + H₂ and the curve 10⁴ ÷ T = 7.45 − \
             0.42 log₁₀ P_T − 0.84 [M/H], 1,342 K at 1 bar (Visscher et al. 2006, §3.2.1, eqs. \
             25–26)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("Cr"),
        law: vapour_law(7.490, 20_592.0, 0.0, 0.0),
        limiting: element(Element::Cr, 1),
        others: &[],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(fitted((-0.5, 0.5), (1e-3, 1e4), (300.0, 2_500.0))),
        reference: Some(reference(8.87e-7, 1_520.68)),
        sources: &[source(
            "the law, its reaction and its check",
            "Morley et al. 2012, ApJ 756, 172, §2.4.2: log₁₀ p_Cr = 7.490 − 20592 ÷ T (eq. 5), Cr = \
             Cr(s) (eq. 2), x₀ = 10^−6.052 (eq. 6), and the curve 10⁴ ÷ T = 6.576 − 0.486 log₁₀ P_T \
             − 0.486 [Fe/H] (eq. 7), 1,521 K at 1 bar; Cr₂O₃ replaces the metal near 10⁻³ bar \
             (Lodders and Fegley 2006, §1.5.3)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("Fe"),
        law: vapour_law(7.09, 20_995.0, 0.0, 0.0),
        limiting: element(Element::Fe, 1),
        others: &[],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(DIAMONDBACK),
        reference: Some(reference(5.095e-5, 1_838.24)),
        sources: &[source(
            "the law, its reaction and its check",
            "C. V. Morley et al. 2024 (Sonora Diamondback), arXiv:2402.00758, §2.2.2 eq. 4, and \
             Batalha et al. 2025 (Virga), arXiv:2508.15102, Table 2: log₁₀ p_Fe = 7.09 − 20995 ÷ \
             T, with Batalha et al.'s x₀ = 50.95 ppm; Fe = Fe(s,l), liquid above 1,809 K, and the \
             curve 10⁴ ÷ T = 5.44 − 0.48 log₁₀ P_T − 0.48 [Fe/H], 1,838 K at 1 bar (Visscher, \
             Lodders and Fegley 2010, ApJ 716, 1060, §3.1, eqs. 1–2). With Morley et al. 2012's \
             x₀ = 57.8 ppm the law lands 15 K above the curve",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("MgSiO3"),
        law: vapour_law(13.43, 28_665.0, 1.0, 0.0),
        limiting: element(Element::Si, 1),
        others: &[element(Element::Mg, 1), species("H2O", 2)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(DIAMONDBACK),
        reference: Some(reference(2.92e-5, 1_597.44)),
        sources: &[source(
            "the law, its reaction and its check",
            "Batalha et al. 2025 (Virga), arXiv:2508.15102, Table 2: log₁₀ p_SiO = 13.43 − 28665 ÷ \
             T − [M/H], with x₀ = 29.2 ppm, the silicon forsterite leaves (Morley et al. 2024, \
             arXiv:2402.00758, eq. 3, prints it with no [M/H] term, against Visscher et al. 2010's \
             c ÷ b = 2); Si limits it, as SiO, after forsterite takes the Mg (Morley et al. 2024, \
             §2.2.2); Mg + 2H₂O + SiO = MgSiO₃(s,l) + 2H₂ and the curve 10⁴ ÷ T = 6.26 − 0.35 \
             log₁₀ P_T − 0.70 [Fe/H], 1,597 K at 1 bar, 'plausibly' by SiO reacting with forsterite \
             grains (Visscher, Lodders and Fegley 2010, ApJ 716, 1060, §4.1, eqs. 19–20, §5.1)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("Mg2SiO4"),
        law: vapour_law(14.88, 32_488.0, 1.4, 0.2),
        limiting: element(Element::Mg, 2),
        others: &[element(Element::Si, 1), species("H2O", 3)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(DIAMONDBACK),
        reference: Some(reference(5.936e-5, 1_697.79)),
        sources: &[source(
            "the law, its reaction and its check",
            "Morley et al. 2024 (Sonora Diamondback), arXiv:2402.00758, §2.2.2 eq. 2: log₁₀ p_Mg = \
             14.88 − 32488 ÷ T − 1.4 [M/H] − 0.2 log₁₀ P_T (Gao et al. 2020's Supplementary Table 2 \
             prints 4.88, a typo), with x₀ = 59.36 ppm (Gao et al. 2020, Nature Astronomy 4, 951, \
             Supplementary Table 4); 2Mg + 3H₂O + SiO = Mg₂SiO₄(s,l) + 3H₂ and the curve 10⁴ ÷ T = \
             5.89 − 0.37 log₁₀ P_T − 0.73 [Fe/H], 1,698 K at 1 bar (Visscher, Lodders and Fegley \
             2010, ApJ 716, 1060, §4.1, eqs. 17–18); Mg limits it, and it takes nearly half the Si \
             (§5.1)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("SiO2"),
        law: vapour_law(13.168, 28_265.0, 1.0, 0.0),
        limiting: element(Element::Si, 1),
        others: &[species("H2O", 1)],
        vapour_share: 1.0,
        displaced_by: Some(SubstanceKey::new_const("MgSiO3")),
        fitted: Some(fitted((-0.5, 0.5), (1e-4, 1e2), (800.0, 2_500.0))),
        reference: Some(reference(6.03e-5, 1_628.66)),
        sources: &[source(
            "the law, its reaction, its check and its displacement",
            "D. Grant et al. 2023, ApJL 956, L29, §3.2: log₁₀ p_SiO = 13.168 − 28265 ÷ T − [Fe/H] \
             (eq. 3) with x₀ = 60.3 ppm (Batalha et al. 2025, Table 2), SiO + H₂O = SiO₂(s) + H₂, \
             and the curve 10⁴ ÷ T = 6.14 − 0.35 log₁₀ P_T − 0.70 [Fe/H] (eq. 4), 1,629 K at 1 bar, \
             for undepleted gas; Visscher, Lodders and Fegley 2010, ApJ 716, 1060, §5.1: quartz \
             does not form where enstatite can, appearing near 1,550 K only after forsterite with \
             enstatite suppressed; a gas poor in Mg (Burningham et al. 2021) is not modelled",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("Al2O3"),
        law: vapour_law(17.7, 45_893.0, 1.66, 0.0),
        limiting: element(Element::Al, 2),
        others: &[species("H2O", 3)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(WAKEFORD),
        reference: Some(reference(4.937e-6, 1_994.42)),
        sources: &[source(
            "the law, its reaction and its check",
            "P. Gao et al. 2020, Nature Astronomy 4, 951, Supplementary Table 2: log₁₀ p_Al = 17.7 − \
             45893 ÷ T − 1.66 [Fe/H], with x₀ = 4.937 ppm (Supplementary Table 4), the linear part \
             of H. R. Wakeford et al. 2017, MNRAS 464, 4247, §3.1, eq. 4: 10⁴ ÷ T = 5.014 − 0.2179 \
             log₁₀ P_T − 0.580 [Fe/H] + 2.264 × 10⁻³ (log₁₀ P_T)², 1,994 K at 1 bar, computed at \
             [M/H] 0, 0.5 and 1.0; 2Al + 3H₂O = Al₂O₃(s) + 3H₂, Al limiting (Morley et al. 2024, \
             §2.2.2); corundum is the first Al condensate only below about 0.1 bar, hibonite \
             (2,001 K at 1 bar) and grossite above (Wakeford et al. 2017, §3.1)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("CaTiO3"),
        law: vapour_law(30.24, 72_160.0, 3.0, 1.0),
        limiting: element(Element::Ti, 1),
        others: &[element(Element::Ca, 1), species("H2O", 2)],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: Some(WAKEFORD),
        reference: Some(reference(1.69e-7, 1_951.22)),
        sources: &[source(
            "the law, its reaction and its check",
            "Batalha et al. 2025 (Virga), arXiv:2508.15102, eqs. 18–19 and Table 2: log₁₀ p_TiO = \
             30.24 − 72160 ÷ T − 3 [M/H] − log₁₀ P_T, Ca + TiO + 2H₂O = CaTiO₃(c) + 2H₂, Ti \
             limiting, with x₀ = 0.169 ppm (its Table 2's 2.51 ppm is not the TiO's fraction); the \
             curve 10⁴ ÷ T = 5.125 − 0.277 log₁₀ P_T − 0.554 [Fe/H], 1,951 K at 1 bar (Wakeford et \
             al. 2017, MNRAS 464, 4247, §3.1, eq. 7); perovskite is the first Ti condensate only \
             below about 0.03 bar, Ca₄Ti₃O₁₀ (1,956 K at 1 bar) above (Lodders and Fegley 2006, \
             §1.5.2)",
            Basis::Fact,
        )],
    },
    LawAloft {
        condensate: SubstanceKey::new_const("TiO2"),
        law: vapour_law(13.95, 38_266.0, 1.0, 0.0),
        limiting: element(Element::Ti, 1),
        others: &[species("H2O", 1)],
        vapour_share: 1.0,
        displaced_by: Some(SubstanceKey::new_const("CaTiO3")),
        fitted: None,
        reference: None,
        sources: &[source(
            "the law and its displacement",
            "Batalha et al. 2025 (Virga), arXiv:2508.15102, Table 2: log₁₀ p_TiO = 13.95 − 38266 ÷ \
             T − [M/H] ('cf. Ma21, Lo02'; Marley et al. 2021 gives no closed form), 1,847 K at 1 \
             bar with x₀ = 0.169 ppm, no paper's curve to check; TiO + H₂O = TiO₂(s) + H₂ is implied \
             by its [M/H] term. Titanium first condenses as calcium titanates (Lodders 2002, through \
             Wakeford et al. 2017, Table 1; Lodders and Fegley 2006, §1.5.2), so in a gas with \
             perovskite's law this one gives no deck",
            Basis::Fact,
        )],
    },
];

/// A condensate row with no law aloft, and why (the [module](self)'s documentation).
///
/// The fields are plain data with no invariant between them, so they are public.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotModelledAloft {
    /// The row's key.
    pub key: &'static str,
    /// Why it has no law.
    pub reason: &'static str,
}

/// The condensate rows of the registry that have no law aloft. A closed set, for the composition
/// audit.
pub const NOT_MODELLED: [NotModelledAloft; 4] = [
    NotModelledAloft {
        key: "NaCl",
        reason: "no curve in the papers read; in a solar gas sodium condenses as Na₂S and NaCl \
                 stays a gas (Visscher, Lodders and Fegley 2006, §3.2.2; Lodders and Fegley 2006, \
                 §1.5.4)",
    },
    NotModelledAloft {
        key: "FeS",
        reason: "no closed form: Fe + H₂S = FeS near 700 K is independent of pressure and needs \
                 iron grains, which have rained out of a planet's air (Visscher et al. 2006, §3.2; \
                 Lodders and Fegley 2024, p. 8)",
    },
    NotModelledAloft {
        key: "S8",
        reason: "a photochemical haze, not a deck of a deep vapour: P14.T52.b's sulphur haze \
                 (Gao et al. 2017), made from H₂S aloft on giants at 250–700 K",
    },
    NotModelledAloft {
        key: "graphite",
        reason: "no curve for a planet's air: Adams and Lodders 2025 (arXiv:2411.11832) fit \
                 graphite only for carbon stars' envelopes, at C/O 1.2 and 10⁻¹⁰–10⁻⁴ bar",
    },
];

/// The sources of every row's own saturation curve: its phase diagram's.
const DIAGRAM: [Source; 1] = [source(
    "the law",
    "the row's own phase diagram's saturation curve (P14.T49.b; PhaseDiagram's sources)",
    Basis::Reduced,
)];

/// The law aloft of the row `id`: its closed form, or its own saturation curve if it has a phase
/// diagram; `None` for a row with neither.
///
/// # Examples
///
/// Potassium chloride condenses near 800 K under a bar of solar gas (Morley et al. 2012):
///
/// ```
/// use hyperion_sim::substance::aloft::law_aloft;
/// use hyperion_sim::substance::by_key;
/// use hyperion_sim::units::Pascals;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let law = law_aloft(by_key("KCl")?)?;
/// let base = law.condensation_temperature(Pascals::new(1e5), 2.55e-7, 0.0)?;
/// assert!((base.value() - 801.3).abs() < 0.5);
/// // Water's own curve: its frost point under a millibar of vapour.
/// let water = law_aloft(by_key("H2O")?)?;
/// let frost = water.condensation_temperature(Pascals::new(1e5), 1e-3, 0.0)?;
/// assert!((250.0..255.0).contains(&frost.value()));
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn law_aloft(id: SubstanceId) -> Option<LawAloft> {
    let row = id.substance();
    let key = row.key();
    if let Some(law) = LAWS
        .iter()
        .find(|law| law.condensate.as_str() == key.as_str())
    {
        return Some(*law);
    }
    row.phase_diagram().map(|_| LawAloft {
        condensate: key,
        law: SaturationLaw::Diagram,
        limiting: Reactant {
            vapour: Vapour::Species(key),
            count: 1,
        },
        others: &[],
        vapour_share: 1.0,
        displaced_by: None,
        fitted: None,
        reference: None,
        sources: &DIAGRAM,
    })
}

/// Every row with a law aloft and its law, in registry order.
pub fn laws_aloft() -> impl Iterator<Item = (SubstanceId, LawAloft)> {
    ids().filter_map(|id| law_aloft(id).map(|law| (id, law)))
}

#[cfg(test)]
mod tests;
