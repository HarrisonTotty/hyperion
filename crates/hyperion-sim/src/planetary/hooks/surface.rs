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
//!   ([`RadiationClass`]), the **climate's range** of temperature, and the **liquid-water flag**
//!   from water's phase over that range ([`liquid_water`]).
//!
//! The surface gravity, which the column reads, is the bulk section's and is not repeated here
//! (P14.T35.e).
//!
//! # The contrasts' model
//!
//! Both contrasts are closed forms of a linear energy balance on the body's mean state, so that
//! they need no field and are continuous in every input but two switches: a world's wetness
//! (liquid water at its mean surface temperature, which sets Φ and the transport's share, until
//! P14.T24.b's ocean fraction weights them) and a gas's reaching its saturation pressure (which
//! turns on its vapour feedback). They are science-p14-t24a-climate.md's
//! (2026-10-11), each piece cited on its constant; plan 14's Risks ("Deviations in P14.T24.a, as
//! built") records how they compare with the Solar System's bodies.
//!
//! - **Equator–pole**, T(0°) − T(90°), as rendering plan R09 imposes it on its annual field's
//!   `P₂` term (its Design note 8). With no transport each latitude is in radiative balance with
//!   its annual-mean insolation, which relative to the mean S ÷ 4 is (8 ÷ π²) E(sin β) at the
//!   equator and (4 ÷ π) sin β at a pole, at obliquity β, E the complete elliptic integral of
//!   the second kind ([`annual_insolation`]). An eccentric orbit changes only the mean flux, since
//!   by Kepler's second law (a ÷ r)² dt runs uniformly in the sun's longitude. The grey column
//!   scales each to the surface (P14.T13.c's `T_s` = `T_eq` (1 + ¾τ)^¼), so the radiative ends
//!   are `T_s` `q_eq`^¼ and `T_s` `q_pole`^¼. Transport damps the insolation's Legendre modes,
//!   each at its own rate n(n + 1)D as in North's (1975, J. Atmos. Sci. 32, 2033) diffusive model,
//!   in the emitted flux and at each end's own restoring. The contrast is the difference of the
//!   ends this gives (`zonal_ends_k`; science-p14-t24a-climate.md §7). It works in the partition
//!   North, Cahalan and Coakley (1981) measure on Earth, D ÷ B = 0.310
//!   ([`EARTH_TRANSPORT_PARTITION`]), with each mode damped by `B_P` ÷ (`B_eff` +
//!   max(n(n + 1)D, W `B_P`)).
//!   - `B_P` = 4F ÷ `T_s` is the grey column's restoring at fixed τ, F the absorbed mean flux.
//!     `B_eff` = `B_P` (1 − g) is its restoring with its own vapour feedback, g the gain of the
//!     gases T13.c holds at their saturation pressure (0.38 on Earth).
//!   - Φ multiplies a wet world's `P₂` forcing, its cold zones' brightening by ice, cloud and low
//!     sun, which a uniform albedo misses ([`WET_ALBEDO_GRADIENT`], fitted on that partition; the
//!     Earth Radiation Budget's gradient within 3%).
//!   - D is Earth's conductance ([`EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K`]) scaled by the air's
//!     specific heat and by Kaspi and Showman's (2015, ApJ 804, 60) sweeps: in rotation, as R09
//!     takes them ([`rotation_transport_factor`]), in the column's mass
//!     ([`COLUMN_TRANSPORT_EXPONENT`], linear to zero below a fifth of Earth's) and in gravity
//!     ([`GRAVITY_TRANSPORT_EXPONENT`]), with no term of its own in the radius; it is halved on a
//!     dry world ([`DRY_TRANSPORT_SHARE`]).
//!   - W is Koll's (2022, ApJ 924, 134) weak-temperature-gradient transport over the restoring,
//!     X ÷ k (below), which governs a slow rotator such as Venus. It is weighted by
//!     1 ÷ (1 + (3a ÷ `L_Ro`)²), so that it governs only where the deformation radius `L_Ro`
//!     exceeds three planetary radii ([`WEAK_GRADIENT_DEFORMATION_RADII`]).
//!
//!   The sign changes at 53.9° of obliquity on an airless body, where the equator's and the
//!   poles' insolation are equal, and toward 54.7° as the transport grows.
//! - **Day–night** (Cowan and Agol 2011, ApJ 729, 54, eqs. 4–5). With a redistribution efficiency
//!   ε from 0 (none) to 1 (uniform), the dayside and nightside temperatures are
//!   `T_eq` (8/3 − 5ε/3)^¼ and `T_eq` ε^¼, scaled to the surface by `T_s` ÷ `T_eq`. ε joins the
//!   air's share, the ground's and the night's floor:
//!   - the air's, Koll's scaling of locked rocky planets' GCMs, ε = X ÷ (k + X), with
//!     X = `τ_LW`^⅓ (`p_s` ÷ 1 bar)^⅔ (`T_eq` ÷ 600 K)^−4/3 at `τ_LW` = (3/2)τ
//!     ([`FLUX_OPTICAL_DEPTH_PER_GREY`]) and k the body's own, by his eq. 9's definition;
//!   - the ground's, its regolith's diurnal storage, x ÷ (1 + x) with x = 0.635 Θ^0.905
//!     ([`REGOLITH_REDISTRIBUTION_FIT`]), Θ the thermal parameter at its material's inertia;
//!   - the two in series, 1 − ε = (1 − `ε_air`)(1 − `ε_ground`), to which the floor adds
//!     (`T_floor` ÷ `T_eq`)⁴, with ε capped at 1.
//!
//!   A rotating body's departures from `T_s` are each damped by the column's storage over the
//!   solar day, 1 ÷ √(1 + (ω C (1 − ε) ÷ `B_P`)²), C the air's heat capacity per unit area: the
//!   linear response of a slab forced at the solar rate ω under the locked contrast's own
//!   restoring, `B_P` ÷ (1 − ε).
//! - **The climate's range**, the span of the orbit-mean climate: the warmer end of the zonal
//!   profile plus the day's excess over `T_s`, down to the colder end less the night's deficit,
//!   never below the floor ([`SurfaceConditions::climate_range`]). The profile's ends are the
//!   equator–pole contrast's own (`zonal_ends_k`, science-p14-t24a-climate.md §6 and §7): an
//!   airless body's radiative ends, `T_s` `q_eq`^¼ and `T_s` `q_pole`^¼, at no transport, tending
//!   to `T_s` as the transport grows.
//!
//! An airless body's equator–pole contrast is the radiative one. Its night is its regolith's: the
//! diurnal storage enters Cowan and Agol's ε as x ÷ (1 + x), x = 0.635 Θ^0.905, Θ the thermal
//! parameter at the material's inertia, above a floor set by the internal heat and the cosmic
//! background (science-p14-t24a-climate §2). The Moon's night is 106 K (Diviner, ~95 K before
//! dawn) and Mercury's 102 K (Mariner 10, 100–130 K). Until P14.T24.b supplies the heat flow the
//! floor is the cosmic background's, [`COSMIC_BACKGROUND_TEMPERATURE`].
//!
//! Koll's scaling is valid for dry, tidally locked rocky planets with a high mean molecular
//! weight, at `T_eq` of 391–805 K, 0.01–100 bar and `τ_LW` of 0.1–5, to a factor of two in his χ
//! (Koll 2022, §3). It is extrapolated here to rotators and colder worlds, labelled. It does not
//! hold where the bulk gas condenses on the night side (an N₂ or CO₂ collapse, Wordsworth 2015,
//! ApJ 806, 180): no cutoff is applied here, which P14.T48.d's climate regime takes as its thin
//! end.
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
    REFERENCE_SUN_ZAMS_LUMINOSITY, SurfaceMaterial, SurfaceState, optical_depth_terms,
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
/// Earth-like reference world takes the same figure). The figure [`WET_ALBEDO_GRADIENT`] is
/// fitted to.
pub const EARTH_EQUATOR_POLE_CONTRAST_K: f64 = 42.0;

