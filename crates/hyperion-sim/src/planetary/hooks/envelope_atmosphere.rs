//! A gas envelope's visible atmosphere (plan 14, P14.T24.d with P14.T52.d): for every body in
//! [`SurfaceState::GasEnvelope`](crate::planetary::derive::atmosphere::SurfaceState::GasEnvelope),
//! the giants and every sub-Neptune, its internal temperature (P14.T24.d) and its
//! [`EnvelopeAtmosphere`] (P14.T52.d): its profile ([`EnvelopeProfile`], Guillot's grey profile
//! joined to its adiabat, P14.T24.d) on its own gas, its speciated, quenched and cold-trapped
//! composition at 1 bar, its condensate decks, its photochemical hazes and its absorbers at
//! 10 mbar.
//!
//! # The internal temperature ([`envelope_internal_temperature`])
//!
//! The profile needs an internal heat: with `T_int` = 0 its gradient falls to 0 at depth and no
//! envelope joins its adiabat, an isothermal column to 100 bar (decision-p14-t24d-profile §3.1).
//! The record gives a giant's internal luminosity from 0.3 Jupiter masses, faded in to full at
//! 0.414 by [`giant_share`] (ruling 112.7; P14.T11.d), and none below, so the generated Saturn,
//! every ice giant and every sub-Neptune would have none. This module states one, a cooling law
//! blended with the record's by [`giant_share`], the radius blend's weight
//! (decision-p14-t24d-profile §3.2–§3.3):
//!
//! ```text
//! T_int⁴ = L_int ÷ (4π R² σ) + (1 − s) T_law⁴
//! T_law  = 84.13 K × (M ÷ 95.161 M⊕)^0.2744 × (max(t, 1 Myr) ÷ 4.57 Gyr)^−0.32
//! ```
//!
//! with `L_int` the record's internal luminosity (already times s) and s = [`giant_share`] of M: the
//! law alone below 0.3 Jupiter masses, the record's alone from 0.414, continuous in mass and time.
//! A blend rather than the larger of the two, so that the law never overrides the record's giant
//! cooling where that applies: with modern anchors it would put Jupiter at 117.1 K against the
//! record's 104.8 K.
//!
//! - **The anchor** ([`COOLING_LAW_TEMPERATURE`] at [`COOLING_LAW_MASS`]) is Saturn's measured
//!   internal heat, 2.84 ± 0.20 W m⁻² (Wang et al. 2024, Nat. Commun. 15, 5045, Cassini), (F ÷
//!   σ)^¼ = 84.13 K, at the Solar System table's Saturn, 568.32 × 10²⁴ kg.
//! - **The mass index** ([`COOLING_LAW_MASS_INDEX`]) is ln(84.13 ÷ 52.57) ÷ ln(95.161 ÷ 17.148),
//!   Neptune's 0.433 ± 0.046 W m⁻² (Pearl and Conrath 1991, JGR 96, 18921) at the table's mass;
//!   0.25–0.30 across both 1σ ranges.
//! - **The age index** ([`COOLING_LAW_AGE_INDEX`]) is Burrows et al. 2001's `T_eff` ∼ 1550 K (10⁹ yr
//!   ÷ t)^0.32 for older substellar objects (Rev. Mod. Phys. 73, 719, Eq. 2), its L ∝ t^−1.3 at
//!   nearly fixed radius; its mass index, the brown dwarfs', is not taken.
//! - **The reference age** is 4.57 Gyr ([`COOLING_LAW_AGE`]), the table's, and the floor
//!   [`MIN_AGE`], 1 Myr, plan 13's: a younger envelope has a 1 Myr one's heat.
//!
//! Checks, recorded: the law gives Jupiter 117.1 K against Li et al. 2018's 107.2 K (the record's
//! 104.8 K is the one used), and Uranus 50.2 K against Wang et al. 2025's 34.2 K (0.078 W m⁻²) or
//! Pearl et al. 1990's 29.3 K, the known outlier. It is fitted on 17–95 M⊕ and extrapolated to the
//! smallest sub-Neptunes, whose cores' heat is the physical floor.
//!
//! The law lives here, not in the record's derivation: the record's internal luminosity, the bulk
//! section's effective temperature and every reader of them (the rings, the moons' heating, the
//! photometry) are unchanged, so below 0.414 Jupiter masses the envelope's `T_int` is not the bulk
//! section's (decision-p14-t24d-profile §3.4). P14.T54.b.i passes P14.T50.b the envelope's
//! `T_eff`, (`T_eq`⁴ + `T_int`⁴)^¼, so that the profile and the helium read one temperature.
//!
//! An input out of its range (a mass that is not finite and positive, an age that is not finite, a
//! luminosity that is not finite and not negative, a radius that is not finite and positive) is
//! refused ([`DeriveInternalTemperatureError`]) rather than given a NaN. [`EnvelopeAtmosphere`]
//! takes `T_int` as an input and refuses one that is not finite and positive, as
//! [`EnvelopeProfile::derive`] refuses one that is not finite, so a NaN reaches neither the profile
//! nor the record.
//!
//! # The visible atmosphere ([`EnvelopeAtmosphere`], P14.T52.d)
//!
//! [`EnvelopeAtmosphere::derive`] chains the envelope's chemistry along its own profile: P14.T50.b's
//! element abundances ([`EnvelopeAbundances`]), speciated by P14.T52.a's [`Network`], quenched and
//! photolysed by P14.T52.b's [`Quench`] and rules, and condensed by P14.T52.c's loop
//! ([`Condensation`]) into P14.T24.c's inventory types. It solves P14.T52.a's equilibrium at seven
//! stated levels in all, never per layer of the profile nor inside another solver's steps (P14.T52.a's
//! cost rule), and runs the loop twice.
//!
//! ## The profile, in two passes
//!
//! The adiabat's `c_p` reads the deep gas, CO-rich on a hot envelope and CH₄-rich on a cold one,
//! which the profile itself sets. So the gas is speciated twice at [`ADIABAT_BOTTOM`] (100 bar):
//! first at the network's floor, 300 K, the cold limit, on whose gas a first profile is derived;
//! then at that profile's temperature there, held at the network's ceiling (6,000 K) where it is
//! hotter, on whose gas the published profile is derived. Each pass's mixture is its speciation's
//! rows with gas columns, those under [`COMPOSITION_FLOOR`] dropped (together they move `c_p` by
//! under 10⁻⁹, and each costs a statistical sum at every level of the join's scan), renormalised.
//! `T_irr` is [`irradiation_temperature`] of the equilibrium temperature, and Guillot's parameters
//! are the caller's ([`GreyProfile::ADOPTED`] for every envelope of the record).
//!
//! ## The quench, and the gas at two levels
//!
//! P14.T52.b's [`Quench`] along the profile, in a [`MixingColumn`] of the second pass's mean molar
//! mass and [`EddyDiffusion::JUPITER_LIKE`] (10³ m² s⁻¹, Zahnle and Marley 2014's "more
//! Jupiter-like Kzz = 10^7 cm2/s", the constant P14.T52.b states "P14.T52.d gives every
//! envelope"; decision-p14-t24d-profile §4), at the metallicity E × 10^\[Fe/H\] (P14.T50.b's
//! enrichment times the host's, as the column's and the rules' docs ask), searched from
//! [`ABSORBER_LEVEL`], the shallowest level read, down to [`QUENCH_COLUMN_BOTTOM`], 1 kbar, P14.T52.a's
//! ideal gas's limit. Its gas is read at two levels: [`COMPOSITION_LEVEL`], 1 bar, from which the
//! decks form and which the record publishes, and [`ABSORBER_LEVEL`], 10 mbar, the absorbers' and
//! the photochemistry's. A read level hotter than the network's records (6,000 K) is refused
//! ([`DeriveEnvelopeAtmosphereError::LevelAboveRecords`]): nothing is extrapolated. The quench's own
//! levels never reach the ceiling: a level at which the chemistry is as slow as the mixing is far
//! cooler (about 900–2,300 K over Zahnle and Marley's fits), and the column's hot bottom is only
//! scanned, never solved, so its depth needs no cap.
//!
//! ## The decks
//!
//! P14.T52.c's loop on the 1-bar gas (P14.T52.c's record: "T52.d should pass T52.a's gas at the
//! level its decks' carriers hold, such as the 1-bar"; a gas-only equilibrium keeps each metal's
//! whole gas, which the metals' laws read), along the profile, in a column of P14.T24.c's
//! [`CLOUD_EDDY_DIFFUSION`] (10 m² s⁻¹, Ackerman and Marley's `K_min`), from [`DECK_COLUMN_BOTTOM`]
//! (10 kbar, below every volatile's base on a gas giant; Jupiter's iron and perovskite, and most of
//! a colder giant's refractories and salts, are saturated there already and bound at it, their
//! condensate from below not counted) up to [`DECK_COLUMN_TOP`] (1 Pa), covering the whole sky.
//! Each deck takes the one rule of its row's kind ([`DeckKind`]; decision-bump-plan-after-22, "The
//! decks' `f_sed`, K and humidity", G5: "T52.d gives every condensate row one stated rule, by row
//! kind, so that no deck falls between the two"), each at `σ_g` [`DECK_WIDTH`] (2):
//!
//! - **volatiles** (a row's own saturation curve, ammonium hydrosulphide's product law, the acid's
//!   curve): P14.T24.c's [`sedimentation_for_supply`], an envelope's humidity 1 (G5), in two
//!   passes as P14.T24.c takes them, the supply read at the first pass's base;
//! - **refractories** ([`REFRACTORY_CONDENSATES`]): [`REFRACTORY_SEDIMENTATION`], 2;
//! - **sulphides and salts** ([`SULPHIDE_AND_SALT_CONDENSATES`]):
//!   [`SULPHIDE_AND_SALT_SEDIMENTATION`], 4.5.
//!
//! K is `K_min` for every deck. Ackerman and Marley's eq. 5 convective K (F = σ `T_int`⁴, L = H;
//! decision-p14-t24d-profile §5) for a hot deck whose base lies below the join is recorded, not
//! built: at a fixed `f_sed` it moves only the particles' sizes (the columns and the traps follow
//! d`q_t` ÷ d ln p = `f_sed` `q_c`, free of K), and it would cost one more run of the loop for each
//! such deck, ten on a cold giant. Those decks' particles are therefore `K_min`'s, smaller than
//! eq. 5's, so their τ is high; they lie far below the photosphere.
//!
//! The column's one mean molar mass differs between the passes. The first takes the 1-bar gas's
//! own, its condensables all vapour; the second, and the supply rule, the 1-bar gas trimmed by the
//! first pass's decks as the composition is (below), the published composition's own, so that the
//! decks and the published molar mass agree. A deck's base reads only the profile and the mole
//! fractions, so no base moves with it; its column, sizes and supply do, and the gas about the
//! visible decks is the trimmed one: on an ice giant the untrimmed gas still holds the few percent
//! of water that its deep water deck takes (Neptune's 4.11 g mol⁻¹ against its composition's 3.07),
//! which raised its methane deck's τ by a fifth (6.95 against 5.77).
//!
//! The decks are published as P14.T24.c's: [`AerosolDeck::of`] each, [`AerosolMode::of_settling`]
//! each mode at its base's temperature (a `NoDensity` mode passing through, labelled), and the
//! loop's rows not formed.
//!
//! ## The composition at 1 bar
//!
//! The 1-bar gas, each species scaled by what the decks leave of its vapour there
//! ([`Condensation::remaining`] over the deep gas's amount): a species that is a deck's own or
//! co-reacting vapour (H₂O, NH₃, CH₄, H₂S, …) by its own, any other carrier of a trapped element
//! (TiO, SiO, HCl, a metal's atom and its ion) by its element's. No species is both, nor carries
//! two trapped elements, since each of P14.T52.a's trace species holds one trace atom. Three metals
//! have no law aloft of their own, and are taken with the deck that condenses them in the gas
//! ([`CO_TRAPPED`], a stated stand-in until laws of their own): nickel with iron, vanadium (and VO)
//! and calcium with perovskite's titanium; each such carrier takes the smaller of its own share and
//! its host element's. The charge is not rebalanced: an ion goes with its metal, the electrons
//! stay. Shares under [`COMPOSITION_FLOOR`] are dropped and the rest renormalised to 1, sorted
//! largest first (ties by row), its mean molar mass and `c_p` ÷ R at the 1-bar temperature
//! P14.T49.b's mixture functions'. The free electron, H⁻ and the ions are members where they reach
//! the floor, as an ideal gas's.
//!
//! ## The absorbers at 10 mbar
//!
//! R08.T4.e's lines (science-r08-unowned-absorbers §2.3, ruling 1: "T52.d publishes every atom and
//! ion T52.a gives, each with its column and layer"): every atom and first ion of P14.T52.a's metals,
//! with TiO, VO, SiO, H, H⁻ and e⁻ ([`ABSORBERS`]), from the 10-mbar gas trimmed as the composition
//! is (a cold-trapped metal vanishes) and renormalised, each a slab of its 10-mbar share from
//! [`ABSORBER_LEVEL`] to [`DECK_COLUMN_TOP`] (§2.2: "The 10 mbar column is the realistic one"; its
//! column above 0.1 bar is the upper bound, and would carry an ultra-hot dayside's 10-mbar ions
//! down to where its atoms hold the metals), listed where its share is at least
//! [`COMPOSITION_FLOOR`]. Its column is x (`p_base` − `p_top`) `N_A` ÷ (μ g), μ the 10-mbar
//! gas's. FeH has no thermochemistry ([`formation::NOT_MODELLED`]) and is listed as not modelled
//! ([`EnvelopeAtmosphere::not_modelled`]). Photochemical SO₂ (below) is an absorber too, a slab of
//! its layer.
//!
//! ## The hazes
//!
//! Only on a starlit envelope, whose host's ultraviolet drives them, on an [`UpperAtmosphere`] of the
//! equilibrium temperature, the profile's skin, the metallicity, the gravity and the 10-mbar gas's
//! molar mass, each rule reading the trimmed 10-mbar gas, the gas above the decks:
//!
//! - **SO₂** ([`sulphur_dioxide`], Tsai et al. 2023) from its sulphur atoms per molecule and its
//!   water. P14.T52.b's record leaves its booking to this task: its H₂S + 2 H₂O → SO₂ + 3 H₂ acts
//!   within its layer, 10–0.01 mbar, while the composition lies below it at 1 bar and none of the
//!   10-mbar absorbers is H₂S, H₂O or H₂, so the booking moves no published quantity.
//! - **The S₈ haze** ([`sulphur_haze`], Gao et al. 2017) from the same sulphur.
//! - **Soot** ([`soot`], Gao et al. 2020) from its methane, the quenched methane above any cold trap,
//!   only within Gao et al.'s grid: their models span `T_eq` 700–2,800 K, so the rule's column is
//!   scaled by [`SOOT_GRID_EDGE`]'s linear taper, 0 at and below 600 K and whole from 700 K (stated;
//!   the rule's own 950 K edge is unchanged). The cool giants' stratospheric hazes are not modelled.
//! - **The ice giants' haze** ([`ice_giant_haze`], Irwin et al. 2022), based at the CH₄ deck's base,
//!   and **the chromophore** ([`chromophore`], Braude et al. 2020) at the NH₃ deck's, on a
//!   Jupiter-like ammonia cloud alone: an NH₃ deck with no CH₄ deck above it (stated; Braude et al.
//!   retrieve it on Jupiter alone). Each appears with its deck at full strength, a state change at
//!   the deck's onset, as P14.T52.b's record leaves it (no taper: none is sourced).
//!
//! Each haze is a mode of its own material ([`AerosolMode`], its deck its material, the whole sky
//! covered), its sizes its rule's (already volume-to-area radii), its τ(550) P14.T49.e's mass
//! extinction of its column at the profile's temperature at its base, or the rule's own optical
//! depth. A layer is a [`ModeProfile`] of its base and top; the ice giants' haze's mixing ratio,
//! rising with height as (p ÷ `p_base`)^−0.5, is taken well mixed (a profile's exponent is not
//! negative), its τ above p going as p ÷ `p_base` in place of (p ÷ `p_base`)^0.5. The chromophore's
//! Gaussian in ln p is a well-mixed slab between its 1/e points, `p_peak` e^±w. The ice giants' haze's
//! optical depth, 0.035 at 0.8 µm, is taken as its τ(550): P14.T49.e's table is 550 nm's alone, and
//! a small absorbing haze is thicker in the blue, so this is a lower bound.
//! [`EnvelopeAtmosphere::haze_optical_depth`] is the hazes' τ(550) summed over those modelled.
//!
//! ## What is not modelled
//!
//! - **An envelope that is not hydrogen-dominated**: P14.T52.a's speciation refuses it, and the
//!   record's whole envelope section is not modelled
//!   ([`DeriveEnvelopeAtmosphereError::NotHydrogenDominated`]; decision-p14-t24d-profile §4): its
//!   profile is H₂'s collision-induced absorption's.
//! - **Sudarsky et al. 2000's high silicate clouds** (their class V) outside a window: the grey
//!   profile's upper air at γ 0.4 lies between 0.91 and 1.12 `T_eq`, so a silicate deck reaches the
//!   photosphere only for a `T_eff` of about 1,650–1,800 K (their class reads "above about 1,500
//!   K"). Cooler, its base lies at its adiabat crossing, a hundred bars or more down; hotter, from
//!   about 1,850 K, no silicate deck forms. A non-grey profile, or γ above 1, is the route.
//! - **The convective K of the hot decks** (above), and **a deck's second crossing**: the loop
//!   forms each condensate's deck at its deepest crossing, so a vapour that saturates again in a
//!   cooler layer above is not condensed twice.
//! - **Nickel's, vanadium's and calcium's own condensates**: no law aloft takes Ni, V or the Ca
//!   aluminates, so each is held with its host deck's element ([`CO_TRAPPED`]); laws of their own
//!   are P14.T49.b's and P14.T52.c's to add. Vanadium so tracked leaves the gas too hot: its 50%
//!   removal is at 1,429 K against titanium's 1,582 K (Lodders 2003, Table 8), so VO and V are
//!   depleted in a window of roughly 1,750–1,930 K of `T_eq` where they should stay (Pelletier et
//!   al. 2023, Nature 619, 491, find V and VO near protosolar on WASP-76b with its Ti depleted).
//!
//! # On the record from 24
//!
//! This module is pure. The record calls it behind the air's sections' hold
//! (`system::envelope`, P14.T54.b.i; `AIR_SECTIONS_VERSION`), so that at generator version 23
//! nothing on the record calls it (decision-bump-plan-after-22 §1.3): from 24 the record's
//! envelope section of every sub-Neptune and giant is its [`EnvelopeAtmosphere`] on P14.T50.b's
//! abundances, at the envelope's `T_eff` (`T_eq`⁴ + `T_int`⁴)^¼ with this module's `T_int`.
//!
//! # Determinism
//!
//! Every count is fixed (the solves', the loop's and the profile's), every sum runs in registry
//! order, every transcendental is [`math`]'s and every comparison is explicit, so the result is the
//! same bits on every target.

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::chem::condense::{
    BuildModeError, BuildSedimentationError, Condensation, DeckBound, DeriveCondensationError,
    Mode, ModeProfile, NotFormed, Sedimentation,
};
use crate::planetary::chem::envelope_abundances::EnvelopeAbundances;
use crate::planetary::chem::equilibrium::{GasEquilibrium, Network, SolveEquilibriumError};
use crate::planetary::chem::photochem::{
    EvaluateRuleError, Haze, HazeAmount, HazeProfile, UpperAtmosphere, chromophore, ice_giant_haze,
    soot, sulphur_dioxide, sulphur_haze,
};
use crate::planetary::chem::quench::{
    ApplyQuenchError, DeriveQuenchError, EddyDiffusion, MixingColumn, Quench,
};
use crate::planetary::derive::radius::giant_share;
use crate::planetary::hooks::aerosols::{
    Absorber, AerosolDeck, AerosolMode, BuildAbsorberError, CLOUD_EDDY_DIFFUSION, DECK_WIDTH,
    ShapeClass, sedimentation_for_supply,
};
use crate::planetary::hooks::structure::envelope::{
    ADIABAT_BOTTOM, DeriveEnvelopeProfileError, EnvelopeProfile, EnvelopeProfileInputs,
    GreyProfile, irradiation_temperature,
};
use crate::stellar::substellar::MIN_AGE;
use crate::substance::aloft::{SaturationLaw, Vapour, law_aloft};
use crate::substance::{
    Element, MOLAR_GAS_CONSTANT, SubstanceId, by_key, formation, mean_molar_mass_g_per_mol,
    mixture_heat_capacity_over_r_at,
};
use crate::units::consts::{BOLTZMANN_CONSTANT, STEFAN_BOLTZMANN};
use crate::units::{
    Dex, EarthMasses, Kelvin, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, Pascals,
    Watts, Years,
};

