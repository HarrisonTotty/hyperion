//! Below 0.1 M☉: cooling fits giving luminosity, radius and temperature from mass, age and
//! composition (plan 06, P06.T13), which the brown dwarf and rogue planet layers of plan 13 share.
//!
//! The brainstorm's row "Below 0.1 M☉" asks for "cooling fits for the latest M dwarfs and the
//! brown dwarfs, giving luminosity and temperature from mass and age (Burrows et al. 2001, checked
//! against Baraffe et al. 2015). These supply classes L, T and Y." [`cooling`] is that fit, valid
//! from [`MIN_MASS`] (0.01 M☉, 10.5 Jupiter masses) to [`MAX_MASS`] (0.1 M☉), where it meets the
//! backbone's zero-age main sequence; plan 13's [`giant_cooling`] continues it below, from 13 down
//! to 0.3 Jupiter masses, with its own [`CoolingState`].
//!
//! # The model
//!
//! Every quantity is a closed form in mass m, age t and metal fraction Z, with no table and no
//! step in age, and it is continuous in all three. Three regimes are joined by p-norms, a smooth
//! maximum (aᵖ + bᵖ)^(1/p) or minimum (a⁻ᵖ + b⁻ᵖ)^(−1/p):
//!
//! 1. **Contraction.** A young object shrinks along its Hayashi track at a nearly fixed effective
//!    temperature `T_H(m)`, radiating its binding energy. For a fully convective n = 3/2 polytrope
//!    the energy is −(3/7) GM²/R, so contraction from a large radius gives, with no free
//!    parameter but `T_H`, R³ = `t_KH` m² (T☉ ÷ `T_H`)⁴ ÷ (7t) in solar units, `t_KH` = GM☉² ÷
//!    (R☉L☉) the Sun's Kelvin–Helmholtz time (31.4 Myr), and L = R² (`T_H` ÷ T☉)⁴ ∝ t^(−2/3).
//!    `T_H` = 3,020 K ×
//!    (m ÷ 0.1 M☉)^0.09 is fitted to Baraffe et al.'s (2015, A&A 577, A42; "BHAC15") tracks at
//!    10–300 Myr: the fit's radii are within 4.3% of theirs at 30–300 Myr for 0.05–0.1 M☉ (the
//!    law alone within 1–4% at 20–120 Myr for 0.07–0.1 M☉), and at 0.09 M☉ and 0.1 Gyr it gives
//!    0.176 R☉ and log L −2.677 against their 0.176 and −2.68.
//! 2. **Degenerate cooling.** Once electron degeneracy halts the contraction, an object below the
//!    hydrogen-burning limit cools on Burrows, Hubbard, Lunine and Liebert's (2001, Rev. Mod.
//!    Phys. 73, 719) power laws: their equation 1, L = 4 × 10⁻⁵ L☉ (t ÷ 1 Gyr)^−1.3 (m ÷
//!    0.05 M☉)^2.64 κ̂^0.35, and the radius of their equation 5, R = 6.7 × 10⁴ km (g ÷ 10⁵ cm
//!    s⁻²)^−0.18 (T ÷ 1,000 K)^0.11, with the gravity of their equation 3 and the temperature of
//!    their equation 2, T = 1,550 K (t ÷ 1 Gyr)^−0.32 (m ÷ 0.05 M☉)^0.83 κ̂^0.088. κ̂ is their
//!    atmospheric Rosseland opacity over 10⁻² cm² g⁻¹, read here as Z ÷ Z☉ above a metal-free
//!    floor κ̂₀ = 4^(−1/0.35) = 0.0190, so that at zero metallicity L is a quarter of the solar
//!    value, as their text after equation 5 says. The contraction and the cooling are joined by a
//!    p-norm of order 4: the smaller luminosity and the larger radius.
//! 3. **The hydrogen-burning floor.** Above the hydrogen-burning limit `m_e(Z)` nuclear burning
//!    halts the contraction on the main sequence. Its radius and temperature, `R_hb` and `T_hb`,
//!    are pinned at 0.1 M☉ to the backbone's zero-age main sequence at the same metallicity (Tout
//!    et al. 1996 through [`zams`], with Hurley, Pols and Tout's equation 24 radius floor, which
//!    binds there), so that the state is continuous in mass with the backbone at 0.1 M☉; below
//!    it they follow BHAC15's 10 Gyr isochrone: `R_hb` = R₀.₁ (m ÷ 0.1)^1.2 x^0.05 and `T_hb` =
//!    T₀.₁ x^0.18, with x = (m − `m_e`) ÷ (0.1 M☉ − `m_e`), which reaches zero weight at the
//!    limit. The object's radius and temperature are p-norms of order 20 of the two branches'
//!    (the larger of each), and its luminosity is 4πR²σT⁴.
//!
//! Joining radius and temperature, rather than luminosity and radius as plan 06's P06.T13
//! sketches, makes every quantity monotone by construction: temperature, radius and luminosity
//! never rise with age at a fixed mass, and temperature never falls with mass at a fixed age.
//! With luminosity and radius joined instead, the temperature of a star settling onto the main
//! sequence rose by up to 200 K at Z = 10⁻⁴ once its luminosity had reached the floor and its
//! radius had not.
//!
//! The limit follows Burrows et al.'s equation 7, `m_e` ∝ κ̂^(−1/9), calibrated at its two ends:
//! 0.068 M☉ at solar metallicity, where the floor's weight must vanish for BHAC15's 0.07 M☉
//! model, their stated hydrogen-burning limit, to keep burning at 10 Gyr, and 0.083 M☉ at the
//! metal-poor end. Baraffe et al.'s (1997, A&A 327, 1054) Tables II–V start at 0.083 M☉, which
//! they call the limit, at every \[M/H\] from −2.0 to −1.0; at −2.0 that model is at the edge
//! (1,779 K, log L −4.27 at 10 Gyr), but at −1.0 it still has 2,359 K and log L −3.66, so the
//! limit there lies lower, and the fit's 0.079 M☉ at −1.0 gives their 0.083 M☉ model a
//! luminosity within 0.12 dex of theirs, relative to 0.1 M☉. The opacity of this relation is Z ÷ Z☉ above its own metal-free floor, (0.068 ÷ 0.083)⁹,
//! since the equation's κ is another "ersatz for metallicity" than equation 1's. So `m_e` runs
//! from 0.065 M☉ at Z = 0.03 through 0.068 (solar), 0.079 (\[M/H\] = −1) and 0.083 at the
//! formulae's floor of Z = 10⁻⁴ ([`hydrogen_burning_limit`]).
//!
//! # Against the models
//!
//! At solar metallicity, against BHAC15's isochrones at 0.05, 0.075, 0.08 and 0.09 M☉ and 0.1, 1
//! and 10 Gyr, the fit is within 120 K in effective temperature and 0.125 dex in luminosity (the
//! plan's 150 K and 0.15 dex); BHAC15's grid stops at 1,300 K, so the 0.05 M☉ point at 10 Gyr is
//! taken from the same group's ATMO 2020 models (Phillips et al. 2020, A&A 637, A38), which the
//! fit meets within 23 K and 0.028 dex. The largest residual in luminosity, 0.125 dex at 0.05 M☉
//! and 1 Gyr, is Burrows et al.'s equation 1 itself, which ATMO 2020 matches to 0.02 dex there.
//! Over BHAC15's whole grid from 30 Myr to 10 Gyr and 0.03–0.1 M☉ it is within 190 K and 0.16
//! dex, and against ATMO 2020 from 30 Myr to 10 Gyr and 0.02–0.075 M☉ within 210 K and 0.16 dex,
//! except at 0.07–0.075 M☉ beyond 3 Gyr, where ATMO 2020 puts the hydrogen-burning limit higher
//! (its 0.07 M☉ object is a brown dwarf, BHAC15's a feeble star). The metal-poor floors follow Baraffe et al. (1997) in luminosity to
//! 0.2 dex and are 230–440 K cooler than theirs, because the backbone's radius at 0.1 M☉ and
//! Z = 10⁻⁴ is 0.143 R☉, not their 0.108 (HPT's equation 24 floor), and the pin carries it down.
//!
//! # What is not modelled
//!
//! - Deuterium burning, which holds objects above 13 Jupiter masses brighter for a few to a
//!   hundred Myr (Burrows et al. 2001, §II): the fit is up to 0.6 dex faint there against ATMO
//!   2020 (0.53 dex at 0.015 M☉ and 30 Myr, 0.58 at 0.013 M☉ and 0.1 Gyr). Plan 13 may add it (its P13.T5.a).
//! - The pre-main sequence above 0.1 M☉: until P06.T15.b gives the backbone a contraction phase,
//!   the fit's contraction at 0.1 M☉ meets the backbone's zero-age main sequence only once it has
//!   finished: 0.42 dex brighter at 0.1 Gyr (as BHAC15's 0.1 M☉ star is, 0.41 dex above the
//!   backbone's zero-age value), 0.10 at 0.3 Gyr, 0.03 at 0.5 Gyr and 0.45% at 1 Gyr at solar
//!   metallicity. P06.T15.b's Hayashi segment should use this module's contraction law so that
//!   the two meet at every age.
//! - Winds, rotation and any change of mass.

