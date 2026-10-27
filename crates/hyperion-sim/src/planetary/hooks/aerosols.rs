//! The aerosol and absorber inventory of a surface's air (plan 14, P14.T24.c; rendering plan R08's
//! Design notes 6 and 13): its condensate decks, its hazes and dust as modes of particles, and the
//! absorbing gases it lists apart from P14.T24.a's well-mixed fractions.
//!
//! # What it holds ([`AerosolInventory`])
//!
//! - **Modes** ([`AerosolMode`]): a material (a registry row), its phase (solid or liquid, never a
//!   vapour), its [`ShapeClass`], the deck it belongs to by the deck's condensate, its size (a
//!   log-normal or gamma distribution of the volume-to-area radius r = 3V ÷ 4A, or a fractal
//!   aggregate's monomers), its column mass, its τ(550) (in-cloud, over its coverage) and its
//!   vertical profile, a base and a top pressure between which its mixing ratio goes as
//!   (p ÷ `p_base`)^f, f its deck's `f_sed` or 0 for a well-mixed layer, its coverage and the
//!   temperature its optics were taken at.
//! - **Decks** ([`AerosolDeck`]): each condensate's deck, its base, top, vapour below and above
//!   (the cold trap's, which R08 reads for the vapour's column there), its `f_sed` and its τ(550),
//!   the sum of its modes'.
//! - **Absorbers** ([`Absorber`]): a registry gas row outside T24.a's well-mixed gases, with its
//!   column (molecules m⁻²) and its layer, a slab between two pressures or, for ozone, a profile of
//!   (pressure, relative mixing ratio) nodes.
//!
//! # The decks
//!
//! P14.T52.c's loop ([`Condensation::derive`]) over the registry's laws aloft along P14.T24.e's
//! profile, from T24.a's published fractions at the ground (the surface humidity is T24.a's, 0.77
//! of saturation for water and 0.5 for methane, never re-derived here), covering T51.b's cloud
//! fraction. Each deck takes Ackerman and Marley's (2001, ApJ 556, 872) two parameters by the rule
//! [`sedimentation_for_supply`], the serve gate's must-fix (decision-bump-plan-after-22, V3.9;
//! brief-notes-24, "From p14-condense's T52.c interface"):
//!
//! - **K**, the cloud layer's eddy diffusion, is [`CLOUD_EDDY_DIFFUSION`], AM01's `K_min`, fixed
//!   before the fit. At a given `f_sed` it sets only the particles' size (AM01's balance with L =
//!   H gives a column mass independent of K); since `f_sed` = v H ÷ K here, a change of K moves the
//!   columns too and is a refit of v₁.
//! - **`f_sed`** is AM01's own definition, the mass-weighted sedimentation speed over w* = K ÷ L
//!   (their eq. 10), with L = H at the deck's base and the speed rising with the condensate's
//!   supply, [`SEDIMENTATION_SPEED_AT_UNIT_SUPPLY`] times a power
//!   ([`SEDIMENTATION_SUPPLY_EXPONENT`]) of its vapour column above its base in kg m⁻²: a sparse
//!   vapour (Mars's water) makes a lofted haze, an abundant one (Earth's water, the ice giants'
//!   methane) precipitates. The sign has its literature: `f_sed` "should generally increase with
//!   increasing depth of the cloud base … due to higher abundances of condensate vapor at depth
//!   aiding growth of larger cloud particles" (Gao, Marley and Ackerman 2018, ApJ 855, 86); AM01's
//!   own `f_rain` is under 1 in stratocumulus and 2–6 through trade cumulus; Virga's water clouds
//!   are "up to two orders of magnitude" thicker than microphysical ones at the cloud base, in the
//!   homogeneously nucleated case (Mang et al. 2022, ApJ 927, 184); and the ice giants' methane
//!   particles "may grow to a size of ∼5 mm in as little as ∼100s … and almost immediately
//!   precipitate" (Irwin et al. 2022, JGR Planets 127, e2022JE007189, after Carlson et al. 1988). The
//!   form's exponent and its constant are this plan's fit, through the loop, on six measured decks
//!   (Mars, Earth, Jupiter, Saturn, Uranus and Neptune; the constants' docs list each), as
//!   P14.T24.g's `σ_d` is fitted on Venus's.
//! - **The speed is an effective one**, AM01's `f_sed` w*, not the served particles' fall speed:
//!   their eq. 13 fits the fall law's slope α at `r_w`, where the fall is Stokes's, and carries it to
//!   `r_g`, 4–20 `r_w` away at these `f_sed`. At the fitted constants the served mode itself
//!   falls at about 0.37 m s⁻¹ on Earth (against an effective 0.85), 1.16 on Uranus (6.4) and 0.47
//!   on Titan (6.2, more than any stable methane drop's about 1.6 m s⁻¹; Lorenz 1993), by the loop's
//!   own fall law (decision-bump-plan-after-22, "The decks' `f_sed`, K and humidity", G1, whose
//!   replica gives these; the lane's refit, 2026-10-22).
//!
//! The loop's `r_eff` is AM01's settling radius, the volume-equivalent sphere's `r_V`. Each mode is
//! converted on the way in to the volume-to-area radius, r = Ψ `r_V` ([`ShapeClass::sphericity`];
//! science-r08-mmf.md §2–3), and its τ(550) taken again from P14.T49.e's mass extinction at r.
//!
//! # The other rules
//!
//! - **Ozone** ([`ozone_oxygen_factor`], [`ozone_star_factor`]): `N_⊕` F(r) U(star) on the AFGL
//!   template ([`AFGL_US_STANDARD_OZONE`]), each node moved to the pressure at which the O₂ column
//!   above it equals the node's on Earth, r the O₂ column over Earth's. No O₂ or no star, no ozone.
//! - **Methane haze** ([`haze_optical_depth`]): a fractal aggregate of `tholin` with Titan's
//!   monomers above an index of CH₄ over its floor and the CO₂ ([`haze_index`]), calibrated on
//!   Titan's column.
//! - **Dust** ([`dust_optical_depth`]): `mars_dust` on a dry, thin and liftable ground, calibrated on
//!   Mars's background column.
//! - **Sulphur** (P14.T24.g, [`sulphur`](super::sulphur)): a runaway world's four-mode acid deck,
//!   whose base the loop finds from its SO₂, a temperate or snowball world's sulphate layer, and SO₂
//!   as an absorber.
//!
//! # What is not modelled
//!
//! - A bulk condensable (a Mars's CO₂, a Triton's N₂) sets its own profile and is labelled
//!   ([`NotFormed::BulkGas`]); its clouds (Mars's mesospheric CO₂ ice) are not formed.
//! - The decks' coverage is the snapshot path's cloud fraction, which never reads them.
//! - The ozone layer's shift with O₂ follows the fixed overhead column further than Segura et al.
//!   2003's profiles (their Fig. 2b: a peak at 18–20 km at 0.1 PAL and 12–15 km at 10⁻² PAL, against
//!   22 km at 1 PAL); F's shape about other stars, and an M dwarf's activity, are held only in U.
//! - The haze's suppression by a host's far ultraviolet (Arney et al. 2017), and any wind but
//!   Mars's for the dust, whose rule reads no rotation.
//!
//! # Not on the record yet
//!
//! This module is pure: nothing on the record calls it at generator version 23
//! (decision-bump-plan-after-22 §1.3). P14.T54.b's wiring (`system/air.rs`, P14.T54.b.b) builds
//! the record's inventory from the published surface behind its hold, `AIR_SECTIONS_VERSION`,
//! whose held arm calls nothing here, so that the record and the wire carry it from the 23 → 24
//! bump.

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::chem::condense::{
    BuildModeError, BuildSedimentationError, Condensation, Deck, DeriveCondensationError, Mode,
    ModeProfile, NotFormed, OpticalDepth, OpticalDepthBasis, ResolveOpticalDepthError,
    Sedimentation, SizeModeError,
};
use crate::planetary::chem::quench::{EddyDiffusion, MixingColumn};
use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::hooks::structure::RadiativeConvective;
use crate::planetary::hooks::sulphur::{
    DeriveSulphurError, RunawayDeck, Sulphur, SulphurCycle, SulphurInputs,
};
use crate::substance::aloft::law_aloft;
use crate::substance::{
    CondensedPhase, ExtinctionBasis, MOLAR_GAS_CONSTANT, SizeDistribution, SubstanceId,
    SubstanceKind,
};
use crate::units::{
    Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecond,
    MetresPerSecondSquared, Pascals,
};

/// The cloud layer's eddy diffusion, m² s⁻¹: 10, Ackerman and Marley's `K_min`, 10⁵ cm² s⁻¹ (2001,
/// §2: "an eddy diffusion coefficient no less than a prescribed minimum value (`K_min` = 10⁵ cm²
/// s⁻¹ in our baseline model), which represents residual turbulence due to breaking buoyancy waves
/// (Lindzen 1981)"; MacDonald et al. 2018, ApJ 858, 69, keep the same minimum for cold giants).
/// Fixed before the fit (a change is a refit); with [`SEDIMENTATION_SPEED_AT_UNIT_SUPPLY`] it keeps
/// the Solar System anchors' `r_eff` at 2–100 µm, against millimetres at AM01's free-convection K
/// of 10³–2 × 10⁴ m² s⁻¹ (their Jupiter's 2 × 10⁸ cm² s⁻¹).
pub const CLOUD_EDDY_DIFFUSION: EddyDiffusion = EddyDiffusion::from_square_metres_per_second(10.0);

/// The effective sedimentation speed (AM01's `f_sed` w*, not a particle's fall speed; the
/// [module](self)'s documentation) of a deck whose vapour column above its base is 1 kg m⁻², m s⁻¹:
/// v₁ of [`sedimentation_for_supply`]. `Adopted`: this plan's fit, with
/// [`SEDIMENTATION_SUPPLY_EXPONENT`], through P14.T52.c's loop (its exact step) at
/// [`CLOUD_EDDY_DIFFUSION`] and [`DECK_WIDTH`], on six measured decks given by hand, equal weights
/// in log₁₀(τ ÷ centre) (decision-bump-plan-after-22, "The decks' `f_sed`, K and humidity", G1–G3;
/// the anchors' states and sources are in this module's tests, which hold the fit):
///
/// | Deck | `M_vap` (kg m⁻²) | v (m s⁻¹) | `f_sed` | τ(550) | Observed range (centre) | Residual (dex) |
/// | --- | --- | --- | --- | --- | --- | --- |
/// | Mars H₂O | 0.00909 | 0.0050 | 5.1 | 1.01 | 0.2–2 (0.6) | +0.23 |
/// | Earth H₂O | 73.7 | 0.85 | 708 | 2.61 | 2.5–10 (3.7) | −0.15 |
/// | Jupiter NH₃ | 6.08 | 0.21 | 441 | 2.58 | 0.8–5 (1.5) | +0.24 |
/// | Saturn NH₃ | 30.2 | 0.51 | 2,674 | 0.87 | 0.5–5 (1.5) | −0.24 |
/// | Uranus CH₄ | 2,549 | 6.4 | 18,320 | 4.1 | 1–5 (2.5) | +0.22 |
/// | Neptune CH₄ | 2,936 | 7.0 | 15,860 | 5.8 | 0.5–4 (1.5) | +0.59 |
///
/// RMS 0.31 dex. Neptune's deck lies above its range, below R08's cloud-deck split of 10 and within
/// 3 times its range's top: G3's record for P14.T52.d, not a serve-gate item. No pair puts all six
/// inside their ranges (the equal-weight best, v₁ 0.087 and 0.585, leaves Earth at 1.8); this one
/// is the least-squares point with Mars, Earth, Jupiter and Saturn inside by a 5% margin before
/// rounding (Earth's 2.59 after it, 3.6% inside; 2.61 on the loop's final mass integral, as
/// tabled) and the ice giants under G3, accepted by main (2026-10-22). Titan, a check: 1.4 in the
/// cloud, inside 0.3–30.
pub const SEDIMENTATION_SPEED_AT_UNIT_SUPPLY: MetresPerSecond = MetresPerSecond::new(0.0735);

