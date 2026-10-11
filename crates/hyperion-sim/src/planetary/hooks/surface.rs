//! A body's surface conditions (plan 14, P14.T24.a, with P14.T48.e's signed contrasts): what the
//! layers above read of its air and climate, computed from its derivation at a time and its spin.
//!
//! [`SurfaceConditions::derive`] gives, from plain values ([`SurfaceInputs`]):
//!
//! - the mean surface temperature, P14.T13's;
//! - the **equator–pole contrast**, signed (a warm pole is negative), from the obliquity to the
//!   seasonal orbit, the rotation and the column;
//! - the **day–night contrast**, from the rotation state and the column, stated about the
//!   substellar axis for a world locked 1:1 to its seasonal host;
//! - the surface pressure and the **gases as mole fractions** by substance, largest first, with
//!   their mean molar mass μ and mixed `c_p` ÷ R (signoff-3.md, item 1.1; [`mole_fractions`]);
//! - the stellar flux, the **radiation class** of the hosts' X-ray and ultraviolet flux
//!   ([`RadiationClass`]), and the **liquid-water flag** from water's phase over the climate's
//!   range of temperature ([`liquid_water`]).
//!
//! The surface gravity, which the column reads, is the bulk section's and is not repeated here
//! (P14.T35.e).
//!
//! # The contrasts' model
//!
//! Both contrasts are closed forms of a linear energy balance on the body's mean state, so that
//! they are continuous in every input and need no field. They are the lane's, provisional, each
//! piece cited on its constant; plan 14's Risks ("Deviations in P14.T24.a, as built") records how
//! they compare with the Solar System's bodies.
//!
//! - **Equator–pole**, T(0°) − T(90°), as rendering plan R09 imposes it on its annual field's
//!   `P₂` term (its Design note 8). With no transport each latitude is in radiative balance with
//!   its annual-mean insolation, which relative to the mean S ÷ 4 is (8 ÷ π²) E(sin β) at the
//!   equator and (4 ÷ π) sin β at a pole, at obliquity β, E the complete elliptic integral of
//!   the second kind ([`annual_insolation`]). An eccentric orbit changes only the mean flux, since
//!   by Kepler's second law (a ÷ r)² dt runs uniformly in the sun's longitude. The grey column
//!   scales each to the surface (P14.T13.c's `T_s` = `T_eq` (1 + ¾τ)^¼), so the radiative
//!   contrast is `T_s` (`q_eq`^¼ − `q_pole`^¼). Transport flattens it as North's two-mode diffusive
//!   model does (North 1975, J. Atmos. Sci. 32, 2033; North, Cahalan and Coakley 1981): by
//!   1 + 6D ÷ B, with B = 4F ÷ `T_s` the grey column's radiative restoring at the surface at fixed
//!   τ, F the absorbed mean flux, and 6D the diffusion's on `P₂`. D = `c_p` (`p_s` ÷ g) K f(Ω) w
//!   ÷ R² is the column's heat capacity diffusing at K, fitted on Earth
//!   ([`MERIDIONAL_DIFFUSIVITY_M2_S`]), times a rotation factor in the form R09 takes from Kaspi
//!   and Showman 2015 ([`rotation_transport_factor`]) and half on a dry world
//!   ([`DRY_TRANSPORT_SHARE`]). The sign changes at 53.9° of obliquity, where the equator's and
//!   the poles' insolation are equal.
//! - **Day–night** (Cowan and Agol 2011, ApJ 729, 54, eqs. 4–5, with Koll 2022, ApJ 924, 134).
//!   With a redistribution efficiency ε from 0 (none) to 1 (uniform), the dayside and nightside
//!   temperatures are `T_eq` (8/3 − 5ε/3)^¼ and `T_eq` ε^¼, scaled to the surface by
//!   `T_s` ÷ `T_eq`. Koll's scaling of locked rocky planets' GCMs gives ε = X ÷ (k + X), with
//!   X = τ^⅓ (`p_s` ÷ 1 bar)^⅔ (`T_eq` ÷ 600 K)^−4/3 and k = 2 ([`REDISTRIBUTION_SCALE`]). A
//!   rotating body's contrast is that one damped by the column's storage over the solar day,
//!   1 ÷ √(1 + (ω τ)²), with τ = `c_p` (`p_s` ÷ g) ÷ B the column's radiative time (Showman and
//!   Guillot 2002, A&A 385, 166, eq. 10, at the grey restoring), the linear response of a slab
//!   forced at the solar rate ω.
//!
//! An airless body's column holds nothing, so its contrasts are the radiative ones: the night
//! side's regolith inertia (some 50 J m⁻² K⁻¹ s^−½ on the Moon, from memory) and its internal
//! heat are not modelled, and its night is the radiative limit, 0 K.
//!
//! # Held until the batch's bump
//!
//! T24.a is built in the 21 → 22 batch, whose goldens move once, at its bump in P14.T48.e
//! (plan 14, "Order and parallelism", the bump plan). Until then
//! [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at) publishes every
//! surface section as [`Section::NotModelled`](crate::planetary::record::Section::NotModelled),
//! as version 21 did (`SURFACE_CONDITIONS_VERSION`); every function here is in force.

use core::fmt;
use std::error::Error;

use crate::orbit::KeplerElements;
use crate::planetary::context::XuvHistory;
use crate::planetary::derive::atmosphere::{
    Atmosphere, EARTH_REFERENCE_AGE, Gas, MOLAR_GAS_CONSTANT, PartialPressures,
    REFERENCE_SUN_ZAMS_LUMINOSITY, SurfaceState,
};
use crate::planetary::derive::rotation::{SpinOrbitResonance, SpinState};
use crate::planetary::frames::BodyFixedFrame;
use crate::substance::{
    Phase, SubstanceId, heat_capacity_over_r, mean_molar_mass_g_per_mol, phase_at,
};
use crate::time::UniverseTime;
use crate::units::consts::{METRES_PER_AU, STEFAN_BOLTZMANN};
use crate::units::{
    Kelvin, Metres, MetresPerSecondSquared, Pascals, Radians, Seconds, SolarMasses,
    WattsPerSquareMetre,
};
use crate::{GENERATOR_VERSION, GeneratorVersion, math};

