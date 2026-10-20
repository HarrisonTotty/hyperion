//! A body's climate regime (plan 14, P14.T48.d; the rendering brainstorm's open question 9): the
//! classifier's three fields, a thermal regime, a forcing and a condensable, and the coarse model
//! the regime names, which rendering plan R09's climate step runs (its Design note 8).
//!
//! [`ClimateRegime::of`] reads a body's surface conditions (P14.T24.a) with what the batch's later
//! steps give ([`ClimateInputs`]): its mass, the reservoirs its surface holds
//! ([`surface_reservoirs`]), whether it came from a colder state (P14.T48.b) and its ice cover
//! (P14.T24.b). It is step 10 of science-p14-t24b-laws §4.6. The regime reads the cover, not
//! P14.T13.c's `Snowball` label, wherever it asks "ice-covered" (§4.x.2), and the cover reads no
//! regime: a [`Forcing::IceBelt`]'s band is the zonal profile's cold equator, with no extra
//! hysteresis (§4.3). So the regime is classified after the cover, with no loop.
//!
//! # The thermal regime ([`ThermalRegime`])
//!
//! How much of the climate's heat the air moves from day to night.
//!
//! - **No air**: airless-like.
//! - **Otherwise** Koll's (2022, ApJ 924, 134) redistribution factor f = 2/3 − (5/12) X ÷ (k + X)
//!   (his eq. 10), at the body's own k (his eq. 9's definition) and `τ_LW` = (3/2)τ, as P14.T24.a
//!   computes it, decides: airless-like below X = [`KOLL_AIRLESS_LIKE_RATIO`] k, where f lies
//!   within 5% of a bare rock's 2/3, efficient above X = [`KOLL_EFFICIENT_RATIO`] k, where f lies
//!   within 10% of a uniform planet's ¼, and transitional between. Koll's scaling holds for a
//!   locked world; on a rotator the column's storage over the solar day damps the day–night
//!   departures by κ = 1 ÷ √(1 + (ω C (1 − ε) ÷ `B_P`)²), P14.T24.a's, so its effective
//!   redistribution, 1 − κ (1 − ε) with ε the air's, takes the thresholds
//!   (`effective_redistribution`; "main"'s ruling of 2026-10-13). That is Koll's own ε on a locked
//!   world and as the storage vanishes, continuous in the rotation. The generated Earth's κ is
//!   0.020, so efficient, and the measured Mars's, at 6 mbar, 0.27, so transitional.
//! - **Its thin end** is the air's collapse onto its night side, which leaves it airless-like, read
//!   in Koll's own regime: a locked world, or a rotator whose solar day is long against its
//!   column's effective radiative time (ω C (1 − ε) ÷ `B_P` ≤ 1, science-p14-t24a-climate §4). For
//!   pure carbon dioxide ([`PURE_CARBON_DIOXIDE_SHARE`]) on a body of 1 to 10 M⊕ under 0.2 to 3
//!   times Wordsworth's F₀, it is his (2015, ApJ 806, 180) eq. 43, the collapse pressure of his
//!   correlated-k GCMs ([`carbon_dioxide_collapse_pressure`]). Anywhere else it is where the bulk
//!   gas condenses at its partial pressure at P14.T24.a's nightside mean temperature
//!   (science-p14-t24a-climate §4: "cut where the bulk gas condenses"), unless the surface holds a
//!   reservoir of it: a bulk gas held at its saturation pressure condenses on its night as its own
//!   cycle, not as a collapse.
//!
//! # The forcing ([`Forcing`])
//!
//! What drives the climate's cycle, from the locking state, the solar day and the obliquity, in
//! this order:
//!
//! - **Locked**: a world locked 1:1 to its seasonal host, which has no solar day.
//! - **A slow rotator**: a sidereal rotation period at least the onset, which rises with the flux
//!   ([`slow_rotator_onset`]): Yang et al. 2014's Table 1 brackets it between 8 and 16 d at
//!   1.40 S⊕ and between 32 and 48 d at 1.92 S⊕, at an orbital period of 225 d.
//! - **An ice belt**: a faster rotator whose equator takes less of the year's light than its poles
//!   (an obliquity of 53.9° to 126.1°, [`annual_insolation`]), which came from a colder state
//!   (P14.T48.b), since Kilic et al. 2018's belt "is only reached from more glaciated conditions",
//!   and whose ice cover is a belt: some of the sphere but not all, its poles free.
//! - **Seasonal** otherwise.
//!
//! # The condensable ([`climate_condensable`])
//!
//! Of the registry's condensables the air holds, in registry order, those whose phase changes
//! inside the climate's range ([`SurfaceConditions::climate_range`]), where the saturation
//! temperature at the partial pressure x `p_s` lies within it, or that the surface holds a
//! reservoir of; the one of these with the largest partial pressure, ties by registry order; or
//! none (signoff-3.md, item 1.12). Any later condensable row joins as data.
//!
//! # The coarse model ([`ClimateModelKind`])
//!
//! - **Radiative equilibrium with thermal inertia** for an airless-like world, and for a thin air:
//!   a transitional one whose meridional transport is under its radiative restoring, so that each
//!   latitude lies nearer its own radiative balance (the measured Mars, at 6 mbar: 0.069).
//! - **An isothermal surface**, a Venus's, for an efficient world whose climate's range is within
//!   the efficient threshold's own tolerance, its warmest emitting at most
//!   [`ISOTHERMAL_FLUX_SPREAD`] more than its coldest.
//! - **The body-fixed energy-balance model** for a locked world, and **the seasonal one** for
//!   every other.

