//! Derivation: everything about a body that is computed from its mass, orbit and host rather than
//! drawn (plan 14, phase C; the brainstorm's "Everything else is computed, not rolled").
//!
//! The pieces, each a pure function of plain quantities that can be tested on Solar System values
//! without a generator:
//!
//! - [`radius`]: Chen and Kipping's mass–radius relation, through which a body's one drawn
//!   quantile sets its radius, and Zeng et al.'s curves of iron, rock and water (P14.T11.a–b).
//! - [`envelope`]: the radius of a core with a hydrogen and helium envelope, from Lopez and
//!   Fortney's and Fortney, Marley and Barnes's models (P14.T11.b).
//! - [`composition`](mod@composition): the solve that turns a (mass, radius) point into iron, rock, water and
//!   envelope, constrained by the side of the snow line the body formed on (P14.T11.c; design note
//!   8).
//! - [`irradiation`]: the flux a body receives from its hosts and its equilibrium temperature
//!   (P14.T12.a).
//! - [`habitable_zone`](mod@habitable_zone): Kopparapu et al.'s habitable zone of a host or of a
//!   multiple system (P14.T12.b).
//! - [`m_dwarfs`]: the rocky branch of M dwarfs' inner planets (ruling 102.1).
//! - [`limits`]: Roche limits, Hill radii, the stability limit of satellites and the heaviest moon
//!   a close-in planet can keep (P14.T15).
//!
//! and their assembly, [`derive_body`] (P14.T16.a), the one entry point through which the
//! generator derives a body.
//!
//! # The assembly, in its order
//!
//! [`derive_body`] runs, in an order it keeps as the deferred tasks join it (the vertical slice,
//! ruling 33):
//!
//! 1. **Radius and composition** (T11, fixed at formation). The body's drawn rank is confined to
//!    the radii its composition can hold ([`radius_rank_in_window`], ruling 47). Where it falls
//!    between the iron curve and the rock curve (the Earth-like curve beyond the snow line, ruling
//!    58) the outcome is rocky: its composition is the same quantile of the observed spread of
//!    rocky planets' ([`rocky_core_mass_fraction`], ruling 53) and its radius Zeng et al.'s at it.
//!    Above that curve the rank places the body within Chen and Kipping's scatter at its mass, and
//!    that radius is solved into a composition on the side of the disc's snow line where it
//!    formed, at the flux of its host's zero-age luminosity. About a host under 0.6 M☉ (blended
//!    away by 0.70 M☉) a body formed inside the snow line is rocky unless it draws an envelope,
//!    with a probability rising with its mass ([`m_dwarfs`], ruling 102.1); its rank then splits
//!    the window at that probability instead of at the rock curve's own rank.
//! 2. **Irradiation** (T12): the flux from its hosts at the time and the equilibrium temperature,
//!    with a Bond albedo of 0.3 until T13's atmospheres close the loop (T13 will iterate here).
//! 3. **The radius at the time** (T11): an envelope's radius at the body's age and present flux
//!    (T13.b's escape will strip it here), and from it density, surface gravity and class.
//! 4. Rotation and tides (T14) will follow here.
//! 5. **Limits** (T15): the Hill radius, the stability limits of satellites and the heaviest moon
//!    that survives to the time.
//!
//! Giants from 0.3 Jupiter masses take their radius from plan 13's `giant_cooling` at the body's
//! age, inflated for its flux ([`radius_giant`]), their composition from their heavy elements
//! ([`giant_composition`]), and an internal luminosity that warms them (P14.T11.d, ruling 56);
//! from 0.3 to 0.414 Jupiter masses both blend into the envelope model's by [`giant_share`].
//!
//! These read, from the stages above, plan 06's [`UnitUniform`] for the radius quantile and
//! [`math::normal_quantile`](crate::math::normal_quantile) to apply it, and a host's luminosity,
//! effective temperature and radius as plain values from its
//! [`StarState`](crate::stellar::StarState) (ruling 34); `units` gains `EarthRadii`,
//! `JupiterRadii`, `WattsPerSquareMetre`, `EarthFluxes` and `MetresPerSecondSquared`, and
//! `units::consts` the Earth and Jupiter radii, σ and the solar constant.

pub mod composition;
pub mod envelope;
pub mod habitable_zone;
pub mod irradiation;
pub mod limits;
pub mod m_dwarfs;
pub mod radius;
pub mod rocky;

use std::error::Error;
use std::fmt;

pub use composition::{
    GiantComposition, MassFractions, RadiusWindow, SnowLineSide, SolvedComposition, composition,
    giant_composition, giant_heavy_elements, radius_window,
};
pub use habitable_zone::{HabitableZone, habitable_zone, habitable_zone_of};
pub use irradiation::{
    BondAlbedo, HostLight, Illumination, equilibrium_temperature, total_flux, with_internal_heat,
};
pub use limits::{
    OrbitSense, hill_radius, maximum_surviving_moon_mass, roche_limit_fluid, roche_limit_rigid,
    satellite_stability_limit,
};
pub use radius::{
    CoreComposition, DeriveGiantError, GiantRadius, chen_kipping_rank, giant_share,
    radius_chen_kipping, radius_giant, radius_zeng,
};

use crate::orbit::KeplerElements;
use crate::planetary::derive::composition::{SolveCompositionError, dry_composition};
use crate::planetary::derive::envelope::radius_with_envelope;
use crate::planetary::derive::irradiation::luminosity_flux;
use crate::planetary::derive::limits::TidalPlanet;
use crate::planetary::disc::DiscProfile;
use crate::planetary::params::{
    GAS_GIANT_ENVELOPE_FRACTION, GIANT_LOVE_NUMBER, GIANT_TIDAL_Q, ICE_GIANT_MASS,
    ICY_WATER_FRACTION, ROCKY_LOVE_NUMBER, ROCKY_TIDAL_Q, THIN_ENVELOPE_FRACTION,
};
use crate::stellar::Composition;
use crate::stellar::draws::UnitUniform;
use crate::stellar::substellar::{EvaluateGiantCoolingError, giant_cooling};
use crate::time::UniverseTime;
use crate::units::consts::GM_EARTH;
use crate::units::{
    EarthFluxes, EarthMasses, EarthRadii, Gigayears, JupiterMasses, Kelvin, Kilograms,
    KilogramsPerCubicMetre, Metres, MetresPerSecondSquared, Seconds, SolarMasses, Watts, Years,
};

/// A body as the derivation reads it: its mass and orbit, where it formed, and its one drawn
/// number (design note 8).
///
/// Placement (P14.T8's `PlacedPlanet`, not built) fixes the mass, the primordial orbit and the
/// formation distance, which are all that the part of the derivation fixed at formation reads.
/// The fate transform (T28, not built) turns the primordial orbit into the orbit at a time, which
/// [`with_orbit_now`](Self::with_orbit_now) sets and the rest of [`derive_body`] reads; until
/// then the two are the same. The rank is drawn on the body's own `planet.radius` stream by the
/// generator (T30), which registers that tag when it first opens it; here it is a plain argument,
/// so that the derivation can be tested with any rank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedBody {
    mass: EarthMasses,
    orbit: KeplerElements,
    orbit_now: KeplerElements,
    formation_distance: Metres,
    radius_rank: UnitUniform,
}

impl PlacedBody {
    /// A body of mass `mass` on the primordial orbit `orbit` about its primary, formed at
    /// `formation_distance` from its host (its semi-major axis, unless it migrated), with radius
    /// rank `radius_rank`; its orbit at the time is `orbit` until
    /// [`with_orbit_now`](Self::with_orbit_now) says otherwise.
    ///
    /// # Errors
    ///
    /// [`BuildPlacedBodyError::MassNotPositive`] or
    /// [`BuildPlacedBodyError::FormationDistanceNotPositive`] for a value that is not positive and
    /// finite.
    pub fn new(
        mass: EarthMasses,
        orbit: KeplerElements,
        formation_distance: Metres,
        radius_rank: UnitUniform,
    ) -> Result<Self, BuildPlacedBodyError> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(mass.value()) {
            return Err(BuildPlacedBodyError::MassNotPositive);
        }
        if !positive(formation_distance.value()) {
            return Err(BuildPlacedBodyError::FormationDistanceNotPositive);
        }
        Ok(Self {
            mass,
            orbit,
            orbit_now: orbit,
            formation_distance,
            radius_rank,
        })
    }

    /// The same body on `orbit_now` at the time derived: the fate transform's elements (P14.T28),
    /// such as an orbit widened by its host's mass loss.
    #[must_use]
    pub const fn with_orbit_now(self, orbit_now: KeplerElements) -> Self {
        Self { orbit_now, ..self }
    }

    /// The body's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The body's primordial orbit about its primary, which what is fixed at formation reads.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// The body's orbit about its primary at the time derived.
    #[must_use]
    pub const fn orbit_now(&self) -> &KeplerElements {
        &self.orbit_now
    }

    /// How far from its host the body formed, which the disc's snow line judges.
    #[must_use]
    pub const fn formation_distance(&self) -> Metres {
        self.formation_distance
    }

    /// The body's drawn radius rank, before [`radius_rank_in_window`] confines it.
    #[must_use]
    pub const fn radius_rank(&self) -> UnitUniform {
        self.radius_rank
    }
}

/// A [`PlacedBody`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildPlacedBodyError {
    /// The mass was not positive and finite.
    MassNotPositive,
    /// The formation distance was not positive and finite.
    FormationDistanceNotPositive,
}

impl fmt::Display for BuildPlacedBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MassNotPositive => "a body's mass must be positive and finite",
            Self::FormationDistanceNotPositive => {
                "a body's formation distance must be positive and finite"
            }
        })
    }
}

impl Error for BuildPlacedBodyError {}

