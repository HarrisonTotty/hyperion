//! A surface melt's own vapour, its silicate vapour (plan 14, P14.T55.b; decision-backlog-2 §2,
//! decision-triton-and-e101 §2): the rock-forming species a melt evaporates over it, chiefly Na,
//! O₂, O and `SiO` over a melt with no volatiles (Schaefer and Fegley 2009, Astrophys. J. 703,
//! L113, here SF09), with whether the body holds them or they stream off it.
//!
//! Every piece is a closed form of plain quantities, held off the record below generator version
//! 35 ([`SILICATE_VAPOUR_VERSION`]). It is not part of the volatile air the surface's conditions
//! hold: the melt's temperature sets it where melt lies, not a volatile inventory's weight.
//!
//! # The pressure
//!
//! The vapour's total pressure over a melt of the bulk silicate Earth's composition (BSE) is
//! Fegley and Schaefer's closed form, fitted to the MAGMA code's equilibrium over the molten BSE
//! from 2,000 to 6,000 K with no zinc ([`bse_log10_pressure_bar`]); it agrees with SF09's own BSE
//! curve within 0.1 dex from 1,500 to 2,800 K, and is read below 1,500 K, SF09's lowest, down to
//! the coolest melt's solidus by its own extrapolation (a lean, 2026-11-19). Another melt's is the BSE's moved by its
//! composition class's offset ([`MeltComposition`]), SF09's Fig. 1 read for their oceanic and
//! continental crusts against their BSE at 1,500, 2,000, 2,500 and 3,000 K, linear in 1 ÷ T
//! between those nodes, held below the first and fading to none at 4,000 K, where Visscher and
//! Fegley's compositions meet within 0.1 dex (2013, Astrophys. J. Lett. 767, L12, here VF13,
//! their Fig. 3a).
//!
//! # The species
//!
//! The species' mole fractions are the BSE vapour's at the melt's temperature, for every
//! composition (a lean, 2026-11-19: SF09 publish the split for their BSE alone), from
//! `BSE_PARTIALS_LOG10_BAR`'s nodes, log-linear in 1 ÷ T between them and held outside
//! 2,000–4,000 K: Na, then O₂ at a quarter of it (`Na₂O` → 2 Na + ½ O₂), up to about 2,750 K;
//! `SiO` leading from about 3,100 K. The nodes balance oxygen, the O₂ and O against what the
//! metal-bearing species freed, within 6%.
//!
//! # Bound or an outflow
//!
//! The vapour's Jeans parameter λ = G M m ÷ (k T R) = g R m ÷ (k T), at the melt's temperature
//! and its mean molecular mass at the body's surface, sets whether the body holds it
//! ([`VapourEscape`]), against the critical value of an atomic gas where atoms lead the vapour
//! ([`ATOMIC_CRITICAL_JEANS_PARAMETER`]) and of a diatomic one where molecules do
//! ([`DIATOMIC_CRITICAL_JEANS_PARAMETER`]): a lava world's dayside vapour is a bound air (λ in the
//! hundreds on a super-Earth), while `e101`'s λ of 0.17–0.32 at 1,684 K (decision-triton-and-e101 §2.2)
//! makes a small hot body's an outflow, its pressure the melt's at its surface and not an air's
//! weight. Whether such a body survives is P14.T55.b.a's evaporation (`fate::evaporation`, from
//! generator version 28).
//!
//! # Not modelled
//!
//! - **A melt depleted by its own evaporation.** SF09's fractional vaporisation loses Na once 2%
//!   of a BSE melt has gone at 2,200 K, its pressure falling tenfold; every melt here keeps its
//!   lid's composition.
//! - **The species' composition dependence**, above, and an alkali-poor melt's lower pressure: SF09's
//!   bulk silicate Moon (`Na₂O` 0.05 wt%) lies 0.5–0.7 dex under their BSE at 2,000–2,500 K (their
//!   Fig. 1), and such a melt reads as the BSE's, `basalt_reduced`, whose analysis lists no alkali,
//!   among them.
//! - **Zinc and the other trace volatiles**, which VF13's melt holds and SF09's does not.
//!
//! # Determinism
//!
//! Pure functions of their arguments; every sum in a declared order; every transcendental
//! through the crate's [`math`].

use core::fmt;
use std::error::Error;