#[cfg(test)]
mod tests;

/// The generator version from which
/// [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at) publishes a body's
/// surface conditions: 22, the 21 → 22 batch's bump (plan 14, "Order and parallelism", the bump
/// plan).
///
/// P14.T24.a is built in that batch, whose goldens move once, at its bump in P14.T48.e, which
/// removes this hold (`the_surface_conditions_are_held_only_until_the_batch_s_bump`).
pub(crate) const SURFACE_CONDITIONS_VERSION: GeneratorVersion = GeneratorVersion::new(22);

/// Whether [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at) publishes
/// a body's surface conditions (P14.T24.a, held until the 21 → 22 batch's bump,
/// [`SURFACE_CONDITIONS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ConditionsHold {
    /// Version 21's: every surface section is not modelled.
    Held,
    /// P14.T24.a's: a body with a surface has its conditions.
    Published,
}

impl ConditionsHold {
    /// The hold of the generator version in force: [`Self::Published`] from
    /// [`SURFACE_CONDITIONS_VERSION`], [`Self::Held`] below it.
    #[must_use]
    pub(crate) fn in_force() -> Self {
        if GENERATOR_VERSION >= SURFACE_CONDITIONS_VERSION {
            Self::Published
        } else {
            Self::Held
        }
    }
}

/// Earth's annual, zonal-mean equator–pole contrast of surface temperature, K: 42, −(3/2) T₂ of
/// the fit T₀ + T₂ `P₂`(sin φ) with the observed T₂ = −28.0 °C (North, Cahalan and Coakley 1981,
/// Rev. Geophys. 19, 91, read in its NASA preprint, NTRS 19810008165; rendering plan R09's
/// Earth-like reference world takes the same figure). The figure
/// [`MERIDIONAL_DIFFUSIVITY_M2_S`] is fitted to.
pub const EARTH_EQUATOR_POLE_CONTRAST_K: f64 = 42.0;

/// The diffusivity at which a column's heat spreads between equator and pole on Earth, m² s⁻¹:
/// 9.4325 × 10⁵, fitted so that Earth's measured state gives [`EARTH_EQUATOR_POLE_CONTRAST_K`]
/// through the module's equator–pole model (`Fitted`; a test refits it).
///
/// Earth's state: 288 K at the surface (NASA's Earth fact sheet), 23.44° of obliquity, an absorbed
/// flux of (1 − 0.294) × 1,361.17 ÷ 4 = 240.25 W m⁻² (the fact sheet's Bond albedo, and
/// [`SOLAR_CONSTANT_W_PER_M2`](crate::units::consts::SOLAR_CONSTANT_W_PER_M2)), a column of
/// 5.1480 × 10¹⁸ kg over 5.100 × 10¹⁴ m², 10,094 kg m⁻² (Trenberth and Smith 2005, J. Climate 18,
/// 864), of dry air's 28.9647 g mol⁻¹ at `c_p` ÷ R = 7 ÷ 2 (the registry's class for N₂ and O₂),
/// and a radius of 6,371.0 km. It gives 6D ÷ B = 0.4238, so that the radiative contrast,
/// 59.8 K, is flattened by 30%.
///
/// It is an effective figure, not a measured one. Siler, Roe and Armour (2018, J. Climate 31,
/// 7481) diffuse Earth's moist static energy at 1.16 × 10⁶ m² s⁻¹, but this one acts on the
/// surface temperature, against a grey restoring that lacks water vapour's feedback (North,
/// Cahalan and Coakley's fit takes B = 2.09 W m⁻² K⁻¹ against this model's 3.34) and under a
/// uniform albedo, which lacks the poles' brighter ice, cloud and low sun. So Earth sits nearer
/// its radiative limit than in their fit (6D ÷ B = 1.86), and the transport's rotation and
/// moisture factors move its contrast less than an idealised GCM's (plan 14's Risks).
pub const MERIDIONAL_DIFFUSIVITY_M2_S: f64 = 9.432_483_9e5;

/// The share of the meridional transport a world without surface liquid keeps: 0.5, halved
/// where no latent heat is carried, as rendering plan R09's energy-balance model takes it (its
/// Design note 8, after Williams and Kasting 1997, Icarus 129, 254, not re-read). Kaspi and
/// Showman (2015, ApJ 804, 60, §3.2 and Fig. 9b) find Earth's dry and latent fluxes "very similar
/// in magnitude", and R09's split of Siler et al.'s diffusivity, about 0.3 dry of 0.7 moist
/// W m⁻² K⁻¹, gives 0.43 (`Adopted`).
pub const DRY_TRANSPORT_SHARE: f64 = 0.5;

/// Earth's rotation period against the stars, s: 86,164.0989, 2π over the nominal rotation rate
/// Ω = 7.292 115 × 10⁻⁵ rad s⁻¹ (IERS Conventions 2010, TN 36, Table 1.1), the rate Ω⊕ that
/// [`rotation_transport_factor`] is relative to. The sidereal day against the equinox is
/// 86,164.0905 s.
pub const EARTH_SIDEREAL_DAY: Seconds = Seconds::new(86_164.098_9);

/// The rotation rates, relative to Earth's, between which [`rotation_transport_factor`] follows
/// Kaspi and Showman's runs, and beyond which it holds its end values: Ω⊕ ÷ 24 to 12 Ω⊕ (Kaspi
/// and Showman 2015, ApJ 804, 60, Fig. 8a, as rendering plan R09's transport law reads them).
pub const ROTATION_TRANSPORT_RANGE: (f64, f64) = (1.0 / 24.0, 12.0);

/// The exponents of the meridional transport in the rotation rate relative to Earth's: −¼ below
/// Earth's rate and −½ above it (rendering plan R09, Design note 8's transport law, inverted from
/// Kaspi and Showman 2015's Fig. 8a in North, Cahalan and Coakley's partition of B and D; medium
/// confidence). In this module's partition the same factors move an Earth's contrast over
/// 31–53 K, against the GCM's 23–77 K.
pub const ROTATION_TRANSPORT_EXPONENTS: (f64, f64) = (-0.25, -0.5);