/// The exponent of the effective sedimentation speed in the condensate's supply, its vapour column
/// above its base: 0.57, fitted with [`SEDIMENTATION_SPEED_AT_UNIT_SUPPLY`] (its table). `Adopted`.
pub const SEDIMENTATION_SUPPLY_EXPONENT: f64 = 0.57;

/// The least `f_sed` a deck takes, 0.01: below Gao, Marley and Ackerman 2018's lowest best fit,
/// 0.025 (ApJ 855, 86), the floor decision-bump-plan-after-22 ("The decks' `f_sed`, K and humidity",
/// G5) sets so that a trace vapour's `f_sed`, tending to 0 with its supply, keeps its mode above
/// 0.01 µm.
pub const LEAST_SEDIMENTATION_EFFICIENCY: f64 = 0.01;

/// The width `σ_g` of every deck's log-normal: Ackerman and Marley's baseline, 2.
pub const DECK_WIDTH: f64 = 2.0;

/// A deck's Ackerman–Marley parameters by its supply (the [module](self)'s documentation): `f_sed` =
/// v H ÷ K, with v = [`SEDIMENTATION_SPEED_AT_UNIT_SUPPLY`] (`M_vap` ÷ 1 kg m⁻²)^[`SEDIMENTATION_SUPPLY_EXPONENT`]
/// the mass-weighted sedimentation speed, `M_vap` = ε x `p_base` ÷ g its vapour column above its
/// base (ε its condensate's molar mass per limiting vapour over the gas's), H = R `T_base` ÷ (μ g)
/// the scale height there and K the column's eddy diffusion; at least
/// [`LEAST_SEDIMENTATION_EFFICIENCY`], at `σ_g` [`DECK_WIDTH`].
///
/// # Errors
///
/// [`BuildSedimentationError::Efficiency`] where the deck's inputs give no finite `f_sed`: a
/// condensate or a gas of no molar mass, a column of no or not finite gravity or eddy diffusion.
/// The floor applies to a finite `f_sed` only.
pub fn sedimentation_for_supply(
    deck: &Deck,
    column: &MixingColumn,
) -> Result<Sedimentation, BuildSedimentationError> {
    let mu = column.molar_mass_g_per_mol;
    let g = column.gravity.value();
    let condensate = deck.condensate();
    let per_vapour = law_aloft(condensate).map_or(1.0, |law| f64::from(law.limiting().count()));
    let epsilon = condensate
        .substance()
        .molar_mass_g_per_mol()
        .map_or(f64::NAN, |m| m / per_vapour / mu);
    let supply = epsilon * deck.vapour_below() * deck.base().value() / g;
    let speed = if supply.is_finite() && supply >= 0.0 {
        SEDIMENTATION_SPEED_AT_UNIT_SUPPLY.value()
            * math::powf(supply, SEDIMENTATION_SUPPLY_EXPONENT)
    } else {
        f64::NAN
    };
    let scale_height =
        MOLAR_GAS_CONSTANT * deck.base_temperature().value() / (mu / GRAMS_PER_KILOGRAM * g);
    let efficiency = speed * scale_height / column.eddy_diffusion.square_metres_per_second();
    // A finite f_sed takes the floor; anything else is refused by `Sedimentation::new`.
    let efficiency = if efficiency.is_finite() {
        efficiency.max(LEAST_SEDIMENTATION_EFFICIENCY)
    } else {
        efficiency
    };
    Sedimentation::new(efficiency, DECK_WIDTH)
}

/// Grams per kilogram.
const GRAMS_PER_KILOGRAM: f64 = 1_000.0;

/// Avogadro's number, mol⁻¹: R ÷ k, both exact in the 2019 SI.
const AVOGADRO: f64 = MOLAR_GAS_CONSTANT / crate::units::consts::BOLTZMANN_CONSTANT;

/// One Dobson unit, molecules m⁻²: 2.687 × 10²⁰ (WMO/UNEP, _Twenty Questions and Answers About the
/// Ozone Layer: 2018 Update_, glossary), rendering plan R08's `DOBSON_UNIT_PER_M2`.
pub const DOBSON_UNIT: f64 = 2.687e20;

/// Earth's ozone column `N_⊕`, molecules m⁻²: 300 DU, its global average (WMO/UNEP 2018, Q3), R08's
/// `OZONE_COLUMN_DU`.
pub const EARTH_OZONE_COLUMN: f64 = 300.0 * DOBSON_UNIT;

/// The AFGL U.S. Standard ozone, as (pressure, mb; mixing ratio, ppmv) at its 50 levels, 0–120 km:
/// Anderson, Clough, Kneizys, Chetwynd and Shettle, _AFGL Atmospheric Constituent Profiles
/// (0–120 km)_, AFGL-TR-86-0110 (Air Force Geophysics Laboratory, 15 May 1986), Table 1f, as
/// rendering plan R08.T6.h holds it (`view/atmosphere/earth.ts`'s `AFGL_US_STANDARD_OZONE_MB_PPMV`,
/// its reading and checks there: the column from the report's own densities is 345.7 DU to 120 km).
/// It follows the U.S. Standard Atmosphere 1976's mid-latitude ozone model (Krueger and Minzner
/// 1976, J. Geophys. Res. 81, 4477) to 2% in number density from 2 to 50 km. The template of
/// P14.T24.c's ozone rule; its mixing ratios are relative, the column scaling them.
pub const AFGL_US_STANDARD_OZONE: [(f64, f64); 50] = [
    (1_013.25, 2.66e-2),
    (898.8, 2.93e-2),
    (795.0, 3.24e-2),
    (701.2, 3.32e-2),
    (616.6, 3.39e-2),
    (540.5, 3.77e-2),
    (472.2, 4.11e-2),
    (411.1, 5.01e-2),
    (356.5, 5.97e-2),
    (308.0, 9.17e-2),
    (265.0, 1.31e-1),
    (227.0, 2.15e-1),
    (194.0, 3.1e-1),
    (165.8, 3.85e-1),
    (141.7, 5.03e-1),
    (121.1, 6.51e-1),
    (103.5, 8.7e-1),
    (88.5, 1.19),
    (75.65, 1.59),
    (64.67, 2.03),
    (55.29, 2.58),
    (47.29, 3.03),
    (40.47, 3.65),
    (34.67, 4.17),
    (29.72, 4.63),
    (25.49, 5.12),
    (17.43, 5.8),
    (11.97, 6.55),
    (8.01, 7.37),
    (5.746, 7.84),
    (4.15, 7.8),
    (2.871, 7.3),
    (2.06, 6.2),
    (1.491, 5.25),
    (1.09, 4.1),
    (7.978e-1, 3.1),
    (4.25e-1, 1.8),
    (2.19e-1, 1.1),
    (1.09e-1, 7.0e-1),
    (5.22e-2, 3.0e-1),
    (2.4e-2, 2.5e-1),
    (1.05e-2, 3.0e-1),
    (4.46e-3, 5.0e-1),
    (1.84e-3, 7.0e-1),
    (7.6e-4, 7.0e-1),
    (3.2e-4, 4.0e-1),
    (1.45e-4, 2.0e-1),
    (7.1e-5, 5.0e-2),
    (4.01e-5, 5.0e-3),
    (2.54e-5, 5.0e-4),
];

/// Pascals per millibar.
const PASCALS_PER_MILLIBAR: f64 = 100.0;

/// Earth's O₂ molecules above each pascal of its own pressure, m⁻² Pa⁻¹: `x_O₂` ÷ (m̄ g) at the U.S.
/// Standard Atmosphere 1976's 0.209476 O₂ by volume, 28.9644 g mol⁻¹ and 9.80665 m s⁻² (NOAA-S/T
/// 76-1562, Table 3 and its constants), by which an O₂ level is a multiple of its present level
/// (PAL), and the ozone template's nodes move to equal overhead O₂ columns (Chapman 1930).
const EARTH_OXYGEN_PER_PASCAL: f64 = 0.209_476 * AVOGADRO / (28.964_4e-3 * 9.806_65);

/// Earth's O₂ column at its 1,013.25 mb, molecules m⁻²: 1 PAL.
const PRESENT_OXYGEN_COLUMN: f64 = EARTH_OXYGEN_PER_PASCAL * 101_325.0;

/// The ozone column over its value at 1 PAL against the O₂ level (PAL), F of P14.T24.c's rule:
/// Segura et al. 2003's photochemical columns about the Sun (Astrobiology 3, 689, Table 1: 8.36 ×
/// 10¹⁸ cm⁻² at 1 PAL; 7.16 × 10¹⁸ at 0.1, 3.33 × 10¹⁸ at 10⁻², 6.79 × 10¹⁷ at 10⁻³, 3.26 × 10¹⁶ at
/// 10⁻⁴ and 2.24 × 10¹⁵ at 10⁻⁵ PAL; "the total column depth did not decrease significantly until
/// PO₂ dropped below 10⁻² PAL"). Above 1 PAL F is held at 1 (the last node's), a saturating
/// extrapolation, finite and monotonic to any O₂: the models disagree there (Kasting and Donahue
/// 1980 rising to about 1.35 at 1.5 PAL, Levine et al. 1979 falling to 0.74, both via Cooke et al.
/// 2022, R. Soc. Open Sci. 9, 211165, Fig. 6), all within about ±40% to 1.5 PAL.
pub const OZONE_BY_OXYGEN: [(f64, f64); 7] = [
    (1e-5, 2.24e15 / 8.36e18),
    (1e-4, 3.26e16 / 8.36e18),
    (1e-3, 6.79e17 / 8.36e18),
    (1e-2, 3.33e18 / 8.36e18),
    (0.1, 7.16e18 / 8.36e18),
    (1.0, 1.0),
    (1e12, 1.0),
];