use core::fmt;
use std::error::Error;

use crate::math;
use crate::stellar::composition::Z_SOLAR;
use crate::stellar::sse::{ZCoeffs, zams};
use crate::stellar::{Composition, Phase, StarState, StarStateParts};
use crate::units::consts::{
    GM_JUPITER, GM_SUN, SECONDS_PER_JULIAN_YEAR, SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_LUMINOSITY_W,
    SOLAR_MASS_KG, SOLAR_RADIUS_M,
};
use crate::units::{
    MetalFraction, SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years,
};

pub mod giant;

pub use giant::{
    CoolingState, EvaluateGiantCoolingError, GIANT_MAX_MASS, GIANT_MIN_MASS, giant_cooling,
};

/// The lowest mass the fits cover, 0.01 M☉ (10.5 Jupiter masses): the lower end of the range
/// plan 06 gives Burrows et al.'s power laws, below which plan 13's `giant_cooling` takes over.
pub const MIN_MASS: SolarMasses = SolarMasses::new(0.01);

/// The highest mass the fits cover, 0.1 M☉, where they meet the backbone's zero-age main
/// sequence (Hurley, Pols and Tout 2000 cover 0.1–100 M☉).
pub const MAX_MASS: SolarMasses = SolarMasses::new(0.1);

/// The youngest age the fits resolve, 1 Myr: a younger object has the state of a 1 Myr one.
///
/// BHAC15's and ATMO 2020's tracks start at 0.5–1 Myr, and plan 13's giant-planet fit at 1 Myr.
/// The contraction law diverges as t → 0, where the object is still a protostar gathering its
/// mass, which this fit does not model.
pub const MIN_AGE: Years = Years::new(1.0e6);

/// Why [`cooling`] has no state for its arguments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvaluateCoolingError {
    /// The mass is outside [`MIN_MASS`]–[`MAX_MASS`], or not a number. Above 0.1 M☉ the backbone
    /// ([`sse`](crate::stellar::sse)) gives the state; below 0.01 M☉ plan 13's giant-planet fit.
    MassOutsideFits(SolarMasses),
    /// The age is negative or not finite: an object has no state before it forms.
    AgeOutsideLife(Years),
}

impl fmt::Display for EvaluateCoolingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassOutsideFits(m) => write!(
                f,
                "a mass of {} M☉ is outside the cooling fits' {} to {} M☉",
                m.value(),
                MIN_MASS.value(),
                MAX_MASS.value()
            ),
            Self::AgeOutsideLife(t) => {
                write!(f, "an age of {} years is negative or not finite", t.value())
            }
        }
    }
}