use hyperion_surface::field::ClimateModelKind;

use crate::math;
use crate::planetary::derive::atmosphere::{Atmosphere, Gas, METHANE_RELATIVE_HUMIDITY};
use crate::planetary::hooks::surface::{
    AirTransport, SurfaceConditions, annual_insolation, atmosphere_mole_fractions,
};
use crate::substance::{self, Phase, SubstanceId, phase_at, saturation_pressure};
use crate::units::{Days, EarthMasses, Kelvin, Pascals, Seconds, WattsPerSquareMetre};

#[cfg(test)]
pub(crate) mod tests;

/// Koll's X ÷ k below which the air is airless-like: 0.087, where f = 2/3 − (5/12) X ÷ (k + X)
/// lies within 5% of a bare rock's 2/3, f = 0.6333 (rendering plan R09's Design note 3, its
/// research of 2026-09-29 on Koll 2022, ApJ 924, 134; the 5% is the lane's reading of the
/// figure, which it reproduces to three digits).
pub const KOLL_AIRLESS_LIKE_RATIO: f64 = 0.087;

/// Koll's X ÷ k above which the air redistributes efficiently: 15.7, where f lies within 10% of a
/// uniform planet's ¼, f = 0.2750 (rendering plan R09's Design note 3, as
/// [`KOLL_AIRLESS_LIKE_RATIO`]).
pub const KOLL_EFFICIENT_RATIO: f64 = 15.7;

/// How much more flux, as a share, an isothermal surface's warmest point may emit than its
/// coldest: 0.0998, Koll's efficient threshold's own tolerance, f ÷ ¼ − 1 at
/// X = [`KOLL_EFFICIENT_RATIO`] k (the lane's, plan 14's Risks).
///
/// A Venus's climate's range, 733.5 to 736.1 K, spreads its emission by 1.4%, and an Earth's,
/// 262 to 305 K, by 84%.
pub const ISOTHERMAL_FLUX_SPREAD: f64 =
    (2.0 / 3.0 - 5.0 / 12.0 * (KOLL_EFFICIENT_RATIO / (1.0 + KOLL_EFFICIENT_RATIO))) * 4.0 - 1.0;

/// The mole fraction of carbon dioxide from which an air is pure carbon dioxide for Wordsworth's
/// collapse pressure: 0.95.
///
/// The lane's, so that Mars's 0.951 and Venus's 0.965 count (NASA's fact sheets) and an air with a
/// tenth of something else does not. Wordsworth's (2015, ApJ 806, 180) runs were pure.
pub const PURE_CARBON_DIOXIDE_SHARE: f64 = 0.95;

/// The masses Wordsworth's collapse pressure is fitted at: 1 and 10 M⊕, his Table 2's two planets
/// (Wordsworth 2015, ApJ 806, 180, Table 1: radii 1.0 and 1.88 r⊕, gravities 9.8 and
/// 27.8 m s⁻²).
pub const COLLAPSE_FIT_MASSES: (EarthMasses, EarthMasses) =
    (EarthMasses::new(1.0), EarthMasses::new(10.0));

/// The stellar fluxes Wordsworth's collapse pressure is fitted over: 0.2 to 3.0 F₀ with F₀ = 1,366
/// W m⁻², 273.2 to 4,098 W m⁻² (Wordsworth 2015, Table 1 and §3.6).
pub const COLLAPSE_FIT_FLUXES: (WattsPerSquareMetre, WattsPerSquareMetre) = (
    WattsPerSquareMetre::new(0.2 * 1_366.0),
    WattsPerSquareMetre::new(3.0 * 1_366.0),
);

/// Wordsworth's eq. 43, log₁₀(`p_crit` ÷ 1 Pa) = c₁ F³ + c₂ F² + c₃ F + c₄ with F the stellar
/// flux in W m⁻², its coefficients (c₁, c₂, c₃, c₄) at 1 and at 10 M⊕.
///
/// They are Wordsworth's (2015, ApJ 806, 180, Table 2, read at source, arXiv:1412.5575): a
/// least-squares fit to his correlated-k GCMs of tidally locked planets with pure CO₂ air about
/// AD Leo, of surface albedo 0.2 and thermal inertia 250 tiu, whose minimum surface temperature
/// falls below the condensation temperature. It gives 0.14 bar at 1,366 W m⁻² on an Earth, his
/// abstract's figure, which he calls "a conservative upper limit" (§4).
pub const CARBON_DIOXIDE_COLLAPSE_FIT: [[f64; 4]; 2] = [
    [-0.84e-10, 0.73e-6, -0.0022, 6.01],
    [-0.81e-10, 0.63e-6, -0.0017, 6.34],
];