/// Earth's meridional conductance over its radiative restoring, D ÷ B: 0.310, North, Cahalan and
/// Coakley's partition (1981, Rev. Geophys. 19, 91, §2, read in its NASA preprint, NTRS
/// 19810008165).
///
/// It comes from three measurements: Budyko's B = 2.09 W m⁻² K⁻¹ fitted to the observed outgoing
/// longwave radiation (North and Coakley 1979's Northern Hemisphere data), the absorbed
/// sunlight's coalbedo a₀ = 0.681 and a₂ = −0.202 (the Earth Radiation Budget's data), and the
/// observed T₂ = −28.0 °C, which give D = 0.649 W m⁻² K⁻¹. Rendering plan R09 inverted Kaspi and
/// Showman's rotation sweep in this partition ([`ROTATION_TRANSPORT_EXPONENTS`]), and Earth's
/// measured state is fitted to it (science-p14-t24a-climate.md §1).
pub const EARTH_TRANSPORT_PARTITION: f64 = 0.310;

/// Earth's meridional conductance D⊕, W m⁻² K⁻¹ (North, Cahalan and Coakley's units: the
/// diffusion's flux divergence per kelvin of the `P₂` mode is 6D): 0.6433, so that Earth's
/// measured state takes 6D⊕ ÷ `B_eff` = 6 × [`EARTH_TRANSPORT_PARTITION`], `B_eff` its restoring
/// with its own vapour feedback (`Fitted`, in closed form, on the partition alone, whatever the
/// contrast's form; [`WET_ALBEDO_GRADIENT`] is then fitted to the contrast; a test refits both).
///
/// Earth's measured state: 288 K at the surface (NASA's Earth fact sheet), 23.44° of obliquity, an
/// absorbed flux of (1 − 0.294) × 1,361.17 ÷ 4 = 240.25 W m⁻² (the fact sheet's Bond albedo, and
/// [`SOLAR_CONSTANT_W_PER_M2`](crate::units::consts::SOLAR_CONSTANT_W_PER_M2)), the grey τ that
/// holds 288 K over it, a column of [`EARTH_COLUMN_MASS_KG_PER_M2`] of dry air
/// ([`EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K`]) under [`STANDARD_GRAVITY`], water vapour at its
/// saturation pressure at 288 K on the substance registry's curve, a radius of 6,371.0 km and
/// [`EARTH_SIDEREAL_DAY`]. Its `B_eff` is 2.08 W m⁻² K⁻¹, against North, Cahalan and Coakley's
/// 2.09 and the grey column's 3.34 at fixed τ.
///
/// Cross-checks, not fitted (science-p14-t24a-climate.md §1): the peak diffusive transport,
/// 2πR² D⊕ (2 ÷ √3) × 28 K, is 5.3 PW, against North, Cahalan and Coakley's 5.35 at their D and
/// the 5.5–6 PW measured (Trenberth and Caron 2001, J. Climate 14, 3433, not re-read). As a
/// diffusivity of temperature, D⊕ R² ÷ (`c_p` m⊕), it is 2.6 × 10⁶ m² s⁻¹, within 5% of Siler,
/// Roe and Armour's (2018, J. Climate 31, 7481, not re-read) diffusivity of moist static energy,
/// 1.16 × 10⁶ m² s⁻¹, converted to temperature at 288 K and 80% relative humidity (× 2.33).
pub const EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K: f64 = 0.643_338_15;

/// The factor Φ by which a wet world's cold zones brighten its equator–pole forcing: 1.4948,
/// multiplying the `P₂` mode of the annual insolation of a world whose mean surface holds liquid
/// water, and 1 on any other (`Fitted`, so that Earth's measured state gives
/// [`EARTH_EQUATOR_POLE_CONTRAST_K`]; a test refits it).
///
/// Earth's cold zones are brighter, by ice, cloud and low sun, which a uniform albedo misses. No
/// closed form on this task's inputs supplies it: P14.T24.b's ice and P14.T24.c's clouds come
/// later, and an ice-albedo solve would bring Budyko's multiple equilibria, which are R09's and
/// P14.T48.d's (science-p14-t24a-climate.md §1). It multiplies the `P₂` forcing alone, the
/// coalbedo gradient it stands for (science-p14-t24a-climate.md §7).
///
/// Cross-checks, not fitted:
/// - the Earth Radiation Budget's coalbedo gradient amplifies the uniform `P₂` forcing by
///   `H₂` ÷ (`S₂` `a₀`) = 1.54 (their `S₂` = −0.477), within 3%;
/// - with it, the measured Earth's equator and pole are 302.4 and 260.4 K, against North,
///   Cahalan and Coakley's `P₂` fit of 302.1 and 260.1 K;
/// - Komacek and Abbot (2019, ApJ 871, 245, Table 2) find that sea ice and clouds raise an
///   Earth-like contrast by up to some 100 K over Kaspi and Showman's, so the amplification is
///   real and, if anything, larger.
///
/// A Mars's CO₂ caps and a Titan's methane are not brightened until P14.T51.a names a cap at the
/// cold zone (22 → 23).
pub const WET_ALBEDO_GRADIENT: f64 = 1.494_818_98;

/// Earth's mean column of air, kg m⁻²: 10,094, its atmosphere's 5.1480 × 10¹⁸ kg over
/// 5.100 × 10¹⁴ m² (Trenberth and Smith 2005, J. Climate 18, 864), the column the conductance's
/// mass factor is relative to ([`COLUMN_TRANSPORT_EXPONENT`]).
pub const EARTH_COLUMN_MASS_KG_PER_M2: f64 = 10_094.0;

/// Dry air's specific heat at constant pressure, the one the conductance is relative to: 1,004.7
/// J kg⁻¹ K⁻¹, 7/2 R over 28.9644 g mol⁻¹.
///
/// 7/2 is the registry's `c_p` ÷ R class for N₂ and O₂, and 28.9644 g mol⁻¹ the sea-level mean
/// molar mass M₀ of the US Standard Atmosphere 1976 (NOAA, NASA and USAF; not re-read). T24.a as
/// first built took 28.9647.
pub const EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K: f64 =
    3.5 * MOLAR_GAS_CONSTANT / (EARTH_AIR_MOLAR_MASS_G_PER_MOL * 1e-3);

/// Dry air's mean molar mass at sea level, M₀: 28.9644 g mol⁻¹ (the US Standard Atmosphere 1976;
/// not re-read), the measured Earth's air ([`EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K`]).
pub const EARTH_AIR_MOLAR_MASS_G_PER_MOL: f64 = 28.964_4;

/// The standard acceleration of gravity g₀: 9.806 65 m s⁻² (the 3rd CGPM, 1901), the gravity the
/// conductance's gravity factor is relative to ([`GRAVITY_TRANSPORT_EXPONENT`]).
pub const STANDARD_GRAVITY: MetresPerSecondSquared = MetresPerSecondSquared::new(9.806_65);

/// The exponent of the meridional conductance in the air's column mass relative to Earth's: 0.2,
/// from Kaspi and Showman's (2015, ApJ 804, 60) Fig. 15c, as rendering plan R09's transport law
/// takes it.
///
/// Their contrasts run from 0.2 to 50 bar at fixed τ, and "the increase in MSE flux with
/// atmospheric mass becomes more gradual beyond 10 bars" (their §3.3). The variable is the
/// column's mass, not the surface pressure, by their §3.4: "if atmospheric surface pressure,
/// rather than atmospheric mass, is held constant while the gravity is varying (not shown), then
/// the simulations would resemble those of varying atmospheric mass".
pub const COLUMN_TRANSPORT_EXPONENT: f64 = 0.2;

/// The column masses, relative to [`EARTH_COLUMN_MASS_KG_PER_M2`], over which the conductance
/// follows [`COLUMN_TRANSPORT_EXPONENT`]: 0.2 to 50, Kaspi and Showman's runs.
///
/// Below 0.2 it falls linearly to zero, 0.2^0.2 × m ÷ 0.2, continuous at 0.2, since with no air
/// there is no transport (a 6 mbar Mars's is weak, 6D ÷ B = 0.07); above 50 it holds its end
/// value (science-p14-t24a-climate.md §1).
pub const COLUMN_TRANSPORT_RANGE: (f64, f64) = (0.2, 50.0);