/// Koll's constant k in the redistribution efficiency ε = X ÷ (k + X): 2, adopted from the
/// 1.2–2.3 he finds for his three test planets (Koll 2022, ApJ 924, 134, §3 and eqs. 9–10, fitted
/// to GCMs of dry, tidally locked rocky planets at `T_eq` of 391–805 K). His k scales as
/// L g ÷ `c_p`, about 0.1–0.3 for a Mars- or Titan-sized body, and his scaling is applied here
/// to rotating and colder worlds too (`Adopted`; plan 14's Risks).
pub const REDISTRIBUTION_SCALE: f64 = 2.0;

/// The surface pressure Koll's X is scaled to, Pa: 1 bar.
pub const REDISTRIBUTION_REFERENCE_PRESSURE: Pascals = Pascals::new(1e5);

/// The equilibrium temperature Koll's X is scaled to: 600 K.
pub const REDISTRIBUTION_REFERENCE_TEMPERATURE: Kelvin = Kelvin::new(600.0);

/// What [`SurfaceConditions::derive`] reads of a body at a time, as plain values.
///
/// The fields carry no invariant the derivation relies on: a gravity, radius or temperature that
/// is not positive gives no column and no contrast, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceInputs<'a> {
    /// The body's atmosphere at the time (P14.T13, with P14.T24.f's gases).
    pub atmosphere: &'a Atmosphere,
    /// Its equilibrium temperature from its hosts' light at the time, at its Bond albedo
    /// (P14.T12): σ `T_eq`⁴ is the mean flux it absorbs.
    pub equilibrium_temperature: Kelvin,
    /// Its mean surface gravity, the bulk section's: the column is `p_s` ÷ g.
    pub surface_gravity: MetresPerSecondSquared,
    /// Its mean radius, the bulk section's.
    pub radius: Metres,
    /// The flux of its hosts' light at it, orbit-averaged, at the time.
    pub stellar_flux: WattsPerSquareMetre,
    /// The X-ray and extreme-ultraviolet flux of its hosts at it, orbit-averaged, at the time
    /// ([`XuvHistory::flux`]).
    pub xuv_flux: WattsPerSquareMetre,
    /// How it turns under its seasonal host.
    pub spin: SeasonalSpin,
}

/// How a body turns under the host that sets its seasons, as its contrasts read it: its
/// obliquity to the seasonal orbit, its sidereal rotation period and its solar day (as rendering
/// plan R09's `Spin` states them).
///
/// The seasonal orbit is the body's own about its star or stars, or its planet's for a moon
/// (`decision-r09-t2.md` item 1); a moon's moves with P14.T56.a's Laplace planes at 22 → 23.
///
/// Plain data with public fields: the contrasts read any values, a period that is not positive
/// and finite holding the slowest rotation's transport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeasonalSpin {
    /// The angle between the spin axis and the seasonal orbit's normal, rad, 0 to π.
    pub obliquity: Radians,
    /// The sidereal rotation period, s, positive: the rate Ω the meridional transport scales with.
    pub rotation_period: Seconds,
    /// The mean solar day, s, positive; `None` for a world locked 1:1 to its seasonal host, whose
    /// hosts stand still in its sky.
    pub solar_day: Option<Seconds>,
}

/// Whose orbit sets a body's seasons ([`SeasonalSpin::of`]): a selector, not rendering plan R09's
/// `SeasonalOrbit`, which states that orbit's eccentricity and period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SeasonsFrom {
    /// The body's own: a planet's, a dwarf planet's or a belt member's about its host.
    Own,
    /// Its planet's about the planet's host: a moon's.
    Planet,
}

impl SeasonalSpin {
    /// The spin at `t` of the body whose frame is `frame`, under the seasonal orbit `seasonal`,
    /// which is `whose`.
    ///
    /// - Its obliquity is the frame's for its own orbit, and for a moon the angle between its pole
    ///   and its planet's orbital normal, which the frame's obliquity to the moon's own orbit is
    ///   not (a Titan in its planet's equator sees its planet's 26.7°).
    /// - Its mean solar day is 2π ÷ |ω ∓ n|, with ω its spin rate at `t` and n the seasonal
    ///   orbit's mean motion, less for a spin within 90° of the orbit's normal and more beyond it,
    ///   since the sun's right ascension turns once a year whatever the tilt (Earth's 24 h from
    ///   its 23.93 h; Venus's 116.75 d from its 243.0 d at 177.4°). A body locked 1:1 to its own
    ///   orbit's host has none.
    ///
    /// # Examples
    ///
    /// A moon's seasons are its planet's: pass the planet's orbit, not the moon's own. A Titan
    /// spinning along its orbit's normal, in an orbit tilted 26.7° from its planet's, sees that
    /// tilt, and turns under the Sun once in about its orbit:
    ///
    /// ```
    /// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// use hyperion_sim::planetary::frames::{BodyFixedFrame, FrameSpin};
    /// use hyperion_sim::planetary::hooks::surface::{SeasonalSpin, SeasonsFrom};
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::consts::{METRES_PER_AU, SECONDS_PER_DAY};
    /// use hyperion_sim::units::{Degrees, GravitationalParameter, Metres, Radians, Seconds, SolarMasses, Years};
    ///
    /// let circular = |a: f64, mu: GravitationalParameter, tilt: Radians| {
    ///     KeplerElements::from_semi_major_axis(Metres::new(a), mu, Eccentricity::new(0.0)?, Orientation::new(tilt, Radians::ZERO, Radians::ZERO)?, Radians::ZERO)
    /// };
    /// let tilt = Radians::from(Degrees::new(26.73));
    /// let saturn = circular(9.58 * METRES_PER_AU, GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)), Radians::ZERO)?;
    /// let titan = circular(1.221_87e9, GravitationalParameter::new(3.793_1e16), tilt)?;
    /// let spin = FrameSpin {
    ///     obliquity: Radians::ZERO,
    ///     pole_azimuth: Radians::ZERO,
    ///     primordial_period: Seconds::new(15.945 * SECONDS_PER_DAY),
    ///     locking_time: Seconds::new(1e10),
    ///     phase_at_epoch: Radians::ZERO,
    /// };
    /// let frame = BodyFixedFrame::new(&titan, &spin, Years::new(4.57e9))?;
    /// let seasons = SeasonalSpin::of(&frame, UniverseTime::EPOCH, &saturn, SeasonsFrom::Planet);
    /// assert!((seasons.obliquity.value() - tilt.value()).abs() < 1e-9);
    /// let day = seasons.solar_day.expect("a moon turns under its star").value() / SECONDS_PER_DAY;
    /// assert!((day - 15.97).abs() < 0.05);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn of(
        frame: &BodyFixedFrame,
        t: UniverseTime,
        seasonal: &KeplerElements,
        whose: SeasonsFrom,
    ) -> Self {
        let obliquity = match whose {
            SeasonsFrom::Own => frame.obliquity(),
            SeasonsFrom::Planet => {
                let [px, py, pz] = frame.pole();
                let [nx, ny, nz] = seasonal.orientation().normal();
                let cosine = (px * nx + py * ny + pz * nz).clamp(-1.0, 1.0);
                Radians::new(math::acos(cosine))
            }
        };
        let rate = frame.rate().rate_at(t);
        let tau = 2.0 * core::f64::consts::PI;
        // A law's rates are positive; one that is not would be a body that does not turn, whose
        // period is infinite.
        let rotation_period = Seconds::new(if rate > 0.0 {
            tau / rate
        } else {
            f64::INFINITY
        });
        let locked = matches!(
            (whose, frame.state_at(t)),
            (
                SeasonsFrom::Own,
                SpinState::Locked(SpinOrbitResonance::Synchronous)
            )
        );
        // Over a year the sun's right ascension turns once, with the spin below 90° of
        // obliquity and against it above, whatever the tilt: the mean solar day is the synodic
        // one.
        let mean_motion = tau / seasonal.period().value();
        let solar_rate = if obliquity.value() <= core::f64::consts::FRAC_PI_2 {
            (rate - mean_motion).abs()
        } else {
            rate + mean_motion
        };
        let solar_day = (!locked && solar_rate > 0.0).then(|| Seconds::new(tau / solar_rate));
        Self {
            obliquity,
            rotation_period,
            solar_day,
        }
    }
}

