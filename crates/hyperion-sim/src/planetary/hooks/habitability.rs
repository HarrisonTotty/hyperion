//! A body's habitability assessment (plan 14, P14.T25; the brainstorm's "Hooks for the layers
//! above"): a class of whether liquid water stands on or under its surface at a time, and the
//! limits that bear on it, from a closed list ([`HabitabilityAssessment::assess`]).
//!
//! It says nothing about life: the life generator reads it with the other hooks, and decides by
//! its own rules.
//!
//! # The class, by the first that applies
//!
//! 1. [`Temperate`](HabitabilityClass::Temperate): a rocky body ([`PlanetClass::Rocky`]) of
//!    0.1–5 M⊕ ([`TEMPERATE_MASSES`]) inside the conservative zone, with liquid water at its
//!    surface. The water is its surface's flag ([`SurfaceReading::liquid_water`]), which the record
//!    reads from its published condensates
//!    ([`SurfaceConditions::liquid_water`](crate::planetary::hooks::SurfaceConditions::liquid_water)).
//! 2. [`Marginal`](HabitabilityClass::Marginal): a solid surface under an air
//!    ([`SurfaceState::Temperate`] or [`SurfaceState::Snowball`]) inside the optimistic zone. That
//!    is a cold or dry world such as Mars, a snowball, a wet world of the optimistic zone alone,
//!    or one of another class or mass.
//! 3. [`SubsurfaceOcean`](HabitabilityClass::SubsurfaceOcean): a solid body whose global figures
//!    hold a subsurface ocean, as Europa's do.
//! 4. [`Hostile`](HabitabilityClass::Hostile): every other: an airless, molten or runaway surface,
//!    one outside the optimistic zone with no ocean beneath it, and a body with no surface.
//!
//! # The zone
//!
//! A body is inside a zone where its orbit-mean distance lies between the zone's edges, both
//! included. That distance is a (1 − e²)^¼ for its seasonal orbit of semi-major axis a and
//! eccentricity e: the circular orbit that receives the same orbit-mean flux, L ÷ (4π a²
//! √(1 − e²)). That mean is the one Kopparapu et al. (2013, ApJ 765, 131, eq. 4) apply to their
//! zone after Williams and Pollard (2002), as the derivation's flux is
//! ([`irradiation`](crate::planetary::derive::irradiation)). The seasonal orbit is the body's own
//! about its zone's host, and a moon's planet's.
//!
//! # The limits
//!
//! [`HabitabilityLimit`] is the plan's closed list, with four the plan left out (lean,
//! 2026-11-01): a molten surface, a dry one, a mass outside `Temperate`'s and a frozen surface,
//! each read from the generator's own states, so that every body short of `Temperate` names a
//! reason. A body takes every limit that applies, whatever its class, so a `Temperate` body can be
//! tidally locked about an active host. A body outside the optimistic zone is outside the
//! conservative one too, and takes both.
//!
//! # Determinism
//!
//! A pure function of its inputs, with no draw and so no domain tag. Its arithmetic is two square
//! roots, products and sums, each correctly rounded, and comparisons.

#[cfg(test)]
mod tests;

use crate::orbit::Eccentricity;
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::atmosphere::{Gas, SurfaceState, WATER_MELTING_POINT};
use crate::planetary::derive::habitable_zone::HabitableZone;
use crate::planetary::hooks::surface::RadiationClass;
use crate::units::{EarthMasses, Kelvin, Metres, Pascals, Years};

/// The masses of a `Temperate` body, inclusive: 0.1–5 M⊕, the plan's range. They are the masses
/// Kopparapu et al. (2014, ApJL 787, L29) extend their zone to, from memory and not checked here.
pub const TEMPERATE_MASSES: (EarthMasses, EarthMasses) =
    (EarthMasses::new(0.1), EarthMasses::new(5.0));

/// How soon a host's main sequence must end to limit a body, 1 Gyr (the plan's
/// [`LeavingMainSequence`](HabitabilityLimit::LeavingMainSequence)).
pub const MAIN_SEQUENCE_HORIZON: Years = Years::new(1e9);