/// The exponent of the meridional conductance in the surface gravity relative to
/// [`STANDARD_GRAVITY`]: −0.43, a least-squares fit to Kaspi and Showman's (2015) Fig. 18a,
/// densities of 500 to 11,200 kg m⁻³ at Earth's radius and column mass, with an rms error of 1.8%
/// (science-p14-t24a-climate.md §1).
///
/// The radius takes no term of its own: their Fig. 20b, radii of 1,000 to 50,000 km at Earth's
/// density, so with g ∝ R, is reproduced within 8% by this factor alone. Komacek and Abbot 2019
/// (their Table 2) find a trend in radius at fixed gravity, confounded by glaciation; Kaspi and
/// Showman's cleanly separated sweeps are taken.
pub const GRAVITY_TRANSPORT_EXPONENT: f64 = -0.43;

/// The gravities, relative to [`STANDARD_GRAVITY`], over which the conductance follows
/// [`GRAVITY_TRANSPORT_EXPONENT`], and beyond which it holds its end values: 0.09 to 7.85, the
/// span of Kaspi and Showman's Fig. 18a and of Fig. 20b's rocky line.
pub const GRAVITY_TRANSPORT_RANGE: (f64, f64) = (0.09, 7.85);

/// The planetary radii the deformation radius must exceed for Koll's weak-temperature-gradient
/// transport to govern the equator–pole contrast: 3 (science-p14-t24a-climate.md §4's).
///
/// It weights the transport by 1 ÷ (1 + (3a ÷ `L_Ro`)²), with a ÷ `L_Ro` = 2Ωa ÷ c and
/// c = `R_air` √(`T_eq` ÷ `c_p`) the gravity waves' speed (Koll 2022, §3.1). The weak-gradient
/// regime holds where the deformation radius exceeds the planet (Pierrehumbert 2011; Showman et
/// al. 2013; neither re-read). The factor 3 is fitted so that Kaspi and Showman's slowest
/// (Ω⊕ ÷ 24, a ÷ `L_Ro` = 0.27) and smallest (1,000 km, 1.0) Earths keep their GCM's contrasts,
/// 24 and 22 K.
pub const WEAK_GRADIENT_DEFORMATION_RADII: f64 = 3.0;

/// The share of the meridional transport a world without surface liquid keeps: 0.5, halved
/// where no latent heat is carried, as rendering plan R09's energy-balance model takes it (its
/// Design note 8, after Williams and Kasting 1997, Icarus 129, 254, not re-read; `Adopted`).
///
/// Kaspi and Showman (2015, ApJ 804, 60, §3.2 and Fig. 9b) find Earth's dry and latent fluxes
/// "very similar in magnitude", and R09's split of Siler et al.'s diffusivity, about 0.3 dry of
/// 0.7 moist W m⁻² K⁻¹, gives 0.43.
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
/// confidence). This module's model is in that partition ([`EARTH_TRANSPORT_PARTITION`]), and
/// the factors move an Earth's 42 K over 23.5–78.2 K from Ω⊕ ÷ 24 to 12 Ω⊕, against the GCM's
/// 24.2–80.9 K at the same scale, within 3.3% but at Ω⊕ ÷ 4, the GCM's plateau (−12.5%).
pub const ROTATION_TRANSPORT_EXPONENTS: (f64, f64) = (-0.25, -0.5);

/// Koll's inefficiency factor χ of the day–night heat engine: 0.2, which "should be treated as
/// uncertain by at least a factor of two since χ accounts not only for the atmosphere's
/// thermodynamic efficiency but also the circulation geometry" (Koll 2022, ApJ 924, 134, §3.2,
/// read), adopted on his GCMs of dry, tidally locked rocky planets.
pub const REDISTRIBUTION_INEFFICIENCY: f64 = 0.2;

/// Koll's surface drag coefficient `C_d`: 1.9 × 10⁻³ (Koll 2022, §3.2).
pub const REDISTRIBUTION_DRAG_COEFFICIENT: f64 = 1.9e-3;

/// Koll's `n_LW`, the power of the pressure in the longwave optical depth, τ ∝ p^`n_LW`: 2, for
/// pressure broadening (Koll 2022, §2; 1 where the opacity is independent of pressure).
pub const REDISTRIBUTION_BROADENING: f64 = 2.0;

/// Koll's longwave optical depth `τ_LW` over P14.T13.c's grey τ: 3/2 (science-p14-t24a-climate.md
/// §4).
///
/// His FMS model solves the grey two-stream dU ÷ dτ = U − σT⁴, dD ÷ dτ = σT⁴ − D (Frierson, Held
/// and Zurita-Gotor 2006, J. Atmos. Sci. 63, 2548, not re-read; Kaspi and Showman 2015 print it as
/// their eqs. 6–7), whose radiative equilibrium holds the ground at `T_eq`⁴ (1 + `τ_LW` ÷ 2).
/// T13.c's Eddington form holds it at `T_eq`⁴ (1 + ¾τ).
pub const FLUX_OPTICAL_DEPTH_PER_GREY: f64 = 1.5;

/// The surface pressure Koll's X and k are scaled to, Pa: 1 bar.
pub const REDISTRIBUTION_REFERENCE_PRESSURE: Pascals = Pascals::new(1e5);

/// The equilibrium temperature Koll's X and k are scaled to: 600 K.
pub const REDISTRIBUTION_REFERENCE_TEMPERATURE: Kelvin = Kelvin::new(600.0);

/// A regolith's infrared emissivity `ε_r`: 0.95, the lunar regolith's (Hayne et al. 2017, JGR
/// Planets 122, 2371, Table A1, read).
pub const REGOLITH_EMISSIVITY: f64 = 0.95;

/// The thermal inertia of a regolith of rock, J m⁻² K⁻¹ s^−½: 55, the Moon's at 273 K (Hayne et
/// al. 2017, abstract and Table A1, read).
///
/// Hayne et al. give "55 ± 2" over the Moon's top 4–7 cm. Mercury's is of the same order: under
/// 50 over most of Mariner 10's transect, 19 at the surface and 71 at depth (Bandfield, Osterloo
/// and Holsclaw 2019, LPSC 50, abstract 2653, read). Provisional, until P14.T49.c's inertia by
/// layer (22 → 23). A body with air takes the same vacuum regolith's, though pore gas raises a
/// real one's (Mars's some 250, not re-read), so its ground's storage is under-read
/// (science-p14-t24a-climate.md §2).
pub const ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S: f64 = 55.0;

/// The thermal inertia of a regolith of ice, J m⁻² K⁻¹ s^−½: 20 (provisional,
/// science-p14-t24a-climate.md §2), within the 5–70 of the Saturnian icy moons, Pluto and
/// Europa (Howett et al. 2010, Icarus 206, 573; Lellouch et al. 2011, Icarus 214, 701; Rathbun et
/// al. 2010, Icarus 210, 763, Europa's 70; none re-read).
pub const ICE_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S: f64 = 20.0;

/// The fit (a, n) of a regolith's redistribution ε = x ÷ (1 + x), x = a Θ^n: (0.635, 0.905),
/// science-p14-t24a-climate.md §2's.
///
/// Θ is the thermal parameter Γ √ω ÷ (`ε_r` σ `T_ss`³) (Spencer, Lebofsky and Sykes 1989, Icarus
/// 78, 337, not re-read). The fit is to a 1-D model of constant inertia over seven latitudes,
/// whose night hemisphere emits 0.0053, 0.0246, 0.103, 0.254 and 0.480 of σ `T_eq`⁴ at
/// Θ = 0.0051, 0.028, 0.152, 0.506 and 1.52, each within 1%. It tends to 0 for a slow or locked
/// body and to 1, isothermal in longitude, as Θ grows. It is checked against Hayne et al.'s full
/// lunar regolith (a 102 K mean night on the Moon's equator) and Diviner's "about 95 K before
/// sunrise" (Williams et al. 2017, Icarus 283, 300).
///
/// It is fitted over Θ of 0.0051 to 1.52 and extrapolated beyond. Mercury's Θ, 0.0027, lies
/// below: its night, 102 K, is under the 117 K mean that the ruling's 1-D model of Hayne et al.'s
/// lunar regolith gives on Mercury's orbit-mean flux, and within Mariner 10's 100–130 K
/// (Bandfield, Osterloo and Holsclaw 2019).
pub const REGOLITH_REDISTRIBUTION_FIT: (f64, f64) = (0.635, 0.905);