/// The class of the X-ray and extreme-ultraviolet flux a body's hosts give it, in decades of the
/// present Earth's ([`earth_xuv_flux`]): the host's activity level as the body receives it
/// (HYPERION's bands, a decade each, as plan 06's activity levels are).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RadiationClass {
    /// No X-ray or ultraviolet light: a body with no luminous host.
    Dark,
    /// Below 10 times the present Earth's flux: the Solar System's planets today.
    Quiet,
    /// 10 to 100 times: a Sun-like host under 1.0 Gyr old at 1 au, or a quieter one close in.
    Active,
    /// 100 to 1,000 times: a saturated Sun-like host at 1 au (some 300 times), or a hot Jupiter's
    /// orbit about an old one.
    Intense,
    /// 1,000 times or more: a saturated M dwarf's habitable zone, or the closest orbits.
    Extreme,
}

impl RadiationClass {
    /// The class of the X-ray and ultraviolet flux `xuv_flux` at a body: [`Self::Dark`] for none,
    /// and otherwise by its ratio to [`earth_xuv_flux`], each band closed below.
    #[must_use]
    pub fn of(xuv_flux: WattsPerSquareMetre) -> Self {
        let flux = xuv_flux.value();
        if flux.is_nan() || flux <= 0.0 {
            return Self::Dark;
        }
        let ratio = flux / earth_xuv_flux().value();
        if ratio < 10.0 {
            Self::Quiet
        } else if ratio < 100.0 {
            Self::Active
        } else if ratio < 1_000.0 {
            Self::Intense
        } else {
            Self::Extreme
        }
    }
}

/// The X-ray and extreme-ultraviolet flux Earth receives from the Sun today, the unit of the
/// radiation class: [`XuvHistory`] of 1 M☉ at
/// [`REFERENCE_SUN_ZAMS_LUMINOSITY`] at [`EARTH_REFERENCE_AGE`], at 1 au on a circular orbit,
/// about 9.75 × 10⁻⁴ W m⁻², as Earth's fluence is taken
/// ([`earth_xuv_fluence`](crate::planetary::derive::atmosphere::earth_xuv_fluence)).
#[must_use]
pub fn earth_xuv_flux() -> WattsPerSquareMetre {
    XuvHistory::new(SolarMasses::new(1.0), REFERENCE_SUN_ZAMS_LUMINOSITY).flux(
        EARTH_REFERENCE_AGE,
        Metres::new(METRES_PER_AU),
        0.0,
    )
}

/// A body's surface conditions at a time (P14.T24.a): its mean temperature and contrasts, its
/// surface pressure and gases, the light it receives, and whether water can be liquid on it.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceConditions {
    mean_surface_temperature: Kelvin,
    equator_pole_contrast_k: f64,
    day_night_contrast_k: f64,
    surface_pressure: Pascals,
    gases: Vec<(SubstanceId, f64)>,
    mean_molar_mass_g_per_mol: Option<f64>,
    heat_capacity_over_r: Option<f64>,
    stellar_flux: WattsPerSquareMetre,
    radiation: RadiationClass,
    liquid_water: bool,
}