/// Yang et al.'s brackets of the slow-rotator onset: at each stellar flux, the sidereal rotation
/// periods between which the climate turns from a fast rotator's to a slow rotator's.
///
/// They are Yang, Boué, Fabrycky and Abbot 2014 (ApJ 787, L2, arXiv:1404.4992), Table 1's third
/// group, at an orbital period of 225 d with sidereal rotation periods (read at source by the
/// lane's science check): at 1,900 W m⁻², 1.396 S⊕, 8 d "tends to runaway warming" and 16 d is
/// stable; at 2,615 W m⁻², 1.921 S⊕, 32 d runs away and 48 d is stable. A slower rotator's
/// clouds converge on its substellar point and raise its albedo, so the rotation that holds it
/// off the runaway marks its circulation's change (rendering plan R09's Design note 3).
pub const SLOW_ROTATOR_BRACKETS: [(WattsPerSquareMetre, Days, Days); 2] = [
    (
        WattsPerSquareMetre::new(1_900.0),
        Days::new(8.0),
        Days::new(16.0),
    ),
    (
        WattsPerSquareMetre::new(2_615.0),
        Days::new(32.0),
        Days::new(48.0),
    ),
];

/// How close to its humidity's cap methane's partial pressure lies where the surface holds a
/// reservoir of it, as a share of the cap: 10⁻⁹, far beyond the rounding of P14.T24.f's
/// quadratic, which meets the cap to 10⁻¹² (its tests).
pub const RESERVOIR_CAP_TOLERANCE: f64 = 1e-9;

/// How much of a climate's heat its air moves from day to night, the classifier's thermal regime
/// (P14.T48.d, after Koll 2022's redistribution factor f = 2/3 − (5/12) X ÷ (k + X) at the body's
/// own k; the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ThermalRegime {
    /// Airless-like: no air, an air collapsed onto its night side, or X below 0.087 k, so that
    /// each point sits near its own radiative equilibrium.
    AirlessLike,
    /// Between the two, with a redistribution factor between 0.633 and 0.275.
    Transitional,
    /// Efficient redistribution, X above 15.7 k.
    Efficient,
}

/// What drives a climate's cycle, the classifier's forcing (P14.T48.d): from the locking state,
/// the solar day and the obliquity (the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Forcing {
    /// A fast rotator's seasons, its ice where its zonal profile is coldest: at the poles below
    /// 53.9° of obliquity, and at the equator above it unless it came from a colder state (an
    /// [`IceBelt`](Self::IceBelt) then).
    Seasonal,
    /// A fast rotator at an obliquity of 53.9–126.1°, where the equator takes less of the year's
    /// light than the poles, that reached its state from a colder one, with an equatorial ice belt
    /// (Kilic et al. 2018, ApJ 864, 106, arXiv:1803.10751, whose belts form
    /// "above 54°" and are reached only from a colder state; with P14.T48.b's history). Its band
    /// is the zonal profile's cold equator, with no extra hysteresis (science-p14-t24b-laws §4.3).
    IceBelt,
    /// A slow rotator, past the onset that rises with flux, its circulation converging on the
    /// substellar point: in Yang et al. 2014's Table 1 (sidereal rotation periods, at an orbital
    /// period of 225 d), between 8 and 16 d at 1.40 S⊕ and between 32 and 48 d at 1.92 S⊕.
    SlowRotator,
    /// A world locked 1:1 to its seasonal host, in tidally locked coordinates about the
    /// substellar axis.
    Locked,
}

/// What [`ClimateRegime::of`] reads of a body beyond its surface conditions, as plain values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClimateInputs<'a> {
    /// The body's mass, which Wordsworth's collapse pressure reads.
    pub mass: EarthMasses,
    /// The condensables its surface holds a reservoir of, in registry order
    /// ([`surface_reservoirs`]).
    pub reservoirs: &'a [SubstanceId],
    /// Whether it reached its present state from a colder one (P14.T48.b,
    /// [`VolatileHistory::from_colder`](crate::planetary::hooks::figures::VolatileHistory::from_colder)).
    pub from_colder: bool,
    /// Its ice cover, the share of its surface under perennial ice, 0 to 1 (P14.T24.b,
    /// [`GlobalFigures::ice_fraction`](crate::planetary::hooks::figures::GlobalFigures::ice_fraction)).
    pub ice_fraction: f64,
    /// The temperature below which its ice cover lies
    /// ([`GlobalFigures::ice_threshold`](crate::planetary::hooks::figures::GlobalFigures::ice_threshold)).
    pub ice_threshold: Kelvin,
}

/// A body's climate regime, the classifier of the rendering brainstorm's open question 9
/// (P14.T48.d): its three fields and the coarse model it names (the [module](self)
/// documentation).
///
/// Rendering plan R09's coarse pass reads it as its own
/// [`ClimateRegime`](crate::planetary::surface::ClimateRegime), each substance by its registry
/// key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClimateRegime {
    thermal: ThermalRegime,
    forcing: Forcing,
    condensable: Option<SubstanceId>,
    model: ClimateModelKind,
}