impl Error for EvaluateCoolingError {}

/// The state of an object of `mass` at `age` with composition `comp` on the cooling fits below
/// 0.1 M☉: a brown dwarf, or above the hydrogen-burning limit one of the latest M dwarfs.
///
/// `mass` is in M☉ from [`MIN_MASS`] to [`MAX_MASS`] inclusive, and `age` in Julian years since the
/// onset of collapse (plan 06, design note 4), below [`MIN_AGE`] held at its state then. The fits
/// read the metal fraction the backbone reads, [`Composition::z_fit`]. The model, its sources and
/// its residuals against the stellar models are in the [module documentation](self).
///
/// The returned [`StarState`] is in [`Phase::Substellar`] at every age, since such an object never
/// leaves it within any age the fields draw, whether it burns hydrogen or not
/// ([`hydrogen_burning_limit`] tells which): its mass is `mass` for good (no wind), all of it
/// envelope (no core), its mass-loss rate zero and its phase fraction zero, since the phase has no
/// end to measure towards. The effective temperature is `StarState`'s own, from the luminosity and
/// radius returned.
///
/// This is where the backbone's `evolve` (P06.T10.e) is to dispatch below 0.1 M☉, a seam wired
/// when the two are merged (ruling 33 of 2026-09-22): at 0.1 M☉ itself the state is within 1% in
/// luminosity and 0.03% in radius of the backbone's zero-age main sequence at every metallicity
/// from 1 Gyr on, the backbone taking over from there.
///
/// # Errors
///
/// - [`EvaluateCoolingError::MassOutsideFits`] if `mass` is below 0.01 or above 0.1 M☉, or NaN.
/// - [`EvaluateCoolingError::AgeOutsideLife`] if `age` is negative or not finite.
///
/// # Examples
///
/// A brown dwarf of 50 Jupiter masses is a late M dwarf in its first hundred million years, an L
/// dwarf at a gigayear and a T dwarf by five, while a star of 0.09 M☉ settles on the main
/// sequence and stays there:
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::substellar::{EvaluateCoolingError, cooling};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let sun = Composition::SOLAR;
/// let teff = |m: f64, gyr: f64| -> Result<f64, EvaluateCoolingError> {
///     let state = cooling(SolarMasses::new(m), Years::new(gyr * 1e9), &sun)?;
///     Ok(state.effective_temperature().value())
/// };
/// assert!(teff(0.048, 0.1)? > 2_310.0); // M
/// assert!((1_310.0..2_310.0).contains(&teff(0.048, 1.0)?)); // L
/// assert!(teff(0.048, 5.0)? < 1_300.0); // T
/// assert!((teff(0.09, 10.0)? - teff(0.09, 3.0)?).abs() < 5.0);
/// // Above 0.1 M☉ the backbone answers.
/// assert_eq!(
///     cooling(SolarMasses::new(0.2), Years::new(1e9), &sun),
///     Err(EvaluateCoolingError::MassOutsideFits(SolarMasses::new(0.2)))
/// );
/// # Ok::<(), EvaluateCoolingError>(())
/// ```
pub fn cooling(
    mass: SolarMasses,
    age: Years,
    comp: &Composition,
) -> Result<StarState, EvaluateCoolingError> {
    let m = mass.value();
    if !(MIN_MASS.value()..=MAX_MASS.value()).contains(&m) {
        return Err(EvaluateCoolingError::MassOutsideFits(mass));
    }
    let t = age.value();
    if !(t.is_finite() && t >= 0.0) {
        return Err(EvaluateCoolingError::AgeOutsideLife(age));
    }
    let ln = ln_state(m, t.max(MIN_AGE.value()), comp.z_fit());
    Ok(StarState::new(StarStateParts {
        phase: Phase::Substellar,
        age,
        mass,
        core_mass: SolarMasses::ZERO,
        luminosity: SolarLuminosities::new(math::exp(ln.luminosity)),
        radius: SolarRadii::new(math::exp(ln.radius)),
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.0,
    }))
}

/// The hydrogen-burning limit at the composition `comp`, M☉: the mass below which the fits give an
/// object no hydrogen-burning floor, so that it cools for ever.
///
/// Burrows et al. (2001) equation 7's `m_e` ∝ κ^(−1/9), calibrated to 0.068 M☉ at solar
/// metallicity and 0.083 M☉ at the metal-poor end (module documentation): 0.065 M☉ at Z = 0.03,
/// 0.079 at \[M/H\] = −1 and 0.083 at Z = 10⁻⁴. An object just above it burns hydrogen feebly, and
/// one of BHAC15's 0.07 M☉ still does at 10 Gyr. Plan 06's P06.T13 gave 0.072–0.078 M☉; Burrows
/// et al. give 0.07–0.075 M☉ at solar metallicity and 0.092 M☉ at none, and Baraffe et al. 0.07
/// (2015, solar) and 0.083 M☉ (1997, \[M/H\] = −2.0, their lowest model from −2.0 to −1.0).
///
/// # Examples
///
/// A halo object of 0.08 M☉ is a brown dwarf; the same mass in the solar neighbourhood is a star:
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::substellar::hydrogen_burning_limit;
/// use hyperion_sim::units::{Dex, HeliumExcess};
///
/// let halo = Composition::from_fe_h(Dex::new(-1.5), HeliumExcess::ZERO);
/// assert!(hydrogen_burning_limit(&halo).value() > 0.08);
/// assert!(hydrogen_burning_limit(&Composition::SOLAR).value() < 0.08);
/// ```
#[must_use]
pub fn hydrogen_burning_limit(comp: &Composition) -> SolarMasses {
    SolarMasses::new(edge_mass(comp.z_fit()))
}

/// The contraction's Hayashi temperature at 0.1 M☉, K (fitted to BHAC15; module documentation).
const HAYASHI_T_AT_TENTH: f64 = 3_020.0;