/// A body's hosts as the derivation reads them at the time: the mass it orbits, and the light of
/// every star (ruling 34: each host as plain values of L, `T_eff` and R from its `StarState`).
///
/// The `orbited` hosts are the ones the body's orbit goes round, seen at the body's own distance:
/// one star, or both of a pair for a circumbinary body. Each of the `companions` is another star of
/// the system, seen at the orbit that separates it from the body's host (design note 10; P14.T12.a).
/// The system's composition is the one a giant's cooling reads (plan 13's `giant_cooling`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyHosts<'a> {
    primary_mass: Kilograms,
    composition: Composition,
    orbited: &'a [HostLight],
    companions: &'a [Illumination],
}

impl<'a> BodyHosts<'a> {
    /// Hosts of which the body orbits `primary_mass` (a star's mass at the time, or a pair's), in a
    /// system of composition `composition` (plan 06's, as drawn for the system), lit by `orbited`
    /// at its own distance and by `companions` at theirs.
    ///
    /// # Errors
    ///
    /// [`BuildBodyHostsError::PrimaryMassNotPositive`] for a mass that is not positive and finite,
    /// and [`BuildBodyHostsError::NoOrbitedHost`] for an empty `orbited`. A dark host, a black hole,
    /// is a [`HostLight`] of zero luminosity.
    pub fn new(
        primary_mass: Kilograms,
        composition: Composition,
        orbited: &'a [HostLight],
        companions: &'a [Illumination],
    ) -> Result<Self, BuildBodyHostsError> {
        if !(primary_mass.value().is_finite() && primary_mass.value() > 0.0) {
            return Err(BuildBodyHostsError::PrimaryMassNotPositive);
        }
        if orbited.is_empty() {
            return Err(BuildBodyHostsError::NoOrbitedHost);
        }
        Ok(Self {
            primary_mass,
            composition,
            orbited,
            companions,
        })
    }

    /// The mass the body orbits.
    #[must_use]
    pub const fn primary_mass(&self) -> Kilograms {
        self.primary_mass
    }

    /// The system's composition.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The hosts the body orbits.
    #[must_use]
    pub const fn orbited(&self) -> &'a [HostLight] {
        self.orbited
    }

    /// The system's other stars, each at its own orbit.
    #[must_use]
    pub const fn companions(&self) -> &'a [Illumination] {
        self.companions
    }
}

/// [`BodyHosts`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildBodyHostsError {
    /// The primary's mass was not positive and finite.
    PrimaryMassNotPositive,
    /// No orbited host was given.
    NoOrbitedHost,
}

impl fmt::Display for BuildBodyHostsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PrimaryMassNotPositive => "a body's primary mass must be positive and finite",
            Self::NoOrbitedHost => "a body must orbit at least one host",
        })
    }
}

impl Error for BuildBodyHostsError {}

/// What a body is, by its bulk composition: the class a body record shows at the `Bulk` detail
/// level (design note 16).
///
/// Classed from the solved mass fractions and the mass, by [`PlanetClass::of`]. The thresholds are
/// this plan's, in [`params`](crate::planetary::params). A gas giant has no surface (ruling 34), an
/// ice giant none either, and a sub-Neptune's is the floor of its envelope (P14.T13.c's "gas
/// envelope" state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlanetClass {
    /// Iron and rock, with no more than a trace of water or hydrogen: Mercury to Earth, and dry
    /// super-Earths.
    Rocky,
    /// Rock and iron under [`ICY_WATER_FRACTION`] or more of water, with no more than a trace of
    /// hydrogen: water worlds, and icy moons such as Ganymede.
    Icy,
    /// A core under a hydrogen and helium envelope of at least [`THIN_ENVELOPE_FRACTION`], lighter
    /// than [`ICE_GIANT_MASS`].
    SubNeptune,
    /// A core of [`ICE_GIANT_MASS`] or more under an envelope of less than
    /// [`GAS_GIANT_ENVELOPE_FRACTION`] of its mass: Uranus and Neptune.
    IceGiant,
    /// A body mostly of hydrogen and helium: Saturn, and every giant of P14.T11.d's range.
    GasGiant,
}

impl PlanetClass {
    /// The class of a body of mass `mass` and mass fractions `fractions`.
    #[must_use]
    pub fn of(mass: EarthMasses, fractions: &MassFractions) -> Self {
        let envelope = fractions.envelope();
        if envelope >= GAS_GIANT_ENVELOPE_FRACTION {
            Self::GasGiant
        } else if envelope >= THIN_ENVELOPE_FRACTION {
            if mass.value() >= ICE_GIANT_MASS.value() {
                Self::IceGiant
            } else {
                Self::SubNeptune
            }
        } else if fractions.water() >= ICY_WATER_FRACTION {
            Self::Icy
        } else {
            Self::Rocky
        }
    }

    /// Whether a body of this class has a surface section: every class but the giants, whose
    /// section is not applicable (ruling 34).
    #[must_use]
    pub const fn has_surface(self) -> bool {
        match self {
            Self::Rocky | Self::Icy | Self::SubNeptune => true,
            Self::IceGiant | Self::GasGiant => false,
        }
    }

    /// The Love number k₂ and tidal quality factor Q of the class: a rocky body's for every class
    /// with a surface, a giant's for the giants (P14.T14.b).
    #[must_use]
    const fn tides(self) -> (f64, f64) {
        if self.has_surface() {
            (ROCKY_LOVE_NUMBER, ROCKY_TIDAL_Q)
        } else {
            (GIANT_LOVE_NUMBER, GIANT_TIDAL_Q)
        }
    }
}

/// Everything [`derive_body`] computes of a body at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DerivedBody {
    mass: EarthMasses,
    radius_rank: UnitUniform,
    formed: SnowLineSide,
    composition: Option<SolvedComposition>,
    fractions: MassFractions,
    core: CoreComposition,
    flux: EarthFluxes,
    albedo: BondAlbedo,
    equilibrium_temperature: Kelvin,
    internal_luminosity: Watts,
    radius: EarthRadii,
    density: KilogramsPerCubicMetre,
    surface_gravity: MetresPerSecondSquared,
    class: PlanetClass,
    hill_radius: Metres,
    prograde_limit: Metres,
    retrograde_limit: Metres,
    maximum_moon_mass: EarthMasses,
}

impl DerivedBody {
    /// The body's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The body's drawn rank, confined to its [`RadiusWindow`] by [`radius_rank_in_window`].
    ///
    /// It set the composition of a rocky outcome ([`rocky_core_mass_fraction`]) and otherwise
    /// the body's place within Chen and Kipping's scatter. From 0.414 Jupiter masses, where no
    /// window exists, it is the drawn rank, which a giant's radius does not read yet (ruling 56,
    /// item 4).
    #[must_use]
    pub const fn radius_rank(&self) -> UnitUniform {
        self.radius_rank
    }

    /// Which side of the disc's snow line the body formed on.
    #[must_use]
    pub const fn formed(&self) -> SnowLineSide {
        self.formed
    }

    /// The composition solved at formation, below 0.414 Jupiter masses: its mass fractions, core,
    /// primordial envelope and radius at
    /// [`COMPOSITION_REFERENCE_AGE`](crate::planetary::params::COMPOSITION_REFERENCE_AGE).
    ///
    /// `None` for a giant from 0.414 Jupiter masses, whose composition is
    /// [`giant_composition`]'s alone; from 0.3, [`fractions`](Self::fractions) blend the two.
    #[must_use]
    pub const fn composition(&self) -> Option<&SolvedComposition> {
        self.composition.as_ref()
    }

    /// The body's mass fractions of iron, rock, water and envelope.
    #[must_use]
    pub const fn fractions(&self) -> MassFractions {
        self.fractions
    }

    /// The composition of everything but the envelope: the solve's core, or a giant's heavy
    /// elements from 0.414 Jupiter masses.
    #[must_use]
    pub const fn core(&self) -> CoreComposition {
        self.core
    }

    /// The luminosity the body radiates of its own: a giant's ([`GiantRadius::internal_luminosity`])
    /// from 0.3 Jupiter masses, and zero below, where nothing models one yet.
    #[must_use]
    pub const fn internal_luminosity(&self) -> Watts {
        self.internal_luminosity
    }

    /// The flux the body receives from all its hosts at the time, averaged over its orbit.
    #[must_use]
    pub const fn flux(&self) -> EarthFluxes {
        self.flux
    }

    /// The Bond albedo the equilibrium temperature was taken at: 0.3 until P14.T13
    /// ([`BondAlbedo::BEFORE_ATMOSPHERES`]).
    #[must_use]
    pub const fn albedo(&self) -> BondAlbedo {
        self.albedo
    }

    /// The equilibrium temperature at the time, with the body's internal luminosity added for a
    /// giant, T⁴ = `T_eq`⁴ + `L_int` ÷ (4πR²σ) (P14.T12.a, [`with_internal_heat`]).
    #[must_use]
    pub const fn equilibrium_temperature(&self) -> Kelvin {
        self.equilibrium_temperature
    }

    /// The radius at the time: the solved radius for a body without an envelope, for one with an
    /// envelope its radius at the body's age and present flux, and for a giant
    /// [`radius_giant`]'s, blended into the envelope model's from 0.3 to 0.414 Jupiter masses.
    #[must_use]
    pub const fn radius(&self) -> EarthRadii {
        self.radius
    }

    /// The mean density, M ÷ (4π R³ ÷ 3).
    #[must_use]
    pub const fn density(&self) -> KilogramsPerCubicMetre {
        self.density
    }

    /// The gravity at the radius, G M ÷ R².
    #[must_use]
    pub const fn surface_gravity(&self) -> MetresPerSecondSquared {
        self.surface_gravity
    }

    /// The body's class by composition.
    #[must_use]
    pub const fn class(&self) -> PlanetClass {
        self.class
    }

    /// The circular Hill radius about the primary, a (m ÷ 3M)^⅓, which the satellite limits are
    /// measured in.
    #[must_use]
    pub const fn hill_radius(&self) -> Metres {
        self.hill_radius
    }

    /// The largest semi-major axis at which a satellite on a circular orbit going round the body in
    /// `sense` stays bound (Domingos et al. 2006, with the body's eccentricity).
    #[must_use]
    pub const fn satellite_limit(&self, sense: OrbitSense) -> Metres {
        match sense {
            OrbitSense::Prograde => self.prograde_limit,
            OrbitSense::Retrograde => self.retrograde_limit,
        }
    }