#[cfg(test)]
mod tests;

/// The cooling law's temperature at its anchor, K: Saturn's internal heat, 2.84 ± 0.20 W m⁻² (Wang
/// et al. 2024, Nat. Commun. 15, 5045), (F ÷ σ)^¼ = 84.13 K (82.6–85.6 K at 1σ).
pub const COOLING_LAW_TEMPERATURE: Kelvin = Kelvin::new(84.13);

/// The cooling law's anchor mass: the Solar System table's Saturn, 568.32 × 10²⁴ kg, 95.161 M⊕.
pub const COOLING_LAW_MASS: EarthMasses = EarthMasses::new(95.161);

/// The cooling law's mass index b: 0.2744, ln(84.13 ÷ 52.57) ÷ ln(95.161 ÷ 17.148), with Neptune's
/// internal heat, 0.433 ± 0.046 W m⁻² (Pearl and Conrath 1991, JGR 96, 18921), 52.57 K at the
/// table's 17.148 M⊕; 0.25–0.30 across both anchors' 1σ ranges.
pub const COOLING_LAW_MASS_INDEX: f64 = 0.2744;

/// The cooling law's age index c: 0.32, Burrows et al. 2001's `T_eff` ∝ t^−0.32 for older
/// substellar objects (Rev. Mod. Phys. 73, 719, Eq. 2, after Burrows and Liebert 1993 and Marley
/// et al. 1996).
pub const COOLING_LAW_AGE_INDEX: f64 = 0.32;