use crate::math;
use crate::planetary::derive::atmosphere::ATOMIC_MASS_CONSTANT_KG;
use crate::substance::liquid::Oxide;
use crate::substance::{SubstanceId, by_key};
use crate::units::consts::BOLTZMANN_CONSTANT;
use crate::units::{Kelvin, Metres, MetresPerSecondSquared, Pascals};
use crate::{GENERATOR_VERSION, GeneratorVersion};

#[cfg(test)]
mod tests;

/// The generator version from which a record's surface holds its melt's vapour, and a volcanic
/// lid its active lava (P14.T55.b): 35, a planetary hold (main, 2026-11-19). Below it the vapour is
/// not modelled and the melt area reads starlight alone, so that no output moves at 33 or 34.
pub const SILICATE_VAPOUR_VERSION: GeneratorVersion = GeneratorVersion::new(35);

/// Whether a record's surface holds its melt's vapour and a volcanic lid its active lava (their
/// hold, [`SILICATE_VAPOUR_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SilicateVapourRules {
    /// Version 34's and before: the vapour not modelled, the melt area starlight's alone.
    Held,
    /// Version 35's: the vapour over melt, and a volcanic lid's active lava in its melt area.
    Modelled,
}

impl SilicateVapourRules {
    /// The rules this generator version gives: modelled from [`SILICATE_VAPOUR_VERSION`], held
    /// below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_silicate_vapour`]).
    #[must_use]
    pub fn in_force() -> Self {
        #[cfg(any(test, feature = "testing"))]
        if let Some(rules) = SILICATE_VAPOUR_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= SILICATE_VAPOUR_VERSION {
            Self::Modelled
        } else {
            Self::Held
        }
    }
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// The silicate vapour's rules a test sets on its thread, `None` for the version's.
    static SILICATE_VAPOUR_RULES: core::cell::Cell<Option<SilicateVapourRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the silicate vapour's rules `rules` on this thread, for the tests to reach
/// version 35's ahead of its bump (the `testing` feature's, for other crates' tests); the version's
/// rules are set back after, a panic in `run` included. A record derives lazily, so a test reads it
/// inside the switch. 35's bump deletes it with the hold.
#[cfg(any(test, feature = "testing"))]
pub fn with_silicate_vapour<R>(rules: SilicateVapourRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<SilicateVapourRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SILICATE_VAPOUR_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(SILICATE_VAPOUR_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// Fegley and Schaefer's BSE vapour pressure's constant, log₁₀ bar: −30.6757, of log₁₀ P =
/// −30.6757 − 8,228.146 ÷ T + 9.3974 log₁₀ T, P in bar and T in K, the total pressure of the
/// saturated vapour over the molten BSE from 2,000 to 6,000 K by the MAGMA code, its one-bar
/// boiling point about 3,352 K ("Chemistry of the Earth's Earliest Atmosphere", arXiv:1210.0270,
/// Eq. 13, read; published as Fegley and Schaefer 2014, Treatise on Geochemistry, 2nd ed., vol.
/// 6, pp. 71–90, recalled).
const BSE_PRESSURE_CONSTANT: f64 = -30.6757;

/// Fegley and Schaefer's BSE vapour pressure's 1 ÷ T coefficient, K: −8,228.146 (their Eq. 13,
/// [`BSE_PRESSURE_CONSTANT`]'s).
const BSE_PRESSURE_INVERSE_K: f64 = -8_228.146;

/// Fegley and Schaefer's BSE vapour pressure's log₁₀ T coefficient: 9.3974 (their Eq. 13,
/// [`BSE_PRESSURE_CONSTANT`]'s).
const BSE_PRESSURE_LOG_T: f64 = 9.3974;

/// The warmest melt the BSE pressure is read at, K: 6,000, the top of the range its closed form was
/// fitted over (Fegley and Schaefer 2014, Eq. 13), above which it is held.
const BSE_PRESSURE_CEILING_K: f64 = 6_000.0;

/// Pascals per bar.
const PASCALS_PER_BAR: f64 = 1e5;

/// The temperatures of the composition offsets' nodes, K: 1,500, 2,000, 2,500 and 3,000, those of
/// SF09's Fig. 1 read for [`OCEANIC_CRUST_OFFSET_DEX`] and [`CONTINENTAL_CRUST_OFFSET_DEX`].
const OFFSET_NODES_K: [f64; 4] = [1_500.0, 2_000.0, 2_500.0, 3_000.0];

/// The temperature at which every composition's pressure meets the BSE's, K: 4,000, where VF13's
/// six melts lie within 0.1 dex of one another (their Fig. 3a); a class's offset fades linearly in
/// T from its 3,000 K node to none here.
const CONVERGED_K: f64 = 4_000.0;

/// SF09's oceanic crust's total vapour pressure less their BSE's, dex, at [`OFFSET_NODES_K`]:
/// +0.03, +0.11, +0.16 and +0.18, their Fig. 1's curves (log₁₀ bar −6.31, −3.71, −1.84, −0.70
/// against the BSE's −6.34, −3.82, −2.00, −0.88), read from the arXiv version's vector paths
/// (arXiv:0906.1204; ±0.1 dex), for a melt of their Table 1's oceanic crust (`SiO₂` 50.36, `MgO`
/// 7.61, `FeO` 9.56, `Na₂O` 2.77, K₂O 0.13 wt%).
const OCEANIC_CRUST_OFFSET_DEX: [f64; 4] = [0.03, 0.11, 0.16, 0.18];

/// SF09's continental crust's total vapour pressure less their BSE's, dex, at [`OFFSET_NODES_K`]:
/// −0.53, −0.33, −0.10 and +0.18, their Fig. 1 (log₁₀ bar −6.87, −4.15, −2.10, −0.70), read as
/// [`OCEANIC_CRUST_OFFSET_DEX`] is, for their Table 1's continental crust (`SiO₂` 62.93, `MgO`
/// 3.79, `FeO` 5.78, `Na₂O` 3.27, K₂O 2.45 wt%): under their BSE's below about 2,700 K, where the
/// two curves cross, and over it above.
const CONTINENTAL_CRUST_OFFSET_DEX: [f64; 4] = [-0.53, -0.33, -0.10, 0.18];

/// The silica of a melt from which it is read as SF09's continental crust, weight per cent: 60, a
/// boundary between their oceanic crust's 50.36 and continental crust's 62.93 (a lean,
/// 2026-11-19).
const CONTINENTAL_SILICA_WT_PCT: f64 = 60.0;

/// The alkalis (`Na₂O` + K₂O) of a melt from which one below [`CONTINENTAL_SILICA_WT_PCT`] is read
/// as SF09's oceanic crust, weight per cent: 1.0, about the geometric mean of their BSE's 0.39
/// and oceanic crust's 2.90 (a lean, 2026-11-19).
const OCEANIC_ALKALIS_WT_PCT: f64 = 1.0;

/// The species the vapour holds, in [`BSE_PARTIALS_LOG10_BAR`]'s column order, by registry key.
const SPECIES: [&str; 8] = ["Na", "O2", "O", "SiO", "Fe", "Mg", "SiO2", "K"];

/// The temperatures of the species' nodes, K, [`BSE_PARTIALS_LOG10_BAR`]'s rows.
const SPECIES_NODES_K: [f64; 5] = [2_000.0, 2_500.0, 3_000.0, 3_500.0, 4_000.0];

/// The BSE vapour's partial pressures, log₁₀ bar, of [`SPECIES`] at [`SPECIES_NODES_K`], from
/// which the species' mole fractions are read, each row's over their sum:
///
/// - 2,000, 2,500 and 3,000 K: SF09's Fig. 2 (their column densities, read from the arXiv
///   version's vector paths at their g of 36.2 m s⁻² back to pressures, P in bar = σ g μ ÷ `N_A`
///   with σ as plotted, g in m s⁻² and μ in g mol⁻¹, their Eq. 1's figures, which agree with their
///   Fig. 1's totals; ±0.1 dex), which hold no zinc; at 2,000 K the species below that figure's
///   floor (`SiO`, Fe, Mg, `SiO₂`, K) are VF13's Fig. 2a, as is K at 2,500 K;
/// - 3,500 and 4,000 K: VF13's Fig. 2a (its mole fractions times its Fig. 3a total; ±0.05 dex),
///   without its zinc, about 2% of its vapour at 3,500 K and under 1% at 4,000 K.
///
/// Each row's sum lies within 0.1 dex of [`bse_log10_pressure_bar`] at its temperature, and its
/// O₂ and O balance what its metal-bearing species freed within 6% (5.2% at 3,500 K, without
/// VF13's zinc).
const BSE_PARTIALS_LOG10_BAR: [[f64; 8]; 5] = [
    [-3.86, -4.48, -5.42, -6.74, -6.51, -6.98, -7.67, -7.80],
    [-2.27, -2.86, -3.27, -3.42, -3.91, -3.97, -4.75, -5.88],
    [-1.29, -1.57, -1.74, -1.43, -2.43, -2.23, -2.79, -4.61],
    [-0.65, -0.43, -0.54, -0.12, -1.49, -1.11, -1.39, -3.84],
    [-0.20, 0.46, 0.38, 0.85, -0.80, -0.30, -0.34, -3.27],
];

/// The Jeans parameter below which a melt's vapour led by atoms is an outflow: 2.1, Volkov,
/// Johnson, Tucker and Erwin's critical value for an atomic gas heated at its base ("For an atomic
/// gas, the transition occurs over λ₀ ∼ 2–3, where the lower bound, λ₀ ∼ 2.1, corresponds to the
/// upper limit for isentropic, supersonic outflow", 2011, Astrophys. J. Lett. 729, L24, abstract,
/// read). Na, O, Fe, Mg and K are atoms, and they lead every vapour below about 3,100 K; the
/// transition above the critical value is read as bound.
pub const ATOMIC_CRITICAL_JEANS_PARAMETER: f64 = 2.1;

/// The Jeans parameter below which a melt's vapour led by molecules (O₂, `SiO` and `SiO₂`, more
/// than half of it, as above about 3,100 K) is an outflow: 2.4, Volkov et al.'s ("In a gas
/// composed of diatomic molecules, the transition shifts to λ₀ ∼ 2.4–3.6", 2011, abstract, read),
/// `SiO₂` read with them.
pub const DIATOMIC_CRITICAL_JEANS_PARAMETER: f64 = 2.4;

/// Whether each of [`SPECIES`] is a molecule, for the critical Jeans parameter.
const MOLECULAR: [bool; 8] = [false, true, false, true, false, false, true, false];

/// The composition class a melt's vapour pressure is read at (the [module](self)'s "The pressure"):
/// the nearest of SF09's computed melts by its silica and alkalis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MeltComposition {
    /// SF09's bulk silicate Earth: a magma ocean's melt, an ultramafic or Mg-rich lava, and
    /// every melt below the alkalis' and the silica's bounds.
    BulkSilicate,
    /// SF09's oceanic crust: an alkali-bearing basalt.
    OceanicCrust,
    /// SF09's continental crust: a silica-rich melt.
    ContinentalCrust,
}

impl MeltComposition {
    /// The class of a melt of major oxides `oxides`, weight per cent: [`ContinentalCrust`] from
    /// 60 wt% `SiO₂`, otherwise [`OceanicCrust`] from 1.0 wt% `Na₂O` + K₂O, otherwise
    /// [`BulkSilicate`].
    ///
    /// [`ContinentalCrust`]: Self::ContinentalCrust
    /// [`OceanicCrust`]: Self::OceanicCrust
    /// [`BulkSilicate`]: Self::BulkSilicate
    #[must_use]
    pub(crate) fn of_oxides(oxides: &[(Oxide, f64)]) -> Self {
        let wt = |oxide| {
            oxides
                .iter()
                .find(|&&(listed, _)| listed == oxide)
                .map_or(0.0, |&(_, w)| w)
        };
        if wt(Oxide::Silica) >= CONTINENTAL_SILICA_WT_PCT {
            Self::ContinentalCrust
        } else if wt(Oxide::Soda) + wt(Oxide::Potash) >= OCEANIC_ALKALIS_WT_PCT {
            Self::OceanicCrust
        } else {
            Self::BulkSilicate
        }
    }

    /// The class of the melt of the registry row `row`, a crust's lid, by its melt record's oxides
    /// (`of_oxides`): `basalt` the oceanic crust's, `granite` the continental
    /// crust's and every other lithology the BSE's, a row with no melt record among them (the
    /// chondrites, `cometary`, `phyllosilicate`, `graphite` and ice, whose melt, where it has one,
    /// is chondritic or another row's).
    #[must_use]
    pub fn of_row(row: SubstanceId) -> Self {
        row.substance()
            .liquid()
            .and_then(crate::substance::liquid::LiquidColumns::melt_oxides)
            .map_or(Self::BulkSilicate, Self::of_oxides)
    }

    /// The class's offset from the BSE's pressure at its nodes, dex.
    #[must_use]
    const fn offsets(self) -> [f64; 4] {
        match self {
            Self::BulkSilicate => [0.0; 4],
            Self::OceanicCrust => OCEANIC_CRUST_OFFSET_DEX,
            Self::ContinentalCrust => CONTINENTAL_CRUST_OFFSET_DEX,
        }
    }

    /// The class's offset from the BSE's pressure at `t_k`, dex (the [module](self)'s "The
    /// pressure").
    #[must_use]
    fn offset_dex(self, t_k: f64) -> f64 {
        let offsets = self.offsets();
        let last = OFFSET_NODES_K.len() - 1;
        if t_k <= OFFSET_NODES_K[0] {
            return offsets[0];
        }
        if t_k >= CONVERGED_K {
            return 0.0;
        }
        if t_k >= OFFSET_NODES_K[last] {
            return offsets[last] * (CONVERGED_K - t_k) / (CONVERGED_K - OFFSET_NODES_K[last]);
        }
        let k = OFFSET_NODES_K
            .windows(2)
            .position(|pair| t_k <= pair[1])
            .unwrap_or(last - 1);
        let w = inverse_weight(OFFSET_NODES_K[k], OFFSET_NODES_K[k + 1], t_k);
        offsets[k] + w * (offsets[k + 1] - offsets[k])
    }
}

/// The weight of `t_k` between the nodes `lo` and `hi`, K, linear in 1 ÷ T: 0 at `lo`, 1 at `hi`.
#[must_use]
fn inverse_weight(lo: f64, hi: f64, t_k: f64) -> f64 {
    (1.0 / t_k - 1.0 / lo) / (1.0 / hi - 1.0 / lo)
}

/// The total vapour pressure over a molten BSE at `t_k`, K, finite and positive, log₁₀ bar
/// (Fegley and Schaefer's Eq. 13, `BSE_PRESSURE_CONSTANT`'s): fitted from 2,000 to 6,000 K, read
/// below 2,000 K by its own extrapolation (within 0.1 dex of SF09's BSE down to 1,500 K, their
/// lowest), and held above 6,000 K (`BSE_PRESSURE_CEILING_K`); NaN for a temperature at or
/// below 0.
#[must_use]
pub fn bse_log10_pressure_bar(t_k: f64) -> f64 {
    let t = t_k.min(BSE_PRESSURE_CEILING_K);
    BSE_PRESSURE_CONSTANT + BSE_PRESSURE_INVERSE_K / t + BSE_PRESSURE_LOG_T * math::log10(t)
}

/// The BSE vapour's mole fractions at `t_k`, in [`SPECIES`]' order, summing to 1 (the
/// [module](self)'s "The species").
#[must_use]
fn bse_shares(t_k: f64) -> [f64; 8] {
    let last = SPECIES_NODES_K.len() - 1;
    let t = t_k.clamp(SPECIES_NODES_K[0], SPECIES_NODES_K[last]);
    let k = SPECIES_NODES_K
        .windows(2)
        .position(|pair| t <= pair[1])
        .unwrap_or(last - 1);
    let w = inverse_weight(SPECIES_NODES_K[k], SPECIES_NODES_K[k + 1], t);
    let (lo, hi) = (&BSE_PARTIALS_LOG10_BAR[k], &BSE_PARTIALS_LOG10_BAR[k + 1]);
    let mut partials = [0.0; 8];
    for (partial, (a, b)) in partials.iter_mut().zip(lo.iter().zip(hi)) {
        *partial = math::exp10(a + w * (b - a));
    }
    let sum = partials.iter().fold(0.0, |sum, p| sum + p);
    partials.map(|p| p / sum)
}

/// Whether a melt's vapour is held by its body or streams off it, by its Jeans parameter against
/// its critical value ([`ATOMIC_CRITICAL_JEANS_PARAMETER`], [`DIATOMIC_CRITICAL_JEANS_PARAMETER`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VapourEscape {
    /// λ at or above the critical value: a bound air over its melt.
    Bound,
    /// λ below it: a hydrodynamic outflow, its pressure the melt's at the surface.
    Outflow,
}

/// What a melt's vapour is derived from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SilicateVapourInputs {
    /// The melt's warmest temperature, K, finite and positive.
    pub melt_temperature: Kelvin,
    /// The melt's composition class.
    pub composition: MeltComposition,
    /// The body's surface gravity g = G M ÷ R², m s⁻², finite and positive.
    pub gravity: MetresPerSecondSquared,
    /// The body's radius, m, finite and positive.
    pub radius: Metres,
}