impl ClimateRegime {
    /// The regime of the body whose surface conditions are `conditions`, with `inputs` (the
    /// [module](self) documentation).
    ///
    /// # Examples
    ///
    /// Earth, as [`SurfaceConditions::derive`]'s example derives it, takes the seasonal
    /// energy-balance model with water:
    ///
    /// ```
    /// # use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// # use hyperion_sim::planetary::derive::atmosphere::SurfaceMaterial;
    /// # use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
    /// # use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    /// # use hyperion_sim::planetary::hooks::surface::{
    /// #     OceanWeight, SeasonalSpin, SurfaceConditions, SurfaceInputs,
    /// # };
    /// # use hyperion_sim::stellar::Composition;
    /// # use hyperion_sim::stellar::draws::UnitUniform;
    /// # use hyperion_sim::time::UniverseTime;
    /// # use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
    /// # use hyperion_sim::units::{
    /// #     Degrees, Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres,
    /// #     Radians, Seconds, SolarLuminosities, SolarMasses, SolarRadii, WattsPerSquareMetre, Years,
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
    /// # let derived = derive_body(&earth, &hosts, disc, Years::new(4.57e9), UniverseTime::EPOCH)?;
    /// # let conditions = SurfaceConditions::derive(&SurfaceInputs {
    /// #     atmosphere: derived.atmosphere(),
    /// #     equilibrium_temperature: derived.equilibrium_temperature(),
    /// #     surface_gravity: derived.surface_gravity(),
    /// #     radius: Metres::from(derived.radius()),
    /// #     material: SurfaceMaterial::of(&derived.fractions()),
    /// #     stellar_flux: WattsPerSquareMetre::from(derived.flux()),
    /// #     xuv_flux: derived.xuv_flux(),
    /// #     spin: SeasonalSpin {
    /// #         obliquity: Radians::from(Degrees::new(23.44)),
    /// #         rotation_period: Seconds::new(86_164.1),
    /// #         solar_day: Some(Seconds::new(86_400.0)),
    /// #     },
    /// #     heat_flow: WattsPerSquareMetre::new(0.0925),
    /// #     ocean_weight: OceanWeight::FULL,
    /// # })
    /// # .expect("an Earth has a surface");
    /// use hyperion_sim::planetary::derive::atmosphere::Gas;
    /// use hyperion_sim::planetary::hooks::figures::cover::PERENNIAL_ICE_TEMPERATURE;
    /// use hyperion_sim::planetary::hooks::surface::regime::{
    ///     ClimateInputs, ClimateRegime, Forcing, ThermalRegime, surface_reservoirs,
    /// };
    /// use hyperion_surface::field::ClimateModelKind;
    ///
    /// let reservoirs = surface_reservoirs(derived.atmosphere());
    /// let regime = ClimateRegime::of(&conditions, &ClimateInputs {
    ///     mass: EarthMasses::new(1.0),
    ///     reservoirs: &reservoirs,
    ///     from_colder: false,
    ///     ice_fraction: 0.03,
    ///     ice_threshold: PERENNIAL_ICE_TEMPERATURE,
    /// });
    /// assert_eq!(regime.thermal(), ThermalRegime::Efficient);
    /// assert_eq!(regime.forcing(), Forcing::Seasonal);
    /// assert_eq!(regime.condensable(), Some(Gas::Water.substance()));
    /// assert_eq!(regime.model(), ClimateModelKind::EnergyBalance);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn of(conditions: &SurfaceConditions, inputs: &ClimateInputs<'_>) -> Self {
        let thermal = thermal_regime(conditions, inputs);
        let forcing = forcing(conditions, inputs);
        let condensable = climate_condensable(
            conditions.gases(),
            conditions.surface_pressure(),
            conditions.climate_range(),
            inputs.reservoirs,
        );
        Self {
            thermal,
            forcing,
            condensable,
            model: coarse_model(conditions, thermal, forcing),
        }
    }

    /// How much of the climate's heat the air moves.
    #[must_use]
    pub const fn thermal(&self) -> ThermalRegime {
        self.thermal
    }

    /// What drives the climate's cycle.
    #[must_use]
    pub const fn forcing(&self) -> Forcing {
        self.forcing
    }

    /// The climate's condensable, a registry row, or `None` ([`climate_condensable`]): of the
    /// condensables whose phase changes inside the climate's range, the one with the largest
    /// partial pressure in the air.
    #[must_use]
    pub const fn condensable(&self) -> Option<SubstanceId> {
        self.condensable
    }

    /// The coarse model the regime names, which rendering plan R09's field header carries (its
    /// Design note 8).
    #[must_use]
    pub const fn model(&self) -> ClimateModelKind {
        self.model
    }
}

/// The thermal regime of the body whose surface conditions are `conditions`, with `inputs` (the
/// [module](self) documentation).
#[must_use]
fn thermal_regime(conditions: &SurfaceConditions, inputs: &ClimateInputs<'_>) -> ThermalRegime {
    let pressure = conditions.surface_pressure();
    if conditions.gases().is_empty() || pressure.value().is_nan() || pressure.value() <= 0.0 {
        return ThermalRegime::AirlessLike;
    }
    let air = NightAir {
        gases: conditions.gases(),
        pressure,
        reservoirs: inputs.reservoirs,
        night: conditions.day_and_night.1,
    };
    thermal_of(&conditions.air, || {
        collapsed(&air, conditions.stellar_flux(), inputs.mass)
    })
}