/// The cooling law's reference age: 4.57 Gyr, the Solar System table's.
pub const COOLING_LAW_AGE: Years = Years::new(4.57e9);

/// [`cooling_law_temperature`] or [`envelope_internal_temperature`] refused an input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveInternalTemperatureError {
    /// The mass is not finite and positive.
    Mass(EarthMasses),
    /// The age is not finite.
    Age(Years),
    /// The internal luminosity is not finite and not negative.
    Luminosity(Watts),
    /// The radius is not finite and positive.
    Radius(Metres),
}

impl fmt::Display for DeriveInternalTemperatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mass(m) => write!(
                f,
                "an envelope's mass is finite and positive, got {} Earth masses",
                m.value()
            ),
            Self::Age(t) => write!(f, "an envelope's age is finite, got {} yr", t.value()),
            Self::Luminosity(l) => write!(
                f,
                "an internal luminosity is finite and not negative, got {} W",
                l.value()
            ),
            Self::Radius(r) => {
                write!(f, "a radius is finite and positive, got {} m", r.value())
            }
        }
    }
}

impl Error for DeriveInternalTemperatureError {}

/// The cooling law's internal temperature of an envelope of mass `mass` at age `age`.
///
/// It is 84.13 K (M ÷ 95.161 M⊕)^0.2744 (max(t, 1 Myr) ÷ 4.57 Gyr)^−0.32 (the [module](self)'s
/// documentation): positive and finite for every finite positive mass and every finite age, an
/// age under 1 Myr (a negative one too) taking a 1 Myr envelope's.
///
/// # Errors
///
/// [`DeriveInternalTemperatureError::Mass`] for a mass that is not finite and positive, and
/// [`DeriveInternalTemperatureError::Age`] for an age that is not finite.
///
/// # Examples
///
/// Saturn's internal heat at the table's mass and age is the anchor's:
///
/// ```
/// use hyperion_sim::planetary::hooks::envelope_atmosphere::{
///     COOLING_LAW_AGE, COOLING_LAW_MASS, DeriveInternalTemperatureError, cooling_law_temperature,
/// };
///
/// let saturn = cooling_law_temperature(COOLING_LAW_MASS, COOLING_LAW_AGE)?;
/// assert!((saturn.value() - 84.13).abs() < 1e-9);
/// # Ok::<(), DeriveInternalTemperatureError>(())
/// ```
pub fn cooling_law_temperature(
    mass: EarthMasses,
    age: Years,
) -> Result<Kelvin, DeriveInternalTemperatureError> {
    let (m, t) = (mass.value(), age.value());
    if !(m.is_finite() && m > 0.0) {
        return Err(DeriveInternalTemperatureError::Mass(mass));
    }
    if !t.is_finite() {
        return Err(DeriveInternalTemperatureError::Age(age));
    }
    let t = if t > MIN_AGE.value() {
        t
    } else {
        MIN_AGE.value()
    };
    Ok(Kelvin::new(
        COOLING_LAW_TEMPERATURE.value()
            * math::powf(m / COOLING_LAW_MASS.value(), COOLING_LAW_MASS_INDEX)
            * math::powf(t / COOLING_LAW_AGE.value(), -COOLING_LAW_AGE_INDEX),
    ))
}

/// The internal temperature of a gas envelope, the record's heat blended with the cooling law.
///
/// For an envelope of mass `mass`, radius `radius` and age `age` whose record radiates
/// `internal_luminosity` of its own (the record's, already times [`giant_share`]), it is
/// (`L_int` ÷ (4π R² σ) + (1 − s) `T_law`⁴)^¼, with s = [`giant_share`] of M and `T_law`
/// [`cooling_law_temperature`] (the [module](self)'s documentation): the record's alone from
/// 0.414 Jupiter masses and the law's alone below 0.3, finite and positive for every envelope. A
/// luminosity of −0 W gives +0's bits.
///
/// # Errors
///
/// [`DeriveInternalTemperatureError::Luminosity`] for a luminosity that is not finite and not
/// negative, [`DeriveInternalTemperatureError::Radius`] for a radius that is not finite and
/// positive, and a mass or an age [`cooling_law_temperature`] refuses.
///
/// # Examples
///
/// The generated Saturn, at 0.2994 Jupiter masses, has no internal luminosity in the record, and
/// takes the law's heat:
///
/// ```
/// use hyperion_sim::planetary::hooks::envelope_atmosphere::{
///     COOLING_LAW_AGE, COOLING_LAW_MASS, DeriveInternalTemperatureError,
///     envelope_internal_temperature,
/// };
/// use hyperion_sim::units::{Metres, Watts};
///
/// let saturn = envelope_internal_temperature(
///     Watts::new(0.0),
///     Metres::new(5.8232e7),
///     COOLING_LAW_MASS,
///     COOLING_LAW_AGE,
/// )?;
/// assert!((saturn.value() - 84.13).abs() < 1e-9);
/// # Ok::<(), DeriveInternalTemperatureError>(())
/// ```
pub fn envelope_internal_temperature(
    internal_luminosity: Watts,
    radius: Metres,
    mass: EarthMasses,
    age: Years,
) -> Result<Kelvin, DeriveInternalTemperatureError> {
    let (luminosity, r) = (internal_luminosity.value(), radius.value());
    if !(luminosity.is_finite() && luminosity >= 0.0) {
        return Err(DeriveInternalTemperatureError::Luminosity(
            internal_luminosity,
        ));
    }
    if !(r.is_finite() && r > 0.0) {
        return Err(DeriveInternalTemperatureError::Radius(radius));
    }
    // A signed zero's sign is not portable through max; the luminosity is +0 or positive here.
    let luminosity = if luminosity > 0.0 { luminosity } else { 0.0 };
    let record = luminosity / (4.0 * core::f64::consts::PI * r * r * STEFAN_BOLTZMANN);
    let law = math::powi(cooling_law_temperature(mass, age)?.value(), 4);
    Ok(Kelvin::new(math::powf(
        record + (1.0 - giant_share(mass)) * law,
        0.25,
    )))
}

/// The level of the envelope's published composition, 1 bar (10⁵ Pa): P14.T54.b's
/// `BodyEnvelopeDto::composition`, and the level whose gas the decks form from (P14.T52.c's record:
/// "such as the 1-bar").
pub const COMPOSITION_LEVEL: Pascals = Pascals::new(1.0e5);

/// The absorbers' level, 10 mbar (10³ Pa): science-r08-unowned-absorbers §2.2's "10 mbar column",
/// "the realistic one: there an H⁻ photosphere sits at 3,000–4,600 K", where Kitzmann et al. 2018
/// (ApJ 863, 183, Fig. 2) have Fe⁺ above Fe on an ultra-hot dayside. Each absorber is a slab of its
/// share there from this level to [`DECK_COLUMN_TOP`]. It is also the shallowest level the quench
/// is applied at, and the base of Tsai et al. 2023's SO₂ layer.
pub const ABSORBER_LEVEL: Pascals = Pascals::new(1.0e3);

/// The `T_eq` below which no soot is drawn and above which all of it is, K: 600 and 700, a linear
/// taper between them (stated) below Gao et al. 2020's grid, whose models span `T_eq` 700–2,800 K
/// (Nature Astronomy 4, 951); [`soot`]'s own 950 K edge, above which it gives way to the silicates,
/// is unchanged.
pub const SOOT_GRID_EDGE: (Kelvin, Kelvin) = (Kelvin::new(600.0), Kelvin::new(700.0));

/// The deepest level the quench is sought at, 1 kbar (10⁸ Pa): [`MixingColumn::bottom`]'s "within
/// P14.T52.a's ideal gas, about 1 kbar", the depth P14.T52.b's Jupiter column searches from.
pub const QUENCH_COLUMN_BOTTOM: Pascals = Pascals::new(1.0e8);

/// The top of the decks' column, 1 Pa (10 µbar): five decades above an envelope's photosphere,
/// within its skin, and the absorbers' slabs' top (their base [`ABSORBER_LEVEL`]).
pub const DECK_COLUMN_TOP: Pascals = Pascals::new(1.0);

/// The bottom of the decks' column, 10 kbar (10⁹ Pa), the deepest level the loop reads the profile
/// at (the adiabat's last segment below 100 bar): below every volatile's base on the table's gas
/// giants. A deck already saturated there is bound at it, its condensate from below not
/// counted: Jupiter's iron and perovskite, six of Saturn's refractories and salts, eight of each ice
/// giant's, all far below the photosphere.
pub const DECK_COLUMN_BOTTOM: Pascals = Pascals::new(1.0e9);

/// The least share of a gas a published species holds, and the least share a profile's mixture
/// keeps, 10⁻¹²: the composition's (dropped below it and the rest renormalised) and the absorbers'
/// (listed at or above it).
pub const COMPOSITION_FLOOR: f64 = 1e-12;

