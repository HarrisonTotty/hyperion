//! Moons: the regular satellites of giants, giant-impact moons and captured irregulars (plan 14,
//! P14.T17–T19; the brainstorm's "Moons (regular satellites scaled to the planet's mass, captured
//! irregulars, the occasional giant-impact moon)").
//!
//! Every draw here is on the parent's own streams, keyed by the parent's
//! [`BodyId`] (design note 4): a planet's moons can be generated alone, and
//! adding a property to moons can never move a planet. The three kinds are independent, each on
//! its own domain tags, so that the order in which P14.T22's `generate_satellites` runs them
//! (regular moons, the giant-impact moon, captures and their removals) moves no draw of another:
//!
//! - [`regular`]: a giant's regular satellites (P14.T17), on
//!   [`tags::MOON_COUNT`](crate::rng::tags::MOON_COUNT),
//!   [`tags::MOON_MASS`](crate::rng::tags::MOON_MASS) and
//!   [`tags::MOON_ORBIT`](crate::rng::tags::MOON_ORBIT), with their derivation (T17.b);
//! - [`impact`]: the moon of a giant impact on a rocky or icy planet or a dwarf planet, and its
//!   tidal recession (P14.T18), on [`tags::MOON_IMPACT`](crate::rng::tags::MOON_IMPACT);
//! - [`irregular`]: the captured population of a giant, a Triton-like capture about an ice giant,
//!   and a rocky planet's small captures (P14.T19), on
//!   [`tags::MOON_CAPTURE`](crate::rng::tags::MOON_CAPTURE).
//!
//! # What the parent is
//!
//! A [`MoonParent`] is what every kind reads of the body it orbits: its ID, its mass, radius,
//! density and class, its orbit about its host and the host's mass, and the heaviest moon that
//! tides let survive about it, all at the epoch (P14.T16.a's [`DerivedBody`] at
//! [`UniverseTime::EPOCH`](crate::time::UniverseTime::EPOCH)). Moons are primordial, like planets
//! (design note 1): they are drawn from the parent as it is at the epoch and never from a time.
//! The satellite derivation of T17.b and the recession of T18 are the functions of time.
//!
//! # Frames
//!
//! Every satellite orbit is a [`KeplerElements`] about its parent, with μ = G (`M_p` + m), whose
//! inclination, node and periapsis the generators here draw against a reference plane they never
//! orient: the equator for regular moons and for a giant-impact moon, which forms in the
//! equatorial plane of the debris disc (see [`impact`]), and the orbital plane for captures.
//! P14.T22.a places that plane in the parent's body frame: the parent's local Laplace plane at
//! the moon's distance (P14.T56.a, [`laplace`](crate::planetary::laplace)), which is the equator
//! well inside the Laplace radius and the orbital plane well beyond it, from [`MOONS_VERSION`],
//! and the parent's orbital plane below it.
//!
//! # Sub-indices
//!
//! Moons are numbered by P14.T22.a, which runs the three kinds for one planet and gives each moon
//! its sub-index of design note 3 (`0x01`–`0x7F`). Each kind here numbers its own moons from 1
//! inside out ([`regular::RegularMoon::ordinal`] and its siblings), and draws by that ordinal, so
//! that no kind's draws depend on another's count.

use std::error::Error;
use std::fmt;

use crate::GeneratorVersion;
use crate::id::BodyId;
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::planetary::derive::{
    DerivedBody, OrbitSense, PlanetClass, hill_radius, roche_limit_fluid, satellite_stability_limit,
};
use crate::planetary::laplace::SatellitePlanes;
use crate::planetary::moons::regular::{MoonInventories, MoonTideRule};
use crate::rng::{Stream, Threshold};
use crate::stellar::draws::UnitUniform;
use crate::units::{
    EarthMasses, GravitationalParameter, Kilograms, KilogramsPerCubicMetre, Metres, Radians,
};

pub mod impact;
pub mod irregular;
pub mod regular;
pub mod tides;

pub use impact::{ImpactMoon, giant_impact_moon};
pub use irregular::{
    BeltAdjacency, CaptureKind, CapturedMoon, Captures, IrregularPopulation, NearestBelt, captures,
};
pub use regular::{DerivedMoon, RegularMoon, RegularMoons, Volcanism, regular_moons};

