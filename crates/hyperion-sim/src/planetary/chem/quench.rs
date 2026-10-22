//! Disequilibrium by vertical mixing (plan 14, P14.T52.b; decision-composition §1.3): where an
//! envelope's CO, CH₄, NH₃, HCN and CO₂ stop following the local equilibrium as the gas rises, by
//! Zahnle and Marley's quench relations, and the gas above those levels, on the Surface path.
//!
//! Nothing on the record reads it until P14.T54.b (P14.T52.d calls it), so no generated output
//! moves with it.
//!
//! # The quench approximation
//!
//! A parcel mixed upward cools, and a reaction whose chemical time exceeds the mixing time stops
//! following the equilibrium: "the disequilibrium composition that one can observe is
//! approximated by the equilibrium composition when the relevant chemical reaction time scale
//! tchem equals the relevant cooling timescale, which if due to mixing can be written tmix (Prinn
//! and Barshay 1977)" (K. J. Zahnle and M. S. Marley, "Methane, carbon monoxide, and ammonia in
//! brown dwarfs and self-luminous giant planets", ApJ 797, 41, 2014, arXiv:1408.6283v2, §1;
//! hereafter ZM14). Above its quench level a system's mixing ratios are those of the equilibrium
//! there.
//!
//! - **The chemical times** are ZM14's fits to their full kinetics network, t = A p^−b m^−c
//!   exp(B ÷ T) (their eq. 10), p in bar, T in K, m the metallicity as a multiple of solar
//!   ([`ChemicalTime`], [`QuenchedSystem::chemical_time`]):
//!
//!   | System | A, s bar^b | b | c | B, K | ZM14 |
//!   | --- | --- | --- | --- | --- | --- |
//!   | CO–CH₄, warm | 1.5 × 10⁻⁶ | 1 | 0.7 | 42,000 | eq. 12 |
//!   | CO–CH₄, cold | 40 | 2 | 0 | 25,000 | eq. 13 |
//!   | NH₃–N₂ | 1.0 × 10⁻⁷ | 1 | 0 | 52,000 | eq. 32 |
//!   | HCN | 1.5 × 10⁻⁴ | 1 | 0.7 | 36,000 | eq. 40 |
//!   | CO₂ | 1.0 × 10⁻¹⁰ | 0.5 | 0 | 38,000 | eq. 44 |
//!
//!   The two CO–CH₄ fits "combine harmonically to give a general description of reaction times in
//!   the CO-CH4 system", `t_CO` = (1 ÷ t₁ + 1 ÷ t₂)⁻¹ (eq. 14), which "when used with the timescale
//!   tmix = H^2/Kzz works for all of our models for CO and CH4 and we expect should work generally
//!   for compositions not too far from solar". Their eq. 11 (A = 3.0 × 10⁻⁶) is the uncorrected fit
//!   and is not used. For ammonia they "have no information to constrain dependence on p or m".
//! - **The mixing time** is `t_mix` = H² ÷ `K_zz`, H = R T ÷ (μ g) the pressure scale height at the
//!   level ([`mixing_time`]). ZM14 take the mixing length L = H on purpose, against Smith 1998's
//!   shorter L ("we therefore stick with the old rule L = H to define tmix"), and their fits are
//!   calibrated to it, so L = H here too: the 0.1–0.3 H Visscher and Moses (2011, ApJ 738, 72,
//!   §3) find for Jupiter's and Gliese 229B's CO belongs to their own times, not to these.
//! - **The level** is found as ZM14 find it, "working up from the lower boundary" ([`quench_level`]):
//!   the deepest level of the caller's range at which the chemical time reaches the mixing time,
//!   by a scan upward in steps of [`SCAN_STEP_LN_P`] in ln p and then [`BISECTIONS`] halvings of
//!   the step that crosses. A system that is slow at the range's bottom is quenched there
//!   ([`QuenchBound::Bottom`]), and one fast to its top is in equilibrium throughout
//!   ([`QuenchBound::Top`]). A profile with a second crossing higher up (ZM14 §8: "the atmosphere
//!   can have a second quench point in the stratosphere") keeps the first, so that the gas stays
//!   frozen through an irradiated giant's inversion, where the chemistry may be fast again.
//!
//! # `K_zz`, the stated law
//!
//! The mixing is one constant `K_zz` over the column ([`EddyDiffusion`]), as ZM14 treat it ("we
//! choose to treat Kzz as constant with height, but vary it over a range wide enough to encompass
//! all likely values", §3). P14.T52.d gives every envelope [`EddyDiffusion::JUPITER_LIKE`], ZM14's
//! "more Jupiter-like Kzz = 10^7 cm2/s" (§3.1), with which "the transition from methane to CO
//! occurs at 500 K in a planet. Sluggish vertical mixing can raise this to 600 K; but clouds or
//! more vigorous vertical mixing could lower this to 400 K" (their abstract). ZM14 give no source
//! for calling it Jupiter's; Visscher, Moses and Saslow (2010, Icarus 209, 602, §2.2) explore
//! 4 × 10⁷ to 10⁹ cm² s⁻¹ for Jupiter's deep troposphere, and ZM14's free-convection law, `K_zz` =
//! (H ÷ 3)(R `F_conv` ÷ (μ ρ `c_p`))^(1/3) (their eq. 3), gives 10⁹–10¹¹ cm² s⁻¹ in a self-luminous
//! planet's convecting zone, which ZM14 regard "with some skepticism". The constant is the law the
//! plan states; a law by the body's own convective flux is the recorded alternative.
//!
//! # The gas above the quench levels ([`Quench::apply`])
//!
//! [`Quench::derive`] finds the four levels and solves P14.T52.a's equilibrium at three: those of
//! CO–CH₄, NH₃–N₂ and HCN. [`Quench::apply`] solves the network at the level asked for and, above
//! each system's level (at a lower pressure), gives that system's species their mole fractions
//! there, each per hydrogen atom, which mixing preserves; at or below it, the local equilibrium's:
//!
//! - **CO–CH₄**: CO and CH₄ ("fCO, fCH4 above that level = equilibrium at (Tq, pq)").
//! - **NH₃–N₂**: NH₃; N₂ takes the nitrogen the others leave (decision-p14-t52a-rows, "For main":
//!   "N₂ takes the nitrogen that NH₃'s quench leaves, so the held floor's N₂ never reaches the
//!   record above the quench level").
//! - **HCN**: HCN at its own level. ZM14 do not say which equilibrium sets it there ("there does
//!   not yet appear to be a consensus on what the quench chemistry of HCN actually is", §8), and
//!   the network's is the whole gas's.
//! - **CO₂** "quenches with respect to the disequilibrium (quenched) abundance of CO" (ZM14 §6):
//!   above the CO–CH₄ level it is in equilibrium with the quenched CO, H₂O and H₂, CO + H₂O ⇌ CO₂ +
//!   H₂ at the local temperature, until its own level, above which it holds that level's value
//!   ("CO2 quenching is pinned at the altitude where the equilibrium defined by Eq 43 breaks
//!   down"). Its search starts at the CO–CH₄ level, so it is never deeper
//!   ([`QuenchBound::Pinned`]). The reaction's K is the registry's own (P14.T49.d's records),
//!   which the network's CO₂ obeys, rather than ZM14's fit, eq. 43, K = 18.3 exp(−2,376 ÷ T −
//!   (932 ÷ T)²) for the reverse, so that the two agree at the CO–CH₄ level; the tests compare
//!   them. Outside the records' range K is held at its floor or ceiling, as the network holds its
//!   gas.
//!
//! Each element is then balanced by one carrier, so that every element is conserved: carbon by
//! CO and CH₄ together, in proportion to their values; nitrogen by N₂; oxygen by H₂O (ZM14 freeze
//! H₂O with CO, "fO ≈ fH2O + fCO", and the rest of the oxygen is its minor species); hydrogen by
//! H₂. A carrier the frozen species would drive below zero is held at zero, which a gas within the
//! network's domain does not reach (the tests hold it so). Every other species is the local
//! equilibrium's, so the charge is unchanged. At the levels themselves the gas is continuous: a
//! quench level that reaches the evaluated level gives the local solution's own values.
//!
//! # Where it holds
//!
//! - ZM14's grid: 500 K < `T_eff` < 1,100 K, 10³ < g < 10⁵ cm s⁻², 10⁴ < `K_zz` < 10¹¹ cm² s⁻¹, m = 1
//!   and 3, cloudless, solar and self-luminous (§3.2); the quench temperatures their fits span
//!   (read by ZM14's figures, by the lane): CO–CH₄ about 1,050–2,170 K, HCN 1,090–2,330 K, CO₂
//!   885–1,590 K. Beyond, the relations are extrapolated, as P14.T52.d's warm and hot giants need.
//! - Irradiation: "in practice, CO2 can be much enhanced by photochemistry if the world in
//!   question is subject to significant stellar irradiation. Under such conditions quenching is a
//!   poor guide" (ZM14 §6). The photochemical rules are [`photochem`](super::photochem)'s.
//! - The levels' gas is P14.T52.a's, gas only and ideal: the search's range should stay within
//!   its ideal gas, about 1 kbar ([`equilibrium`](super::equilibrium)'s validation).
//!
//! # Determinism and cost
//!
//! The scan and the halvings are fixed counts from the caller's range; every sum runs in the
//! network's order, and every transcendental goes through [`math`]. A quench costs a scan of at
//! most ln(bottom ÷ top) ÷ [`SCAN_STEP_LN_P`] + 1 steps and [`BISECTIONS`] halvings a system, each
//! one profile evaluation and three logarithms, and three solves of the network; each level
//! [`Quench::apply`] serves costs one more. A section quenched at its 1-bar level and its
//! photosphere is five solves, against decision-composition §1.3's one to three (a recorded
//! finding; P14.T33.a times it).