impl SurfaceConditions {
    /// The conditions of the body `inputs` describe; `None` for a body in the gas-envelope state,
    /// which has no surface (P14.T48.e's split).
    ///
    /// An airless body has no gases, and its surface pressure is zero whatever residue its
    /// partial pressures hold, so that the list agrees with the state (P14.T53.c's test); its μ
    /// and `c_p` ÷ R are none, as T49.a's functions give for an empty mixture.
    ///
    /// # Examples
    ///
    /// Earth, as [`derive_body`](crate::planetary::derive::derive_body)'s example derives it, with
    /// its own spin:
    ///
    /// ```
    /// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
    /// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    /// use hyperion_sim::planetary::hooks::surface::{SeasonalSpin, SurfaceConditions, SurfaceInputs};
    /// use hyperion_sim::stellar::Composition;
    /// use hyperion_sim::stellar::draws::UnitUniform;
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
    /// use hyperion_sim::units::{
    ///     Degrees, Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres,
    ///     Radians, Seconds, SolarLuminosities, SolarMasses, SolarRadii, WattsPerSquareMetre, Years,
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
    /// let derived = derive_body(&earth, &hosts, disc, Years::new(4.57e9), UniverseTime::EPOCH)?;
    /// let conditions = SurfaceConditions::derive(&SurfaceInputs {
    ///     atmosphere: derived.atmosphere(),
    ///     equilibrium_temperature: derived.equilibrium_temperature(),
    ///     surface_gravity: derived.surface_gravity(),
    ///     radius: Metres::from(derived.radius()),
    ///     stellar_flux: WattsPerSquareMetre::from(derived.flux()),
    ///     xuv_flux: derived.xuv_flux(),
    ///     spin: SeasonalSpin {
    ///         obliquity: Radians::from(Degrees::new(23.44)),
    ///         rotation_period: Seconds::new(86_164.1),
    ///         solar_day: Some(Seconds::new(86_400.0)),
    ///     },
    /// })
    /// .expect("an Earth has a surface");
    /// // Its fractions are by number and sum to 1, its equator is the warmer, and water can be
    /// // liquid on it.
    /// let sum = conditions.gases().iter().fold(0.0, |sum, &(_, x)| sum + x);
    /// assert!((sum - 1.0).abs() < 1e-9);
    /// assert!(conditions.equator_pole_contrast_k() > 30.0);
    /// assert!(conditions.liquid_water());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn derive(inputs: &SurfaceInputs<'_>) -> Option<Self> {
        let air = inputs.atmosphere;
        let airless = match air.state() {
            SurfaceState::GasEnvelope => return None,
            SurfaceState::Airless => true,
            SurfaceState::MagmaOcean
            | SurfaceState::RunawayGreenhouse
            | SurfaceState::Temperate
            | SurfaceState::Snowball => false,
        };
        let (surface_pressure, gases) = if airless {
            (Pascals::ZERO, Vec::new())
        } else {
            (
                air.surface_pressure().unwrap_or(Pascals::ZERO),
                atmosphere_mole_fractions(air.partial_pressures()),
            )
        };
        let mean_molar_mass = mean_molar_mass_g_per_mol(&gases);
        let heat_capacity = heat_capacity_over_r(&gases);
        let column = Column {
            surface_temperature: air.surface_temperature(),
            equilibrium_temperature: inputs.equilibrium_temperature,
            surface_pressure,
            heat_capacity_j_per_m2_k: match (mean_molar_mass, heat_capacity) {
                (Some(mu), Some(cp)) => {
                    specific_heat_j_per_kg_k(cp, mu)
                        * column_mass_kg_per_m2(surface_pressure, inputs.surface_gravity)
                }
                (None, _) | (_, None) => 0.0,
            },
            optical_depth: if airless { 0.0 } else { air.optical_depth() },
            radius: inputs.radius,
        };
        // A world whose mean surface holds liquid water carries latent heat poleward.
        let water = Gas::Water;
        let moisture = if !airless
            && air.partial_pressures().of(water) > Pascals::ZERO
            && phase_at(
                water.substance(),
                column.surface_temperature,
                surface_pressure,
            ) == Some(Phase::Liquid)
        {
            Moisture::Wet
        } else {
            Moisture::Dry
        };
        let equator_pole = equator_pole_contrast_k(&column, &inputs.spin, moisture);
        let day_night = day_night_contrast_k(&column, &inputs.spin);
        Some(Self {
            mean_surface_temperature: column.surface_temperature,
            equator_pole_contrast_k: equator_pole,
            day_night_contrast_k: day_night,
            surface_pressure,
            gases,
            mean_molar_mass_g_per_mol: mean_molar_mass,
            heat_capacity_over_r: heat_capacity,
            stellar_flux: inputs.stellar_flux,
            radiation: RadiationClass::of(inputs.xuv_flux),
            liquid_water: liquid_water(
                column.surface_temperature,
                equator_pole,
                day_night,
                surface_pressure,
            ),
        })
    }

    /// The mean surface temperature (P14.T13.c), which rendering plan R08's profile starts from.
    #[must_use]
    pub const fn mean_surface_temperature(&self) -> Kelvin {
        self.mean_surface_temperature
    }

    /// The annual-mean equator–pole contrast T(0°) − T(90°), K, which rendering plan R09 imposes
    /// on its zonal temperature's `P₂` term: negative where the poles are warmer, as from 53.9° to
    /// 126.1° of obliquity (the [module](self)'s model; P14.T48.e's sign).
    #[must_use]
    pub const fn equator_pole_contrast_k(&self) -> f64 {
        self.equator_pole_contrast_k
    }

    /// The day–night contrast, the dayside's less the nightside's temperature, K, at least 0
    /// (the [module](self)'s model). For a world locked 1:1 to its seasonal host (its rotation
    /// section's [`SpinState::Locked`] with [`SpinOrbitResonance::Synchronous`]) it is stated
    /// about the substellar axis (P14.T48.e); for every other, the solar day turns it.
    #[must_use]
    pub const fn day_night_contrast_k(&self) -> f64 {
        self.day_night_contrast_k
    }

    /// The surface pressure: the sum of the gases' weights, and zero for an airless body.
    #[must_use]
    pub const fn surface_pressure(&self) -> Pascals {
        self.surface_pressure
    }

    /// The gases at the surface as `(SubstanceId, x)` mole fractions, largest first with ties by
    /// `SubstanceId`, summing to 1; empty for an airless body ([`mole_fractions`]). Every gas the
    /// atmosphere holds is listed, whatever its share.
    #[must_use]
    pub fn gases(&self) -> &[(SubstanceId, f64)] {
        &self.gases
    }

    /// The gases' mean molar mass μ = Σ xᵢ μᵢ, g mol⁻¹, summed over [`gases`](Self::gases) in its
    /// order (T49.a's [`mean_molar_mass_g_per_mol`]); `None` for an airless body.
    #[must_use]
    pub const fn mean_molar_mass_g_per_mol(&self) -> Option<f64> {
        self.mean_molar_mass_g_per_mol
    }

    /// The gases' mixed `c_p` ÷ R, Σ xᵢ (`c_p,ᵢ` ÷ R), summed over [`gases`](Self::gases) in its
    /// order (T49.a's [`heat_capacity_over_r`]), each row's class value at 21 → 22 and T49.b's
    /// function's at the mean surface temperature from 22 → 23; `None` for an airless body.
    #[must_use]
    pub const fn heat_capacity_over_r(&self) -> Option<f64> {
        self.heat_capacity_over_r
    }

    /// The flux of the hosts' light at the body, orbit-averaged.
    #[must_use]
    pub const fn stellar_flux(&self) -> WattsPerSquareMetre {
        self.stellar_flux
    }

    /// The class of the hosts' X-ray and ultraviolet flux at the body.
    #[must_use]
    pub const fn radiation(&self) -> RadiationClass {
        self.radiation
    }

    /// Whether water can be liquid at the surface pressure somewhere in the climate's range of
    /// temperature ([`liquid_water`]): a condition of the surface, whether or not the body holds
    /// water. From 22 → 23 P14.T51.a derives it from the partition.
    #[must_use]
    pub const fn liquid_water(&self) -> bool {
        self.liquid_water
    }
}