/// The thermal regime of an air that carries its heat as `air` does, where `collapsed` tells
/// whether it has collapsed onto its night side ([`collapsed`]), which only Koll's own regime
/// reads (the [module](self) documentation): its effective redistribution
/// ([`effective_redistribution`]) against Koll's thresholds, airless-like where it has collapsed.
#[must_use]
pub(crate) fn thermal_of(air: &AirTransport, collapsed: impl FnOnce() -> bool) -> ThermalRegime {
    // Koll's own regime, where the thin end is read: a locked world, or a solar day long against
    // the column's radiative time.
    let kolls = !air.diurnal_storage.is_some_and(|storage| storage > 1.0);
    if kolls && collapsed() {
        ThermalRegime::AirlessLike
    } else {
        by_redistribution(effective_redistribution(air))
    }
}

/// The effective redistribution of an air that carries its heat as `air` does, 0 to 1: Koll's ε
/// of the air alone where the world is locked, and 1 − κ (1 − ε) on a rotator, κ = 1 ÷ √(1 +
/// (ω C (1 − ε) ÷ `B_P`)²) the column's damping of its day–night departures (P14.T24.a's).
///
/// It is continuous in the rotation and tends to Koll's ε as the column's storage over the solar
/// day vanishes ("main"'s ruling of 2026-10-13, the science check's form).
#[must_use]
pub(crate) fn effective_redistribution(air: &AirTransport) -> f64 {
    match air.diurnal_storage {
        Some(storage) => 1.0 - (1.0 - air.redistribution) / math::hypot(1.0, storage),
        None => air.redistribution,
    }
}

/// The thermal regime of an air of redistribution ε, X ÷ (k + X) for Koll's X and k: airless-like
/// below X = [`KOLL_AIRLESS_LIKE_RATIO`] k, efficient above X = [`KOLL_EFFICIENT_RATIO`] k.
#[must_use]
pub(crate) fn by_redistribution(redistribution: f64) -> ThermalRegime {
    let below = |ratio: f64| ratio / (1.0 + ratio);
    if redistribution.is_nan() || redistribution < below(KOLL_AIRLESS_LIKE_RATIO) {
        ThermalRegime::AirlessLike
    } else if redistribution > below(KOLL_EFFICIENT_RATIO) {
        ThermalRegime::Efficient
    } else {
        ThermalRegime::Transitional
    }
}

/// The air whose collapse onto its night side [`collapsed`] tests, as plain values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NightAir<'a> {
    /// Its gases as mole fractions, largest first.
    pub(crate) gases: &'a [(SubstanceId, f64)],
    /// Its surface pressure.
    pub(crate) pressure: Pascals,
    /// The condensables its surface holds a reservoir of ([`surface_reservoirs`]).
    pub(crate) reservoirs: &'a [SubstanceId],
    /// Its nightside's mean temperature, P14.T24.a's.
    pub(crate) night: Kelvin,
}

/// Whether the air `air`, on a body of mass `mass` under the orbit-averaged flux `flux`, has
/// collapsed onto its night side, the thermal regime's thin end (the [module](self)
/// documentation).
///
/// - Pure carbon dioxide within Wordsworth's fit collapses below his collapse pressure
///   ([`carbon_dioxide_collapse_pressure`]).
/// - Any other air collapses where its bulk gas, the largest by number, is condensed at its partial
///   pressure at the night's temperature, by the registry's phase diagram (P14.T49.b's, whose
///   fourteen rows include an oxygen or an ammonia air's bulk, which the held phase columns had
///   no data for and never collapsed): Wordsworth's own criterion, a minimum temperature below the
///   condensation temperature, at the night's mean.
///   A bulk gas the surface holds a reservoir of is held at its saturation pressure already, so
///   its night's condensation is its own cycle, the climate's condensable, and not a collapse:
///   its pressure is its cold trap's, P14.T51.a's (22 → 23).
#[must_use]
pub(crate) fn collapsed(air: &NightAir<'_>, flux: WattsPerSquareMetre, mass: EarthMasses) -> bool {
    let Some(&(bulk, share)) = air.gases.first() else {
        return false;
    };
    if bulk == Gas::CarbonDioxide.substance()
        && share >= PURE_CARBON_DIOXIDE_SHARE
        && let Some(critical) = carbon_dioxide_collapse_pressure(flux, mass)
    {
        return air.pressure < critical;
    }
    if air.reservoirs.contains(&bulk) {
        return false;
    }
    let (night, pressure) = (air.night, air.pressure);
    matches!(
        phase_at(bulk, night, Pascals::new(share * pressure.value())),
        Some(Phase::Solid | Phase::Liquid)
    )
}