    /// The heaviest moon that, starting at the prograde limit, tides let survive to the time
    /// (Barnes and O'Brien 2002), with a rocky body's k₂ and Q or a giant's by class.
    #[must_use]
    pub const fn maximum_moon_mass(&self) -> EarthMasses {
        self.maximum_moon_mass
    }

    /// The fluid Roche limit about the body for material of density `satellite_density`, such as
    /// a ring's ice of 600 kg m⁻³ (P14.T20).
    #[must_use]
    pub fn roche_limit_fluid(&self, satellite_density: KilogramsPerCubicMetre) -> Metres {
        roche_limit_fluid(Metres::from(self.radius), self.density, satellite_density)
    }

    /// The rigid Roche limit about the body for a satellite of density `satellite_density`.
    #[must_use]
    pub fn roche_limit_rigid(&self, satellite_density: KilogramsPerCubicMetre) -> Metres {
        roche_limit_rigid(Metres::from(self.radius), self.density, satellite_density)
    }
}

/// [`derive_body`] could not derive a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveBodyError {
    /// The system is not yet born at the time: its age there is not positive.
    ///
    /// The fate transform (P14.T28) calls such a body not yet formed, and derives nothing.
    NotYetFormed {
        /// The system's age at the time.
        age: Years,
    },
    /// A giant's radius or composition could not be derived (P14.T11.d): its mass is above plan
    /// 13's 13 Jupiter masses, a brown dwarf's, which is no planet.
    Giant(DeriveGiantError),
    /// Plan 13's cooling of giant planets refused the body's mass or age.
    GiantCooling(EvaluateGiantCoolingError),
    /// The composition solve refused its inputs.
    ///
    /// The constructors of [`PlacedBody`], [`BodyHosts`] and the disc validate everything it
    /// reads, so this does not happen; it is kept so that no input can panic.
    Composition(SolveCompositionError),
}

impl fmt::Display for DeriveBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotYetFormed { age } => write!(
                f,
                "a body of a system aged {} years has not formed",
                age.value()
            ),
            Self::Giant(_) => f.write_str("a giant's radius or composition could not be derived"),
            Self::GiantCooling(_) => f.write_str("a giant's cooling could not be evaluated"),
            Self::Composition(_) => f.write_str("the composition solve refused its inputs"),
        }
    }
}

impl Error for DeriveBodyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Giant(e) => Some(e),
            Self::GiantCooling(e) => Some(e),
            Self::Composition(e) => Some(e),
            Self::NotYetFormed { .. } => None,
        }
    }
}

impl From<SolveCompositionError> for DeriveBodyError {
    fn from(e: SolveCompositionError) -> Self {
        Self::Composition(e)
    }
}

impl From<DeriveGiantError> for DeriveBodyError {
    fn from(e: DeriveGiantError) -> Self {
        Self::Giant(e)
    }
}

impl From<EvaluateGiantCoolingError> for DeriveBodyError {
    fn from(e: EvaluateGiantCoolingError) -> Self {
        Self::GiantCooling(e)
    }
}

/// The core mass fraction of a rocky outcome: `Some` when the confined rank `radius_rank` falls
/// between the iron curve of `window` and the top of its rocky outcomes
/// ([`RadiusWindow::dry_top`]), and then the fraction at the same position in the observed
/// distribution of rocky planets' (ruling 53, amending ruling 47.1; ruling 58).
///
/// With F Chen and Kipping's distribution at the window's mass, a rank r in (F(`R_iron`),
/// F(`R_top`)] is at s = (r − F(`R_iron`)) ÷ (F(`R_top`) − F(`R_iron`)) of the rocky part, and its
/// core mass fraction is [`rocky::core_mass_fraction_at_least`] of 1 − s above the top's own
/// fraction, so that the rocky part's ranks, uniform in s, follow Plotnykov and Valencia's (2020)
/// spread; the radius is then Zeng et al.'s at that composition.
///
/// - Inside the snow line the top is the rock curve and its fraction 0, so the rocky part takes
///   the whole distribution ([`rocky::core_mass_fraction_at`] of 1 − s, bit for bit).
/// - Beyond it the top is the Earth-like curve (ruling 58), and the rocky part takes the
///   distribution held to Earth's 0.325 and above: the ranks between the Earth-like and rock
///   curves keep Chen and Kipping's radius, which the solve reads as water on an Earth-like core,
///   so a body formed where ice condenses keeps its water.
///
/// The fraction is 1 at the iron curve's rank and the top's own at the top's rank, where the
/// radius meets Chen and Kipping's, which every rank above keeps: the radius is continuous in the
/// rank, and in the mass wherever Chen and Kipping's is, which is everywhere but their segment
/// transitions (P14.T11.a). Still one quantile per body (design note 8). `None` above the top:
/// envelopes, sub-Neptunes and water worlds.
///
/// # Examples
///
/// Earth's mass inside the snow line is rocky in nearly all its window, and its median rank has
/// the observed median composition:
///
/// ```
/// use hyperion_sim::planetary::derive::{SnowLineSide, radius_rank_in_window, radius_window, rocky_core_mass_fraction};
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::units::{EarthFluxes, EarthMasses};
///
/// let window = radius_window(EarthMasses::new(1.0), SnowLineSide::Inside, EarthFluxes::new(1.0))?;
/// let rank = radius_rank_in_window(UnitUniform::HALF, &window);
/// let cmf = rocky_core_mass_fraction(rank, &window).expect("a rocky outcome");
/// assert!((0.2..0.3).contains(&cmf));
/// # Ok::<(), hyperion_sim::planetary::derive::composition::SolveCompositionError>(())
/// ```
#[must_use]
pub fn rocky_core_mass_fraction(radius_rank: UnitUniform, window: &RadiusWindow) -> Option<f64> {
    let iron = chen_kipping_rank(window.mass(), window.least());
    let top = chen_kipping_rank(window.mass(), window.dry_top());
    let r = radius_rank.value();
    (r <= top).then(|| {
        let share = ((r - iron) / (top - iron)).clamp(0.0, 1.0);
        rocky::core_mass_fraction_at_least(1.0 - share, window.dry_top_core_mass_fraction())
    })
}

/// A body's drawn radius rank `rank`, confined to the part of Chen and Kipping's distribution at
/// the window's mass that lies inside `window` (ruling 47 of 2026-09-22, item 1).
///
/// The confined rank is F(`R_least`) + u (F(`R_greatest`) − F(`R_least`)), where F is
/// [`chen_kipping_rank`].
///
/// Chen and Kipping's scatter alone puts part of every mass outside what the composition curves
/// hold, and the solve would push those bodies onto a boundary: at 1 M⊕ inside the snow line 1.4%
/// onto pure iron and 27% onto rock, above 2.04 M⊕ a quarter onto iron, and at 100 M⊕ over half
/// onto the envelope limit. The rank is rescaled instead, which changes no draw: the radius is still
/// [`radius_chen_kipping`] of a rank, now that of the distribution truncated to the window, and no
/// body lands on a boundary. Where the window's upper edge lies more than about 8.3 σ above the
/// median, as a light core's envelope limit at low flux can, F rounds to 1 there; the rank is then
/// kept below 1, at a radius still inside the window (tested).
///
/// # Panics
///
/// Never: a [`RadiusWindow`] holds a positive, finite mass and radii, so that both ranks are
/// finite.
///
/// # Examples
///
/// At 3 M⊕ inside the snow line a quarter of Chen and Kipping's radii lie below pure iron, and the
/// lowest confined rank is still on iron's side of the window:
///
/// ```
/// use hyperion_sim::planetary::derive::{SnowLineSide, chen_kipping_rank, radius_rank_in_window, radius_window};
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::units::{EarthFluxes, EarthMasses};
///
/// let mass = EarthMasses::new(3.0);
/// let window = radius_window(mass, SnowLineSide::Inside, EarthFluxes::new(10.0))?;
/// let iron = chen_kipping_rank(mass, window.least());
/// assert!(iron > 0.1);
/// let low = radius_rank_in_window(UnitUniform::new(1e-6).expect("inside (0, 1)"), &window);
/// assert!(low.value() > iron);
/// # Ok::<(), hyperion_sim::planetary::derive::composition::SolveCompositionError>(())
/// ```
#[must_use]
pub fn radius_rank_in_window(rank: UnitUniform, window: &RadiusWindow) -> UnitUniform {
    let least = chen_kipping_rank(window.mass(), window.least());
    let greatest = chen_kipping_rank(window.mass(), window.greatest());
    let confined = least + rank.value() * (greatest - least);
    UnitUniform::new(confined.clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON / 2.0))
        .expect("a finite rank clamped into (0, 1) is a rank")
}