/// The gases of the column weights `weights` as `(SubstanceId, x)` mole fractions (signoff-3.md,
/// item 1.1): xᵢ = (pᵢ ÷ μᵢ) ÷ Σⱼ (pⱼ ÷ μⱼ), μᵢ each row's molar mass, for every gas of positive
/// weight, largest first with ties by `SubstanceId`.
///
/// P14.T13.c's partial pressures are column weights, pᵢ = Mᵢ g ÷ 4πR², which sum to the surface
/// pressure, so pᵢ ÷ `p_s` is a mass fraction; their mean molar mass is then
/// μ = Σ xᵢ μᵢ = `p_s` ÷ Σⱼ (pⱼ ÷ μⱼ). Any registry row with a molar mass may be weighed, each
/// once: the sum runs in the order given, and the list is empty if no gas has weight.
///
/// # Errors
///
/// - [`CountMolesError::NoMolarMass`] for a row of positive weight that has no molar mass;
/// - [`CountMolesError::NotFinite`] for a weight that is not finite, NaN included.
///
/// # Examples
///
/// Equal weights of nitrogen and carbon dioxide hold more nitrogen molecules:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::planetary::hooks::surface::mole_fractions;
/// use hyperion_sim::units::Pascals;
///
/// let (n2, co2) = (Gas::Nitrogen, Gas::CarbonDioxide);
/// let gases = mole_fractions([(n2.substance(), Pascals::new(5e4)), (co2.substance(), Pascals::new(5e4))])?;
/// assert_eq!(gases[0].0, n2.substance());
/// let (m_n2, m_co2) = (n2.molar_mass_g_per_mol(), co2.molar_mass_g_per_mol());
/// assert!((gases[0].1 - m_co2 / (m_co2 + m_n2)).abs() < 1e-12);
/// # Ok::<(), hyperion_sim::planetary::hooks::surface::CountMolesError>(())
/// ```
pub fn mole_fractions(
    weights: impl IntoIterator<Item = (SubstanceId, Pascals)>,
) -> Result<Vec<(SubstanceId, f64)>, CountMolesError> {
    mole_fractions_by(weights, |id| id.substance().molar_mass_g_per_mol())
}

/// [`mole_fractions`] with each row's molar mass, g mol⁻¹, from `molar_mass`.
fn mole_fractions_by(
    weights: impl IntoIterator<Item = (SubstanceId, Pascals)>,
    molar_mass: impl Fn(SubstanceId) -> Option<f64>,
) -> Result<Vec<(SubstanceId, f64)>, CountMolesError> {
    let mut moles = Vec::new();
    for (id, weight) in weights {
        let p = weight.value();
        if !p.is_finite() {
            return Err(CountMolesError::NotFinite(id));
        }
        if p > 0.0 {
            let mu = molar_mass(id).ok_or(CountMolesError::NoMolarMass(id))?;
            moles.push((id, p / mu));
        }
    }
    let total = moles.iter().fold(0.0, |sum, &(_, n)| sum + n);
    let mut gases: Vec<(SubstanceId, f64)> =
        moles.into_iter().map(|(id, n)| (id, n / total)).collect();
    debug_assert!(
        gases.iter().all(|&(_, x)| x.is_finite()),
        "finite weights over positive molar masses give finite fractions"
    );
    gases.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(gases)
}

/// The gases of an atmosphere's partial pressures `partials` as mole fractions
/// ([`mole_fractions`] over [`Gas::ALL`]'s rows, in registry order).
///
/// # Panics
///
/// Never: every [`Gas`] row of the registry has a molar mass (the registry's tests).
#[must_use]
pub fn atmosphere_mole_fractions(partials: &PartialPressures) -> Vec<(SubstanceId, f64)> {
    mole_fractions(Gas::ALL.map(|gas| (gas.substance(), partials.of(gas))))
        .expect("every `Gas` row of the registry has a molar mass")
}

/// [`mole_fractions`] was given a weight it cannot count in molecules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CountMolesError {
    /// The row of positive weight has no molar mass: it is not a gas.
    NoMolarMass(SubstanceId),
    /// The row's weight is infinite or not a number.
    NotFinite(SubstanceId),
}

impl fmt::Display for CountMolesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMolarMass(id) => write!(
                f,
                "the substance {} has weight but no molar mass",
                id.substance().key().as_str()
            ),
            Self::NotFinite(id) => write!(
                f,
                "the substance {} has a weight that is not finite",
                id.substance().key().as_str()
            ),
        }
    }
}

impl Error for CountMolesError {}

/// Whether a world's meridional transport carries latent heat ([`DRY_TRANSPORT_SHARE`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Moisture {
    /// Its mean surface holds liquid water: the whole transport.
    Wet,
    /// It does not: half.
    Dry,
}