/// The least radiation class at which a body's host is active: [`RadiationClass::Active`], 10
/// times the present Earth's X-ray and ultraviolet flux or more, as a Sun-like host under 1.0 Gyr
/// old gives at 1 au.
pub const ACTIVE_HOST: RadiationClass = RadiationClass::Active;

/// A body's habitability class (P14.T25), in the plan's order: from a surface where no liquid
/// water stands, through one with an ocean beneath it, to one with liquid water on it.
///
/// [`Temperate`](Self::Temperate) is the plan's name. It is a different type from
/// [`SurfaceState::Temperate`], the climate's state, whose name is with the owner: a body in that
/// state can be any class here (Mars is `Marginal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HabitabilityClass {
    /// No liquid water on its surface now, nor an ocean beneath it: an airless, molten or runaway
    /// surface, one outside the optimistic zone, or a body with no surface (Venus, the Moon, a gas
    /// giant).
    Hostile,
    /// A subsurface ocean under a surface that is not `Marginal` or `Temperate` (Europa).
    SubsurfaceOcean,
    /// A solid surface under an air inside the optimistic zone that is not `Temperate` (Mars).
    Marginal,
    /// A rocky body of 0.1–5 M⊕ inside the conservative zone with liquid water at its surface
    /// (Earth).
    Temperate,
}

/// A limit on a body's habitability (P14.T25): the plan's closed list, and the four after it that
/// the plan left out (lean, 2026-11-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HabitabilityLimit {
    /// Its orbit-mean distance lies outside the optimistic zone, recent Venus to early Mars, or
    /// its zone's stars have not formed.
    OutsideOptimisticZone,
    /// Its orbit-mean distance lies outside the conservative zone, the moist to the maximum
    /// greenhouse, or its zone's stars have not formed.
    OutsideConservativeZone,
    /// No air: its state is [`SurfaceState::Airless`] or [`SurfaceState::Tenuous`], below
    /// [`AIRLESS_PRESSURE`](crate::planetary::derive::atmosphere::AIRLESS_PRESSURE).
    NoAtmosphere,
    /// Its surface pressure is below water's triple point, 611.657 Pa (the substance registry's
    /// water row, IAPWS-95), where no liquid water stands at any temperature.
    BelowTriplePoint,
    /// Its state is [`SurfaceState::RunawayGreenhouse`], or a steam envelope
    /// ([`SurfaceState::SteamEnvelope`]), which the derivation groups with it.
    RunawayGreenhouse,
    /// Its state is [`SurfaceState::Snowball`].
    Snowball,
    /// It has no surface section: a hydrogen and helium envelope, or a supercritical one that hides
    /// a solid surface.
    GasEnvelope,
    /// It is locked 1:1 to its seasonal host, which stands still in its sky. A moon is never: its
    /// lock is to its planet, under which it turns.
    TidallyLocked,
    /// Its orbit-mean distance lies inside the optimistic zone, but its pericentre or apocentre
    /// does not: it leaves the zone for part of each orbit.
    HighEccentricity,
    /// Its hosts' X-ray and ultraviolet flux at it is [`ACTIVE_HOST`]'s class or above.
    ActiveHost,
    /// One of its zone's stars is a protostar or still contracting to its main sequence, its light
    /// still falling.
    YoungHost,
    /// One of its zone's stars leaves its main sequence within [`MAIN_SEQUENCE_HORIZON`], or has
    /// left it and still burns, so that its zone moves outward.
    LeavingMainSequence,
    /// A star of its zone lies outside Kopparapu et al.'s fit, 2,600–7,200 K
    /// ([`HabitableZone::extrapolated`]).
    ExtrapolatedZone,
    /// Its state is [`SurfaceState::MagmaOcean`]: a molten crust, or a mean surface temperature at
    /// or above the silicate solidus, 1,394 K
    /// ([`SILICATE_SOLIDUS`](crate::planetary::derive::atmosphere::SILICATE_SOLIDUS), Hirschmann
    /// 2000, G³ 1, 1042). Lean, 2026-11-01.
    MoltenSurface,
    /// Its surface holds no water, liquid or ice (its record's published condensates hold no
    /// `H2O` entry), or its water follows a past runaway greenhouse
    /// ([`Atmosphere::water_state`](crate::planetary::derive::atmosphere::Atmosphere::water_state)),
    /// its oceans lost, under a present state that is not one. Lean, 2026-11-01.
    DrySurface,
    /// Its mass lies outside [`TEMPERATE_MASSES`], 0.1–5 M⊕, which `Temperate` requires. Lean,
    /// 2026-11-01.
    MassOutsideTemperateRange,
    /// No liquid water stands at its surface at a mean surface temperature under water's melting
    /// point, 273.15 K ([`WATER_MELTING_POINT`]), in a state short of
    /// [`Snowball`](SurfaceState::Snowball)'s ice over half its surface: a cold desert, such as
    /// Mars. Lean, 2026-11-01.
    FrozenSurface,
}

