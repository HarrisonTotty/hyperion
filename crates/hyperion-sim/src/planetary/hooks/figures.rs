//! A body's global figures (plan 14, P14.T24.b, with P14.T48's inputs to the coarse surface):
//! what the surface generator reads of a body's whole surface.
//!
//! Built so far:
//!
//! - [`GlobalFigures`] (P14.T24.b, its interior and its craters): the body's heat flow, its
//!   tectonic regime, its magnetic field, its present volcanic outgassing and its subsurface
//!   ocean, from its [`interior`] (science-p14-t24b-laws Part 1); and its surface age and crater
//!   contract (P14.T48.c), from its [`surface_age`] and its impactors (Part 2); and its volatile
//!   history (P14.T48.b), its wet epoch and whether it came from a colder state, from its
//!   [`wet_epoch`] law (Part 3); and its volcanism level, continents and relief (P14.T48.a), from
//!   its [`relief`], and its surface water, oceans, ices and cloud, from its [`cover`] (Part 4).
//! - [`impactors`] (P14.T48.f): what strikes a body and what it strikes, and how often over the
//!   Moon ([`Impactors::rate`]), which P14.T48.c's crater contract carries beside the crater
//!   density ([`CraterParams`]).
//!
//! # The global figures (P14.T24.b)
//!
//! [`GlobalFigures::derive`] reads plain values ([`FiguresInputs`]): the body's mass, radius,
//! gravity, class and mass fractions (its bulk section's), its atmosphere and water, its host's
//! \[Fe/H\], its age, its rotation, the tidal heat its parent raises in it, and its crater inputs
//! ([`CraterInputs`]: where it goes round, its density, its host's sources and the dates of its
//! magma ocean's end and its last giant impact). A body in the gas-envelope state has none, as it
//! has no surface. The heat flow, the regime, the field and the outgassing are [`interior`]'s;
//! the subsurface ocean is the rule's that computes it, so far a regular moon's alone
//! ([`Subsurface`]); the surface age is [`surface_age`]'s, at that heat flow, and the crater
//! contract the lunar chronology at it times the impactors' rate.
//!
//! The first stage then gives the volcanism level V, by [`relief::volcanism_level`] over the
//! surface age at the heat flow then; the continental fraction, by the regime and P14.T48.b's
//! start condition ([`window_entry`]), which the caller reads into [`FiguresInputs::continents`];
//! `σ_h` and its hypsometry ([`relief::Relief`]); and the surface water and its potential ocean,
//! with the ocean weight θ the surface conditions read ([`cover`]).
//!
//! The second stage, [`GlobalFigures::with_surface`], reads the surface conditions, which read the
//! first stage's heat flow and θ: the published ocean (the liquid-water flag's, or a `Snowball`'s
//! under its ice), the volatile history ([`VolatileHistory::derive`] of the hosts' window over the
//! body's past and of its outgassing then, [`past_outgassing`]), the ice (the zonal profile's cold
//! area) and the cloud. Each step reads only what is above it, science-p14-t24b-laws §4.6's order,
//! with no loop.
//!
//! [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at) publishes them with
//! the surface conditions, under the same hold until the batch's bump, and passes their heat
//! flow to the conditions' night floor.
//!
//! # The impactors (P14.T48.f)
//!
//! A closed form with no draws, from the system the generator made (science-r09-craters.md items 2
//! and 4, the ruling plan 14's task text follows).
//!
//! - **The mean impact velocity.** For a planet, a dwarf planet or a belt's member,
//!   v² = v∞² + `v_esc`²; for a moon, v² = v∞,P² + 3 `v_K,s`² + `v_esc,s`², where v∞,P is its
//!   parent's encounter speed with the impactors, `v_K,s` its orbital speed about its parent and
//!   `v_esc,s` its own escape speed. Two of the three `v_K,s`² are the square of the parent's
//!   escape speed at the moon's orbit, 2 `v_K`² = `v_esc`² (Brasser, Wong and Werner 2025, A&A
//!   695, A276, arXiv 2501.17509, §2.2, after its eq. 2), and the moon's own motion makes the
//!   mean encounter speed about √3 `v_K` (its §2.3, after Zahnle et al. 2003, Icarus 163, 263,
//!   eq. (1)); science-r09-craters.md item 4 assembles the sum. The escape speed is √(2 g R), the
//!   bulk section's gravity and radius.
//! - **The encounter speed** is v∞ = U `v_K`(a), with `v_K` = √(G M ÷ a) the circular speed about
//!   the host at the body's (or its parent's) semi-major axis, and U² = `U_src`² + e² + sin² i,
//!   which adds the target's own eccentricity and inclination to its host's plane to the
//!   impactors' own excitation `U_src`² (Öpik, with random phases).
//! - **The impactors' excitation `U_src`²**, by the first rule that applies ([`ImpactorSource`]):
//!   1. _In a belt_ (a belt's member, or an orbit inside one of a belt's annuli, edges included):
//!      that component's own excitation, ⟨e²⟩ + ⟨sin² i⟩ = 2σₑ² + 2 sin² σᵢ from the Rayleigh
//!      widths of plan 14's belts ([`belt_excitation`]): 0.0587 for a belt proper and 0.414 for a
//!      scattered component.
//!   2. _A giant_ (at least [`IMPACTOR_GIANT_MASS`]), or any body at or beyond the innermost
//!      giant's orbit, and their moons: the giants' scattered comets, [`COMET_EXCITATION`]² =
//!      0.30².
//!   3. _Otherwise_, inside the innermost giant or with no giant: the nearest belt of
//!      [`BeltKind::Asteroid`], or failing one the nearest of any kind, by `U_b`² + (1 − a ÷ `a_b`)³
//!      inside it and `U_b`² + (1 − `a_b` ÷ a)³ outside, with `U_b`² and `a_b` its belt proper's
//!      excitation and mid-radius. The cube is the plan's own calibrated form: the minimal
//!      crossing orbit's excess speed grows linearly in the fractional distance, and the real, more
//!      excited crossers faster. With no belt but a giant, [`COMET_EXCITATION`]² + (1 − a ÷ `a_g`)³
//!      from the innermost giant's `a_g`; with neither, [`COMET_EXCITATION`]². The nearest belt is
//!      the one of the least ratio max(a, `a_b`) ÷ min(a, `a_b`), the first in the given order on a
//!      tie; the cube is continuous at `a_b` but not at a belt's edges, where rule 1 takes over.
//! - **The projectiles' density**: [`ROCKY_PROJECTILE_DENSITY`] from a rocky belt,
//!   [`ICY_PROJECTILE_DENSITY`] from an icy belt or the comets.
//! - **The target** ([`Crust`]): porous below a radius of [`POROUS_TARGET_RADIUS`], at the body's
//!   bulk density; otherwise non-porous, at ice Ih's density at its mean surface temperature for
//!   an ice-rich crust or [`SILICATE_TARGET_DENSITY`] for a silicate one. `k_target` is the
//!   crust's, [`ICE_K_TARGET`] or [`SILICATE_K_TARGET`], whatever the class. A crust is ice-rich
//!   ([`Crust::of`]) for a body colder than [`ICE_RICH_TEMPERATURE`] that is a moon formed beyond
//!   its parent's ice line, a member of an icy belt, or a body whose water would make a global
//!   layer at least [`ICE_RICH_LAYER_DEPTH`] deep ([`ice_layer_depth`]).
//!
//! Against the Solar System (this module's tests, on JPL's elements and masses) it gives Mercury
//! 41.4, Venus 25.7, Earth 19.9, the Moon 16.7, Mars 10.55 and Ceres 5.65 km/s, within 0.1 km/s
//! of the ruling's own figures; Mars ÷ the Moon 0.632 (Ivanov 2001's 0.60, Bottke's and Le Feuvre
//! and Wieczorek's 0.53–0.54); the 15 regular moons of Jupiter, Saturn and Uranus within −6.3% and
//! +7.4% of Brasser et al. 2025's Table 5; and Pluto 2.48 km/s against Singer et al. 2019's 2.2.
//!
//! Not modelled (science-r09-craters.md item 4): comets striking the inner planets, a minority of
//! their crater-forming impacts (Ivanov et al. 2002 conclude main-belt asteroids made most of
//! them), and the velocity's spread: the diameter map takes the mean.
//!
//! With them, for P14.T48.c's crater contract (science-p14-t24b-laws Part 2):
//!
//! - **The rate** `R_b` = Ψ × V ÷ V☾ ([`Impactors::rate`]; science-p14-t24b-laws §2.2, with its
//!   §2.7 re-rule against Zahnle et al. 2003): the projectiles' abundance over the Moon's, by the
//!   same source rule from the belts' masses at [`SOURCE_MASS_AGE`], times the encounter factor
//!   over the Moon's [`LUNAR_ENCOUNTER_SPEED`]. It gives the Moon 1, Earth 1.42, Mars 1.96 (Ivanov
//!   2001's 1.92), Venus 1.17, Mercury 1.25, Ceres 134, Ganymede 3.71 and Pluto 30.1.
//! - **A moon's birthplace** ([`MoonBirth::formed`]; §2.4): a regular moon's by its ice, a
//!   giant-impact moon's its parent's, a giant's capture beyond the ice line, and a rocky planet's
//!   capture its neighbouring belt's side.
//!
//! # On the record
//!
//! P14.T48.c's crater contract reads [`impactors`] inside [`GlobalFigures`], which a body's
//! record's surface section carries from the 21 → 22 batch's bump (P14.T48.e). It draws nothing.

use core::fmt;
use std::error::Error;

use hyperion_surface::craters::{
    self, BuildCraterParamsError, CraterParams, CraterParamsParts, CraterTarget, Screening,
    TargetClass,
};

use crate::math;
use crate::planetary::belts::{
    BELT_GIANT_MASS, Belt, BeltComposition, BeltPart, MEMBER_ECCENTRICITY_WIDTH,
    MEMBER_INCLINATION_WIDTH_DEG, SCATTERED_ECCENTRICITY_WIDTH, SCATTERED_INCLINATION_WIDTH_DEG,
};
use crate::planetary::chem::partition::{
    Basins, ClimateSpan, CondensatePhase, Condensates, Cover, Ground, Partition, WaterCover,
    partition,
};
use crate::planetary::derive::atmosphere::{Atmosphere, Gas, SurfaceState};
use crate::planetary::derive::{MassFractions, PlanetClass, SnowLineSide};
use crate::planetary::hooks::cold_traps::{self, ColdTrapBody, ColdTrapRules, RecordColdTraps};
use crate::planetary::hooks::surface::{ColdProfile, OceanWeight, SurfaceConditions};
use crate::planetary::moons::CaptureKind;
use crate::planetary::moons::regular::{ICE_MELTING_POINT, ocean_layers};
use crate::planetary::record::BeltKind;
use crate::substance::{self, SubstanceId};
use crate::units::consts::METRES_PER_AU;
use crate::units::{
    Dex, EarthMasses, Gigayears, GravitationalParameter, Kelvin, Kilograms, KilogramsPerCubicMetre,
    KilogramsPerSquareMetre, Metres, MetresPerSecond, MetresPerSecondSquared, PerSquareKilometre,
    Radians, Seconds, WattsPerSquareMetre, Years,
};

pub mod cover;
pub mod interior;
pub mod relief;
pub mod surface_age;
pub mod wet_epoch;

pub use interior::{
    Interior, MagneticField, RadiogenicIsotope, TectonicRegime, conductive_lag,
    convecting_heat_flow, magnetic_field, outgassing, radiogenic_heat_flow, tectonic_regime,
};
pub use relief::{Continents, Hypsometry, Relief};
pub use wet_epoch::{
    VolatileHistory, VolatileHistoryInputs, WetEpoch, WindowAt, WindowEntry, WindowEntryInputs,
    window_entry,
};

#[cfg(test)]
pub(crate) mod tests;

/// The mass from which a planet is one of the impactors' giants, whose scattered comets strike it
/// and every body at or beyond the innermost one: [`BELT_GIANT_MASS`], 10 M⊕, the giants the
/// belts are placed by (science-r09-craters.md item 4: "a giant (≥ 10 M⊕)").
pub const IMPACTOR_GIANT_MASS: EarthMasses = BELT_GIANT_MASS;

/// The giants' scattered comets' encounter speed in units of the target's circular speed:
/// `U_c` = 0.30 (science-r09-craters.md item 4).
///
/// Brasser, Wong and Werner 2025 (A&A 695, A276, §2.3) find the comets' mean v∞ at Jupiter,
/// Saturn and Uranus 3.17, 3.65 and 2.39 km/s, after Vokrouhlický et al. 2008 and Nesvorný et al.
/// 2017: 0.24, 0.38 and 0.35 of the planets' orbital speeds (their §2.2 quotes 0.27–0.38 from
/// their migration runs). Nesvorný et al. 2023 (PSJ 4, 139, §3.2.1) find 4.1 km/s for the bodies
/// that strike Jupiter, "approximately 30%" of its orbital speed.
pub const COMET_EXCITATION: f64 = 0.30;

/// The density of a rocky belt's projectiles, kg m⁻³: 3,000, Design note 12's stony projectile
/// and the diameter map's reference ([`craters::LUNAR_PROJECTILE_DENSITY`]; science-r09-craters.md
/// item 2), which the ruling keeps over the 2,700 kg m⁻³ plan 14 gives a rocky belt's members
/// ([`ROCKY_MEMBER_DENSITY`](crate::planetary::belts::ROCKY_MEMBER_DENSITY), the cascade's).
pub const ROCKY_PROJECTILE_DENSITY: KilogramsPerCubicMetre = craters::LUNAR_PROJECTILE_DENSITY;

/// The density of an icy belt's projectiles and of the comets, kg m⁻³: 500, Britt, Consolmagno and
/// Merline 2006's nominal cometary bulk density (LPSC 37, 2214).
///
/// It lies inside Groussin et al. 2019's mean of 20 estimates, 480 ± 220 kg m⁻³ (Space Sci. Rev.
/// 215, 29, Table 1), between 9P/Tempel 1's 400 (200–1,000, Richardson et al. 2007, Icarus 190,
/// 357) or 470 (Thomas et al. 2013) and 67P's 532 ± 7 (Jorda et al. 2016, Icarus 277, 257), the
/// one precise measurement, and under the Jupiter-family comets' spin barrier of about 600 (Lowry
/// and Weissman 2003, via Groussin et al.). Brasser, Wong and Werner 2025 take 400 after Wong et
/// al. 2023, and Zahnle et al. 2003 took 600; 400 would scale a crater by (400 ÷ 500)^0.261 =
/// 0.943 (science-r09-craters.md F5.2).
pub const ICY_PROJECTILE_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(500.0);

/// The radius below which a body is a porous crater target, m: 150 km, the plan's own threshold
/// between Hyperion (135 km, 544 kg m⁻³, over 40% porous; Thomas et al. 2007, Nature 448, 50),
/// which is porous, and Mimas (198 km), which is not; the small Saturnian moons lie below it, at
/// 400–650 kg m⁻³ as science-r09-craters.md item 2 recalls them, unverified.
pub const POROUS_TARGET_RADIUS: Metres = Metres::new(150e3);

/// The density of a non-porous silicate crust as a crater target, kg m⁻³: 2,550, the lunar
/// highlands crust's bulk density from GRAIL (Wieczorek et al. 2013, Science 339, 671), the
/// diameter map's reference target ([`craters::LUNAR_TARGET`]; science-r09-craters.md item 2).
pub const SILICATE_TARGET_DENSITY: KilogramsPerCubicMetre = craters::LUNAR_TARGET.density;

/// The target factor `k_target` of a silicate crust, which scales the simple-to-complex
/// transition: 1 (R09's Design note 12).
pub const SILICATE_K_TARGET: f64 = 1.0;

/// The target factor `k_target` of an ice-rich crust: 0.12.
///
/// The morphological transitions of nine icy bodies give a geometric mean of 0.125 and a range of
/// 0.06–0.26 (Ganymede's 2.1 ± 0.2 km, Schenk et al. 2004; Rhea's 12 ± 2 km, Aponte-Hernández et
/// al. 2021), with no resolvable temperature term between 40 and 150 K; Ceres's mixed crust
/// behaves as ice (science-r09-craters.md item 1).
pub const ICE_K_TARGET: f64 = 0.12;

/// The mean surface temperature below which a body's crust can be ice-rich: 273 K, water's
/// melting point, whatever the body's birthplace; at or above it its ice melts or, airless,
/// sublimates within years (science-r09-craters.md item 2 and F5.4).
pub const ICE_RICH_TEMPERATURE: Kelvin = Kelvin::new(273.0);

/// The depth of global water layer from which a cold body's crust is ice-rich, m: 10 km, the
/// plan's own figure, deeper than any crater below the transition excavates; Europa's 80–170 km
/// and Ceres's 41 km crust lie far above it, as science-r09-craters.md item 2 states them.
pub const ICE_RICH_LAYER_DEPTH: Metres = Metres::new(10e3);

/// The density a body's water is spread at to make its global layer, kg m⁻³: 1,000, liquid
/// water's, as a global equivalent layer is stated (this module's choice: ice's 917–934 would
/// deepen the layer by 7–9%).
pub const WATER_LAYER_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_000.0);

/// The age at which a source belt's mass is taken, yr: 4.567 Gyr, the Solar System's, the lunar
/// chronology's clock ([`craters::SOLAR_SYSTEM_AGE`]; science-p14-t24b-laws §2.2).
///
/// The chronology already carries the bombardment's time profile, counted from each system's
/// formation, so the masses compare like with like and the rate is constant in time: a crater
/// density cannot fall as a belt grinds down.
pub const SOURCE_MASS_AGE: Years = Years::new(4.567e9);