/// The ozone column at 1 PAL about a host of effective temperature `T_eff` (K) over the Sun's, U
/// of P14.T24.c's rule, linear in `T_eff` between the nodes and held at their ends: Segura et al.
/// 2003's F2V σ Boo (6,700 K, 1.56 × 10¹⁹ cm⁻², 1.87) and K2V ε Eri (5,180 K, 6.64 × 10¹⁸, 0.79),
/// and Segura et al. 2005's active M4.5V AD Leo (3,400 K, 4.4 × 10¹⁸, 0.53; Astrobiology 5, 706,
/// Table 2), each over Segura et al. 2003's Sun, 8.36 × 10¹⁸ (Segura et al. 2005's own Sun is 8.4
/// × 10¹⁸). An M dwarf's ozone follows its activity more than its temperature (Segura et al. 2005:
/// GJ 643's 1.55, quiet models' 0.14–0.38), and F's shape differs about each star (Segura et al.
/// 2003's F2V and K2V fall differently with O₂: F × U is off by up to 2.7 times, the F2V's at 10⁻⁴
/// PAL), which the product does not hold: recorded.
pub const OZONE_BY_STAR: [(f64, f64); 4] = [
    (3_400.0, 4.4e18 / 8.36e18),
    (5_180.0, 6.64e18 / 8.36e18),
    (5_772.0, 1.0),
    (6_700.0, 1.56e19 / 8.36e18),
];

/// The methane haze's index floor, a mole fraction: 10⁻⁵, the least CH₄ Trainer et al. 2006 (PNAS
/// 103, 18035, Fig. 3) tried in N₂ (0.001%), at which they still made haze (their production rises
/// with the CH₄ and peaks "near 0.02% CH₄"): the lowest concentration tried, not a measured
/// threshold. They found none in N₂–CH₄ mixtures, so the plan's "CH₄ ≳ 10⁻³" stands unsourced and
/// this floor replaces it (recorded; accepted by main, 2026-10-22).
pub const HAZE_METHANE_FLOOR: f64 = 1e-5;

/// The CH₄ to CO₂ ratio at which a CO₂ world's haze is thick: 0.2, Arney et al. 2016's ("a 'thick'
/// haze refers to the haze at a CH₄/CO₂ ratio ∼0.2", Astrobiology 16, 873; their onsets 0.15–0.28,
/// Table 2), about the Sun (Arney et al. 2017, ApJ 836, 49: 0.2 for the Sun and GJ 876, 0.3 for a
/// K2V, 0.9 for AD Leo, and none for an F2V up to 2, which this rule does not hold: recorded).
pub const HAZE_METHANE_TO_CARBON_DIOXIDE: f64 = 0.2;

/// Titan's haze τ(550) from the ground: 8.1, Tomasko et al. 2008's (Planet. Space Sci. 56, 669,
/// Fig. 47) as Bazzon, Schmid and Buenzli 2014 fit it (A&A 572, A6, App. A: τ80 + τ30 + τ0 =
/// 1.012 × 10⁷ λ^−2.339 + 2.029 × 10⁴ λ^−1.409 + 627.0 λ^−0.9706, λ in nm), at 550 nm. Tomasko et al.
/// 2005's "total vertical optical depth of 4–5 at 531 nm" may be the haze above 80 km alone
/// (Bazzon et al.'s τ80 is 4.3 there): recorded.
pub const TITAN_HAZE_OPTICAL_DEPTH: f64 = 8.1;

/// Titan's surface methane, by number: 5.65% (Niemann et al. 2010, JGR 115, E12006), at which the
/// haze's index reaches Titan's own.
pub const TITAN_SURFACE_METHANE: f64 = 0.0565;

/// The radius of the haze's monomers (Tomasko et al. 2008, as P14.T24.c and R08.T5.c take them):
/// 0.05 µm, 3,000 of them ([`TITAN_HAZE_MONOMERS`]) at the fractal dimension
/// [`TITAN_HAZE_FRACTAL_DIMENSION`]. Hörst 2017 (JGR Planets 122, 432) quotes 4,000 monomers of
/// 0.04 µm from Tomasko et al. 2008 and 2009: recorded.
pub const TITAN_HAZE_MONOMER_RADIUS: Metres = Metres::new(0.05e-6);

/// The number of the haze's monomers: 3,000 ([`TITAN_HAZE_MONOMER_RADIUS`]).
pub const TITAN_HAZE_MONOMERS: f64 = 3_000.0;

/// The haze's fractal dimension: 2, Tomasko et al. 2008's ("not constrained by observations and is
/// set to 2").
pub const TITAN_HAZE_FRACTAL_DIMENSION: f64 = 2.0;

/// The haze's top over the ground's pressure: four decades, well mixed between (Titan's particles
/// reach from the surface to its ionosphere; its extinction's 65 km scale height above 80 km,
/// longer than its air's, is not modelled).
pub const HAZE_TOP_RATIO: f64 = 1e-4;

/// Mars's background dust τ(550): 0.5, an annual mean read from Montabone et al. 2015's
/// climatology's maps (Icarus 251, 65: 9.3 µm absorption referred to 610 Pa about 0.09–0.13 at
/// aphelion and 0.3–0.4 at the non-storm perihelion peak, visible extinction 2.6 times it,
/// "affected by large errors"; they state no annual mean), the plan's "τ of order 0.5 in the
/// visible".
pub const MARS_DUST_OPTICAL_DEPTH: f64 = 0.5;

/// The dust's effective radius: rendering plan R08's Mars dust (science-r08-mmf.md §3), a gamma
/// distribution of the volume-to-area radius, `r_eff` 0.8974 µm (Wolff et al. 2009's sizes in R08's
/// convention), of `mars_dust`.
pub const DUST_EFFECTIVE_RADIUS: Metres = Metres::new(0.8974e-6);

/// The dust's effective variance, `v_eff` 0.3, with [`DUST_EFFECTIVE_RADIUS`].
pub const DUST_EFFECTIVE_VARIANCE: f64 = 0.3;

/// The dust's top over the ground's pressure: e⁻³, about three scale heights, well mixed between
/// (Mars's dust reaches 1–5 scale heights, Conrath 1975's profile; 50–75 km in a global storm).
pub const DUST_TOP_LN_RATIO: f64 = -3.0;

/// The thinnest air past which a world holds no dust: 0.1 bar, a stated bound (a thicker air's
/// dust is a desert's, scavenged and patchy, which this rule does not hold).
pub const DUST_AIR_LIMIT: Pascals = Pascals::new(1e4);

/// Mars's surface air, at which the dust's τ is [`MARS_DUST_OPTICAL_DEPTH`]: 636 Pa (NASA's Mars fact
/// sheet's 6.36 mb), 210 K and 3.71 m s⁻² (the module's measured Mars's, from memory; the current
/// sheet gives about 214 K and a mean 3.73 m s⁻²), and 43.34 g mol⁻¹, MSL's composition (Mahaffy et
/// al. 2013; the sheet's 43.49). Each difference moves the saltation threshold by under 1%.
const MARS_AIR: (f64, f64, f64, f64) = (636.0, 210.0, 43.34, 3.71);

/// The saltation threshold's friction velocity at the ground, m s⁻¹: Shao and Lu 2000's u*ₜ =
/// `A_N` √(ρₚ g D ÷ ρₐ + γ ÷ (ρₐ D)) (Kok, Parteli, Michaels and Bou Karam 2012, Rep. Prog.
/// Phys. 75, 106901, eq. 2.8: `A_N` 0.111, γ 1.65–5.00 × 10⁻⁴ N m⁻¹, here 3 × 10⁻⁴), at a grain
/// diameter D of
/// 75 µm (their §2.1.1's "~75–100 µm" minimum on Earth; by eq. 2.8 itself the minimum is at
/// √(γ ÷ (ρₚ g)), 107 µm on Earth and 175 µm on Mars, where Mars's threshold is 1.64 m s⁻¹) and
/// density ρₚ 2,650 kg m⁻³, under gravity `g` in air of density `air`: about 1.9 m s⁻¹ on Mars and
/// 0.2 on Earth (Kok et al.'s Table 2.3: 1.5 and 0.2).
#[must_use]
pub fn saltation_threshold(
    air: KilogramsPerCubicMetre,
    g: MetresPerSecondSquared,
) -> MetresPerSecond {
    let (a_n, gamma, diameter, grain) = (0.111, 3e-4, 75e-6, 2_650.0);
    let (air, g) = (air.value(), g.value());
    MetresPerSecond::new(a_n * (grain * g * diameter / air + gamma / (air * diameter)).sqrt())
}

/// The value at `x` of the curve through `nodes` ((x, y), x rising, every value positive), linear
/// in ln y against ln x between them, and beyond the last node the last segment's power law.
/// Below the first node it is the caller's.
#[must_use]
fn log_log(nodes: &[(f64, f64)], x: f64) -> f64 {
    let pair = nodes
        .windows(2)
        .find(|w| x <= w[1].0)
        .unwrap_or(&nodes[nodes.len() - 2..]);
    let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
    let slope = math::ln(y1 / y0) / math::ln(x1 / x0);
    y0 * math::exp(slope * math::ln(x / x0))
}

/// The ozone column's factor F of an O₂ level `level` (its O₂ column over Earth's present one,
/// PAL): [`OZONE_BY_OXYGEN`]'s curve, F(1) = 1, linear in the level below its first node so that
/// F(0) = 0, and its last segment's power law above its last node (a stated extrapolation, finite
/// and monotonic to any O₂ the generator makes).
#[must_use]
pub fn ozone_oxygen_factor(level: f64) -> f64 {
    if !(level.is_finite() && level > 0.0) {
        return 0.0;
    }
    let (first, f_first) = OZONE_BY_OXYGEN[0];
    if level <= first {
        return f_first * level / first;
    }
    log_log(&OZONE_BY_OXYGEN, level)
}

/// The ozone column's factor U of a host star of effective temperature `star`, relative to the
/// Sun's: [`OZONE_BY_STAR`]'s nodes, linear in the temperature between them and held at their ends.
#[must_use]
pub fn ozone_star_factor(star: Kelvin) -> f64 {
    let t = star.value();
    let nodes = &OZONE_BY_STAR;
    if !t.is_finite() {
        return 0.0;
    }
    if t <= nodes[0].0 {
        return nodes[0].1;
    }
    let last = nodes[nodes.len() - 1];
    if t >= last.0 {
        return last.1;
    }
    let pair = nodes
        .windows(2)
        .find(|w| t <= w[1].0)
        .unwrap_or(&nodes[nodes.len() - 2..]);
    let ((t0, u0), (t1, u1)) = (pair[0], pair[1]);
    u0 + (u1 - u0) * (t - t0) / (t1 - t0)
}