use std::error::Error;
use std::fmt;

use crate::math;
use crate::substance::phase::MOLAR_GAS_CONSTANT;
use crate::substance::{Element, Reaction, SubstanceId, by_key};
use crate::units::{Kelvin, Metres, MetresPerSecondSquared, Pascals, Seconds};

use super::equilibrium::{GasEquilibrium, Network, SolveEquilibriumError};

/// Pascals per bar: the chemical times' pressures are in bar.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// Grams per kilogram, for the molar mass.
const GRAMS_PER_KILOGRAM: f64 = 1.0e3;

/// The step of the upward scan for a quench level, in ln p: a factor of e^0.5 ≈ 1.65 in pressure,
/// over which a chemical time changes by a factor of tens, so that a crossing is never stepped
/// over unseen where the profile is monotonic.
pub const SCAN_STEP_LN_P: f64 = 0.5;

/// The halvings of the crossing's step: 0.5 × 2⁻⁴⁰ ≈ 5 × 10⁻¹³ in ln p, a relative 5 × 10⁻¹³ in
/// the level's pressure.
pub const BISECTIONS: u32 = 40;

/// An Arrhenius-like chemical time, t = A p^−b m^−c exp(B ÷ T) (ZM14, eq. 10), p in bar, T in K
/// and m the metallicity as a multiple of solar.
///
/// ZM14: "It is reasonable to hope that the net reaction has an Arrhenius-like rate ... where
/// possible pressure and metallicity dependencies are written as power laws".
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ChemicalTime {
    prefactor: f64,
    pressure_power: f64,
    metallicity_power: f64,
    barrier: Kelvin,
}

impl ChemicalTime {
    /// A, in s bar^b.
    #[must_use]
    pub const fn prefactor(&self) -> f64 {
        self.prefactor
    }

    /// b, the power of the pressure in bar.
    #[must_use]
    pub const fn pressure_power(&self) -> f64 {
        self.pressure_power
    }

    /// c, the power of the metallicity.
    #[must_use]
    pub const fn metallicity_power(&self) -> f64 {
        self.metallicity_power
    }

    /// B, the barrier.
    #[must_use]
    pub const fn barrier(&self) -> Kelvin {
        self.barrier
    }