/// The Solar main belt's mass, M⊕: 4.0 × 10⁻⁴, Pitjeva and Pitjev's (4.008 ± 0.029) × 10⁻⁴ (3σ;
/// Astronomy Letters 44, 554, 2018), the unit of an asteroid belt's projectile abundance
/// (science-p14-t24b-laws §2.2, after P14.T19's normalisation of the captures). It is the whole
/// belt's, Ceres, Vesta, Pallas and Hygiea included, where a generated belt's abundance reads its
/// cascade alone ([`SourceBelt::from`]), which is recorded.
pub const SOLAR_MAIN_BELT_MASS: EarthMasses = EarthMasses::new(4.0e-4);

/// The Solar main belt's inner and outer edges, au: 2.065 and 3.278, Jupiter's 4:1 and 2:1
/// resonances at its 5.2029 au (P14.T21.a's rule), the reference torus of the abundance
/// (science-p14-t24b-laws §2.2).
pub const SOLAR_MAIN_BELT_EDGES_AU: (f64, f64) = (2.065, 3.278);

/// The Solar Kuiper belt's mass, M⊕: 0.02, Pitjeva and Pitjev's (1.97 ± 0.30) × 10⁻² rounded (3σ;
/// Celestial Mechanics and Dynamical Astronomy 130, 57, 2018), the largest bodies included: the
/// unit of the comets' abundance (science-p14-t24b-laws §2.2).
pub const KUIPER_BELT_MASS: EarthMasses = EarthMasses::new(0.02);

/// `K_in` of a rocky belt, the projectile abundance inside a belt with the Solar main belt's
/// density, over the Moon's: 400, a low-confidence estimate (science-p14-t24b-laws §2.2; an icy
/// belt's is [`ICY_IN_BELT_DENSITY`], §2.7).
///
/// The ruling builds it from the main belt's 1.36 × 10⁶ bodies above 1 km (Bottke et al. 2005,
/// Icarus 175, 111), their intrinsic collision probability 2.86 × 10⁻¹⁸ km⁻² yr⁻¹ (Bottke et al.
/// 1994, Icarus 107, 255) and Earth's interval of about 0.6 Myr between impacts of 1 km bodies
/// (Stuart and Binzel 2004, Icarus 170, 295): Ceres is struck at 3.1 × 10⁻¹³ km⁻² yr⁻¹ against the
/// Moon's 2.3 × 10⁻¹⁵, an `R_b` of about 134 at its encounter factor of 0.332, so `K_in` ≈ 400.
pub const IN_BELT_DENSITY: f64 = 400.0;

/// `K_in` of an icy belt: 6,700, fitted on Pluto (science-p14-t24b-laws §2.7 (b)).
///
/// Zahnle et al. 2003 (Icarus 163, 263, §5.5, Case A) give Pluto Ṅ(d > 1.5 km) = 10⁻⁶ yr⁻¹, Table
/// 1's 2.0 × 10⁻⁴ of Jupiter's rate: per unit area 8.17 times Ganymede's, an `R_b` of 30.3, and so
/// `K_in` = 30.3 ÷ 0.1679 (its V ÷ V☾) ÷ 0.02678 (its torus density over the Solar main belt's) =
/// 6,748, rounded (Pluto 30.1). Case A, not Case B's 2.6 times more, as Zahnle reads the survival
/// of Uranus's and Neptune's inner moons (§5.4). Per unit mass an icy belt holds more impactors of
/// one size: bulk density alone gives 3,000 ÷ 500 = 6, and the rest of the 16.75 lumps the Kuiper
/// belt's size distribution with the torus proxy's misfit to its structure (low confidence).
pub const ICY_IN_BELT_DENSITY: f64 = 6_700.0;

/// `K_c`, the giants' comets' abundance at 5.2 au about the Sun with the Solar Kuiper belt's mass,
/// over the Moon's projectiles': 0.80 (science-p14-t24b-laws §2.7 (a2)).
///
/// It keeps Ganymede's `R_b` at the earlier calibration's 3.7 (exactly 0.794; 0.80 gives 3.71).
/// That carries the ecliptic comets' impact rate on Jupiter, 0.005 (+0.006 −0.003) yr⁻¹ above
/// 1.5 km (Zahnle, Schenk, Levison and Dones 2003, Icarus 163, 263, abstract), to Ganymede, struck
/// at 1.2 × 10⁻⁴ of it (their Table 1).
pub const COMET_DENSITY: f64 = 0.80;

/// The distance at which the comets' abundance about the Sun is [`COMET_DENSITY`] × `μ_ic`, au:
/// 5.2, Jupiter's.
pub const COMET_REFERENCE_DISTANCE_AU: f64 = 5.2;

/// The exponent k of the comets' abundance, Ψ ∝ a^k: 1.15, the log-least-squares fit (1.149) of
/// Saturn's, Uranus's and Neptune's whole-planet impact rates over Jupiter's against Zahnle et al.
/// 2003's Table 1, 0.42, 0.25 and 0.27 (Neptune's Levison et al. 2000 value halved, as Zahnle
/// does), with T48.f's own encounter factors (science-p14-t24b-laws §2.7 (a2)). Empirical: it
/// absorbs the comets' density and the change of v∞ between the planets that a fixed `U_c` cannot
/// carry. The residuals, model over Table 1, are 1.60, 0.58 and 1.29, inside Zahnle's stated
/// factors of 4, 6 and 10.
pub const COMET_DISTANCE_EXPONENT: f64 = 1.15;

/// The Solar Kuiper belt proper's inner edge, au: 39.4027, Neptune's 3:2 at its 30.0699 au
/// (P14.T21.b's rule), which scales the comets' abundance to a host's own Kuiper-like belt
/// (science-p14-t24b-laws §2.7 (a2)).
pub const SOLAR_KUIPER_BELT_INNER_EDGE_AU: f64 = 39.402_7;

/// The exponent of the asteroids' abundance inside a belt's inner edge, Ψ ∝ (1 − a ÷ `a_i`)^−1.5:
/// 1.5, fitted on Mars's `R_b` = 1.92, Ivanov 2001's Mars-to-Moon impact rate through the diameter
/// map (science-r09-craters.md F1 and F4.1; science-p14-t24b-laws §2.2, its replica's 1.956).
pub const APPROACH_EXPONENT: f64 = 1.5;

/// The least projectile abundance over the Moon's: 0.01, the leftover planetesimals every system
/// has, so that no surface is uncratered for want of a belt (the plan's own, provisional;
/// science-p14-t24b-laws §2.2).
pub const LEFTOVER_DENSITY: f64 = 0.01;

/// The Moon's encounter factor V☾, m/s: 16,922, the moon formula of [`Impactors::rate`] for the
/// Moon about an Earth at 1 au with the Sun's belts, so that the Moon's `R_b` is 1
/// (science-p14-t24b-laws §2.2, with §2.7's parent focusing; the re-rule's replica gives
/// 16,921.79). `Fitted`: a test refits it.
///
/// The configuration: the Sun's nominal GM; an Earth of 1 M⊕ at 1 au, eccentricity 0.016 710 22
/// (NASA's Earth fact sheet) and no inclination, inside the Solar main belt
/// ([`SOLAR_MAIN_BELT_EDGES_AU`], [`SOLAR_MAIN_BELT_MASS`]); the Moon at 384,400 km, of GM
/// 4,902.800 km³ s⁻² and mean radius 1,737.4 km (JPL's mean elements and physical parameters,
/// DE440).
pub const LUNAR_ENCOUNTER_SPEED: MetresPerSecond = MetresPerSecond::new(16_921.789_060_825_588);

/// A body's orbit about its host as its impactors read it.
///
/// Plain data with public fields: [`impactors`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostOrbit {
    /// The semi-major axis about the host, m; finite and positive.
    pub semi_major_axis: Metres,
    /// The eccentricity, in `[0, 1)`.
    pub eccentricity: f64,
    /// The inclination to the host's plane, the mean plane of its belts and planets, rad; finite.
    ///
    /// Not the system frame's: a host's plane is tilted against the galactic axes.
    pub inclination: Radians,
}

/// Which belt a body is a named member of, as its impactors read it (P14.T21.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BeltMembership {
    /// The part of its belt it orbits in, whose excitation it shares.
    pub part: BeltPart,
    /// Its belt's composition class, which sets its projectiles' density and, if icy, makes its
    /// crust ice-rich.
    pub composition: BeltComposition,
}

/// A body that goes round the host itself: a planet, a dwarf planet or a belt's member.
///
/// Plain data with public fields: [`impactors`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostOrbiter {
    /// Its orbit about the host.
    pub orbit: HostOrbit,
    /// Its mass, finite and non-negative (positive for a moon's parent), which says whether it is
    /// a giant ([`IMPACTOR_GIANT_MASS`]) and, for a moon's parent, sets the moon's orbital speed.
    pub mass: EarthMasses,
    /// The belt it is a named member of, if any; `None` for a planet or a body that is no
    /// member.
    pub belt: Option<BeltMembership>,
}

/// Where a body goes round, as its impactors read it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImpactorOrbit {
    /// The body goes round the host itself.
    Host(HostOrbiter),
    /// The body is a moon of `parent`.
    Moon {
        /// Its parent, which goes round the host: the impactors are the parent's.
        parent: HostOrbiter,
        /// Its semi-major axis about its parent, m; finite and positive.
        semi_major_axis: Metres,
        /// Which side of its parent's ice line it formed on: beyond it for a moon with ice
        /// ([`DerivedMoon::ice_fraction`](crate::planetary::moons::DerivedMoon::ice_fraction)
        /// above zero), whose crust is ice-rich.
        formed: SnowLineSide,
    },
}

/// One annulus of a source belt, its edges from the host.
///
/// Plain data with public fields: [`ImpactorSources::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceAnnulus {
    /// The inner edge, m; finite and positive.
    pub inner_edge: Metres,
    /// The outer edge, m; finite and no less than the inner edge.
    pub outer_edge: Metres,
}

impl SourceAnnulus {
    /// Its mid-radius, the midpoint of its edges.
    #[must_use]
    pub fn mid_radius(&self) -> Metres {
        Metres::new(f64::midpoint(
            self.inner_edge.value(),
            self.outer_edge.value(),
        ))
    }

    /// Whether a semi-major axis `a` lies inside it, edges included.
    #[must_use]
    fn holds(&self, a: Metres) -> bool {
        self.inner_edge <= a && a <= self.outer_edge
    }
}

/// A belt as the impactors read it: its kind, its composition, its annuli and their masses.
///
/// Plain data with public fields: [`ImpactorSources::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceBelt {
    /// Its kind: an asteroid belt is the inner bodies' source before any other (rule 3).
    pub kind: BeltKind,
    /// Its composition class, which sets its projectiles' density.
    pub composition: BeltComposition,
    /// The belt proper.
    pub main: SourceAnnulus,
    /// Its scattered component, if it has one.
    pub scattered: Option<SourceAnnulus>,
    /// The belt proper's mass at [`SOURCE_MASS_AGE`], its collisional cascade's without its named
    /// members, finite and non-negative: what sets its projectiles' abundance (P14.T48.c's rate).
    pub main_mass: EarthMasses,
    /// The scattered component's mass at [`SOURCE_MASS_AGE`], finite and non-negative; zero for a
    /// belt with none.
    pub scattered_mass: EarthMasses,
}

impl From<&Belt> for SourceBelt {
    /// The belt `belt` as its impactors read it: its edges, and each component's cascade worn down
    /// to [`SOURCE_MASS_AGE`] about its host by P14.T21's collisional decay
    /// ([`BeltComponent::mass_at`](crate::planetary::belts::BeltComponent::mass_at)).
    ///
    /// The named members are left out: they are single large bodies, not projectiles of one size,
    /// and with them the sample's greatest abundance in a belt rises from 2.2 × 10³ to 1.5 × 10⁸.
    /// The Solar belts' masses the abundance is normalised by include their largest bodies, which
    /// is recorded (plan 14's Risks).
    fn from(belt: &Belt) -> Self {
        let annulus = |inner_edge, outer_edge| SourceAnnulus {
            inner_edge,
            outer_edge,
        };
        let mass_of = |part: &crate::planetary::belts::BeltComponent| {
            part.mass_at(SOURCE_MASS_AGE, belt.host_mass())
        };
        Self {
            kind: belt.kind(),
            composition: belt.composition(),
            main: annulus(belt.main().inner_edge(), belt.main().outer_edge()),
            scattered: belt
                .scattered()
                .map(|part| annulus(part.inner_edge(), part.outer_edge())),
            main_mass: mass_of(belt.main()),
            scattered_mass: belt.scattered().map_or(EarthMasses::ZERO, mass_of),
        }
    }
}

/// The impactors' sources about one orbit host: its mass, its belts and its innermost giant.
///
/// P14.T24.b builds one per host, which [`ImpactorSources::new`] validates once, and lends it to
/// each of the host's bodies.
#[derive(Debug, Clone, PartialEq)]
pub struct ImpactorSources {
    host: GravitationalParameter,
    belts: Vec<SourceBelt>,
    innermost_giant: Option<Metres>,
    icy_mass: EarthMasses,
    comet_edge: Option<Metres>,
}

impl ImpactorSources {
    /// The sources about a host of gravitational parameter `host`, with the belts `belts`, in
    /// their generated order, which breaks a tie between equally near belts, and the planets
    /// `planets`, as their masses and semi-major axes about it, in any order.
    ///
    /// # Errors
    ///
    /// - [`BuildImpactorSourcesError::HostParameter`] for a parameter that is not finite and
    ///   positive;
    /// - [`BuildImpactorSourcesError::BeltEdges`] for an annulus whose edges are not finite,
    ///   positive and ordered;
    /// - [`BuildImpactorSourcesError::BeltMass`] for a component's mass that is not finite and
    ///   non-negative, or a scattered mass other than zero on a belt with no scattered component;
    /// - [`BuildImpactorSourcesError::Planet`] for a planet whose mass is not finite and
    ///   non-negative, or whose semi-major axis is not finite and positive.
    pub fn new(
        host: GravitationalParameter,
        belts: Vec<SourceBelt>,
        planets: impl IntoIterator<Item = (EarthMasses, Metres)>,
    ) -> Result<Self, BuildImpactorSourcesError> {
        if !positive(host.value()) {
            return Err(BuildImpactorSourcesError::HostParameter(host));
        }
        let mut icy_mass = EarthMasses::ZERO;
        let mut comet_edge: Option<Metres> = None;
        for belt in &belts {
            for annulus in core::iter::once(belt.main).chain(belt.scattered) {
                let (inner, outer) = (annulus.inner_edge, annulus.outer_edge);
                if !(positive(inner.value()) && outer.value().is_finite() && inner <= outer) {
                    return Err(BuildImpactorSourcesError::BeltEdges(inner, outer));
                }
            }
            for mass in [belt.main_mass, belt.scattered_mass] {
                if !non_negative(mass.value()) {
                    return Err(BuildImpactorSourcesError::BeltMass(mass));
                }
            }
            if belt.scattered.is_none() && belt.scattered_mass.value() > 0.0 {
                return Err(BuildImpactorSourcesError::BeltMass(belt.scattered_mass));
            }
            match belt.composition {
                BeltComposition::Icy => {
                    icy_mass = icy_mass + belt.main_mass + belt.scattered_mass;
                    let edge = belt.main.inner_edge;
                    if comet_edge.is_none_or(|outermost| edge > outermost) {
                        comet_edge = Some(edge);
                    }
                }
                BeltComposition::Rocky => {}
            }
        }
        let mut innermost_giant: Option<Metres> = None;
        for (mass, a) in planets {
            if !(non_negative(mass.value()) && positive(a.value())) {
                return Err(BuildImpactorSourcesError::Planet(mass, a));
            }
            if mass >= IMPACTOR_GIANT_MASS && innermost_giant.is_none_or(|giant| a < giant) {
                innermost_giant = Some(a);
            }
        }
        Ok(Self {
            host,
            belts,
            innermost_giant,
            icy_mass,
            comet_edge,
        })
    }

    /// The host's gravitational parameter, m³ s⁻²: the star's, or a close binary's sum for a
    /// circumbinary orbit.
    #[must_use]
    pub const fn host(&self) -> GravitationalParameter {
        self.host
    }

    /// The host's belts, in their generated order.
    #[must_use]
    pub fn belts(&self) -> &[SourceBelt] {
        &self.belts
    }

    /// The semi-major axis of the host's innermost giant, a planet of at least
    /// [`IMPACTOR_GIANT_MASS`]; `None` for a host with none.
    #[must_use]
    pub const fn innermost_giant(&self) -> Option<Metres> {
        self.innermost_giant
    }

    /// The host's icy belts' mass over the Solar Kuiper belt's, `μ_ic`, dimensionless: every
    /// component of each belt of [`BeltComposition::Icy`] at [`SOURCE_MASS_AGE`], over
    /// [`KUIPER_BELT_MASS`]; the comets' abundance scales as it (science-p14-t24b-laws §2.2).
    #[must_use]
    pub fn icy_belt_ratio(&self) -> f64 {
        self.icy_mass / KUIPER_BELT_MASS
    }

    /// `a_K`, the inner edge of the belt proper of the host's outermost icy belt (the greatest
    /// inner edge among them), from which the comets' abundance is scaled (science-p14-t24b-laws
    /// §2.7 (a2)); `None` for a host with no icy belt.
    #[must_use]
    pub const fn comet_edge(&self) -> Option<Metres> {
        self.comet_edge
    }
}