/// The refractory condensates, whose decks take [`REFRACTORY_SEDIMENTATION`]: iron, the silicates,
/// quartz, corundum, perovskite and titanium dioxide, every row with a vapour law aloft but the
/// sulphides', the salts' and chromium's. A closed set, for the composition audit.
pub const REFRACTORY_CONDENSATES: [&str; 7] =
    ["Fe", "MgSiO3", "Mg2SiO4", "SiO2", "Al2O3", "CaTiO3", "TiO2"];

/// The sulphides and salts (and chromium), whose decks take [`SULPHIDE_AND_SALT_SEDIMENTATION`]:
/// Morley et al. 2012's (ApJ 756, 172) set of a T dwarf's clouds, Cr, `MnS`, Na₂S, ZnS and KCl. A
/// closed set, for the composition audit.
pub const SULPHIDE_AND_SALT_CONDENSATES: [&str; 5] = ["KCl", "ZnS", "Na2S", "MnS", "Cr"];

/// A refractory deck's `f_sed`, 2: Saumon and Marley 2008 (ApJ 689, 1327, §2.5 and §4.1): "the fsed
/// = 2 evolution sequence can be used for all cloudy cases", L dwarfs fitting 1–3; Morley et al.
/// 2012 (§4.2.2) name them: "silicate, iron, and corundum clouds with … fsed = 2 ± 1".
pub const REFRACTORY_SEDIMENTATION: f64 = 2.0;

/// A sulphide's or a salt's deck's `f_sed`, 4.5: Morley et al. 2012's (§4.2.2) "fsed values of
/// 4-5" for the T dwarfs' sulphide and salt clouds (their best fits: UGPS 0722 at 5, Ross 458C at
/// 3), its middle.
pub const SULPHIDE_AND_SALT_SEDIMENTATION: f64 = 4.5;

/// The absorbers listed for R08.T4.e's lines, in registry order: every atom and first ion of
/// P14.T52.a's metals (Na, K, Mg, Al, Si, Ca, Ti, V, Cr, Mn, Fe, Ni and Zn), with TiO, VO, SiO, H,
/// H⁻ and e⁻ (plan 14, P14.T52.d; science-r08-unowned-absorbers §2.3, ruling 1). FeH, the plan's
/// other, has no thermochemistry ([`formation::NOT_MODELLED`]). A closed set, for the composition
/// audit.
pub const ABSORBERS: [&str; 32] = [
    "H", "e-", "H-", "Na", "K", "Fe", "Mg", "Si", "Ca", "Ti", "SiO", "TiO", "VO", "Al", "V", "Mn",
    "Ni", "Zn", "Na+", "Mg+", "Al+", "Si+", "K+", "Ca+", "Ti+", "V+", "Cr+", "Mn+", "Fe+", "Ni+",
    "Zn+", "Cr",
];

/// The metals with no law aloft of their own, each with the element whose deck takes it in the gas,
/// whose share above that deck it keeps (the [module](self)'s documentation): a stated stand-in
/// until laws of their own, a closed set for the composition audit.
///
/// - **Nickel with iron**: Ni's 50% condensation temperature, 1,353 K, is 4 K below iron metal's
///   appearance (1,357 K) and 19 K above iron's own 50% (1,334 K), at 10⁻⁴ bar (Lodders 2003, ApJ
///   591, 1220, Table 8; §3.1: "iron metal is the host phase for other less abundant metals such as
///   Ni, Co, Ge, or Au").
/// - **Calcium with titanium**, as P14.T52.c's perovskite (CaTiO₃) traps it: calcium leaves the gas
///   in the calcium aluminates and gehlenite, not in hibonite, which holds one Ca to twelve Al, about
///   12% of the calcium (Wakeford et al. 2017, MNRAS 464, 4247, §2.1); "At 1818 K ... 97% of all Ca
///   and 96% of all Ti are in solids" (Lodders 2002, ApJ 577, 974, §3, Table 2); "Most Ca is
///   expected to be removed at deeper atmospheric levels by the condensation of refractory calcium
///   aluminates" (Visscher, Lodders and Fegley 2010, ApJ 716, 1060, §4.1). Titanium's condensation
///   lies within about 40–65 K of gehlenite's onset, aluminium's 90–140 K.
/// - **Vanadium with titanium**: "Condensation of V into solid solution starts when Ca titanates
///   begin to form" (Lodders 2002, §7; Lodders and Fegley 2006, Astrophysics Update 2, §1.5.2). But
///   "at a given T and Ptot, the fraction of V removed from the gas is much less than the fraction
///   of Ti removed" (Lodders 2002, §7): V's 50% removal is at 1,429 K against Ti's 1,582 K (Lodders
///   2003, Table 8), so tracking titanium removes vanadium about 150 K too hot (the
///   [module](self)'s "What is not modelled").
pub const CO_TRAPPED: [(Element, Element); 3] = [
    (Element::Ni, Element::Fe),
    (Element::V, Element::Ti),
    (Element::Ca, Element::Ti),
];

/// Avogadro's number, mol⁻¹, as the registry's own ratio of its gas constant to Boltzmann's.
const AVOGADRO: f64 = MOLAR_GAS_CONSTANT / BOLTZMANN_CONSTANT;

/// Grams per kilogram.
const GRAMS_PER_KILOGRAM: f64 = 1.0e3;

/// Which of the envelope's stated `f_sed` rules a deck takes, by its condensate's law aloft (the
/// [module](self)'s documentation; decision-bump-plan-after-22, "The decks' `f_sed`, K and
/// humidity", G5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeckKind {
    /// A volatile's: a row's own saturation curve
    /// ([`SaturationLaw::Diagram`]: water, methane, ammonia, hydrogen sulphide and the other
    /// condensables), ammonium hydrosulphide's product law or the acid's curve. P14.T24.c's
    /// [`sedimentation_for_supply`].
    Volatile,
    /// A refractory's ([`REFRACTORY_CONDENSATES`]): [`REFRACTORY_SEDIMENTATION`].
    Refractory,
    /// A sulphide's or a salt's ([`SULPHIDE_AND_SALT_CONDENSATES`]):
    /// [`SULPHIDE_AND_SALT_SEDIMENTATION`].
    SulphideOrSalt,
}

impl DeckKind {
    /// The kind of the deck of `condensate`, or `None` for a row with no law aloft. A row with a
    /// vapour law aloft is a sulphide or a salt where [`SULPHIDE_AND_SALT_CONDENSATES`] names it, and
    /// otherwise a refractory, Saumon and Marley's `f_sed` being theirs "for all cloudy cases" (the
    /// tests hold that [`REFRACTORY_CONDENSATES`] names every other).
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::hooks::envelope_atmosphere::DeckKind;
    /// use hyperion_sim::substance::by_key;
    ///
    /// let kind = |key: &str| by_key(key).and_then(DeckKind::of);
    /// assert_eq!(kind("NH3"), Some(DeckKind::Volatile));
    /// assert_eq!(kind("Mg2SiO4"), Some(DeckKind::Refractory));
    /// assert_eq!(kind("Na2S"), Some(DeckKind::SulphideOrSalt));
    /// assert_eq!(kind("He"), None);
    /// ```
    #[must_use]
    pub fn of(condensate: SubstanceId) -> Option<Self> {
        let law = law_aloft(condensate)?;
        Some(match law.law() {
            SaturationLaw::Diagram
            | SaturationLaw::Product { .. }
            | SaturationLaw::KulmalaLaaksonen => Self::Volatile,
            SaturationLaw::Vapour { .. } => {
                if SULPHIDE_AND_SALT_CONDENSATES.contains(&condensate.substance().key().as_str()) {
                    Self::SulphideOrSalt
                } else {
                    Self::Refractory
                }
            }
        })
    }

    /// The kind's fixed `f_sed`: [`REFRACTORY_SEDIMENTATION`] or
    /// [`SULPHIDE_AND_SALT_SEDIMENTATION`]; `None` for a volatile, whose `f_sed` is its supply's.
    #[must_use]
    pub const fn fixed_sedimentation(self) -> Option<f64> {
        match self {
            Self::Volatile => None,
            Self::Refractory => Some(REFRACTORY_SEDIMENTATION),
            Self::SulphideOrSalt => Some(SULPHIDE_AND_SALT_SEDIMENTATION),
        }
    }
}

/// What [`EnvelopeAtmosphere::derive`] reads, as plain values. The derivation checks each, so they
/// are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopeAtmosphereInputs<'a> {
    /// The envelope's element abundances, P14.T50.b's.
    pub abundances: &'a EnvelopeAbundances,
    /// P14.T52.a's network, the registry's ([`Network::registry`]), built once by the caller.
    pub network: &'a Network,
    /// The internal temperature `T_int`, finite and positive: P14.T24.d's
    /// [`envelope_internal_temperature`].
    pub internal_temperature: Kelvin,
    /// The equilibrium temperature `T_eq`, finite and not negative: the record's, of the starlight
    /// alone, over the whole sphere with its Bond albedo folded in (ruling 112.7's light-only split).
    pub equilibrium_temperature: Kelvin,
    /// The gravity, the bulk section's surface gravity, finite and positive.
    pub gravity: MetresPerSecondSquared,
    /// The host's \[Fe/H\], finite: with P14.T50.b's enrichment E it sets the metallicity, E ×
    /// 10^\[Fe/H\] times solar, that the quench, the laws aloft and the rules read.
    pub fe_h: Dex,
    /// Guillot's parameters: [`GreyProfile::ADOPTED`] for every envelope of the record.
    pub grey: GreyProfile,
    /// Whether a star's ultraviolet lights the envelope, which drives its photochemistry: without
    /// it no SO₂ and no haze form.
    pub starlit: bool,
}