/// The cosmic microwave background's temperature, the floor of a night with no internal heat:
/// 2.725 48 K (Fixsen 2009, ApJ 707, 916: 2.725 48 ± 0.000 57 K).
///
/// P14.T24.b's heat flow F raises the floor to (`T_CMB`⁴ + F ÷ `ε_r` σ)^¼.
pub const COSMIC_BACKGROUND_TEMPERATURE: Kelvin = Kelvin::new(2.725_48);

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
    /// What its surface is made of, from its record's composition, the bulk section's mass
    /// fractions ([`SurfaceMaterial::of`]), as its photometry section's Bond albedo reads it
    /// (P14.T47): the regolith whose thermal inertia carries the day's heat into the night.
    ///
    /// The equilibrium temperature is the derivation's, whose albedo reads the placement's
    /// fractions. For an icy belt member, a moon, a giant-impact moon or a capture the two can
    /// differ, by up to 8.2% in the equilibrium temperature, until P14.T51.b's passes read the
    /// record's composition (science-p14-t24a-climate.md §6, item 3).
    pub material: SurfaceMaterial,
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

/// A body's surface conditions at a time (P14.T24.a): its mean temperature, contrasts and range
/// of temperature, its surface pressure and gases, the light it receives, and whether water can
/// be liquid on it.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceConditions {
    mean_surface_temperature: Kelvin,
    equator_pole_contrast_k: f64,
    day_night_contrast_k: f64,
    climate_range: (Kelvin, Kelvin),
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
    /// use hyperion_sim::planetary::derive::atmosphere::SurfaceMaterial;
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
    ///     material: SurfaceMaterial::of(&derived.fractions()),
    ///     stellar_flux: WattsPerSquareMetre::from(derived.flux()),
    ///     xuv_flux: derived.xuv_flux(),
    ///     spin: SeasonalSpin {
    ///         obliquity: Radians::from(Degrees::new(23.44)),
    ///         rotation_period: Seconds::new(86_164.1),
    ///         solar_day: Some(Seconds::new(86_400.0)),
    ///     },
    /// })
    /// .expect("an Earth has a surface");
    /// // Its fractions are by number and sum to 1, its equator is the warmer, its poles stay
    /// // above 250 K, and water can be liquid on it.
    /// let sum = conditions.gases().iter().fold(0.0, |sum, &(_, x)| sum + x);
    /// assert!((sum - 1.0).abs() < 1e-9);
    /// assert!(conditions.equator_pole_contrast_k() > 30.0);
    /// let (coldest, warmest) = conditions.climate_range();
    /// assert!(coldest.value() > 250.0 && warmest.value() < 320.0);
    /// assert!(conditions.liquid_water());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn derive(inputs: &SurfaceInputs<'_>) -> Option<Self> {
        let Surface {
            column,
            moisture,
            gases,
            mean_molar_mass_g_per_mol,
            heat_capacity_over_r,
        } = Surface::of(inputs)?;
        // P14.T24.b's heat flow raises it.
        let floor = COSMIC_BACKGROUND_TEMPERATURE;
        let ends = zonal_ends_k(&column, &inputs.spin, moisture);
        let equator_pole = ends.0 - ends.1;
        let (day, night) = day_and_night(&column, &inputs.spin, inputs.material, floor);
        let range = climate_range(column.surface_temperature, ends, day, night, floor);
        Some(Self {
            mean_surface_temperature: column.surface_temperature,
            equator_pole_contrast_k: equator_pole,
            day_night_contrast_k: day.value() - night.value(),
            climate_range: range,
            surface_pressure: column.surface_pressure,
            gases,
            mean_molar_mass_g_per_mol,
            heat_capacity_over_r,
            stellar_flux: inputs.stellar_flux,
            radiation: RadiationClass::of(inputs.xuv_flux),
            liquid_water: liquid_water(range, column.surface_pressure),
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

    /// The climate's range of temperature, coldest then warmest: the span of its orbit-mean
    /// climate, from the coldest latitude's nightside to the warmest latitude's dayside, each a
    /// half-day mean on the orbit-averaged flux (science-p14-t24a-climate.md §3 and §6, item 1;
    /// P14.T48.d's).
    ///
    /// It is not the noon peak, the predawn minimum or a perihelion's, which rendering plan R09's
    /// field resolves.
    ///
    /// - The zonal profile's ends, at the equator and a pole, are those whose difference is the
    ///   equator–pole contrast: the radiative ends, `T_s` `q_eq`^¼ and `T_s` `q_pole`^¼, under the
    ///   transport that damps each of the insolation's Legendre modes at its own rate
    ///   (science-p14-t24a-climate.md §7). An airless body's are its radiative profile's, and a
    ///   strong transport's tend to `T_s`.
    /// - The warmest is the warmer end plus the dayside's excess over `T_s`, `T_day` − `T_s`.
    /// - The coldest is the colder end, at most `T_s`, less the nightside's deficit,
    ///   `T_s` − `T_night`. It never lies below the night's floor, the cosmic background's
    ///   ([`COSMIC_BACKGROUND_TEMPERATURE`]) until P14.T24.b's heat flow raises it.
    ///
    /// Cowan and Agol's day and night are not symmetric about `T_s`: with no redistribution the
    /// day stands 0.278 `T_s` above it and the night `T_s` below. An airless body's warmest lies
    /// between its dayside and its orbit-mean subsolar temperature, `T_eq` (4 ÷ `ε_r`)^¼. The
    /// signed contrast puts a warm-pole world's coldest at its equator.
    #[must_use]
    pub const fn climate_range(&self) -> (Kelvin, Kelvin) {
        self.climate_range
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
    /// temperature ([`climate_range`](Self::climate_range), [`liquid_water`]): a condition of the
    /// surface, whether or not the body holds water. From 22 → 23 P14.T51.a derives it from the
    /// partition.
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

/// Whether a world's meridional transport carries latent heat ([`DRY_TRANSPORT_SHARE`]) and its
/// cold zones brighten ([`WET_ALBEDO_GRADIENT`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Moisture {
    /// Its mean surface holds liquid water: the whole transport, and the brighter cold zones.
    Wet,
    /// It does not: half the transport, under a uniform albedo.
    Dry,
}

/// The column the contrasts read, as plain values ([`SurfaceConditions::derive`] builds it from
/// its inputs). Its fields carry no invariant: a temperature, pressure, mass, gravity or radius
/// that is not positive gives no contrast or no transport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Column {
    /// The mean surface temperature.
    pub(crate) surface_temperature: Kelvin,
    /// The equilibrium temperature: σ `T_eq`⁴ is the absorbed mean flux.
    pub(crate) equilibrium_temperature: Kelvin,
    /// The surface pressure.
    pub(crate) surface_pressure: Pascals,
    /// The air's mass per unit area, `p_s` ÷ g, kg m⁻²: zero for no air.
    pub(crate) air_mass_kg_per_m2: f64,
    /// The air's specific heat at constant pressure, J kg⁻¹ K⁻¹: zero for no air.
    pub(crate) specific_heat_j_per_kg_k: f64,
    /// The air's mean molar mass μ, g mol⁻¹: zero for no air.
    pub(crate) molar_mass_g_per_mol: f64,
    /// The mean surface gravity.
    pub(crate) surface_gravity: MetresPerSecondSquared,
    /// The grey optical depth of the greenhouse (P14.T13.c).
    pub(crate) optical_depth: f64,
    /// The greenhouse gain g of the gases held at saturation ([`greenhouse_gain`]): zero for
    /// none.
    pub(crate) greenhouse_gain: f64,
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

    /// The radiative restoring at the surface, `B_P` = 4F ÷ `T_s`, W m⁻² K⁻¹: the grey column's
    /// dOLR ÷ d`T_s` at fixed τ.
    #[must_use]
    pub(crate) fn restoring_w_per_m2_k(&self) -> f64 {
        4.0 * self.absorbed().value() / self.surface_temperature.value()
    }

    /// The radiative restoring with the column's own vapour feedback, `B_eff` = `B_P` (1 − g),
    /// W m⁻² K⁻¹, never negative ([`greenhouse_gain`]; science-p14-t24a-climate.md §1).
    #[must_use]
    pub(crate) fn effective_restoring_w_per_m2_k(&self) -> f64 {
        self.restoring_w_per_m2_k() * (1.0 - self.greenhouse_gain).max(0.0)
    }

    /// The air's heat capacity per unit area, `c_p` (`p_s` ÷ g), J m⁻² K⁻¹: zero for no air.
    #[must_use]
    pub(crate) fn heat_capacity_j_per_m2_k(&self) -> f64 {
        self.specific_heat_j_per_kg_k * self.air_mass_kg_per_m2
    }

    /// The air's specific gas constant `R_air` = R ÷ μ, J kg⁻¹ K⁻¹: infinite for no air, whose μ
    /// is zero.
    #[must_use]
    fn gas_constant_j_per_kg_k(&self) -> f64 {
        MOLAR_GAS_CONSTANT / (self.molar_mass_g_per_mol * 1e-3)
    }
}

/// What [`SurfaceConditions::derive`] reads off its inputs before the contrasts: the column, its
/// moisture and its gases.
#[derive(Debug, Clone, PartialEq)]
struct Surface {
    column: Column,
    moisture: Moisture,
    gases: Vec<(SubstanceId, f64)>,
    mean_molar_mass_g_per_mol: Option<f64>,
    heat_capacity_over_r: Option<f64>,
}

impl Surface {
    /// The surface of the body `inputs` describe; `None` in the gas-envelope state. An airless
    /// body's column holds no air, whatever residue its partial pressures hold.
    #[must_use]
    fn of(inputs: &SurfaceInputs<'_>) -> Option<Self> {
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
            air_mass_kg_per_m2: column_mass_kg_per_m2(surface_pressure, inputs.surface_gravity),
            specific_heat_j_per_kg_k: match (mean_molar_mass, heat_capacity) {
                (Some(mu), Some(cp)) => specific_heat_j_per_kg_k(cp, mu),
                (None, _) | (_, None) => 0.0,
            },
            molar_mass_g_per_mol: mean_molar_mass.unwrap_or(0.0),
            surface_gravity: inputs.surface_gravity,
            optical_depth: if airless { 0.0 } else { air.optical_depth() },
            greenhouse_gain: if airless {
                0.0
            } else {
                greenhouse_gain(air, &gases, inputs.equilibrium_temperature)
            },
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
        Some(Self {
            column,
            moisture,
            gases,
            mean_molar_mass_g_per_mol: mean_molar_mass,
            heat_capacity_over_r: heat_capacity,
        })
    }
}

/// The greenhouse gain g of the atmosphere `air`, whose published mole fractions are `gases`,
/// over a surface in equilibrium at `equilibrium`: the share of the grey column's Planck
/// restoring that its condensing gases' vapour feedback takes back, `B_eff` = `B_P` (1 − g)
/// (science-p14-t24a-climate.md §1).
///
/// P14.T13.c holds each condensing gas at its saturation pressure at `T_s`, so the column's
/// outgoing flux σ `T_s`⁴ ÷ (1 + ¾τ(`T_s`)) has the slope `B_P` (1 − g), with
///
/// g = (`T_s` ÷ 4) ¾ Σᵢ (1 − xᵢ) ½τᵢ Lᵢ ÷ (R `T_s`²) ÷ (1 + ¾τ),
///
/// summed in their order over T13.c's τ terms whose gas it holds at saturation, where its
/// partial pressure is at least the saturation pressure at `T_s` (equal to the bit, since T13.c
/// took the lesser of the two at the same temperature):
///
/// - ½τᵢ is ∂τᵢ ÷ ∂ln pᵢ for the terms in √pᵢ;
/// - Lᵢ ÷ (R `T_s`²) is the exact slope of the registry's curve, with the enthalpy of its branch
///   in force, of sublimation below the triple point and of vaporisation at and above it;
/// - xᵢ is the gas's published mole fraction: the factor 1 − xᵢ removes a dominant condensable,
///   whose pressure the cold trap sets globally, not locally, continuously in its share.
///
/// Earth's measured state gives 0.38 (`B_eff` 2.08 W m⁻² K⁻¹), the counterpart of Kaspi and
/// Showman's prescribed τ, which they state stands in for water vapour, with no feedback of its
/// own. Zero where no gas is held, or at a surface not above 0 K.
#[must_use]
pub(crate) fn greenhouse_gain(
    air: &Atmosphere,
    gases: &[(SubstanceId, f64)],
    equilibrium: Kelvin,
) -> f64 {
    let surface = air.surface_temperature();
    let t = surface.value();
    if t.is_nan() || t <= 0.0 {
        return 0.0;
    }
    let partials = air.partial_pressures();
    let slope =
        optical_depth_terms(partials, equilibrium)
            .into_iter()
            .fold(0.0, |sum, (gas, term)| {
                let id = gas.substance();
                let Some(phase) = id.substance().phase() else {
                    return sum;
                };
                if partials.of(gas) < phase.saturation_pressure(surface) {
                    return sum;
                }
                let enthalpy = if t < phase.triple_temperature().value() {
                    phase.sublimation_enthalpy_j_per_mol()
                } else {
                    phase.vaporisation_enthalpy_j_per_mol()
                };
                let share = gases
                    .iter()
                    .find(|&&(gas, _)| gas == id)
                    .map_or(0.0, |&(_, x)| x);
                sum + (1.0 - share) * 0.5 * term * enthalpy / (MOLAR_GAS_CONSTANT * t * t)
            });
    grey_gain(surface, air.optical_depth(), slope)
}

/// The gain (`T_s` ÷ 4) ¾ τ′ ÷ (1 + ¾τ) of a grey column of optical depth `optical_depth` over a
/// surface at `surface`, whose τ rises with the surface temperature at τ′ =
/// `slope_per_k` per kelvin ([`greenhouse_gain`]).
#[must_use]
fn grey_gain(surface: Kelvin, optical_depth: f64, slope_per_k: f64) -> f64 {
    surface.value() / 4.0 * 0.75 * slope_per_k / (1.0 + 0.75 * optical_depth)
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

/// The meridional conductance D of the column `column` under the spin `spin`, W m⁻² K⁻¹ (North,
/// Cahalan and Coakley's units), before the dry world's halving: Earth's,
/// [`EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K`], times
///
/// - the air's specific heat over [`EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K`];
/// - s(m ÷ m⊕) = (m ÷ m⊕)^0.2 in the column's mass ([`COLUMN_TRANSPORT_EXPONENT`]), linear to zero
///   below 0.2 and held above 50 ([`COLUMN_TRANSPORT_RANGE`]);
/// - h(g ÷ g₀) = (g ÷ g₀)^−0.43 in the gravity ([`GRAVITY_TRANSPORT_EXPONENT`]), held beyond
///   [`GRAVITY_TRANSPORT_RANGE`];
/// - f(Ω), [`rotation_transport_factor`];
///
/// each after Kaspi and Showman 2015's sweeps (science-p14-t24a-climate.md §1). The radius takes
/// no term of its own. Zero with no air or no gravity.
#[must_use]
pub(crate) fn meridional_conductance_w_per_m2_k(column: &Column, spin: &SeasonalSpin) -> f64 {
    let g = column.surface_gravity.value();
    let mass = column.air_mass_kg_per_m2 / EARTH_COLUMN_MASS_KG_PER_M2;
    if !(g > 0.0 && mass > 0.0) {
        return 0.0;
    }
    let (low, high) = COLUMN_TRANSPORT_RANGE;
    let mass_factor = if mass < low {
        math::powf(low, COLUMN_TRANSPORT_EXPONENT) * mass / low
    } else {
        math::powf(mass.min(high), COLUMN_TRANSPORT_EXPONENT)
    };
    let (g_low, g_high) = GRAVITY_TRANSPORT_RANGE;
    let gravity = (g / STANDARD_GRAVITY.value()).clamp(g_low, g_high);
    EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K
        * (column.specific_heat_j_per_kg_k / EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K)
        * mass_factor
        * math::powf(gravity, GRAVITY_TRANSPORT_EXPONENT)
        * rotation_transport_factor(spin.rotation_period)
}

/// Koll's weak-temperature-gradient transport over the dry restoring, W of the module's
/// equator–pole model: X ÷ k (his ε ÷ (1 − ε), [`heat_redistribution`]), weighted by
/// 1 ÷ (1 + (3a ÷ `L_Ro`)²), with a ÷ `L_Ro` = 2Ωa ÷ c, c = `R_air` √(`T_eq` ÷ `c_p`) the gravity
/// waves' speed (Koll 2022, §3.1) and Ω the sidereal rate, 0 for a body that does not turn
/// ([`WEAK_GRADIENT_DEFORMATION_RADII`]; science-p14-t24a-climate.md §1 and §4). Zero with no air
/// or no greenhouse.
#[must_use]
pub(crate) fn weak_gradient_transport(column: &Column, spin: &SeasonalSpin) -> f64 {
    let x = redistribution_parameter(column);
    let k = redistribution_scale(column);
    if !(x > 0.0 && k > 0.0) {
        return 0.0;
    }
    let wave = column.gas_constant_j_per_kg_k()
        * (column.equilibrium_temperature.value() / column.specific_heat_j_per_kg_k).sqrt();
    let period = spin.rotation_period.value();
    let omega = if period.is_finite() && period > 0.0 {
        core::f64::consts::TAU / period
    } else {
        0.0
    };
    let ratio = WEAK_GRADIENT_DEFORMATION_RADII * 2.0 * omega * column.radius.value() / wave;
    (x / k) / (1.0 + ratio * ratio)
}

/// The annual-mean equator–pole contrast of the column `column` under the spin `spin`, K:
/// T(0°) − T(90°), signed (the module's model): the difference of the zonal ends
/// ([`zonal_ends_k`]; science-p14-t24a-climate.md §7).
///
/// Zero where no light is absorbed. It changes sign at 53.9° on an airless body, where the
/// equator's and the poles' light are equal, and toward 54.7°, where `P₂`(cos β) vanishes, as the
/// transport grows. It is negative beyond, up to the mirror angles near 126°.
///
/// [`SurfaceConditions::derive`] takes the same difference of the ends it solves once, so this
/// form is the tests'.
#[cfg(test)]
#[must_use]
pub(crate) fn equator_pole_contrast_k(
    column: &Column,
    spin: &SeasonalSpin,
    moisture: Moisture,
) -> f64 {
    let (equator, pole) = zonal_ends_k(column, spin, moisture);
    equator - pole
}

/// The even Legendre degree to which the zonal ends sum the annual insolation's modes one by one:
/// 32, in 16 modes.
///
/// The modes beyond enter as their tail at degree 34's response (science-p14-t24a-climate.md §7).
/// The tail is known exactly, since the modes sum to the insolation at each end, so the ends
/// converge by degree 16 to 10⁻⁴ `T_s`.
const ZONAL_MODES: usize = 16;

/// The bisection steps that solve each zonal end for its own restoring: 64, past the last bit of
/// its bracket, which doubles from 2 `T_s` at most as many times.
const ZONAL_END_STEPS: u32 = 64;

/// One even Legendre mode of the annual-mean insolation over S ÷ 4 ([`insolation_modes`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InsolationMode {
    /// Its degree n.
    pub(crate) degree: f64,
    /// Its coefficient qₙ.
    pub(crate) coefficient: f64,
    /// Pₙ(0), its value at the equator; Pₙ(1) = 1 at a pole.
    pub(crate) at_equator: f64,
}

/// The even Legendre modes of the annual-mean insolation over S ÷ 4 at the obliquity `obliquity`,
/// degrees 2 to 32 (science-p14-t24a-climate.md §7).
///
/// The daily-mean insolation is the longitude mean of max(0, μ) = Σ hₙ Pₙ(μ). The addition
/// theorem, once over the day and once over the year along the ecliptic, gives
/// q(x) = Σₙ qₙ Pₙ(x), with qₙ = 4 hₙ Pₙ(0) Pₙ(cos β), where:
/// - hₙ = (2n + 1) ÷ 2 × cₙ, with cₙ = ∫₀¹ μ Pₙ(μ) dμ, c₂ = 1/8 and
///   c_{n+2} = −(n − 1) ÷ (n + 4) cₙ;
/// - P_{n+2}(0) = −(n + 1) ÷ (n + 2) Pₙ(0);
/// - Pₙ(cos β) is summed by Bonnet's recurrence.
///
/// The odd modes vanish, the year being symmetric. q₂ = −(5/8) `P₂`(cos β) is North's (1975)
/// `s₂`, −0.477 at 23.44°.
#[must_use]
pub(crate) fn insolation_modes(obliquity: Radians) -> [InsolationMode; ZONAL_MODES] {
    let c = math::cos(obliquity.value());
    let mut modes = [InsolationMode {
        degree: 0.0,
        coefficient: 0.0,
        at_equator: 0.0,
    }; ZONAL_MODES];
    // Bonnet's recurrence, (n + 1) P_{n+1} = (2n + 1) c Pₙ − n P_{n−1}, from P₀ and P₁.
    let (mut previous, mut current, mut n) = (1.0, c, 1.0_f64);
    let (mut integral, mut at_equator) = (0.125, -0.5);
    for mode in &mut modes {
        (previous, current) = (
            current,
            ((2.0 * n + 1.0) * c * current - n * previous) / (n + 1.0),
        );
        n += 1.0;
        *mode = InsolationMode {
            degree: n,
            coefficient: 2.0 * (2.0 * n + 1.0) * integral * at_equator * current,
            at_equator,
        };
        integral *= -(n - 1.0) / (n + 4.0);
        at_equator *= -(n + 1.0) / (n + 2.0);
        (previous, current) = (
            current,
            ((2.0 * n + 1.0) * c * current - n * previous) / (n + 1.0),
        );
        n += 1.0;
    }
    modes
}

/// What the zonal ends read of a column's restoring and transport, W m⁻² K⁻¹ ([`zonal_ends_k`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ZonalTransport {
    /// The grey column's restoring at fixed τ, `B_P`.
    pub(crate) planck: f64,
    /// Its restoring with its own vapour feedback, `B_eff`.
    pub(crate) effective: f64,
    /// The meridional conductance D, after the dry world's share.
    pub(crate) conductance: f64,
    /// Koll's weak-gradient transport as a conductance, W `B_P`.
    pub(crate) weak: f64,
    /// Φ, the brightening of the cold zones that multiplies the `P₂` forcing:
    /// [`WET_ALBEDO_GRADIENT`] on a wet world, 1 on a dry one.
    pub(crate) albedo: f64,
}