/// Why [`ImpactorSources::new`] refused its parts, with the value it refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildImpactorSourcesError {
    /// The host's gravitational parameter is not finite and positive.
    HostParameter(GravitationalParameter),
    /// A belt's annulus, its inner and outer edges, is not finite, positive and ordered.
    BeltEdges(Metres, Metres),
    /// A belt component's mass is not finite and non-negative, or a belt with no scattered
    /// component has a scattered mass.
    BeltMass(EarthMasses),
    /// A planet's mass is not finite and non-negative, or its semi-major axis is not finite and
    /// positive.
    Planet(EarthMasses, Metres),
}

impl fmt::Display for BuildImpactorSourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostParameter(mu) => write!(
                f,
                "host parameter {} m³ s⁻² is not finite and positive",
                mu.value()
            ),
            Self::BeltEdges(inner, outer) => write!(
                f,
                "belt edges {} m to {} m are not finite, positive and ordered",
                inner.value(),
                outer.value()
            ),
            Self::BeltMass(mass) => write!(
                f,
                "belt mass {} M⊕ is out of range or has no component",
                mass.value()
            ),
            Self::Planet(mass, a) => write!(
                f,
                "planet of {} M⊕ at {} m is out of range",
                mass.value(),
                a.value()
            ),
        }
    }
}

impl Error for BuildImpactorSourcesError {}

/// What [`impactors`] reads of a body: where it goes round, its bulk and surface, and its host's
/// sources.
///
/// Plain data with public fields: [`impactors`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpactorBody<'a> {
    /// Where it goes round.
    pub orbit: ImpactorOrbit,
    /// Its mean radius, m, finite and positive: the bulk section's.
    pub radius: Metres,
    /// Its bulk density, kg m⁻³, finite and positive: the bulk section's, a porous target's.
    pub density: KilogramsPerCubicMetre,
    /// Its mean surface gravity, m s⁻², finite and positive: the bulk section's.
    pub surface_gravity: MetresPerSecondSquared,
    /// Its mean surface temperature, K, finite and non-negative: P14.T24.a's.
    pub surface_temperature: Kelvin,
    /// Its water, ice and liquid, at its surface and in its crust, kg, finite and non-negative:
    /// the volatile inventory's (P14.T13.a), or a moon's ice.
    pub water: Kilograms,
    /// Its host's sources.
    pub sources: &'a ImpactorSources,
}

/// Whether a body formed icy, as its crust reads it ([`Crust::of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Birthplace {
    /// A moon formed beyond its parent's ice line, or an icy belt's member.
    Icy,
    /// Any other body, whose crust is ice-rich by its water layer alone.
    Other,
}

/// What a body's crust is, as its craters and its relief read it (science-r09-craters.md items 1
/// and 2, and F5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Crust {
    /// Rock: [`SILICATE_K_TARGET`], and [`SILICATE_TARGET_DENSITY`] for a non-porous target.
    Silicate,
    /// Ice-rich: [`ICE_K_TARGET`], and ice Ih's density for a non-porous target.
    IceRich,
}

impl Crust {
    /// The crust of a body of birthplace `birthplace`, at mean surface temperature
    /// `surface_temperature`, holding `water` over a radius `radius`.
    ///
    /// It is ice-rich when the body lies under [`ICE_RICH_TEMPERATURE`] and either formed icy or
    /// holds a global water layer of at least [`ICE_RICH_LAYER_DEPTH`] ([`ice_layer_depth`]). It
    /// is the one predicate of the crust's mechanical factors: `k_target`, the ice target's
    /// density, and P14.T48.a's `k_comp` and ice lithosphere.
    /// [`SurfaceMaterial`](crate::planetary::derive::atmosphere::SurfaceMaterial) classes the
    /// surface's albedo instead (science-r09-craters.md F5.4).
    #[must_use]
    pub fn of(
        birthplace: Birthplace,
        surface_temperature: Kelvin,
        water: Kilograms,
        radius: Metres,
    ) -> Self {
        let cold = surface_temperature < ICE_RICH_TEMPERATURE;
        let icy = match birthplace {
            Birthplace::Icy => true,
            Birthplace::Other => ice_layer_depth(water, radius) >= ICE_RICH_LAYER_DEPTH,
        };
        if cold && icy {
            Self::IceRich
        } else {
            Self::Silicate
        }
    }

    /// Its target factor `k_target`.
    #[must_use]
    pub const fn k_target(self) -> f64 {
        match self {
            Self::Silicate => SILICATE_K_TARGET,
            Self::IceRich => ICE_K_TARGET,
        }
    }

    /// Its density as a non-porous target: a silicate crust's [`SILICATE_TARGET_DENSITY`], or an
    /// ice-rich crust's ice Ih at the body's mean surface temperature `surface_temperature`.
    ///
    /// Ice Ih's is [`ice_ih_density`](crate::substance::ice_ih_density)'s: IAPWS R10-06(2009),
    /// 933.8 kg m⁻³ at 0 K to 916.7 at the melting point. Water ice is of the non-porous class in
    /// the gravity regime ("the scaling parameters are similar for all non-porous materials (e.g.,
    /// rock versus ice)", Singer et al. 2019, Science 363, 955, supplement). It is the pure ice's
    /// value: rock dust raises it and porosity lowers it, and P14.T51.c's primary crust replaces it
    /// (science-r09-craters.md F5.3).
    #[must_use]
    pub fn non_porous_density(self, surface_temperature: Kelvin) -> KilogramsPerCubicMetre {
        match self {
            Self::Silicate => SILICATE_TARGET_DENSITY,
            Self::IceRich => substance::ice_ih_density(surface_temperature),
        }
    }
}

/// How a moon came to orbit its parent, as its birthplace reads it ([`MoonBirth::formed`]).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MoonBirth {
    /// A regular moon, formed in its parent's nursery (P14.T17), with its ice fraction by mass
    /// ([`DerivedMoon::ice_fraction`](crate::planetary::moons::DerivedMoon::ice_fraction)).
    Regular {
        /// Its ice fraction by mass, 0 to 1: above zero beyond its parent's ice line.
        ice_fraction: f64,
    },
    /// A giant-impact moon (P14.T18), formed from its parent's mantle and the impactor's.
    GiantImpact,
    /// A captured moon of its kind (P14.T19).
    Captured(CaptureKind),
}

impl MoonBirth {
    /// The side of its parent's ice line a moon of this birth formed on, as its crust reads it
    /// ([`ImpactorOrbit::Moon`], [`Crust::of`]; science-p14-t24b-laws §2.4), about the parent
    /// `parent`, next to a belt of composition `neighbouring_belt` (`None` with no belt next to
    /// it).
    ///
    /// - A regular moon formed beyond the ice line when it holds ice.
    /// - A giant-impact moon takes its parent's birthplace: beyond for an icy belt's member, and
    ///   inside for any other parent, as [`Crust::of`]'s [`Birthplace`] reads a host orbiter. The
    ///   Moon's isotopes match Earth's mantle (Zhang et al. 2012, Nature Geoscience 5, 251; Young
    ///   et al. 2016, Science 351, 493). Charon formed in a giant impact on Pluto, probably as the
    ///   intact impactor, between bodies of the early Kuiper belt (Canup 2005, Science 307, 546).
    /// - A giant's capture, an irregular or a Triton-like one, formed beyond: irregulars come from
    ///   the outer planetesimal disc (Nesvorný, Vokrouhlický and Morbidelli 2007, AJ 133, 1962),
    ///   and Triton from the Kuiper belt (Agnor and Hamilton 2006, Nature 441, 192).
    /// - A rocky planet's capture comes from the belt it needs next to it (P14.T19): beyond for an
    ///   icy belt and inside otherwise, so Phobos and Deimos, beside the rocky main belt, formed
    ///   inside.
    ///
    /// `neighbouring_belt` is the composition of the belt next to the parent, with no planet
    /// between them
    /// ([`PlanetarySystem::nearest_belt`](crate::planetary::PlanetarySystem::nearest_belt) with
    /// [`BeltAdjacency::Neighbouring`](crate::planetary::moons::BeltAdjacency::Neighbouring)), and
    /// `None` with no belt next to it; only a rocky planet's capture reads it.
    ///
    /// # Examples
    ///
    /// Mars's small captures, beside the rocky main belt, formed inside the ice line, and would
    /// have formed beyond it beside an icy belt; a giant's irregulars formed beyond it whatever
    /// lies beside the giant.
    ///
    /// ```
    /// use hyperion_sim::planetary::belts::BeltComposition;
    /// use hyperion_sim::planetary::derive::SnowLineSide;
    /// use hyperion_sim::planetary::hooks::figures::{HostOrbit, HostOrbiter, MoonBirth};
    /// use hyperion_sim::planetary::moons::CaptureKind;
    /// use hyperion_sim::units::consts::METRES_PER_AU;
    /// use hyperion_sim::units::{EarthMasses, Metres, Radians};
    ///
    /// let orbiter = |a_au: f64, mass: f64| HostOrbiter {
    ///     orbit: HostOrbit {
    ///         semi_major_axis: Metres::new(a_au * METRES_PER_AU),
    ///         eccentricity: 0.05,
    ///         inclination: Radians::ZERO,
    ///     },
    ///     mass: EarthMasses::new(mass),
    ///     belt: None,
    /// };
    /// let (mars, jupiter) = (orbiter(1.524, 0.107), orbiter(5.2, 317.8));
    /// let small = MoonBirth::Captured(CaptureKind::Small);
    /// assert_eq!(small.formed(&mars, Some(BeltComposition::Rocky)), SnowLineSide::Inside);
    /// assert_eq!(small.formed(&mars, Some(BeltComposition::Icy)), SnowLineSide::Beyond);
    /// let irregular = MoonBirth::Captured(CaptureKind::Irregular);
    /// assert_eq!(irregular.formed(&jupiter, None), SnowLineSide::Beyond);
    /// ```
    #[must_use]
    pub fn formed(
        self,
        parent: &HostOrbiter,
        neighbouring_belt: Option<BeltComposition>,
    ) -> SnowLineSide {
        let icy = match self {
            Self::Regular { ice_fraction } => ice_fraction > 0.0,
            Self::GiantImpact => matches!(
                parent.belt,
                Some(BeltMembership {
                    composition: BeltComposition::Icy,
                    ..
                })
            ),
            Self::Captured(CaptureKind::Irregular | CaptureKind::Large) => true,
            Self::Captured(CaptureKind::Small) => neighbouring_belt == Some(BeltComposition::Icy),
        };
        if icy {
            SnowLineSide::Beyond
        } else {
            SnowLineSide::Inside
        }
    }
}

/// Which rule gave a body's impactors (the [module](self) documentation's three).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImpactorSource {
    /// Rule 1: a belt the body is a member of or orbits inside, by that component's excitation.
    InBelt {
        /// The component's excitation `U_b`².
        excitation: f64,
        /// The belt's composition.
        composition: BeltComposition,
    },
    /// Rule 2: the giants' comets, for a giant, a body at or beyond the innermost giant, and their
    /// moons.
    Comets,
    /// Rule 3: the nearest belt, from inside or outside it.
    NearestBelt {
        /// Its excitation `U_b`² plus the cube of the fractional distance.
        excitation: f64,
        /// The belt's composition.
        composition: BeltComposition,
    },
    /// Rule 3 with no belt: the comets, from inside the innermost giant, by
    /// [`COMET_EXCITATION`]² plus the cube of the fractional distance, or alone with no giant.
    InnerComets {
        /// [`COMET_EXCITATION`]² plus the cube.
        excitation: f64,
    },
}

impl ImpactorSource {
    /// The impactors' own excitation `U_src`², dimensionless.
    #[must_use]
    pub const fn excitation(self) -> f64 {
        match self {
            Self::InBelt { excitation, .. }
            | Self::NearestBelt { excitation, .. }
            | Self::InnerComets { excitation } => excitation,
            Self::Comets => COMET_EXCITATION * COMET_EXCITATION,
        }
    }

    /// The projectiles' density: [`ROCKY_PROJECTILE_DENSITY`] from a rocky belt and
    /// [`ICY_PROJECTILE_DENSITY`] from an icy belt or the comets.
    #[must_use]
    pub const fn projectile_density(self) -> KilogramsPerCubicMetre {
        match self {
            Self::InBelt { composition, .. } | Self::NearestBelt { composition, .. } => {
                match composition {
                    BeltComposition::Rocky => ROCKY_PROJECTILE_DENSITY,
                    BeltComposition::Icy => ICY_PROJECTILE_DENSITY,
                }
            }
            Self::Comets | Self::InnerComets { .. } => ICY_PROJECTILE_DENSITY,
        }
    }

    /// The source of the impactors striking `orbiter`, or a moon of it, under `sources`.
    #[cfg(test)]
    #[must_use]
    fn of(orbiter: &HostOrbiter, sources: &ImpactorSources) -> Self {
        Self::with_abundance(orbiter, sources).0
    }

    /// The source of the impactors striking `orbiter`, or a moon of it, under `sources`, with
    /// their abundance Ψ over the Moon's by the same rule, before the floor
    /// ([`floored_abundance`]): the rate and the velocity read one population.
    #[must_use]
    fn with_abundance(orbiter: &HostOrbiter, sources: &ImpactorSources) -> (Self, f64) {
        let a = orbiter.orbit.semi_major_axis;
        // Rule 1: a member shares its part's excitation; any body orbiting in an annulus, that
        // annulus's.
        if let Some(membership) = orbiter.belt {
            let source = Self::InBelt {
                excitation: belt_excitation(membership.part),
                composition: membership.composition,
            };
            let abundance =
                member_component(sources, membership, a).map_or(0.0, |(annulus, mass)| {
                    in_belt_abundance(annulus, mass, membership.part, membership.composition)
                });
            return (source, abundance);
        }
        for belt in &sources.belts {
            let parts = [
                (BeltPart::Main, Some(belt.main), belt.main_mass),
                (BeltPart::Scattered, belt.scattered, belt.scattered_mass),
            ];
            for (part, annulus, mass) in parts {
                if let Some(annulus) = annulus.filter(|annulus| annulus.holds(a)) {
                    let source = Self::InBelt {
                        excitation: belt_excitation(part),
                        composition: belt.composition,
                    };
                    let abundance = in_belt_abundance(annulus, mass, part, belt.composition);
                    return (source, abundance);
                }
            }
        }
        // Rule 2.
        if orbiter.mass >= IMPACTOR_GIANT_MASS || sources.innermost_giant.is_some_and(|g| a >= g) {
            return (Self::Comets, comet_abundance(sources, a));
        }
        // Rule 3: an asteroid belt first, then any belt, then the innermost giant. The
        // abundance reads an asteroid belt alone (science-p14-t24b-laws §2.2): with none, the
        // giants' comets at the giant thin inward as (a ÷ a_g)², rule 2 having taken every body at
        // or beyond it (§2.7), and with no giant either it is the floor's.
        let inner_comets = sources.innermost_giant.map_or(0.0, |a_g| {
            let inward = a.value() / a_g.value();
            comet_abundance(sources, a_g) * inward * inward
        });
        let asteroid = nearest_belt(
            sources
                .belts
                .iter()
                .filter(|b| b.kind == BeltKind::Asteroid),
            a,
        );
        if let Some(belt) = asteroid.or_else(|| nearest_belt(sources.belts.iter(), a)) {
            let a_b = belt.main.mid_radius();
            let source = Self::NearestBelt {
                excitation: belt_excitation(BeltPart::Main) + approach(a, a_b),
                composition: belt.composition,
            };
            let abundance = match belt.kind {
                BeltKind::Asteroid => approach_abundance(belt, a),
                BeltKind::Kuiper => inner_comets,
            };
            return (source, abundance);
        }
        let excitation = COMET_EXCITATION * COMET_EXCITATION;
        let source = Self::InnerComets {
            excitation: sources
                .innermost_giant
                .map_or(excitation, |a_g| excitation + approach(a, a_g)),
        };
        (source, inner_comets)
    }
}

/// The giants' comets' abundance `Ψ_c` at a semi-major axis `a` about the host of `sources`
/// (science-p14-t24b-laws §2.7 (a2)): [`COMET_DENSITY`] × `μ_ic` × s³ (s · min(a, `a_K`) ÷ 5.2
/// au)^k, with `a_K` the inner edge of the belt proper of the host's outermost icy belt
/// ([`ImpactorSources::comet_edge`]), s = 39.4027 au ÷ `a_K` ([`SOLAR_KUIPER_BELT_INNER_EDGE_AU`])
/// and k [`COMET_DISTANCE_EXPONENT`]; zero with no icy belt, where `μ_ic` is zero too.
///
/// The s³ keeps a scaled copy of the Solar System at its number density, as rule 3's cube and rule
/// 1's torus do; about the Sun s is 1. It rises outward to its value at `a_K`, which it holds
/// beyond. It is calibrated from 5.2 to 30.1 au about the Sun, and inward of 5.2 au the same power
/// goes on falling, an extrapolation.
#[must_use]
fn comet_abundance(sources: &ImpactorSources, a: Metres) -> f64 {
    let Some(a_k) = sources.comet_edge else {
        return 0.0;
    };
    let s = SOLAR_KUIPER_BELT_INNER_EDGE_AU * METRES_PER_AU / a_k.value();
    let reach = if a < a_k { a } else { a_k };
    let x = s * reach.value() / (COMET_REFERENCE_DISTANCE_AU * METRES_PER_AU);
    COMET_DENSITY * sources.icy_belt_ratio() * s * s * s * math::powf(x, COMET_DISTANCE_EXPONENT)
}

/// The projectile abundance Ψ, the impactors' number density at a body over the Moon's for
/// projectiles of one size, from its source rule's `abundance`: at least [`LEFTOVER_DENSITY`]
/// (science-p14-t24b-laws §2.2; the [module](self) documentation).
#[must_use]
fn floored_abundance(abundance: f64) -> f64 {
    // By comparison, so that a NaN, which validated inputs never make, takes the floor.
    if abundance > LEFTOVER_DENSITY {
        abundance
    } else {
        LEFTOVER_DENSITY
    }
}