/// Why [`EnvelopeAtmosphere::derive`] gives no atmosphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveEnvelopeAtmosphereError {
    /// The internal temperature is not finite and positive.
    InternalTemperature(Kelvin),
    /// The equilibrium temperature is not finite and not negative.
    EquilibriumTemperature(Kelvin),
    /// The gravity is not finite and positive.
    Gravity(MetresPerSecondSquared),
    /// The host's \[Fe/H\] is not finite.
    FeH(Dex),
    /// The metallicity, E × 10^\[Fe/H\] times solar, is not finite and positive: an \[Fe/H\] whose
    /// power of ten overflows, or underflows to 0.
    Metallicity(f64),
    /// The envelope is not hydrogen-dominated: its all-in hydrides would take more than half its
    /// hydrogen (P14.T52.a), so the record's whole envelope section is not modelled
    /// (decision-p14-t24d-profile §4; [`is_not_modelled`](Self::is_not_modelled)).
    NotHydrogenDominated {
        /// The share of the hydrogen the hydrides would take.
        share: f64,
    },
    /// A level the record reads, 1 bar or 10 mbar, is hotter than the network's records, above
    /// which nothing is extrapolated.
    LevelAboveRecords {
        /// The level.
        pressure: Pascals,
        /// The profile's temperature there.
        temperature: Kelvin,
    },
    /// A pass of the profile's speciation refused its gas.
    Speciation {
        /// The pass's level.
        pressure: Pascals,
        /// Why.
        error: SolveEquilibriumError,
    },
    /// The profile refused its inputs.
    Profile(DeriveEnvelopeProfileError),
    /// The quench refused its column or its gas.
    Quench(DeriveQuenchError),
    /// The quenched gas could not be given at a level.
    Gas {
        /// The level.
        pressure: Pascals,
        /// Why.
        error: ApplyQuenchError,
    },
    /// The condensation loop refused its gas or its column.
    Condensation(DeriveCondensationError),
    /// A deck's sedimentation is not one.
    Sedimentation(BuildSedimentationError),
    /// A photochemical rule refused its inputs.
    Rule(EvaluateRuleError),
    /// A mode could not be built.
    Mode(BuildModeError),
    /// An absorber could not be built.
    Absorber(BuildAbsorberError),
    /// A gas has no molar mass or no heat capacity, which the registry's gas rows always have.
    Mixture,
}

impl DeriveEnvelopeAtmosphereError {
    /// Whether the record's whole envelope section is `NotModelled` for this refusal: a gas that is
    /// not hydrogen-dominated (decision-p14-t24d-profile §4). The others are inputs out of range or
    /// a level past the records.
    #[must_use]
    pub const fn is_not_modelled(&self) -> bool {
        matches!(self, Self::NotHydrogenDominated { .. })
    }
}

impl fmt::Display for DeriveEnvelopeAtmosphereError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InternalTemperature(t) => write!(
                f,
                "an envelope's internal temperature is finite and positive, got {} K",
                t.value()
            ),
            Self::EquilibriumTemperature(t) => write!(
                f,
                "an equilibrium temperature is finite and not negative, got {} K",
                t.value()
            ),
            Self::Gravity(g) => write!(
                f,
                "a gravity is finite and positive, got {} m/s2",
                g.value()
            ),
            Self::FeH(fe_h) => write!(f, "a host's [Fe/H] is finite, got {} dex", fe_h.value()),
            Self::Metallicity(m) => {
                write!(
                    f,
                    "an envelope's metallicity is finite and positive, got {m}"
                )
            }
            Self::NotHydrogenDominated { share } => write!(
                f,
                "the envelope is not hydrogen-dominated: its hydrides would take {share} of its \
                 hydrogen"
            ),
            Self::LevelAboveRecords {
                pressure,
                temperature,
            } => write!(
                f,
                "the profile gives {} K at {} Pa, above the network's records",
                temperature.value(),
                pressure.value()
            ),
            Self::Speciation { pressure, error } => {
                write!(f, "the speciation at {} Pa: {error}", pressure.value())
            }
            Self::Profile(error) => write!(f, "the profile: {error}"),
            Self::Quench(error) => write!(f, "the quench: {error}"),
            Self::Gas { pressure, error } => {
                write!(f, "the gas at {} Pa: {error}", pressure.value())
            }
            Self::Condensation(error) => write!(f, "the decks: {error}"),
            Self::Sedimentation(error) => write!(f, "a deck's sedimentation: {error}"),
            Self::Rule(error) => write!(f, "a photochemical rule: {error}"),
            Self::Mode(error) => write!(f, "a mode: {error}"),
            Self::Absorber(error) => write!(f, "an absorber: {error}"),
            Self::Mixture => write!(f, "a gas with no molar mass or heat capacity"),
        }
    }
}

impl Error for DeriveEnvelopeAtmosphereError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Speciation { error, .. } => Some(error),
            Self::Profile(error) => Some(error),
            Self::Quench(error) => Some(error),
            Self::Gas { error, .. } => Some(error),
            Self::Condensation(error) => Some(error),
            Self::Sedimentation(error) => Some(error),
            Self::Rule(error) => Some(error),
            Self::Mode(error) => Some(error),
            Self::Absorber(error) => Some(error),
            Self::InternalTemperature(_)
            | Self::EquilibriumTemperature(_)
            | Self::Gravity(_)
            | Self::FeH(_)
            | Self::Metallicity(_)
            | Self::NotHydrogenDominated { .. }
            | Self::LevelAboveRecords { .. }
            | Self::Mixture => None,
        }
    }
}

impl From<DeriveEnvelopeProfileError> for DeriveEnvelopeAtmosphereError {
    fn from(error: DeriveEnvelopeProfileError) -> Self {
        Self::Profile(error)
    }
}

impl From<DeriveQuenchError> for DeriveEnvelopeAtmosphereError {
    fn from(error: DeriveQuenchError) -> Self {
        Self::Quench(error)
    }
}

impl From<DeriveCondensationError> for DeriveEnvelopeAtmosphereError {
    fn from(error: DeriveCondensationError) -> Self {
        Self::Condensation(error)
    }
}

impl From<BuildSedimentationError> for DeriveEnvelopeAtmosphereError {
    fn from(error: BuildSedimentationError) -> Self {
        Self::Sedimentation(error)
    }
}

impl From<EvaluateRuleError> for DeriveEnvelopeAtmosphereError {
    fn from(error: EvaluateRuleError) -> Self {
        Self::Rule(error)
    }
}

impl From<BuildModeError> for DeriveEnvelopeAtmosphereError {
    fn from(error: BuildModeError) -> Self {
        Self::Mode(error)
    }
}

impl From<BuildAbsorberError> for DeriveEnvelopeAtmosphereError {
    fn from(error: BuildAbsorberError) -> Self {
        Self::Absorber(error)
    }
}

/// A gas envelope's visible atmosphere (P14.T52.d; the [module](self)'s documentation): its
/// profile, its composition at 1 bar, its decks and hazes, and its absorbers at 10 mbar.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeAtmosphere {
    profile: EnvelopeProfile,
    composition: Vec<(SubstanceId, f64)>,
    mean_molar_mass_g_per_mol: f64,
    heat_capacity_over_r: f64,
    upper_gas: Vec<(SubstanceId, f64)>,
    decks: Vec<AerosolDeck>,
    deck_bounds: Vec<DeckBound>,
    deck_column_masses: Vec<KilogramsPerSquareMetre>,
    modes: Vec<AerosolMode>,
    haze_optical_depth: f64,
    absorbers: Vec<Absorber>,
    not_formed: Vec<(SubstanceId, NotFormed)>,
    not_modelled: Vec<SubstanceId>,
}