impl HabitabilityLimit {
    /// Every limit, in the plan's order and then the four added after it.
    pub const ALL: [Self; 17] = [
        Self::OutsideOptimisticZone,
        Self::OutsideConservativeZone,
        Self::NoAtmosphere,
        Self::BelowTriplePoint,
        Self::RunawayGreenhouse,
        Self::Snowball,
        Self::GasEnvelope,
        Self::TidallyLocked,
        Self::HighEccentricity,
        Self::ActiveHost,
        Self::YoungHost,
        Self::LeavingMainSequence,
        Self::ExtrapolatedZone,
        Self::MoltenSurface,
        Self::DrySurface,
        Self::MassOutsideTemperateRange,
        Self::FrozenSurface,
    ];

    /// The limit's bit in a [`HabitabilityLimits`].
    #[must_use]
    const fn bit(self) -> u32 {
        1 << match self {
            Self::OutsideOptimisticZone => 0,
            Self::OutsideConservativeZone => 1,
            Self::NoAtmosphere => 2,
            Self::BelowTriplePoint => 3,
            Self::RunawayGreenhouse => 4,
            Self::Snowball => 5,
            Self::GasEnvelope => 6,
            Self::TidallyLocked => 7,
            Self::HighEccentricity => 8,
            Self::ActiveHost => 9,
            Self::YoungHost => 10,
            Self::LeavingMainSequence => 11,
            Self::ExtrapolatedZone => 12,
            Self::MoltenSurface => 13,
            Self::DrySurface => 14,
            Self::MassOutsideTemperateRange => 15,
            Self::FrozenSurface => 16,
        }
    }
}

/// A set of [`HabitabilityLimit`]s, one bit each.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::hooks::habitability::{HabitabilityLimit, HabitabilityLimits};
///
/// let venus = HabitabilityLimits::of(HabitabilityLimit::RunawayGreenhouse)
///     .with(HabitabilityLimit::OutsideOptimisticZone);
/// assert!(venus.contains(HabitabilityLimit::RunawayGreenhouse));
/// assert!(!venus.contains(HabitabilityLimit::TidallyLocked));
/// assert_eq!(
///     venus.iter().collect::<Vec<_>>(),
///     [HabitabilityLimit::OutsideOptimisticZone, HabitabilityLimit::RunawayGreenhouse]
/// );
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HabitabilityLimits(u32);

impl HabitabilityLimits {
    /// No limit.
    pub const EMPTY: Self = Self(0);

    /// The set of `limit` alone.
    #[must_use]
    pub const fn of(limit: HabitabilityLimit) -> Self {
        Self(limit.bit())
    }

    /// This set with `limit` added.
    #[must_use]
    pub const fn with(self, limit: HabitabilityLimit) -> Self {
        Self(self.0 | limit.bit())
    }

    /// Whether `limit` is in the set.
    #[must_use]
    pub const fn contains(self, limit: HabitabilityLimit) -> bool {
        self.0 & limit.bit() != 0
    }

    /// Whether the set holds no limit.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The limits in the set, in [`HabitabilityLimit::ALL`]'s order.
    pub fn iter(self) -> impl Iterator<Item = HabitabilityLimit> {
        HabitabilityLimit::ALL
            .into_iter()
            .filter(move |&limit| self.contains(limit))
    }
}