/// The component a belt's member belongs to, with its mass: of the belts of the member's
/// composition that have its part, the first whose annulus holds the member's semi-major axis
/// `a`, as P14.T21.c places every member, or failing one the nearest of them by
/// [`nearest_belt`]'s ratio; `None` with no such belt.
#[must_use]
fn member_component(
    sources: &ImpactorSources,
    membership: BeltMembership,
    a: Metres,
) -> Option<(SourceAnnulus, EarthMasses)> {
    let component = |belt: &SourceBelt| match membership.part {
        BeltPart::Main => Some((belt.main, belt.main_mass)),
        BeltPart::Scattered => belt.scattered.map(|annulus| (annulus, belt.scattered_mass)),
    };
    let candidates = || {
        sources
            .belts
            .iter()
            .filter(|belt| belt.composition == membership.composition)
            .filter_map(component)
    };
    candidates()
        .find(|(annulus, _)| annulus.holds(a))
        .or_else(|| {
            candidates()
                .fold(
                    None,
                    |best: Option<((SourceAnnulus, EarthMasses), f64)>, c| {
                        let (r, x) = (c.0.mid_radius().value(), a.value());
                        let closeness = r.min(x) / r.max(x);
                        match best {
                            Some((_, b)) if closeness <= b => best,
                            Some(_) | None => Some((c, closeness)),
                        }
                    },
                )
                .map(|(c, _)| c)
        })
}

/// The density proxy of a component of mass `mass` between `inner_au` and `outer_au` of the part
/// `part`, M⊕ au⁻³: its mass over a torus of its annulus and its inclination width,
/// M ÷ ((`a_o`² − `a_i`²) √(`a_i` `a_o`) sin σᵢ), with σᵢ [`MEMBER_INCLINATION_WIDTH_DEG`] or
/// [`SCATTERED_INCLINATION_WIDTH_DEG`]; zero for an annulus of no width.
#[must_use]
fn torus_density_earth_masses_per_au3(
    mass: EarthMasses,
    (inner_au, outer_au): (f64, f64),
    part: BeltPart,
) -> f64 {
    let width_deg = match part {
        BeltPart::Main => MEMBER_INCLINATION_WIDTH_DEG,
        BeltPart::Scattered => SCATTERED_INCLINATION_WIDTH_DEG,
    };
    let volume = (outer_au * outer_au - inner_au * inner_au)
        * (inner_au * outer_au).sqrt()
        * math::sin(width_deg.to_radians());
    if volume > 0.0 {
        mass.value() / volume
    } else {
        0.0
    }
}

/// Rule 1's abundance in the annulus `annulus` of part `part` holding `mass`, of a belt of
/// composition `composition`: `K_in` × its torus's density over the Solar main belt's
/// ([`torus_density_earth_masses_per_au3`]), with `K_in` [`IN_BELT_DENSITY`] for a rocky belt and
/// [`ICY_IN_BELT_DENSITY`] for an icy one (science-p14-t24b-laws §2.7 (b)).
#[must_use]
fn in_belt_abundance(
    annulus: SourceAnnulus,
    mass: EarthMasses,
    part: BeltPart,
    composition: BeltComposition,
) -> f64 {
    let au = |m: Metres| m.value() / METRES_PER_AU;
    let edges = (au(annulus.inner_edge), au(annulus.outer_edge));
    let own = torus_density_earth_masses_per_au3(mass, edges, part);
    let solar = torus_density_earth_masses_per_au3(
        SOLAR_MAIN_BELT_MASS,
        SOLAR_MAIN_BELT_EDGES_AU,
        BeltPart::Main,
    );
    let k_in = match composition {
        BeltComposition::Rocky => IN_BELT_DENSITY,
        BeltComposition::Icy => ICY_IN_BELT_DENSITY,
    };
    k_in * own / solar
}

/// Rule 3's abundance at a semi-major axis `a` outside the belt proper of the asteroid belt `belt`:
/// `μ_b` (`a_i☉` ÷ `a_i`)³ ((1 − 1 au ÷ `a_i☉`) ÷ (1 − a ÷ `a_i`))^1.5 inside its inner edge `a_i`,
/// with (1 − `a_o` ÷ a) in place of (1 − a ÷ `a_i`) beyond its outer edge `a_o`; `μ_b` its mass over
/// [`SOLAR_MAIN_BELT_MASS`] and `a_i☉` the Solar main belt's inner edge. The cube keeps a scaled
/// copy of the Solar System at its density; the power is [`APPROACH_EXPONENT`]. It is 1 at 1 au
/// inside the Solar main belt.
#[must_use]
fn approach_abundance(belt: &SourceBelt, a: Metres) -> f64 {
    let solar_inner = SOLAR_MAIN_BELT_EDGES_AU.0 * METRES_PER_AU;
    let (a, inner, outer) = (
        a.value(),
        belt.main.inner_edge.value(),
        belt.main.outer_edge.value(),
    );
    let distance = if a < inner {
        1.0 - a / inner
    } else {
        1.0 - outer / a
    };
    let scale = solar_inner / inner;
    let reference = 1.0 - METRES_PER_AU / solar_inner;
    belt.main_mass / SOLAR_MAIN_BELT_MASS
        * (scale * scale * scale)
        * math::powf(reference / distance, APPROACH_EXPONENT)
}

/// A belt component's own excitation `U_b`² = ⟨e²⟩ + ⟨sin² i⟩ = 2σₑ² + 2 sin² σᵢ, dimensionless,
/// from the Rayleigh widths its members are placed by (P14.T21.c): σₑ
/// [`MEMBER_ECCENTRICITY_WIDTH`] and σᵢ [`MEMBER_INCLINATION_WIDTH_DEG`] in the belt proper, 0.0587,
/// and [`SCATTERED_ECCENTRICITY_WIDTH`] and [`SCATTERED_INCLINATION_WIDTH_DEG`] in a scattered
/// component, 0.414 (science-r09-craters.md item 4, whose 0.30 was 0.297, the inclination's factor
/// 2 dropped, F5.1).
///
/// To second order it is Öpik's U² = 2 − 2√(1 − e²) cos i at equal semi-major axes. Over the same
/// untruncated laws, that U²'s exact mean is 0.0586 and 0.408, within 0.2% and 1.4% of these. The
/// exact ⟨e²⟩ + ⟨sin² i⟩, 0.0580 and 0.388, lies 1.0% and 5.0% below it (F5.6). The members'
/// truncation at the crossing limit is not applied: the record's population moments are the
/// untruncated laws' (P14.T48.g).
#[must_use]
pub fn belt_excitation(part: BeltPart) -> f64 {
    let (sigma_e, sigma_i_deg) = match part {
        BeltPart::Main => (MEMBER_ECCENTRICITY_WIDTH, MEMBER_INCLINATION_WIDTH_DEG),
        BeltPart::Scattered => (
            SCATTERED_ECCENTRICITY_WIDTH,
            SCATTERED_INCLINATION_WIDTH_DEG,
        ),
    };
    let sin_i = math::sin(sigma_i_deg.to_radians());
    2.0 * sigma_e * sigma_e + 2.0 * sin_i * sin_i
}

/// The approach term of rule 3, (1 − a ÷ `a_b`)³ inside the reference radius `a_b` and
/// (1 − `a_b` ÷ a)³ outside it: the cube of the fractional distance, zero at `a_b`.
#[must_use]
fn approach(a: Metres, a_b: Metres) -> f64 {
    let x = if a < a_b {
        1.0 - a.value() / a_b.value()
    } else {
        1.0 - a_b.value() / a.value()
    };
    x * x * x
}

/// The nearest of `belts` to a semi-major axis `a`, by its belt proper's mid-radius `a_b`: the
/// least ratio max(a, `a_b`) ÷ min(a, `a_b`), the first on a tie; `None` for no belt.
#[must_use]
fn nearest_belt<'b>(
    belts: impl Iterator<Item = &'b SourceBelt>,
    a: Metres,
) -> Option<&'b SourceBelt> {
    let closeness = |belt: &SourceBelt| {
        let a_b = belt.main.mid_radius().value();
        let a = a.value();
        a.min(a_b) / a.max(a_b)
    };
    belts
        .fold(None, |best: Option<(&SourceBelt, f64)>, belt| {
            let c = closeness(belt);
            match best {
                Some((_, b)) if c <= b => best,
                Some(_) | None => Some((belt, c)),
            }
        })
        .map(|(belt, _)| belt)
}

/// The depth of the global layer a body's water would make, m: its mass spread over a sphere of
/// the body's radius at [`WATER_LAYER_DENSITY`], m ÷ (ρ 4π R²).
#[must_use]
pub fn ice_layer_depth(water: Kilograms, radius: Metres) -> Metres {
    let area = 4.0 * core::f64::consts::PI * radius.value() * radius.value();
    Metres::new(water.value() / (WATER_LAYER_DENSITY.value() * area))
}

/// A body's impactors (P14.T48.f): their mean impact velocity and density, and the crater target
/// they strike, its class, density and factor, which P14.T48.c's crater contract carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impactors {
    velocity: MetresPerSecond,
    source: ImpactorSource,
    target: CraterTarget,
    crust: Crust,
    abundance: f64,
    encounter: MetresPerSecond,
}

impl Impactors {
    /// The mean impact velocity, m/s, finite and positive.
    #[must_use]
    pub const fn velocity(&self) -> MetresPerSecond {
        self.velocity
    }

    /// `R_b`, the impact rate per unit area on the body over the Moon's for projectiles of one
    /// size (P14.T48.c; science-p14-t24b-laws §2.2), dimensionless and positive: the projectile
    /// abundance Ψ over the Moon's times the encounter factor V over the Moon's
    /// [`LUNAR_ENCOUNTER_SPEED`], in particle-in-a-box form (Öpik 1951).
    ///
    /// - V is v∞ (1 + `v_esc`² ÷ v∞²) for a body that goes round the host, the encounter speed
    ///   with the escape speed's focusing; and for a moon `v_enc` √(1 + 2 `v_K,s`² ÷ v∞,P²)
    ///   (1 + `v_esc,s`² ÷ `v_enc`²), with `v_enc`² = v∞,P² + 3 `v_K,s`². The middle factor is its
    ///   parent's focusing at its orbit as the impactors' number-density gain, n∞ v ÷ v∞ (Liouville;
    ///   §2.7 (a1)), since `v_enc` already holds the speed's gain: the regular moons' rates over
    ///   their planets' then match Zahnle et al. 2003's Table 1 within 11%.
    /// - Ψ follows the source rule ([`ImpactorSource`]): in a belt, [`IN_BELT_DENSITY`] (rocky) or
    ///   [`ICY_IN_BELT_DENSITY`] (icy) × the component's density over the Solar main belt's; the
    ///   giants' comets, [`COMET_DENSITY`] × `μ_ic` ([`ImpactorSources::icy_belt_ratio`]) × s³
    ///   (s · min(a, `a_K`) ÷ 5.2 au)^1.15 (`comet_abundance`, §2.7), with a the body's or its
    ///   parent's semi-major axis; toward the nearest asteroid belt, its mass over
    ///   [`SOLAR_MAIN_BELT_MASS`] × (2.065 au ÷ `a_i`)³ × ((1 − 1 au ÷ 2.065 au) ÷ (1 − a ÷
    ///   `a_i`))^1.5 inside its inner edge, or (1 − `a_o` ÷ a) beyond its outer edge; with no
    ///   asteroid belt, the comets at the innermost giant thinned by (a ÷ `a_g`)² inside it,
    ///   whatever belt the velocity reads, and with no giant either none; and at least
    ///   [`LEFTOVER_DENSITY`].
    /// - The belts' masses are at [`SOURCE_MASS_AGE`], so it is constant in time: the lunar
    ///   chronology it multiplies carries the time profile.
    ///
    /// The Moon's is 1, Mars's 1.96, Ganymede's 3.71 and Pluto's 30.1. It multiplies the lunar
    /// chronology into the crater contract's
    /// lunar-equivalent N☾(>1 km), as R09's reference worlds read it.
    #[must_use]
    pub fn rate(&self) -> f64 {
        self.abundance * (self.encounter / LUNAR_ENCOUNTER_SPEED)
    }

    /// The projectile abundance Ψ over the Moon's, dimensionless, at least [`LEFTOVER_DENSITY`]:
    /// the first factor of [`rate`](Self::rate).
    #[must_use]
    pub const fn abundance(&self) -> f64 {
        self.abundance
    }

    /// The encounter factor V, m/s, finite and positive: the second factor of
    /// [`rate`](Self::rate), over [`LUNAR_ENCOUNTER_SPEED`].
    #[must_use]
    pub const fn encounter_speed(&self) -> MetresPerSecond {
        self.encounter
    }

    /// The projectiles' density, kg m⁻³: [`ROCKY_PROJECTILE_DENSITY`] or
    /// [`ICY_PROJECTILE_DENSITY`].
    #[must_use]
    pub const fn projectile_density(&self) -> KilogramsPerCubicMetre {
        self.source.projectile_density()
    }

    /// The crater target: porous at the body's bulk density below [`POROUS_TARGET_RADIUS`], and
    /// otherwise non-porous at its crust's density.
    #[must_use]
    pub const fn target(&self) -> CraterTarget {
        self.target
    }

    /// The target factor `k_target`, its crust's.
    #[must_use]
    pub const fn k_target(&self) -> f64 {
        self.crust.k_target()
    }

    /// What its crust is, by the ice-rich predicate of science-r09-craters.md item 2 and F5.4
    /// ([`Crust::of`]), which sets `k_target` and the non-porous target's density.
    #[must_use]
    pub const fn crust(&self) -> Crust {
        self.crust
    }

    /// Which rule gave the impactors.
    #[must_use]
    pub const fn source(&self) -> ImpactorSource {
        self.source
    }
}

/// Why [`impactors`] refused a body, with the value it refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveImpactorsError {
    /// The semi-major axis about the host is not finite and positive.
    SemiMajorAxis(Metres),
    /// The eccentricity is not in `[0, 1)`.
    Eccentricity(f64),
    /// The inclination is not finite.
    Inclination(Radians),
    /// The mass is not finite and non-negative, or a moon's parent's is not positive.
    Mass(EarthMasses),
    /// A moon's semi-major axis about its parent is not finite and positive.
    MoonSemiMajorAxis(Metres),
    /// The radius is not finite and positive.
    Radius(Metres),
    /// The bulk density is not finite and positive.
    Density(KilogramsPerCubicMetre),
    /// The surface gravity is not finite and positive.
    SurfaceGravity(MetresPerSecondSquared),
    /// The surface temperature is not finite and non-negative.
    SurfaceTemperature(Kelvin),
    /// The water is not finite and non-negative.
    Water(Kilograms),
}

impl fmt::Display for DeriveImpactorsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemiMajorAxis(a) => write!(
                f,
                "semi-major axis {} m is not finite and positive",
                a.value()
            ),
            Self::Eccentricity(e) => write!(f, "eccentricity {e} is not in [0, 1)"),
            Self::Inclination(i) => write!(f, "inclination {} rad is not finite", i.value()),
            Self::Mass(m) => write!(f, "mass {} M⊕ is out of range", m.value()),
            Self::MoonSemiMajorAxis(a) => write!(
                f,
                "moon's semi-major axis {} m is not finite and positive",
                a.value()
            ),
            Self::Radius(r) => write!(f, "radius {} m is not finite and positive", r.value()),
            Self::Density(rho) => write!(
                f,
                "density {} kg m⁻³ is not finite and positive",
                rho.value()
            ),
            Self::SurfaceGravity(g) => write!(
                f,
                "surface gravity {} m s⁻² is not finite and positive",
                g.value()
            ),
            Self::SurfaceTemperature(t) => write!(
                f,
                "surface temperature {} K is not finite and non-negative",
                t.value()
            ),
            Self::Water(w) => write!(f, "water {} kg is not finite and non-negative", w.value()),
        }
    }
}

impl Error for DeriveImpactorsError {}

/// Whether `x` is finite and positive.
#[must_use]
fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// Whether `x` is finite and non-negative.
#[must_use]
fn non_negative(x: f64) -> bool {
    x.is_finite() && x >= 0.0
}

/// Checks a host orbiter's orbit and mass.
fn check_orbiter(orbiter: &HostOrbiter) -> Result<(), DeriveImpactorsError> {
    let HostOrbit {
        semi_major_axis: a,
        eccentricity: e,
        inclination: i,
    } = orbiter.orbit;
    if !positive(a.value()) {
        return Err(DeriveImpactorsError::SemiMajorAxis(a));
    }
    if !(e.is_finite() && (0.0..1.0).contains(&e)) {
        return Err(DeriveImpactorsError::Eccentricity(e));
    }
    if !i.value().is_finite() {
        return Err(DeriveImpactorsError::Inclination(i));
    }
    if !non_negative(orbiter.mass.value()) {
        return Err(DeriveImpactorsError::Mass(orbiter.mass));
    }
    Ok(())
}

/// A body's two impact speeds ([`speeds`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Speeds {
    /// The square of the mean impact velocity, m² s⁻².
    velocity_squared_m2_s2: f64,
    /// The encounter factor V of the rate ([`Impactors::rate`]).
    encounter: MetresPerSecond,
}