    /// ln t, t in seconds, at `t` kelvin, `p_bar` bar and metallicity `m`.
    #[must_use]
    fn ln_time(&self, t: f64, p_bar: f64, m: f64) -> f64 {
        math::ln(self.prefactor)
            - self.pressure_power * math::ln(p_bar)
            - self.metallicity_power * math::ln(m)
            + self.barrier.value() / t
    }
}

/// CO–CH₄ where CO is plentiful, t₁ = 1.5 × 10⁻⁶ p⁻¹ m^−0.7 exp(42,000 ÷ T) s (ZM14, eq. 12, the
/// corrected fit, by their Fig. 6; it sets the time above about 1,000 K at 1 bar).
pub const CARBON_MONOXIDE_WARM: ChemicalTime = ChemicalTime {
    prefactor: 1.5e-6,
    pressure_power: 1.0,
    metallicity_power: 0.7,
    barrier: Kelvin::new(42_000.0),
};

/// CO–CH₄ where CO is scarce, t₂ = 40 p⁻² exp(25,000 ÷ T) s (ZM14, eq. 13, "where we have given the
/// iterated rate": "The models fit by Eq 13 are those with very low quench temperatures Tq and very
/// little CO. We have no such models with m = 3").
pub const CARBON_MONOXIDE_COLD: ChemicalTime = ChemicalTime {
    prefactor: 40.0,
    pressure_power: 2.0,
    metallicity_power: 0.0,
    barrier: Kelvin::new(25_000.0),
};

/// NH₃–N₂, t = 1.0 × 10⁻⁷ p⁻¹ exp(52,000 ÷ T) s (ZM14, eq. 32: "This particular choice presumes
/// that the energy barrier is set by N2 + H2 → NNH + H").
pub const AMMONIA: ChemicalTime = ChemicalTime {
    prefactor: 1.0e-7,
    pressure_power: 1.0,
    metallicity_power: 0.0,
    barrier: Kelvin::new(52_000.0),
};

/// HCN, t = 1.5 × 10⁻⁴ p⁻¹ m^−0.7 exp(36,000 ÷ T) s (ZM14, eq. 40: "This expression seems to work
/// well for all cases we have considered"; it "overestimates HCN, especially in cool worlds").
pub const HYDROGEN_CYANIDE: ChemicalTime = ChemicalTime {
    prefactor: 1.5e-4,
    pressure_power: 1.0,
    metallicity_power: 0.7,
    barrier: Kelvin::new(36_000.0),
};

/// CO₂, t = 1.0 × 10⁻¹⁰ p^−0.5 exp(38,000 ÷ T) s (ZM14, eq. 44: "unlike the CO-CH4 system there
/// is no discernible dependence on metallicity").
pub const CARBON_DIOXIDE: ChemicalTime = ChemicalTime {
    prefactor: 1.0e-10,
    pressure_power: 0.5,
    metallicity_power: 0.0,
    barrier: Kelvin::new(38_000.0),
};

/// One of the four systems ZM14 give a quench relation for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuenchedSystem {
    /// CO and CH₄, by their harmonic time ([`CARBON_MONOXIDE_WARM`], [`CARBON_MONOXIDE_COLD`]).
    CarbonMonoxide,
    /// NH₃ and N₂ ([`AMMONIA`]).
    Ammonia,
    /// HCN ([`HYDROGEN_CYANIDE`]).
    HydrogenCyanide,
    /// CO₂, against the quenched CO ([`CARBON_DIOXIDE`]).
    CarbonDioxide,
}

impl QuenchedSystem {
    /// Every system, in the order [`Quench`] holds them.
    pub const ALL: [Self; 4] = [
        Self::CarbonMonoxide,
        Self::Ammonia,
        Self::HydrogenCyanide,
        Self::CarbonDioxide,
    ];

    /// The system's name, as messages write it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CarbonMonoxide => "CO–CH₄",
            Self::Ammonia => "NH₃–N₂",
            Self::HydrogenCyanide => "HCN",
            Self::CarbonDioxide => "CO₂",
        }
    }

    /// The system's place in [`ALL`](Self::ALL).
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::CarbonMonoxide => 0,
            Self::Ammonia => 1,
            Self::HydrogenCyanide => 2,
            Self::CarbonDioxide => 3,
        }
    }

    /// The system's chemical time at `temperature` and total pressure `pressure` in a gas of
    /// metallicity `metallicity` times solar (the [module](self)'s table).
    ///
    /// Each input is to be finite and positive; one that is not gives a time that is not either
    /// (a zero metallicity gives NaN, even for a time that does not depend on it).
    ///
    /// # Examples
    ///
    /// CO–CH₄ slows by orders of magnitude as the gas cools, which is why a rising parcel
    /// quenches:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::quench::QuenchedSystem;
    /// use hyperion_sim::units::{Kelvin, Pascals};
    ///
    /// let at = |t: f64| {
    ///     QuenchedSystem::CarbonMonoxide
    ///         .chemical_time(Kelvin::new(t), Pascals::new(1e5), 1.0)
    ///         .value()
    /// };
    /// assert!(at(1_000.0) > 1e3 * at(1_500.0));
    /// ```
    #[must_use]
    pub fn chemical_time(
        self,
        temperature: Kelvin,
        pressure: Pascals,
        metallicity: f64,
    ) -> Seconds {
        Seconds::new(math::exp(self.ln_chemical_time(
            temperature.value(),
            pressure.value() / PASCALS_PER_BAR,
            metallicity,
        )))
    }

    /// ln of [`chemical_time`](Self::chemical_time), in seconds, at `t` K, `p_bar` bar and
    /// metallicity `m`.
    #[must_use]
    fn ln_chemical_time(self, t: f64, p_bar: f64, m: f64) -> f64 {
        match self {
            Self::CarbonMonoxide => {
                // −ln t = ln(1 ÷ t₁ + 1 ÷ t₂), taken from the larger rate so that it stays finite.
                let fast = -CARBON_MONOXIDE_WARM.ln_time(t, p_bar, m);
                let slow = -CARBON_MONOXIDE_COLD.ln_time(t, p_bar, m);
                let (high, low) = if fast >= slow {
                    (fast, slow)
                } else {
                    (slow, fast)
                };
                -(high + math::ln_1p(math::exp(low - high)))
            }
            Self::Ammonia => AMMONIA.ln_time(t, p_bar, m),
            Self::HydrogenCyanide => HYDROGEN_CYANIDE.ln_time(t, p_bar, m),
            Self::CarbonDioxide => CARBON_DIOXIDE.ln_time(t, p_bar, m),
        }
    }
}