/// The column the contrasts read, as plain values ([`SurfaceConditions::derive`] builds it from
/// its inputs). Its fields carry no invariant: a temperature, pressure or radius that is not
/// positive gives no contrast or no transport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Column {
    /// The mean surface temperature.
    pub(crate) surface_temperature: Kelvin,
    /// The equilibrium temperature: σ `T_eq`⁴ is the absorbed mean flux.
    pub(crate) equilibrium_temperature: Kelvin,
    /// The surface pressure.
    pub(crate) surface_pressure: Pascals,
    /// The air's heat capacity per unit area, `c_p` (`p_s` ÷ g), J m⁻² K⁻¹: zero for no air.
    pub(crate) heat_capacity_j_per_m2_k: f64,
    /// The grey optical depth of the greenhouse (P14.T13.c).
    pub(crate) optical_depth: f64,
    /// The body's mean radius.
    pub(crate) radius: Metres,
}

impl Column {
    /// The absorbed mean flux σ `T_eq`⁴.
    #[must_use]
    fn absorbed(&self) -> WattsPerSquareMetre {
        WattsPerSquareMetre::new(
            STEFAN_BOLTZMANN * math::powi(self.equilibrium_temperature.value(), 4),
        )
    }

    /// The radiative restoring at the surface, B = 4F ÷ `T_s`, W m⁻² K⁻¹: the grey column's
    /// dOLR ÷ d`T_s` at fixed τ.
    #[must_use]
    pub(crate) fn restoring_w_per_m2_k(&self) -> f64 {
        4.0 * self.absorbed().value() / self.surface_temperature.value()
    }
}

/// The specific heat at constant pressure of a mixture of `c_p` ÷ R `heat_capacity_over_r` and
/// molar mass `molar_mass_g_per_mol`, J kg⁻¹ K⁻¹.
#[must_use]
pub(crate) fn specific_heat_j_per_kg_k(
    heat_capacity_over_r: f64,
    molar_mass_g_per_mol: f64,
) -> f64 {
    heat_capacity_over_r * MOLAR_GAS_CONSTANT / (molar_mass_g_per_mol * 1e-3)
}

/// The column's mass per unit area, `p_s` ÷ g, kg m⁻²: zero for a gravity that is not positive.
#[must_use]
fn column_mass_kg_per_m2(pressure: Pascals, gravity: MetresPerSecondSquared) -> f64 {
    let g = gravity.value();
    if g > 0.0 { pressure.value() / g } else { 0.0 }
}

/// The share of Earth's meridional transport a world rotating once in `rotation_period` keeps,
/// D(Ω) ÷ D(Ω⊕) = (Ω ÷ Ω⊕)^−¼ below Earth's rate and (Ω ÷ Ω⊕)^−½ above it
/// ([`ROTATION_TRANSPORT_EXPONENTS`]), held at its ends beyond [`ROTATION_TRANSPORT_RANGE`]
/// (rendering plan R09's transport law, after Kaspi and Showman 2015, Fig. 8a: their contrast,
/// 40 K at Ω⊕, is 23 K at Ω⊕ ÷ 24 and 77 K at 12 Ω⊕).
///
/// A slower world's transport is stronger: its eddies are larger and its Hadley cells wider. A
/// period that is not positive and finite holds the slow end.
#[must_use]
pub fn rotation_transport_factor(rotation_period: Seconds) -> f64 {
    let period = rotation_period.value();
    let (slowest, fastest) = ROTATION_TRANSPORT_RANGE;
    let ratio = if period.is_finite() && period > 0.0 {
        (EARTH_SIDEREAL_DAY.value() / period).clamp(slowest, fastest)
    } else {
        slowest
    };
    let (slow, fast) = ROTATION_TRANSPORT_EXPONENTS;
    math::powf(ratio, if ratio <= 1.0 { slow } else { fast })
}

/// The annual-mean equator–pole contrast of the column `column` under the spin `spin`, K:
/// T(0°) − T(90°) = `T_s` (`q_eq`^¼ − `q_pole`^¼) ÷ (1 + 6D ÷ B), signed (the module's model), with
/// q the annual insolation of [`annual_insolation`].
///
/// D = `c_p` (`p_s` ÷ g) K f(Ω) w ÷ R², with K [`MERIDIONAL_DIFFUSIVITY_M2_S`], f
/// [`rotation_transport_factor`] and w 1 for a wet world or [`DRY_TRANSPORT_SHARE`]; B = 4F ÷
/// `T_s`. Zero where no light is absorbed. It changes sign at 53.9° and at 126.1° of obliquity
/// and is negative between, where the poles take more of the year's light than the equator.
#[must_use]
pub(crate) fn equator_pole_contrast_k(
    column: &Column,
    spin: &SeasonalSpin,
    moisture: Moisture,
) -> f64 {
    let restoring = column.restoring_w_per_m2_k();
    let t_s = column.surface_temperature.value();
    if !(restoring.is_finite() && restoring > 0.0 && t_s > 0.0) {
        return 0.0;
    }
    let r = column.radius.value();
    let share = match moisture {
        Moisture::Wet => 1.0,
        Moisture::Dry => DRY_TRANSPORT_SHARE,
    };
    let diffusion = if r > 0.0 {
        column.heat_capacity_j_per_m2_k
            * MERIDIONAL_DIFFUSIVITY_M2_S
            * rotation_transport_factor(spin.rotation_period)
            * share
            / (r * r)
    } else {
        0.0
    };
    let (equator, pole) = annual_insolation(spin.obliquity);
    t_s * (math::powf(equator, 0.25) - math::powf(pole, 0.25)) / (1.0 + 6.0 * diffusion / restoring)
}

/// The annual-mean insolation at the equator and at a pole of a world at obliquity `obliquity`,
/// each relative to the mean S ÷ 4: `(8 ÷ π²) E(sin β)` and `(4 ÷ π) sin β`.
///
/// The equator is lit half of every day, so its daily mean is (S ÷ π) cos δ, and the year's mean
/// of cos δ, with sin δ = sin β sin λ uniform in the sun's longitude λ, is (2 ÷ π) E(sin β). A
/// pole's daily mean is S sin δ in its summer and nothing in its winter, (S ÷ π) sin β over the
/// year. At 23.44° they are 1.2213 and 0.5065; they are equal at 53.9°.
#[must_use]
pub fn annual_insolation(obliquity: Radians) -> (f64, f64) {
    let sine = math::sin(obliquity.value()).abs();
    let pi = core::f64::consts::PI;
    (8.0 / (pi * pi) * complete_elliptic_e(sine), 4.0 / pi * sine)
}