impl EnvelopeAtmosphere {
    /// The visible atmosphere of the envelope `inputs` describe (the [module](self)'s
    /// documentation): two profile passes, the quench, the gas at 1 bar and at 10 mbar, the decks in
    /// two passes, the composition, the absorbers and, starlit, the photochemistry.
    ///
    /// # Errors
    ///
    /// - [`DeriveEnvelopeAtmosphereError::InternalTemperature`],
    ///   [`EquilibriumTemperature`](DeriveEnvelopeAtmosphereError::EquilibriumTemperature),
    ///   [`Gravity`](DeriveEnvelopeAtmosphereError::Gravity),
    ///   [`FeH`](DeriveEnvelopeAtmosphereError::FeH) and
    ///   [`Metallicity`](DeriveEnvelopeAtmosphereError::Metallicity) for an input out of its range;
    /// - [`NotHydrogenDominated`](DeriveEnvelopeAtmosphereError::NotHydrogenDominated) for a gas
    ///   P14.T52.a does not speciate, whose section is not modelled;
    /// - [`LevelAboveRecords`](DeriveEnvelopeAtmosphereError::LevelAboveRecords) where the profile is
    ///   hotter than 6,000 K at 1 bar or 10 mbar;
    /// - the steps' own refusals ([`Speciation`](DeriveEnvelopeAtmosphereError::Speciation),
    ///   [`Profile`](DeriveEnvelopeAtmosphereError::Profile),
    ///   [`Quench`](DeriveEnvelopeAtmosphereError::Quench),
    ///   [`Gas`](DeriveEnvelopeAtmosphereError::Gas),
    ///   [`Condensation`](DeriveEnvelopeAtmosphereError::Condensation),
    ///   [`Sedimentation`](DeriveEnvelopeAtmosphereError::Sedimentation),
    ///   [`Rule`](DeriveEnvelopeAtmosphereError::Rule),
    ///   [`Mode`](DeriveEnvelopeAtmosphereError::Mode),
    ///   [`Absorber`](DeriveEnvelopeAtmosphereError::Absorber) and
    ///   [`Mixture`](DeriveEnvelopeAtmosphereError::Mixture)), which a checked envelope within
    ///   P14.T52.a's domain does not reach, kept as errors rather than panics.
    ///
    /// # Examples
    ///
    /// A Jupiter-like envelope, its abundances P14.T50.b's for a Jupiter's mass about a solar
    /// host, carries its ammonia cloud near 0.7 bar and its methane at about 2 × 10⁻³ of its
    /// hydrogen:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::envelope_abundances::{
    ///     EnvelopeAbundances, EnvelopeInputs,
    /// };
    /// use hyperion_sim::planetary::chem::equilibrium::Network;
    /// use hyperion_sim::planetary::chem::{AbundanceDraws, HostAbundances};
    /// use hyperion_sim::planetary::derive::{SnowLineSide, giant_composition};
    /// use hyperion_sim::planetary::hooks::envelope_atmosphere::{
    ///     EnvelopeAtmosphere, EnvelopeAtmosphereInputs,
    /// };
    /// use hyperion_sim::planetary::hooks::structure::envelope::GreyProfile;
    /// use hyperion_sim::substance::{Element, by_key};
    /// use hyperion_sim::units::consts::METRES_PER_AU;
    /// use hyperion_sim::units::{
    ///     Dex, EarthMasses, JupiterMasses, Kelvin, Metres, MetresPerSecondSquared, Years,
    /// };
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mass = EarthMasses::from(JupiterMasses::new(1.0));
    /// let solved = giant_composition(mass, SnowLineSide::Beyond)?;
    /// let abundances = EnvelopeAbundances::derive(&EnvelopeInputs {
    ///     mass,
    ///     fractions: solved.fractions(),
    ///     formation_distance: Metres::new(5.2 * METRES_PER_AU),
    ///     snow_line: Metres::new(2.255 * METRES_PER_AU),
    ///     fe_h: Dex::ZERO,
    ///     alpha_fe: Dex::ZERO,
    ///     host: HostAbundances::of(Dex::ZERO, &AbundanceDraws::MEDIAN),
    ///     effective_temperature: Kelvin::new(125.6),
    ///     age: Years::new(4.57e9),
    ///     surface_gravity: MetresPerSecondSquared::new(24.79),
    ///     primordial_helium_per_hydrogen: Element::He.protosolar_abundance_per_hydrogen(),
    /// })?;
    /// let network = Network::registry()?;
    /// let jupiter = EnvelopeAtmosphere::derive(&EnvelopeAtmosphereInputs {
    ///     abundances: &abundances,
    ///     network: &network,
    ///     internal_temperature: Kelvin::new(104.8),
    ///     equilibrium_temperature: Kelvin::new(103.6),
    ///     gravity: MetresPerSecondSquared::new(24.79),
    ///     fe_h: Dex::ZERO,
    ///     grey: GreyProfile::ADOPTED,
    ///     starlit: true,
    /// })?;
    /// let ammonia = by_key("NH3").ok_or("a row")?;
    /// let deck = jupiter
    ///     .decks()
    ///     .iter()
    ///     .find(|d| d.condensate() == ammonia)
    ///     .ok_or("an ammonia deck")?;
    /// assert!((0.5..1.0).contains(&(deck.base().value() / 1e5)));
    /// let share = |key: &str| {
    ///     let id = by_key(key)?;
    ///     jupiter.composition().iter().find(|(s, _)| *s == id).map(|&(_, x)| x)
    /// };
    /// let methane = share("CH4").ok_or("methane")? / share("H2").ok_or("hydrogen")?;
    /// assert!((1.5e-3..3e-3).contains(&methane));
    /// # Ok(())
    /// # }
    /// ```
    pub fn derive(
        inputs: &EnvelopeAtmosphereInputs<'_>,
    ) -> Result<Self, DeriveEnvelopeAtmosphereError> {
        let (metallicity, equilibrium_temperature) = check(inputs)?;
        let inputs = &EnvelopeAtmosphereInputs {
            equilibrium_temperature,
            ..*inputs
        };
        let network = inputs.network;
        let elements: Vec<(Element, f64)> = inputs.abundances.elements().collect();
        let (profile, deep_molar_mass_g_per_mol) = profile_in_two_passes(inputs, &elements)?;
        let at = |p: Pascals| profile.temperature_at(p);
        let ceiling = network.ceiling().value();
        for pressure in [COMPOSITION_LEVEL, ABSORBER_LEVEL] {
            let temperature = at(pressure);
            // A NaN temperature is refused with a hot one.
            if temperature.value().is_nan() || temperature.value() > ceiling {
                return Err(DeriveEnvelopeAtmosphereError::LevelAboveRecords {
                    pressure,
                    temperature,
                });
            }
        }
        let quench = Quench::derive(
            network,
            &elements,
            at,
            &MixingColumn {
                gravity: inputs.gravity,
                molar_mass_g_per_mol: deep_molar_mass_g_per_mol,
                eddy_diffusion: EddyDiffusion::JUPITER_LIKE,
                metallicity,
                top: ABSORBER_LEVEL,
                bottom: QUENCH_COLUMN_BOTTOM,
            },
        )?;
        let gas_at = |pressure: Pascals| {
            quench
                .apply(network, at(pressure), pressure)
                .map(|gas| gas.mole_fractions().collect::<Vec<_>>())
                .map_err(|error| DeriveEnvelopeAtmosphereError::Gas { pressure, error })
        };
        let deep = gas_at(COMPOSITION_LEVEL)?;
        let upper = gas_at(ABSORBER_LEVEL)?;
        let column = MixingColumn {
            gravity: inputs.gravity,
            molar_mass_g_per_mol: molar_mass_g_per_mol(&deep)?,
            eddy_diffusion: CLOUD_EDDY_DIFFUSION,
            metallicity,
            top: DECK_COLUMN_TOP,
            bottom: DECK_COLUMN_BOTTOM,
        };
        let clouds = condense(&deep, at, &column)?;
        Self::assemble(inputs, metallicity, profile, &deep, &upper, &clouds)
    }

    /// The atmosphere's parts from its profile, its gas at 1 bar (`deep`) and at 10 mbar (`upper`),
    /// and its decks (`clouds`).
    fn assemble(
        inputs: &EnvelopeAtmosphereInputs<'_>,
        metallicity: f64,
        profile: EnvelopeProfile,
        deep: &[(SubstanceId, f64)],
        upper: &[(SubstanceId, f64)],
        clouds: &Condensation,
    ) -> Result<Self, DeriveEnvelopeAtmosphereError> {
        let drawn = drawn_vapours(clouds);
        let mut composition = composition_of(deep, clouds)?;
        composition.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let one_bar = profile.temperature_at(COMPOSITION_LEVEL);
        let heat_capacity_over_r = mixture_heat_capacity_over_r_at(&composition, one_bar)
            .ok_or(DeriveEnvelopeAtmosphereError::Mixture)?;
        let mean_molar_mass_g_per_mol = molar_mass_g_per_mol(&composition)?;
        let mut upper_gas = trimmed(upper, clouds, &drawn, ABSORBER_LEVEL);
        normalise(&mut upper_gas)?;
        let upper_molar_mass_g_per_mol = molar_mass_g_per_mol(&upper_gas)?;
        let mut decks = Vec::with_capacity(clouds.decks().len());
        let mut deck_bounds = Vec::with_capacity(clouds.decks().len());
        let mut deck_column_masses = Vec::with_capacity(clouds.decks().len());
        let mut modes = Vec::new();
        for deck in clouds.decks() {
            decks.push(AerosolDeck::of(deck));
            deck_bounds.push(deck.bound());
            deck_column_masses.push(deck.column_mass());
            for mode in deck.modes() {
                modes.push(AerosolMode::of_settling(mode, deck.base_temperature())?);
            }
        }
        let (hazes, sulphur_dioxide) = if inputs.starlit {
            let air = UpperAtmosphere {
                equilibrium_temperature: inputs.equilibrium_temperature,
                stratosphere_temperature: profile.skin_temperature(),
                metallicity,
                gravity: inputs.gravity,
                molar_mass_g_per_mol: upper_molar_mass_g_per_mol,
            };
            photochemistry(&air, &profile, &upper_gas, &decks)?
        } else {
            (Vec::new(), None)
        };
        let mut haze_optical_depth = 0.0;
        for haze in &hazes {
            // A haze whose optics are not modelled (none of the rules' materials, today) carries its
            // reason on its mode rather than a τ; it adds nothing here, as no number is known.
            if let Ok(depth) = haze.optical_depth() {
                haze_optical_depth += depth.value();
            }
        }
        modes.extend(hazes);
        let absorbers = absorbers(
            &upper_gas,
            upper_molar_mass_g_per_mol,
            inputs.gravity,
            sulphur_dioxide,
        )?;
        Ok(Self {
            profile,
            composition,
            mean_molar_mass_g_per_mol,
            heat_capacity_over_r,
            upper_gas,
            decks,
            deck_bounds,
            deck_column_masses,
            modes,
            haze_optical_depth,
            absorbers,
            not_formed: clouds.not_formed().to_vec(),
            not_modelled: formation::NOT_MODELLED
                .iter()
                .filter_map(|&(key, _)| by_key(key))
                .collect(),
        })
    }

    /// The envelope's profile, P14.T24.d's, on its own gas (the [module](self)'s two passes): the
    /// record's
    /// [`VerticalStructure::Envelope`](crate::planetary::hooks::structure::VerticalStructure::Envelope)
    /// is its clone, where the caller needs one.
    #[must_use]
    pub const fn profile(&self) -> &EnvelopeProfile {
        &self.profile
    }

    /// The internal temperature `T_int`, the profile's.
    #[must_use]
    pub const fn internal_temperature(&self) -> Kelvin {
        self.profile.internal_temperature()
    }

    /// The irradiation temperature `T_irr`, the profile's: `T_irr`⁴ = 4 `T_eq`⁴.
    #[must_use]
    pub const fn irradiation_temperature(&self) -> Kelvin {
        self.profile.irradiation_temperature()
    }

    /// The gas at 1 bar by mole fraction, quenched and trimmed by the decks, each share at least
    /// [`COMPOSITION_FLOOR`], summing to 1, largest first (ties by row).
    #[must_use]
    pub fn composition(&self) -> &[(SubstanceId, f64)] {
        &self.composition
    }

    /// The composition's mean molar mass, g mol⁻¹.
    #[must_use]
    pub const fn mean_molar_mass_g_per_mol(&self) -> f64 {
        self.mean_molar_mass_g_per_mol
    }

    /// The composition's `c_p` ÷ R at the profile's temperature at 1 bar, dimensionless (P14.T49.b's
    /// mixture, normal hydrogen).
    #[must_use]
    pub const fn heat_capacity_over_r(&self) -> f64 {
        self.heat_capacity_over_r
    }

    /// The gas at 10 mbar ([`ABSORBER_LEVEL`]) by mole fraction, quenched, trimmed by the decks and
    /// renormalised, every species of the network in registry order: the absorbers' and the
    /// photochemistry's.
    #[must_use]
    pub fn upper_gas(&self) -> &[(SubstanceId, f64)] {
        &self.upper_gas
    }

    /// The decks, hottest base first, as P14.T24.c's inventory states them.
    #[must_use]
    pub fn decks(&self) -> &[AerosolDeck] {
        &self.decks
    }

    /// Each deck's bound, in [`decks`](Self::decks)' order: `Bottom` for a vapour already saturated
    /// at [`DECK_COLUMN_BOTTOM`], whose condensate from below is not counted.
    #[must_use]
    pub fn deck_bounds(&self) -> &[DeckBound] {
        &self.deck_bounds
    }

    /// Each deck's condensate column mass, kg m⁻², in [`decks`](Self::decks)' order.
    #[must_use]
    pub fn deck_column_masses(&self) -> &[KilogramsPerSquareMetre] {
        &self.deck_column_masses
    }

    /// The modes: each deck's, in the decks' order, then the hazes' (S₈, soot, the ice giants'
    /// haze, the chromophore), each naming its own material as its deck.
    #[must_use]
    pub fn modes(&self) -> &[AerosolMode] {
        &self.modes
    }