impl FromIterator<HabitabilityLimit> for HabitabilityLimits {
    fn from_iter<I: IntoIterator<Item = HabitabilityLimit>>(limits: I) -> Self {
        limits.into_iter().fold(Self::EMPTY, Self::with)
    }
}

/// What a body's surface gives its assessment: its surface section's conditions and figures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceReading {
    /// The surface pressure of its modelled air, zero for an airless body.
    pub pressure: Pascals,
    /// Whether liquid water stands at its surface: its conditions' flag, which the record reads
    /// from its published condensates.
    pub liquid_water: bool,
    /// Whether its global figures hold a subsurface ocean
    /// ([`Subsurface::Ocean`](crate::planetary::hooks::Subsurface::Ocean)).
    pub subsurface_ocean: bool,
    /// Its mean surface temperature, its conditions'.
    pub mean_temperature: Kelvin,
    /// Whether its surface holds water, liquid or ice: its record's published condensates' `H2O`
    /// entries.
    pub holds_water: bool,
    /// The state its water follows
    /// ([`Atmosphere::water_state`](crate::planetary::derive::atmosphere::Atmosphere::water_state)):
    /// [`SurfaceState::RunawayGreenhouse`] for one whose oceans a past runaway took, whatever its
    /// present state.
    pub water_state: SurfaceState,
}

/// What a body's assessment reads of the stars its zone holds at the time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostClock {
    /// Whether one of them is a protostar or still contracting to its main sequence.
    pub before_main_sequence: bool,
    /// The least time any of them has left on its main sequence: zero for one that has left it and
    /// still burns, and `None` where none will leave it (a brown dwarf, a remnant, a star whose
    /// main sequence the track does not hold).
    pub main_sequence_left: Option<Years>,
}

/// What a body's habitability assessment reads at a time (P14.T25), as plain data:
/// [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at) gathers it from the
/// body's record, its seasonal orbit, its zone and its zone's stars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HabitabilityInputs {
    /// The body's class, its bulk section's.
    pub class: PlanetClass,
    /// Its mass.
    pub mass: EarthMasses,
    /// Its state: its surface section's where it has one, and otherwise its derivation's published
    /// state ([`SurfaceState::GasEnvelope`] for a gas envelope).
    pub state: SurfaceState,
    /// Its surface, `None` where it has no surface section (a gas envelope, or a supercritical
    /// envelope that hides a solid surface).
    pub surface: Option<SurfaceReading>,
    /// The habitable zone of its zone's stars at the time, with their companions' light; `None`
    /// before the stars form.
    pub zone: Option<HabitableZone>,
    /// Its seasonal orbit's semi-major axis, positive: its own about its zone's host, its planet's
    /// for a moon.
    pub semi_major_axis: Metres,
    /// That orbit's eccentricity.
    pub eccentricity: Eccentricity,
    /// Whether it is locked 1:1 to its seasonal host (its rotation's
    /// [`SpinState::Locked`](crate::planetary::derive::rotation::SpinState::Locked) in the
    /// synchronous resonance, about its own orbit): never for a moon.
    pub locked: bool,
    /// The class of its hosts' X-ray and ultraviolet flux at it.
    pub radiation: RadiationClass,
    /// What it reads of its zone's stars.
    pub host: HostClock,
}