/// The ozone of an air of O₂ mole fraction `oxygen` at the ground (P14.T24.c's rule): `N_⊕` F(r)
/// U(star), r its O₂ column over Earth's, on the AFGL template ([`AFGL_US_STANDARD_OZONE`]) with
/// each node moved to the pressure at which the O₂ column above it equals the node's on Earth (the
/// layer forms at a fixed overhead O₂ column, to first order, Chapman 1930), the nodes below the
/// ground dropped and a node at the ground taken between the two about it; `None` with no O₂, no
/// star, or no node left above the ground.
///
/// # Panics
///
/// Never: `O3` is a registry row.
fn ozone(inputs: &AerosolInputs<'_>, oxygen: f64) -> Result<Option<Absorber>, BuildAbsorberError> {
    let Some(star) = inputs.star_temperature else {
        return Ok(None);
    };
    if !(oxygen.is_finite() && oxygen > 0.0) {
        return Ok(None);
    }
    let per_pascal = oxygen * AVOGADRO
        / (inputs.mean_molar_mass_g_per_mol / GRAMS_PER_KILOGRAM * inputs.gravity.value());
    let p_s = inputs.surface_pressure.value();
    let level = per_pascal * p_s / PRESENT_OXYGEN_COLUMN;
    let column = EARTH_OZONE_COLUMN * ozone_oxygen_factor(level) * ozone_star_factor(star);
    let scale = EARTH_OXYGEN_PER_PASCAL / per_pascal;
    let moved: Vec<(f64, f64)> = AFGL_US_STANDARD_OZONE
        .iter()
        .map(|&(mb, ppmv)| (mb * PASCALS_PER_MILLIBAR * scale, ppmv))
        .collect();
    let first_above = moved.iter().position(|&(p, _)| p < p_s);
    let Some(first_above) = first_above else {
        return Ok(None);
    };
    let mut nodes: Vec<(Pascals, f64)> = Vec::with_capacity(moved.len() - first_above + 1);
    if first_above > 0 {
        // The ground's node, between the template's two about it, ln x linear in ln p.
        let ((p0, x0), (p1, x1)) = (moved[first_above - 1], moved[first_above]);
        let w = math::ln(p_s / p0) / math::ln(p1 / p0);
        nodes.push((Pascals::new(p_s), x0 * math::exp(w * math::ln(x1 / x0))));
    }
    nodes.extend(
        moved[first_above..]
            .iter()
            .map(|&(p, x)| (Pascals::new(p), x)),
    );
    if nodes.len() < 2 || column <= 0.0 {
        return Ok(None);
    }
    let ozone = crate::substance::by_key("O3").expect("O3 is a registry row");
    Absorber::profile(ozone, column, nodes).map(Some)
}

/// A particle's shape class, which with its material fixes its optics (R08's Design note 6; a
/// closed physics enum, P14.T24.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShapeClass {
    /// A sphere: every liquid (water's droplets, sulphuric acid, liquid methane).
    Sphere,
    /// A non-spherical mineral grain: dust, the silicates, the metals, the salts and the sulphides,
    /// R08's `TAMUdust2020` hexahedra (Saito et al. 2021, J. Atmos. Sci. 78, 2089).
    NonSphericalMineral,
    /// An ice crystal: the molecular ices, Yang et al. 2013's roughened aggregate of eight columns
    /// for water's (J. Atmos. Sci. 70, 330).
    Crystal,
    /// A fractal aggregate of spherical monomers: tholin's and soot's hazes ([`Aggregate`]).
    Aggregate,
}

impl ShapeClass {
    /// The shape class of `material`'s particles in `phase`, as rendering plan R08's index files
    /// (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`) give theirs: every liquid a
    /// sphere; a solid molecule (or a noble gas's atom) a crystal, but sulphur's S₈, a mineral
    /// grain; a solid mineral, metal or lithology a non-spherical mineral, but ammonium
    /// hydrosulphide, a crystal; an organic solid (tholin, soot) a sphere, as a deck's or a dust's
    /// particle, an aggregate's monomer. A solution's solid is a crystal.
    #[must_use]
    pub fn of(material: SubstanceId, phase: CondensedPhase) -> Self {
        let row = material.substance();
        let key = row.key();
        match phase {
            CondensedPhase::Liquid => Self::Sphere,
            CondensedPhase::Solid => match row.kind() {
                SubstanceKind::Molecule => {
                    if MINERAL_MOLECULES.contains(&key.as_str()) {
                        Self::NonSphericalMineral
                    } else {
                        Self::Crystal
                    }
                }
                SubstanceKind::Atom => {
                    if NOBLE_GASES.contains(&key.as_str()) {
                        Self::Crystal
                    } else {
                        Self::NonSphericalMineral
                    }
                }
                SubstanceKind::Mineral => {
                    if ICE_MINERALS.contains(&key.as_str()) {
                        Self::Crystal
                    } else {
                        Self::NonSphericalMineral
                    }
                }
                SubstanceKind::Lithology => Self::NonSphericalMineral,
                SubstanceKind::Organic => Self::Sphere,
                SubstanceKind::Solution | SubstanceKind::Ion => Self::Crystal,
            },
        }
    }

    /// The sphericity Ψ = (`r_V` ÷ `r_S`)² of the class's particles, by which a settling radius
    /// converts to the volume-to-area radius, r = Ψ `r_V` (science-r08-mmf.md §2): 1 for a sphere
    /// and an aggregate's monomers, 0.71 for the hexahedra (`TAMUdust2020`'s, Saito et al. 2021;
    /// their `r_VA` ÷ `r_A` = 0.598 = 0.71^(3/2), the R08 lane's kernels), and 1 for a crystal, whose
    /// sphericity no source here states (Yang et al. 2013's aggregate): a labelled stand-in.
    #[must_use]
    pub const fn sphericity(self) -> f64 {
        match self {
            Self::Sphere | Self::Aggregate | Self::Crystal => 1.0,
            Self::NonSphericalMineral => 0.71,
        }
    }

    /// The class's name as R08's index files write it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sphere => "sphere",
            Self::NonSphericalMineral => "nonSphericalMineral",
            Self::Crystal => "crystal",
            Self::Aggregate => "aggregate",
        }
    }
}

/// The molecules whose solid is a mineral grain in R08's files: sulphur's S₈ (`sulphur.json`).
const MINERAL_MOLECULES: [&str; 1] = ["S8"];

/// The atoms whose solid is an ice crystal: the noble gases (`argon-ice.json`).
const NOBLE_GASES: [&str; 6] = ["He", "Ne", "Ar", "Kr", "Xe", "Rn"];

/// The minerals whose solid is an ice crystal: ammonium hydrosulphide
/// (`ammonium-hydrosulphide.json`).
const ICE_MINERALS: [&str; 1] = ["NH4SH"];

/// A fractal aggregate of `N` spherical monomers of radius `a₀` and fractal dimension `D_f`, with
/// prefactor `k_f` (Tazaki and Tanaka 2018, ApJ 860, 79, the model R08's Design note 6 validates
/// to `D_f` 2.5): the methane haze's particles.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Aggregate {
    monomer_radius: Metres,
    monomers: f64,
    fractal_dimension: f64,
}

/// The largest fractal dimension an aggregate takes: 2.5, inside which Tazaki and Tanaka 2018's
/// model is validated (R08's Design note 6).
pub const MAX_FRACTAL_DIMENSION: f64 = 2.5;

impl Aggregate {
    /// An aggregate of `monomers` monomers of radius `monomer_radius` and fractal dimension
    /// `fractal_dimension`.
    ///
    /// # Errors
    ///
    /// [`BuildAggregateError`] unless the radius is finite and positive, the count at least 1 and
    /// finite, and the dimension within 1 to [`MAX_FRACTAL_DIMENSION`].
    pub fn new(
        monomer_radius: Metres,
        monomers: f64,
        fractal_dimension: f64,
    ) -> Result<Self, BuildAggregateError> {
        let a = monomer_radius.value();
        if !(a.is_finite() && a > 0.0) {
            return Err(BuildAggregateError::MonomerRadius { radius_m: a });
        }
        if !(monomers.is_finite() && monomers >= 1.0) {
            return Err(BuildAggregateError::Monomers { monomers });
        }
        if !(1.0..=MAX_FRACTAL_DIMENSION).contains(&fractal_dimension) {
            return Err(BuildAggregateError::FractalDimension {
                dimension: fractal_dimension,
            });
        }
        Ok(Self {
            monomer_radius,
            monomers,
            fractal_dimension,
        })
    }

    /// The monomers' radius `a₀`.
    #[must_use]
    pub const fn monomer_radius(&self) -> Metres {
        self.monomer_radius
    }

    /// The number of monomers N, at least 1.
    #[must_use]
    pub const fn monomers(&self) -> f64 {
        self.monomers
    }

    /// The fractal dimension `D_f`, 1 to [`MAX_FRACTAL_DIMENSION`].
    #[must_use]
    pub const fn fractal_dimension(&self) -> f64 {
        self.fractal_dimension
    }

    /// The fractal prefactor `k_f` = (5 ÷ 3)^(`D_f` ÷ 2), P14.T24.c's default where the wire carries
    /// none (Tazaki and Tanaka 2018's).
    #[must_use]
    pub fn prefactor(&self) -> f64 {
        math::powf(5.0 / 3.0, self.fractal_dimension / 2.0)
    }
}

/// [`Aggregate::new`] refused its inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildAggregateError {
    /// The monomer radius is not finite and positive.
    MonomerRadius {
        /// The radius given, m.
        radius_m: f64,
    },
    /// The monomer count is not finite and at least 1.
    Monomers {
        /// The count given.
        monomers: f64,
    },
    /// The fractal dimension is outside 1 to [`MAX_FRACTAL_DIMENSION`].
    FractalDimension {
        /// The dimension given.
        dimension: f64,
    },
}

impl fmt::Display for BuildAggregateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MonomerRadius { radius_m } => {
                write!(f, "monomer radius {radius_m} m is not finite and positive")
            }
            Self::Monomers { monomers } => {
                write!(f, "monomer count {monomers} is not finite and at least 1")
            }
            Self::FractalDimension { dimension } => write!(
                f,
                "fractal dimension {dimension} is outside 1 to {MAX_FRACTAL_DIMENSION}"
            ),
        }
    }
}

impl Error for BuildAggregateError {}

/// The share of its condensate's critical temperature above which a liquid mode's size and τ(550)
/// are not modelled ([`AerosolMode::of_mode`]): 0.99, the cut to which the registry's grain-density
/// mirror (`packages/protocol/fixtures/substances.json`) holds a phase diagram's saturated liquid,
/// above which no table of nodes follows its fall to the critical density within 10⁻⁴
/// (decision-p14-t54a-grain-density §2.3 and F4).
pub const NEAR_CRITICAL_LIQUID_SHARE: f64 = 0.99;

/// A mode's particle sizes.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum ParticleSize {
    /// A size distribution of the volume-to-area radius r = 3V ÷ 4A, A the orientation-averaged
    /// projected area (a sphere's radius), by its effective radius and variance.
    Distribution(SizeDistribution),
    /// A fractal aggregate's monomers.
    Aggregate(Aggregate),
}

/// What a mode's τ(550) rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DepthBasis {
    /// P14.T49.e's mass extinction times the column mass, on the table's basis.
    Table(ExtinctionBasis),
    /// Ackerman and Marley's geometric scatterers, for a mode larger than the table's grid.
    GeometricLimit,
    /// A rule's own column, calibrated on a measured one: the methane haze's on Titan's.
    Rule,
}

/// A mode's column optical depth at 550 nm, and what it rests on.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ModeDepth {
    value: f64,
    basis: DepthBasis,
}

impl ModeDepth {
    /// τ(550), finite and non-negative.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// What it rests on.
    #[must_use]
    pub const fn basis(&self) -> DepthBasis {
        self.basis
    }
}