/// The impact speeds of a body struck from `source` that goes round its host as `orbiter` does,
/// or a moon of it at `moon` from it, of escape speed squared `escape_squared` m² s⁻², about a
/// host of parameter `host` (the [module](self) documentation, and [`Impactors::rate`]).
#[must_use]
fn speeds(
    source: ImpactorSource,
    orbiter: &HostOrbiter,
    moon: Option<Metres>,
    host: GravitationalParameter,
    escape_squared: f64,
) -> Speeds {
    let HostOrbit {
        semi_major_axis: a,
        eccentricity: e,
        inclination: i,
    } = orbiter.orbit;
    let sin_i = math::sin(i.value());
    let encounter = source.excitation() + e * e + sin_i * sin_i;
    let circular_squared = host.value() / a.value();
    let v_infinity_squared = encounter * circular_squared;
    let (velocity_squared_m2_s2, encounter) = match moon {
        None => (
            v_infinity_squared + escape_squared,
            v_infinity_squared.sqrt() * (1.0 + escape_squared / v_infinity_squared),
        ),
        Some(a_s) => {
            let parent = GravitationalParameter::from_earth_masses(orbiter.mass);
            let orbital_squared = parent.value() / a_s.value();
            let meeting_squared = v_infinity_squared + 3.0 * orbital_squared;
            (
                meeting_squared + escape_squared,
                meeting_squared.sqrt()
                    * (1.0 + 2.0 * orbital_squared / v_infinity_squared).sqrt()
                    * (1.0 + escape_squared / meeting_squared),
            )
        }
    };
    Speeds {
        velocity_squared_m2_s2,
        encounter: MetresPerSecond::new(encounter),
    }
}

/// The impactors of the body `body` describes (P14.T48.f): their mean impact velocity, their
/// density and the crater target, by the [module](self) documentation's closed form, with no
/// draw.
///
/// # Errors
///
/// - [`DeriveImpactorsError::SemiMajorAxis`] for a semi-major axis about the host that is not
///   finite and positive, and [`DeriveImpactorsError::MoonSemiMajorAxis`] for a moon's about its
///   parent;
/// - [`DeriveImpactorsError::Eccentricity`] for an eccentricity outside `[0, 1)`, and
///   [`DeriveImpactorsError::Inclination`] for an inclination that is not finite;
/// - [`DeriveImpactorsError::Mass`] for a mass that is not finite and non-negative, or a moon's
///   parent's that is not positive;
/// - [`DeriveImpactorsError::Radius`], [`DeriveImpactorsError::Density`] and
///   [`DeriveImpactorsError::SurfaceGravity`] for a radius, bulk density or gravity that is not
///   finite and positive;
/// - [`DeriveImpactorsError::SurfaceTemperature`] and [`DeriveImpactorsError::Water`] for a
///   temperature or a water mass that is not finite and non-negative.
///
/// # Examples
///
/// The Moon, whose impactors are Earth's, from the asteroid belt inside Jupiter: its mean impact
/// velocity is the diameter map's lunar reference.
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::planetary::belts::BeltComposition;
/// use hyperion_sim::planetary::derive::SnowLineSide;
/// use hyperion_sim::planetary::hooks::figures::{
///     HostOrbit, HostOrbiter, ImpactorBody, ImpactorOrbit, ImpactorSources, SOLAR_MAIN_BELT_MASS,
///     SourceAnnulus, SourceBelt, impactors,
/// };
/// use hyperion_sim::planetary::record::BeltKind;
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{
///     EarthMasses, GravitationalParameter, Kelvin, Kilograms, KilogramsPerCubicMetre, Metres,
///     MetresPerSecondSquared, Radians, SolarMasses,
/// };
/// use hyperion_surface::craters::{LUNAR_IMPACT_VELOCITY, TargetClass};
///
/// let au = |x: f64| Metres::new(x * METRES_PER_AU);
/// // The asteroid belt between Jupiter's 4:1 and 2:1 resonances.
/// let jupiter = 5.2029;
/// let belt = SourceBelt {
///     kind: BeltKind::Asteroid,
///     composition: BeltComposition::Rocky,
///     main: SourceAnnulus {
///         inner_edge: au(jupiter * math::powf(0.25, 2.0 / 3.0)),
///         outer_edge: au(jupiter * math::powf(0.5, 2.0 / 3.0)),
///     },
///     scattered: None,
///     main_mass: SOLAR_MAIN_BELT_MASS,
///     scattered_mass: EarthMasses::ZERO,
/// };
/// let sun = ImpactorSources::new(
///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///     vec![belt],
///     [(EarthMasses::new(1.0), au(1.0)), (EarthMasses::new(317.8), au(jupiter))],
/// )?;
/// let earth = HostOrbiter {
///     orbit: HostOrbit { semi_major_axis: au(1.0), eccentricity: 0.0167, inclination: Radians::ZERO },
///     mass: EarthMasses::new(1.0),
///     belt: None,
/// };
/// let moon = impactors(&ImpactorBody {
///     orbit: ImpactorOrbit::Moon { parent: earth, semi_major_axis: Metres::new(3.844e8), formed: SnowLineSide::Inside },
///     radius: Metres::new(1.7374e6),
///     density: KilogramsPerCubicMetre::new(3_344.0),
///     surface_gravity: MetresPerSecondSquared::new(1.62),
///     surface_temperature: Kelvin::new(250.0),
///     water: Kilograms::ZERO,
///     sources: &sun,
/// })?;
/// let v = moon.velocity().value();
/// assert!((v - LUNAR_IMPACT_VELOCITY.value()).abs() < 100.0);
/// assert_eq!(moon.target().class, TargetClass::NonPorous);
/// assert!((moon.k_target() - 1.0).abs() < 1e-12);
/// // Its impact rate per unit area is about the Moon's own.
/// assert!((moon.rate() - 1.0).abs() < 0.01);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn impactors(body: &ImpactorBody<'_>) -> Result<Impactors, DeriveImpactorsError> {
    let (orbiter, moon) = match body.orbit {
        ImpactorOrbit::Host(orbiter) => {
            check_orbiter(&orbiter)?;
            (orbiter, None)
        }
        ImpactorOrbit::Moon {
            parent,
            semi_major_axis,
            formed,
        } => {
            check_orbiter(&parent)?;
            if !positive(parent.mass.value()) {
                return Err(DeriveImpactorsError::Mass(parent.mass));
            }
            if !positive(semi_major_axis.value()) {
                return Err(DeriveImpactorsError::MoonSemiMajorAxis(semi_major_axis));
            }
            (parent, Some((semi_major_axis, formed)))
        }
    };
    let (radius, density, gravity) = (body.radius, body.density, body.surface_gravity);
    if !positive(radius.value()) {
        return Err(DeriveImpactorsError::Radius(radius));
    }
    if !positive(density.value()) {
        return Err(DeriveImpactorsError::Density(density));
    }
    if !positive(gravity.value()) {
        return Err(DeriveImpactorsError::SurfaceGravity(gravity));
    }
    if !non_negative(body.surface_temperature.value()) {
        return Err(DeriveImpactorsError::SurfaceTemperature(
            body.surface_temperature,
        ));
    }
    if !non_negative(body.water.value()) {
        return Err(DeriveImpactorsError::Water(body.water));
    }

    let (source, abundance) = ImpactorSource::with_abundance(&orbiter, body.sources);
    let escape_squared = 2.0 * gravity.value() * radius.value();
    let speeds = speeds(
        source,
        &orbiter,
        moon.map(|(a_s, _)| a_s),
        body.sources.host,
        escape_squared,
    );

    let birthplace = match (moon, orbiter.belt) {
        (Some((_, SnowLineSide::Beyond)), _)
        | (
            None,
            Some(BeltMembership {
                composition: BeltComposition::Icy,
                ..
            }),
        ) => Birthplace::Icy,
        (Some((_, SnowLineSide::Inside)), _)
        | (
            None,
            Some(BeltMembership {
                composition: BeltComposition::Rocky,
                ..
            })
            | None,
        ) => Birthplace::Other,
    };
    let crust = Crust::of(birthplace, body.surface_temperature, body.water, radius);
    let target = if radius < POROUS_TARGET_RADIUS {
        CraterTarget {
            class: TargetClass::Porous,
            density,
        }
    } else {
        CraterTarget {
            class: TargetClass::NonPorous,
            density: crust.non_porous_density(body.surface_temperature),
        }
    };
    Ok(Impactors {
        velocity: MetresPerSecond::new(speeds.velocity_squared_m2_s2.sqrt()),
        source,
        target,
        crust,
        abundance: floored_abundance(abundance),
        encounter: speeds.encounter,
    })
}

/// What a subsurface ocean rests on (P14.T24.b's `subsurface_ocean`): rock, or the high-pressure
/// ice whose liquidus meets the ocean's temperature (P14.T51.g, 23 → 24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OceanFloor {
    /// The rock core or mantle.
    Rock,
    /// Ice III.
    IceIii,
    /// Ice V.
    IceV,
    /// Ice VI.
    IceVi,
    /// Ice VII.
    IceVii,
}

/// A body's ocean under its surface (P14.T24.b, P14.T51.g): its ice shell, its liquid's depth,
/// its floor, the liquid and its solutes, and its temperature at its base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubsurfaceOcean {
    shell_thickness: Metres,
    thickness: Metres,
    floor: OceanFloor,
    liquid: SubstanceId,
    solute_mass_fraction: f64,
    base_temperature: Kelvin,
}

impl SubsurfaceOcean {
    /// The ocean of a regular moon of radius `radius`, density `density` and water mass fraction
    /// `water`, with a surface at `surface` and a tidal heat flux `flux` (P14.T17.b's flag,
    /// [`has_subsurface_ocean`](crate::planetary::moons::regular::has_subsurface_ocean), with the
    /// inputs its derivation gives it); `None` where the flag says there is none.
    ///
    /// Its shell is the flag's own, 651 ln(273 K ÷ `T_s`) ÷ F, and its depth the moon's water layer
    /// less that shell. It is pure water on rock, at 273 K at its base
    /// ([`ICE_MELTING_POINT`]): the flag's model, until P14.T51.g's liquidus, solutes and floor
    /// (23 → 24).
    ///
    /// # Examples
    ///
    /// Europa, 8% water under a surface at 102 K, with 50 mW m⁻² from below, keeps an ocean about
    /// 13 km under its ice and some 124 km deep to its rock:
    ///
    /// ```
    /// use hyperion_sim::planetary::hooks::figures::{OceanFloor, SubsurfaceOcean};
    /// use hyperion_sim::units::{Kelvin, KilogramsPerCubicMetre, Metres, WattsPerSquareMetre};
    ///
    /// let europa = SubsurfaceOcean::of_regular_moon(
    ///     Metres::new(1.5608e6),
    ///     KilogramsPerCubicMetre::new(3_013.0),
    ///     0.08,
    ///     Kelvin::new(102.0),
    ///     WattsPerSquareMetre::new(0.05),
    /// )
    /// .expect("an ocean");
    /// assert!((europa.shell_thickness().value() / 1e3 - 12.8).abs() < 0.1);
    /// assert!((europa.thickness().value() / 1e3 - 124.2).abs() < 0.5);
    /// assert_eq!(europa.floor(), OceanFloor::Rock);
    /// ```
    #[must_use]
    pub fn of_regular_moon(
        radius: Metres,
        density: KilogramsPerCubicMetre,
        water: f64,
        surface: Kelvin,
        flux: WattsPerSquareMetre,
    ) -> Option<Self> {
        let layers = ocean_layers(radius, density, water, surface, flux)?;
        (layers.shell_thickness < layers.water_depth).then(|| Self {
            shell_thickness: layers.shell_thickness,
            thickness: Metres::new(layers.water_depth.value() - layers.shell_thickness.value()),
            floor: OceanFloor::Rock,
            liquid: Gas::Water.substance(),
            solute_mass_fraction: 0.0,
            base_temperature: ICE_MELTING_POINT,
        })
    }

    /// The ice shell over it, m.
    #[must_use]
    pub const fn shell_thickness(&self) -> Metres {
        self.shell_thickness
    }

    /// Its liquid's depth, from the shell's base to its floor, m.
    #[must_use]
    pub const fn thickness(&self) -> Metres {
        self.thickness
    }

    /// What it rests on.
    #[must_use]
    pub const fn floor(&self) -> OceanFloor {
        self.floor
    }

    /// Its liquid, a registry row: water, `H2O`, for every rule so far.
    #[must_use]
    pub const fn liquid(&self) -> SubstanceId {
        self.liquid
    }

    /// The mass fraction of solutes in its liquid, 0 to 1: none for every rule so far.
    #[must_use]
    pub const fn solute_mass_fraction(&self) -> f64 {
        self.solute_mass_fraction
    }

    /// Its temperature at its base, where the shell meets it.
    #[must_use]
    pub const fn base_temperature(&self) -> Kelvin {
        self.base_temperature
    }
}

/// Whether a body holds an ocean under its surface, as far as a rule computes it (P14.T24.b's
/// `subsurface_ocean`: none for a body computed to have none, and absent for one no rule computes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Subsurface {
    /// No rule computes it for this body: until P14.T51.g's bump (23 → 24), every body but a
    /// regular moon. It must never be read as "no ocean".
    NotComputed,
    /// A rule computes that the body holds none.
    NoOcean,
    /// The body holds this ocean.
    Ocean(SubsurfaceOcean),
}

impl Subsurface {
    /// A regular moon's, by P14.T17.b's flag ([`SubsurfaceOcean::of_regular_moon`], with its
    /// inputs).
    #[must_use]
    pub fn of_regular_moon(
        radius: Metres,
        density: KilogramsPerCubicMetre,
        water: f64,
        surface: Kelvin,
        flux: WattsPerSquareMetre,
    ) -> Self {
        SubsurfaceOcean::of_regular_moon(radius, density, water, surface, flux)
            .map_or(Self::NoOcean, Self::Ocean)
    }
}

/// What [`GlobalFigures::derive`] reads of a body at a time, as plain values: its mass, its bulk
/// section's radius, gravity, class and mass fractions, its atmosphere and its water, its host's
/// \[Fe/H\], its age, its rotation, the tidal heat raised in it, its subsurface ocean as its rule
/// computes it, what its crater contract reads ([`CraterInputs`]) and what sets its continents
/// ([`Continents`]).
///
/// The fields are public: [`GlobalFigures::derive`] refuses, for a body with a surface, an age
/// that is not finite and non-negative, and the radius, gravity, air temperature, water and crater
/// inputs that [`impactors`] refuses; it reads the rest as given. At an age of 0 the rock makes
/// its birth's production, 4.95 times today's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FiguresInputs<'a> {
    /// The body's atmosphere at the time (P14.T13), whose state says whether it has a surface and
    /// whose water whether a rocky lid can move.
    pub atmosphere: &'a Atmosphere,
    /// Its mass at the time.
    pub mass: EarthMasses,
    /// Its mean radius, the bulk section's.
    pub radius: Metres,
    /// Its mean surface gravity, the bulk section's: its water's floor pressure is ρ g `d_w`.
    pub surface_gravity: MetresPerSecondSquared,
    /// Its class, the bulk section's: a `Rocky` mantle convects, and so does an `Icy` body's rock
    /// from [`CONVECTING_ROCK_RADIUS`](interior::CONVECTING_ROCK_RADIUS) ([`Interior::of`]).
    pub class: PlanetClass,
    /// Its mass fractions, the bulk section's: the rock holds the radiogenic isotopes, and the
    /// iron the core whose heat the mantle carries.
    pub fractions: MassFractions,
    /// The water of its volatile inventory (P14.T13.a), before escape and the climate divide it.
    /// Its water's global-equivalent depth is the larger of this and its bulk water, a moon's or
    /// an icy member's ice, over its surface at
    /// [`REGIME_WATER_DENSITY`](interior::REGIME_WATER_DENSITY).
    pub inventory_water: Kilograms,
    /// Its sidereal rotation period at the time, s, positive, infinite for a body that does not
    /// turn (P14.T14's; the orbital period for a 1:1 lock), which sets a thermal dynamo's geometry.
    pub rotation_period: Seconds,
    /// Its host's \[Fe/H\], which scales its isotopes.
    pub fe_h: Dex,
    /// Its age, the system's at the time: its isotopes decay from its birth.
    pub age: Years,
    /// The tidal heat its parent raises in it, over its surface, W m⁻², at least 0 (a negative or
    /// NaN value is read as 0): a moon's, and zero for a body that goes round a star.
    pub tidal_heat_flow: WattsPerSquareMetre,
    /// Its ocean under the surface, as its rule computes it.
    pub subsurface_ocean: Subsurface,
    /// What its crater contract reads beyond these.
    pub craters: CraterInputs<'a>,
    /// What sets its continental fraction before its regime gates it (P14.T48.a): its drawn
    /// asymptote ([`relief::continental_asymptote`]) with whether it entered P14.T48.b's window
    /// holding water ([`window_entry`]), or a fraction given.
    pub continents: Continents,
}

/// What a body's surface age and crater contract read beyond its bulk section, its air and its
/// heat flow ([`GlobalFigures::derive`]; P14.T24.b, T48.c and T48.f), as plain values.
///
/// Plain data with public fields: [`impactors`] validates the orbit, the density and the
/// sources' use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterInputs<'a> {
    /// Where it goes round, a moon with its birthplace ([`MoonBirth::formed`]).
    pub orbit: ImpactorOrbit,
    /// Its bulk density, the bulk section's: a porous target's.
    pub density: KilogramsPerCubicMetre,
    /// Its host's impactor sources.
    pub sources: &'a ImpactorSources,
    /// The system age at which its young magma ocean ended (P14.T28.a's `molten_until`); `None`
    /// for a body with no magma-ocean draw, whose surface age then reads its own age.
    pub molten_until: Option<Years>,
    /// The system age of its last giant impact (P14.T18), its parent's for a giant-impact moon;
    /// `None` for a body that had none.
    pub last_giant_impact: Option<Years>,
}