/// An eddy diffusion coefficient `K_zz`, the vertical mixing's, in m² s⁻¹ (10⁴ cm² s⁻¹ is
/// 1 m² s⁻¹).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct EddyDiffusion {
    square_metres_per_second: f64,
}

impl EddyDiffusion {
    /// ZM14's "more Jupiter-like Kzz = 10^7 cm2/s" (§3.1), 10³ m² s⁻¹: the stated law P14.T52.d
    /// gives every envelope (the [module](self)'s documentation).
    pub const JUPITER_LIKE: Self = Self::from_square_metres_per_second(1.0e3);

    /// A coefficient of `value` m² s⁻¹; [`quench_level`] refuses one that is not finite and
    /// positive.
    #[must_use]
    pub const fn from_square_metres_per_second(value: f64) -> Self {
        Self {
            square_metres_per_second: value,
        }
    }

    /// The coefficient, m² s⁻¹.
    #[must_use]
    pub const fn square_metres_per_second(self) -> f64 {
        self.square_metres_per_second
    }
}

/// The column a quench is found in, as plain values: its gravity and mean molar mass, which set
/// the scale height, its mixing, its metallicity and the range of pressures searched.
///
/// The fields are public, as P14.T50.b's inputs are; [`quench_level`] and [`Quench::derive`]
/// check them.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct MixingColumn {
    /// The gravity, taken as constant over the column.
    pub gravity: MetresPerSecondSquared,
    /// The gas's mean molar mass, g mol⁻¹, taken as constant over the column (about 2.3 for a
    /// protosolar envelope).
    pub molar_mass_g_per_mol: f64,
    /// The vertical mixing.
    pub eddy_diffusion: EddyDiffusion,
    /// The gas's metallicity as a multiple of solar, ZM14's m (P14.T52.d passes the envelope's
    /// enrichment times 10^\[Fe/H\]).
    pub metallicity: f64,
    /// The shallowest level searched: a system still in equilibrium there is in equilibrium at
    /// every level the caller evaluates.
    pub top: Pascals,
    /// The deepest level searched, where every system should still be in equilibrium (ZM14: "The
    /// lower boundary is set deep enough that all species are in thermodynamic equilibrium. We
    /// find that pressures above 300 bars and temperatures above 2000 K usually suffice"); within
    /// P14.T52.a's ideal gas, about 1 kbar.
    pub bottom: Pascals,
}

/// Where a system's quench level lies in the range searched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuenchBound {
    /// Within the range, where the chemical time equals the mixing time.
    Crossing,
    /// At the range's bottom: the system is already slower than the mixing there, so it quenched
    /// at or below it.
    Bottom,
    /// At the range's top: the system is faster than the mixing at every level searched, in
    /// equilibrium throughout.
    Top,
    /// CO₂ at the CO–CH₄ level: already slower than the mixing there, it quenches with the CO.
    Pinned,
}

/// A system's quench level: where its chemical time reaches the mixing time, or the bound it is
/// held at.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct QuenchLevel {
    system: QuenchedSystem,
    pressure: Pascals,
    temperature: Kelvin,
    bound: QuenchBound,
}

impl QuenchLevel {
    /// The system.
    #[must_use]
    pub const fn system(&self) -> QuenchedSystem {
        self.system
    }

    /// The level's pressure.
    #[must_use]
    pub const fn pressure(&self) -> Pascals {
        self.pressure
    }

    /// The profile's temperature there.
    #[must_use]
    pub const fn temperature(&self) -> Kelvin {
        self.temperature
    }

    /// Whether it is a crossing or a bound of the range.
    #[must_use]
    pub const fn bound(&self) -> QuenchBound {
        self.bound
    }
}

/// [`quench_level`] could not find a level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FindQuenchLevelError {
    /// The gravity is not finite and positive.
    Gravity(MetresPerSecondSquared),
    /// The molar mass is not finite and positive.
    MolarMass {
        /// The molar mass as given, g mol⁻¹.
        molar_mass_g_per_mol: f64,
    },
    /// The eddy diffusion coefficient is not finite and positive.
    EddyDiffusion(EddyDiffusion),
    /// The metallicity, a multiple of solar, is not finite and positive.
    Metallicity(f64),
    /// The range is not two finite, positive pressures with the top above (below) the bottom.
    Range {
        /// The top as given.
        top: Pascals,
        /// The bottom as given.
        bottom: Pascals,
    },
    /// The profile gave a temperature that is not finite and positive.
    Temperature {
        /// The pressure it was asked at.
        pressure: Pascals,
        /// What it gave.
        temperature: Kelvin,
    },
}

impl fmt::Display for FindQuenchLevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gravity(g) => write!(f, "gravity {} m/s² is not finite and positive", g.value()),
            Self::MolarMass {
                molar_mass_g_per_mol,
            } => write!(
                f,
                "molar mass {molar_mass_g_per_mol} g/mol is not finite and positive"
            ),
            Self::EddyDiffusion(k) => write!(
                f,
                "eddy diffusion {} m²/s is not finite and positive",
                k.square_metres_per_second()
            ),
            Self::Metallicity(m) => write!(f, "metallicity {m} is not finite and positive"),
            Self::Range { top, bottom } => write!(
                f,
                "the range from {} Pa to {} Pa is not two positive pressures, top above bottom",
                top.value(),
                bottom.value()
            ),
            Self::Temperature {
                pressure,
                temperature,
            } => write!(
                f,
                "the profile gives {} K at {} Pa, not finite and positive",
                temperature.value(),
                pressure.value()
            ),
        }
    }
}