impl From<OpticalDepth> for ModeDepth {
    fn from(depth: OpticalDepth) -> Self {
        Self {
            value: depth.value(),
            basis: match depth.basis() {
                OpticalDepthBasis::Table(basis) => DepthBasis::Table(basis),
                OpticalDepthBasis::GeometricLimit => DepthBasis::GeometricLimit,
            },
        }
    }
}

/// One population of particles of one material in one phase (the [module](self)'s
/// documentation): what P14.T24.c's inventory carries, and the wire after P14.T54.b.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AerosolMode {
    material: SubstanceId,
    phase: CondensedPhase,
    shape: ShapeClass,
    deck: SubstanceId,
    size: Result<ParticleSize, SizeModeError>,
    column_mass: Option<KilogramsPerSquareMetre>,
    optical_depth: Result<ModeDepth, ResolveOpticalDepthError>,
    profile: ModeProfile,
    coverage: f64,
    temperature: Option<Kelvin>,
}

impl AerosolMode {
    /// The inventory's mode of a deck's or a rule's mode `mode` (P14.T52.c's), its size converted
    /// from the settling radius to the volume-to-area radius by its shape class, r = Ψ `r_V`, and
    /// its τ(550) taken again at r at `temperature`, the temperature the loop took the mode's
    /// phase and grain density at (its deck's base's).
    ///
    /// # Errors
    ///
    /// [`BuildModeError`] for a temperature that is not finite and positive.
    pub fn of_settling(mode: &Mode, temperature: Kelvin) -> Result<Self, BuildModeError> {
        let shape = ShapeClass::of(mode.material(), mode.phase());
        let converted = match mode.size() {
            Ok(size) => {
                let radius = Metres::new(size.effective_radius().value() * shape.sphericity());
                let size = rebuilt(&size, radius);
                Mode::new(
                    mode.material(),
                    mode.phase(),
                    mode.deck(),
                    size,
                    mode.column_mass(),
                    mode.profile(),
                    mode.coverage(),
                    temperature,
                )?
            }
            Err(_) => *mode,
        };
        Ok(Self::of_mode(&converted, shape, temperature))
    }

    /// The inventory's mode of a mode `mode` whose size is already the volume-to-area radius (a
    /// rule's: P14.T24.g's acid, the dust), of shape `shape`, whose phase, grain density and τ(550)
    /// were taken at `temperature` (the caller's, the one `mode` was built at).
    ///
    /// A liquid mode above [`NEAR_CRITICAL_LIQUID_SHARE`] of its condensate's critical temperature
    /// has its size and τ(550) not modelled, for want of its grain density there
    /// ([`SizeModeError::NoDensity`]): the registry's mirror, which a client evaluates the
    /// density from, holds no liquid above it, so the inventory publishes none it could not
    /// (decision-p14-t54a-grain-density F4). Its column mass stays the loop's.
    #[must_use]
    pub fn of_mode(mode: &Mode, shape: ShapeClass, temperature: Kelvin) -> Self {
        let near_critical = matches!(mode.phase(), CondensedPhase::Liquid)
            && mode.size().is_ok()
            && mode
                .material()
                .substance()
                .phase_diagram()
                .is_some_and(|diagram| {
                    temperature.value()
                        > NEAR_CRITICAL_LIQUID_SHARE * diagram.critical_temperature().value()
                });
        let (size, optical_depth) = if near_critical {
            let refused = SizeModeError::NoDensity {
                key: mode.material().substance().key(),
                phase: mode.phase(),
                temperature,
            };
            (Err(refused), Err(ResolveOpticalDepthError::Size(refused)))
        } else {
            (
                mode.size().map(ParticleSize::Distribution),
                mode.optical_depth().map(ModeDepth::from),
            )
        };
        Self {
            material: mode.material(),
            phase: mode.phase(),
            shape,
            deck: mode.deck(),
            size,
            column_mass: Some(mode.column_mass()),
            optical_depth,
            profile: mode.profile(),
            coverage: mode.coverage(),
            temperature: Some(temperature),
        }
    }

    /// A rule's mode of `material` in `phase` and shape `shape`, of deck `deck` and size `size`
    /// (of the volume-to-area radius), whose τ(550) is the rule's `optical_depth`: its column mass
    /// that τ over its mass extinction at `temperature` (P14.T49.e's), so that its own τ is the
    /// rule's to rounding, between its `profile`'s base and top, covering `coverage` of the sky.
    /// Where its optics are not modelled its column mass is not modelled either (`None`, never a
    /// placeholder 0; decision-p14-t54b-open §2.2) and its τ is the optics' error.
    ///
    /// # Errors
    ///
    /// [`BuildModeError::OpticalDepth`] for a τ that is not finite and non-negative, and as
    /// [`Mode::new`] for a coverage outside 0 to 1 or a temperature that is not finite and positive.
    #[expect(
        clippy::too_many_arguments,
        reason = "a mode's columns as P14.T24.c lists them, each a different quantity, and the \
                  temperature its optics are taken at: a builder would only rename them"
    )]
    pub fn of_rule(
        material: SubstanceId,
        phase: CondensedPhase,
        shape: ShapeClass,
        deck: SubstanceId,
        size: SizeDistribution,
        optical_depth: f64,
        profile: ModeProfile,
        coverage: f64,
        temperature: Kelvin,
    ) -> Result<Self, BuildModeError> {
        if !(optical_depth.is_finite() && optical_depth >= 0.0) {
            return Err(BuildModeError::OpticalDepth { optical_depth });
        }
        let unit = Mode::new(
            material,
            phase,
            deck,
            size,
            KilogramsPerSquareMetre::new(1.0),
            profile,
            coverage,
            temperature,
        )?;
        let per_kilogram = unit.optical_depth().map_or(f64::NAN, |d| d.value());
        let modelled = per_kilogram.is_finite() && per_kilogram > 0.0;
        let mass = if modelled {
            optical_depth / per_kilogram
        } else {
            0.0
        };
        let mode = Mode::new(
            material,
            phase,
            deck,
            size,
            KilogramsPerSquareMetre::new(mass),
            profile,
            coverage,
            temperature,
        )?;
        let built = Self::of_mode(&mode, shape, temperature);
        Ok(if modelled {
            built
        } else {
            Self {
                column_mass: None,
                ..built
            }
        })
    }

    /// An aggregate mode of `material`, of deck `deck`, whose τ(550) is a rule's `optical_depth`,
    /// its column mass not modelled (P14.T49.e holds no aggregate's mass extinction), and so no
    /// temperature its optics were taken at, between its `profile`'s base and top, covering
    /// `coverage` of the sky.
    ///
    /// # Errors
    ///
    /// [`BuildModeError::Coverage`] for a coverage outside 0 to 1, and
    /// [`BuildModeError::OpticalDepth`] for a τ that is not finite and non-negative.
    pub fn aggregate(
        material: SubstanceId,
        deck: SubstanceId,
        aggregate: Aggregate,
        optical_depth: f64,
        profile: ModeProfile,
        coverage: f64,
    ) -> Result<Self, BuildModeError> {
        if !(0.0..=1.0).contains(&coverage) {
            return Err(BuildModeError::Coverage { coverage });
        }
        if !(optical_depth.is_finite() && optical_depth >= 0.0) {
            return Err(BuildModeError::OpticalDepth { optical_depth });
        }
        Ok(Self {
            material,
            phase: CondensedPhase::Solid,
            shape: ShapeClass::Aggregate,
            deck,
            size: Ok(ParticleSize::Aggregate(aggregate)),
            column_mass: None,
            optical_depth: Ok(ModeDepth {
                value: optical_depth,
                basis: DepthBasis::Rule,
            }),
            profile,
            coverage,
            temperature: None,
        })
    }

    /// Its material, a registry row.
    #[must_use]
    pub const fn material(&self) -> SubstanceId {
        self.material
    }

    /// Its phase: solid or liquid, never a vapour.
    #[must_use]
    pub const fn phase(&self) -> CondensedPhase {
        self.phase
    }

    /// Its shape class.
    #[must_use]
    pub const fn shape(&self) -> ShapeClass {
        self.shape
    }

    /// The deck it belongs to, by the deck's condensate. A rule's mode names its own material: the
    /// dust `mars_dust`, the haze `tholin`, a sulphate layer `H2SO4`, none of which has an
    /// [`AerosolDeck`] of its own (P14.T54.b's wire field states it).
    #[must_use]
    pub const fn deck(&self) -> SubstanceId {
        self.deck
    }

    /// Its sizes, or why they are not modelled.
    ///
    /// # Errors
    ///
    /// [`SizeModeError`] where the size is not modelled (no grain density, no collision constants).
    pub const fn size(&self) -> Result<ParticleSize, SizeModeError> {
        self.size
    }

    /// Its column mass, kg m⁻²; `None` where it is not modelled: for an aggregate, whose mass
    /// extinction is not modelled, and for a rule's mode whose optics are not
    /// ([`optical_depth`](Self::optical_depth) says why), whose τ the rule sets.
    #[must_use]
    pub const fn column_mass(&self) -> Option<KilogramsPerSquareMetre> {
        self.column_mass
    }

    /// Its τ(550), or why it is not modelled.
    ///
    /// # Errors
    ///
    /// [`ResolveOpticalDepthError`] where it is not modelled.
    pub const fn optical_depth(&self) -> Result<ModeDepth, ResolveOpticalDepthError> {
        self.optical_depth
    }

    /// Its vertical profile.
    #[must_use]
    pub const fn profile(&self) -> ModeProfile {
        self.profile
    }

    /// The share of the sky it covers, 0 to 1. Its [`optical_depth`](Self::optical_depth) is its
    /// column's where it lies, over the covered share (in-cloud), before the coverage applies.
    #[must_use]
    pub const fn coverage(&self) -> f64 {
        self.coverage
    }

    /// The temperature its phase, grain density and τ(550) were taken at, P14.T49.e's
    /// `aerosol_optics`' (decision-p14-t54a-grain-density §3): a loop deck's base's, the acid
    /// deck's base's for P14.T24.g's acid modes, the skin's for a sulphate layer and the ground's
    /// for the dust. `None` for an aggregate, whose τ is a rule's and whose optics no table holds.
    #[must_use]
    pub const fn temperature(&self) -> Option<Kelvin> {
        self.temperature
    }
}

/// `size` with its effective radius `radius`, its family and variance kept (a conversion of
/// convention leaves `v_eff` unchanged, science-r08-mmf.md §2).
#[must_use]
fn rebuilt(size: &SizeDistribution, radius: Metres) -> SizeDistribution {
    let variance = size.effective_variance();
    let built = match size.kind() {
        crate::substance::SizeDistributionKind::Gamma => SizeDistribution::gamma(radius, variance),
        crate::substance::SizeDistributionKind::LogNormal => {
            SizeDistribution::log_normal(radius, variance)
        }
    };
    built.expect("a radius scaled by a sphericity in (0, 1] stays finite and positive")
}

/// A condensate's deck as the inventory states it (the [module](self)'s documentation).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct AerosolDeck {
    condensate: SubstanceId,
    phase: CondensedPhase,
    base: Pascals,
    base_temperature: Kelvin,
    top: Pascals,
    vapour_below: f64,
    vapour_above: f64,
    sedimentation: f64,
    extrapolated: bool,
}