    /// The τ(550) of the modes that name `condensate` as their deck, the sum of those modelled;
    /// `None` where no mode names it or none of them has a τ.
    #[must_use]
    pub fn deck_optical_depth(&self, condensate: SubstanceId) -> Option<f64> {
        let mut total = None;
        for mode in self.modes.iter().filter(|m| m.deck() == condensate) {
            if let Ok(depth) = mode.optical_depth() {
                total = Some(total.unwrap_or(0.0) + depth.value());
            }
        }
        total
    }

    /// The hazes' τ(550) summed over those modelled, dimensionless and not negative: P14.T54.b's
    /// `haze_tau_550`.
    #[must_use]
    pub const fn haze_optical_depth(&self) -> f64 {
        self.haze_optical_depth
    }

    /// The absorbers, in registry order, each at most once: [`ABSORBERS`]' species at or above
    /// [`COMPOSITION_FLOOR`] of the 10-mbar gas, and photochemical SO₂'s layer.
    #[must_use]
    pub fn absorbers(&self) -> &[Absorber] {
        &self.absorbers
    }

    /// The condensables the loop labels rather than traps, in registry order.
    #[must_use]
    pub fn not_formed(&self) -> &[(SubstanceId, NotFormed)] {
        &self.not_formed
    }

    /// The gas rows the plan lists that have no thermochemistry, so no share
    /// ([`formation::NOT_MODELLED`]: FeH), in that list's order.
    #[must_use]
    pub fn not_modelled(&self) -> &[SubstanceId] {
        &self.not_modelled
    }
}

/// The inputs' checks (their doc comments' ranges), giving the metallicity E × 10^\[Fe/H\] and the
/// equilibrium temperature, a −0 K one as +0.
fn check(
    inputs: &EnvelopeAtmosphereInputs<'_>,
) -> Result<(f64, Kelvin), DeriveEnvelopeAtmosphereError> {
    let positive = |x: f64| x.is_finite() && x > 0.0;
    let internal = inputs.internal_temperature;
    if !positive(internal.value()) {
        return Err(DeriveEnvelopeAtmosphereError::InternalTemperature(internal));
    }
    let equilibrium = inputs.equilibrium_temperature;
    let t_eq = equilibrium.value();
    if !(t_eq.is_finite() && t_eq >= 0.0) {
        return Err(DeriveEnvelopeAtmosphereError::EquilibriumTemperature(
            equilibrium,
        ));
    }
    // A signed zero's sign would reach the profile's T_irr and the rules; +0 is the one value.
    let equilibrium = Kelvin::new(if t_eq > 0.0 { t_eq } else { 0.0 });
    if !positive(inputs.gravity.value()) {
        return Err(DeriveEnvelopeAtmosphereError::Gravity(inputs.gravity));
    }
    if !inputs.fe_h.value().is_finite() {
        return Err(DeriveEnvelopeAtmosphereError::FeH(inputs.fe_h));
    }
    let metallicity = inputs.abundances.enrichment() * math::exp10(inputs.fe_h.value());
    if !positive(metallicity) {
        return Err(DeriveEnvelopeAtmosphereError::Metallicity(metallicity));
    }
    Ok((metallicity, equilibrium))
}

/// The network's speciation of `elements` at `temperature` and `pressure`, its refusal of a gas
/// that is not hydrogen-dominated given its own variant.
fn speciate(
    network: &Network,
    elements: &[(Element, f64)],
    temperature: Kelvin,
    pressure: Pascals,
) -> Result<GasEquilibrium, DeriveEnvelopeAtmosphereError> {
    network
        .solve(elements.iter().copied(), temperature, pressure)
        .map_err(|error| match error {
            SolveEquilibriumError::NotHydrogenDominated { share } => {
                DeriveEnvelopeAtmosphereError::NotHydrogenDominated { share }
            }
            SolveEquilibriumError::Temperature(_)
            | SolveEquilibriumError::AboveRecords(_)
            | SolveEquilibriumError::Pressure(_)
            | SolveEquilibriumError::Abundance { .. }
            | SolveEquilibriumError::Repeated(_)
            | SolveEquilibriumError::NoHydrogen
            | SolveEquilibriumError::NoRecord { .. }
            | SolveEquilibriumError::NotConverged { .. } => {
                DeriveEnvelopeAtmosphereError::Speciation { pressure, error }
            }
        })
}

/// `gas` scaled to sum to 1, summed in its order.
///
/// # Errors
///
/// [`DeriveEnvelopeAtmosphereError::Mixture`] where its total is not finite and positive.
fn normalise(gas: &mut [(SubstanceId, f64)]) -> Result<(), DeriveEnvelopeAtmosphereError> {
    let mut total = 0.0;
    for &(_, x) in gas.iter() {
        total += x;
    }
    if !(total.is_finite() && total > 0.0) {
        return Err(DeriveEnvelopeAtmosphereError::Mixture);
    }
    for (_, x) in gas.iter_mut() {
        *x /= total;
    }
    Ok(())
}

/// The mean molar mass of `gas`, g mol⁻¹.
fn molar_mass_g_per_mol(gas: &[(SubstanceId, f64)]) -> Result<f64, DeriveEnvelopeAtmosphereError> {
    mean_molar_mass_g_per_mol(gas).ok_or(DeriveEnvelopeAtmosphereError::Mixture)
}

/// The mixture whose `c_p` sets a profile pass's adiabat: `gas`'s rows with gas columns and a share
/// of at least [`COMPOSITION_FLOOR`], renormalised.
fn heat_mixture(
    gas: &GasEquilibrium,
) -> Result<Vec<(SubstanceId, f64)>, DeriveEnvelopeAtmosphereError> {
    let mut mixture: Vec<(SubstanceId, f64)> = gas
        .mole_fractions()
        .filter(|&(id, x)| id.substance().gas().is_some() && x >= COMPOSITION_FLOOR)
        .collect();
    normalise(&mut mixture)?;
    Ok(mixture)
}

/// The envelope's profile on the mixture `gases`.
fn profile_on(
    inputs: &EnvelopeAtmosphereInputs<'_>,
    gases: &[(SubstanceId, f64)],
) -> Result<EnvelopeProfile, DeriveEnvelopeAtmosphereError> {
    Ok(EnvelopeProfile::derive(&EnvelopeProfileInputs {
        internal_temperature: inputs.internal_temperature,
        irradiation_temperature: irradiation_temperature(inputs.equilibrium_temperature),
        gravity: inputs.gravity,
        gases,
        grey: inputs.grey,
    })?)
}

/// The envelope's profile in two passes (the [module](self)'s documentation), and the mean molar
/// mass of its second pass's mixture, g mol⁻¹, the quench's.
fn profile_in_two_passes(
    inputs: &EnvelopeAtmosphereInputs<'_>,
    elements: &[(Element, f64)],
) -> Result<(EnvelopeProfile, f64), DeriveEnvelopeAtmosphereError> {
    let network = inputs.network;
    let cold = speciate(network, elements, network.floor(), ADIABAT_BOTTOM)?;
    let first = profile_on(inputs, &heat_mixture(&cold)?)?;
    let deep = first.temperature_at(ADIABAT_BOTTOM);
    let ceiling = network.ceiling();
    let deep = if deep.value() > ceiling.value() {
        ceiling
    } else {
        deep
    };
    let warm = heat_mixture(&speciate(network, elements, deep, ADIABAT_BOTTOM)?)?;
    let profile = profile_on(inputs, &warm)?;
    Ok((profile, molar_mass_g_per_mol(&warm)?))
}

/// The decks of the 1-bar gas `deep` along `profile` in `column`, in two passes: the first at
/// each fixed kind's `f_sed` and Ackerman and Marley's baseline for the volatiles, in `column` (the
/// untrimmed gas's molar mass); the second, with each volatile's `f_sed` by its supply at its
/// first-pass base, in `column` of the molar mass of `deep` trimmed by the first pass's decks
/// ([`composition_of`]).
fn condense(
    deep: &[(SubstanceId, f64)],
    profile: impl Fn(Pascals) -> Kelvin + Copy,
    column: &MixingColumn,
) -> Result<Condensation, DeriveEnvelopeAtmosphereError> {
    let refractory = Sedimentation::new(REFRACTORY_SEDIMENTATION, DECK_WIDTH)?;
    let salt = Sedimentation::new(SULPHIDE_AND_SALT_SEDIMENTATION, DECK_WIDTH)?;
    let fixed = |id: SubstanceId| match DeckKind::of(id) {
        Some(DeckKind::Refractory) => Some(refractory),
        Some(DeckKind::SulphideOrSalt) => Some(salt),
        Some(DeckKind::Volatile) | None => None,
    };
    let first = Condensation::derive(
        deep.iter().copied(),
        profile,
        column,
        |id| fixed(id).unwrap_or(Sedimentation::BASELINE),
        1.0,
    )?;
    let column = MixingColumn {
        molar_mass_g_per_mol: molar_mass_g_per_mol(&composition_of(deep, &first)?)?,
        ..*column
    };
    let mut supply: Vec<(SubstanceId, Sedimentation)> = Vec::with_capacity(first.decks().len());
    for deck in first.decks() {
        if DeckKind::of(deck.condensate()) == Some(DeckKind::Volatile) {
            supply.push((deck.condensate(), sedimentation_for_supply(deck, &column)?));
        }
    }
    let of = |id: SubstanceId| {
        fixed(id)
            .or_else(|| supply.iter().find(|(deck, _)| *deck == id).map(|&(_, s)| s))
            .unwrap_or(Sedimentation::BASELINE)
    };
    Ok(Condensation::derive(
        deep.iter().copied(),
        profile,
        &column,
        of,
        1.0,
    )?)
}

/// The 1-bar gas `deep` trimmed by the decks of `clouds` ([`trimmed`]), its shares under
/// [`COMPOSITION_FLOOR`] dropped and the rest renormalised, in registry order.
fn composition_of(
    deep: &[(SubstanceId, f64)],
    clouds: &Condensation,
) -> Result<Vec<(SubstanceId, f64)>, DeriveEnvelopeAtmosphereError> {
    let drawn = drawn_vapours(clouds);
    let mut composition: Vec<(SubstanceId, f64)> = trimmed(deep, clouds, &drawn, COMPOSITION_LEVEL)
        .into_iter()
        .filter(|&(_, x)| x >= COMPOSITION_FLOOR)
        .collect();
    normalise(&mut composition)?;
    Ok(composition)
}