/// The generator version from which every moon and ring lies on its parent's Laplace surface
/// (P14.T56.a), and its later subtasks' moons' own ranks, read at their own bulk, and tides
/// (P14.T56.c and its follow-up, T56.d): 23, the
/// 22 → 23 batch's bump (plan 14, P14.T56; decision-bump-plan-after-22 §1.2).
///
/// Below it every value is version 22's, bit for bit
/// ([`SatellitePlanes::in_force`](crate::planetary::laplace::SatellitePlanes::in_force), and the
/// crate-private `MoonInventories::in_force`, `MoonBulkRule::in_force` and
/// `MoonTideRule::in_force` for the moons' volatiles, the bulk their inventory and atmosphere are
/// read at, and their tides; `MoonRules` gathers the generation's).
/// P14.T54.a deletes the hold with its tripwire (`the_moons_are_held_only_until_the_batch_s_bump`)
/// and blesses the moved goldens with its bump.
pub const MOONS_VERSION: GeneratorVersion = GeneratorVersion::new(23);

/// The rules a generated system's satellites are made under, P14.T56's holds at generation
/// (crate-private): the planes they lie in (T56.a), the volatiles and bulk their moons' air is read
/// with (T56.c and its follow-up), and the moons' tides (T56.d), which the stored rotation law
/// reads, so that they are a switch of
/// [`generate_with`](crate::planetary::system::generate_with) and not a setter after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MoonRules {
    /// The planes its satellites lie in.
    pub(crate) planes: SatellitePlanes,
    /// The volatiles its moons are derived with, and the bulk their air is read at.
    pub(crate) inventories: MoonInventories,
    /// Its moons' tides.
    pub(crate) tides: MoonTideRule,
}

impl MoonRules {
    /// The rules the generator version in force takes: version 22's below [`MOONS_VERSION`].
    #[must_use]
    pub(crate) const fn in_force() -> Self {
        Self {
            planes: SatellitePlanes::in_force(),
            inventories: MoonInventories::in_force(),
            tides: MoonTideRule::in_force(),
        }
    }

    /// Every rule of the 22 → 23 batch, as version 23 takes them: what the tests reach ahead of
    /// the bump.
    #[cfg(test)]
    pub(crate) const BATCH: Self = Self {
        planes: SatellitePlanes::LaplaceSurface,
        inventories: MoonInventories::Own,
        tides: MoonTideRule::ByState,
    };
}

/// Which kind of body a [`MoonParent`] is: a planet, or a dwarf planet (P14.T21.c's largest belt
/// members), which T18 gives its own range of mass ratios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ParentKind {
    /// A planet, primordial or second-generation, or a rogue planet (P14.T27.b).
    Planet,
    /// A dwarf planet: one of a belt's largest members (P14.T21.c).
    DwarfPlanet,
}

/// The plain values a [`MoonParent`] is built from ([`MoonParent::new`]), for a parent that is not
/// (or not yet) a [`DerivedBody`]: synthetic parents in tests and tools.
///
/// Every quantity is the parent's at the epoch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonParentParts {
    /// The parent's ID, whose streams every moon draws on.
    pub id: BodyId,
    /// A planet or a dwarf planet.
    pub kind: ParentKind,
    /// The parent's mass.
    pub mass: EarthMasses,
    /// The parent's radius.
    pub radius: Metres,
    /// The parent's class by composition (P14.T16.a).
    pub class: PlanetClass,
    /// The parent's orbit about its host.
    pub orbit: KeplerElements,
    /// The mass the parent orbits: its star's, or a pair's.
    pub host_mass: Kilograms,
    /// The heaviest moon tides let survive about the parent (P14.T15,
    /// [`DerivedBody::maximum_moon_mass`]).
    pub maximum_moon_mass: EarthMasses,
}

/// The body a moon orbits, as the moon generators read it (see the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonParent {
    id: BodyId,
    kind: ParentKind,
    mass: EarthMasses,
    radius: Metres,
    density: KilogramsPerCubicMetre,
    class: PlanetClass,
    orbit: KeplerElements,
    formation_distance: Metres,
    host_mass: Kilograms,
    maximum_moon_mass: EarthMasses,
}