/// An input of [`SilicateVapour::derive`] that is not finite and positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveSilicateVapourError {
    /// The melt's temperature, K.
    MeltTemperature(f64),
    /// The body's surface gravity, m s⁻².
    Gravity(f64),
    /// The body's radius, m.
    Radius(f64),
}

impl fmt::Display for DeriveSilicateVapourError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MeltTemperature(t) => {
                write!(f, "melt temperature {t} K is not finite and positive")
            }
            Self::Gravity(g) => write!(f, "surface gravity {g} m s⁻² is not finite and positive"),
            Self::Radius(r) => write!(f, "radius {r} m is not finite and positive"),
        }
    }
}

impl Error for DeriveSilicateVapourError {}

/// The vapour a melt holds over it (the [module](self)'s model): its temperature, its pressure,
/// its species by mole fraction, their mean molar mass, its Jeans parameter and whether it is
/// bound or an outflow.
///
/// Built only by [`SilicateVapour::derive`], which guarantees every figure finite and positive and
/// the species' fractions summing to 1, largest first.
#[derive(Debug, Clone, PartialEq)]
pub struct SilicateVapour {
    melt_temperature: Kelvin,
    pressure: Pascals,
    species: Vec<(SubstanceId, f64)>,
    mean_molar_mass_g_per_mol: f64,
    jeans_parameter: f64,
    critical_jeans_parameter: f64,
    escape: VapourEscape,
}