/// An end of the zonal profile ([`zonal_ends_under`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZonalEnd {
    /// The equator, x = sin φ = 0.
    Equator,
    /// A pole, x = 1.
    Pole,
}

impl ZonalEnd {
    /// The mode `mode`'s Legendre polynomial at the end: Pₙ(0) at the equator, 1 at a pole.
    fn legendre(self, mode: &InsolationMode) -> f64 {
        match self {
            Self::Equator => mode.at_equator,
            Self::Pole => 1.0,
        }
    }
}

/// The emitted flux over the mean at the end `end`, of insolation `insolation` over S ÷ 4 and
/// modes `modes`, under the transport `transport` at the end's restoring over `B_P`, `scale`
/// ([`zonal_ends_under`]).
fn end_flux(
    end: ZonalEnd,
    insolation: f64,
    modes: &[InsolationMode; ZONAL_MODES],
    transport: &ZonalTransport,
    scale: f64,
) -> f64 {
    let response = |degree: f64| {
        transport.planck
            / (transport.effective
                + (degree * (degree + 1.0) * transport.conductance).max(transport.weak) / scale)
    };
    let first = modes[0];
    let mut flux = insolation
        + (transport.albedo - 1.0)
            * first.coefficient
            * end.legendre(&first)
            * response(first.degree);
    let mut partial = 1.0;
    for mode in modes {
        let value = mode.coefficient * end.legendre(mode);
        flux += value * (response(mode.degree) - 1.0);
        partial += value;
    }
    flux + (insolation - partial) * (response(modes[ZONAL_MODES - 1].degree + 2.0) - 1.0)
}