/// Every vapour the decks of `clouds` draw on, the limiting one and the others of each deck's law,
/// each once, in the decks' order.
#[must_use]
fn drawn_vapours(clouds: &Condensation) -> Vec<Vapour> {
    let mut drawn = Vec::new();
    for deck in clouds.decks() {
        if let Some(law) = law_aloft(deck.condensate()) {
            for reactant in core::iter::once(law.limiting()).chain(law.others().iter().copied()) {
                let vapour = reactant.vapour();
                if !drawn.contains(&vapour) {
                    drawn.push(vapour);
                }
            }
        }
    }
    drawn
}

/// The vapour of `drawn` that trims the species `id`: the species itself where a deck draws it,
/// otherwise the first element it carries that a deck draws; `None` where the decks draw neither.
#[must_use]
fn vapour_of(id: SubstanceId, drawn: &[Vapour]) -> Option<Vapour> {
    let row = id.substance();
    let own = Vapour::Species(row.key());
    if drawn.contains(&own) {
        return Some(own);
    }
    row.elements()
        .iter()
        .map(|&(element, _)| Vapour::Element(element))
        .find(|vapour| drawn.contains(vapour))
}

/// The share of `vapour` the decks of `clouds` leave at `pressure`: its remaining amount there over
/// the deep gas's (the remaining amount at the column's bottom, where no deck has drawn), 1 where
/// the deep gas holds none.
#[must_use]
fn left(clouds: &Condensation, vapour: Vapour, pressure: Pascals) -> f64 {
    match (
        clouds.remaining(vapour, pressure),
        clouds.remaining(vapour, DECK_COLUMN_BOTTOM),
    ) {
        (Some(left), Some(deep)) if deep > 0.0 => left / deep,
        _ => 1.0,
    }
}

/// `gas`'s rows with gas columns, each scaled by the share of its vapour ([`vapour_of`]) the decks of
/// `clouds` leave at `pressure`, and a carrier of a [`CO_TRAPPED`] metal by the smaller of that and
/// its host element's share.
#[must_use]
fn trimmed(
    gas: &[(SubstanceId, f64)],
    clouds: &Condensation,
    drawn: &[Vapour],
    pressure: Pascals,
) -> Vec<(SubstanceId, f64)> {
    gas.iter()
        .filter(|(id, _)| id.substance().gas().is_some())
        .map(|&(id, x)| {
            let own = vapour_of(id, drawn).map_or(1.0, |vapour| left(clouds, vapour, pressure));
            let host = id.substance().elements().iter().find_map(|&(element, _)| {
                CO_TRAPPED
                    .iter()
                    .find(|&&(metal, _)| metal == element)
                    .map(|&(_, host)| left(clouds, Vapour::Element(host), pressure))
            });
            let share = match host {
                Some(host) if host < own => host,
                _ => own,
            };
            (id, x * share)
        })
        .collect()
}

/// The atoms of `element` per molecule of `gas`, summed in its order.
#[must_use]
fn atoms_per_molecule(gas: &[(SubstanceId, f64)], element: Element) -> f64 {
    let mut total = 0.0;
    for &(id, x) in gas {
        for &(e, n) in id.substance().elements() {
            if e == element {
                total += f64::from(n) * x;
            }
        }
    }
    total
}

/// The share of `gas` of the row `key`, 0 where it holds none.
#[must_use]
fn share_of(gas: &[(SubstanceId, f64)], key: &str) -> f64 {
    let mut total = 0.0;
    for &(id, x) in gas {
        if id.substance().key().as_str() == key {
            total += x;
        }
    }
    total
}

/// The vertical column of a gas of mole fraction `x` between `base` and `top`, molecules m⁻², in a
/// gas of mean molar mass `molar_mass_g_per_mol` g mol⁻¹ under `gravity`: x (`p_base` − `p_top`)
/// `N_A` ÷ (μ g).
#[must_use]
fn column_per_m2(
    x: f64,
    base: Pascals,
    top: Pascals,
    molar_mass_g_per_mol: f64,
    gravity: MetresPerSecondSquared,
) -> f64 {
    x * (base.value() - top.value()) * AVOGADRO
        / (molar_mass_g_per_mol / GRAMS_PER_KILOGRAM * gravity.value())
}

/// The share of [`soot`]'s column an envelope of equilibrium temperature `equilibrium_temperature`
/// keeps: 0 at and below [`SOOT_GRID_EDGE`]'s 600 K, 1 at and above its 700 K, linear between.
#[must_use]
fn soot_weight(equilibrium_temperature: Kelvin) -> f64 {
    let (none, whole) = (SOOT_GRID_EDGE.0.value(), SOOT_GRID_EDGE.1.value());
    let s = (equilibrium_temperature.value() - none) / (whole - none);
    if s >= 1.0 {
        1.0
    } else if s > 0.0 {
        s
    } else {
        0.0
    }
}

/// The photochemistry of a starlit envelope of upper atmosphere `air` and profile `profile`, whose
/// trimmed 10-mbar gas is `upper` and whose decks are `decks` (the [module](self)'s documentation):
/// its hazes' modes, in their rules' order, and its SO₂ layer's absorber.
fn photochemistry(
    air: &UpperAtmosphere,
    profile: &EnvelopeProfile,
    upper: &[(SubstanceId, f64)],
    decks: &[AerosolDeck],
) -> Result<(Vec<AerosolMode>, Option<Absorber>), DeriveEnvelopeAtmosphereError> {
    let sulphur = atoms_per_molecule(upper, Element::S);
    let deck = |key: &str| {
        decks
            .iter()
            .find(|d| d.condensate().substance().key().as_str() == key)
    };
    // Each haze with the share of its rule's amount drawn: soot's taper below Gao et al.'s grid.
    let mut hazes: Vec<(Haze, f64)> = Vec::new();
    hazes.extend(sulphur_haze(air, sulphur)?.map(|haze| (haze, 1.0)));
    let weight = soot_weight(air.equilibrium_temperature);
    if weight > 0.0 {
        hazes.extend(soot(air, share_of(upper, "CH4"))?.map(|haze| (haze, weight)));
    }
    let methane = deck("CH4");
    if let Some(methane) = methane {
        hazes.extend(ice_giant_haze(methane.base())?.map(|haze| (haze, 1.0)));
    }
    // The chromophore on a Jupiter-like ammonia cloud alone, no methane deck above it: Braude et al.
    // 2020 retrieve it on Jupiter.
    if let Some(ammonia) = deck("NH3")
        && methane.is_none_or(|m| m.base().value() >= ammonia.base().value())
    {
        hazes.push((chromophore(ammonia.base())?, 1.0));
    }
    let mut modes = Vec::with_capacity(hazes.len());
    for (haze, weight) in &hazes {
        modes.push(haze_mode(haze, *weight, profile)?);
    }
    let layer = sulphur_dioxide(air, sulphur, share_of(upper, "H2O"))?;
    // SO₂ is a registry gas row (P14.T49.b), which the network carries; a registry without it would
    // make no absorber of the layer rather than fail.
    let absorber = match (layer, by_key("SO2")) {
        (Some(layer), Some(id)) => Some(Absorber::slab(
            id,
            column_per_m2(
                layer.mole_fraction(),
                layer.base(),
                layer.top(),
                air.molar_mass_g_per_mol,
                air.gravity,
            ),
            layer.base(),
            layer.top(),
        )?),
        _ => None,
    };
    Ok((modes, absorber))
}

/// The inventory's mode of the photochemical haze `haze`, `weight` of its rule's amount drawn, along
/// `profile` (the [module](self)'s documentation): its own material as its deck, the whole sky
/// covered, a layer's profile well mixed, a Gaussian's a slab between its 1/e points, its optics at
/// the profile's temperature at its base.
fn haze_mode(
    haze: &Haze,
    weight: f64,
    profile: &EnvelopeProfile,
) -> Result<AerosolMode, BuildModeError> {
    let (material, phase) = (haze.material(), haze.phase());
    let shape = ShapeClass::of(material, phase);
    let layer = match haze.profile() {
        HazeProfile::Layer {
            base,
            top,
            exponent,
        } => {
            // A mode's profile takes no negative exponent: the ice giants' haze, its mixing ratio
            // rising with height, is taken well mixed.
            let exponent = if exponent > 0.0 { exponent } else { 0.0 };
            ModeProfile::new(base, top, exponent)?
        }
        HazeProfile::Gaussian { peak, width } => ModeProfile::new(
            Pascals::new(peak.value() * math::exp(width)),
            Pascals::new(peak.value() * math::exp(-width)),
            0.0,
        )?,
    };
    let temperature = profile.temperature_at(layer.base());
    match haze.amount() {
        HazeAmount::ColumnMass(mass) => {
            let mode = Mode::new(
                material,
                phase,
                material,
                haze.size(),
                KilogramsPerSquareMetre::new(mass.value() * weight),
                layer,
                1.0,
                temperature,
            )?;
            Ok(AerosolMode::of_mode(&mode, shape, temperature))
        }
        HazeAmount::OpticalDepth { optical_depth, .. } => AerosolMode::of_rule(
            material,
            phase,
            shape,
            material,
            haze.size(),
            optical_depth * weight,
            layer,
            1.0,
            temperature,
        ),
    }
}

/// The absorbers of the trimmed 10-mbar gas `upper`, of mean molar mass `molar_mass_g_per_mol` g
/// mol⁻¹ under `gravity`, each a slab from [`ABSORBER_LEVEL`] to [`DECK_COLUMN_TOP`], with
/// photochemical SO₂'s `sulphur_dioxide`, in registry order.
fn absorbers(
    upper: &[(SubstanceId, f64)],
    molar_mass_g_per_mol: f64,
    gravity: MetresPerSecondSquared,
    sulphur_dioxide: Option<Absorber>,
) -> Result<Vec<Absorber>, BuildAbsorberError> {
    let mut absorbers = Vec::new();
    for &(id, x) in upper {
        if x >= COMPOSITION_FLOOR && ABSORBERS.contains(&id.substance().key().as_str()) {
            absorbers.push(Absorber::slab(
                id,
                column_per_m2(
                    x,
                    ABSORBER_LEVEL,
                    DECK_COLUMN_TOP,
                    molar_mass_g_per_mol,
                    gravity,
                ),
                ABSORBER_LEVEL,
                DECK_COLUMN_TOP,
            )?);
        }
    }
    absorbers.extend(sulphur_dioxide);
    absorbers.sort_by_key(Absorber::species);
    Ok(absorbers)
}