impl Error for FindQuenchLevelError {}

/// The pressure scale height H = R T ÷ (μ g) at `temperature` in `column`.
///
/// `temperature` and `column`'s gravity and molar mass are to be finite and positive, as
/// [`quench_level`] holds them; otherwise H is not either.
///
/// # Examples
///
/// About 24 km at Jupiter's 1-bar level (166 K, 24.8 m s⁻², 2.3 g mol⁻¹):
///
/// ```
/// use hyperion_sim::planetary::chem::quench::{EddyDiffusion, MixingColumn, scale_height};
/// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
///
/// let column = MixingColumn {
///     gravity: MetresPerSecondSquared::new(24.79),
///     molar_mass_g_per_mol: 2.3,
///     eddy_diffusion: EddyDiffusion::JUPITER_LIKE,
///     metallicity: 3.0,
///     top: Pascals::new(1e3),
///     bottom: Pascals::new(1e8),
/// };
/// let h = scale_height(Kelvin::new(166.1), &column).value();
/// assert!((h / 1e3 - 24.2).abs() < 0.5);
/// ```
#[must_use]
pub fn scale_height(temperature: Kelvin, column: &MixingColumn) -> Metres {
    Metres::new(
        MOLAR_GAS_CONSTANT * temperature.value()
            / (column.molar_mass_g_per_mol / GRAMS_PER_KILOGRAM * column.gravity.value()),
    )
}

/// The mixing time `t_mix` = H² ÷ `K_zz` at `temperature` in `column`, the mixing length a scale
/// height (ZM14's L = H).
///
/// Its inputs are to be finite and positive, as [`quench_level`] holds them; otherwise the time
/// is not either.
#[must_use]
pub fn mixing_time(temperature: Kelvin, column: &MixingColumn) -> Seconds {
    let h = scale_height(temperature, column).value();
    Seconds::new(h * h / column.eddy_diffusion.square_metres_per_second())
}

/// Checks `column`'s values.
fn check(column: &MixingColumn) -> Result<(), FindQuenchLevelError> {
    let positive = |x: f64| x.is_finite() && x > 0.0;
    if !positive(column.gravity.value()) {
        return Err(FindQuenchLevelError::Gravity(column.gravity));
    }
    if !positive(column.molar_mass_g_per_mol) {
        return Err(FindQuenchLevelError::MolarMass {
            molar_mass_g_per_mol: column.molar_mass_g_per_mol,
        });
    }
    if !positive(column.eddy_diffusion.square_metres_per_second()) {
        return Err(FindQuenchLevelError::EddyDiffusion(column.eddy_diffusion));
    }
    if !positive(column.metallicity) {
        return Err(FindQuenchLevelError::Metallicity(column.metallicity));
    }
    let (top, bottom) = (column.top.value(), column.bottom.value());
    if !(positive(top) && positive(bottom) && top < bottom) {
        return Err(FindQuenchLevelError::Range {
            top: column.top,
            bottom: column.bottom,
        });
    }
    Ok(())
}

/// The quench level of `system` along the temperature profile `profile`, T(p), in `column`: the
/// deepest level of `column`'s range at which the chemical time reaches the mixing time, found
/// working up from its bottom (the [module](self)'s documentation).
///
/// # Errors
///
/// [`FindQuenchLevelError`] for a gravity, molar mass, eddy diffusion coefficient or metallicity
/// that is not finite and positive, a range that is not two positive pressures with the top above
/// the bottom, or a profile temperature that is not finite and positive at a level searched.
///
/// # Examples
///
/// Jupiter's CO is quenched deep, near 1,000 K, along its adiabat (166 K at 1 bar, 428 K at
/// 22 bar; Seiff et al. 1998):
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::planetary::chem::quench::{
///     EddyDiffusion, MixingColumn, QuenchBound, QuenchedSystem, quench_level,
/// };
/// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let power = math::ln(427.7 / 166.1) / math::ln(22.0);
/// let jupiter = |p: Pascals| Kelvin::new(166.1 * math::powf(p.value() / 1e5, power));
/// let column = MixingColumn {
///     gravity: MetresPerSecondSquared::new(24.79),
///     molar_mass_g_per_mol: 2.3,
///     eddy_diffusion: EddyDiffusion::JUPITER_LIKE,
///     metallicity: 3.0,
///     top: Pascals::new(1e3),
///     bottom: Pascals::new(1e8),
/// };
/// let level = quench_level(QuenchedSystem::CarbonMonoxide, jupiter, &column)?;
/// assert_eq!(level.bound(), QuenchBound::Crossing);
/// assert!((800.0..1_300.0).contains(&level.temperature().value()));
/// # Ok(())
/// # }
/// ```
pub fn quench_level(
    system: QuenchedSystem,
    profile: impl Fn(Pascals) -> Kelvin,
    column: &MixingColumn,
) -> Result<QuenchLevel, FindQuenchLevelError> {
    check(column)?;
    find(system, &profile, column, column.top, column.bottom)
}

/// The quench level of `system` between `top` and `bottom` (`top` ≤ `bottom`), as
/// [`quench_level`]; a range of one level gives that level as its top.
fn find(
    system: QuenchedSystem,
    profile: &impl Fn(Pascals) -> Kelvin,
    column: &MixingColumn,
    top: Pascals,
    bottom: Pascals,
) -> Result<QuenchLevel, FindQuenchLevelError> {
    let at = |pressure: Pascals| -> Result<(Kelvin, f64), FindQuenchLevelError> {
        let temperature = profile(pressure);
        let t = temperature.value();
        if !(t.is_finite() && t > 0.0) {
            return Err(FindQuenchLevelError::Temperature {
                pressure,
                temperature,
            });
        }
        let chemical =
            system.ln_chemical_time(t, pressure.value() / PASCALS_PER_BAR, column.metallicity);
        let mixing = math::ln(mixing_time(temperature, column).value());
        Ok((temperature, chemical - mixing))
    };
    let level = |pressure: Pascals, temperature: Kelvin, bound: QuenchBound| QuenchLevel {
        system,
        pressure,
        temperature,
        bound,
    };
    let (deepest, excess) = at(bottom)?;
    if excess >= 0.0 {
        return Ok(level(bottom, deepest, QuenchBound::Bottom));
    }
    let ln_top = math::ln(top.value());
    let mut below = math::ln(bottom.value());
    loop {
        let candidate = below - SCAN_STEP_LN_P;
        let reached_top = candidate <= ln_top;
        let (ln_above, above) = if reached_top {
            (ln_top, top)
        } else {
            (candidate, Pascals::new(math::exp(candidate)))
        };
        let (temperature, excess) = at(above)?;
        if excess >= 0.0 {
            // Halve the step: `shallow` is quenched, `deep` in equilibrium.
            let (mut shallow, mut deep) = (ln_above, below);
            for _ in 0..BISECTIONS {
                let middle = shallow.midpoint(deep);
                if at(Pascals::new(math::exp(middle)))?.1 >= 0.0 {
                    shallow = middle;
                } else {
                    deep = middle;
                }
            }
            let pressure = Pascals::new(math::exp(shallow.midpoint(deep)));
            let (temperature, _) = at(pressure)?;
            return Ok(level(pressure, temperature, QuenchBound::Crossing));
        }
        if reached_top {
            return Ok(level(top, temperature, QuenchBound::Top));
        }
        below = candidate;
    }
}

/// The places in the network of the species the quench moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Places {
    hydrogen: usize,
    water: usize,
    methane: usize,
    carbon_monoxide: usize,
    carbon_dioxide: usize,
    nitrogen: usize,
    ammonia: usize,
    hydrogen_cyanide: usize,
}

