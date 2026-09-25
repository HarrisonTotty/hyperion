//! Moons: the regular satellites of giants, giant-impact moons and captured irregulars (plan 14,
//! P14.T17–T19; the brainstorm's "Moons (regular satellites scaled to the planet's mass, captured
//! irregulars, the occasional giant-impact moon)").
//!
//! Every draw here is on the parent's own streams, keyed by the parent's
//! [`BodyId`](crate::id::BodyId) (design note 4): a planet's moons can be generated alone, and
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
//! Every satellite orbit is a [`KeplerElements`] about its parent, with μ = G (`M_p` + m), in the
//! parent's body frame (plan 14, phase D): referred to the parent's equator for regular moons and
//! for a giant-impact moon, which forms in the equatorial plane of the debris disc (see
//! [`impact`]), and to its orbital plane for captures. The parent's equator is placed by its
//! obliquity and pole (P14.T14.a, T14.c), which are not built; until they are, the elements are
//! relative to that plane and nothing here needs its orientation.
//!
//! # Sub-indices
//!
//! Moons are numbered by P14.T22.a, which runs the three kinds for one planet and gives each moon
//! its sub-index of design note 3 (`0x01`–`0x7F`). Each kind here numbers its own moons from 1
//! inside out ([`regular::RegularMoon::ordinal`] and its siblings), and draws by that ordinal, so
//! that no kind's draws depend on another's count.

use std::error::Error;
use std::fmt;

use crate::id::BodyId;
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::planetary::derive::{
    DerivedBody, OrbitSense, PlanetClass, hill_radius, roche_limit_fluid, satellite_stability_limit,
};
use crate::rng::Stream;
use crate::stellar::draws::UnitUniform;
use crate::units::{
    EarthMasses, GravitationalParameter, Kilograms, KilogramsPerCubicMetre, Metres, Radians,
};

pub mod impact;
pub mod irregular;
pub mod regular;

pub use impact::{ImpactMoon, giant_impact_moon};
pub use irregular::{
    BeltAdjacency, CaptureKind, CapturedMoon, Captures, IrregularPopulation, NearestBelt, captures,
};
pub use regular::{DerivedMoon, RegularMoon, RegularMoons, Volcanism, regular_moons};

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
            host_mass: parts.host_mass,
            maximum_moon_mass: parts.maximum_moon_mass,
        })
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