/// Why [`GlobalFigures::derive`] refused a body, with what it refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveFiguresError {
    /// The age is not finite and non-negative.
    Age(Years),
    /// The impactors' inputs are out of range ([`impactors`]): the radius, the gravity, the air
    /// temperature, the water or the crater inputs.
    Impactors(DeriveImpactorsError),
    /// The crater contract's parts are out of the shared density's range ([`CraterParams::new`]).
    Craters(BuildCraterParamsError),
}

impl fmt::Display for DeriveFiguresError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Age(age) => write!(f, "age {} yr is not finite and non-negative", age.value()),
            Self::Impactors(error) => write!(f, "impactors: {error}"),
            Self::Craters(error) => write!(f, "crater contract: {error}"),
        }
    }
}

impl Error for DeriveFiguresError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Age(_) => None,
            Self::Impactors(error) => Some(error),
            Self::Craters(error) => Some(error),
        }
    }
}

/// Julian years in a thousand million, the chronology's unit.
const YEARS_PER_GIGAYEAR: f64 = 1e9;

/// A body's global figures at a time (P14.T24.b): its heat flow, its interior, its tectonic
/// regime, its magnetic field, its present outgassing, its subsurface ocean, its surface age and
/// its crater contract (P14.T48.c) with its impactors (P14.T48.f), its volatile history
/// (P14.T48.b): its wet epoch and whether it came from a colder state, its volcanism level,
/// continents and relief (P14.T48.a), and its surface water, oceans, ices and cloud.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalFigures {
    state: SurfaceState,
    radiogenic_heat_flow: WattsPerSquareMetre,
    non_tidal_heat_flow: WattsPerSquareMetre,
    heat_flow: WattsPerSquareMetre,
    interior: Interior,
    tectonic_regime: TectonicRegime,
    magnetic_field: MagneticField,
    outgassing: f64,
    subsurface_ocean: Subsurface,
    surface_age: Years,
    impactors: Impactors,
    craters: CraterParams,
    wet_epoch: Option<WetEpoch>,
    from_colder: bool,
    volcanism: f64,
    relief: Relief,
    surface_water: KilogramsPerSquareMetre,
    potential_ocean: f64,
    ocean_weight: OceanWeight,
    surface_liquids: Vec<(SubstanceId, f64)>,
    surface_ices: Vec<(SubstanceId, f64)>,
    cloud_fraction: f64,
    /// The atmosphere's cloud fraction, which the second stage publishes.
    air_cloud_fraction: f64,
}