/// The keys of [`Places`]' species, in its order.
const MOVED: [&str; 8] = ["H2", "H2O", "CH4", "CO", "CO2", "N2", "NH3", "HCN"];

impl Places {
    /// The places of [`MOVED`]'s species among `species`, the registry network's.
    ///
    /// # Panics
    ///
    /// Never with the registry's network: every one of them is a gas row with a record, which
    /// the network carries (P14.T52.a's `the_registry_s_network_builds` holds it so).
    #[must_use]
    fn of(species: &[SubstanceId]) -> Self {
        let place = MOVED.map(|key| {
            by_key(key)
                .and_then(|id| species.iter().position(|&s| s == id))
                .expect("the quenched species are gas rows with records, so in the network")
        });
        let [
            hydrogen,
            water,
            methane,
            carbon_monoxide,
            carbon_dioxide,
            nitrogen,
            ammonia,
            hydrogen_cyanide,
        ] = place;
        Self {
            hydrogen,
            water,
            methane,
            carbon_monoxide,
            carbon_dioxide,
            nitrogen,
            ammonia,
            hydrogen_cyanide,
        }
    }
}

/// The elements the quench balances, in the order of [`Quench`]'s atom counts.
const BALANCED: [Element; 4] = [Element::H, Element::C, Element::N, Element::O];

/// Each of `species`' atoms of [`BALANCED`]'s elements.
#[must_use]
fn balanced_atoms(species: &[SubstanceId]) -> Vec<[f64; 4]> {
    species
        .iter()
        .map(|id| {
            BALANCED.map(|element| {
                id.substance()
                    .elements()
                    .iter()
                    .find(|&&(e, _)| e == element)
                    .map_or(0.0, |&(_, n)| f64::from(n))
            })
        })
        .collect()
}

/// [`Quench::derive`] could not quench the gas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveQuenchError {
    /// A quench level could not be found.
    Level(FindQuenchLevelError),
    /// The equilibrium at a system's level could not be solved.
    Equilibrium {
        /// The system.
        system: QuenchedSystem,
        /// Why.
        error: SolveEquilibriumError,
    },
}

impl fmt::Display for DeriveQuenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Level(error) => write!(f, "no quench level: {error}"),
            Self::Equilibrium { system, error } => {
                write!(f, "the equilibrium at the {} level: {error}", system.name())
            }
        }
    }
}

impl Error for DeriveQuenchError {}

impl From<FindQuenchLevelError> for DeriveQuenchError {
    fn from(error: FindQuenchLevelError) -> Self {
        Self::Level(error)
    }
}

/// [`Quench::apply`] could not give a level's gas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApplyQuenchError {
    /// The network could not solve the gas at the level.
    Equilibrium(SolveEquilibriumError),
}

impl fmt::Display for ApplyQuenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Equilibrium(error) => write!(f, "the equilibrium at the level: {error}"),
        }
    }
}

impl Error for ApplyQuenchError {}

/// An envelope's quench: its gas, its four systems' levels and the equilibria at three of them,
/// from which [`apply`](Self::apply) gives any level's gas (the [module](self)'s documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct Quench {
    /// The gas's element abundances, as [`derive`](Self::derive) was given them.
    abundances: Vec<(Element, f64)>,
    /// Each system's level, in [`QuenchedSystem::ALL`]'s order.
    levels: [QuenchLevel; 4],
    /// The equilibria at the CO–CH₄, NH₃–N₂ and HCN levels, each per hydrogen atom, in the
    /// network's order.
    frozen: [Vec<f64>; 3],
    /// The network's species, in its order.
    species: Vec<SubstanceId>,
    /// Each species' atoms of [`BALANCED`]'s elements.
    atoms: Vec<[f64; 4]>,
    places: Places,
    /// CO + H₂O ⇌ CO₂ + H₂.
    carbon_dioxide: Reaction,
    /// The network's floor and ceiling, between which the reaction's K is held, as the network
    /// holds its gas.
    floor: Kelvin,
    ceiling: Kelvin,
}

/// `gas`'s mole fractions per hydrogen atom, in the network's order: x ÷ Σ (H atoms × x).
#[must_use]
fn per_hydrogen(gas: &GasEquilibrium, atoms: &[[f64; 4]]) -> Vec<f64> {
    let mut hydrogen = 0.0;
    for ((_, x), a) in gas.mole_fractions().zip(atoms) {
        hydrogen += a[0] * x;
    }
    gas.mole_fractions().map(|(_, x)| x / hydrogen).collect()
}