/// The surface pressure below which a pure carbon dioxide air collapses onto the night side of a
/// tidally locked body of mass `mass` under the stellar flux `flux`: Wordsworth's (2015, ApJ 806,
/// 180) eq. 43 ([`CARBON_DIOXIDE_COLLAPSE_FIT`]); `None` outside his fit's masses
/// ([`COLLAPSE_FIT_MASSES`]) and fluxes ([`COLLAPSE_FIT_FLUXES`]), where it does not hold.
///
/// Between his two masses log₁₀ `p_crit` is interpolated linearly in log₁₀ M (the lane's, plan
/// 14's Risks).
///
/// # Examples
///
/// An Earth under Earth's light keeps a pure CO₂ air above some 0.14 bar, and a planet of 10 M⊕
/// needs more:
///
/// ```
/// use hyperion_sim::planetary::hooks::surface::regime::carbon_dioxide_collapse_pressure;
/// use hyperion_sim::units::{EarthMasses, WattsPerSquareMetre};
///
/// let flux = WattsPerSquareMetre::new(1_366.0);
/// let earth = carbon_dioxide_collapse_pressure(flux, EarthMasses::new(1.0)).expect("in the fit");
/// assert!((earth.value() / 1.4e4 - 1.0).abs() < 0.05, "{}", earth.value());
/// let heavy = carbon_dioxide_collapse_pressure(flux, EarthMasses::new(10.0)).expect("in the fit");
/// assert!(heavy > earth);
/// assert_eq!(carbon_dioxide_collapse_pressure(flux, EarthMasses::new(0.1)), None);
/// ```
#[must_use]
pub fn carbon_dioxide_collapse_pressure(
    flux: WattsPerSquareMetre,
    mass: EarthMasses,
) -> Option<Pascals> {
    let (f, m) = (flux.value(), mass.value());
    let (lightest, heaviest) = (COLLAPSE_FIT_MASSES.0.value(), COLLAPSE_FIT_MASSES.1.value());
    let (faintest, brightest) = (COLLAPSE_FIT_FLUXES.0.value(), COLLAPSE_FIT_FLUXES.1.value());
    // By comparison, so that a NaN is outside.
    if !((lightest..=heaviest).contains(&m) && (faintest..=brightest).contains(&f)) {
        return None;
    }
    let fit = |[c1, c2, c3, c4]: [f64; 4]| ((c1 * f + c2) * f + c3) * f + c4;
    let [light, heavy] = CARBON_DIOXIDE_COLLAPSE_FIT;
    let weight = math::log10(m / lightest) / math::log10(heaviest / lightest);
    let log = fit(light) + weight * (fit(heavy) - fit(light));
    Some(Pascals::new(math::exp10(log)))
}

/// The forcing of the body whose surface conditions are `conditions`, with `inputs` (the
/// [module](self) documentation).
#[must_use]
fn forcing(conditions: &SurfaceConditions, inputs: &ClimateInputs<'_>) -> Forcing {
    if conditions.locked {
        return Forcing::Locked;
    }
    let period = conditions.spin.rotation_period;
    // A period that is not finite and positive is the slowest rotation, as the transport takes it.
    if !(period.value().is_finite() && period.value() > 0.0)
        || period >= slow_rotator_onset(conditions.stellar_flux())
    {
        return Forcing::SlowRotator;
    }
    let (equator, pole) = annual_insolation(conditions.spin.obliquity);
    let ice = inputs.ice_fraction;
    let belt = equator < pole
        && inputs.from_colder
        && ice > 0.0
        && ice < 1.0
        && conditions.zonal_temperature(1.0) >= inputs.ice_threshold;
    if belt {
        Forcing::IceBelt
    } else {
        Forcing::Seasonal
    }
}

/// The sidereal rotation period from which a body under the orbit-averaged flux `flux` is a slow
/// rotator ([`SLOW_ROTATOR_BRACKETS`]; Yang et al. 2014, ApJ 787, L2, Table 1).
///
/// It is each bracket's geometric mean, 11.31 d at 1,900 W m⁻² (1.396 S⊕) and 39.19 d at 2,615
/// W m⁻² (1.921 S⊕), linear between them in the logarithms of both, and held beyond them (the
/// lane's, plan 14's Risks). The onset rises with flux. Both holds are extrapolations: Yang et
/// al.'s Table 1 brackets the inner edge alone, and at Earth's flux their climates change
/// gradually with the rotation (their Fig. 1). So a world slower than 11.3 d is a slow rotator
/// under any fainter light, the Moon (27.3 d), Mercury, Venus and a Titan (15.9 d) among them,
/// and an Earth, a Mars and a Ganymede (7.15 d) are not. A flux that is not a number takes the
/// fainter hold.
///
/// # Examples
///
/// Whether a planet under 1.6 times Earth's light circulates as a slow rotator, with its clouds on
/// its substellar point, or as a fast one: the onset there is some 19 d, so a planet turning once
/// in 30 days is a slow rotator and one turning in 10 days is not.
///
/// ```
/// use hyperion_sim::planetary::hooks::surface::regime::slow_rotator_onset;
/// use hyperion_sim::units::{Days, EarthFluxes, Seconds, WattsPerSquareMetre};
///
/// let onset = |s: f64| Days::from(slow_rotator_onset(WattsPerSquareMetre::from(EarthFluxes::new(s)))).value();
/// let at = slow_rotator_onset(WattsPerSquareMetre::from(EarthFluxes::new(1.6)));
/// assert!(Seconds::from(Days::new(30.0)) >= at && Seconds::from(Days::new(10.0)) < at);
/// assert!((8.0..16.0).contains(&onset(1.40)) && (32.0..48.0).contains(&onset(1.92)));
/// assert!(onset(1.6) > onset(1.5) && (onset(1.0) - onset(1.3)).abs() < 1e-12);
/// ```
#[must_use]
pub fn slow_rotator_onset(flux: WattsPerSquareMetre) -> Seconds {
    let [(dim, dim_low, dim_high), (bright, bright_low, bright_high)] = SLOW_ROTATOR_BRACKETS;
    let (dim, bright) = (dim.value(), bright.value());
    let (at_dim, at_bright) = (
        math::ln(dim_low.value() * dim_high.value()) / 2.0,
        math::ln(bright_low.value() * bright_high.value()) / 2.0,
    );
    let s = flux.value();
    let log = if s.is_nan() || s <= dim {
        at_dim
    } else if s >= bright {
        at_bright
    } else {
        at_dim + (at_bright - at_dim) * math::ln(s / dim) / math::ln(bright / dim)
    };
    Seconds::from(Days::new(math::exp(log)))
}