/// A body's habitability assessment at a time (P14.T25): its class and its limits (the
/// [module](self) documentation). It says nothing about life.
///
/// # Examples
///
/// Earth about the Sun:
///
/// ```
/// use hyperion_sim::orbit::Eccentricity;
/// use hyperion_sim::planetary::derive::atmosphere::SurfaceState;
/// use hyperion_sim::planetary::derive::{PlanetClass, habitable_zone};
/// use hyperion_sim::planetary::hooks::habitability::{
///     HabitabilityAssessment, HabitabilityClass, HabitabilityInputs, HostClock, SurfaceReading,
/// };
/// use hyperion_sim::planetary::hooks::surface::RadiationClass;
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{EarthMasses, Kelvin, Metres, Pascals, SolarLuminosities, Years};
///
/// let earth = HabitabilityAssessment::assess(&HabitabilityInputs {
///     class: PlanetClass::Rocky,
///     mass: EarthMasses::new(1.0),
///     state: SurfaceState::Temperate,
///     surface: Some(SurfaceReading {
///         pressure: Pascals::new(101_325.0),
///         liquid_water: true,
///         subsurface_ocean: false,
///         mean_temperature: Kelvin::new(288.0),
///         holds_water: true,
///         water_state: SurfaceState::Temperate,
///     }),
///     zone: Some(habitable_zone(SolarLuminosities::new(1.0), Kelvin::new(5_772.0))),
///     semi_major_axis: Metres::new(METRES_PER_AU),
///     eccentricity: Eccentricity::new(0.0167)?,
///     locked: false,
///     radiation: RadiationClass::Quiet,
///     host: HostClock {
///         before_main_sequence: false,
///         main_sequence_left: Some(Years::new(5e9)),
///     },
/// });
/// assert_eq!(earth.class(), HabitabilityClass::Temperate);
/// assert!(earth.limits().is_empty());
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HabitabilityAssessment {
    class: HabitabilityClass,
    limits: HabitabilityLimits,
}

impl HabitabilityAssessment {
    /// The assessment of the body `inputs` describe (the [module](self) documentation).
    #[must_use]
    pub fn assess(inputs: &HabitabilityInputs) -> Self {
        let orbit = SeasonalOrbit::of(inputs.semi_major_axis, inputs.eccentricity);
        let (optimistic, conservative) = inputs.zone.map_or((false, false), |zone| {
            (
                orbit.mean_inside(zone.optimistic()),
                orbit.mean_inside(zone.conservative()),
            )
        });
        let class = match inputs.surface {
            Some(surface)
                if conservative
                    && surface.liquid_water
                    && inputs.class == PlanetClass::Rocky
                    && temperate_mass(inputs.mass) =>
            {
                HabitabilityClass::Temperate
            }
            Some(_) if optimistic && under_air(inputs.state) => HabitabilityClass::Marginal,
            Some(surface) if surface.subsurface_ocean => HabitabilityClass::SubsurfaceOcean,
            Some(_) | None => HabitabilityClass::Hostile,
        };
        let mut limits = HabitabilityLimits::EMPTY;
        let mut add = |limit, applies: bool| {
            if applies {
                limits = limits.with(limit);
            }
        };
        add(HabitabilityLimit::OutsideOptimisticZone, !optimistic);
        add(HabitabilityLimit::OutsideConservativeZone, !conservative);
        add(
            HabitabilityLimit::NoAtmosphere,
            matches!(inputs.state, SurfaceState::Airless | SurfaceState::Tenuous),
        );
        add(
            HabitabilityLimit::BelowTriplePoint,
            inputs
                .surface
                .is_some_and(|surface| surface.pressure.value() < water_triple_pressure().value()),
        );
        add(HabitabilityLimit::RunawayGreenhouse, runaway(inputs.state));
        add(
            HabitabilityLimit::Snowball,
            inputs.state == SurfaceState::Snowball,
        );
        add(HabitabilityLimit::GasEnvelope, inputs.surface.is_none());
        add(HabitabilityLimit::TidallyLocked, inputs.locked);
        add(
            HabitabilityLimit::HighEccentricity,
            optimistic
                && inputs
                    .zone
                    .is_some_and(|zone| !orbit.ends_inside(zone.optimistic())),
        );
        add(
            HabitabilityLimit::ActiveHost,
            inputs.radiation >= ACTIVE_HOST,
        );
        add(
            HabitabilityLimit::YoungHost,
            inputs.host.before_main_sequence,
        );
        add(
            HabitabilityLimit::LeavingMainSequence,
            inputs
                .host
                .main_sequence_left
                .is_some_and(|left| left.value() < MAIN_SEQUENCE_HORIZON.value()),
        );
        add(
            HabitabilityLimit::ExtrapolatedZone,
            inputs.zone.is_some_and(|zone| zone.extrapolated()),
        );
        add(
            HabitabilityLimit::MoltenSurface,
            inputs.state == SurfaceState::MagmaOcean,
        );
        add(
            HabitabilityLimit::DrySurface,
            inputs.surface.is_some_and(|surface| {
                let lost = surface.water_state == SurfaceState::RunawayGreenhouse
                    && !runaway(inputs.state);
                !surface.holds_water || lost
            }),
        );
        add(
            HabitabilityLimit::MassOutsideTemperateRange,
            !temperate_mass(inputs.mass),
        );
        add(
            HabitabilityLimit::FrozenSurface,
            inputs.surface.is_some_and(|surface| {
                !surface.liquid_water
                    && surface.mean_temperature.value() < WATER_MELTING_POINT.value()
                    && inputs.state != SurfaceState::Snowball
            }),
        );
        Self { class, limits }
    }