/// The complete elliptic integral of the second kind E(k) = ∫₀^{π/2} √(1 − k² sin²θ) dθ for a
/// modulus `k` in [0, 1], by the arithmetic–geometric mean (DLMF §19.8(i)):
/// E = (π ÷ 2M) (1 − Σₙ 2ⁿ⁻¹ cₙ²) with c₀ = k, over [`ELLIPTIC_STEPS`] fixed steps; E(1) = 1.
#[must_use]
fn complete_elliptic_e(k: f64) -> f64 {
    if k >= 1.0 {
        return 1.0;
    }
    let (mut a, mut b) = (1.0, (1.0 - k * k).sqrt());
    let mut weight = 0.5;
    let mut sum = weight * k * k;
    for _ in 0..ELLIPTIC_STEPS {
        let c = 0.5 * (a - b);
        weight *= 2.0;
        sum += weight * c * c;
        (a, b) = (f64::midpoint(a, b), (a * b).sqrt());
    }
    core::f64::consts::FRAC_PI_2 / a * (1.0 - sum)
}

/// The arithmetic–geometric mean's steps in [`complete_elliptic_e`]: 40, past its convergence to
/// the last bit for any modulus below 1 (it converges quadratically once a and b are close, within
/// some ten steps from b₀ = 10⁻¹⁶), so that every modulus takes the same arithmetic.
const ELLIPTIC_STEPS: u32 = 40;

/// Koll's heat-redistribution efficiency ε of the column `column`, from 0 (none) to 1 (uniform):
/// X ÷ (k + X) with X = τ^⅓ (`p_s` ÷ 1 bar)^⅔ (`T_eq` ÷ 600 K)^−4/3 and k [`REDISTRIBUTION_SCALE`]
/// (Koll 2022, ApJ 924, 134, whose f = 2/3 − (5/12) ε is Cowan and Agol 2011's with this ε).
///
/// Zero with no air or no greenhouse, and 1 where no light is absorbed.
#[must_use]
pub(crate) fn heat_redistribution(column: &Column) -> f64 {
    let (tau, p, t_eq) = (
        column.optical_depth,
        column.surface_pressure.value(),
        column.equilibrium_temperature.value(),
    );
    if !(tau > 0.0 && p > 0.0) {
        return 0.0;
    }
    if t_eq.is_nan() || t_eq <= 0.0 {
        return 1.0;
    }
    let x = math::cbrt(tau)
        * math::powf(p / REDISTRIBUTION_REFERENCE_PRESSURE.value(), 2.0 / 3.0)
        * math::powf(
            t_eq / REDISTRIBUTION_REFERENCE_TEMPERATURE.value(),
            -4.0 / 3.0,
        );
    if x.is_finite() {
        x / (REDISTRIBUTION_SCALE + x)
    } else {
        1.0
    }
}

/// The day–night contrast of the column `column` under the spin `spin`, K, at least 0: the
/// dayside's less the nightside's surface temperature at Koll's redistribution ε
/// ([`heat_redistribution`]; Cowan and Agol 2011, eqs. 4–5), `T_s` ((8/3 − 5ε/3)^¼ − ε^¼), damped
/// on a rotating body by 1 ÷ √(1 + (ω τ)²) with ω = 2π over the solar day and τ = `c_p` (`p_s` ÷
/// g) ÷ B the column's radiative time (the module's model).
///
/// An airless body's is `T_s` (8/3)^¼, its dayside's radiative temperature over a night at 0 K;
/// a locked world's is stated about its substellar axis. Zero where no light is absorbed.
#[must_use]
pub(crate) fn day_night_contrast_k(column: &Column, spin: &SeasonalSpin) -> f64 {
    let restoring = column.restoring_w_per_m2_k();
    let t_s = column.surface_temperature.value();
    if !(restoring.is_finite() && restoring > 0.0 && t_s > 0.0) {
        return 0.0;
    }
    let epsilon = heat_redistribution(column);
    let day = math::powf(8.0 / 3.0 - 5.0 / 3.0 * epsilon, 0.25);
    let night = math::powf(epsilon, 0.25);
    let locked = t_s * (day - night);
    match spin.solar_day {
        None => locked,
        Some(solar_day) => {
            let omega_tau = 2.0 * core::f64::consts::PI / solar_day.value()
                * column.heat_capacity_j_per_m2_k
                / restoring;
            locked / math::hypot(1.0, omega_tau)
        }
    }
}

/// Whether water can be liquid at `pressure` somewhere between `T_s − ½|ΔT_ep| − ½|ΔT_dn|` and
/// `T_s + ½|ΔT_ep| + ½|ΔT_dn|` (the climate's range, as P14.T48.d takes it from the mean and
/// signed contrasts), by the registry's [`phase_at`] on `H2O`'s row.
///
/// The registry's liquid at a pressure lies from the triple temperature to the boiling point, so
/// the range holds liquid exactly where its coldest point not below the triple temperature does.
#[must_use]
pub fn liquid_water(
    surface_temperature: Kelvin,
    equator_pole_contrast_k: f64,
    day_night_contrast_k: f64,
    pressure: Pascals,
) -> bool {
    let half = 0.5 * equator_pole_contrast_k.abs() + 0.5 * day_night_contrast_k.abs();
    let (coldest, warmest) = (
        surface_temperature.value() - half,
        surface_temperature.value() + half,
    );
    let water = Gas::Water.substance();
    let Some(triple) = water
        .substance()
        .phase()
        .map(|p| p.triple_temperature().value())
    else {
        return false;
    };
    let candidate = coldest.max(triple);
    candidate <= warmest && phase_at(water, Kelvin::new(candidate), pressure) == Some(Phase::Liquid)
}