/// The Hayashi temperature's power of mass, `T_H` ∝ m^0.09.
const HAYASHI_T_MASS_POWER: f64 = 0.09;

/// The Sun's Kelvin–Helmholtz time GM☉² ÷ (R☉ L☉) divided by the polytrope's 7, in Julian years:
/// (R ÷ R☉)³ = this × m² (T☉ ÷ T)⁴ ÷ t, with the IAU 2015 nominal solar values.
const KELVIN_HELMHOLTZ_OVER_7_YR: f64 =
    GM_SUN * SOLAR_MASS_KG / (SOLAR_RADIUS_M * SOLAR_LUMINOSITY_W) / 7.0 / SECONDS_PER_JULIAN_YEAR;

/// Burrows et al. (2001) equation 1: L at 1 Gyr, 0.05 M☉ and κ̂ = 1, L☉.
const BURROWS_L: f64 = 4.0e-5;

/// Equation 1's powers of age, mass and opacity.
const BURROWS_L_POWERS: [f64; 3] = [-1.3, 2.64, 0.35];

/// Burrows et al. (2001) equation 2: T at 1 Gyr, 0.05 M☉ and κ̂ = 1, in units of 1,000 K.
const BURROWS_T_KK: f64 = 1.55;

/// Equation 2's powers of age, mass and opacity.
const BURROWS_T_POWERS: [f64; 3] = [-0.32, 0.83, 0.088];

/// Burrows et al. (2001) equation 3: the mass at g = 10⁵ cm s⁻² and 1,000 K, 35 Jupiter masses, in
/// M☉ (the IAU 2015 nominal GM of Jupiter and the Sun).
const BURROWS_G_MASS: f64 = 35.0 * GM_JUPITER / GM_SUN;

/// Equation 3's powers of gravity and temperature: M ∝ g^0.64 T^0.23.
const BURROWS_G_POWERS: [f64; 2] = [0.64, 0.23];

/// Burrows et al. (2001) equation 5: R at g = 10⁵ cm s⁻² and 1,000 K, 6.7 × 10⁴ km, in R☉.
const BURROWS_R: f64 = 6.7e7 / SOLAR_RADIUS_M;

/// Equation 5's powers of gravity and temperature: R ∝ g^−0.18 T^0.11.
const BURROWS_R_POWERS: [f64; 2] = [-0.18, 0.11];

/// The metal-free floor of the cooling laws' opacity, κ̂₀ = 4^(−1/0.35): Burrows et al.'s text
/// after equation 5 divides the solar luminosity by about 4 at zero metallicity.
const COOLING_OPACITY_FLOOR: f64 = 0.019_047_088_346_944_928;

/// The hydrogen-burning limit at solar metallicity, M☉ (module documentation).
const EDGE_MASS_SOLAR: f64 = 0.068;

/// The metal-free floor of the limit's opacity, (0.068 ÷ 0.083)⁹, which puts the limit at
/// 0.083 M☉ as Z → 0 (Baraffe et al. 1997).
const EDGE_OPACITY_FLOOR: f64 = 0.166_294_307_515_195_78;

/// The floor's radius power of mass and its power of x: `R_hb` = R₀.₁ (m ÷ 0.1)^1.2 x^0.05.
const FLOOR_R_POWERS: [f64; 2] = [1.2, 0.05];

/// The floor's temperature power of x: `T_hb` = T₀.₁ x^0.18.
const FLOOR_T_POWER: f64 = 0.18;

/// The order of the p-norm between contraction and degenerate cooling.
const DEGENERATE_ORDER: f64 = 4.0;

/// The order of the p-norm between the cooling branch and the hydrogen-burning floor.
const FLOOR_ORDER: f64 = 20.0;

/// ln of the smooth maximum (aᵖ + bᵖ)^(1/p) of a = e^`ln_a` and b = e^`ln_b`, formed from the
/// larger so that it never overflows.
#[must_use]
fn ln_soft_max(ln_a: f64, ln_b: f64, p: f64) -> f64 {
    let (hi, lo) = if ln_a >= ln_b {
        (ln_a, ln_b)
    } else {
        (ln_b, ln_a)
    };
    hi + math::ln_1p(math::exp(-p * (hi - lo))) / p
}

/// ln of the smooth minimum (a⁻ᵖ + b⁻ᵖ)^(−1/p) of a = e^`ln_a` and b = e^`ln_b`.
#[must_use]
fn ln_soft_min(ln_a: f64, ln_b: f64, p: f64) -> f64 {
    -ln_soft_max(-ln_a, -ln_b, p)
}

/// The opacity ratio κ̂ = floor + (1 − floor) Z ÷ Z☉ of a relation with metal-free `floor`.
#[must_use]
fn opacity(z: MetalFraction, floor: f64) -> f64 {
    floor + (1.0 - floor) * (z / Z_SOLAR)
}

/// The hydrogen-burning limit at `z`, M☉: 0.068 κ̂^(−1/9) (Burrows et al. 2001, equation 7).
#[must_use]
fn edge_mass(z: MetalFraction) -> f64 {
    EDGE_MASS_SOLAR * math::exp(-math::ln(opacity(z, EDGE_OPACITY_FLOOR)) / 9.0)
}

/// The natural logarithms of one state of the model: luminosity in L☉ and radius in R☉, which fix
/// the effective temperature by Stefan–Boltzmann.
#[derive(Debug, Clone, Copy)]
struct LnState {
    luminosity: f64,
    radius: f64,
}

impl LnState {
    /// The state of radius e^`radius` R☉ and effective temperature e^`temperature` K:
    /// L = R² (T ÷ T☉)⁴.
    #[must_use]
    fn of_radius_and_temperature(radius: f64, temperature: f64) -> Self {
        Self {
            luminosity: 2.0 * radius + 4.0 * (temperature - ln_solar_temperature()),
            radius,
        }
    }