impl AerosolDeck {
    /// The deck of P14.T52.c's `deck`.
    #[must_use]
    pub const fn of(deck: &Deck) -> Self {
        Self {
            condensate: deck.condensate(),
            phase: deck.phase(),
            base: deck.base(),
            base_temperature: deck.base_temperature(),
            top: deck.top(),
            vapour_below: deck.vapour_below(),
            vapour_above: deck.vapour_above(),
            sedimentation: deck.sedimentation(),
            extrapolated: deck.extrapolated(),
        }
    }

    /// The deck a rule makes of the loop's `found` (P14.T24.g's acid): its condensate, base, base
    /// temperature, vapour below and `extrapolated` flag the loop's, its phase `phase`, its top
    /// `top` (no deeper than the base: the caller's), its vapour above `vapour_above`, and the
    /// exponent of its modes' profiles `sedimentation`, 0 for well-mixed modes.
    #[must_use]
    pub(crate) const fn of_rule(
        found: &Deck,
        phase: CondensedPhase,
        top: Pascals,
        vapour_above: f64,
        sedimentation: f64,
    ) -> Self {
        Self {
            condensate: found.condensate(),
            phase,
            base: found.base(),
            base_temperature: found.base_temperature(),
            top,
            vapour_below: found.vapour_below(),
            vapour_above,
            sedimentation,
            extrapolated: found.extrapolated(),
        }
    }

    /// Its condensate, the registry row its modes name as their deck.
    #[must_use]
    pub const fn condensate(&self) -> SubstanceId {
        self.condensate
    }

    /// Its phase at the base.
    #[must_use]
    pub const fn phase(&self) -> CondensedPhase {
        self.phase
    }

    /// Its base's pressure.
    #[must_use]
    pub const fn base(&self) -> Pascals {
        self.base
    }

    /// Its base's temperature, on the profile.
    #[must_use]
    pub const fn base_temperature(&self) -> Kelvin {
        self.base_temperature
    }

    /// Its top's pressure.
    #[must_use]
    pub const fn top(&self) -> Pascals {
        self.top
    }

    /// Its limiting vapour's mole fraction below the base.
    #[must_use]
    pub const fn vapour_below(&self) -> f64 {
        self.vapour_below
    }

    /// Its limiting vapour's mole fraction above it, the cold trap's: what R08 reads for the
    /// vapour's column there rather than re-deriving the saturation curve.
    #[must_use]
    pub const fn vapour_above(&self) -> f64 {
        self.vapour_above
    }

    /// Its `f_sed`, its loop modes' profile exponent.
    #[must_use]
    pub const fn sedimentation(&self) -> f64 {
        self.sedimentation
    }

    /// Whether its law is applied outside what it was fitted over.
    #[must_use]
    pub const fn extrapolated(&self) -> bool {
        self.extrapolated
    }
}

/// An absorber's layer (P14.T24.c).
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum AbsorberLayer {
    /// A constant mixing ratio between a base and a top pressure.
    Slab {
        /// The base's pressure.
        base: Pascals,
        /// The top's pressure, no deeper than the base.
        top: Pascals,
    },
    /// A profile of nodes, the pressures descending: the mixing ratio linear in ln x against ln p
    /// between nodes and 0 outside them, the first and last nodes the base and the top (R08.T6.h;
    /// decision-r08-t6d-findings.md §2).
    Profile(Vec<AbsorberNode>),
}

/// One node of an absorber's profile: a pressure and the mixing ratio there relative to the
/// profile's others, the column scaling them.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct AbsorberNode {
    pressure: Pascals,
    relative_mixing_ratio: f64,
}

impl AbsorberNode {
    /// The node's pressure, finite and positive.
    #[must_use]
    pub const fn pressure(&self) -> Pascals {
        self.pressure
    }

    /// The node's mixing ratio relative to its profile's others, finite and positive.
    #[must_use]
    pub const fn relative_mixing_ratio(&self) -> f64 {
        self.relative_mixing_ratio
    }
}

/// A gas that absorbs, listed apart from P14.T24.a's well-mixed gases (P14.T24.c): ozone, and SO₂
/// by P14.T24.g.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Absorber {
    species: SubstanceId,
    column_per_m2: f64,
    layer: AbsorberLayer,
}

impl Absorber {
    /// An absorber of `species` of `column_per_m2` molecules m⁻², at a constant mixing ratio
    /// between `base` and `top`.
    ///
    /// # Errors
    ///
    /// [`BuildAbsorberError::Column`] unless the column is finite and non-negative, and
    /// [`BuildAbsorberError::Layer`] unless the top is finite, positive and no deeper than the
    /// finite base.
    pub fn slab(
        species: SubstanceId,
        column_per_m2: f64,
        base: Pascals,
        top: Pascals,
    ) -> Result<Self, BuildAbsorberError> {
        check_column(column_per_m2)?;
        let (b, t) = (base.value(), top.value());
        if !(t.is_finite() && t > 0.0 && b.is_finite() && t <= b) {
            return Err(BuildAbsorberError::Layer);
        }
        Ok(Self {
            species,
            column_per_m2,
            layer: AbsorberLayer::Slab { base, top },
        })
    }

    /// An absorber of `species` of `column_per_m2` molecules m⁻² on the profile `nodes`, given as
    /// (pressure, relative mixing ratio) pairs, the pressures descending.
    ///
    /// # Errors
    ///
    /// [`BuildAbsorberError::Column`] unless the column is finite and non-negative, and
    /// [`BuildAbsorberError::Layer`] unless there are at least two nodes, each pressure finite,
    /// positive and below the one before, and each mixing ratio finite and positive.
    pub fn profile(
        species: SubstanceId,
        column_per_m2: f64,
        nodes: impl IntoIterator<Item = (Pascals, f64)>,
    ) -> Result<Self, BuildAbsorberError> {
        check_column(column_per_m2)?;
        let mut built: Vec<AbsorberNode> = Vec::new();
        for (pressure, relative_mixing_ratio) in nodes {
            let p = pressure.value();
            let descending = built.last().is_none_or(|last| p < last.pressure.value());
            if !(p.is_finite()
                && p > 0.0
                && descending
                && relative_mixing_ratio.is_finite()
                && relative_mixing_ratio > 0.0)
            {
                return Err(BuildAbsorberError::Layer);
            }
            built.push(AbsorberNode {
                pressure,
                relative_mixing_ratio,
            });
        }
        if built.len() < 2 {
            return Err(BuildAbsorberError::Layer);
        }
        Ok(Self {
            species,
            column_per_m2,
            layer: AbsorberLayer::Profile(built),
        })
    }

    /// The gas, a registry row.
    #[must_use]
    pub const fn species(&self) -> SubstanceId {
        self.species
    }

    /// Its column, molecules m⁻², finite and non-negative.
    #[must_use]
    pub const fn column_per_m2(&self) -> f64 {
        self.column_per_m2
    }

    /// Its layer.
    #[must_use]
    pub const fn layer(&self) -> &AbsorberLayer {
        &self.layer
    }
}

/// [`Absorber::slab`] or [`Absorber::profile`] refused its inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildAbsorberError {
    /// The column is not finite and non-negative.
    Column(f64),
    /// The layer is not one: a slab's top not finite and positive or deeper than its base, or a
    /// profile of fewer than two nodes, its pressures not descending or a mixing ratio not
    /// positive.
    Layer,
}

impl fmt::Display for BuildAbsorberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Column(column) => {
                write!(f, "column {column} m⁻² is not finite and non-negative")
            }
            Self::Layer => write!(f, "the layer is not a slab or a descending profile"),
        }
    }
}

impl Error for BuildAbsorberError {}

/// `column` checked: finite and non-negative.
fn check_column(column: f64) -> Result<(), BuildAbsorberError> {
    if column.is_finite() && column >= 0.0 {
        Ok(())
    } else {
        Err(BuildAbsorberError::Column(column))
    }
}

/// What [`AerosolInventory::derive`] reads, as plain values.
///
/// The fields carry no invariant the derivation relies on: it checks each, so they are public.
#[derive(Debug, Clone, Copy)]
pub struct AerosolInputs<'a> {
    /// P14.T13.c's surface state: an airless body, a magma ocean and a gas envelope hold none.
    pub state: SurfaceState,
    /// The surface pressure, P14.T24.a's.
    pub surface_pressure: Pascals,
    /// The air's gases as `(SubstanceId, x)` mole fractions at the ground, P14.T24.a's published
    /// list.
    pub gases: &'a [(SubstanceId, f64)],
    /// The gases' mean molar mass, g mol⁻¹, P14.T24.a's.
    pub mean_molar_mass_g_per_mol: f64,
    /// The surface gravity.
    pub gravity: MetresPerSecondSquared,
    /// The air's vertical structure, P14.T24.e's.
    pub structure: &'a RadiativeConvective,
    /// The cloud fraction of the snapshot path (P14.T51.b's), which the decks' coverage takes.
    pub cloud_fraction: f64,
    /// The effective temperature of the star whose ultraviolet drives the air's photochemistry,
    /// its brightest host's; `None` for a body with no star as host (a rogue planet, a brown
    /// dwarf's), which makes no ozone, haze or acid.
    pub star_temperature: Option<Kelvin>,
    /// The share of the ground its surface liquids cover, 0 to 1 (P14.T24.b's `surface_liquids`,
    /// summed), whose dry rest lifts the dust.
    pub surface_liquid_cover: f64,
    /// P14.T24.b's present outgassing O, relative to Earth's, which P14.T24.g's SO₂ reads.
    pub outgassing: f64,
    /// Whether the surface holds liquid water (P14.T24.a's flag), which sets SO₂'s life on a
    /// temperate or snowball world (P14.T24.g).
    pub liquid_water: bool,
    /// Whether a runaway greenhouse's climate keeps Venus's acid deck (P14.T53.b part 1 (b)),
    /// which with the state gives P14.T24.g's case ([`SulphurCycle::case_of`]); read only for a
    /// runaway.
    pub runaway_deck: RunawayDeck,
}

/// [`AerosolInventory::derive`] refused its inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveAerosolsError {
    /// The cloud fraction is not within 0 to 1.
    CloudFraction(f64),
    /// The gas's molar mass, or the gravity, is not finite and positive.
    Column {
        /// The field, by its name.
        field: &'static str,
        /// The value given.
        value: f64,
    },
    /// The condensation loop refused the air.
    Condensation(DeriveCondensationError),
    /// A deck's sedimentation is not one.
    Sedimentation(BuildSedimentationError),
    /// A mode could not be built.
    Mode(BuildModeError),
    /// An absorber could not be built.
    Absorber(BuildAbsorberError),
    /// P14.T24.g's sulphur could not build a mode or its absorber.
    Sulphur(DeriveSulphurError),
}

impl From<DeriveSulphurError> for DeriveAerosolsError {
    fn from(error: DeriveSulphurError) -> Self {
        Self::Sulphur(error)
    }
}

impl From<BuildAbsorberError> for DeriveAerosolsError {
    fn from(error: BuildAbsorberError) -> Self {
        Self::Absorber(error)
    }
}