/// The annual-mean surface temperatures at the equator and at a pole of the column `column` under
/// the spin `spin`, K ([`zonal_ends_under`]; science-p14-t24a-climate.md §7). Their difference is
/// the equator–pole contrast. Both are `T_s` where no light is absorbed, or where nothing restores
/// the column.
#[must_use]
pub(crate) fn zonal_ends_k(column: &Column, spin: &SeasonalSpin, moisture: Moisture) -> (f64, f64) {
    let t_s = column.surface_temperature.value();
    let planck = column.restoring_w_per_m2_k();
    if !(planck.is_finite() && planck > 0.0 && t_s > 0.0) {
        return (t_s, t_s);
    }
    let (share, albedo) = match moisture {
        Moisture::Wet => (1.0, WET_ALBEDO_GRADIENT),
        Moisture::Dry => (DRY_TRANSPORT_SHARE, 1.0),
    };
    let transport = ZonalTransport {
        planck,
        effective: column.effective_restoring_w_per_m2_k(),
        conductance: meridional_conductance_w_per_m2_k(column, spin) * share,
        weak: weak_gradient_transport(column, spin) * planck,
        albedo,
    };
    zonal_ends_under(t_s, &transport, spin.obliquity)
}

/// The annual-mean surface temperatures at the equator and at a pole, K, of a profile about the
/// mean `surface_k` under the transport `transport`, at the obliquity `obliquity`: the
/// insolation's Legendre modes ([`insolation_modes`]), each damped at its own rate in the emitted
/// flux, at each end's own restoring (science-p14-t24a-climate.md §7).
///
/// Written in the emitted flux E, the transport −∇·(D∇T) diffuses at D ÷ `B_local`, with
/// `B_local` = 4E ÷ T. So the mode n of the flux is damped by
///
/// rₙ = `B_P` ÷ (`B_eff` + max(n(n + 1) D, W `B_P`) ÷ s),   s = (T ÷ `T_s`)³,
///
/// and each end's flux over the mean is
///
/// E ÷ F = q + (Φ − 1) q₂ `P₂` r₂ + Σₙ qₙ Pₙ (rₙ − 1) + (q − Σₙ qₙ Pₙ)(r₃₄ − 1),
///
/// with T = `T_s` (E ÷ F)^¼.
/// - The sum runs to degree 32, and the tail beyond takes degree 34's response; the modes sum to
///   q at each end.
/// - Φ multiplies the `P₂` forcing, the coalbedo gradient it stands for.
/// - Each end solves for its own s by bisection over a fixed number of steps, since
///   T − T(E(T)) rises with T.
///
/// The limits:
/// - With no transport the ends are the radiative ones, `T_s` q^¼, bit for bit on an airless
///   body.
/// - With strong transport they are the linear modes' response, whose `P₂` term is North's
///   (15/64) `P₂`(cos β) `T_s` damped by 1 + 6D ÷ B.
///
/// Against a nonlinear one-dimensional diffusive balance over obliquities of 0–98° and 6D ÷ B of
/// 0.001–50, the contrast lies within 5.7%, the equator within 0.0013 `T_s` and the pole within
/// 0.016 `T_s`. Damping the whole radiative contrast at `P₂`'s rate overstated it up to 4.3 times
/// at low obliquity, where it is a narrow polar cap that any transport erases. Both ends are
/// `surface_k` where nothing restores the column.
#[must_use]
pub(crate) fn zonal_ends_under(
    surface_k: f64,
    transport: &ZonalTransport,
    obliquity: Radians,
) -> (f64, f64) {
    // Only a column whose gain cancels its restoring and that has no transport, which no air
    // with a gravity has, leaves nothing to restore it.
    let total = transport.effective + (6.0 * transport.conductance).max(transport.weak);
    if total.is_nan() || total <= 0.0 {
        return (surface_k, surface_k);
    }
    let modes = insolation_modes(obliquity);
    let (equator, pole) = annual_insolation(obliquity);
    let transported = transport.conductance > 0.0 || transport.weak > 0.0;
    let solve = |end: ZonalEnd, insolation: f64| {
        let temperature = |scale: f64| {
            surface_k
                * math::powf(
                    end_flux(end, insolation, &modes, transport, scale).max(0.0),
                    0.25,
                )
        };
        if !transported {
            return temperature(1.0);
        }
        let excess = |t: f64| t - temperature(math::powi(t / surface_k, 3));
        let mut high = 2.0 * surface_k;
        for _ in 0..ZONAL_END_STEPS {
            if excess(high) > 0.0 {
                break;
            }
            high *= 2.0;
        }
        let mut low = 0.0;
        for _ in 0..ZONAL_END_STEPS {
            let middle = f64::midpoint(low, high);
            if excess(middle) > 0.0 {
                high = middle;
            } else {
                low = middle;
            }
        }
        f64::midpoint(low, high)
    };
    (
        solve(ZonalEnd::Equator, equator),
        solve(ZonalEnd::Pole, pole),
    )
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

/// Koll's parameter X of the column `column`: `τ_LW`^⅓ (`p_s` ÷ 1 bar)^⅔ (`T_eq` ÷ 600 K)^−4/3,
/// with `τ_LW` = (3/2)τ ([`FLUX_OPTICAL_DEPTH_PER_GREY`]; Koll 2022, ApJ 924, 134, eqs. 9–10).
/// Zero with no air, no greenhouse or no light.
#[must_use]
pub(crate) fn redistribution_parameter(column: &Column) -> f64 {
    let (tau, p, t_eq) = (
        column.optical_depth,
        column.surface_pressure.value(),
        column.equilibrium_temperature.value(),
    );
    if !(tau > 0.0 && p > 0.0 && t_eq > 0.0) {
        return 0.0;
    }
    math::cbrt(FLUX_OPTICAL_DEPTH_PER_GREY * tau)
        * math::powf(p / REDISTRIBUTION_REFERENCE_PRESSURE.value(), 2.0 / 3.0)
        * math::powf(
            t_eq / REDISTRIBUTION_REFERENCE_TEMPERATURE.value(),
            -4.0 / 3.0,
        )
}

/// Koll's k of the column `column`, the body's own by his eq. 9's definition:
/// k = `n_LW` L g ÷ (χ `R_air`) × (`C_d` σ² ÷ `R_air`)^⅓ (1 bar)^−⅔ (600 K)^4/3, with L its radius
/// and `R_air` = R ÷ μ ([`REDISTRIBUTION_BROADENING`], [`REDISTRIBUTION_INEFFICIENCY`],
/// [`REDISTRIBUTION_DRAG_COEFFICIENT`]). It gives 1.23, 1.89 and 2.35 at his three planets, against
/// his 1.2, 1.9 and 2.3, and goes as a g μ^4/3: Earth's is 1.3, a Titan's 0.07 and a Venus's 2.1
/// (science-p14-t24a-climate.md §4).
/// Zero for no air, whose μ is zero.
#[must_use]
pub(crate) fn redistribution_scale(column: &Column) -> f64 {
    let r_air = column.gas_constant_j_per_kg_k();
    REDISTRIBUTION_BROADENING * column.radius.value() * column.surface_gravity.value()
        / (REDISTRIBUTION_INEFFICIENCY * r_air)
        * math::cbrt(REDISTRIBUTION_DRAG_COEFFICIENT * STEFAN_BOLTZMANN * STEFAN_BOLTZMANN / r_air)
        * math::powf(REDISTRIBUTION_REFERENCE_PRESSURE.value(), -2.0 / 3.0)
        * math::powf(REDISTRIBUTION_REFERENCE_TEMPERATURE.value(), 4.0 / 3.0)
}

/// Koll's heat-redistribution efficiency ε of the air of the column `column`, from 0 (none) to 1
/// (uniform): X ÷ (k + X) ([`redistribution_parameter`], [`redistribution_scale`]; Koll 2022,
/// whose f = 2/3 − (5/12) ε is Cowan and Agol 2011's with this ε).
///
/// Zero with no air, no greenhouse or no light, and 1 where k is not positive.
#[must_use]
pub(crate) fn heat_redistribution(column: &Column) -> f64 {
    let x = redistribution_parameter(column);
    let k = redistribution_scale(column);
    if x.is_nan() || x <= 0.0 {
        0.0
    } else if !(x.is_finite() && k > 0.0) {
        1.0
    } else {
        x / (k + x)
    }
}

/// The regolith's redistribution efficiency ε of the ground of the column `column` under the spin
/// `spin`, of the material `material`: x ÷ (1 + x) with x = 0.635 Θ^0.905
/// ([`REGOLITH_REDISTRIBUTION_FIT`]), Θ = Γ √ω ÷ (`ε_r` σ `T_ss`³) the thermal parameter at the
/// material's inertia Γ ([`ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S`],
/// [`ICE_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S`]), ω = 2π over the solar day, and `T_ss` = `T_eq` (4 ÷ `ε_r`)^¼ the subsolar point's temperature
/// ([`REGOLITH_EMISSIVITY`]; science-p14-t24a-climate.md §2).
///
/// What the regolith stores by day it gives back by night, so its diurnal storage is a
/// redistribution. Zero for a world locked 1:1 to its seasonal host, which has no solar day, and
/// where no light is absorbed.
#[must_use]
pub(crate) fn ground_redistribution(
    column: &Column,
    spin: &SeasonalSpin,
    material: SurfaceMaterial,
) -> f64 {
    let Some(day) = spin.solar_day else {
        return 0.0;
    };
    let subsolar =
        column.equilibrium_temperature.value() * math::powf(4.0 / REGOLITH_EMISSIVITY, 0.25);
    let d = day.value();
    if !(d > 0.0 && subsolar > 0.0) {
        return 0.0;
    }
    let inertia = match material {
        SurfaceMaterial::Rock => ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S,
        SurfaceMaterial::Ice => ICE_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S,
    };
    let theta = inertia * (core::f64::consts::TAU / d).sqrt()
        / (REGOLITH_EMISSIVITY * STEFAN_BOLTZMANN * math::powi(subsolar, 3));
    regolith_redistribution(theta)
}

/// The regolith's redistribution efficiency at the thermal parameter `thermal_parameter`,
/// x ÷ (1 + x) with x = a Θ^n ([`REGOLITH_REDISTRIBUTION_FIT`]).
#[must_use]
fn regolith_redistribution(thermal_parameter: f64) -> f64 {
    let (a, n) = REGOLITH_REDISTRIBUTION_FIT;
    let x = a * math::powf(thermal_parameter, n);
    x / (1.0 + x)
}

/// The dayside's and the nightside's surface temperatures of the column `column` under the spin
/// `spin`, its ground of the material `material`, over a night floored at `floor` (the module's
/// model; science-p14-t24a-climate.md §2 and §4).
///
/// At a redistribution ε they are Cowan and Agol's (2011, eqs. 4–5), `T_s` (8/3 − 5ε/3)^¼ and
/// `T_s` ε^¼, where:
///
/// - 1 − ε = (1 − `ε_air`)(1 − `ε_ground`), the air's ([`heat_redistribution`]) and the ground's
///   ([`ground_redistribution`]) in series, so that a thin air joins the airless body
///   continuously;
/// - the floor adds its share of the absorbed flux, (`T_floor` ÷ `T_eq`)⁴, and ε is capped at 1.
///
/// On a rotating body each departure from `T_s` is damped by 1 ÷ √(1 + (ω C (1 − ε) ÷ `B_P`)²),
/// ω = 2π over the solar day and C the air's heat capacity per unit area: a slab's linear
/// response under the locked contrast's own restoring, `B_P` ÷ (1 − ε). `B_P`, not `B_eff`, since
/// over a day the column's vapour follows the surface's swing, not the air's. A locked world's are
/// about its substellar axis, undamped; an airless body's, with no air to store, are undamped too.
/// Both are `T_s` where no light is absorbed.
#[must_use]
pub(crate) fn day_and_night(
    column: &Column,
    spin: &SeasonalSpin,
    material: SurfaceMaterial,
    floor: Kelvin,
) -> (Kelvin, Kelvin) {
    let t_s = column.surface_temperature.value();
    let planck = column.restoring_w_per_m2_k();
    if !(planck.is_finite() && planck > 0.0 && t_s > 0.0) {
        return (column.surface_temperature, column.surface_temperature);
    }
    let through =
        (1.0 - heat_redistribution(column)) * (1.0 - ground_redistribution(column, spin, material));
    let floor_share = math::powi(floor.value() / column.equilibrium_temperature.value(), 4);
    let epsilon = (1.0 - through + floor_share).min(1.0);
    // A solar day that is not positive is no solar day, as `ground_redistribution` takes it.
    let damping = spin
        .solar_day
        .map(Seconds::value)
        .filter(|&day| day > 0.0)
        .map_or(1.0, |day| {
            1.0 / math::hypot(
                1.0,
                core::f64::consts::TAU / day * column.heat_capacity_j_per_m2_k() * (1.0 - epsilon)
                    / planck,
            )
        });
    let day = t_s * math::powf(8.0 / 3.0 - 5.0 / 3.0 * epsilon, 0.25);
    let night = t_s * math::powf(epsilon, 0.25);
    (
        Kelvin::new(t_s + damping * (day - t_s)),
        Kelvin::new(t_s - damping * (t_s - night)),
    )
}

/// The climate's range of temperature, coldest then warmest, of a mean surface temperature
/// `surface` whose zonal profile ends at `ends_k`, K, the equator's then a pole's
/// ([`zonal_ends_k`]),
/// with a dayside at `day` and a nightside at `night`, over a night floored at `floor`
/// ([`SurfaceConditions::climate_range`]; science-p14-t24a-climate.md §3 and §6, item 1):
///
/// - the coldest, max(`T_floor`, min(`T₀`, `T₉₀`, `T_s`) − (`T_s` − `T_night`));
/// - the warmest, max(`T₀`, `T₉₀`) + (`T_day` − `T_s`), and never below the coldest.
///
/// The colder end is bounded by `T_s`. From about 52° to 59° of obliquity both ends receive more
/// than the mean light, and the coldest zone lies at mid-latitudes, within 0.008 `T_s` of `T_s`.
#[must_use]
pub(crate) fn climate_range(
    surface: Kelvin,
    ends_k: (f64, f64),
    day: Kelvin,
    night: Kelvin,
    floor: Kelvin,
) -> (Kelvin, Kelvin) {
    let t_s = surface.value();
    let (equator, pole) = ends_k;
    let cold = (equator.min(pole).min(t_s) - (t_s - night.value())).max(floor.value());
    let warm = (equator.max(pole) + (day.value() - t_s)).max(cold);
    (Kelvin::new(cold), Kelvin::new(warm))
}

/// Whether water can be liquid at `pressure` somewhere over the climate's range `range`, coldest
/// then warmest ([`SurfaceConditions::climate_range`]; P14.T48.d's), by the registry's
/// [`phase_at`] on `H2O`'s row.
///
/// The registry's liquid at a pressure lies from the triple temperature to the boiling point, so
/// the range holds liquid exactly where its coldest point not below the triple temperature does.
#[must_use]
pub fn liquid_water(range: (Kelvin, Kelvin), pressure: Pascals) -> bool {
    let (coldest, warmest) = (range.0.value(), range.1.value());
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