impl GlobalFigures {
    /// The figures of the body `inputs` describe; `None` for a body in the gas-envelope state or
    /// of a class with no solid surface, which takes no solid heat flow.
    ///
    /// The heat flow is [`interior`]'s: the rock's radiogenic heat, with the residual of its
    /// interior (a conducting rock's lag, or a convecting mantle's Urey ratio by its regime with
    /// its core's heat), and the tidal heat added. The regime reads the radiogenic term before the
    /// residual, the tide, the state, the water's depth, the gravity and the ocean's shell, and the
    /// field the regime, the iron's share, the heat, the age and the rotation.
    ///
    /// The crater contract reads the impactors ([`impactors`]) of the body at its air's mean
    /// surface temperature, P14.T13's, which is the surface conditions' mean, with the water its
    /// regime reads; the surface age ([`surface_age::surface_age`]) its heat flow and their crust;
    /// and the lunar chronology the surface age in the system's age, times their rate
    /// ([`GlobalFigures::craters`]).
    ///
    /// # Errors
    ///
    /// For a body with a surface (a gas envelope or a class with none is `Ok(None)` first):
    /// - [`DeriveFiguresError::Age`] for an age that is not finite and non-negative;
    /// - [`DeriveFiguresError::Impactors`] for what [`impactors`] refuses: a radius or gravity that
    ///   is not finite and positive, an air temperature or water that is not finite and
    ///   non-negative, or crater inputs out of range;
    /// - [`DeriveFiguresError::Craters`] for a contract [`CraterParams::new`] refuses, which
    ///   inputs in range never make.
    ///
    /// # Examples
    ///
    /// Earth, as [`derive_body`](crate::planetary::derive::derive_body)'s example derives it, a
    /// wet mobile lid that outgasses at about Earth's rate, under the Sun's asteroid belt and
    /// Jupiter, renews its ocean floor in some 64 Myr and keeps few craters:
    ///
    /// ```
    /// use hyperion_sim::math;
    /// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// use hyperion_sim::planetary::belts::BeltComposition;
    /// use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
    /// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    /// use hyperion_sim::planetary::hooks::figures::{
    ///     Continents, CraterInputs, FiguresInputs, GlobalFigures, HostOrbit, HostOrbiter,
    ///     ImpactorOrbit, ImpactorSources, MagneticField, SOLAR_MAIN_BELT_MASS, SourceAnnulus,
    ///     SourceBelt, Subsurface, TectonicRegime, WindowEntry,
    /// };
    /// use hyperion_sim::planetary::record::BeltKind;
    /// use hyperion_sim::stellar::Composition;
    /// use hyperion_sim::stellar::draws::UnitUniform;
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
    /// use hyperion_sim::units::{
    ///     Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres, Radians,
    ///     Seconds, SolarLuminosities, SolarMasses, SolarRadii, WattsPerSquareMetre, Years,
    /// };
    ///
    /// let host = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
    /// let disc = disc::derive(&host, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
    /// let disc = disc.profile().expect("a median disc exists");
    /// let a = Metres::new(METRES_PER_AU);
    /// let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
    /// let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
    /// let orbit = KeplerElements::from_semi_major_axis(a, mu, Eccentricity::new(0.0167)?, orientation, Radians::ZERO)?;
    /// let rank = UnitUniform::new(0.384).expect("inside (0, 1)");
    /// let earth = PlacedBody::new(EarthMasses::new(1.0), orbit, a, rank)?;
    /// let sun = [HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?];
    /// let hosts = BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[])?;
    /// let age = Years::new(4.57e9);
    /// let derived = derive_body(&earth, &hosts, disc, age, UniverseTime::EPOCH)?;
    /// // The Sun's asteroid belt, between Jupiter's 4:1 and 2:1 resonances, and Jupiter.
    /// let au = |x: f64| Metres::new(x * METRES_PER_AU);
    /// let belt = SourceBelt {
    ///     kind: BeltKind::Asteroid,
    ///     composition: BeltComposition::Rocky,
    ///     main: SourceAnnulus {
    ///         inner_edge: au(5.2029 * math::powf(0.25, 2.0 / 3.0)),
    ///         outer_edge: au(5.2029 * math::powf(0.5, 2.0 / 3.0)),
    ///     },
    ///     scattered: None,
    ///     main_mass: SOLAR_MAIN_BELT_MASS,
    ///     scattered_mass: EarthMasses::ZERO,
    /// };
    /// let sources = ImpactorSources::new(mu, vec![belt], [(EarthMasses::new(317.8), au(5.2029))])?;
    /// let orbit = ImpactorOrbit::Host(HostOrbiter {
    ///     orbit: HostOrbit { semi_major_axis: a, eccentricity: 0.0167, inclination: Radians::ZERO },
    ///     mass: EarthMasses::new(1.0),
    ///     belt: None,
    /// });
    /// let figures = GlobalFigures::derive(&FiguresInputs {
    ///     atmosphere: derived.atmosphere(),
    ///     mass: derived.mass(),
    ///     radius: Metres::from(derived.radius()),
    ///     surface_gravity: derived.surface_gravity(),
    ///     class: derived.class(),
    ///     fractions: derived.fractions(),
    ///     inventory_water: derived.inventory().water(),
    ///     rotation_period: Seconds::new(86_164.1),
    ///     fe_h: Dex::new(0.0),
    ///     age,
    ///     tidal_heat_flow: WattsPerSquareMetre::new(0.0),
    ///     subsurface_ocean: Subsurface::NotComputed,
    ///     craters: CraterInputs {
    ///         orbit,
    ///         density: derived.density(),
    ///         sources: &sources,
    ///         molten_until: Some(Years::new(3.2e7)),
    ///         last_giant_impact: Some(Years::ZERO),
    ///     },
    ///     continents: Continents::Grown { asymptote: 0.416, window: WindowEntry::Entered },
    /// })?
    /// .expect("an Earth has a surface");
    /// assert_eq!(figures.tectonic_regime(), TectonicRegime::MobileLid);
    /// assert_eq!(figures.magnetic_field(), MagneticField::Strong);
    /// assert!((figures.heat_flow().value() / 0.092 - 1.0).abs() < 0.05);
    /// assert!((figures.outgassing() - 1.0).abs() < 0.1);
    /// assert!((figures.surface_age().value() / 64.2e6 - 1.0).abs() < 0.05);
    /// assert!(figures.crater_density().value() < 1e-4);
    /// // Its continents hold 0.405 of its surface, its relief is about Earth2014's 2.51 km, and
    /// // its water would fill some 73% of it, which weighs fully in its contrasts.
    /// assert!((figures.continental_fraction() - 0.405).abs() < 0.002);
    /// assert!((figures.sigma_h().value() / 2_510.0 - 1.0).abs() < 0.05);
    /// assert!((0.70..0.76).contains(&figures.potential_ocean()));
    /// assert!((figures.ocean_weight().get() - 1.0).abs() < 1e-12);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn derive(inputs: &FiguresInputs<'_>) -> Result<Option<Self>, DeriveFiguresError> {
        let air = inputs.atmosphere;
        if matches!(air.state(), SurfaceState::GasEnvelope) {
            return Ok(None);
        }
        let heat_inputs = inputs.heat();
        let Some((interior, rock_core)) = interior_of(&heat_inputs) else {
            return Ok(None);
        };
        let age = inputs.age;
        if !non_negative(age.value()) {
            return Err(DeriveFiguresError::Age(age));
        }
        let heat = heat_at(&heat_inputs, (interior, rock_core), age, air.state());
        let field = magnetic_field(&interior::FieldInputs {
            class: inputs.class,
            interior,
            mass: inputs.mass,
            iron_share: heat.iron_share,
            regime: heat.regime,
            radiogenic: heat.radiogenic,
            tidal: heat.tidal,
            age,
            rotation_period: inputs.rotation_period,
        });
        let (surface_age, impactors, craters) =
            crater_contract(inputs, heat.water, heat.heat_flow)?;
        let outgassing = outgassing(heat.heat_flow);
        let crust = impactors.crust();
        let surface_temperature = air.surface_temperature();
        // Step 3 (science-p14-t24b-laws §4.6): the volcanism level over the surface's life, at the
        // heat flow then under today's state.
        let volcanism = relief::volcanism_level(&relief::VolcanismInputs {
            crust,
            surface_age,
            age,
            surface_temperature,
            surface_gravity: inputs.surface_gravity,
            radius: inputs.radius,
            iron_share: heat.iron_share,
            heat_flow_at: |t: Years| {
                heat_at(&heat_inputs, (interior, rock_core), t, air.state()).heat_flow
            },
        });
        // Steps 4 and 5: the continents, by the regime and T48.b's start condition, and the relief.
        let relief = Relief::of(&relief::ReliefInputs {
            regime: heat.regime,
            continental_fraction: relief::continental_fraction(inputs.continents, heat.regime, age),
            crust,
            surface_gravity: inputs.surface_gravity,
            radius: inputs.radius,
            crater_density: craters.n_1km(),
            volcanism,
            lithosphere: relief::lithosphere_thickness(crust, surface_temperature, heat.heat_flow),
        });
        // Step 6: the surface water, its potential ocean, and the weight θ the conditions read.
        let surface_water = cover::surface_water(
            air,
            inputs.inventory_water,
            inputs.radius,
            inputs.surface_gravity,
            outgassing,
        );
        let potential_ocean = cover::potential_ocean(relief.hypsometry(), surface_water);
        Ok(Some(Self {
            state: air.state(),
            radiogenic_heat_flow: heat.radiogenic,
            non_tidal_heat_flow: heat.non_tidal,
            heat_flow: heat.heat_flow,
            interior,
            tectonic_regime: heat.regime,
            magnetic_field: field,
            outgassing,
            subsurface_ocean: inputs.subsurface_ocean,
            surface_age,
            impactors,
            craters,
            wet_epoch: None,
            from_colder: false,
            volcanism,
            relief,
            surface_water,
            potential_ocean,
            ocean_weight: OceanWeight::of_potential_ocean(potential_ocean),
            surface_liquids: Vec::new(),
            surface_ices: Vec::new(),
            cloud_fraction: 0.0,
            air_cloud_fraction: air.cloud_fraction(),
        }))
    }

    /// The figures with what reads the surface conditions `conditions`, P14.T24.a's at this
    /// stage's heat flow and ocean weight θ, and the volatile history `history` (P14.T48.b) of
    /// their liquid-water flag ([`VolatileHistory::derive`]): the second stage, steps 8, 9, 11 and
    /// 12 of science-p14-t24b-laws §4.6.
    ///
    /// P14.T48.d's climate regime, step 10, reads this stage's history and ice and is the
    /// conditions'
    /// ([`SurfaceConditions::with_climate`](crate::planetary::hooks::surface::SurfaceConditions::with_climate)):
    /// the ice reads no regime, since an ice belt's band is the profile's own cold equator (§4.3).
    ///
    /// - **The published ocean** `f_o` ([`ocean_fraction`](Self::ocean_fraction)): the potential
    ///   ocean where the body holds liquid now, by the conditions' liquid-water flag or a
    ///   `Snowball`'s ocean under its ice ([`cover::under_ice_ocean`]), and zero otherwise;
    ///   [`surface_liquids`](Self::surface_liquids) is `[(H2O, f_o)]`.
    /// - **The volatile history**: its wet epoch and [`from_colder`](Self::from_colder).
    /// - **The ice** ([`cover::ice_fraction`]): the conditions' cold area, a `Snowball`'s whole
    ///   sphere, capped on a silicate crust by the surface water;
    ///   [`surface_ices`](Self::surface_ices) is `[(H2O, f_ice)]`.
    /// - **The cloud**: the atmosphere's ([`Atmosphere::cloud_fraction`]), with which its Bond
    ///   albedo was solved: P14.T13.c's state's ([`SurfaceState::cloud_fraction`]), and under the
    ///   `p14-snapshot` lane's batch the area mix's (P14.T51.b).
    ///
    /// [`derive`](Self::derive) alone gives the first stage, with no wet epoch, no ocean, ice or
    /// cloud listed and [`from_colder`](Self::from_colder) false: the figures
    /// [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at) publishes always
    /// carry the second.
    ///
    /// # Examples
    ///
    /// The order matters: the conditions read the first stage's heat flow and ocean weight θ, and
    /// the second stage reads the conditions. Earth, as [`derive`](Self::derive)'s example derives
    /// it, publishes its ocean where its water is liquid, a little polar ice and its cloud:
    ///
    /// ```
    /// # use hyperion_sim::math;
    /// # use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// # use hyperion_sim::planetary::belts::BeltComposition;
    /// # use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
    /// # use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    /// # use hyperion_sim::planetary::hooks::figures::{
    /// #     Continents, CraterInputs, FiguresInputs, GlobalFigures, HostOrbit, HostOrbiter,
    /// #     ImpactorOrbit, ImpactorSources, SOLAR_MAIN_BELT_MASS, SourceAnnulus, SourceBelt,
    /// #     Subsurface, WindowEntry,
    /// # };
    /// # use hyperion_sim::planetary::record::BeltKind;
    /// # use hyperion_sim::stellar::Composition;
    /// # use hyperion_sim::stellar::draws::UnitUniform;
    /// # use hyperion_sim::time::UniverseTime;
    /// # use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
    /// # use hyperion_sim::units::{
    /// #     Degrees, Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears,
    /// #     Metres, Radians, Seconds, SolarLuminosities, SolarMasses, SolarRadii,
    /// #     WattsPerSquareMetre, Years,
    /// # };
    /// # let host = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
    /// # let disc = disc::derive(&host, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
    /// # let disc = disc.profile().expect("a median disc exists");
    /// # let a = Metres::new(METRES_PER_AU);
    /// # let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
    /// # let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
    /// # let orbit = KeplerElements::from_semi_major_axis(a, mu, Eccentricity::new(0.0167)?, orientation, Radians::ZERO)?;
    /// # let earth = PlacedBody::new(EarthMasses::new(1.0), orbit, a, UnitUniform::new(0.384).expect("inside (0, 1)"))?;
    /// # let sun = [HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?];
    /// # let hosts = BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[])?;
    /// # let age = Years::new(4.57e9);
    /// # let derived = derive_body(&earth, &hosts, disc, age, UniverseTime::EPOCH)?;
    /// # let au = |x: f64| Metres::new(x * METRES_PER_AU);
    /// # let belt = SourceBelt {
    /// #     kind: BeltKind::Asteroid,
    /// #     composition: BeltComposition::Rocky,
    /// #     main: SourceAnnulus {
    /// #         inner_edge: au(5.2029 * math::powf(0.25, 2.0 / 3.0)),
    /// #         outer_edge: au(5.2029 * math::powf(0.5, 2.0 / 3.0)),
    /// #     },
    /// #     scattered: None,
    /// #     main_mass: SOLAR_MAIN_BELT_MASS,
    /// #     scattered_mass: EarthMasses::ZERO,
    /// # };
    /// # let sources = ImpactorSources::new(mu, vec![belt], [(EarthMasses::new(317.8), au(5.2029))])?;
    /// # let figures = GlobalFigures::derive(&FiguresInputs {
    /// #     atmosphere: derived.atmosphere(),
    /// #     mass: derived.mass(),
    /// #     radius: Metres::from(derived.radius()),
    /// #     surface_gravity: derived.surface_gravity(),
    /// #     class: derived.class(),
    /// #     fractions: derived.fractions(),
    /// #     inventory_water: derived.inventory().water(),
    /// #     rotation_period: Seconds::new(86_164.1),
    /// #     fe_h: Dex::new(0.0),
    /// #     age,
    /// #     tidal_heat_flow: WattsPerSquareMetre::new(0.0),
    /// #     subsurface_ocean: Subsurface::NotComputed,
    /// #     craters: CraterInputs {
    /// #         orbit: ImpactorOrbit::Host(HostOrbiter {
    /// #             orbit: HostOrbit { semi_major_axis: a, eccentricity: 0.0167, inclination: Radians::ZERO },
    /// #             mass: EarthMasses::new(1.0),
    /// #             belt: None,
    /// #         }),
    /// #         density: derived.density(),
    /// #         sources: &sources,
    /// #         molten_until: Some(Years::new(3.2e7)),
    /// #         last_giant_impact: Some(Years::ZERO),
    /// #     },
    /// #     continents: Continents::Grown { asymptote: 0.416, window: WindowEntry::Entered },
    /// # })?
    /// # .expect("an Earth has a surface");
    /// use hyperion_sim::planetary::derive::atmosphere::SurfaceMaterial;
    /// use hyperion_sim::planetary::hooks::figures::VolatileHistory;
    /// use hyperion_sim::planetary::hooks::surface::{SeasonalSpin, SurfaceConditions, SurfaceInputs};
    ///
    /// // The first stage's heat flow and θ go into the conditions.
    /// let conditions = SurfaceConditions::derive(&SurfaceInputs {
    ///     atmosphere: derived.atmosphere(),
    ///     equilibrium_temperature: derived.equilibrium_temperature(),
    ///     surface_gravity: derived.surface_gravity(),
    ///     radius: Metres::from(derived.radius()),
    ///     material: SurfaceMaterial::of(&derived.fractions()),
    ///     stellar_flux: WattsPerSquareMetre::from(derived.flux()),
    ///     xuv_flux: derived.xuv_flux(),
    ///     spin: SeasonalSpin {
    ///         obliquity: Radians::from(Degrees::new(23.44)),
    ///         rotation_period: Seconds::new(86_164.1),
    ///         solar_day: Some(Seconds::new(86_400.0)),
    ///     },
    ///     heat_flow: figures.heat_flow(),
    ///     ocean_weight: figures.ocean_weight(),
    /// })
    /// .expect("an Earth has a surface");
    /// // The second stage reads them.
    /// let figures = figures.with_surface(&conditions, VolatileHistory::NONE);
    /// assert!((0.70..0.76).contains(&figures.ocean_fraction()));
    /// assert!(figures.ice_fraction() > 0.0 && figures.ice_fraction() < 0.1);
    /// assert!((figures.cloud_fraction() - 0.67).abs() < 1e-12);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn with_surface(self, conditions: &SurfaceConditions, history: VolatileHistory) -> Self {
        self.with_surface_under(conditions, history, None)
    }

    /// [`with_surface`](Self::with_surface), with its ocean and ices from P14.T51.a's published
    /// condensates `condensates` where they are given, as the record gives them
    /// ([`condensates_under`](Self::condensates_under)).
    ///
    /// - The liquids are the condensates' ([`Condensates::surface_liquids`]), with water's ocean
    ///   under a `Snowball`'s ice ([`cover::under_ice_ocean`]) at its potential ocean where they hold
    ///   no liquid water.
    /// - The ices are the condensates' ([`Condensates::surface_ices`]), with T24.b's water ice
    ///   ([`cover::ice_fraction`]) where they hold none, as an icy body's crust is whose water the
    ///   partition does not hold (decision-p14-t51a-partition §2).
    /// - Each list is scaled down, in proportion, where it would cover more than the whole surface:
    ///   ices of several substances compete for the same cold zones.
    #[must_use]
    pub(crate) fn with_surface_under(
        self,
        conditions: &SurfaceConditions,
        history: VolatileHistory,
        condensates: Option<&Condensates>,
    ) -> Self {
        let water = Gas::Water.substance();
        // Step 8: the published ocean.
        let under_ice = cover::under_ice_ocean(
            self.state,
            self.surface_water,
            conditions.mean_surface_temperature(),
            self.heat_flow,
        );
        // Step 11: the ice, at the conditions' θ.
        let ice = || {
            cover::ice_fraction(&cover::IceInputs {
                conditions,
                state: self.state,
                crust: self.impactors.crust(),
                surface_water: self.surface_water,
            })
        };
        let (surface_liquids, surface_ices) = match condensates {
            None => {
                let ocean = if conditions.liquid_water() || under_ice {
                    self.potential_ocean
                } else {
                    0.0
                };
                (vec![(water, ocean)], vec![(water, ice())])
            }
            Some(condensates) => {
                let mut liquids = condensates.surface_liquids();
                if under_ice && condensates.entry(water, CondensatePhase::Liquid).is_none() {
                    liquids.push((water, self.potential_ocean));
                }
                let mut ices = condensates.surface_ices();
                if condensates.entry(water, CondensatePhase::Solid).is_none() {
                    ices.push((water, ice()));
                }
                (within_the_surface(liquids), within_the_surface(ices))
            }
        };
        Self {
            // Step 9: the volatile history.
            wet_epoch: history.wet_epoch().copied(),
            from_colder: history.from_colder(),
            surface_liquids,
            surface_ices,
            // Step 12: the cloud, the atmosphere's, bit for bit.
            cloud_fraction: self.air_cloud_fraction,
            ..self
        }
    }

    /// What P14.T51.a's partition gives the record of the body whose atmosphere is `atmosphere`,
    /// whose conditions are `conditions` and whose surface gravity is `gravity` (P14.T51.a's second
    /// part), under the cold traps' rules, sky and regolith `traps` (P14.T51.f and P14.T51.i, held
    /// by [`COLD_TRAPS_VERSION`](crate::planetary::hooks::cold_traps)): its partition and its
    /// published condensates.
    ///
    /// The partition is of the atmosphere's inventory before condensation
    /// ([`Atmosphere::column_inventory`]), over the conditions' climate (their range about their
    /// mean, [`ClimateSpan::about`]), on these figures' relief's basins ([`Basins::of`]). The
    /// condensates are published over these figures' cover ([`Cover`]): the conditions' cold
    /// area, the state, the crust and the water's surface share, all of it while the volcanic
    /// greenhouse runs and [`cover::DRY_SURFACE_WATER_SHARE`] of it after
    /// ([`cover::surface_water`]). Under [`ColdTrapRules::Held`] they are P14.T51.a's; under
    /// [`ColdTrapRules::Batch`] a body with no collisional atmosphere that is not locked to its
    /// star takes the cold traps' airless branch and their ices ([`cold_traps::publish`]).
    #[must_use]
    pub(crate) fn condensates_under(
        &self,
        atmosphere: &Atmosphere,
        conditions: &SurfaceConditions,
        gravity: MetresPerSecondSquared,
        traps: &RecordColdTraps<'_>,
    ) -> (Partition, Condensates) {
        let (coldest, warmest) = conditions.climate_range();
        let climate = ClimateSpan::about((coldest, warmest), conditions.mean_surface_temperature());
        let ground = Ground::new(
            gravity,
            Basins::of(self.relief.hypsometry()),
            cover::water_surface_share(self.outgassing),
        );
        let solved = partition(&climate, &ground, &atmosphere.column_inventory());
        // One profile for every solid's cold area, water's T24.b ice included.
        let profile = conditions.cold_profile();
        let cold_area = |t: Kelvin| profile.area_below(t);
        let cover = Cover {
            cold_area: &cold_area,
            state: atmosphere.state(),
            water: self.water_cover(conditions, &profile),
        };
        let published = match traps.rules {
            ColdTrapRules::Held => solved.publish(&cover),
            ColdTrapRules::Batch => cold_traps::publish(
                &solved,
                &cover,
                &ColdTrapBody {
                    atmosphere,
                    profile: &profile,
                    gravity,
                    heat_flow: self.heat_flow,
                    crust: self.impactors.crust(),
                    craters: &self.craters,
                    sky: traps.sky,
                    regolith_albedo: traps.regolith_albedo,
                },
            ),
        };
        (solved, published)
    }

    /// Water's cover under the conditions `conditions`, whose cold profile is `profile`: P14.T24.b's
    /// surface water, potential ocean and ice, the ice's area read off the profile at its edge, and
    /// ice Ih's density at the mean surface temperature ([`WaterCover`]).
    #[must_use]
    pub(crate) fn water_cover(
        &self,
        conditions: &SurfaceConditions,
        profile: &ColdProfile,
    ) -> WaterCover {
        let ice = cover::ice_fraction_over(
            &cover::IceInputs {
                conditions,
                state: self.state,
                crust: self.impactors.crust(),
                surface_water: self.surface_water,
            },
            profile.area_below(cover::ice_threshold(self.state)),
        );
        WaterCover {
            surface_water: self.surface_water,
            potential_ocean: self.potential_ocean,
            ice_fraction: ice,
            ice_density: substance::ice_ih_density(conditions.mean_surface_temperature()),
        }
    }

    /// The heat flow at the surface F, W m⁻²: the non-tidal part with the tidal heat added
    /// ([`interior`]). The generated Earth's is 0.0925 W m⁻², against Davies and Davies 2010's
    /// measured 0.0916.
    #[must_use]
    pub const fn heat_flow(&self) -> WattsPerSquareMetre {
        self.heat_flow
    }

    /// The non-tidal heat flow `F_nt`, W m⁻²: the radiogenic term with its interior's residual,
    /// which P14.T56.d's tidal branches add their own heat to (decision-p14-moons §F-ocean).
    #[must_use]
    pub const fn non_tidal_heat_flow(&self) -> WattsPerSquareMetre {
        self.non_tidal_heat_flow
    }

    /// The radiogenic heat flow `F_rad`, W m⁻²: the rock's present production over the surface,
    /// before the residual ([`radiogenic_heat_flow`]), which decides the regime.
    #[must_use]
    pub const fn radiogenic_heat_flow(&self) -> WattsPerSquareMetre {
        self.radiogenic_heat_flow
    }

    /// How the interior loses its heat ([`Interior::of`]): a conducting icy body's rock, or a
    /// convecting mantle.
    #[must_use]
    pub const fn interior(&self) -> Interior {
        self.interior
    }

    /// The tectonic regime, one of five by [`tectonic_regime`]'s rule.
    #[must_use]
    pub const fn tectonic_regime(&self) -> TectonicRegime {
        self.tectonic_regime
    }

    /// The global intrinsic magnetic field, by [`magnetic_field`]'s rule.
    #[must_use]
    pub const fn magnetic_field(&self) -> MagneticField {
        self.magnetic_field
    }

    /// The present volcanic outgassing O, dimensionless, relative to Earth's rate, at least 0
    /// ([`outgassing`]; the plan's member `outgassing`): 1 on the generated Earth, 0 where the heat
    /// flow is under
    /// [`SURFACE_MELT_HEAT_FLOW`](crate::planetary::params::SURFACE_MELT_HEAT_FLOW).
    #[must_use]
    pub const fn outgassing(&self) -> f64 {
        self.outgassing
    }

    /// The ocean under the surface, as its rule computes it ([`Subsurface`]).
    #[must_use]
    pub const fn subsurface_ocean(&self) -> &Subsurface {
        &self.subsurface_ocean
    }

    /// The surface age S, the time since the last global resurfacing, yr, in [0, age]
    /// ([`surface_age::surface_age`]): the least of the time since the magma ocean's end, the
    /// resurfacing time at the heat flow, and the time since a giant impact.
    #[must_use]
    pub const fn surface_age(&self) -> Years {
        self.surface_age
    }

    /// The lunar-equivalent crater density N☾(>1 km), per km²: the lunar chronology at the surface
    /// age in the system's age, times the impactors' rate `R_b` ([`Impactors::rate`]), before
    /// any screening or diameter map (P14.T48.c), the contract's
    /// [`n_1km`](CraterParams::n_1km).
    #[must_use]
    pub const fn crater_density(&self) -> PerSquareKilometre {
        self.craters.n_1km()
    }

    /// What strikes the body and what it strikes (P14.T48.f), with their rate.
    #[must_use]
    pub const fn impactors(&self) -> &Impactors {
        &self.impactors
    }

    /// The crater contract (P14.T48.c; R09's Design note 12): N☾(>1 km), the screening of the
    /// air's column mass p ÷ g (none for an airless body), the gravity, the system's age, and the
    /// impactors' velocity, density, target and `k_target`. The shared density
    /// ([`cumulative_density`](hyperion_surface::craters::cumulative_density)) derives the rest.
    #[must_use]
    pub const fn craters(&self) -> &CraterParams {
        &self.craters
    }

    /// The wet epoch (P14.T48.b; [`wet_epoch`](mod@wet_epoch)): the span over which the surface
    /// held liquid water, with what R09's erosion reads of it; `None` for a body with none, and
    /// before the second stage ([`with_surface`](Self::with_surface)).
    #[must_use]
    pub const fn wet_epoch(&self) -> Option<&WetEpoch> {
        self.wet_epoch.as_ref()
    }

    /// Whether the body reached its present state from a colder one (P14.T48.b), which P14.T48.d's
    /// ice belt reads: its flux lay beyond the maximum-greenhouse limit when its magma ocean ended
    /// and lies inside it now. False before the second stage.
    #[must_use]
    pub const fn from_colder(&self) -> bool {
        self.from_colder
    }

    /// The volcanism level V (P14.T48.a; [`relief::volcanism_level`]), dimensionless, in [0,
    /// 1.984]: the melt the surface's own life has erupted while its lithosphere held it, times its
    /// plume vigour, over the table's Mars's, whose V is 1, held to the strength limit of the
    /// constructional share ([`relief::VOLCANISM_SATURATION`]). Zero on an ice-rich crust.
    #[must_use]
    pub const fn volcanism(&self) -> f64 {
        self.volcanism
    }

    /// The continental fraction `f_c` (P14.T48.a; [`relief::continental_fraction`]), 0 to 1: zero
    /// on every lid but a mobile one, before its continents begin to grow, and on a world that
    /// never entered P14.T48.b's window holding water.
    #[must_use]
    pub const fn continental_fraction(&self) -> f64 {
        self.relief.continental_fraction()
    }

    /// The hypsometric standard deviation `σ_h`, m (P14.T48.a; [`Relief::sigma_h`]), relative to
    /// the reference equipotential with degree 1 included. No greatest relief is published.
    #[must_use]
    pub fn sigma_h(&self) -> Metres {
        self.relief.sigma_h()
    }

    /// The relief (P14.T48.a): `σ_h`'s three shares, the continental fraction and the hypsometry
    /// the oceans fill.
    #[must_use]
    pub const fn relief(&self) -> &Relief {
        &self.relief
    }

    /// The surface water `σ_s`, kg m⁻² ([`cover::surface_water`]): P14.T13.a's water over the
    /// surface while the volcanic greenhouse runs, and 2% of it after.
    #[must_use]
    pub const fn surface_water(&self) -> KilogramsPerSquareMetre {
        self.surface_water
    }

    /// The potential ocean `f_pot`, 0 to 1 ([`cover::potential_ocean`]): the share of the
    /// surface the surface water fills of the hypsometry, whether or not it is liquid now.
    #[must_use]
    pub const fn potential_ocean(&self) -> f64 {
        self.potential_ocean
    }

    /// The ocean's weight θ in P14.T24.a's contrasts, min(1, `f_pot` ÷ 0.708) ([`OceanWeight`]),
    /// which the surface conditions read.
    #[must_use]
    pub const fn ocean_weight(&self) -> OceanWeight {
        self.ocean_weight
    }

    /// The ocean fraction `f_o`, 0 to 1: the sum of [`surface_liquids`](Self::surface_liquids),
    /// zero before the second stage.
    #[must_use]
    pub fn ocean_fraction(&self) -> f64 {
        self.surface_liquids
            .iter()
            .fold(0.0, |sum, &(_, f)| sum + f)
    }

    /// The liquids at the surface as `(SubstanceId, area fraction)` (P14.T24.b): on the record
    /// P14.T51.a's published liquids, with water's ocean under a `Snowball`'s ice
    /// (`with_surface_under`); with no condensates given
    /// ([`with_surface`](Self::with_surface)) `H2O` alone, at the potential ocean where the body
    /// holds liquid now and zero otherwise; empty before the second stage.
    #[must_use]
    pub fn surface_liquids(&self) -> &[(SubstanceId, f64)] {
        &self.surface_liquids
    }

    /// The ice fraction, 0 to 1: the sum of [`surface_ices`](Self::surface_ices), zero before the
    /// second stage.
    #[must_use]
    pub fn ice_fraction(&self) -> f64 {
        self.surface_ices.iter().fold(0.0, |sum, &(_, f)| sum + f)
    }

    /// The temperature below which the climate holds the ice cover, by the body's own state
    /// ([`cover::ice_threshold`]): what P14.T48.d's climate regime reads beside
    /// [`ice_fraction`](Self::ice_fraction) to tell an equatorial belt from caps.
    #[must_use]
    pub fn ice_threshold(&self) -> Kelvin {
        cover::ice_threshold(self.state)
    }

    /// The ices at the surface as `(SubstanceId, area fraction)` (P14.T24.b): on the record
    /// P14.T51.a's published ices, with T24.b's water ice where they hold none
    /// (`with_surface_under`); with no condensates given `H2O` alone,
    /// the perennial water ice of [`cover::ice_fraction`]; empty before the second stage. An ice
    /// may lie over an ocean, as R09's cover convention has it.
    #[must_use]
    pub fn surface_ices(&self) -> &[(SubstanceId, f64)] {
        &self.surface_ices
    }

    /// The cloud fraction, 0 to 1: the atmosphere's, the area mix's (P14.T51.b; the state's share,
    /// [`SurfaceState::cloud_fraction`], times its condensable's availability), with which its Bond
    /// albedo was solved; zero before the second stage.
    #[must_use]
    pub const fn cloud_fraction(&self) -> f64 {
        self.cloud_fraction
    }
}