impl fmt::Display for DeriveAerosolsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CloudFraction(value) => {
                write!(f, "cloud fraction {value} is not within 0 to 1")
            }
            Self::Column { field, value } => {
                write!(f, "the air's {field} {value} is not finite and positive")
            }
            Self::Condensation(error) => write!(f, "the condensation: {error}"),
            Self::Sedimentation(error) => write!(f, "a deck's sedimentation: {error}"),
            Self::Mode(error) => write!(f, "a mode: {error}"),
            Self::Absorber(error) => write!(f, "an absorber: {error}"),
            Self::Sulphur(error) => write!(f, "the sulphur: {error}"),
        }
    }
}

impl Error for DeriveAerosolsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Condensation(error) => Some(error),
            Self::Sedimentation(error) => Some(error),
            Self::Mode(error) => Some(error),
            Self::Absorber(error) => Some(error),
            Self::Sulphur(error) => Some(error),
            Self::CloudFraction(_) | Self::Column { .. } => None,
        }
    }
}

impl From<DeriveCondensationError> for DeriveAerosolsError {
    fn from(error: DeriveCondensationError) -> Self {
        Self::Condensation(error)
    }
}

impl From<BuildSedimentationError> for DeriveAerosolsError {
    fn from(error: BuildSedimentationError) -> Self {
        Self::Sedimentation(error)
    }
}

impl From<BuildModeError> for DeriveAerosolsError {
    fn from(error: BuildModeError) -> Self {
        Self::Mode(error)
    }
}

/// A surface's aerosols and absorbers (P14.T24.c; the [module](self)'s documentation).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AerosolInventory {
    decks: Vec<AerosolDeck>,
    modes: Vec<AerosolMode>,
    absorbers: Vec<Absorber>,
    not_formed: Vec<(SubstanceId, NotFormed)>,
}

/// The ratio of the condensation loop's column top to the tropopause's pressure: two decades above
/// it, within the isothermal skin, where no vapour condenses (its saturation mixing ratio rises
/// with height there).
const COLUMN_TOP_BELOW_TROPOPAUSE: f64 = 1e-2;

impl AerosolInventory {
    /// An empty inventory: an airless body's, a magma ocean's or a gas envelope's.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            decks: Vec::new(),
            modes: Vec::new(),
            absorbers: Vec::new(),
            not_formed: Vec::new(),
        }
    }

    /// The inventory of the air `inputs` describe (the [module](self)'s documentation).
    ///
    /// # Errors
    ///
    /// - [`DeriveAerosolsError::CloudFraction`] for a cloud fraction outside 0 to 1;
    /// - [`DeriveAerosolsError::Column`] for a molar mass or a gravity that is not finite and
    ///   positive;
    /// - [`DeriveAerosolsError::Condensation`] for a gas or a profile the condensation loop refuses;
    /// - [`DeriveAerosolsError::Sedimentation`] where a deck's supply gives no finite `f_sed`;
    /// - [`DeriveAerosolsError::Mode`], [`DeriveAerosolsError::Absorber`] and
    ///   [`DeriveAerosolsError::Sulphur`] where a rule's mode or absorber is not one: unreachable
    ///   from finite inputs that pass the checks above, kept as errors rather than panics.
    ///
    /// # Examples
    ///
    /// An Earth-like air by hand, 1.3% of water at 288 K under a 214 K skin, makes a water deck
    /// whose vapour below is the air's:
    ///
    /// ```
    /// use hyperion_sim::planetary::derive::atmosphere::SurfaceState;
    /// use hyperion_sim::planetary::hooks::aerosols::{AerosolInputs, AerosolInventory};
    /// use hyperion_sim::planetary::hooks::structure::{RadiativeConvective, StructureInputs};
    /// use hyperion_sim::planetary::hooks::sulphur::RunawayDeck;
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let (n2, h2o) = (by_key("N2").ok_or("a row")?, by_key("H2O").ok_or("a row")?);
    /// let gases = [(n2, 0.987), (h2o, 0.013)];
    /// let structure = RadiativeConvective::derive(&StructureInputs {
    ///     surface_temperature: Kelvin::new(288.0),
    ///     surface_pressure: Pascals::new(1.0e5),
    ///     gases: &gases,
    ///     condensable: Some(h2o),
    ///     skin_temperature: Kelvin::new(214.0),
    /// })?;
    /// let inventory = AerosolInventory::derive(&AerosolInputs {
    ///     state: SurfaceState::Temperate,
    ///     surface_pressure: Pascals::new(1.0e5),
    ///     gases: &gases,
    ///     mean_molar_mass_g_per_mol: 0.987 * 28.0134 + 0.013 * 18.015,
    ///     gravity: MetresPerSecondSquared::new(9.81),
    ///     structure: &structure,
    ///     cloud_fraction: 0.67,
    ///     star_temperature: Some(Kelvin::new(5_772.0)),
    ///     surface_liquid_cover: 0.7,
    ///     outgassing: 0.0,
    ///     liquid_water: true,
    ///     runaway_deck: RunawayDeck::Kept,
    /// })?;
    /// let water = inventory
    ///     .decks()
    ///     .iter()
    ///     .find(|d| d.condensate() == h2o)
    ///     .ok_or("a water deck")?;
    /// assert!((water.vapour_below() - 0.013).abs() < 1e-12);
    /// assert!(inventory.deck_optical_depth(h2o).is_some_and(|tau| tau > 0.0));
    /// # Ok(())
    /// # }
    /// ```
    pub fn derive(inputs: &AerosolInputs<'_>) -> Result<Self, DeriveAerosolsError> {
        let fraction = inputs.cloud_fraction;
        if !(0.0..=1.0).contains(&fraction) {
            return Err(DeriveAerosolsError::CloudFraction(fraction));
        }
        if !holds_air(inputs) {
            return Ok(Self::empty());
        }
        for (field, value) in [
            ("molar mass", inputs.mean_molar_mass_g_per_mol),
            ("gravity", inputs.gravity.value()),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(DeriveAerosolsError::Column { field, value });
            }
        }
        let mut inventory = Self::empty();
        let sulphur_inputs = sulphur_inputs(inputs);
        let sulphur = Sulphur::derive(&sulphur_inputs);
        let acid_top = inventory.add_decks(inputs, &sulphur)?;
        if let Some(layer) = sulphur
            .sulphate_layer(&sulphur_inputs)
            .map_err(DeriveAerosolsError::Sulphur)?
        {
            inventory.modes.push(layer);
        }
        let so2_top = acid_top.unwrap_or_else(|| inputs.structure.tropopause_pressure());
        if let Some(so2) = sulphur
            .absorber(inputs.surface_pressure, so2_top)
            .map_err(DeriveAerosolsError::Sulphur)?
        {
            inventory.absorbers.push(so2);
        }
        let fraction_of = |key: &str| {
            crate::substance::by_key(key).map_or(0.0, |id| {
                inputs
                    .gases
                    .iter()
                    .filter(|(gas, _)| *gas == id)
                    .map(|&(_, x)| x)
                    .sum()
            })
        };
        if let Some(haze) = methane_haze(inputs, fraction_of("CH4"), fraction_of("CO2"))? {
            inventory.modes.push(haze);
        }
        if let Some(dust) = dust(inputs)? {
            inventory.modes.push(dust);
        }
        if let Some(ozone) = ozone(inputs, fraction_of("O2"))? {
            inventory.absorbers.push(ozone);
        }
        inventory.absorbers.sort_by_key(Absorber::species);
        Ok(inventory)
    }

    /// The decks of the loop, each of its supply's sedimentation, and their modes, converted.
    fn add_decks(
        &mut self,
        inputs: &AerosolInputs<'_>,
        sulphur: &Sulphur,
    ) -> Result<Option<Pascals>, DeriveAerosolsError> {
        let structure = inputs.structure;
        let column = MixingColumn {
            gravity: inputs.gravity,
            molar_mass_g_per_mol: inputs.mean_molar_mass_g_per_mol,
            eddy_diffusion: CLOUD_EDDY_DIFFUSION,
            metallicity: 1.0,
            top: Pascals::new(
                structure.tropopause_pressure().value() * COLUMN_TOP_BELOW_TROPOPAUSE,
            ),
            bottom: inputs.surface_pressure,
        };
        let profile = |p: Pascals| structure.temperature_at(p);
        // A cycling world's SO₂, which T24.a's fractions do not list, joins the loop's gas as the
        // acid's vapour (P14.T24.g); a once-through world's makes no deck.
        let acid = crate::substance::by_key("H2SO4");
        let dioxide = crate::substance::by_key("SO2")
            .filter(|_| sulphur.cycle() == SulphurCycle::Cycling && sulphur.mole_fraction() > 0.0)
            .map(|id| (id, sulphur.mole_fraction()));
        let gas = inputs.gases.iter().copied().chain(dioxide);
        // Two passes: the first finds each deck's base and vapour, from which its supply's
        // sedimentation follows; the second forms the decks with it.
        let first = Condensation::derive(
            gas.clone(),
            profile,
            &column,
            |_| Sedimentation::BASELINE,
            inputs.cloud_fraction,
        )?;
        let mut sedimentation: Vec<(SubstanceId, Sedimentation)> =
            Vec::with_capacity(first.decks().len());
        for deck in first.decks() {
            sedimentation.push((deck.condensate(), sedimentation_for_supply(deck, &column)?));
        }
        let of = |id: SubstanceId| {
            sedimentation
                .iter()
                .find(|(d, _)| *d == id)
                .map_or(Sedimentation::BASELINE, |&(_, s)| s)
        };
        let clouds = Condensation::derive(gas, profile, &column, of, inputs.cloud_fraction)?;
        let mut acid_top = None;
        for deck in clouds.decks() {
            if Some(deck.condensate()) == acid {
                // The acid's deck is P14.T24.g's: the loop finds its base, the rule the rest.
                let sulphur_inputs = sulphur_inputs(inputs);
                if let Some((acid_deck, modes)) =
                    sulphur.acid_deck(&sulphur_inputs, deck, inputs.cloud_fraction)?
                {
                    acid_top = Some(acid_deck.top());
                    self.decks.push(acid_deck);
                    self.modes.extend(modes);
                }
                continue;
            }
            self.decks.push(AerosolDeck::of(deck));
            for mode in deck.modes() {
                self.modes
                    .push(AerosolMode::of_settling(mode, deck.base_temperature())?);
            }
        }
        self.not_formed.extend_from_slice(clouds.not_formed());
        Ok(acid_top)
    }

    /// The decks, hottest base first.
    #[must_use]
    pub fn decks(&self) -> &[AerosolDeck] {
        &self.decks
    }

    /// The modes: each deck's, in the decks' order, then the hazes' and the dust's.
    #[must_use]
    pub fn modes(&self) -> &[AerosolMode] {
        &self.modes
    }

    /// The absorbers, in registry order, each at most once.
    #[must_use]
    pub fn absorbers(&self) -> &[Absorber] {
        &self.absorbers
    }

    /// The condensables the loop labels rather than traps ([`NotFormed`]), in registry order.
    #[must_use]
    pub fn not_formed(&self) -> &[(SubstanceId, NotFormed)] {
        &self.not_formed
    }

    /// The τ(550) of the modes that name `condensate` as their deck, the sum of those modelled: a
    /// deck's, or a rule's layer's (Earth's sulphate layer names `H2SO4` with no deck); `None` where
    /// no mode names it or none of them has a τ.
    #[must_use]
    pub fn deck_optical_depth(&self, condensate: SubstanceId) -> Option<f64> {
        let mut total = None;
        for mode in self.modes.iter().filter(|m| m.deck == condensate) {
            if let Ok(depth) = mode.optical_depth {
                total = Some(total.unwrap_or(0.0) + depth.value);
            }
        }
        total
    }
}