impl MoonParent {
    /// The parent of the plain values `parts`.
    ///
    /// # Errors
    ///
    /// [`BuildMoonParentError`] for a mass, radius or host mass that is not positive and finite,
    /// or a maximum moon mass that is negative or not finite.
    pub fn new(parts: MoonParentParts) -> Result<Self, BuildMoonParentError> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(parts.mass.value()) {
            return Err(BuildMoonParentError::MassNotPositive);
        }
        if !positive(parts.radius.value()) {
            return Err(BuildMoonParentError::RadiusNotPositive);
        }
        if !positive(parts.host_mass.value()) {
            return Err(BuildMoonParentError::HostMassNotPositive);
        }
        let limit = parts.maximum_moon_mass.value();
        if !(limit.is_finite() && limit >= 0.0) {
            return Err(BuildMoonParentError::MoonMassLimitNotValid);
        }
        let r = parts.radius.value();
        let volume = 4.0 / 3.0 * core::f64::consts::PI * (r * r * r);
        let density = KilogramsPerCubicMetre::new(Kilograms::from(parts.mass).value() / volume);
        Ok(Self {
            id: parts.id,
            kind: parts.kind,
            mass: parts.mass,
            radius: parts.radius,
            density,
            class: parts.class,
            orbit: parts.orbit,
            formation_distance: parts.orbit.semi_major_axis(),
            host_mass: parts.host_mass,
            maximum_moon_mass: parts.maximum_moon_mass,
        })
    }

    /// The same parent, formed at `formation_distance` from its host rather than on its orbit: a
    /// giant that migrated in (P14.T8's formation distance). Its irregular moons are captured
    /// there, into its Hill sphere of then, and keep what fits after (ruling 112.4). A distance
    /// that is not positive and finite leaves the parent formed on its orbit.
    #[must_use]
    pub fn with_formation_distance(self, formation_distance: Metres) -> Self {
        let d = formation_distance.value();
        if d.is_finite() && d > 0.0 {
            Self {
                formation_distance,
                ..self
            }
        } else {
            self
        }
    }

    /// Where the parent formed: its orbit's semi-major axis unless
    /// [`with_formation_distance`](Self::with_formation_distance) said otherwise.
    #[must_use]
    pub const fn formation_distance(&self) -> Metres {
        self.formation_distance
    }

    /// The parent's circular Hill radius where it formed, `a_f` (m ÷ 3M)^⅓: the sphere its
    /// irregular moons were captured into (ruling 112.4).
    #[must_use]
    pub fn formation_hill_radius(&self) -> Metres {
        hill_radius(
            self.formation_distance,
            0.0,
            Kilograms::from(self.mass),
            self.host_mass,
        )
    }

    /// The parent `derived`, P14.T16.a's derivation of the body `id` at the epoch, a `kind`, on
    /// `orbit` about a host of mass `host_mass`.
    ///
    /// # Errors
    ///
    /// [`BuildMoonParentError::HostMassNotPositive`] for a host mass that is not positive and
    /// finite; a derived body's own values are always valid.
    pub fn from_derived(
        id: BodyId,
        kind: ParentKind,
        derived: &DerivedBody,
        orbit: KeplerElements,
        host_mass: Kilograms,
    ) -> Result<Self, BuildMoonParentError> {
        Self::new(MoonParentParts {
            id,
            kind,
            mass: derived.mass(),
            radius: Metres::from(derived.radius()),
            class: derived.class(),
            orbit,
            host_mass,
            maximum_moon_mass: derived.maximum_moon_mass(),
        })
    }

    /// The parent's ID.
    #[must_use]
    pub const fn id(&self) -> BodyId {
        self.id
    }

    /// A planet or a dwarf planet.
    #[must_use]
    pub const fn kind(&self) -> ParentKind {
        self.kind
    }

    /// The parent's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The parent's radius.
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }

    /// The parent's mean density, M ÷ (4π R³ ÷ 3).
    #[must_use]
    pub const fn density(&self) -> KilogramsPerCubicMetre {
        self.density
    }

    /// The parent's class by composition.
    #[must_use]
    pub const fn class(&self) -> PlanetClass {
        self.class
    }

    /// The parent's orbit about its host.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// The mass the parent orbits.
    #[must_use]
    pub const fn host_mass(&self) -> Kilograms {
        self.host_mass
    }

    /// The heaviest moon tides let survive about the parent (P14.T15).
    #[must_use]
    pub const fn maximum_moon_mass(&self) -> EarthMasses {
        self.maximum_moon_mass
    }

    /// The parent's circular Hill radius, a (m ÷ 3M)^⅓, in which the satellite stability limits
    /// are measured (P14.T15).
    #[must_use]
    pub fn hill_radius(&self) -> Metres {
        hill_radius(
            self.orbit.semi_major_axis(),
            0.0,
            Kilograms::from(self.mass),
            self.host_mass,
        )
    }

    /// The parent's Hill radius at its pericentre, a (1 − e) (m ÷ 3M)^⅓ (P14.T15's
    /// `hill_radius`), the bound of P14.T22.b's "moons inside Hill spheres".
    #[must_use]
    pub fn hill_radius_at_pericentre(&self) -> Metres {
        hill_radius(
            self.orbit.semi_major_axis(),
            self.orbit.eccentricity().value(),
            Kilograms::from(self.mass),
            self.host_mass,
        )
    }

    /// The largest semi-major axis at which a satellite of eccentricity `e_satellite` going round
    /// the parent in `sense` stays bound: Domingos, Winter and Yokoyama's (2006) fit, with the
    /// parent's own eccentricity (P14.T15, [`satellite_stability_limit`]).
    #[must_use]
    pub fn stability_limit(&self, e_satellite: f64, sense: OrbitSense) -> Metres {
        satellite_stability_limit(
            self.hill_radius(),
            self.orbit.eccentricity().value(),
            e_satellite,
            sense,
        )
    }

    /// The parent's fluid Roche limit for a satellite of density `satellite_density` (P14.T15).
    #[must_use]
    pub fn roche_limit_fluid(&self, satellite_density: KilogramsPerCubicMetre) -> Metres {
        roche_limit_fluid(self.radius, self.density, satellite_density)
    }

    /// The gravitational parameter of a moon of mass `moon` about the parent, G (`M_p` + m).
    #[must_use]
    pub fn mu_with(&self, moon: EarthMasses) -> GravitationalParameter {
        GravitationalParameter::from_earth_masses(self.mass + moon)
    }
}