    /// ln of the effective temperature, K: T = T☉ L^¼ R^−½.
    #[must_use]
    fn temperature(self) -> f64 {
        ln_solar_temperature() + 0.25 * self.luminosity - 0.5 * self.radius
    }
}

/// ln T☉, K.
#[must_use]
fn ln_solar_temperature() -> f64 {
    math::ln(SOLAR_EFFECTIVE_TEMPERATURE_K)
}

/// The state of an object of `m` M☉ at `t` years (at least 1 Myr) and metal fraction `z`: the
/// model of the module documentation.
#[must_use]
fn ln_state(m: f64, t: f64, z: MetalFraction) -> LnState {
    let ln_tenth = math::ln(m / MAX_MASS.value());
    let cool = cooling_branch(m, t, z, ln_tenth);
    let edge = edge_mass(z);
    let x = ((m - edge) / (MAX_MASS.value() - edge)).min(1.0);
    if x <= 0.0 {
        return cool;
    }
    let pin = backbone_at_tenth(z);
    let ln_x = math::ln(x);
    let [radius_mass, radius_x] = FLOOR_R_POWERS;
    let floor_radius = pin.radius + radius_mass * ln_tenth + radius_x * ln_x;
    let floor_temperature = pin.temperature() + FLOOR_T_POWER * ln_x;
    LnState::of_radius_and_temperature(
        ln_soft_max(cool.radius, floor_radius, FLOOR_ORDER),
        ln_soft_max(cool.temperature(), floor_temperature, FLOOR_ORDER),
    )
}

/// The branch without hydrogen burning: contraction on the Hayashi track joined to Burrows et
/// al.'s degenerate cooling, the smaller luminosity and the larger radius. `ln_tenth` is
/// ln(m ÷ 0.1 M☉).
#[must_use]
fn cooling_branch(m: f64, t: f64, z: MetalFraction, ln_tenth: f64) -> LnState {
    let hayashi = math::ln(HAYASHI_T_AT_TENTH) + HAYASHI_T_MASS_POWER * ln_tenth;
    let contraction = LnState::of_radius_and_temperature(
        (math::ln(KELVIN_HELMHOLTZ_OVER_7_YR / t)
            + 2.0 * math::ln(m)
            + 4.0 * (ln_solar_temperature() - hayashi))
            / 3.0,
        hayashi,
    );
    let degenerate = burrows(m, t, z);
    LnState {
        luminosity: ln_soft_min(
            contraction.luminosity,
            degenerate.luminosity,
            DEGENERATE_ORDER,
        ),
        radius: ln_soft_max(contraction.radius, degenerate.radius, DEGENERATE_ORDER),
    }
}

/// Burrows et al.'s (2001) degenerate cooling: the luminosity of their equation 1, and the radius
/// of their equation 5 at the gravity of equation 3 and the temperature of equation 2.
#[must_use]
fn burrows(m: f64, t: f64, z: MetalFraction) -> LnState {
    let logs = [
        math::ln(t / 1.0e9),
        math::ln(m / 0.05),
        math::ln(opacity(z, COOLING_OPACITY_FLOOR)),
    ];
    let luminosity = power_law(BURROWS_L, BURROWS_L_POWERS, logs);
    let kilokelvin = power_law(BURROWS_T_KK, BURROWS_T_POWERS, logs);
    let [g_gravity, g_temperature] = BURROWS_G_POWERS;
    let gravity = (math::ln(m / BURROWS_G_MASS) - g_temperature * kilokelvin) / g_gravity;
    let [r_gravity, r_temperature] = BURROWS_R_POWERS;
    LnState {
        luminosity,
        radius: math::ln(BURROWS_R) + r_gravity * gravity + r_temperature * kilokelvin,
    }
}

/// ln of `coefficient` × (t ÷ 1 Gyr)^a (m ÷ 0.05 M☉)^b κ̂^c, for `powers` [a, b, c] and `logs`
/// the logarithms of the three ratios, summed in that order.
#[must_use]
fn power_law(coefficient: f64, powers: [f64; 3], logs: [f64; 3]) -> f64 {
    let [age, mass, kappa] = logs;
    math::ln(coefficient) + powers[0] * age + powers[1] * mass + powers[2] * kappa
}

/// The backbone's zero-age main-sequence star of 0.1 M☉ at `z`.
///
/// L is Tout et al.'s (1996) [`zams::luminosity`]. R is the larger of their [`zams::radius`] and
/// Hurley, Pols and Tout's (2000) equation 24 floor for partly degenerate stars,
/// 0.0258 (1 + X)^(5/3) M^(−1/3) R☉ with X = 0.76 − 3Z, which binds at 0.1 M☉ at every
/// metallicity: the radius the backbone's main sequence starts from (plan 06, "Deviations in T5,
/// as built"), in the same arithmetic as `sse::ms`'s, whose type this module cannot reach; a test
/// there holds the two to the same bits.
#[must_use]
fn backbone_at_tenth(z: MetalFraction) -> LnState {
    let (luminosity, radius) = backbone_zams_at_tenth(z);
    LnState {
        luminosity: math::ln(luminosity.value()),
        radius: math::ln(radius.value()),
    }
}