impl Quench {
    /// The quench of the gas of element abundances `abundances` (each by number relative to
    /// hydrogen, as [`Network::solve`] takes them) along the temperature profile `profile`, T(p),
    /// in `column`, over `network`.
    ///
    /// # Errors
    ///
    /// [`DeriveQuenchError::Level`] as [`quench_level`] refuses, and
    /// [`DeriveQuenchError::Equilibrium`] if the network cannot solve the gas at a system's level:
    /// a gas it refuses, or a level above its ceiling.
    ///
    /// # Panics
    ///
    /// Never with the registry's network, which carries every species the quench moves.
    ///
    /// # Examples
    ///
    /// Jupiter's CO at 1 bar is its quench level's, some ten orders of magnitude above the
    /// equilibrium there:
    ///
    /// ```
    /// use hyperion_sim::math;
    /// use hyperion_sim::planetary::chem::equilibrium::Network;
    /// use hyperion_sim::planetary::chem::quench::{EddyDiffusion, MixingColumn, Quench};
    /// use hyperion_sim::substance::{Element, by_key};
    /// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let network = Network::registry()?;
    /// let gas: Vec<(Element, f64)> = Element::ALL
    ///     .iter()
    ///     .map(|&e| {
    ///         let scale = if matches!(e, Element::H | Element::He) { 1.0 } else { 3.0 };
    ///         (e, e.protosolar_abundance_per_hydrogen() * scale)
    ///     })
    ///     .collect();
    /// let power = math::ln(427.7 / 166.1) / math::ln(22.0);
    /// let jupiter = |p: Pascals| Kelvin::new(166.1 * math::powf(p.value() / 1e5, power));
    /// let column = MixingColumn {
    ///     gravity: MetresPerSecondSquared::new(24.79),
    ///     molar_mass_g_per_mol: 2.3,
    ///     eddy_diffusion: EddyDiffusion::JUPITER_LIKE,
    ///     metallicity: 3.0,
    ///     top: Pascals::new(1e3),
    ///     bottom: Pascals::new(1e8),
    /// };
    /// let quench = Quench::derive(&network, &gas, jupiter, &column)?;
    /// let one_bar = Pascals::new(1e5);
    /// let local = network.solve(gas.iter().copied(), jupiter(one_bar), one_bar)?;
    /// let quenched = quench.apply(&network, jupiter(one_bar), one_bar)?;
    /// let co = by_key("CO").ok_or("a row")?;
    /// assert!(quenched.mole_fraction(co) > local.mole_fraction(co).map(|x| x * 1e10));
    /// # Ok(())
    /// # }
    /// ```
    pub fn derive(
        network: &Network,
        abundances: &[(Element, f64)],
        profile: impl Fn(Pascals) -> Kelvin,
        column: &MixingColumn,
    ) -> Result<Self, DeriveQuenchError> {
        check(column)?;
        let species: Vec<SubstanceId> = network.species().collect();
        let places = Places::of(&species);
        let atoms = balanced_atoms(&species);
        let reaction = Reaction::new(&[
            (species[places.carbon_monoxide], -1),
            (species[places.water], -1),
            (species[places.carbon_dioxide], 1),
            (species[places.hydrogen], 1),
        ])
        .expect("CO + H2O = CO2 + H2 balances, and each is a gas row with a record");

        let (top, bottom) = (column.top, column.bottom);
        let carbon = find(
            QuenchedSystem::CarbonMonoxide,
            &profile,
            column,
            top,
            bottom,
        )?;
        let nitrogen = find(QuenchedSystem::Ammonia, &profile, column, top, bottom)?;
        let cyanide = find(
            QuenchedSystem::HydrogenCyanide,
            &profile,
            column,
            top,
            bottom,
        )?;
        let dioxide = {
            let found = find(
                QuenchedSystem::CarbonDioxide,
                &profile,
                column,
                top,
                carbon.pressure,
            )?;
            // Slower than the mixing at the CO–CH₄ level itself, or with no range above it: CO₂
            // quenches with the CO.
            if found.bound == QuenchBound::Bottom || carbon.pressure.value() <= top.value() {
                QuenchLevel {
                    system: QuenchedSystem::CarbonDioxide,
                    pressure: carbon.pressure,
                    temperature: carbon.temperature,
                    bound: QuenchBound::Pinned,
                }
            } else {
                found
            }
        };
        let solve = |level: &QuenchLevel| -> Result<Vec<f64>, DeriveQuenchError> {
            let gas = network
                .solve(
                    abundances.iter().copied(),
                    level.temperature,
                    level.pressure,
                )
                .map_err(|error| DeriveQuenchError::Equilibrium {
                    system: level.system,
                    error,
                })?;
            Ok(per_hydrogen(&gas, &atoms))
        };
        let frozen = [solve(&carbon)?, solve(&nitrogen)?, solve(&cyanide)?];
        Ok(Self {
            abundances: abundances.to_vec(),
            levels: [carbon, nitrogen, cyanide, dioxide],
            frozen,
            species,
            atoms,
            places,
            carbon_dioxide: reaction,
            floor: network.floor(),
            ceiling: network.ceiling(),
        })
    }

    /// The level of `system`.
    #[must_use]
    pub const fn level(&self, system: QuenchedSystem) -> QuenchLevel {
        self.levels[system.index()]
    }