/// A [`MoonParent`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildMoonParentError {
    /// The parent's mass was not positive and finite.
    MassNotPositive,
    /// The parent's radius was not positive and finite.
    RadiusNotPositive,
    /// The host's mass was not positive and finite.
    HostMassNotPositive,
    /// The maximum moon mass was negative or not finite.
    MoonMassLimitNotValid,
}

impl fmt::Display for BuildMoonParentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MassNotPositive => "a moon parent's mass must be positive and finite",
            Self::RadiusNotPositive => "a moon parent's radius must be positive and finite",
            Self::HostMassNotPositive => "a moon parent's host mass must be positive and finite",
            Self::MoonMassLimitNotValid => {
                "a moon parent's maximum moon mass must be finite and not negative"
            }
        })
    }
}

impl Error for BuildMoonParentError {}

/// The next word of `stream` as a rank strictly inside (0, 1).
#[must_use]
pub(crate) fn draw_rank(stream: &mut Stream) -> UnitUniform {
    UnitUniform::new(stream.uniform_open()).expect("an open uniform lies strictly between 0 and 1")
}

/// Whether the next word of `stream` decides for probability `p`: its mark lies below `p`'s
/// [`Threshold`], plan 01's integer-threshold decision (ruling 112.3). One word, as
/// [`draw_rank`] takes.
#[must_use]
pub(crate) fn decide(stream: &mut Stream, p: f64) -> bool {
    stream.decide(Threshold::from_probability(p))
}

/// A value log-uniform between `lo` and `hi` (both positive) at rank `rank`.
#[must_use]
pub(crate) fn log_uniform(lo: f64, hi: f64, rank: UnitUniform) -> f64 {
    lo * math::powf(hi / lo, rank.value())
}

/// A Rayleigh variate of scale `sigma` at rank `rank`, σ √(−2 ln(1 − u)).
#[must_use]
pub(crate) fn rayleigh(sigma: f64, rank: UnitUniform) -> f64 {
    sigma * (-2.0 * math::ln_1p(-rank.value())).sqrt()
}

/// The orbit about `parent` of a moon of mass `moon` at semi-major axis `a`, eccentricity `e`,
/// inclination `inclination` to the reference plane, and angles `node`, `periapsis` and
/// `mean_anomaly` at the epoch.
///
/// # Panics
///
/// If the values do not make an orbit: a caller here places every moon at a positive axis, a
/// bound eccentricity and an inclination in [0, π].
#[must_use]
pub(crate) fn moon_orbit(
    parent: &MoonParent,
    moon: EarthMasses,
    a: Metres,
    e: f64,
    inclination: Radians,
    angles: [Radians; 3],
) -> KeplerElements {
    let [node, periapsis, mean_anomaly] = angles;
    let orientation = Orientation::new(inclination, node, periapsis)
        .expect("a moon's inclination lies in [0, π] and its angles are finite");
    let e = Eccentricity::new(e).expect("a moon's eccentricity lies in [0, 1)");
    KeplerElements::from_semi_major_axis(a, parent.mu_with(moon), e, orientation, mean_anomaly)
        .expect("a moon's axis and its parent's mass are positive")
}