/// What a body's heat flow reads of it ([`heat_at`]; P14.T24.b), as plain values: the part of
/// [`FiguresInputs`] its interior, regime and heat take, which P14.T56.d's tides also read for a
/// moon's non-tidal heat flow ([`non_tidal_heat_flow`]) before its tide is known.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeatInputs {
    /// Its mass at the time.
    pub(crate) mass: EarthMasses,
    /// Its mean radius, the bulk section's.
    pub(crate) radius: Metres,
    /// Its mean surface gravity, the bulk section's.
    pub(crate) surface_gravity: MetresPerSecondSquared,
    /// Its class, the bulk section's.
    pub(crate) class: PlanetClass,
    /// Its mass fractions, the bulk section's.
    pub(crate) fractions: MassFractions,
    /// The water of its volatile inventory ([`FiguresInputs::inventory_water`]).
    pub(crate) inventory_water: Kilograms,
    /// Its host's \[Fe/H\].
    pub(crate) fe_h: Dex,
    /// The tidal heat raised in it over its surface ([`FiguresInputs::tidal_heat_flow`]).
    pub(crate) tidal_heat_flow: WattsPerSquareMetre,
    /// Its ocean under the surface, as its rule computes it.
    pub(crate) subsurface_ocean: Subsurface,
}

impl FiguresInputs<'_> {
    /// What its heat flow reads ([`HeatInputs`]).
    #[must_use]
    fn heat(&self) -> HeatInputs {
        HeatInputs {
            mass: self.mass,
            radius: self.radius,
            surface_gravity: self.surface_gravity,
            class: self.class,
            fractions: self.fractions,
            inventory_water: self.inventory_water,
            fe_h: self.fe_h,
            tidal_heat_flow: self.tidal_heat_flow,
            subsurface_ocean: self.subsurface_ocean,
        }
    }
}

/// The area shares `shares` scaled down, in proportion, where they sum to more than the whole
/// surface ([`GlobalFigures::with_surface_under`]), and as they are otherwise.
#[must_use]
fn within_the_surface(mut shares: Vec<(SubstanceId, f64)>) -> Vec<(SubstanceId, f64)> {
    let total = shares.iter().fold(0.0, |sum, &(_, f)| sum + f);
    if total > 1.0 {
        for share in &mut shares {
            share.1 /= total;
        }
    }
    shares
}

/// How a body's interior loses its heat, with its rock's radius ([`Interior::of`]); `None` for a
/// class with no solid surface.
#[must_use]
fn interior_of(inputs: &HeatInputs) -> Option<(Interior, Metres)> {
    let kilograms = Kilograms::from(inputs.mass).value();
    let rock_core = rock_core_radius(kilograms * (1.0 - inputs.fractions.water()));
    Interior::of(inputs.class, rock_core).map(|interior| (interior, rock_core))
}

/// The non-tidal heat flow `F_nt` of the body `inputs` describe at the system age `age` under the
/// surface state `state`, W m⁻²: [`GlobalFigures::derive`]'s, the radiogenic term with its
/// interior's residual, the regime that residual reads decided on `inputs` as given (P14.T24.b).
/// `None` for a body in the gas-envelope state or of a class with no solid surface, which takes no
/// solid heat flow, as [`GlobalFigures::derive`] gives none.
///
/// P14.T56.d's tidal branch reads it for a moon with no tide and no ocean given, before its branch
/// decides them (decision-p14-moons §F-ocean F3): only a mobile lid's Urey ratio parts from every
/// other regime's ([`interior::convecting_heat_flow`]), and the tide and the ocean move the regime
/// only to a heat pipe or an ice shell, which take the stagnant lid's.
#[must_use]
pub(crate) fn non_tidal_heat_flow(
    inputs: &HeatInputs,
    age: Years,
    state: SurfaceState,
) -> Option<WattsPerSquareMetre> {
    if matches!(state, SurfaceState::GasEnvelope) {
        return None;
    }
    interior_of(inputs).map(|interior| heat_at(inputs, interior, age, state).non_tidal)
}

/// A body's heat at a system age ([`heat_at`]): its radiogenic and non-tidal heat flows, its heat
/// flow, its regime, its iron's share of the solids, its tide and its water.
#[derive(Debug, Clone, Copy, PartialEq)]
struct HeatAt {
    radiogenic: WattsPerSquareMetre,
    non_tidal: WattsPerSquareMetre,
    heat_flow: WattsPerSquareMetre,
    regime: TectonicRegime,
    iron_share: f64,
    tidal: WattsPerSquareMetre,
    water: Kilograms,
}

/// The heat of the body `inputs` describe, whose interior and rock radius are `interior`, at the
/// system age `age` under the surface state `state` ([`GlobalFigures::derive`]'s, at its own age
/// and state): the regime from the radiogenic term then, and the heat flow from it with the
/// interior's residual then and the tide as given.
#[must_use]
fn heat_at(
    inputs: &HeatInputs,
    (interior, rock_core): (Interior, Metres),
    age: Years,
    state: SurfaceState,
) -> HeatAt {
    let HeatInputs {
        mass,
        radius,
        surface_gravity,
        fractions,
        inventory_water,
        fe_h,
        ..
    } = *inputs;
    let kilograms = Kilograms::from(mass).value();
    let radiogenic = radiogenic_heat_flow(
        Kilograms::new(kilograms * fractions.rock()),
        radius,
        fe_h,
        age,
    );
    let solids = fractions.iron() + fractions.rock();
    let iron_share = if solids > 0.0 {
        fractions.iron() / solids
    } else {
        0.0
    };
    // Clamped by comparison, not `max`, which may return either zero at a tie of signs.
    let tide = inputs.tidal_heat_flow.value();
    let tidal = WattsPerSquareMetre::new(if tide > 0.0 { tide } else { 0.0 });
    let r = radius.value();
    let (held, bulk_water) = (inventory_water.value(), fractions.water() * kilograms);
    let water = if held > bulk_water { held } else { bulk_water };
    let water_depth = if r > 0.0 {
        water / (interior::REGIME_WATER_DENSITY.value() * 4.0 * core::f64::consts::PI * r * r)
    } else {
        0.0
    };
    let ocean_shell = match inputs.subsurface_ocean {
        Subsurface::Ocean(ocean) => Some(ocean.shell_thickness()),
        Subsurface::NoOcean | Subsurface::NotComputed => None,
    };
    let regime = tectonic_regime(&interior::RegimeInputs {
        interior,
        mass,
        radiogenic,
        tidal,
        state,
        water_depth: Metres::new(water_depth),
        surface_gravity,
        ocean_shell,
    });
    let non_tidal = match interior {
        Interior::Conducting => WattsPerSquareMetre::new(
            radiogenic.value()
                * conductive_lag(rock_core, age, interior::ROCK_THERMAL_DIFFUSIVITY_M2_PER_S),
        ),
        Interior::Convecting => convecting_heat_flow(
            radiogenic_heat_flow(Kilograms::new(kilograms * solids), radius, fe_h, age),
            iron_share,
            regime,
        ),
    };
    HeatAt {
        radiogenic,
        non_tidal,
        heat_flow: WattsPerSquareMetre::new(non_tidal.value() + tidal.value()),
        regime,
        iron_share,
        tidal,
        water: Kilograms::new(water),
    }
}

/// The present outgassing O the body `inputs` describe had at the system age `age`.
///
/// O is relative to Earth's now, [`outgassing`] of the heat flow then (P14.T48.b;
/// science-p14-t24b-laws §3.3), by [`GlobalFigures::derive`]'s pure functions at that age: the
/// radiogenic term then, the interior's residual then, and the regime then of a surface inside the
/// water window ([`SurfaceState::Temperate`]), with the tide, the water and the ocean's shell as
/// `inputs` give them, today's. `inputs.age` is not read. At the inputs' own age, on a temperate
/// surface, it is [`GlobalFigures::outgassing`] bit for bit. Zero for a body with no solid surface,
/// and for an age that is not finite and non-negative.
///
/// It is the outgassing a [`VolatileHistoryInputs`] reads at each past age.
///
/// # Examples
///
/// Earth, as [`GlobalFigures::derive`]'s example derives it, outgassed about twice today's rate
/// 3.5 Gyr ago, when its rock made more heat:
///
/// ```
/// # use hyperion_sim::math;
/// # use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// # use hyperion_sim::planetary::belts::BeltComposition;
/// # use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
/// # use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// # use hyperion_sim::planetary::hooks::figures::{
/// #     Continents, CraterInputs, FiguresInputs, GlobalFigures, HostOrbit, HostOrbiter,
/// #     ImpactorOrbit, ImpactorSources, SOLAR_MAIN_BELT_MASS, SourceAnnulus, SourceBelt,
/// #     Subsurface,
/// # };
/// # use hyperion_sim::planetary::record::BeltKind;
/// # use hyperion_sim::stellar::Composition;
/// # use hyperion_sim::stellar::draws::UnitUniform;
/// # use hyperion_sim::time::UniverseTime;
/// # use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// # use hyperion_sim::units::{
/// #     Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres, Radians,
/// #     Seconds, SolarLuminosities, SolarMasses, SolarRadii, WattsPerSquareMetre,
/// # };
/// use hyperion_sim::planetary::hooks::figures::past_outgassing;
/// use hyperion_sim::units::Years;
/// # let host = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
/// # let disc = disc::derive(&host, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
/// # let disc = disc.profile().expect("a median disc exists");
/// # let a = Metres::new(METRES_PER_AU);
/// # let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
/// # let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
/// # let orbit = KeplerElements::from_semi_major_axis(a, mu, Eccentricity::new(0.0167)?, orientation, Radians::ZERO)?;
/// # let earth = PlacedBody::new(EarthMasses::new(1.0), orbit, a, UnitUniform::new(0.384).expect("inside (0, 1)"))?;
/// # let sun = [HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?];
/// # let hosts = BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[])?;
/// # let age = Years::new(4.57e9);
/// # let derived = derive_body(&earth, &hosts, disc, age, UniverseTime::EPOCH)?;
/// # let au = |x: f64| Metres::new(x * METRES_PER_AU);
/// # let belt = SourceBelt {
/// #     kind: BeltKind::Asteroid,
/// #     composition: BeltComposition::Rocky,
/// #     main: SourceAnnulus {
/// #         inner_edge: au(5.2029 * math::powf(0.25, 2.0 / 3.0)),
/// #         outer_edge: au(5.2029 * math::powf(0.5, 2.0 / 3.0)),
/// #     },
/// #     scattered: None,
/// #     main_mass: SOLAR_MAIN_BELT_MASS,
/// #     scattered_mass: EarthMasses::ZERO,
/// # };
/// # let sources = ImpactorSources::new(mu, vec![belt], [(EarthMasses::new(317.8), au(5.2029))])?;
/// # let inputs = FiguresInputs {
/// #     atmosphere: derived.atmosphere(),
/// #     mass: derived.mass(),
/// #     radius: Metres::from(derived.radius()),
/// #     surface_gravity: derived.surface_gravity(),
/// #     class: derived.class(),
/// #     fractions: derived.fractions(),
/// #     inventory_water: derived.inventory().water(),
/// #     rotation_period: Seconds::new(86_164.1),
/// #     fe_h: Dex::new(0.0),
/// #     age,
/// #     tidal_heat_flow: WattsPerSquareMetre::new(0.0),
/// #     subsurface_ocean: Subsurface::NotComputed,
/// #     craters: CraterInputs {
/// #         orbit: ImpactorOrbit::Host(HostOrbiter {
/// #             orbit: HostOrbit { semi_major_axis: a, eccentricity: 0.0167, inclination: Radians::ZERO },
/// #             mass: EarthMasses::new(1.0),
/// #             belt: None,
/// #         }),
/// #         density: derived.density(),
/// #         sources: &sources,
/// #         molten_until: Some(Years::new(3.2e7)),
/// #         last_giant_impact: Some(Years::ZERO),
/// #     },
/// #     continents: Continents::Given(0.405),
/// # };
/// let figures = GlobalFigures::derive(&inputs)?.expect("an Earth has a surface");
/// // Today's, at its own age on its temperate surface.
/// assert_eq!(past_outgassing(&inputs, age).to_bits(), figures.outgassing().to_bits());
/// // Higher in its youth.
/// let young = past_outgassing(&inputs, Years::new(1.07e9));
/// assert!(young > 1.5 * figures.outgassing(), "{young}");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn past_outgassing(inputs: &FiguresInputs<'_>, age: Years) -> f64 {
    if matches!(inputs.atmosphere.state(), SurfaceState::GasEnvelope) || !non_negative(age.value())
    {
        return 0.0;
    }
    let heat = inputs.heat();
    interior_of(&heat).map_or(0.0, |interior| {
        outgassing(heat_at(&heat, interior, age, SurfaceState::Temperate).heat_flow)
    })
}

/// The surface age, the impactors and the crater contract of the body `inputs` describe, holding
/// `water` and of heat flow `heat_flow` ([`GlobalFigures::derive`]'s, whose age is checked).
fn crater_contract(
    inputs: &FiguresInputs<'_>,
    water: Kilograms,
    heat_flow: WattsPerSquareMetre,
) -> Result<(Years, Impactors, CraterParams), DeriveFiguresError> {
    let (air, crater_inputs, gravity) = (inputs.atmosphere, inputs.craters, inputs.surface_gravity);
    let impactors = impactors(&ImpactorBody {
        orbit: crater_inputs.orbit,
        radius: inputs.radius,
        density: crater_inputs.density,
        surface_gravity: gravity,
        surface_temperature: air.surface_temperature(),
        water,
        sources: crater_inputs.sources,
    })
    .map_err(DeriveFiguresError::Impactors)?;
    let surface_age = surface_age::surface_age(&surface_age::SurfaceAgeInputs {
        age: inputs.age,
        molten_until: crater_inputs.molten_until,
        last_giant_impact: crater_inputs.last_giant_impact,
        heat_flow,
        crust: impactors.crust(),
    });
    let gyr = |years: Years| Gigayears::new(years.value() / YEARS_PER_GIGAYEAR);
    let system_age = gyr(inputs.age);
    let n_1km = craters::lunar_chronology(gyr(surface_age), system_age) * impactors.rate();
    // A column too thin to be a normal number screens nothing (its cutoff underflows to 0 km,
    // which the crater contract refuses): the subnormal residue the partition can leave over an
    // airless body, some 10⁻³²³ Pa.
    let screening = match air.surface_pressure() {
        Some(pressure) if pressure.value() / gravity.value() >= f64::MIN_POSITIVE => {
            Screening::Atmosphere {
                column_mass: KilogramsPerSquareMetre::new(pressure.value() / gravity.value()),
            }
        }
        Some(_) | None => Screening::None,
    };
    let craters = CraterParams::new(CraterParamsParts {
        n_1km,
        screening,
        gravity,
        k_target: impactors.k_target(),
        impact_velocity: Some(impactors.velocity()),
        projectile_density: impactors.projectile_density(),
        target: impactors.target(),
        system_age,
    })
    .map_err(DeriveFiguresError::Craters)?;
    Ok((surface_age, impactors, craters))
}

/// The radius of a sphere of `kilograms` of rock at [`interior::ROCK_CORE_DENSITY`]: an icy
/// body's non-water mass, whose conductive lag it sets.
#[must_use]
fn rock_core_radius(kilograms: f64) -> Metres {
    let density = interior::ROCK_CORE_DENSITY.value();
    Metres::new(math::cbrt(
        3.0 * kilograms / (4.0 * core::f64::consts::PI * density),
    ))
}

/// The figures of a body of the Solar System table (`derive::solar`), derived about the present
/// Sun: its own mass, radius, gravity, class, fractions and water at the Sun's age and \[Fe/H\],
/// turning in Earth's sidereal day, with no tide and no rule for an ocean under it; `None` for a
/// gas envelope. Its heat flow reads no rotation, and its crater contract is a planet's on a
/// circular orbit at 1 au under the Solar System's sources, with no magma ocean: tests of the
/// contract give each body its own. Its continents are Earth's measured fraction, which only the
/// table's one mobile lid, Earth's, reads.
#[cfg(test)]
#[must_use]
pub(crate) fn table_figures(body: &crate::planetary::derive::DerivedBody) -> Option<GlobalFigures> {
    let orbit = ImpactorOrbit::Host(HostOrbiter {
        orbit: HostOrbit {
            semi_major_axis: Metres::new(METRES_PER_AU),
            eccentricity: 0.0,
            inclination: Radians::ZERO,
        },
        mass: body.mass(),
        belt: None,
    });
    GlobalFigures::derive(&FiguresInputs {
        atmosphere: body.atmosphere(),
        mass: body.mass(),
        radius: Metres::from(body.radius()),
        surface_gravity: body.surface_gravity(),
        class: body.class(),
        fractions: body.fractions(),
        inventory_water: body.inventory().water(),
        rotation_period: Seconds::new(86_164.1),
        fe_h: Dex::new(0.0),
        age: crate::planetary::derive::solar::SOLAR_AGE,
        tidal_heat_flow: WattsPerSquareMetre::new(0.0),
        subsurface_ocean: Subsurface::NotComputed,
        craters: CraterInputs {
            orbit,
            density: body.density(),
            sources: tests::solar_sources(),
            molten_until: None,
            last_giant_impact: None,
        },
        continents: Continents::Given(relief::EARTH_CONTINENTAL_FRACTION),
    })
    .expect("the table's bodies are in range")
}