/// Everything plan 14 derives of the body `placed`, about `hosts`, formed in `disc`, in a system
/// aged `age` at the epoch, at time `t` (P14.T16.a): the only entry point the generator uses.
///
/// The steps and their order are the [module](self) documentation's. What is fixed at formation
/// reads only the placed body's mass, primordial orbit, formation distance and rank, and the disc,
/// so it never changes with `t`; the rest reads the orbit at the time and the hosts at the time,
/// which the caller evaluates at the system's age at `t`, `age` + (`t` − epoch), as plan 03's
/// `age_at` does. The function is pure: the same inputs give the same bits.
///
/// # Errors
///
/// - [`DeriveBodyError::NotYetFormed`] if the system's age at `t` is not positive.
/// - [`DeriveBodyError::Giant`] or [`DeriveBodyError::GiantCooling`] above 13 Jupiter masses,
///   the brown dwarfs', where plan 13's giant planets end.
///
/// # Panics
///
/// Never: the orbit's axis and eccentricity, and the derived mass and radius, are positive and
/// finite by construction of [`KeplerElements`], [`PlacedBody`] and the solve.
///
/// # Examples
///
/// Earth, at the rank that gives its composition's place in the observed spread (38%), about the
/// present Sun in the zero-age Sun's disc:
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::derive::{
///     BodyHosts, HostLight, PlacedBody, PlanetClass, derive_body,
/// };
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// use hyperion_sim::units::{
///     Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres, Radians,
///     SolarLuminosities, SolarMasses, SolarRadii, Years,
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
/// assert_eq!(derived.class(), PlanetClass::Rocky);
/// assert!((derived.equilibrium_temperature().value() - 255.0).abs() < 2.0);
/// assert!((derived.surface_gravity().value() - 9.8).abs() < 0.5);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn derive_body(
    placed: &PlacedBody,
    hosts: &BodyHosts<'_>,
    disc: &DiscProfile,
    age: Years,
    t: UniverseTime,
) -> Result<DerivedBody, DeriveBodyError> {
    let age_now = Years::new(age.value() + t.since_epoch().as_julian_years_f64());
    if age_now.value().is_nan() || age_now.value() <= 0.0 {
        return Err(DeriveBodyError::NotYetFormed { age: age_now });
    }
    let mass = placed.mass;
    let (formed, radius_rank, solved) = formation(placed, disc)?;

    // T12, at the time.
    let a = placed.orbit_now.semi_major_axis();
    let e = placed.orbit_now.eccentricity().value();
    let light = |host: &HostLight| {
        Illumination::new(*host, a, e)
            .expect("a Kepler orbit's axis is positive and its eccentricity in [0, 1)")
    };
    let own = hosts
        .orbited
        .iter()
        .fold(EarthFluxes::ZERO, |sum, host| sum + light(host).flux());
    let flux = hosts
        .companions
        .iter()
        .fold(own, |sum, companion| sum + companion.flux());
    let albedo = BondAlbedo::BEFORE_ATMOSPHERES;
    let irradiated = equilibrium_temperature(flux, albedo);

    // T11, the radius at the time, and what follows from it.
    let (radius, fractions, core, giant) = at_the_time(
        mass,
        formed,
        solved.as_ref(),
        flux,
        age_now,
        hosts.composition(),
    )?;
    let r = Metres::from(radius).value();
    let (internal_luminosity, t_eq) = match &giant {
        Some(giant) => {
            let internal = Watts::from(giant.internal_luminosity());
            (
                internal,
                with_internal_heat(irradiated, internal, Metres::new(r)),
            )
        }
        None => (Watts::ZERO, irradiated),
    };
    let mass_kg = Kilograms::from(mass);
    let volume = 4.0 / 3.0 * core::f64::consts::PI * (r * r * r);
    let density = KilogramsPerCubicMetre::new(mass_kg.value() / volume);
    let surface_gravity = MetresPerSecondSquared::new(GM_EARTH * mass.value() / (r * r));
    let class = PlanetClass::of(mass, &fractions);

    // T15.
    let hill = hill_radius(a, 0.0, mass_kg, hosts.primary_mass);
    let prograde_limit = satellite_stability_limit(hill, e, 0.0, OrbitSense::Prograde);
    let retrograde_limit = satellite_stability_limit(hill, e, 0.0, OrbitSense::Retrograde);
    let (love_number, tidal_q) = class.tides();
    let tidal = TidalPlanet::new(mass_kg, Metres::new(r), love_number, tidal_q)
        .expect("a derived body's mass and radius are positive and finite");
    let moon =
        maximum_surviving_moon_mass(&tidal, hosts.primary_mass, a, e, Seconds::from(age_now));

    Ok(DerivedBody {
        mass,
        radius_rank,
        formed,
        composition: solved,
        fractions,
        core,
        flux,
        albedo,
        equilibrium_temperature: t_eq,
        internal_luminosity,
        radius,
        density,
        surface_gravity,
        class,
        hill_radius: hill,
        prograde_limit,
        retrograde_limit,
        maximum_moon_mass: EarthMasses::from(moon),
    })
}

/// What [`derive_body`] fixes at formation (T11): the side of the disc's snow line the body formed
/// on, its confined rank, and its solved composition, which is `None` from 0.414 Jupiter masses,
/// where there is no window and no solve and the body is a giant (T11.d).
fn formation(
    placed: &PlacedBody,
    disc: &DiscProfile,
) -> Result<(SnowLineSide, UnitUniform, Option<SolvedComposition>), DeriveBodyError> {
    let mass = placed.mass;
    let formed = if placed.formation_distance.value() < disc.snow_line().value() {
        SnowLineSide::Inside
    } else {
        SnowLineSide::Beyond
    };
    let primordial = &placed.orbit;
    let formation_flux = luminosity_flux(
        disc.host_luminosity(),
        primordial.semi_major_axis(),
        primordial.eccentricity().value(),
    );
    let host = disc.host_mass();
    match radius_window(mass, formed, formation_flux) {
        Ok(window) if formed == SnowLineSide::Inside && m_dwarfs::rocky_host_share(host) > 0.0 => {
            let (rank, solved) = rocky_branch(placed.radius_rank, &window, host, formation_flux)?;
            Ok((formed, rank, Some(solved)))
        }
        Ok(window) => {
            let rank = radius_rank_in_window(placed.radius_rank, &window);
            let solved = match rocky_core_mass_fraction(rank, &window) {
                Some(cmf) => dry_composition(mass, cmf),
                None => composition(
                    mass,
                    radius_chen_kipping(mass, rank),
                    formed,
                    formation_flux,
                )?,
            };
            Ok((formed, rank, Some(solved)))
        }
        Err(SolveCompositionError::GiantPlanet { .. }) => Ok((formed, placed.radius_rank, None)),
        Err(other) => Err(other.into()),
    }
}