#[cfg(test)]
pub(crate) mod testing {
    //! Synthetic parents for the moons' tests: the Solar System's planets as [`MoonParent`]s.

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{Layer, SystemId};
    use crate::planetary::{BodyIndex, BodySlot, BodySub};
    use crate::units::consts::{
        EARTH_MASS_KG, GM_EARTH, GRAVITATIONAL_CONSTANT, METRES_PER_AU, SOLAR_MASS_KG,
    };

    /// A system ID for the tests' parents, `index` apart.
    pub(crate) fn system(index: u32) -> SystemId {
        SystemId::from_parts(
            Layer::A,
            GenCell::new(CellSize::Ly8, [3, -7, 1]).unwrap(),
            index,
        )
        .unwrap()
    }

    /// The planet in `slot` of the tests' system `index`.
    pub(crate) fn planet_id(index: u32, slot: u8) -> BodyId {
        BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary)
            .unwrap()
            .body_id(system(index))
    }

    /// A circular orbit of `a_au` about a Sun with eccentricity `e`.
    pub(crate) fn heliocentric(a_au: f64, e: f64, mass: EarthMasses) -> KeplerElements {
        let mu = GravitationalParameter::new(
            GRAVITATIONAL_CONSTANT * SOLAR_MASS_KG + GM_EARTH * mass.value(),
        );
        KeplerElements::from_semi_major_axis(
            Metres::new(a_au * METRES_PER_AU),
            mu,
            Eccentricity::new(e).unwrap(),
            Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
            Radians::ZERO,
        )
        .unwrap()
    }

    /// A parent of `mass_earths` and `radius_km`, of class `class`, at `a_au` and `e` about a
    /// Sun, with no limit on its moons' masses, as the body `id`.
    pub(crate) fn parent(
        id: BodyId,
        kind: ParentKind,
        mass_earths: f64,
        radius_km: f64,
        class: PlanetClass,
        a_au: f64,
        e: f64,
    ) -> MoonParent {
        let mass = EarthMasses::new(mass_earths);
        MoonParent::new(MoonParentParts {
            id,
            kind,
            mass,
            radius: Metres::new(radius_km * 1e3),
            class,
            orbit: heliocentric(a_au, e, mass),
            host_mass: Kilograms::new(SOLAR_MASS_KG),
            maximum_moon_mass: EarthMasses::new(f64::MAX / EARTH_MASS_KG),
        })
        .unwrap()
    }

    /// Jupiter: 317.83 M⊕, 69,911 km (volumetric mean), at 5.2029 au, e = 0.0484.
    pub(crate) fn jupiter(id: BodyId) -> MoonParent {
        parent(
            id,
            ParentKind::Planet,
            317.83,
            69_911.0,
            PlanetClass::GasGiant,
            5.2029,
            0.0484,
        )
    }

    /// Saturn: 95.16 M⊕, 58,232 km, at 9.5367 au, e = 0.0539.
    pub(crate) fn saturn(id: BodyId) -> MoonParent {
        parent(
            id,
            ParentKind::Planet,
            95.16,
            58_232.0,
            PlanetClass::GasGiant,
            9.5367,
            0.0539,
        )
    }

    /// Neptune: 17.15 M⊕, 24,622 km, at 30.0699 au, e = 0.0086.
    pub(crate) fn neptune(id: BodyId) -> MoonParent {
        parent(
            id,
            ParentKind::Planet,
            17.15,
            24_622.0,
            PlanetClass::IceGiant,
            30.0699,
            0.0086,
        )
    }

    /// Earth: 1 M⊕, 6,371 km, at 1.000 au, e = 0.0167.
    pub(crate) fn earth(id: BodyId) -> MoonParent {
        parent(
            id,
            ParentKind::Planet,
            1.0,
            6_371.0,
            PlanetClass::Rocky,
            1.000_002_61,
            0.0167,
        )
    }

    /// P14.T56's subtasks are held until the 22 → 23 batch's bump, so that the batch's goldens
    /// move once: until then `generate` refers every moon and ring to its parent's orbital plane,
    /// as version 22 does, the server sends no ring normal, and every moon is derived at the
    /// median ranks in its nursery, its inventory and atmosphere read at its composition solve's
    /// bulk. The bump removes the hold, and this test with it.
    #[test]
    fn the_moons_are_held_only_until_the_batch_s_bump() {
        use crate::GENERATOR_VERSION;
        use crate::planetary::moons::regular::MoonBulkRule;
        assert!(
            GENERATOR_VERSION < MOONS_VERSION,
            "the 22 → 23 bump puts every moon and ring on its parent's Laplace surface and gives \
             every moon its own volatiles, read at its own bulk, and its own tides: make \
             `generate` take `SatellitePlanes::LaplaceSurface` and the server send every ring's \
             normal (P14.T56.a as built), derive every moon with `Satellite::own_volatiles` and \
             `PlanetarySystem::moon_own_bulk` and drop `MoonInventories`, `MoonBulkRule`, \
             `PlanetarySystem::with_moon_inventories` and `derive_moon_under` (P14.T56.c and its \
             follow-up as built), derive every moon's tides by `MoonTides` and drop \
             `MoonTideRule`, `class_tides`, `MoonHolds`, `MoonRules` and \
             `PlanetarySystem::moon_tides`'s branches with `ICE_RESURFACING_HEAT_FLOW_AT_22` and \
             the `_under` arms of the figures' surface age (P14.T56.d as built), and remove \
             `SatellitePlanes::in_force`, `MOONS_VERSION` and this test (plan 14)"
        );
        assert_eq!(SatellitePlanes::in_force(), SatellitePlanes::OrbitalPlane);
        assert_eq!(MoonInventories::in_force(), MoonInventories::Shared);
        assert_eq!(MoonBulkRule::in_force(), MoonBulkRule::Solved);
        assert_eq!(MoonTideRule::in_force(), MoonTideRule::ClassPair);
        assert_eq!(
            MoonRules::in_force(),
            MoonRules {
                planes: SatellitePlanes::OrbitalPlane,
                inventories: MoonInventories::Shared,
                tides: MoonTideRule::ClassPair,
            }
        );
    }

    #[test]
    fn a_parent_s_density_hill_radius_and_limits_are_its_own() {
        let jupiter = jupiter(planet_id(0, 5));
        assert!((jupiter.density().value() - 1_326.0).abs() < 5.0);
        let hill = jupiter.hill_radius().value();
        assert!((hill / 5.31e10 - 1.0).abs() < 0.01, "{hill} m");
        let limit = jupiter.stability_limit(0.0, OrbitSense::Prograde).value();
        assert!((limit / hill - 0.4895 * (1.0 - 1.0305 * 0.0484)).abs() < 1e-12);
        let pericentre = jupiter.hill_radius_at_pericentre().value();
        assert!((pericentre / hill - (1.0 - 0.0484)).abs() < 1e-12);
    }

    #[test]
    fn a_parent_is_validated_once() {
        let good = MoonParentParts {
            id: planet_id(0, 1),
            kind: ParentKind::Planet,
            mass: EarthMasses::new(1.0),
            radius: Metres::new(6.371e6),
            class: PlanetClass::Rocky,
            orbit: heliocentric(1.0, 0.0, EarthMasses::new(1.0)),
            host_mass: Kilograms::new(SOLAR_MASS_KG),
            maximum_moon_mass: EarthMasses::ZERO,
        };
        assert!(MoonParent::new(good).is_ok());
        let cases = [
            (
                MoonParentParts {
                    mass: EarthMasses::new(0.0),
                    ..good
                },
                BuildMoonParentError::MassNotPositive,
            ),
            (
                MoonParentParts {
                    radius: Metres::new(f64::NAN),
                    ..good
                },
                BuildMoonParentError::RadiusNotPositive,
            ),
            (
                MoonParentParts {
                    host_mass: Kilograms::new(-1.0),
                    ..good
                },
                BuildMoonParentError::HostMassNotPositive,
            ),
            (
                MoonParentParts {
                    maximum_moon_mass: EarthMasses::new(-1.0),
                    ..good
                },
                BuildMoonParentError::MoonMassLimitNotValid,
            ),
        ];
        for (parts, error) in cases {
            assert_eq!(MoonParent::new(parts), Err(error));
            let text = error.to_string();
            assert_eq!(text, text.to_lowercase());
            assert!(!text.ends_with('.'));
        }
    }
}