/// The luminosity and radius of [`backbone_at_tenth`], as the backbone's main sequence starts:
/// `sse::ms`'s tests check them bit for bit against `MainSequence::at` at age zero.
#[must_use]
pub(crate) fn backbone_zams_at_tenth(z: MetalFraction) -> (SolarLuminosities, SolarRadii) {
    let coeffs = ZCoeffs::new(z);
    let tenth = MAX_MASS;
    let luminosity = zams::luminosity(tenth, &coeffs);
    let hydrogen = 0.76 - 3.0 * coeffs.z().value();
    let degenerate = 0.0258 * math::powf(1.0 + hydrogen, 5.0 / 3.0) / math::cbrt(tenth.value());
    let radius = zams::radius(tenth, &coeffs).value().max(degenerate);
    (luminosity, SolarRadii::new(radius))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::units::{Dex, HeliumExcess};

    /// A composition with metal fraction `z`.
    fn with_z(z: f64) -> Composition {
        Composition::from_fe_h(Dex::new(math::log10(z / 0.02)), HeliumExcess::ZERO)
    }

    fn state(m: f64, gyr: f64, comp: &Composition) -> StarState {
        cooling(SolarMasses::new(m), Years::new(gyr * 1e9), comp).expect("inside the fits")
    }

    /// Metal fractions across the formulae's range, 10⁻⁴ to 0.03.
    const METAL_FRACTIONS: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    /// The stellar models' states the fit is checked against, at solar metallicity: (M☉, Gyr,
    /// effective temperature in K, log₁₀ L in L☉, source).
    ///
    /// BHAC15: Baraffe, Homeier, Allard and Chabrier (2015, A&A 577, A42), the isochrones
    /// `BHAC15_iso.2mass` at 0.1, 1 and 10 Gyr and, for 0.05 M☉ at 1 Gyr, where the isochrone
    /// stops, the track `BHAC15_tracks+structure` at log t = 9.000569 (both from
    /// perso.ens-lyon.fr/isabelle.baraffe/BHAC15dir, retrieved 2026-09-23). BHAC15 has no 0.05 M☉
    /// model at 10 Gyr (its tracks stop at 1,300 K), so that point is ATMO 2020's
    /// chemical-equilibrium track (Phillips et al. 2020, A&A 637, A38; `0.05_ATMO_CEQ_vega.txt`),
    /// the same group's extension of BHAC15 to cold objects.
    const REFERENCE: [(f64, f64, f64, f64, &str); 12] = [
        (0.05, 0.1, 2_525.0, -3.15, "BHAC15"),
        (0.05, 1.0, 1_383.0, -4.523, "BHAC15 track"),
        (0.05, 10.0, 782.3, -5.670, "ATMO 2020"),
        (0.075, 0.1, 2_839.0, -2.81, "BHAC15"),
        (0.075, 1.0, 2_243.0, -3.67, "BHAC15"),
        (0.075, 10.0, 2_037.0, -3.89, "BHAC15"),
        (0.08, 0.1, 2_879.0, -2.76, "BHAC15"),
        (0.08, 1.0, 2_400.0, -3.51, "BHAC15"),
        (0.08, 10.0, 2_345.0, -3.58, "BHAC15"),
        (0.09, 0.1, 2_942.0, -2.68, "BHAC15"),
        (0.09, 1.0, 2_648.0, -3.25, "BHAC15"),
        (0.09, 10.0, 2_646.0, -3.25, "BHAC15"),
    ];

    #[test]
    fn it_matches_baraffe_et_al_2015_within_150_k_and_0_15_dex() {
        let sun = Composition::SOLAR;
        let (mut worst_t, mut worst_l) = (0.0_f64, 0.0_f64);
        for (m, gyr, teff, log_l, source) in REFERENCE {
            let s = state(m, gyr, &sun);
            let d_t = s.effective_temperature().value() - teff;
            let d_l = math::log10(s.luminosity().value()) - log_l;
            eprintln!("{m} M☉ at {gyr} Gyr: ΔT = {d_t:+.0} K, Δlog L = {d_l:+.3} ({source})");
            assert!(
                d_t.abs() <= 150.0,
                "{m} M☉ at {gyr} Gyr: T_eff off by {d_t} K"
            );
            assert!(d_l.abs() <= 0.15, "{m} M☉ at {gyr} Gyr: log L off by {d_l}");
            worst_t = worst_t.max(d_t.abs());
            worst_l = worst_l.max(d_l.abs());
        }
        eprintln!("worst: {worst_t:.0} K, {worst_l:.3} dex");
    }

    /// Spectral classes by effective temperature, from Pecaut and Mamajek's (2013, ApJS 208, 9)
    /// dwarf sequence as updated (`EEM_dwarf_UBVIJHK_colors_Teff.txt`, version 2022.04.16): M9.5V
    /// 2,350 K and L0V 2,270 K, so M above 2,310 K; L9V 1,370 K and T0V 1,255 K, so T below
    /// 1,310 K.
    const M_L_BOUNDARY_K: f64 = 2_310.0;
    const L_T_BOUNDARY_K: f64 = 1_310.0;

    /// Plan 06 has "2,800 K at 0.1 Gyr"; BHAC15 put a 0.05 M☉ object at 2,525 K then (ATMO 2020,
    /// 2,548 K), a late M dwarf, and at 2,800 K only at 10–25 Myr.
    #[test]
    fn a_brown_dwarf_of_0_05_passes_through_m_l_and_t() {
        let sun = Composition::SOLAR;
        let t = |gyr: f64| state(0.05, gyr, &sun).effective_temperature().value();
        assert!(t(0.1) > M_L_BOUNDARY_K, "M at 0.1 Gyr: {}", t(0.1));
        assert!(
            (L_T_BOUNDARY_K..M_L_BOUNDARY_K).contains(&t(1.0)),
            "L at 1 Gyr: {}",
            t(1.0)
        );
        assert!(t(5.0) < 1_300.0, "T by 5 Gyr: {}", t(5.0));
        assert!(t(0.02) > 2_700.0, "{}", t(0.02));
        eprintln!(
            "0.05 M☉: {:.0} K at 20 Myr, {:.0} K at 0.1 Gyr, {:.0} K at 1 Gyr, {:.0} K at 5 Gyr",
            t(0.02),
            t(0.1),
            t(1.0),
            t(5.0)
        );
        // The first gigayear of cooling, then the long fall: never a Y dwarf (below about 480 K,
        // T9.5V 510 K, Y0V 450 K) within the fields' ages.
        assert!(t(13.8) > 600.0);
    }

    #[test]
    fn it_is_continuous_in_mass_with_the_backbone_at_0_1() {
        for z in METAL_FRACTIONS {
            let comp = with_z(z);
            let pin = backbone_at_tenth(comp.z_fit());
            for gyr in [1.0, 10.0] {
                let s = state(0.1, gyr, &comp);
                let d_l = s.luminosity().value() / math::exp(pin.luminosity) - 1.0;
                let d_r = s.radius().value() / math::exp(pin.radius) - 1.0;
                eprintln!("Z = {z} at {gyr} Gyr: L {d_l:+.5}, R {d_r:+.6}");
                // The plan asks for 2% in L; the documentation claims 1% in L and 0.02% in R.
                assert!(d_l.abs() < 0.01, "Z = {z}, {gyr} Gyr: L off by {d_l}");
                assert!(d_r.abs() < 3e-4, "Z = {z}, {gyr} Gyr: R off by {d_r}");
                // The approach from below is continuous.
                let below = state(0.1 - 1e-9, gyr, &comp);
                assert!((below.luminosity() / s.luminosity() - 1.0).abs() < 1e-6);
            }
        }
        // The pin is the backbone's: the luminosity of Tout et al. (1996), and the radius of HPT's
        // equation 24, which binds at 0.1 M☉.
        let pin = backbone_at_tenth(MetalFraction::new(0.02));
        let c = ZCoeffs::new(MetalFraction::new(0.02));
        let zams_l = zams::luminosity(MAX_MASS, &c).value();
        assert!((math::exp(pin.luminosity) / zams_l - 1.0).abs() < 1e-14);
        let radius = math::exp(pin.radius);
        assert!((radius - 0.1346).abs() < 5e-5, "{radius}");
        assert!(radius > zams::radius(MAX_MASS, &c).value());
    }

    /// The fields' ages run to 13.8 Gyr; the sweep runs past them.
    fn ages() -> impl Iterator<Item = f64> {
        (0..=300).map(|i| 1e6 * math::exp10(f64::from(i) * 4.3 / 300.0))
    }

    fn masses() -> impl Iterator<Item = f64> {
        (0..=90).map(|i| 0.01 * math::exp10(f64::from(i) / 90.0))
    }

    #[test]
    fn temperature_radius_and_luminosity_fall_with_age_and_temperature_rises_with_mass() {
        for z in [1e-4, 2e-3, 0.02, 0.03] {
            let comp = with_z(z);
            for m in masses() {
                let mut previous: Option<StarState> = None;
                for t in ages() {
                    let s = cooling(SolarMasses::new(m), Years::new(t), &comp).unwrap();
                    if let Some(p) = previous {
                        let (tp, tn) = (
                            p.effective_temperature().value(),
                            s.effective_temperature().value(),
                        );
                        assert!(
                            tn <= tp * (1.0 + 1e-12),
                            "T rises: {m} M☉ at {t} yr, Z = {z}"
                        );
                        assert!(s.radius().value() <= p.radius().value() * (1.0 + 1e-12));
                        assert!(s.luminosity().value() <= p.luminosity().value() * (1.0 + 1e-12));
                    }
                    previous = Some(s);
                }
            }
            for t in ages().step_by(10) {
                let mut previous = 0.0;
                for i in 0..=600 {
                    let m = 0.01 * math::exp10(f64::from(i) / 600.0);
                    let teff = cooling(SolarMasses::new(m), Years::new(t), &comp)
                        .unwrap()
                        .effective_temperature()
                        .value();
                    assert!(
                        teff >= previous * (1.0 - 1e-12),
                        "T falls with mass: {m} M☉ at {t} yr"
                    );
                    previous = teff;
                }
            }
        }
    }

    /// Continuity in age: in log–log the luminosity and radius change by at most twice and a third
    /// of the change of age, the steepest slopes of the laws joined (L ∝ t^(−2/3) and t^(−1.3), T
    /// at most as t^(−0.32), R at most as t^(−1/3)), so no step anywhere; and they are constant
    /// below 1 Myr.
    #[test]
    fn it_is_continuous_in_age() {
        for z in [1e-4, 0.02] {
            let comp = with_z(z);
            for m in masses().step_by(3) {
                let mut previous: Option<(f64, StarState)> = None;
                for i in 0..=2_000 {
                    let t = 1e6 * math::exp10(f64::from(i) * 4.3 / 2_000.0);
                    let s = cooling(SolarMasses::new(m), Years::new(t), &comp).unwrap();
                    if let Some((tp, p)) = previous {
                        let d_ln_t = math::ln(t / tp);
                        let d_ln_l = math::ln(s.luminosity() / p.luminosity()).abs();
                        let d_ln_r = math::ln(s.radius() / p.radius()).abs();
                        assert!(d_ln_l <= 2.0 * d_ln_t + 1e-12, "{m} M☉ at {t} yr: L jumps");
                        assert!(d_ln_r <= d_ln_t / 3.0 + 1e-12, "{m} M☉ at {t} yr: R jumps");
                    }
                    previous = Some((t, s));
                }
                let young = cooling(SolarMasses::new(m), Years::new(2.0e5), &comp).unwrap();
                let at_floor = cooling(SolarMasses::new(m), MIN_AGE, &comp).unwrap();
                assert_same_bits(young.luminosity().value(), at_floor.luminosity().value());
                assert_same_bits(young.radius().value(), at_floor.radius().value());
                assert_same_bits(young.age().value(), 2.0e5);
            }
        }
    }

    #[test]
    fn it_is_continuous_in_mass_across_the_hydrogen_burning_limit() {
        for z in METAL_FRACTIONS {
            let comp = with_z(z);
            let edge = hydrogen_burning_limit(&comp).value();
            for gyr in [0.1, 1.0, 10.0, 13.8] {
                for m in [edge, 0.1 - 1e-6, 0.05, 0.0124] {
                    let at = state(m, gyr, &comp);
                    let near = state(m + 1e-10, gyr, &comp);
                    assert!((near.luminosity() / at.luminosity() - 1.0).abs() < 1e-4);
                    assert!((near.radius() / at.radius() - 1.0).abs() < 1e-4);
                }
            }
        }
    }

    #[test]
    fn it_refuses_masses_and_ages_outside_its_range() {
        let sun = Composition::SOLAR;
        for m in [0.009_999, 0.100_001, f64::NAN, -0.05, 1.0] {
            let mass = SolarMasses::new(m);
            match cooling(mass, Years::new(1e9), &sun) {
                Err(EvaluateCoolingError::MassOutsideFits(got)) => {
                    assert!(got.value().total_cmp(&m).is_eq());
                }
                other => panic!("{m} M☉: {other:?}"),
            }
        }
        for t in [-1.0, f64::INFINITY, f64::NAN] {
            assert!(matches!(
                cooling(SolarMasses::new(0.05), Years::new(t), &sun),
                Err(EvaluateCoolingError::AgeOutsideLife(_))
            ));
        }
        // The ends are inside.
        for m in [MIN_MASS, MAX_MASS] {
            assert!(cooling(m, Years::ZERO, &sun).is_ok());
        }
        assert_eq!(
            EvaluateCoolingError::MassOutsideFits(SolarMasses::new(0.2)).to_string(),
            "a mass of 0.2 M☉ is outside the cooling fits' 0.01 to 0.1 M☉"
        );
        assert_eq!(
            EvaluateCoolingError::AgeOutsideLife(Years::new(-3.0)).to_string(),
            "an age of -3 years is negative or not finite"
        );
    }

    #[test]
    fn the_state_is_a_substellar_object_that_keeps_its_mass() {
        let s = state(0.03, 2.0, &Composition::SOLAR);
        assert_eq!(s.phase(), Phase::Substellar);
        assert_same_bits(s.mass().value(), 0.03);
        assert_same_bits(s.core_mass().value(), 0.0);
        assert_same_bits(s.envelope_mass().value(), 0.03);
        assert_same_bits(s.mass_loss_rate().value(), 0.0);
        assert_same_bits(s.phase_fraction(), 0.0);
        assert_same_bits(s.age().value(), 2.0e9);
        // The same arguments give the same bits.
        let again = state(0.03, 2.0, &Composition::SOLAR);
        assert_same_bits(s.luminosity().value(), again.luminosity().value());
        assert_same_bits(s.radius().value(), again.radius().value());
    }

    #[test]
    fn old_brown_dwarfs_cool_on_burrows_et_al_s_laws_and_young_objects_contract() {
        let sun = Composition::SOLAR;
        // At 0.05 M☉ and 10 Gyr the object is degenerate and far from the contraction: equation 1.
        let old = state(0.05, 10.0, &sun);
        let eq_1 = 4e-5 * math::powf(10.0, -1.3);
        assert!((old.luminosity().value() / eq_1 - 1.0).abs() < 1e-3);
        // At 5 Myr a 0.09 M☉ star is on its Hayashi track at 3,020 × 0.9^0.09 K, less than 0.5%
        // above it where the p-norm of order 20 feels the floor's 2,590 K.
        let young = state(0.09, 0.005, &sun);
        let hayashi = 3_020.0 * math::powf(0.9, 0.09);
        assert!((young.effective_temperature().value() / hayashi - 1.0).abs() < 5e-3);
        // Lower metallicity, lower opacity: the brown dwarf cools faster, a quarter of the solar
        // luminosity at the metal-free limit, and faster still in the formulae's floor.
        let poor = state(0.05, 10.0, &with_z(1e-4));
        let kappa = COOLING_OPACITY_FLOOR + (1.0 - COOLING_OPACITY_FLOOR) * 0.005;
        assert!(
            (poor.luminosity() / old.luminosity() / math::powf(kappa, 0.35) - 1.0).abs() < 1e-3
        );
    }

    #[test]
    fn the_hydrogen_burning_limit_rises_as_metallicity_falls() {
        let limit = |z: f64| hydrogen_burning_limit(&with_z(z)).value();
        assert!((limit(0.02) - 0.068).abs() < 1e-12);
        // Baraffe et al. (1997): 0.083 M☉ for [M/H] = −2.0 to −1.0.
        let poor = limit(0.02 * math::exp10(-2.0));
        assert!((0.081..0.084).contains(&poor), "{poor}");
        assert!((limit(1e-4) - 0.083).abs() < 1e-3);
        assert!((0.064..0.068).contains(&limit(0.03)));
        let mut previous = f64::INFINITY;
        for i in 0..=100 {
            let z = 1e-4 * math::exp10(f64::from(i) * math::log10(300.0) / 100.0);
            assert!(limit(z) < previous);
            previous = limit(z);
        }
        // Below the limit an object keeps cooling; above it, it settles.
        let sun = Composition::SOLAR;
        let fading = state(0.066, 13.8, &sun).luminosity() / state(0.066, 5.0, &sun).luminosity();
        let settled = state(0.09, 13.8, &sun).luminosity() / state(0.09, 5.0, &sun).luminosity();
        assert!(fading < 0.4, "{fading}");
        assert!(settled > 0.99, "{settled}");
    }

    #[test]
    fn the_fit_constants_are_their_closed_forms() {
        assert!((COOLING_OPACITY_FLOOR / math::powf(4.0, -1.0 / 0.35) - 1.0).abs() < 1e-15);
        assert!((EDGE_OPACITY_FLOOR / math::powf(0.068 / 0.083, 9.0) - 1.0).abs() < 1e-15);
        // The Sun's Kelvin–Helmholtz time is 31.4 Myr.
        assert!((KELVIN_HELMHOLTZ_OVER_7_YR * 7.0 / 3.14e7 - 1.0).abs() < 5e-3);
        // 35 Jupiter masses and 6.7 × 10⁴ km.
        assert!((BURROWS_G_MASS - 0.033_41).abs() < 1e-5);
        assert!((BURROWS_R - 0.096_31).abs() < 1e-5);
    }
}