/// The methane haze's index of an air of CH₄ and CO₂ mole fractions `methane` and
/// `carbon_dioxide`: x(CH₄) ÷ ([`HAZE_METHANE_TO_CARBON_DIOXIDE`] x(CO₂) + [`HAZE_METHANE_FLOOR`]),
/// above 1 where haze forms; Titan's own is its 5.65% over the floor, about 5,650.
#[must_use]
pub fn haze_index(methane: f64, carbon_dioxide: f64) -> f64 {
    methane / (HAZE_METHANE_TO_CARBON_DIOXIDE * carbon_dioxide.max(0.0) + HAZE_METHANE_FLOOR)
}

/// The methane haze's τ(550) at the index `index` ([`haze_index`]): 0 at and below 1, rising as ln
/// index to Titan's [`TITAN_HAZE_OPTICAL_DEPTH`] at Titan's index and held there, continuous and
/// monotonic. The rule takes the methane reservoir as resupplied, as P14.T24.f does: its carbon is
/// photolysed methane (science-p14-t24f §4's sink, photolysis to hydrocarbons, not escape), whose
/// outcome P14.T53.a records.
///
/// # Examples
///
/// Titan's methane makes Titan's haze, and a trace at the floor none:
///
/// ```
/// use hyperion_sim::planetary::hooks::aerosols::{
///     HAZE_METHANE_FLOOR, TITAN_HAZE_OPTICAL_DEPTH, haze_index, haze_optical_depth,
/// };
///
/// let titan = haze_optical_depth(haze_index(0.0565, 1e-8));
/// assert!((titan / TITAN_HAZE_OPTICAL_DEPTH - 1.0).abs() < 1e-3);
/// assert!(haze_optical_depth(haze_index(HAZE_METHANE_FLOOR, 0.0)) <= 0.0);
/// ```
#[must_use]
pub fn haze_optical_depth(index: f64) -> f64 {
    if !(index.is_finite() && index > 1.0) {
        return 0.0;
    }
    let titan = haze_index(TITAN_SURFACE_METHANE, 0.0);
    TITAN_HAZE_OPTICAL_DEPTH * (math::ln(index) / math::ln(titan)).min(1.0)
}

/// The methane haze of the air `inputs` describe (P14.T24.c): a fractal-aggregate mode of `tholin`
/// with Titan's monomers ([`TITAN_HAZE_MONOMER_RADIUS`]), its τ(550) [`haze_optical_depth`], well mixed
/// from the ground to [`HAZE_TOP_RATIO`] of its pressure over the whole sky; `None` with no star,
/// whose ultraviolet photolyses the methane, or an index at most 1.
///
/// # Panics
///
/// Never: the haze's monomers are an aggregate, and `tholin` a registry row.
fn methane_haze(
    inputs: &AerosolInputs<'_>,
    methane: f64,
    carbon_dioxide: f64,
) -> Result<Option<AerosolMode>, BuildModeError> {
    if inputs.star_temperature.is_none() {
        return Ok(None);
    }
    let tau = haze_optical_depth(haze_index(methane, carbon_dioxide));
    if tau <= 0.0 {
        return Ok(None);
    }
    let aggregate = Aggregate::new(
        TITAN_HAZE_MONOMER_RADIUS,
        TITAN_HAZE_MONOMERS,
        TITAN_HAZE_FRACTAL_DIMENSION,
    )
    .expect("Titan's monomers are an aggregate");
    let tholin = crate::substance::by_key("tholin").expect("tholin is a registry row");
    let ground = inputs.surface_pressure;
    let profile = ModeProfile::new(ground, Pascals::new(ground.value() * HAZE_TOP_RATIO), 0.0)?;
    AerosolMode::aggregate(tholin, tholin, aggregate, tau, profile, 1.0).map(Some)
}

/// The dust's τ(550) of an air of surface pressure `pressure`, temperature `temperature` and molar
/// mass `molar_mass_g_per_mol` under `gravity`, over a ground whose liquids cover `liquid_cover`
/// of it, 0 to 1 (P14.T24.c's dust on arid, windy, thin-aired worlds; a stated rule):
/// Mars's background [`MARS_DUST_OPTICAL_DEPTH`] times the ground's dry share, times a thin-air
/// weight falling as ln p from 1 at Mars's 636 Pa (and below) to 0 at [`DUST_AIR_LIMIT`], times a
/// lifting weight falling as ln u*ₜ from 1 at Mars's [`saltation_threshold`] (and below) to 0 at
/// twice it. Each weight is continuous. The winds are taken as Mars's, which can lift its dust:
/// the rule reads the threshold from the plan's pressure and gravity alone, and no rotation (no
/// citable rule found; recorded). The ground's dry share is continuous where the plan's "no surface
/// liquid of any substance" is a step, so that a partly wet ground keeps some dust.
///
/// # Examples
///
/// Mars's own air holds its background dust; an Earth's, too thick, holds none:
///
/// ```
/// use hyperion_sim::planetary::hooks::aerosols::{MARS_DUST_OPTICAL_DEPTH, dust_optical_depth};
/// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
///
/// let mars = dust_optical_depth(
///     Pascals::new(636.0),
///     Kelvin::new(210.0),
///     43.34,
///     MetresPerSecondSquared::new(3.71),
///     0.0,
/// );
/// assert!((mars - MARS_DUST_OPTICAL_DEPTH).abs() < 1e-12);
/// let earth = dust_optical_depth(
///     Pascals::new(101_325.0),
///     Kelvin::new(288.0),
///     28.96,
///     MetresPerSecondSquared::new(9.81),
///     0.0,
/// );
/// assert_eq!(earth.to_bits(), 0.0_f64.to_bits());
/// ```
#[must_use]
pub fn dust_optical_depth(
    pressure: Pascals,
    temperature: Kelvin,
    molar_mass_g_per_mol: f64,
    gravity: MetresPerSecondSquared,
    liquid_cover: f64,
) -> f64 {
    let limit = DUST_AIR_LIMIT.value();
    let pressure = pressure.value();
    if !(pressure.is_finite() && pressure > 0.0 && pressure < limit) {
        return 0.0;
    }
    let density = |p: f64, t: f64, m: f64| {
        KilogramsPerCubicMetre::new(p * m / GRAMS_PER_KILOGRAM / (MOLAR_GAS_CONSTANT * t))
    };
    let (mars_p, mars_t, mars_mu, mars_g) = MARS_AIR;
    let mars_threshold = saltation_threshold(
        density(mars_p, mars_t, mars_mu),
        MetresPerSecondSquared::new(mars_g),
    )
    .value();
    let threshold = saltation_threshold(
        density(pressure, temperature.value(), molar_mass_g_per_mol),
        gravity,
    )
    .value();
    let thin = (math::ln(limit / pressure) / math::ln(limit / mars_p)).clamp(0.0, 1.0);
    let lifting = (math::ln(2.0 * mars_threshold / threshold) / math::ln(2.0)).clamp(0.0, 1.0);
    let dry = (1.0 - liquid_cover).clamp(0.0, 1.0);
    let tau = MARS_DUST_OPTICAL_DEPTH * dry * thin * lifting;
    if tau.is_finite() { tau } else { 0.0 }
}

/// The dust of the air `inputs` describe: a non-spherical mineral mode of `mars_dust`
/// ([`DUST_EFFECTIVE_RADIUS`]), its τ(550) [`dust_optical_depth`], well mixed from the ground to e^[`DUST_TOP_LN_RATIO`]
/// of its pressure over the whole sky; `None` where it is 0.
///
/// # Panics
///
/// Never: `mars_dust` is a registry row, and [`DUST_EFFECTIVE_RADIUS`] a gamma distribution.
fn dust(inputs: &AerosolInputs<'_>) -> Result<Option<AerosolMode>, BuildModeError> {
    let ground = inputs.surface_pressure;
    let temperature = inputs.structure.surface_temperature();
    let tau = dust_optical_depth(
        ground,
        temperature,
        inputs.mean_molar_mass_g_per_mol,
        inputs.gravity,
        inputs.surface_liquid_cover,
    );
    if tau <= 0.0 {
        return Ok(None);
    }
    let material = crate::substance::by_key("mars_dust").expect("mars_dust is a registry row");
    let size = SizeDistribution::gamma(DUST_EFFECTIVE_RADIUS, DUST_EFFECTIVE_VARIANCE)
        .expect("the dust's mode is a gamma distribution");
    let top = Pascals::new(ground.value() * math::exp(DUST_TOP_LN_RATIO));
    let profile = ModeProfile::new(ground, top, 0.0)?;
    AerosolMode::of_rule(
        material,
        CondensedPhase::Solid,
        ShapeClass::NonSphericalMineral,
        material,
        size,
        tau,
        profile,
        1.0,
        temperature,
    )
    .map(Some)
}

/// What P14.T24.g's sulphur reads of the inventory's inputs.
#[must_use]
const fn sulphur_inputs<'a>(inputs: &AerosolInputs<'a>) -> SulphurInputs<'a> {
    SulphurInputs {
        cycle: SulphurCycle::case_of(inputs.state, inputs.runaway_deck),
        outgassing: inputs.outgassing,
        starlit: inputs.star_temperature.is_some(),
        liquid_water: inputs.liquid_water,
        surface_pressure: inputs.surface_pressure,
        mean_molar_mass_g_per_mol: inputs.mean_molar_mass_g_per_mol,
        gravity: inputs.gravity,
        structure: inputs.structure,
    }
}

/// Whether the inputs describe an air the inventory's rules apply to: a state with a surface and a
/// bulk air, a positive surface pressure and some gas.
///
/// The inventory's rules are not applied to a tenuous air (P14.T53.c;
/// decision-p14-t53c-tenuous-aerosols): its methane haze's τ would read its index alone, with no
/// column or ultraviolet supply (a 1.4 Pa N₂ air at x(CH₄) 10⁻³ would take τ 4.3, against Pluto's
/// measured 0.013), and its dust, decks and ozone are withheld with it, none of their rules fitted
/// to so thin a column. The record sends its aerosols not modelled until P14.T55.n.
#[must_use]
fn holds_air(inputs: &AerosolInputs<'_>) -> bool {
    let p = inputs.surface_pressure.value();
    let state = match inputs.state {
        SurfaceState::Temperate | SurfaceState::Snowball | SurfaceState::RunawayGreenhouse => true,
        SurfaceState::Airless
        | SurfaceState::MagmaOcean
        | SurfaceState::GasEnvelope
        | SurfaceState::Tenuous => false,
    };
    state && p.is_finite() && p > 0.0 && !inputs.gases.is_empty()
}

#[cfg(test)]
mod tests;