impl SilicateVapour {
    /// The vapour over the melt `inputs` describe.
    ///
    /// # Errors
    ///
    /// A [`DeriveSilicateVapourError`] for an input that is not finite and positive, the first in
    /// the order the melt's temperature, the gravity and the radius.
    ///
    /// # Panics
    ///
    /// Never: every species is a registry row with a molar mass, as the module's tests hold.
    ///
    /// # Examples
    ///
    /// A lava world's dayside pool at 2,500 K holds a sodium vapour bound by its super-Earth's
    /// gravity, where a 400 km dwarf planet's melt at 1,684 K streams off it:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::silicate_vapour::{
    ///     MeltComposition, SilicateVapour, SilicateVapourInputs, VapourEscape,
    /// };
    /// use hyperion_sim::units::consts::EARTH_RADIUS_M;
    /// use hyperion_sim::units::{Kelvin, Metres, MetresPerSecondSquared};
    ///
    /// // 8 M⊕ at 1.9 R⊕: g = 9.82 × 8 ÷ 1.9² m s⁻².
    /// let pool = SilicateVapour::derive(&SilicateVapourInputs {
    ///     melt_temperature: Kelvin::new(2_500.0),
    ///     composition: MeltComposition::BulkSilicate,
    ///     gravity: MetresPerSecondSquared::new(21.8),
    ///     radius: Metres::new(1.9 * EARTH_RADIUS_M),
    /// })?;
    /// assert_eq!(pool.species()[0].0.substance().key().as_str(), "Na");
    /// assert!((500.0..1_500.0).contains(&pool.pressure().value()));
    /// assert_eq!(pool.escape(), VapourEscape::Bound);
    ///
    /// let e101 = SilicateVapour::derive(&SilicateVapourInputs {
    ///     melt_temperature: Kelvin::new(1_684.4),
    ///     composition: MeltComposition::BulkSilicate,
    ///     gravity: MetresPerSecondSquared::new(0.254),
    ///     radius: Metres::new(3.99e5),
    /// })?;
    /// assert_eq!(e101.escape(), VapourEscape::Outflow);
    /// # Ok::<(), hyperion_sim::planetary::chem::silicate_vapour::DeriveSilicateVapourError>(())
    /// ```
    pub fn derive(inputs: &SilicateVapourInputs) -> Result<Self, DeriveSilicateVapourError> {
        let t_k = inputs.melt_temperature.value();
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(t_k) {
            return Err(DeriveSilicateVapourError::MeltTemperature(t_k));
        }
        if !positive(inputs.gravity.value()) {
            return Err(DeriveSilicateVapourError::Gravity(inputs.gravity.value()));
        }
        if !positive(inputs.radius.value()) {
            return Err(DeriveSilicateVapourError::Radius(inputs.radius.value()));
        }
        let log10_bar = bse_log10_pressure_bar(t_k) + inputs.composition.offset_dex(t_k);
        let pressure = Pascals::new(math::exp10(log10_bar) * PASCALS_PER_BAR);
        let shares = bse_shares(t_k);
        let mut species: Vec<(SubstanceId, f64)> = SPECIES
            .iter()
            .zip(shares)
            .map(|(&key, share)| {
                (
                    by_key(key).expect("a vapour species is a registry row"),
                    share,
                )
            })
            .collect();
        // Summed in `SPECIES`' declared order, before the sort, so that no share's rank moves it.
        let mean_molar_mass_g_per_mol = species.iter().fold(0.0, |sum, &(id, x)| {
            sum + x * id
                .substance()
                .molar_mass_g_per_mol()
                .expect("a vapour species has a molar mass")
        });
        let molecular =
            MOLECULAR.iter().zip(shares).fold(
                0.0,
                |sum, (&molecule, x)| if molecule { sum + x } else { sum },
            );
        let critical_jeans_parameter = if molecular > 0.5 {
            DIATOMIC_CRITICAL_JEANS_PARAMETER
        } else {
            ATOMIC_CRITICAL_JEANS_PARAMETER
        };
        species.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let molecule_kg = mean_molar_mass_g_per_mol * ATOMIC_MASS_CONSTANT_KG;
        let jeans_parameter = inputs.gravity.value() * inputs.radius.value() * molecule_kg
            / (BOLTZMANN_CONSTANT * t_k);
        let escape = if jeans_parameter < critical_jeans_parameter {
            VapourEscape::Outflow
        } else {
            VapourEscape::Bound
        };
        Ok(Self {
            melt_temperature: inputs.melt_temperature,
            pressure,
            species,
            mean_molar_mass_g_per_mol,
            jeans_parameter,
            critical_jeans_parameter,
            escape,
        })
    }