/// The climate's condensable of an air of mole fractions `gases` at the surface pressure
/// `surface_pressure`, over the climate's range `range` (coldest then warmest), on a surface
/// holding reservoirs of `reservoirs` (P14.T48.d; signoff-3.md, item 1.12).
///
/// The loop runs over the registry's condensables in registry order ([`substance::condensables`]:
/// the fourteen rows with a phase diagram, P14.T49.b's, where the held phase columns had five). A
/// row is a candidate where the air holds it, at a partial pressure p = x `p_s`, and its phase
/// changes inside the range: its saturation temperature at p lies within it, `p_sat`(`T_cold`) ≤ p
/// ≤ `p_sat`(`T_warm`) on its diagram's curve, or the surface holds a reservoir of it. The
/// condensable is the candidate of the largest p, ties by registry order; or none.
///
/// The nine rows the held columns lacked are candidates by the same test. A climate's air of
/// P14.T13.c's nine gases ([`ClimateRegime::of`]) can hold two of them: ammonia, which T13.c never
/// writes, and oxygen, which it writes uncapped where a runaway past left some (P14.T24.f), a
/// candidate only where its curve, which ends at its critical temperature, 154.581 K, crosses the
/// range, or it is a reservoir ([`surface_reservoirs`]). The area mix's air is the partition's
/// species, any of the fourteen, with the partition's reservoirs, named on the same curves
/// (P14.T51.b).
///
/// # Examples
///
/// A Mars given its measured air by hand, 6.1 mbar of 95% carbon dioxide over a climate of 166 to
/// 240 K with its polar caps' ices, takes carbon dioxide, not water: its caps hold more water than
/// carbon dioxide, but its air holds far more carbon dioxide.
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::planetary::hooks::surface::regime::climate_condensable;
/// use hyperion_sim::units::{Kelvin, Pascals};
///
/// let (co2, water) = (Gas::CarbonDioxide.substance(), Gas::Water.substance());
/// let gases = [(co2, 0.951), (Gas::Nitrogen.substance(), 0.028), (Gas::Argon.substance(), 0.02), (water, 2.1e-4)];
/// let range = (Kelvin::new(166.0), Kelvin::new(240.0));
/// let pressure = Pascals::new(610.0);
/// assert_eq!(climate_condensable(&gases, pressure, range, &[water, co2]), Some(co2));
/// // Without its caps, only its water's vapour changes phase inside the range.
/// assert_eq!(climate_condensable(&gases, pressure, range, &[]), Some(water));
/// ```
#[must_use]
pub fn climate_condensable(
    gases: &[(SubstanceId, f64)],
    surface_pressure: Pascals,
    range: (Kelvin, Kelvin),
    reservoirs: &[SubstanceId],
) -> Option<SubstanceId> {
    let (coldest, warmest) = range;
    let mut chosen: Option<(SubstanceId, f64)> = None;
    for id in substance::condensables() {
        let Some(&(_, share)) = gases.iter().find(|&&(gas, _)| gas == id) else {
            continue;
        };
        let partial = share * surface_pressure.value();
        // By comparison, so that a NaN is no candidate.
        if partial.is_nan() || partial <= 0.0 {
            continue;
        }
        let changes = match (
            saturation_pressure(id, coldest),
            saturation_pressure(id, warmest),
        ) {
            (Some(cold), Some(warm)) => cold.value() <= partial && partial <= warm.value(),
            (None, _) | (_, None) => false,
        };
        if !(changes || reservoirs.contains(&id)) {
            continue;
        }
        if chosen.is_none_or(|(_, best)| partial > best) {
            chosen = Some((id, partial));
        }
    }
    chosen.map(|(id, _)| id)
}