    /// The class.
    #[must_use]
    pub const fn class(&self) -> HabitabilityClass {
        self.class
    }

    /// The limits.
    #[must_use]
    pub const fn limits(&self) -> HabitabilityLimits {
        self.limits
    }
}

/// A seasonal orbit's three distances from its host, m: its orbit-mean distance a (1 − e²)^¼, its
/// pericentre and its apocentre.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SeasonalOrbit {
    mean: f64,
    pericentre: f64,
    apocentre: f64,
}

impl SeasonalOrbit {
    /// The distances of an orbit of semi-major axis `a` and eccentricity `e`.
    #[must_use]
    fn of(a: Metres, e: Eccentricity) -> Self {
        let (a, e) = (a.value(), e.value());
        Self {
            mean: a * (1.0 - e * e).sqrt().sqrt(),
            pericentre: a * (1.0 - e),
            apocentre: a * (1.0 + e),
        }
    }

    /// Whether its orbit-mean distance lies inside `(inner, outer)`, both included.
    #[must_use]
    fn mean_inside(&self, (inner, outer): (Metres, Metres)) -> bool {
        inside(self.mean, (inner, outer))
    }

    /// Whether its pericentre and its apocentre both lie inside `(inner, outer)`.
    #[must_use]
    fn ends_inside(&self, (inner, outer): (Metres, Metres)) -> bool {
        inside(self.pericentre, (inner, outer)) && inside(self.apocentre, (inner, outer))
    }
}

/// Whether the distance `d`, m, lies inside `(inner, outer)`, both included: never for a NaN.
#[must_use]
fn inside(d: f64, (inner, outer): (Metres, Metres)) -> bool {
    inner.value() <= d && d <= outer.value()
}

/// Whether `mass` lies in [`TEMPERATE_MASSES`], both included.
#[must_use]
fn temperate_mass(mass: EarthMasses) -> bool {
    let (lo, hi) = TEMPERATE_MASSES;
    lo.value() <= mass.value() && mass.value() <= hi.value()
}

/// Whether `state` is a runaway greenhouse's: [`SurfaceState::RunawayGreenhouse`], or a steam
/// envelope, which the derivation groups with it.
#[must_use]
const fn runaway(state: SurfaceState) -> bool {
    matches!(
        state,
        SurfaceState::RunawayGreenhouse | SurfaceState::SteamEnvelope
    )
}

/// Whether a solid surface in `state` lies under an air that is neither molten nor a runaway's:
/// what a `Marginal` body needs.
#[must_use]
const fn under_air(state: SurfaceState) -> bool {
    match state {
        SurfaceState::Temperate | SurfaceState::Snowball => true,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Tenuous
        | SurfaceState::SteamEnvelope => false,
    }
}

/// Water's triple-point pressure, the substance registry's water row's (611.657 Pa, IAPWS-95 as
/// Wagner and Pruss 2002, J. Phys. Chem. Ref. Data 31, 387, give it).
///
/// # Panics
///
/// Never: the registry's water row carries its phase diagram, which a registry test holds.
#[must_use]
fn water_triple_pressure() -> Pascals {
    Gas::Water
        .substance()
        .substance()
        .phase_diagram()
        .expect("the registry's water row carries its phase diagram")
        .triple_pressure()
}