    /// The melt's temperature the vapour is at, K.
    #[must_use]
    pub const fn melt_temperature(&self) -> Kelvin {
        self.melt_temperature
    }

    /// Its total pressure over the melt, Pa: the sum of its species' partial pressures.
    #[must_use]
    pub const fn pressure(&self) -> Pascals {
        self.pressure
    }

    /// Its species as `(SubstanceId, x)` mole fractions, largest first with ties by `SubstanceId`,
    /// summing to 1.
    #[must_use]
    pub fn species(&self) -> &[(SubstanceId, f64)] {
        &self.species
    }

    /// Its species' mean molar mass μ = Σ xᵢ μᵢ, g mol⁻¹, each row's molar mass.
    #[must_use]
    pub const fn mean_molar_mass_g_per_mol(&self) -> f64 {
        self.mean_molar_mass_g_per_mol
    }

    /// Its Jeans parameter λ = G M m ÷ (k T R) = g R m ÷ (k T) at the body's surface, m its mean
    /// molecular mass, dimensionless.
    #[must_use]
    pub const fn jeans_parameter(&self) -> f64 {
        self.jeans_parameter
    }

    /// The critical Jeans parameter its species give: [`DIATOMIC_CRITICAL_JEANS_PARAMETER`] where
    /// molecules are more than half of it, otherwise [`ATOMIC_CRITICAL_JEANS_PARAMETER`].
    #[must_use]
    pub const fn critical_jeans_parameter(&self) -> f64 {
        self.critical_jeans_parameter
    }

    /// Whether the body holds it or it streams off: an outflow below its
    /// [`critical_jeans_parameter`](Self::critical_jeans_parameter).
    #[must_use]
    pub const fn escape(&self) -> VapourEscape {
        self.escape
    }
}