    /// The gas at `temperature` and `pressure`: `network`'s equilibrium there (P14.T52.a, solved
    /// for the gas [`derive`](Self::derive) was given), with each system quenched above its level
    /// (the [module](self)'s documentation).
    ///
    /// `network` is the one the quench was derived over, the registry's; `temperature` is the
    /// profile's at `pressure`, as `derive`'s levels are, so that the gas is continuous across each
    /// quench level.
    ///
    /// # Errors
    ///
    /// [`ApplyQuenchError::Equilibrium`] if the network cannot solve the gas at the level: a
    /// temperature or pressure that is not finite and positive, or a temperature above its
    /// ceiling.
    ///
    /// # Panics
    ///
    /// Never: CO + H₂O ⇌ CO₂ + H₂'s records hold over the network's range, to which its
    /// temperature is held.
    pub fn apply(
        &self,
        network: &Network,
        temperature: Kelvin,
        pressure: Pascals,
    ) -> Result<QuenchedGas, ApplyQuenchError> {
        let local = network
            .solve(self.abundances.iter().copied(), temperature, pressure)
            .map_err(ApplyQuenchError::Equilibrium)?;
        let p = pressure.value();
        let quenched = self.levels.map(|level| p < level.pressure.value());
        let mut y = per_hydrogen(&local, &self.atoms);
        let totals = self.totals(&y);
        let places = self.places;
        let [carbon, nitrogen, cyanide] = &self.frozen;
        if quenched[QuenchedSystem::CarbonMonoxide.index()] {
            y[places.carbon_monoxide] = carbon[places.carbon_monoxide];
            y[places.methane] = carbon[places.methane];
            // CO₂ against the quenched CO, H₂O and H₂, at the local temperature up to its own
            // level and at that level's above it.
            let t = if quenched[QuenchedSystem::CarbonDioxide.index()] {
                self.level(QuenchedSystem::CarbonDioxide).temperature
            } else {
                temperature
            };
            let t = if t.value() < self.floor.value() {
                self.floor
            } else if t.value() > self.ceiling.value() {
                self.ceiling
            } else {
                t
            };
            let log_k = self
                .carbon_dioxide
                .log10_equilibrium_constant(t)
                .expect("the reaction's records hold over the network's range");
            y[places.carbon_dioxide] =
                math::exp10(log_k) * carbon[places.carbon_monoxide] * carbon[places.water]
                    / carbon[places.hydrogen];
        }
        if quenched[QuenchedSystem::Ammonia.index()] {
            y[places.ammonia] = nitrogen[places.ammonia];
        }
        if quenched[QuenchedSystem::HydrogenCyanide.index()] {
            y[places.hydrogen_cyanide] = cyanide[places.hydrogen_cyanide];
        }
        self.balance(&mut y, &totals);
        let mut sum = 0.0;
        for v in &y {
            sum += v;
        }
        Ok(QuenchedGas {
            temperature,
            pressure,
            quenched,
            fractions: self
                .species
                .iter()
                .zip(&y)
                .map(|(&id, v)| (id, v / sum))
                .collect(),
        })
    }

    /// The atoms of each of [`BALANCED`]'s elements in `y`, per hydrogen atom.
    #[must_use]
    fn totals(&self, y: &[f64]) -> [f64; 4] {
        let mut totals = [0.0; 4];
        for (v, a) in y.iter().zip(&self.atoms) {
            for (total, n) in totals.iter_mut().zip(a) {
                *total += n * v;
            }
        }
        totals
    }

    /// The atoms of element `e` (its place in [`BALANCED`]) in `y` but in the species at `skip`.
    #[must_use]
    fn others(&self, y: &[f64], e: usize, skip: &[usize]) -> f64 {
        let mut held = 0.0;
        for (i, (v, a)) in y.iter().zip(&self.atoms).enumerate() {
            if !skip.contains(&i) {
                held += a[e] * v;
            }
        }
        held
    }

    /// Balances carbon by CO and CH₄, nitrogen by N₂, oxygen by H₂O and hydrogen by H₂, in that
    /// order, against `totals`, each held at zero or above.
    fn balance(&self, y: &mut [f64], totals: &[f64; 4]) {
        let places = self.places;
        let pair = [places.carbon_monoxide, places.methane];
        let carriers = y[places.carbon_monoxide] + y[places.methane];
        if carriers > 0.0 {
            let factor = (totals[1] - self.others(y, 1, &pair)) / carriers;
            let factor = if factor > 0.0 { factor } else { 0.0 };
            y[places.carbon_monoxide] *= factor;
            y[places.methane] *= factor;
        }
        let carrier = |y: &[f64], place: usize, e: usize, per: f64| {
            let left = (totals[e] - self.others(y, e, &[place])) / per;
            if left > 0.0 { left } else { 0.0 }
        };
        y[places.nitrogen] = carrier(y, places.nitrogen, 2, 2.0);
        y[places.water] = carrier(y, places.water, 3, 1.0);
        y[places.hydrogen] = carrier(y, places.hydrogen, 0, 2.0);
    }
}

/// A level's gas with its systems quenched ([`Quench::apply`]): the mole fraction of each species
/// of the network.
#[derive(Debug, Clone, PartialEq)]
pub struct QuenchedGas {
    temperature: Kelvin,
    pressure: Pascals,
    quenched: [bool; 4],
    fractions: Vec<(SubstanceId, f64)>,
}

impl QuenchedGas {
    /// The level's temperature.
    #[must_use]
    pub const fn temperature(&self) -> Kelvin {
        self.temperature
    }

    /// The level's total pressure.
    #[must_use]
    pub const fn pressure(&self) -> Pascals {
        self.pressure
    }

    /// Whether the level lies above `system`'s quench level, so that its species are the quench
    /// level's.
    #[must_use]
    pub const fn is_quenched(&self, system: QuenchedSystem) -> bool {
        self.quenched[system.index()]
    }

    /// Each species of the network and its mole fraction, in registry order.
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

    /// The element's atoms in the gas per hydrogen atom, from the mole fractions.
    #[must_use]
    pub fn per_hydrogen(&self, element: Element) -> f64 {
        let count = |id: SubstanceId, e: Element| {
            id.substance()
                .elements()
                .iter()
                .find(|&&(x, _)| x == e)
                .map_or(0.0, |&(_, n)| f64::from(n))
        };
        let mut held = 0.0;
        let mut hydrogen = 0.0;
        for &(id, x) in &self.fractions {
            held += count(id, element) * x;
            hydrogen += count(id, Element::H) * x;
        }
        held / hydrogen
    }
}

#[cfg(test)]
mod tests;