/// The condensables the surface under the atmosphere `atmosphere` holds a reservoir of, in
/// registry order: those its air holds at their cap at the mean surface temperature, the rest of
/// the inventory condensed onto the surface.
///
/// - P14.T13.c holds water, nitrogen, carbon dioxide and argon at their saturation pressure where
///   their inventory would weigh more: a gas whose partial pressure is at least its saturation
///   pressure is held there, equal to the bit (T13.c's own test, as P14.T24.a's greenhouse gain
///   reads it).
/// - P14.T24.f holds methane's partial pressure in the air, x `p_s`, at
///   [`METHANE_RELATIVE_HUMIDITY`] of its saturation pressure where the surface holds a reservoir
///   of it, met to [`RESERVOIR_CAP_TOLERANCE`].
///
/// Every saturation pressure is the row's phase diagram's, as T13.c's own is (P14.T49.b's curves,
/// since P14.T54.a's switch). The loop runs over the fourteen condensables, of which seven are
/// T13.c's gases and carry a partial pressure here: with the held columns' five, ammonia, which
/// T13.c never writes, and oxygen, which it writes uncapped where a runaway past left some
/// (P14.T24.f), so that it is read as a reservoir only below its critical temperature, 154.581 K,
/// where its weight reaches its saturation pressure: a surface it would frost onto, though
/// version 22's air keeps it all aloft (the batch's partition caps it on the same curve).
///
/// A row the atmosphere does not carry holds none, until P14.T51.a's partition names its
/// reservoir (22 → 23). An atmosphere with no surface pressure, an envelope's, or over a surface
/// not above 0 K holds none.
///
/// # Examples
///
/// Earth's oceans are a reservoir of water, as [`SurfaceConditions::derive`]'s example derives it:
///
/// ```
/// # use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// # use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
/// # use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// # use hyperion_sim::stellar::Composition;
/// # use hyperion_sim::stellar::draws::UnitUniform;
/// # use hyperion_sim::time::UniverseTime;
/// # use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// # use hyperion_sim::units::{
/// #     Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres, Radians,
/// #     SolarLuminosities, SolarMasses, SolarRadii, Years,
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
/// # let derived = derive_body(&earth, &hosts, disc, Years::new(4.57e9), UniverseTime::EPOCH)?;
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::planetary::hooks::surface::regime::surface_reservoirs;
///
/// assert_eq!(surface_reservoirs(derived.atmosphere()), [Gas::Water.substance()]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn surface_reservoirs(atmosphere: &Atmosphere) -> Vec<SubstanceId> {
    let partials = atmosphere.partial_pressures();
    let surface = atmosphere.surface_temperature();
    let pressure = partials.total().value();
    // By comparison: a saturation pressure is zero at a surface that is not above 0 K, which
    // would read every gas as held at its cap.
    if pressure.is_nan() || pressure <= 0.0 || surface.value().is_nan() || surface.value() <= 0.0 {
        return Vec::new();
    }
    let gases = atmosphere_mole_fractions(partials);
    substance::condensables()
        .filter(|&id| {
            let (Some(gas), Some(saturation)) = (id.gas(), saturation_pressure(id, surface)) else {
                return false;
            };
            let weight = partials.of(gas).value();
            if weight.is_nan() || weight <= 0.0 {
                return false;
            }
            match gas {
                Gas::Methane => {
                    let share = gases
                        .iter()
                        .find(|&&(row, _)| row == id)
                        .map_or(0.0, |&(_, x)| x);
                    let cap = METHANE_RELATIVE_HUMIDITY * saturation.value();
                    share * pressure >= cap * (1.0 - RESERVOIR_CAP_TOLERANCE)
                }
                Gas::Hydrogen
                | Gas::Helium
                | Gas::Water
                | Gas::Ammonia
                | Gas::Nitrogen
                | Gas::Oxygen
                | Gas::CarbonDioxide
                | Gas::Argon => weight >= saturation.value(),
            }
        })
        .collect()
}

/// The coarse model the regime of thermal regime `thermal` and forcing `forcing` names for the
/// body whose surface conditions are `conditions` (the [module](self) documentation).
#[must_use]
fn coarse_model(
    conditions: &SurfaceConditions,
    thermal: ThermalRegime,
    forcing: Forcing,
) -> ClimateModelKind {
    model_of(
        thermal,
        forcing,
        conditions.air.meridional_transport,
        conditions.climate_range(),
    )
}

/// The coarse model of a regime of thermal regime `thermal` and forcing `forcing`, whose air's
/// meridional transport over its restoring is `meridional_transport`
/// ([`ZonalProfile::transport_over_restoring`](super::ZonalProfile::transport_over_restoring)) and
/// whose climate's range is `range`, coldest then warmest (the [module](self) documentation).
#[must_use]
pub(crate) fn model_of(
    thermal: ThermalRegime,
    forcing: Forcing,
    meridional_transport: f64,
    range: (Kelvin, Kelvin),
) -> ClimateModelKind {
    let energy_balance = match forcing {
        Forcing::Locked => ClimateModelKind::LockedEnergyBalance,
        Forcing::Seasonal | Forcing::IceBelt | Forcing::SlowRotator => {
            ClimateModelKind::EnergyBalance
        }
    };
    let (coldest, warmest) = (range.0.value(), range.1.value());
    match thermal {
        ThermalRegime::AirlessLike => ClimateModelKind::RadiativeEquilibrium,
        // By comparison, so that a NaN transport is a thick air's.
        ThermalRegime::Transitional if meridional_transport < 1.0 => {
            ClimateModelKind::RadiativeEquilibrium
        }
        // By comparison, so that a range with no coldest is no isothermal surface.
        ThermalRegime::Efficient
            if math::powi(warmest / coldest, 4) - 1.0 <= ISOTHERMAL_FLUX_SPREAD =>
        {
            ClimateModelKind::Isothermal
        }
        ThermalRegime::Transitional | ThermalRegime::Efficient => energy_balance,
    }
}