/// The formation solve of a body formed inside the snow line about a host of `host` that takes the
/// rocky branch (ruling 102.1; [`m_dwarfs`]): its confined rank and composition from its drawn
/// rank `drawn`, in `window` at the flux `flux`.
///
/// Of the window, Chen and Kipping's distribution leaves a share e₀ above the rock curve. The body
/// is enveloped with probability e = [`m_dwarfs::envelope_share`] of it instead: a drawn rank u
/// under 1 − e is rocky at s = u ÷ (1 − e) of the rocky spread ([`rocky_core_mass_fraction`]'s
/// share), and one above it takes Chen and Kipping's radius at (u − (1 − e)) ÷ e of their
/// distribution above the rock curve, solved into its envelope. At a share of 0 this is the
/// ordinary split, e = e₀; the confined rank returned is the one Chen and Kipping's distribution
/// gives the same radius outcome, as [`radius_rank_in_window`]'s is.
fn rocky_branch(
    drawn: UnitUniform,
    window: &RadiusWindow,
    host: SolarMasses,
    flux: EarthFluxes,
) -> Result<(UnitUniform, SolvedComposition), DeriveBodyError> {
    let mass = window.mass();
    let least = chen_kipping_rank(mass, window.least());
    let top = chen_kipping_rank(mass, window.dry_top());
    let greatest = chen_kipping_rank(mass, window.greatest());
    let span = greatest - least;
    let own = if span > 0.0 {
        ((greatest - top) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let enveloped = m_dwarfs::envelope_share(mass, host, own);
    let rocky = 1.0 - enveloped;
    let u = drawn.value();
    let rank = |r: f64| {
        UnitUniform::new(r.clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON / 2.0))
            .expect("a finite rank clamped into (0, 1) is a rank")
    };
    if u <= rocky {
        let share = (u / rocky).clamp(0.0, 1.0);
        let cmf =
            rocky::core_mass_fraction_at_least(1.0 - share, window.dry_top_core_mass_fraction());
        Ok((
            rank(least + share * (top - least)),
            dry_composition(mass, cmf),
        ))
    } else {
        let above = rank(top + (u - rocky) / enveloped * (greatest - top));
        let solved = composition(
            mass,
            radius_chen_kipping(mass, above),
            SnowLineSide::Inside,
            flux,
        )?;
        Ok((above, solved))
    }
}

/// The composition a body is given at formation (P14.T11, ruling 102.1): the part of
/// [`derive_body`] that its mass, primordial orbit, formation distance and rank fix in `disc`,
/// whatever the time. `None` from 0.414 Jupiter masses, a giant's, which has no solve.
///
/// Its [`radius`](SolvedComposition::radius) is the body's at [`COMPOSITION_REFERENCE_AGE`] and
/// the flux of its host's zero-age luminosity on its primordial orbit, the radius a survey of
/// systems some Gyr old compares with.
///
/// [`COMPOSITION_REFERENCE_AGE`]: crate::planetary::params::COMPOSITION_REFERENCE_AGE
///
/// # Errors
///
/// As [`derive_body`]'s for what is fixed at formation: [`DeriveBodyError::Composition`] for a
/// mass the solve refuses.
pub fn formation_composition(
    placed: &PlacedBody,
    disc: &DiscProfile,
) -> Result<Option<SolvedComposition>, DeriveBodyError> {
    formation(placed, disc).map(|(_, _, solved)| solved)
}

/// A body's radius, mass fractions and core at the time (T11), and its giant's radius if it is
/// one (T11.d): an envelope's radius at the body's age `age` and present flux `flux`; a giant's
/// from plan 13's cooling at that age in a system of composition `composition`, inflated for its
/// flux and blended into the envelope model's below 0.414 Jupiter masses.
fn at_the_time(
    mass: EarthMasses,
    formed: SnowLineSide,
    solved: Option<&SolvedComposition>,
    flux: EarthFluxes,
    age: Years,
    composition: &Composition,
) -> Result<
    (
        EarthRadii,
        MassFractions,
        CoreComposition,
        Option<GiantRadius>,
    ),
    DeriveBodyError,
> {
    let envelope_radius = |s: &SolvedComposition| {
        if s.envelope_fraction() > 0.0 {
            radius_with_envelope(
                mass,
                s.core(),
                s.envelope_fraction(),
                flux,
                Gigayears::from(age),
            )
        } else {
            s.radius()
        }
    };
    if let Some(s) = solved.filter(|_| giant_share(mass) <= 0.0) {
        return Ok((envelope_radius(s), s.fractions(), s.core(), None));
    }
    let interior = giant_cooling(JupiterMasses::from(mass), age, composition)?;
    let giant = radius_giant(mass, &interior, flux)?;
    let heavy = giant_composition(mass, formed)?;
    Ok(match solved {
        Some(s) => (
            giant.blended(envelope_radius(s)),
            heavy.blended(s.fractions()),
            s.core(),
            Some(giant),
        ),
        None => (giant.radius(), heavy.fractions(), heavy.core(), Some(giant)),
    })
}

/// The Solar System through [`derive_body`], for this module's tests and the record's.
#[cfg(test)]
pub(crate) mod solar {
    use super::*;
    use crate::orbit::{Eccentricity, Orientation};
    use crate::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    use crate::stellar::Composition;
    use crate::stellar::sse::{ZCoeffs, zams};
    use crate::units::consts::{
        EARTH_MASS_KG, EARTH_RADIUS_M, METRES_PER_AU, SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_MASS_KG,
    };
    use crate::units::{
        GravitationalParameter, Megayears, Radians, SolarLuminosities, SolarMasses, SolarRadii,
    };

    /// The Sun's age at the epoch, 4.57 Gyr.
    pub(crate) const SOLAR_AGE: Years = Years::new(4.57e9);

    /// The eight planets: name, mass (kg), volumetric mean radius (km), semi-major axis (au) and
    /// eccentricity, from NASA's planetary fact sheets.
    pub(crate) const PLANETS: [(&str, f64, f64, f64, f64); 8] = [
        ("Mercury", 0.330_10e24, 2_439.7, 0.387_098, 0.205_630),
        ("Venus", 4.867_3e24, 6_051.8, 0.723_332, 0.006_772),
        ("Earth", 5.972_2e24, 6_371.0, 1.000_001, 0.016_709),
        ("Mars", 0.641_69e24, 3_389.5, 1.523_679, 0.093_4),
        ("Jupiter", 1_898.13e24, 69_911.0, 5.203_8, 0.048_9),
        ("Saturn", 568.32e24, 58_232.0, 9.582_6, 0.056_5),
        ("Uranus", 86.811e24, 25_362.0, 19.191_26, 0.047_17),
        ("Neptune", 102.409e24, 24_622.0, 30.07, 0.008_678),
    ];

    /// The zero-age Sun's disc: plan 06's zero-age luminosity and radius of 1 M☉ at solar
    /// composition, the median draws and a lifetime of 3 Myr.
    pub(crate) fn solar_disc() -> DiscProfile {
        let (mass, composition) = (SolarMasses::new(1.0), Composition::SOLAR);
        let coeffs = ZCoeffs::new(composition.z_fit());
        let host = DiscHost::new(
            mass,
            composition.fe_h(),
            zams::luminosity(mass, &coeffs),
            zams::radius(mass, &coeffs),
        )
        .unwrap();
        *disc::derive(
            &host,
            Megayears::new(3.0),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        )
        .profile()
        .unwrap()
    }

    /// The zero-age disc of a host of `mass` M☉ at solar composition, with the median draws and a
    /// lifetime of 3 Myr.
    pub(crate) fn disc_of(mass: f64) -> DiscProfile {
        let (mass, composition) = (SolarMasses::new(mass), Composition::SOLAR);
        let coeffs = ZCoeffs::new(composition.z_fit());
        let host = DiscHost::new(
            mass,
            composition.fe_h(),
            zams::luminosity(mass, &coeffs),
            zams::radius(mass, &coeffs),
        )
        .unwrap();
        *disc::derive(
            &host,
            Megayears::new(3.0),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        )
        .profile()
        .unwrap()
    }

    pub(crate) fn sun() -> HostLight {
        HostLight::new(
            SolarLuminosities::new(1.0),
            Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K),
            SolarRadii::new(1.0),
        )
        .unwrap()
    }

    pub(crate) fn orbit(a_au: f64, e: f64) -> KeplerElements {
        let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap();
        KeplerElements::from_semi_major_axis(
            Metres::new(a_au * METRES_PER_AU),
            GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
            Eccentricity::new(e).unwrap(),
            orientation,
            Radians::ZERO,
        )
        .unwrap()
    }

    /// A body of `mass_kg` at `a_au` and `e` about the present Sun, formed where it is, with rank
    /// `rank`.
    pub(crate) fn placed(mass_kg: f64, a_au: f64, e: f64, rank: f64) -> PlacedBody {
        let orbit = orbit(a_au, e);
        PlacedBody::new(
            EarthMasses::new(mass_kg / EARTH_MASS_KG),
            orbit,
            orbit.semi_major_axis(),
            UnitUniform::new(rank).unwrap(),
        )
        .unwrap()
    }

    /// The drawn rank at which `derive_body` keeps the radius `radius_km` for a body of `mass_kg`
    /// at `a_au` and `e` in `disc`: the inverse of [`radius_rank_in_window`].
    pub(crate) fn rank_for(
        disc: &DiscProfile,
        mass_kg: f64,
        radius_km: f64,
        a_au: f64,
        e: f64,
    ) -> f64 {
        let mass = EarthMasses::new(mass_kg / EARTH_MASS_KG);
        let a = Metres::new(a_au * METRES_PER_AU);
        let side = if a < disc.snow_line() {
            SnowLineSide::Inside
        } else {
            SnowLineSide::Beyond
        };
        let window =
            radius_window(mass, side, luminosity_flux(disc.host_luminosity(), a, e)).unwrap();
        let least = chen_kipping_rank(mass, window.least());
        let greatest = chen_kipping_rank(mass, window.greatest());
        let radius = EarthRadii::new(radius_km * 1e3 / EARTH_RADIUS_M);
        let confined = if radius <= window.dry_top() {
            // A rocky outcome: the rank whose share of the rocky part puts the observed
            // composition at its place in the observed spread, held to the top's fraction.
            let cmf = composition(mass, radius, SnowLineSide::Inside, EarthFluxes::new(1.0))
                .unwrap()
                .core()
                .core_mass_fraction();
            let top = chen_kipping_rank(mass, window.dry_top());
            let below = rocky::core_mass_fraction_rank(window.dry_top_core_mass_fraction());
            let held = (rocky::core_mass_fraction_rank(cmf) - below) / (1.0 - below);
            least + (1.0 - held) * (top - least)
        } else {
            chen_kipping_rank(mass, radius)
        };
        (confined - least) / (greatest - least)
    }

    pub(crate) fn derive(
        body: &PlacedBody,
        disc: &DiscProfile,
    ) -> Result<DerivedBody, DeriveBodyError> {
        let lights = [sun()];
        let hosts = BodyHosts::new(
            Kilograms::new(SOLAR_MASS_KG),
            Composition::SOLAR,
            &lights,
            &[],
        )
        .unwrap();
        derive_body(body, &hosts, disc, SOLAR_AGE, UniverseTime::EPOCH)
    }

    /// The eight planets through `derive_body`, each at the rank that keeps its radius; Jupiter's
    /// radius reads no rank (ruling 56, item 4), and takes the median.
    pub(crate) fn solar_system() -> Vec<(&'static str, DerivedBody)> {
        let disc = solar_disc();
        PLANETS
            .iter()
            .map(|&(name, kg, km, a, e)| {
                let rank = if name == "Jupiter" {
                    0.5
                } else {
                    rank_for(&disc, kg, km, a, e)
                };
                assert!(
                    rank > 0.0 && rank < 1.0,
                    "{name}'s radius is inside its window"
                );
                (name, derive(&placed(kg, a, e, rank), &disc).unwrap())
            })
            .collect()
    }

    pub(crate) fn found<'a>(bodies: &'a [(&str, DerivedBody)], name: &str) -> &'a DerivedBody {
        &bodies.iter().find(|(n, _)| *n == name).unwrap().1
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    use super::solar::*;
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::planetary::derive::composition::RadiusAdjustment;
    use crate::units::AstronomicalUnits;
    use crate::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M, METRES_PER_AU, SOLAR_MASS_KG};

    #[test]
    fn the_solar_system_through_derive_body_reproduces_t11_to_t15() {
        let bodies = solar_system();
        // T11: radii, core mass fractions (as in `composition`'s tests) and envelopes.
        for (name, kg, km, ..) in PLANETS {
            let Some((_, body)) = bodies.iter().find(|(n, _)| *n == name) else {
                continue;
            };
            let observed = km * 1e3 / EARTH_RADIUS_M;
            let giant = body.fractions().envelope() > 0.0;
            let tolerance = if giant { 0.10 } else { 0.05 };
            let r = body.radius().value();
            assert!((r / observed - 1.0).abs() < tolerance, "{name}: {r} R⊕");
            assert_same_bits(body.mass().value(), kg / EARTH_MASS_KG);
        }
        for (name, cmf) in [
            ("Mercury", 0.70),
            ("Venus", 0.31),
            ("Earth", 0.325),
            ("Mars", 0.2),
        ] {
            let body = found(&bodies, name);
            let got = body.core().core_mass_fraction();
            assert!((got - cmf).abs() < 0.05, "{name}: {got}");
            assert_eq!(body.class(), PlanetClass::Rocky, "{name}");
            assert_eq!(body.formed(), SnowLineSide::Inside, "{name}");
        }
        for (name, range, class) in [
            ("Uranus", 0.05..0.25, PlanetClass::IceGiant),
            ("Neptune", 0.05..0.25, PlanetClass::IceGiant),
            ("Saturn", 0.65..0.9, PlanetClass::GasGiant),
        ] {
            let body = found(&bodies, name);
            let envelope = body.fractions().envelope();
            assert!(range.contains(&envelope), "{name}: {envelope}");
            assert_eq!(body.class(), class, "{name}");
            assert_eq!(body.formed(), SnowLineSide::Beyond, "{name}");
        }
        for (name, body) in &bodies {
            if let Some(solved) = body.composition() {
                assert_eq!(solved.adjustment(), RadiusAdjustment::Unchanged, "{name}");
            }
        }
        // T11.d: Jupiter goes through the giant path, with no solve, as a gas giant of Thorngren
        // et al.'s heavy elements, within 10% of its radius (T11.d's test (d)).
        let jupiter = found(&bodies, "Jupiter");
        assert!(jupiter.composition().is_none());
        assert_eq!(jupiter.class(), PlanetClass::GasGiant);
        let heavy = 1.0 - jupiter.fractions().envelope();
        assert!(
            (heavy * jupiter.mass().value() - 57.9).abs() < 0.1,
            "{heavy}"
        );
        // T12: the flux and equilibrium temperature are T12's at an albedo of 0.3; a giant adds
        // its internal luminosity, and Jupiter's makes it warmer than sunlight alone would.
        for (name, _, _, a, e) in PLANETS {
            let Some((_, body)) = bodies.iter().find(|(n, _)| *n == name) else {
                continue;
            };
            let light = Illumination::new(sun(), Metres::new(a * METRES_PER_AU), e).unwrap();
            let flux = total_flux(&[light]);
            assert_same_bits(body.flux().value(), flux.value());
            let t = equilibrium_temperature(flux, BondAlbedo::BEFORE_ATMOSPHERES);
            if name == "Jupiter" {
                let radius = Metres::from(body.radius());
                let warmed = with_internal_heat(t, body.internal_luminosity(), radius);
                assert_same_bits(body.equilibrium_temperature().value(), warmed.value());
                assert!(body.equilibrium_temperature() > t, "{name}");
            } else {
                assert_same_bits(body.internal_luminosity().value(), 0.0);
                assert_same_bits(body.equilibrium_temperature().value(), t.value());
            }
        }
        let earth = found(&bodies, "Earth");
        assert!((earth.equilibrium_temperature().value() - 255.0).abs() < 2.0);
        assert!((earth.surface_gravity().value() - 9.82).abs() < 0.05);
        assert!((earth.density().value() - 5_514.0).abs() < 50.0);
        // T15: Earth's Hill radius, moons inside their limits, and Saturn's Roche limit.
        let hill = earth.hill_radius().value();
        assert!((hill / 1.5e9 - 1.0).abs() < 0.01, "{hill}");
        for (planet, moon_axis_m, sense) in [
            ("Earth", 3.844e8, OrbitSense::Prograde),
            ("Saturn", 1.221_87e9, OrbitSense::Prograde),
            ("Saturn", 3.560_8e9, OrbitSense::Prograde),
            ("Saturn", 1.295_2e10, OrbitSense::Retrograde),
            ("Neptune", 3.547_6e8, OrbitSense::Retrograde),
            ("Mars", 9.376e6, OrbitSense::Prograde),
            ("Mars", 2.345_9e7, OrbitSense::Prograde),
        ] {
            let limit = found(&bodies, planet).satellite_limit(sense).value();
            assert!(
                moon_axis_m < limit,
                "{planet}: {moon_axis_m} against {limit}"
            );
        }
        let saturn = found(&bodies, "Saturn");
        let roche = saturn.roche_limit_fluid(KilogramsPerCubicMetre::new(600.0));
        let radii = roche.value() / Metres::from(saturn.radius()).value();
        assert!((2.5..2.7).contains(&radii), "{radii} mean radii");
        // The A ring's outer edge, 136,775 km, and the main rings inside it.
        assert!(roche.value() > 136_775e3, "{roche:?}");
        let moon_mass = 0.073_46e24 / EARTH_MASS_KG;
        assert!(earth.maximum_moon_mass().value() > moon_mass);
    }

    #[test]
    fn above_the_giants_a_body_is_refused_not_derived() {
        // 14 Jupiter masses is a brown dwarf's, outside plan 13's cooling of giant planets.
        let kg = 14.0 * crate::units::consts::JUPITER_MASS_KG;
        let result = derive(&placed(kg, 5.2, 0.05, 0.5), &solar_disc());
        assert!(
            matches!(
                result,
                Err(DeriveBodyError::GiantCooling(_) | DeriveBodyError::Giant(_))
            ),
            "{result:?}"
        );
    }

    #[test]
    fn two_calls_agree_bit_for_bit() {
        let (first, second) = (solar_system(), solar_system());
        for ((name, a), (_, b)) in first.iter().zip(&second) {
            let fields = |d: &DerivedBody| {
                let f = d.fractions();
                [
                    d.mass().value(),
                    d.radius_rank().value(),
                    d.flux().value(),
                    d.albedo().value(),
                    d.equilibrium_temperature().value(),
                    d.radius().value(),
                    d.internal_luminosity().value(),
                    d.density().value(),
                    d.surface_gravity().value(),
                    f.iron(),
                    f.rock(),
                    f.water(),
                    f.envelope(),
                    d.hill_radius().value(),
                    d.satellite_limit(OrbitSense::Prograde).value(),
                    d.satellite_limit(OrbitSense::Retrograde).value(),
                    d.maximum_moon_mass().value(),
                ]
            };
            for (x, y) in fields(a).into_iter().zip(fields(b)) {
                assert_same_bits(x, y);
            }
            assert_eq!(a.class(), b.class(), "{name}");
            assert_eq!(a.formed(), b.formed(), "{name}");
        }
    }

    /// Masses spanning the composition solve, M⊕.
    const MASSES: [f64; 13] = [
        1e-4, 0.01, 0.1, 0.5, 1.0, 1.4, 1.6, 2.1, 3.0, 10.0, 30.0, 100.0, 131.0,
    ];

    #[test]
    fn no_confined_rank_is_pushed_onto_a_composition_boundary() {
        for m in MASSES {
            let mass = EarthMasses::new(m);
            for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
                for s in [0.01, 10.0, 1_000.0] {
                    let flux = EarthFluxes::new(s);
                    let window = radius_window(mass, side, flux).unwrap();
                    for i in 1..1_000_u32 {
                        let rank = UnitUniform::new(f64::from(i) / 1_000.0).unwrap();
                        let confined = radius_rank_in_window(rank, &window);
                        let r = radius_chen_kipping(mass, confined);
                        let solved = composition(mass, r, side, flux).unwrap();
                        assert_eq!(
                            solved.adjustment(),
                            RadiusAdjustment::Unchanged,
                            "{m} M⊕ {side:?} at {s} F⊕, rank {i}‰"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn without_the_confinement_the_solve_pushes_the_shares_ruling_47_names() {
        let share = |m: f64, side: SnowLineSide, adjustment: RadiusAdjustment| {
            let (mass, flux) = (EarthMasses::new(m), EarthFluxes::new(10.0));
            let count = (1..1_000_u32)
                .filter(|&i| {
                    let rank = UnitUniform::new(f64::from(i) / 1_000.0).unwrap();
                    let r = radius_chen_kipping(mass, rank);
                    composition(mass, r, side, flux).unwrap().adjustment() == adjustment
                })
                .count();
            f64::from(u32::try_from(count).unwrap()) / 999.0
        };
        let inside = SnowLineSide::Inside;
        let iron = share(1.0, inside, RadiusAdjustment::RaisedToIron);
        assert!((0.010..0.018).contains(&iron), "{iron}");
        let rock = share(1.0, inside, RadiusAdjustment::ClampedToRock);
        assert!((0.24..0.30).contains(&rock), "{rock}");
        let iron = share(2.1, inside, RadiusAdjustment::RaisedToIron);
        assert!((0.23..0.28).contains(&iron), "{iron}");
        let limit = share(100.0, inside, RadiusAdjustment::ClampedToEnvelopeLimit);
        assert!((0.53..0.60).contains(&limit), "{limit}");
    }

    #[test]
    fn every_window_holds_part_of_the_distribution_and_its_extreme_ranks() {
        // The upper edge can be more than 8.3 σ above the median, where Φ rounds to 1 (a light
        // core's envelope limit at low flux); ranks within 10⁻¹² of 0 or 1 must still land inside.
        // Nearer than that a rank can meet an edge by rounding, as its radius then is the edge's
        // to the last bit, a chance of 10⁻¹² per body.
        let extremes = [1e-12, 1.0 - 1e-12];
        for m in MASSES {
            let mass = EarthMasses::new(m);
            for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
                for s in [1e-3, 1.0, 1_000.0] {
                    let flux = EarthFluxes::new(s);
                    let window = radius_window(mass, side, flux).unwrap();
                    let least = chen_kipping_rank(mass, window.least());
                    let greatest = chen_kipping_rank(mass, window.greatest());
                    let label = format!("{m} M⊕ {side:?} at {s} F⊕: {least} to {greatest}");
                    assert!(
                        0.0 < least && least < greatest && greatest <= 1.0,
                        "{label}"
                    );
                    for u in extremes {
                        let rank = radius_rank_in_window(UnitUniform::new(u).unwrap(), &window);
                        let r = radius_chen_kipping(mass, rank);
                        let solved = composition(mass, r, side, flux).unwrap();
                        assert_eq!(solved.adjustment(), RadiusAdjustment::Unchanged, "{label}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_system_not_yet_born_derives_nothing() {
        let (_, kg, _, a, e) = PLANETS[2];
        let lights = [sun()];
        let hosts = BodyHosts::new(
            Kilograms::new(SOLAR_MASS_KG),
            Composition::SOLAR,
            &lights,
            &[],
        )
        .unwrap();
        let body = placed(kg, a, e, 0.5);
        let t = UniverseTime::from_julian_years(-1_000).unwrap();
        let result = derive_body(&body, &hosts, &solar_disc(), Years::new(500.0), t);
        assert_eq!(
            result,
            Err(DeriveBodyError::NotYetFormed {
                age: Years::new(-500.0)
            })
        );
    }

    #[test]
    fn a_close_in_rocky_planet_keeps_no_moon_over_a_millionth_of_an_earth() {
        let disc = solar_disc();
        let lights = [sun()];
        let hosts = BodyHosts::new(
            Kilograms::new(SOLAR_MASS_KG),
            Composition::SOLAR,
            &lights,
            &[],
        )
        .unwrap();
        let body = placed(EARTH_MASS_KG, 0.05, 0.0, 0.5);
        let derived =
            derive_body(&body, &hosts, &disc, Years::new(5e9), UniverseTime::EPOCH).unwrap();
        assert_eq!(derived.class(), PlanetClass::Rocky);
        assert!(derived.maximum_moon_mass().value() < 1e-6);
    }

    #[test]
    fn two_equal_orbited_hosts_make_a_body_two_to_the_quarter_hotter() {
        let disc = solar_disc();
        let body = placed(EARTH_MASS_KG, 2.0, 0.0, 0.5);
        let temperature = |lights: &[HostLight]| {
            let hosts = BodyHosts::new(
                Kilograms::new(SOLAR_MASS_KG),
                Composition::SOLAR,
                lights,
                &[],
            )
            .unwrap();
            derive_body(&body, &hosts, &disc, SOLAR_AGE, UniverseTime::EPOCH)
                .unwrap()
                .equilibrium_temperature()
                .value()
        };
        let ratio = temperature(&[sun(), sun()]) / temperature(&[sun()]);
        assert!(
            (ratio - crate::math::powf(2.0, 0.25)).abs() < 1e-12,
            "{ratio}"
        );
    }

    #[test]
    fn classes_follow_the_composition() {
        let disc = solar_disc();
        let lights = [sun()];
        let hosts = BodyHosts::new(
            Kilograms::new(SOLAR_MASS_KG),
            Composition::SOLAR,
            &lights,
            &[],
        )
        .unwrap();
        let class = |m: f64, a: f64, rank: f64| {
            let body = placed(m * EARTH_MASS_KG, a, 0.0, rank);
            derive_body(&body, &hosts, &disc, SOLAR_AGE, UniverseTime::EPOCH)
                .unwrap()
                .class()
        };
        assert_eq!(class(1.0, 1.0, 0.5), PlanetClass::Rocky);
        assert_eq!(class(1.0, 5.0, 0.99), PlanetClass::Icy);
        assert_eq!(class(5.0, 0.1, 0.9), PlanetClass::SubNeptune);
        assert_eq!(class(30.0, 5.0, 0.5), PlanetClass::IceGiant);
        assert_eq!(class(90.0, 5.0, 0.5), PlanetClass::GasGiant);
        assert!(!PlanetClass::GasGiant.has_surface() && PlanetClass::SubNeptune.has_surface());
    }

    #[test]
    fn the_giant_path_joins_on_continuously_at_both_ends_of_its_blend() {
        // At 0.3 Jupiter masses, where the giant's share rises from zero, and at 0.414, where the
        // solve ends, the radius, the fractions and the temperature are continuous in mass.
        let disc = solar_disc();
        for (m_j, a_au) in [(0.3, 9.58), (0.3, 0.05), (0.414, 5.2), (0.414, 0.05)] {
            let edge = crate::units::consts::GM_JUPITER / crate::units::consts::GM_EARTH * m_j;
            let edge = if m_j > 0.4 {
                radius::NEPTUNIAN_JOVIAN_TRANSITION.value()
            } else {
                edge
            };
            let at = |m: f64| derive(&placed(m * EARTH_MASS_KG, a_au, 0.02, 0.5), &disc).unwrap();
            let (below, above) = (at(edge * (1.0 - 1e-9)), at(edge * (1.0 + 1e-9)));
            let label = format!("{m_j} M_J at {a_au} au");
            let close = |x: f64, y: f64| (x / y - 1.0).abs() < 1e-4;
            assert!(
                close(below.radius().value(), above.radius().value()),
                "{label}: radius"
            );
            assert!(
                (below.fractions().envelope() - above.fractions().envelope()).abs() < 1e-4,
                "{label}: envelope"
            );
            let (t0, t1) = (
                below.equilibrium_temperature(),
                above.equilibrium_temperature(),
            );
            assert!(close(t0.value(), t1.value()), "{label}: {t0:?} {t1:?}");
        }
    }

    #[test]
    fn what_is_fixed_at_formation_does_not_move_with_the_orbit_at_the_time() {
        // A host that has lost half its mass has widened the orbit twofold (design note 11); the
        // composition and the confined rank stay those of the primordial orbit.
        let disc = solar_disc();
        let body = placed(5.0 * EARTH_MASS_KG, 0.1, 0.02, 0.7);
        let widened = body.with_orbit_now(orbit(0.2, 0.02));
        let (before, after) = (
            derive(&body, &disc).unwrap(),
            derive(&widened, &disc).unwrap(),
        );
        let fixed = |d: &DerivedBody| {
            let f = d.fractions();
            [
                d.radius_rank().value(),
                f.iron(),
                f.rock(),
                f.water(),
                f.envelope(),
            ]
        };
        for (x, y) in fixed(&before).into_iter().zip(fixed(&after)) {
            assert_same_bits(x, y);
        }
        assert_eq!(before.class(), after.class());
        let ratio = before.flux().value() / after.flux().value();
        assert!((ratio - 4.0).abs() < 1e-12, "{ratio}");
    }

    #[test]
    fn the_hosts_and_the_placed_body_are_validated() {
        assert_eq!(
            BodyHosts::new(Kilograms::ZERO, Composition::SOLAR, &[sun()], &[]),
            Err(BuildBodyHostsError::PrimaryMassNotPositive)
        );
        assert_eq!(
            BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &[], &[]),
            Err(BuildBodyHostsError::NoOrbitedHost)
        );
        let (orbit, rank) = (orbit(1.0, 0.0), UnitUniform::HALF);
        assert_eq!(
            PlacedBody::new(EarthMasses::new(f64::NAN), orbit, Metres::new(1.0), rank),
            Err(BuildPlacedBodyError::MassNotPositive)
        );
        assert_eq!(
            PlacedBody::new(EarthMasses::new(1.0), orbit, Metres::ZERO, rank),
            Err(BuildPlacedBodyError::FormationDistanceNotPositive)
        );
    }

    /// The radius at formation that a drawn rank `u` gives a body of `m` M⊕ on `side` at `flux`:
    /// Zeng's at the observed composition for a rocky outcome, Chen and Kipping's otherwise.
    fn formation_radius(m: f64, u: f64, side: SnowLineSide, flux: EarthFluxes) -> f64 {
        let mass = EarthMasses::new(m);
        let window = radius_window(mass, side, flux).unwrap();
        let rank = radius_rank_in_window(UnitUniform::new(u).unwrap(), &window);
        confined_radius(mass, rank, &window, side, flux)
    }

    fn confined_radius(
        mass: EarthMasses,
        rank: UnitUniform,
        window: &RadiusWindow,
        side: SnowLineSide,
        flux: EarthFluxes,
    ) -> f64 {
        match rocky_core_mass_fraction(rank, window) {
            Some(cmf) => dry_composition(mass, cmf).radius().value(),
            None => composition(mass, radius_chen_kipping(mass, rank), side, flux)
                .unwrap()
                .radius()
                .value(),
        }
    }

    #[test]
    fn the_radius_is_continuous_across_the_top_of_the_rocky_outcomes_and_in_mass() {
        let flux = EarthFluxes::new(1.0);
        for m in [0.3, 1.0, 1.6, 3.0, 10.0, 60.0] {
            let mass = EarthMasses::new(m);
            for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
                // Either side of the top's rank (the rock curve inside the snow line, the
                // Earth-like curve beyond it, ruling 58): the rocky outcome's lightest composition
                // meets Chen and Kipping's radius there, to the envelope model's own resolution
                // where an envelope begins (dips of up to 1.1 × 10⁻⁵ of the radius, `envelope`'s
                // documentation).
                let window = radius_window(mass, side, flux).unwrap();
                let top = chen_kipping_rank(mass, window.dry_top());
                let at = |r: f64| {
                    confined_radius(mass, UnitUniform::new(r).unwrap(), &window, side, flux)
                };
                let (below, above) = (at(top * (1.0 - 1e-12)), at(top * (1.0 + 1e-12)));
                assert!(
                    (above / below - 1.0).abs() < 1.1e-5,
                    "{m} M⊕ {side:?}: {below}, {above}"
                );
                assert!(
                    rocky_core_mass_fraction(UnitUniform::new(top).unwrap(), &window).is_some()
                );
            }
        }
        // In mass, at fixed drawn ranks, inside each of Chen and Kipping's segments and across
        // the 1.5 M⊕ envelope floor. Just above the floor the window's top rises steeply but
        // continuously beyond the snow line, as the envelope model is linear in fractions under
        // 10⁻⁴, so the step is small.
        for u in [0.05, 0.3, 0.6, 0.9] {
            for m in [0.3, 0.9, 1.49, 1.5, 1.51, 1.9, 2.5, 8.0, 30.0] {
                for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
                    let (r0, r1) = (
                        formation_radius(m, u, side, flux),
                        formation_radius(m * (1.0 + 1e-10), u, side, flux),
                    );
                    assert!(
                        (r1 / r0 - 1.0).abs() < 1e-6,
                        "{m} M⊕ {side:?} rank {u}: {r0}, {r1}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_rocky_outcome_takes_the_observed_spread_and_zeng_s_radius() {
        let (mass, flux) = (EarthMasses::new(1.2), EarthFluxes::new(1.0));
        let window = radius_window(mass, SnowLineSide::Inside, flux).unwrap();
        let iron = chen_kipping_rank(mass, window.least());
        let rock = chen_kipping_rank(mass, window.rock());
        let middle = UnitUniform::new(f64::midpoint(iron, rock)).unwrap();
        let cmf = rocky_core_mass_fraction(middle, &window).unwrap();
        assert!(
            (cmf - rocky::MEDIAN_CORE_MASS_FRACTION).abs() < 1e-12,
            "{cmf}"
        );
        let solved = dry_composition(mass, cmf);
        let zeng = radius_zeng(mass, CoreComposition::new(cmf, 0.0).unwrap());
        assert_same_bits(solved.radius().value(), zeng.value());
        assert!(
            rocky_core_mass_fraction(UnitUniform::new(rock * 1.001).unwrap(), &window).is_none()
        );
    }

    /// Ruling 58: beyond the snow line only the ranks below the Earth-like curve are rocky
    /// outcomes, held to Earth's core fraction and above, and every rank above that curve keeps
    /// its water; inside the snow line the rocky outcomes still reach the rock curve.
    #[test]
    fn beyond_the_snow_line_rocky_outcomes_end_at_the_earth_like_curve_and_bodies_keep_water() {
        let flux = EarthFluxes::new(0.05);
        let (mut watery, mut above_earth, mut between) = (0_u32, 0_u32, 0_u32);
        for m in [0.1, 0.3, 0.5, 0.8, 1.0, 1.2, 1.5, 1.7, 1.9] {
            let mass = EarthMasses::new(m);
            let window = radius_window(mass, SnowLineSide::Beyond, flux).unwrap();
            let earth = chen_kipping_rank(mass, window.dry_top());
            let rock = chen_kipping_rank(mass, window.rock());
            assert!(
                earth < rock,
                "{m} M⊕: the Earth-like curve lies below the rock curve"
            );
            let inside = radius_window(mass, SnowLineSide::Inside, flux).unwrap();
            assert_same_bits(inside.dry_top().value(), inside.rock().value());
            assert!(inside.dry_top_core_mass_fraction().abs() < f64::EPSILON);
            for i in 1..1_000_u32 {
                let rank =
                    radius_rank_in_window(UnitUniform::new(f64::from(i) / 1e3).unwrap(), &window);
                let solved = if let Some(cmf) = rocky_core_mass_fraction(rank, &window) {
                    assert!(rank.value() <= earth);
                    assert!(
                        cmf >= radius::EARTH_CORE_MASS_FRACTION - 1e-12,
                        "{m}: {cmf}"
                    );
                    dry_composition(mass, cmf)
                } else {
                    above_earth += 1;
                    if rank.value() <= rock {
                        between += 1;
                    }
                    let r = radius_chen_kipping(mass, rank);
                    composition(mass, r, SnowLineSide::Beyond, flux).unwrap()
                };
                if solved.fractions().water() > 0.0 {
                    watery += 1;
                }
            }
        }
        // Every body above the Earth-like curve is watery, and those between it and the rock
        // curve, dry under ruling 53 alone, are among them: 5,071 of the 8,991, the count before
        // ruling 53, against 2,596 under it.
        assert_eq!(watery, above_earth);
        assert_eq!(watery, 5_071);
        assert!(
            between > 500,
            "{between} bodies between the Earth-like and rock curves"
        );
    }

    #[test]
    fn derive_body_is_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let pin = |w: &mut GoldenWriter, name: &str, d: &DerivedBody| {
            let f = d.fractions();
            w.f64(&format!("{name}_radius_rank"), d.radius_rank().value());
            w.line(&format!("{name}_formed = {:?}", d.formed()));
            w.f64(&format!("{name}_radius"), d.radius().value());
            w.f64(&format!("{name}_density"), d.density().value());
            w.f64(
                &format!("{name}_surface_gravity"),
                d.surface_gravity().value(),
            );
            w.f64(&format!("{name}_iron"), f.iron());
            w.f64(&format!("{name}_rock"), f.rock());
            w.f64(&format!("{name}_water"), f.water());
            w.f64(&format!("{name}_envelope"), f.envelope());
            w.f64(&format!("{name}_flux"), d.flux().value());
            w.f64(&format!("{name}_t_eq"), d.equilibrium_temperature().value());
            w.f64(
                &format!("{name}_internal_luminosity"),
                d.internal_luminosity().value(),
            );
            w.line(&format!("{name}_class = {:?}", d.class()));
            w.f64(&format!("{name}_hill_radius"), d.hill_radius().value());
            let prograde = d.satellite_limit(OrbitSense::Prograde);
            w.f64(&format!("{name}_prograde_limit"), prograde.value());
            let retrograde = d.satellite_limit(OrbitSense::Retrograde);
            w.f64(&format!("{name}_retrograde_limit"), retrograde.value());
            w.f64(
                &format!("{name}_maximum_moon_mass"),
                d.maximum_moon_mass().value(),
            );
        };
        for (name, body) in solar_system() {
            pin(&mut w, &name.to_lowercase(), &body);
        }
        let disc = solar_disc();
        for (name, m, a_au, e, rank) in [
            ("hot_jupiter", 318.0, 0.05, 0.01, 0.5),
            ("blended_giant", 110.0, 5.0, 0.05, 0.5),
            ("hot_sub_neptune", 5.0, 0.05, 0.01, 0.5),
            ("icy", 1.0, 5.0, 0.05, 0.9),
            ("light", 0.01, 0.8, 0.1, 0.1),
        ] {
            let body = derive(&placed(m * EARTH_MASS_KG, a_au, e, rank), &disc).unwrap();
            pin(&mut w, name, &body);
        }
        let snow = AstronomicalUnits::from(disc.snow_line()).value();
        w.f64("solar_snow_line_au", snow);
        // Ruling 58: beyond the snow line, a body among the rocky outcomes, held to Earth's core
        // fraction and above, and one between the Earth-like and rock curves, which keeps Chen
        // and Kipping's radius and its water.
        for (name, dry) in [("beyond_dry", true), ("beyond_watery", false)] {
            let (earths, a_au, eccentricity) = (1.0, 4.0, 0.05);
            let mass = EarthMasses::new(earths);
            let axis = Metres::new(a_au * METRES_PER_AU);
            let flux = luminosity_flux(disc.host_luminosity(), axis, eccentricity);
            let window = radius_window(mass, SnowLineSide::Beyond, flux).unwrap();
            let rank = |r: EarthRadii| chen_kipping_rank(mass, r);
            let (least, greatest) = (rank(window.least()), rank(window.greatest()));
            let (top, rock) = (rank(window.dry_top()), rank(window.rock()));
            let target = if dry {
                f64::midpoint(least, top)
            } else {
                f64::midpoint(top, rock)
            };
            let u = (target - least) / (greatest - least);
            let placed = placed(earths * EARTH_MASS_KG, a_au, eccentricity, u);
            let body = derive(&placed, &disc).unwrap();
            assert_eq!(body.formed(), SnowLineSide::Beyond);
            if dry {
                assert!(body.fractions().water() <= 0.0);
                assert!(body.core().core_mass_fraction() >= radius::EARTH_CORE_MASS_FRACTION);
            } else {
                assert!(body.fractions().water() > 0.0);
            }
            pin(&mut w, name, &body);
        }
        golden!("planetary/derive_body", w.as_str());
    }

    /// Ruling 102.1: about an M dwarf a body formed inside the snow line is enveloped at the
    /// branch's probability, its radius increasing in its rank and continuous across the split;
    /// across the host blend the share moves from it to Chen and Kipping's own; and about hosts
    /// from 0.70 M☉ the solve is the ordinary one, bit for bit.
    #[test]
    fn m_dwarfs_inner_planets_are_rocky_unless_they_draw_an_envelope() {
        let a_au = 0.03;
        for earths in [0.8, 2.0, 4.0, 6.0, 10.0] {
            let mass = EarthMasses::new(earths);
            let solve = |host: f64, u: f64| {
                let disc = disc_of(host);
                let body = placed(earths * EARTH_MASS_KG, a_au, 0.0, u);
                formation_composition(&body, &disc).unwrap().unwrap()
            };
            let shares = [0.3, 0.62, 0.66, 0.72].map(|host| {
                let n = 4_000_u32;
                let mut last = 0.0;
                let mut enveloped = 0_u32;
                for k in 0..n {
                    let u = (f64::from(k) + 0.5) / f64::from(n);
                    let solved = solve(host, u);
                    let r = solved.radius().value();
                    assert!(
                        r >= last - 1e-9,
                        "{earths} M_earth at {u}: {r} after {last}"
                    );
                    last = r;
                    enveloped += u32::from(solved.envelope_fraction() > 0.0);
                }
                f64::from(enveloped) / f64::from(n)
            });
            let disc = disc_of(0.3);
            let flux = luminosity_flux(
                disc.host_luminosity(),
                orbit(a_au, 0.0).semi_major_axis(),
                0.0,
            );
            let window = radius_window(mass, SnowLineSide::Inside, flux).unwrap();
            let rank = |r: EarthRadii| chen_kipping_rank(mass, r);
            let own = (rank(window.greatest()) - rank(window.rock()))
                / (rank(window.greatest()) - rank(window.least()));
            let expected = m_dwarfs::envelope_share(mass, SolarMasses::new(0.3), own);
            assert!(
                (shares[0] - expected).abs() < 1e-3,
                "{earths}: {shares:?} against {expected}"
            );
            // Continuous across the split between the rocky and the enveloped ranks.
            if expected > 0.0 && expected < 1.0 {
                let split = 1.0 - expected;
                let (below, above) = (solve(0.3, split - 1e-9), solve(0.3, split + 1e-9));
                let jump = above.radius().value() - below.radius().value();
                assert!(jump.abs() < 1e-3, "{earths}: {jump} at {split}");
                assert!(
                    below.envelope_fraction() <= 0.0,
                    "{earths}: rocky below the split"
                );
            }
            assert!(
                shares.windows(2).all(|w| w[0] <= w[1] + 1e-3),
                "{earths}: {shares:?}"
            );
            // From 0.70 M☉ the ordinary solve, bit for bit.
            let disc = disc_of(0.72);
            for u in [0.1, 0.5, 0.9] {
                let body = placed(earths * EARTH_MASS_KG, a_au, 0.0, u);
                let (_, rank, solved) = formation(&body, &disc).unwrap();
                let window = radius_window(
                    mass,
                    SnowLineSide::Inside,
                    luminosity_flux(disc.host_luminosity(), body.orbit().semi_major_axis(), 0.0),
                )
                .unwrap();
                let confined = radius_rank_in_window(body.radius_rank(), &window);
                assert_same_bits(rank.value(), confined.value());
                let expected = match rocky_core_mass_fraction(confined, &window) {
                    Some(cmf) => dry_composition(mass, cmf),
                    None => composition(
                        mass,
                        radius_chen_kipping(mass, confined),
                        SnowLineSide::Inside,
                        luminosity_flux(
                            disc.host_luminosity(),
                            body.orbit().semi_major_axis(),
                            0.0,
                        ),
                    )
                    .unwrap(),
                };
                assert_eq!(solved.unwrap(), expected);
            }
        }
    }
}
