//! The assembled planetary generator: [`generate`], [`generate_planets`] and the
//! [`PlanetarySystem`] they return, with its queries at a time (plan 14, P14.T30).
//!
//! # Generation (P14.T30.a)
//!
//! A system is generated as it was born (design note 1), from its [`SystemContext`] alone, host by
//! host in hierarchy order and each host's planets inside out (design note 3):
//!
//! 1. **Zones** (P14.T9): the context's [stable zones](SystemContext::zones), the zones at birth,
//!    each an orbit host.
//! 2. **The disc** (P14.T3) of each zone, from its components' zero-age states and their own
//!    `star.disc_lifetime` ranks (ruling 33; a circumbinary disc its own rank on `planet.disc`),
//!    through [`ZoneDiscInputs::for_zone`]. It is cut to the zone's limits and, where the zone
//!    reaches further or has no outer limit, to the context's
//!    [strip radius](SystemContext::strip_radius), so that nothing is generated beyond 0.49 of the
//!    sphere of influence (design note 14). A field system's strip radius lies far beyond its
//!    discs, where the cut changes no bit of the disc, the class draw or the placement.
//! 3. **The class** (P14.T4) of each zone, drawn at its host number under the constraints of its
//!    disc, its reach and design note 10's close-binary flag
//!    ([`OrbitZone::host_multiplicity`]).
//! 4. **Placement** (P14.T8), with D5's second fallback, in planet slots that continue from the
//!    host before (design note 3): a host's slots are reserved by its drawn counts, so a planet's
//!    index never depends on anything placed after it. A circumbinary zone of a pair closer than
//!    47 au takes the pair's orbital plane ([`HostPlane::Aligned`]); every other host draws its
//!    own. No planet is ever in slot `0x00`, the stellar level of plan 11's components, brown
//!    dwarfs included.
//! 5. **Each planet's own draws**: its radius rank on [`tags::PLANET_RADIUS`] ([`radius_rank`],
//!    design note 8), its formation on `planet.origin` from its disc's lifetime (P14.T28.a), and its
//!    circularisation time (P14.T8.e, [`circularisation_time`]).
//!
//! [`generate_planets`] stops there. [`generate`] goes on (phase D), each stage on its own streams
//! (design note 4) and in its own slots (design note 3), so that none moves a planet:
//!
//! 6. **Belts** (P14.T21.a–c) of each host with a disc, host by host in the zones' order, in the
//!    belt slots from `0xE1` ([`host_belts`]), with their largest members as dwarf planets.
//! 7. **Satellites** (P14.T22.a, [`generate_satellites`](crate::planetary::satellites::generate_satellites))
//!    of every planet, beside its nearest belt ([`PlanetarySystem::nearest_belt`]), and each
//!    belt member's giant-impact moon. The belts are placed first because a rocky planet's
//!    captures read them (P14.T19); the plan's order, satellites before belts, is the order of
//!    their slots, and neither reads the other's streams.
//! 8. **The cometary halo** (P14.T21.d) about the zone holding the primary with the most
//!    members, if a planet over 10 M⊕ beyond its host's snow line scatters one.
//!
//! Second-generation planets (P14.T28.e) and the free-floating hosts of P14.T27 are not built:
//! their stages do not exist yet, and their streams (`planet.secondgen`) and slots (`0xC0`–`0xCF`,
//! the stellar level of a free-floating system) stay reserved for them. Nor is P14.T28.a's
//! protoplanetary disc (belt slot `0xE0`) or P14.T28.d's debris disc (`0xEE`).
//!
//! [`PlanetarySystem`] holds only that primordial state, which is what the server may cache ("state
//! at the epoch, never positions at some time").
//!
//! # Queries at a time (P14.T30.b)
//!
//! Everything that depends on time is evaluated lazily from the primordial state and the context,
//! so every query is a pure function of seed, ID and time, and agrees with every other:
//!
//! - [`PlanetarySystem::body_at`]: the fate transform (P14.T28, [`BodyFate`]) gives the body's
//!   state and orbit at the time; a present body is then derived (P14.T16.a, [`derive_body`];
//!   a moon by P14.T17.b) about its hosts' states at the time, and the result is a [`BodyRecord`]
//!   (P14.T34) with its label (P14.T30.c). A moon or a ring goes with its planet: not yet formed,
//!   destroyed or unbound when it is; a giant-impact moon may be lost first (P14.T18). A belt's
//!   member has its own history, alone about its host, so that it can never move a planet. A
//!   belt's edges widen as its host's orbits do (design note 11), and its mass wears down; the
//!   halo loses what its host's mass loss unbinds (ruling 84.4).
//! - [`PlanetarySystem::snapshot_at`]: every body's record, in index order, with the system's belts
//!   and halo.
//! - [`PlanetarySystem::position_at`]: the host's position from plan 11's hierarchy, the walk of
//!   [`star_positions_at`](crate::stellar::multiplicity::star_positions_at) (bit for bit for a
//!   star, and a pair's barycentre on the same walk), plus the body's Kepler state about its host;
//!   a moon's is its planet's plus its own offset, and a ring's its planet's.
//! - [`PlanetarySystem::habitable_zone_at`]: Kopparapu et al.'s zone of a host's stars at the
//!   time, with the light of the system's other stars (P14.T12.b).
//!
//! What is not computed is said in every record rather than left to be read as "none" (ruling
//! 34): a record's `surface` and `hooks` are [`Section::NotModelled`] (P14.T13, T14, T23–T26), a
//! giant's surface [`Section::NotApplicable`], and a dwarf planet's rings `NotModelled`. Events on
//! bodies (P14.T31) are not built, so there is no `events_between`.
//!
//! # Streams
//!
//! [`tags::PLANET_RADIUS`], opened with the body's [`BodyId`]: word 0 is its radius rank, and
//! words 1–7 are reserved ([`RADIUS_WORDS`]). A planet and a moon read it; a belt member reads its
//! rank on `belt.member`. Every other draw is its stage's.

use crate::Seed;
use crate::coords::SystemPosition;
use crate::id::{BodyId, SystemId};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::planetary::architecture::{
    ArchitectureClass, ClassConstraints, ZoneLimit, class_weights, draw_class,
};
use crate::planetary::belts::{
    Belt, BeltHost, BeltMember, FIRST_BELT_SLOT, LAST_BELT_SLOT, host_belts,
};
use crate::planetary::context::SystemContext;
use crate::planetary::derive::{
    BodyHosts, DerivedBody, HabitableZone, HostLight, Illumination, MassFractions, PlacedBody,
    PlanetClass, derive_body, habitable_zone_of, radius_chen_kipping,
};
use crate::planetary::disc::{self, Disc, DiscProfile, Truncation};
use crate::planetary::error::ResolveBodyError;
use crate::planetary::fate::{BodyFate, BodyState, FateBody, FateHost, ScatterDraws};
use crate::planetary::halo::{CometaryHalo, HaloHost, Scatterer, halo};
use crate::planetary::hosts::evolved::Circularisation;
use crate::planetary::hosts::young::{Formation, FormationDraws};
use crate::planetary::index::{BodyIndex, LAST_PLANET_SLOT};
use crate::planetary::label::{self, BodyLabel};
use crate::planetary::moons::regular::{
    MOON_ICE_DENSITY, MOON_ROCK_DENSITY, MoonNursery, MoonSky, derive_moon,
};
use crate::planetary::moons::{BeltAdjacency, MoonParent, NearestBelt, ParentKind};
use crate::planetary::params::SPACING_GIANT_MASS;
use crate::planetary::placement::classes::orbits::{HostPlane, SystemPlane, host_plane};
use crate::planetary::placement::classes::tides::{
    GIANT_TIDAL_Q_PRIME, ROCKY_TIDAL_Q_PRIME, circularisation_time,
};
use crate::planetary::placement::zones::CLOSE_BINARY_SEMI_MAJOR_AXIS;
use crate::planetary::placement::{
    Neighbour, OrbitHost, OrbitZone, PlacedPlanet, PlacementHost, ZoneDiscInputs, core_fallback,
    place,
};
use crate::planetary::record::{
    BeltRecord, BodyIdentity, BodyKind, BodyOrbit, BodyRecord, BulkProperties, HaloRecord,
    Population, Section, SystemSnapshot,
};
use crate::planetary::rings::{Ring, RingParent};
use crate::planetary::satellites::{Satellite, SatelliteMoon, Satellites, satellites_of};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::stellar::multiplicity::{HierarchyNode, SystemHierarchy};
use crate::stellar::system::StarModel;
use crate::stellar::{Phase, StarState};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::{GM_EARTH, SECONDS_PER_JULIAN_YEAR};
use crate::units::{
    EarthMasses, EarthRadii, GravitationalParameter, Kilograms, KilogramsPerCubicMetre, Megayears,
    Metres, MetresPerSecondSquared, Radians, SolarLuminosities, SolarMasses, SolarMassesPerYear,
    Years,
};

/// Words of [`tags::PLANET_RADIUS`] a planet reads or reserves: word 0, its radius rank, then
/// seven reserved.
pub const RADIUS_WORDS: u64 = 8;

/// The first planet slot of a system: slot 1, since slot `0x00` is the stellar level (design
/// note 3).
const FIRST_PLANET_SLOT: u8 = 1;

/// The radius rank of the planet `body` in the universe of `seed`: word 0 of its
/// [`tags::PLANET_RADIUS`] stream, the one uniform that places it within Chen and Kipping's
/// scatter at its mass, or for a rocky outcome within the observed spread of core fractions
/// (design note 8, ruling 53; P14.T30.a opens the tag, as ruling 53 says).
///
/// # Panics
///
/// Never: an open uniform lies strictly between 0 and 1.
///
/// # Examples
///
/// Each planet has a stream of its own, so its rank depends on nothing else in its system:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::system::radius_rank;
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let (b, c) = (BodyId::new(system, 0x0100), BodyId::new(system, 0x0200));
/// let seed = Seed::new(7);
/// assert_eq!(radius_rank(seed, b), radius_rank(seed, b));
/// assert_ne!(radius_rank(seed, b), radius_rank(seed, c));
/// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
/// ```
#[must_use]
pub fn radius_rank(seed: Seed, body: BodyId) -> UnitUniform {
    let mut stream = Stream::open(seed, tags::PLANET_RADIUS, ObjectKey::from(body));
    UnitUniform::new(stream.uniform_open()).expect("an open uniform lies strictly between 0 and 1")
}

/// One orbit host of a generated system: its limits, disc, plane and class (P14.T30.a).
#[derive(Debug, Clone, PartialEq)]
pub struct PlanetaryHost {
    host: OrbitHost,
    limits: Truncation,
    disc: Disc,
    plane: SystemPlane,
    drawn_class: ArchitectureClass,
    class: ArchitectureClass,
}

impl PlanetaryHost {
    /// The host: its zone's ([`OrbitZone::host`]).
    #[must_use]
    pub const fn host(&self) -> OrbitHost {
        self.host
    }

    /// The limits its disc and planets were held to: its zone's, and the strip radius wherever
    /// the zone reaches further or has no outer limit (design note 14).
    #[must_use]
    pub const fn limits(&self) -> Truncation {
        self.limits
    }

    /// Its disc at birth (P14.T3), cut to [`limits`](Self::limits).
    #[must_use]
    pub const fn disc(&self) -> &Disc {
        &self.disc
    }

    /// The plane its planets were placed about (P14.T8.d, design note 21): a close pair's own for
    /// its circumbinary zone, and otherwise the host's isotropic draw on `planet.plane`.
    #[must_use]
    pub const fn plane(&self) -> SystemPlane {
        self.plane
    }

    /// Its class as drawn (P14.T4.c).
    #[must_use]
    pub const fn drawn_class(&self) -> ArchitectureClass {
        self.drawn_class
    }

    /// Its class as placed: the drawn one, or its giant-free sibling where the disc grows no
    /// giant's core in time (D5's second fallback), or `Barren` without a disc.
    #[must_use]
    pub const fn class(&self) -> ArchitectureClass {
        self.class
    }
}

/// One body of a generated system, as it was born (design note 1): a planet as placed (P14.T8), a
/// moon or a ring of one (P14.T17–T20, T22.a), or a belt's named member (P14.T21.c), with what it
/// orbits and its own draws.
///
/// Belts and the cometary halo are populations, held apart ([`PlanetarySystem::belts`],
/// [`PlanetarySystem::halo`]), though each is a body of the system's records.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    index: BodyIndex,
    host: OrbitHost,
    part: Part,
}

/// What a [`Body`] is, with the primordial state of its kind.
#[derive(Debug, Clone, PartialEq)]
enum Part {
    /// A planet.
    Planet(PlanetPart),
    /// A moon of a planet or of a belt's member.
    Moon(MoonPart),
    /// A ring of a planet.
    Ring(Ring),
    /// A belt's named member.
    Member(MemberPart),
}

/// A planet's primordial state.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PlanetPart {
    placed: PlacedPlanet,
    radius_rank: UnitUniform,
    fate: FateBody,
}

/// A moon's primordial state: the moon, its parent as the moon generators read it, and its orbit
/// as it formed in the parent's body frame.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MoonPart {
    satellite: Satellite,
    parent: MoonParent,
    orbit: KeplerElements,
}

/// A belt member's primordial state: the member, its belt and what the fate transform reads.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MemberPart {
    member: BeltMember,
    belt: BodyIndex,
    fate: FateBody,
}

impl Body {
    /// The planet `placed` of the host `zone` of `system`, in the universe of `seed`, whose disc
    /// lives `disc_lifetime`: its radius rank, its formation and its circularisation.
    #[must_use]
    fn planet(
        seed: Seed,
        system: SystemId,
        zone: &OrbitZone,
        placed: PlacedPlanet,
        disc_lifetime: Megayears,
    ) -> Self {
        let id = placed.index().body_id(system);
        let mass = placed.mass();
        let formation = Formation::draw(seed, id, mass, disc_lifetime)
            .expect("a placed planet's mass and its disc's lifetime are positive");
        let radius = primordial_radius(mass);
        let fate = FateBody::new(
            formation,
            *placed.orbit(),
            mass,
            sphere_density(mass, radius),
        )
        .expect("a placed planet's mass and its density are positive")
        .with_circularisation(circularisation(&placed, zone.host_mass(), radius))
        .with_scatter_draws(ScatterDraws::Stream { seed, body: id });
        Self {
            index: placed.index(),
            host: zone.host(),
            part: Part::Planet(PlanetPart {
                placed,
                radius_rank: radius_rank(seed, id),
                fate,
            }),
        }
    }

    /// The member `member` of the belt `belt`, whose host's disc lives `disc_lifetime`: a small
    /// body, formed at the disc's lifetime (P14.T28.a) with no draw of its own for it.
    #[must_use]
    fn member(belt: &Belt, member: BeltMember, disc_lifetime: Megayears) -> Self {
        let formation =
            Formation::from_draws(member.mass(), disc_lifetime, &FormationDraws::MEDIAN)
                .expect("a member's mass and its disc's lifetime are positive");
        let fate = FateBody::new(
            formation,
            *member.orbit(),
            member.mass(),
            belt.composition().member_density(),
        )
        .expect("a member's mass and density are positive");
        Self {
            index: member.index(),
            host: belt.host(),
            part: Part::Member(MemberPart {
                member,
                belt: belt.index(),
                fate,
            }),
        }
    }

    /// The moon `satellite` of `parent`.
    #[must_use]
    fn moon(satellite: Satellite, parent: &MoonParent, parent_index: BodyIndex) -> Self {
        Self {
            index: satellite.index(),
            host: OrbitHost::Body(parent_index),
            part: Part::Moon(MoonPart {
                satellite,
                parent: *parent,
                orbit: satellite.orbit_at(parent, Years::new(0.0)),
            }),
        }
    }

    /// The ring `ring` of the planet `planet`.
    #[must_use]
    fn ring(ring: Ring, planet: BodyIndex) -> Self {
        Self {
            index: ring.index(),
            host: OrbitHost::Body(planet),
            part: Part::Ring(ring),
        }
    }

    /// The body's index in its system.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// What the body is: a planet, a moon by its origin, a ring, or a belt's member, which is a
    /// dwarf planet.
    #[must_use]
    pub const fn kind(&self) -> BodyKind {
        match &self.part {
            Part::Planet(_) => BodyKind::Planet,
            Part::Moon(moon) => BodyKind::Moon(moon.satellite.origin()),
            Part::Ring(_) => BodyKind::Ring,
            Part::Member(_) => BodyKind::DwarfPlanet,
        }
    }

    /// What the body orbits: its zone's host for a planet or a belt's member, and its parent body
    /// for a moon or a ring ([`OrbitHost::Body`]).
    #[must_use]
    pub const fn host(&self) -> OrbitHost {
        self.host
    }

    /// What the body's record names as its parent (P14.T34): its host, but a belt's member's belt,
    /// under which it is listed.
    #[must_use]
    pub const fn parent(&self) -> OrbitHost {
        match &self.part {
            Part::Member(member) => OrbitHost::Body(member.belt),
            Part::Planet(_) | Part::Moon(_) | Part::Ring(_) => self.host,
        }
    }

    /// A planet as placed (P14.T8): its group, role, origin and the rest; `None` for any other
    /// body.
    #[must_use]
    pub const fn placed(&self) -> Option<&PlacedPlanet> {
        match &self.part {
            Part::Planet(planet) => Some(&planet.placed),
            Part::Moon(_) | Part::Ring(_) | Part::Member(_) => None,
        }
    }

    /// The moon, if the body is one.
    #[must_use]
    pub const fn satellite(&self) -> Option<&Satellite> {
        match &self.part {
            Part::Moon(moon) => Some(&moon.satellite),
            Part::Planet(_) | Part::Ring(_) | Part::Member(_) => None,
        }
    }

    /// The ring, if the body is one.
    #[must_use]
    pub const fn ring_of(&self) -> Option<&Ring> {
        match &self.part {
            Part::Ring(ring) => Some(ring),
            Part::Planet(_) | Part::Moon(_) | Part::Member(_) => None,
        }
    }

    /// The belt member, if the body is one.
    #[must_use]
    pub const fn member_of(&self) -> Option<&BeltMember> {
        match &self.part {
            Part::Member(member) => Some(&member.member),
            Part::Planet(_) | Part::Moon(_) | Part::Ring(_) => None,
        }
    }

    /// The body's mass: a planet's as placed, a moon's, a ring's, a member's.
    #[must_use]
    pub fn mass(&self) -> EarthMasses {
        match &self.part {
            Part::Planet(planet) => planet.placed.mass(),
            Part::Moon(moon) => moon.satellite.mass(),
            Part::Ring(ring) => EarthMasses::from(ring.mass()),
            Part::Member(member) => member.member.mass(),
        }
    }

    /// The body's primordial orbit: a planet's or a member's about its host in the system frame,
    /// with μ = G (M + m) at the host's initial mass, and a moon's about its parent in the parent's
    /// body frame, as it formed; `None` for a ring, which orbits as a sheet.
    #[must_use]
    pub const fn orbit(&self) -> Option<&KeplerElements> {
        match &self.part {
            Part::Planet(planet) => Some(planet.placed.orbit()),
            Part::Moon(moon) => Some(&moon.orbit),
            Part::Member(member) => Some(member.member.orbit()),
            Part::Ring(_) => None,
        }
    }

    /// The body's radius rank (design note 8): a planet's or a moon's, drawn on its own
    /// `planet.radius` stream ([`radius_rank`]), and a member's on `belt.member`; `None` for a
    /// ring.
    #[must_use]
    pub const fn radius_rank(&self) -> Option<UnitUniform> {
        match &self.part {
            Part::Planet(planet) => Some(planet.radius_rank),
            Part::Moon(moon) => Some(moon.satellite.radius_rank()),
            Part::Member(member) => Some(member.member.radius_rank()),
            Part::Ring(_) => None,
        }
    }

    /// When the body forms (P14.T28.a): a planet or a member; a moon and a ring go with their
    /// parent.
    #[must_use]
    pub const fn formation(&self) -> Option<&Formation> {
        match self.fate() {
            Some(fate) => Some(fate.formation()),
            None => None,
        }
    }

    /// How a planet's orbit circularises (P14.T8.e).
    #[must_use]
    pub const fn circularisation(&self) -> Option<Circularisation> {
        match &self.part {
            Part::Planet(planet) => Some(planet.fate.circularisation()),
            Part::Moon(_) | Part::Ring(_) | Part::Member(_) => None,
        }
    }

    /// What the fate transform reads of a planet or a member (P14.T28): its formation, primordial
    /// orbit, mass, primordial density and circularisation.
    #[must_use]
    pub const fn fate(&self) -> Option<&FateBody> {
        match &self.part {
            Part::Planet(planet) => Some(&planet.fate),
            Part::Member(member) => Some(&member.fate),
            Part::Moon(_) | Part::Ring(_) => None,
        }
    }

    /// The bytes the body owns on the heap: a ring's gaps.
    #[must_use]
    fn heap_bytes(&self) -> usize {
        match &self.part {
            Part::Ring(ring) => ring.heap_bytes(),
            Part::Planet(_) | Part::Moon(_) | Part::Member(_) => 0,
        }
    }
}

/// The density of a sphere of `mass` and `radius`.
#[must_use]
fn sphere_density(mass: EarthMasses, radius: Metres) -> KilogramsPerCubicMetre {
    let r = radius.value();
    let volume = 4.0 / 3.0 * core::f64::consts::PI * (r * r * r);
    KilogramsPerCubicMetre::new(Kilograms::from(mass).value() / volume)
}

/// The radius a planet's dynamics read at birth, before any derivation at a time: Chen and
/// Kipping's median at its mass (P14.T11.a), the radius the placer's Roche limit takes (P14.T8).
///
/// The derived radius depends on the age and the irradiation, and the fate transform needs one
/// fixed at birth: for the circularisation time (τ ∝ R⁻⁵) and the density of the Roche test after
/// a supernova.
#[must_use]
fn primordial_radius(mass: EarthMasses) -> Metres {
    Metres::from(radius_chen_kipping(mass, UnitUniform::HALF))
}

/// The circularisation of `planet`, of radius `radius`, about a host of initial mass `host`:
/// P14.T8.e's damping time at its primordial orbit, with a giant's modified tidal quality factor
/// from [`SPACING_GIANT_MASS`] (design note 7's giant) and a rocky planet's below (ruling 62.5).
#[must_use]
fn circularisation(planet: &PlacedPlanet, host: SolarMasses, radius: Metres) -> Circularisation {
    let q_prime = if planet.mass() >= EarthMasses::from(SPACING_GIANT_MASS) {
        GIANT_TIDAL_Q_PRIME
    } else {
        ROCKY_TIDAL_Q_PRIME
    };
    let orbit = planet.orbit();
    let tau = circularisation_time(
        planet.mass(),
        radius,
        host,
        orbit.semi_major_axis(),
        orbit.period(),
        q_prime,
    );
    Circularisation::new(tau).expect("a damping time of positive quantities is positive")
}

/// A system's planetary bodies as they were born, per orbit host (plan 14's `PlanetarySystem`,
/// P14.T30.a), and the queries that evaluate them at a time (P14.T30.b).
///
/// It holds primordial state only (design note 1), and every query takes the
/// [`SystemContext`] it was generated from, which is what a cache holds beside it.
///
/// # Examples
///
/// A Sun-like star's planets, and one of them at the epoch:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::placement::CellKey;
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::planetary::fate::BodyState;
/// use hyperion_sim::planetary::placement::OrbitHost;
/// use hyperion_sim::planetary::system::generate;
/// use hyperion_sim::planetary::SystemContext;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{Dex, SolarMasses, Years};
///
/// let id = CellKey::new(Layer::C, [0, 812, 0])?.candidate_id(3).ok_or("inside the cell")?;
/// let sun = SystemContext::builder()
///     .system(id)
///     .star(SolarMasses::new(1.0))
///     .fe_h(Dex::ZERO)
///     .age_at_epoch(Years::new(4.57e9))
///     .build()?;
/// let seed = Seed::new(11);
/// let system = generate(seed, &sun);
/// assert_eq!(system.zones().len(), 1);
/// assert!(system.disc(OrbitHost::Star(0)).is_some());
/// for body in system.bodies() {
///     let record = system.body_at(&sun, body.index(), UniverseTime::EPOCH)?;
///     // A Sun's planets and their moons have long formed, and none has been engulfed yet.
///     assert_eq!(record.identity().state(), BodyState::Present);
///     assert!(record.position().is_some());
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PlanetarySystem {
    system: SystemId,
    zones: Vec<OrbitZone>,
    hosts: Vec<PlanetaryHost>,
    bodies: Vec<Body>,
    satellites: Vec<Satellites>,
    belts: Vec<Belt>,
    halo: Option<SystemHalo>,
}

/// A system's cometary halo and the orbit host it surrounds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SystemHalo {
    halo: CometaryHalo,
    host: OrbitHost,
}

/// The system of `ctx` in the universe of `seed`, whole (P14.T30.a): its planets
/// ([`generate_planets`]), then its belts with their members, every planet's and member's
/// satellites, and its cometary halo (the [module](self) documentation, steps 6–8).
///
/// Each stage after the planets has its own streams and slots, so a system's planets are exactly
/// [`generate_planets`]'s.
///
/// # Panics
///
/// As [`generate_planets`].
#[must_use]
pub fn generate(seed: Seed, ctx: &SystemContext) -> PlanetarySystem {
    let mut system = generate_planets(seed, ctx);
    let id = ctx.id();

    // Step 6: belts, host by host.
    let mut slot = FIRST_BELT_SLOT;
    let mut scatterer = Scatterer::Absent;
    let mut belts = Vec::new();
    for (zone, host) in system.zones.iter().zip(&system.hosts) {
        let Some(profile) = host.disc.profile() else {
            continue;
        };
        let neighbours = system.neighbours(zone.host());
        scatterer = scatterer.or(Scatterer::among(&neighbours, profile.snow_line()));
        if slot > LAST_BELT_SLOT {
            continue;
        }
        let found = host_belts(
            seed,
            id,
            &BeltHost::new(
                zone.host(),
                profile,
                &neighbours,
                host.plane,
                ctx.age_at_epoch(),
            ),
            slot,
        );
        slot = found.next_slot();
        belts.extend(found.into_belts());
    }
    system.belts = belts;

    // Belt members, each a dwarf planet about its belt's host.
    let members: Vec<Body> = system
        .belts
        .iter()
        .flat_map(|belt| {
            let lifetime = system
                .host(belt.host())
                .and_then(|host| host.disc.profile())
                .map(DiscProfile::lifetime)
                .expect("a belt's host has a disc");
            belt.members()
                .iter()
                .map(move |&member| Body::member(belt, member, lifetime))
        })
        .collect();

    // Step 7: satellites of every planet and every member.
    let mut satellites = Vec::new();
    for parent in system.bodies.iter().chain(&members) {
        let zone = system.zone_of(parent);
        let profile = system
            .host(zone.host())
            .and_then(|host| host.disc.profile())
            .expect("a host with bodies has a disc");
        let belt = match parent.part {
            Part::Planet(_) => system.nearest_belt(parent.index()),
            Part::Moon(_) | Part::Ring(_) | Part::Member(_) => None,
        };
        let found = parent_at_epoch(ctx, zone, profile, parent).map_or_else(
            || Satellites::none(parent.index()),
            |(moon_parent, ring_parent)| {
                satellites_of(
                    seed,
                    parent.index(),
                    &moon_parent,
                    ring_parent.as_ref(),
                    belt,
                    ctx.age_at_epoch(),
                )
            },
        );
        satellites.push(found);
    }

    // Step 8: the cometary halo.
    system.halo = system_halo(seed, ctx, &system, scatterer);

    system.bodies.extend(members);
    for found in &satellites {
        if let Some(parent) = found.parent() {
            system.bodies.extend(
                found
                    .moons()
                    .iter()
                    .map(|&moon| Body::moon(moon, parent, found.parent_index())),
            );
        }
        system.bodies.extend(
            found
                .rings()
                .iter()
                .map(|ring| Body::ring(ring.clone(), found.parent_index())),
        );
    }
    system.satellites = satellites;
    system.bodies.sort_by_key(Body::index);
    system
}

/// The planets of the system of `ctx` in the universe of `seed`, with no satellites, belts or halo
/// (P14.T30.a): zones, then per zone its disc, class and placement, then each planet's own draws
/// (the [module](self) documentation, steps 1–5).
///
/// The result is a pure function of `seed` and `ctx`, whatever was generated before.
///
/// # Panics
///
/// Never for a context [`SystemContext`] builds, whose components all have valid zero-age states.
#[must_use]
pub fn generate_planets(seed: Seed, ctx: &SystemContext) -> PlanetarySystem {
    let id = ctx.id();
    let hierarchy = ctx.hierarchy();
    let under = members_under(hierarchy);
    let stars = ctx.zone_stars();
    let strip = ctx.strip_radius();
    let zones = ctx.zones();
    let mut hosts = Vec::with_capacity(zones.len());
    let mut bodies = Vec::new();
    let mut slot = FIRST_PLANET_SLOT;
    for zone in &zones {
        let inputs = ZoneDiscInputs::for_zone(seed, id, zone, &stars, ctx.fe_h())
            .expect("a context gives every component a positive, finite zero-age state");
        let limits = within(zone.truncation(), strip);
        let disc = disc::derive(inputs.host(), inputs.lifetime(), inputs.draws(), limits);
        let reach = limits
            .outer()
            .map_or(ZoneLimit::Unbounded, ZoneLimit::Outer);
        let constraints = ClassConstraints::new(&disc, reach, zone.host_multiplicity());
        let weights = class_weights(zone.host_mass(), ctx.fe_h());
        let drawn = draw_class(seed, id, zone.host_number(), &weights, &constraints);
        let plane = plane_of(hierarchy, &under, zone);
        let class = if slot > LAST_PLANET_SLOT {
            // Every primordial slot is taken: the host keeps its disc and class and places
            // nothing, rather than spilling into the second-generation block.
            if disc.profile().is_some() {
                core_fallback(drawn, &disc, reach).0
            } else {
                ArchitectureClass::Barren
            }
        } else {
            let host = PlacementHost::new(zone.host_number(), *inputs.host(), plane);
            let placement = place(seed, id, &host, limits, &disc, drawn, slot);
            slot = placement.next_slot();
            bodies.extend(
                placement
                    .planets()
                    .iter()
                    .map(|&planet| Body::planet(seed, id, zone, planet, inputs.lifetime())),
            );
            placement.class()
        };
        hosts.push(PlanetaryHost {
            host: zone.host(),
            limits,
            disc,
            plane: host_plane(seed, id, zone.host_number(), plane),
            drawn_class: drawn,
            class,
        });
    }
    bodies.sort_by_key(Body::index);
    PlanetarySystem {
        system: id,
        zones,
        hosts,
        bodies,
        satellites: Vec::new(),
        belts: Vec::new(),
        halo: None,
    }
}

/// When the moon generators read a parent (see the `satellites` module): the epoch, or for a
/// system not yet born at the epoch the end of the clock window.
#[must_use]
fn parent_time(ctx: &SystemContext) -> UniverseTime {
    if ctx.age_at_epoch().value() > 0.0 {
        UniverseTime::EPOCH
    } else {
        ClockWindow::END
    }
}

/// The body `body` of `zone`, whose disc is `profile`, as the moon generators read it
/// (P14.T22.a), and for a planet as its rings read it: P14.T16.a's derivation at
/// [`parent_time`], on its primordial orbit about its zone's initial mass. `None` for a moon or a
/// ring, and for a body whose system is not born even then.
#[must_use]
fn parent_at_epoch(
    ctx: &SystemContext,
    zone: &OrbitZone,
    profile: &DiscProfile,
    body: &Body,
) -> Option<(MoonParent, Option<RingParent>)> {
    let (placed, kind) = match &body.part {
        Part::Planet(planet) => (
            PlacedBody::new(
                planet.placed.mass(),
                *planet.placed.orbit(),
                planet.placed.formation_distance(),
                planet.radius_rank,
            )
            .expect("a placed planet's mass and formation distance are positive"),
            ParentKind::Planet,
        ),
        Part::Member(member) => (member.member.placed_body(), ParentKind::DwarfPlanet),
        Part::Moon(_) | Part::Ring(_) => return None,
    };
    let t = parent_time(ctx);
    let epoch = Epoch::of(ctx, t);
    let orbited = epoch.lights(zone)?;
    let companions = epoch.companions(zone)?;
    let host_mass = Kilograms::from(zone.host_mass());
    let hosts = BodyHosts::new(host_mass, *ctx.composition(), &orbited, &companions).ok()?;
    let derived = derive_body(&placed, &hosts, profile, ctx.age_at_epoch(), t).ok()?;
    let id = body.index.body_id(ctx.id());
    let parent = MoonParent::from_derived(id, kind, &derived, *placed.orbit(), host_mass).ok()?;
    let rings = match kind {
        ParentKind::Planet => RingParent::new(
            body.index,
            derived.class(),
            Kilograms::from(derived.mass()),
            Metres::from(derived.radius()),
            derived.equilibrium_temperature(),
        )
        .ok(),
        ParentKind::DwarfPlanet => None,
    };
    Some((parent, rings))
}

/// The body `body` of the system of `ctx`, in the universe of `seed`, as the moon generators read
/// it, and its rings: [`parent_at_epoch`] about its zone and its disc, rebuilt from the context
/// as [`generate_planets`] builds them, for
/// [`generate_satellites`](crate::planetary::satellites::generate_satellites).
///
/// # Panics
///
/// If `body`'s host is not one of the context's zones.
#[must_use]
pub(crate) fn satellite_parent(
    seed: Seed,
    ctx: &SystemContext,
    body: &Body,
) -> Option<(MoonParent, Option<RingParent>)> {
    let zones = ctx.zones();
    let zone = zones
        .iter()
        .find(|zone| zone.host() == body.host)
        .expect("a body's host is one of its context's zones");
    let stars = ctx.zone_stars();
    let inputs = ZoneDiscInputs::for_zone(seed, ctx.id(), zone, &stars, ctx.fe_h())
        .expect("a context gives every component a positive, finite zero-age state");
    let limits = within(zone.truncation(), ctx.strip_radius());
    let disc = disc::derive(inputs.host(), inputs.lifetime(), inputs.draws(), limits);
    parent_at_epoch(ctx, zone, disc.profile()?, body)
}

/// The system's cometary halo (P14.T21.d), if its planets scatter one: about the zone holding
/// the primary with the most members, of its zero-age mass and its disc's solids, cut at the
/// strip radius and a third of the pericentre of the nearest companion outside it.
#[must_use]
fn system_halo(
    seed: Seed,
    ctx: &SystemContext,
    system: &PlanetarySystem,
    scatterer: Scatterer,
) -> Option<SystemHalo> {
    // The innermost of the zones holding the primary with the most members.
    let mut best: Option<(&OrbitZone, &PlanetaryHost)> = None;
    for (zone, host) in system.zones.iter().zip(&system.hosts) {
        if zone.members().any(|member| member == 0)
            && best.is_none_or(|(held, _)| zone.members().count() > held.members().count())
        {
            best = Some((zone, host));
        }
    }
    let (zone, host) = best?;
    let h = ctx.hierarchy();
    let under = members_under(h);
    let members = members_of(zone);
    let companion = h
        .pairs()
        .filter(|(node, _)| {
            let below = under[usize::from(node.get())];
            below & members == members && below != members
        })
        .min_by_key(|(node, _)| under[usize::from(node.get())].count_ones())
        .map(|(_, orbit)| orbit.periapsis());
    let halo_host = HaloHost::new(
        zone.host_mass(),
        host.disc.solid_mass(),
        ctx.strip_radius(),
        companion,
    );
    halo(seed, system.system, &halo_host, scatterer).map(|halo| SystemHalo {
        halo,
        host: zone.host(),
    })
}

/// `limits` held inside `strip` as well: its outer limit, or `strip` where that is nearer or
/// there is none (design note 14).
#[must_use]
fn within(limits: Truncation, strip: Metres) -> Truncation {
    match limits.outer() {
        Some(outer) if outer <= strip => limits,
        Some(_) | None => limits.with_outer(strip),
    }
}

/// The components under each node of `h`, a bit per body index, by node index.
#[must_use]
fn members_under(h: &SystemHierarchy) -> Vec<u32> {
    let mut under = vec![0_u32; h.nodes().len()];
    for (i, node) in h.nodes().iter().enumerate().rev() {
        under[i] = match node {
            HierarchyNode::Star(star) => 1 << star.get(),
            HierarchyNode::Pair { inner, outer, .. } => {
                under[usize::from(inner.get())] | under[usize::from(outer.get())]
            }
        };
    }
    under
}

/// The members of `zone`, a bit per body index.
#[must_use]
fn members_of(zone: &OrbitZone) -> u32 {
    zone.members().fold(0, |bits, member| bits | 1 << member)
}

/// The orbit of the pair of `h` whose members are exactly `members`, if one is.
#[must_use]
fn pair_orbit<'h>(
    h: &'h SystemHierarchy,
    under: &[u32],
    members: u32,
) -> Option<&'h KeplerElements> {
    h.pairs()
        .find_map(|(node, orbit)| (under[usize::from(node.get())] == members).then_some(orbit))
}

/// The plane the planets of `zone` are placed about: a close pair's own for its circumbinary zone
/// (P14.T8.d), and otherwise one the host draws.
#[must_use]
fn plane_of(h: &SystemHierarchy, under: &[u32], zone: &OrbitZone) -> HostPlane {
    match zone.host() {
        OrbitHost::Star(_) | OrbitHost::Body(_) => HostPlane::Isotropic,
        OrbitHost::Pair(_) | OrbitHost::Barycentre => pair_orbit(h, under, members_of(zone))
            .filter(|orbit| orbit.semi_major_axis() < Metres::from(CLOSE_BINARY_SEMI_MAJOR_AXIS))
            .map_or(HostPlane::Isotropic, |orbit| {
                HostPlane::Aligned(SystemPlane::of_orbit(orbit.orientation()))
            }),
    }
}

impl PlanetarySystem {
    /// The system's ID.
    #[must_use]
    pub const fn system(&self) -> SystemId {
        self.system
    }

    /// The stable zones, one per orbit host, in hierarchy order (P14.T9).
    #[must_use]
    pub fn zones(&self) -> &[OrbitZone] {
        &self.zones
    }

    /// The orbit hosts, one per zone and in the same order.
    #[must_use]
    pub fn hosts(&self) -> &[PlanetaryHost] {
        &self.hosts
    }

    /// The orbit host `host`, if it is one of the system's zones.
    #[must_use]
    pub fn host(&self, host: OrbitHost) -> Option<&PlanetaryHost> {
        self.hosts.iter().find(|h| h.host == host)
    }

    /// The zone of the orbit host `host`, if it is one of the system's.
    #[must_use]
    pub fn zone(&self, host: OrbitHost) -> Option<&OrbitZone> {
        self.zones.iter().find(|zone| zone.host() == host)
    }

    /// The disc of the orbit host `host` at birth, if it is one of the system's zones; it may be
    /// [`Disc::None`].
    #[must_use]
    pub fn disc(&self, host: OrbitHost) -> Option<&Disc> {
        self.host(host).map(PlanetaryHost::disc)
    }

    /// The architecture class placed about the orbit host `host`, if it is one of the system's
    /// zones ([`PlanetaryHost::class`]).
    #[must_use]
    pub fn architecture(&self, host: OrbitHost) -> Option<ArchitectureClass> {
        self.host(host).map(PlanetaryHost::class)
    }

    /// Every body but the populations, sorted by index: the planets, their moons and rings, and
    /// the belts' members with their moons.
    #[must_use]
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// The body `index`, if the system holds it among [`bodies`](Self::bodies).
    #[must_use]
    pub fn body(&self, index: BodyIndex) -> Option<&Body> {
        self.bodies
            .binary_search_by_key(&index, Body::index)
            .ok()
            .map(|at| &self.bodies[at])
    }

    /// The planets alone, in index order.
    pub fn planets(&self) -> impl Iterator<Item = &Body> {
        self.bodies
            .iter()
            .filter(|body| matches!(body.part, Part::Planet(_)))
    }

    /// The satellites of every planet and member, empty or not, in the order of their parents'
    /// indices (P14.T22.a).
    #[must_use]
    pub fn satellites(&self) -> &[Satellites] {
        &self.satellites
    }

    /// The satellites of the body `parent`: [`Satellites::none`] for a body with none.
    #[must_use]
    pub fn satellites_of(&self, parent: BodyIndex) -> Satellites {
        self.satellites
            .iter()
            .find(|found| found.parent_index() == parent)
            .cloned()
            .unwrap_or(Satellites::none(parent))
    }

    /// The belts, in index order (P14.T21.a–c).
    #[must_use]
    pub fn belts(&self) -> &[Belt] {
        &self.belts
    }

    /// The belt `index`, if the system has it.
    #[must_use]
    pub fn belt(&self, index: BodyIndex) -> Option<&Belt> {
        self.belts.iter().find(|belt| belt.index() == index)
    }

    /// The cometary halo, if the system has one (P14.T21.d).
    #[must_use]
    pub fn halo(&self) -> Option<&CometaryHalo> {
        self.halo.as_ref().map(|halo| &halo.halo)
    }

    /// The orbit host the cometary halo surrounds, if the system has one.
    #[must_use]
    pub fn halo_host(&self) -> Option<OrbitHost> {
        self.halo.map(|halo| halo.host)
    }

    /// Every index the system holds a record for: its bodies', its belts' and its halo's, in
    /// index order.
    #[must_use]
    pub fn indices(&self) -> Vec<BodyIndex> {
        let mut indices: Vec<BodyIndex> = self
            .bodies
            .iter()
            .map(Body::index)
            .chain(self.belts.iter().map(Belt::index))
            .chain(self.halo().map(CometaryHalo::index))
            .collect();
        indices.sort_unstable();
        indices
    }

    /// The bytes the system owns on the heap, beyond `size_of::<PlanetarySystem>()`: its zones,
    /// hosts, bodies, satellites and belts, by capacity, with what each of those owns (a ring's
    /// gaps, a belt's gaps and members). The charge grows with the counts of bodies, hosts and
    /// belts.
    ///
    /// It is what a server charges a cached system against its byte budget (plan 14, P14.T36.a),
    /// beside [`SystemContext::heap_bytes`]; nothing generated reads it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.zones.capacity() * size_of::<OrbitZone>()
            + self.hosts.capacity() * size_of::<PlanetaryHost>()
            + self.bodies.capacity() * size_of::<Body>()
            + self.bodies.iter().map(Body::heap_bytes).sum::<usize>()
            + self.satellites.capacity() * size_of::<Satellites>()
            + self
                .satellites
                .iter()
                .map(Satellites::heap_bytes)
                .sum::<usize>()
            + self.belts.capacity() * size_of::<Belt>()
            + self.belts.iter().map(Belt::heap_bytes).sum::<usize>()
    }

    /// The bodies whose record names body `index` as its parent: a planet's moons and rings, a
    /// belt's members, a member's moon.
    pub fn children(&self, index: BodyIndex) -> impl Iterator<Item = &Body> {
        self.bodies
            .iter()
            .filter(move |body| body.parent() == OrbitHost::Body(index))
    }

    /// The planets of the orbit host `host`, as the belts read them.
    #[must_use]
    pub(crate) fn neighbours(&self, host: OrbitHost) -> Vec<Neighbour> {
        self.planets()
            .filter(|body| body.host == host)
            .filter_map(|body| {
                let orbit = body.orbit()?;
                Some(Neighbour::new(
                    body.mass(),
                    orbit.semi_major_axis(),
                    orbit.eccentricity().value(),
                ))
            })
            .collect()
    }

    /// The belt nearest the planet `planet` among its host's, as its captures read it (P14.T19,
    /// P14.T22.a): its primordial mass, and whether another of the host's planets lies between
    /// them. `None` for a body that is not a planet, or a host with no belt.
    #[must_use]
    pub fn nearest_belt(&self, planet: BodyIndex) -> Option<NearestBelt> {
        let body = self
            .body(planet)
            .filter(|body| matches!(body.part, Part::Planet(_)))?;
        let a = body.orbit()?.semi_major_axis().value();
        let (belt, edge) = self
            .belts
            .iter()
            .filter(|belt| belt.host() == body.host)
            .map(|belt| {
                let edge = a.clamp(belt.inner_edge().value(), belt.outer_edge().value());
                (belt, edge)
            })
            .min_by(|x, y| (x.1 - a).abs().total_cmp(&(y.1 - a).abs()))?;
        let (lo, hi) = (a.min(edge), a.max(edge));
        let between = self
            .planets()
            .filter(|other| other.host == body.host && other.index != planet)
            .filter_map(Body::orbit)
            .any(|orbit| {
                let b = orbit.semi_major_axis().value();
                lo < b && b < hi
            });
        let adjacency = if between {
            BeltAdjacency::Separated
        } else {
            BeltAdjacency::Neighbouring
        };
        NearestBelt::new(belt.initial_mass(), adjacency)
    }

    /// Every body's record at `t`, at [`DetailLevel::Full`](crate::planetary::record::DetailLevel::Full),
    /// in index order, the belts' and the halo's among them, with the system's belts and halo
    /// sections [`Section::Ok`] (P14.T30.b).
    ///
    /// Body by body it equals [`body_at`](Self::body_at).
    ///
    /// # Panics
    ///
    /// If `ctx` is not the context the system was generated from (its ID differs), a caller's
    /// error that would otherwise read another system's stars. In debug builds also if `t` is
    /// after the end of the clock window and a star lives past it
    /// ([`StarModel::state_at`](crate::stellar::system::StarModel::state_at)).
    #[must_use]
    pub fn snapshot_at(&self, ctx: &SystemContext, t: UniverseTime) -> SystemSnapshot {
        let epoch = Epoch::new(self, ctx, t);
        let labels: Vec<(BodyIndex, BodyLabel)> = label::labels(self);
        let label_of = |index: BodyIndex| {
            labels
                .binary_search_by_key(&index, |(at, _)| *at)
                .map(|at| labels[at].1.clone())
                .expect("every body of a system has a label")
        };
        let hosts: Vec<FateHost<'_>> = self.zones.iter().map(|zone| fate_host(ctx, zone)).collect();
        // Each host's planets are resolved together, for the scattering after a supernova
        // (ruling 71), and each exactly once; a member alone about its host.
        let mut fates: Vec<Option<BodyFate<'_>>> = vec![None; self.bodies.len()];
        for (zone, host) in self.zones.iter().zip(&hosts) {
            let members: Vec<usize> = (0..self.bodies.len())
                .filter(|&k| {
                    let body = &self.bodies[k];
                    body.host == zone.host() && matches!(body.part, Part::Planet(_))
                })
                .collect();
            let bodies: Vec<&FateBody> = members
                .iter()
                .filter_map(|&k| self.bodies[k].fate())
                .collect();
            for (k, fate) in members
                .into_iter()
                .zip(BodyFate::resolve_all(&bodies, host))
            {
                fates[k] = Some(fate);
            }
            for (k, body) in self.bodies.iter().enumerate() {
                if let (Part::Member(member), true) = (&body.part, body.host == zone.host()) {
                    fates[k] = Some(BodyFate::resolve(&member.fate, host));
                }
            }
        }
        let mut records = Vec::with_capacity(self.bodies.len() + self.belts.len() + 1);
        let mut parents: Vec<(BodyIndex, ParentNow)> = Vec::new();
        for (body, fate) in self.bodies.iter().zip(&fates) {
            let label = label_of(body.index);
            match &body.part {
                Part::Planet(_) | Part::Member(_) => {
                    let fate = fate
                        .as_ref()
                        .expect("every planet and member orbits one of its system's zones");
                    let (record, now) = self.primary_record(&epoch, body, label, fate);
                    records.push(record);
                    parents.push((body.index, now));
                }
                Part::Moon(_) | Part::Ring(_) => {
                    let OrbitHost::Body(parent) = body.host else {
                        unreachable!("a moon's and a ring's host is a body")
                    };
                    let now = parents
                        .iter()
                        .find(|(at, _)| *at == parent)
                        .map(|(_, now)| now)
                        .expect("a satellite's parent comes before it in index order");
                    records.push(self.satellite_record(&epoch, body, label, now));
                }
            }
        }
        records.extend(
            self.belts
                .iter()
                .map(|belt| self.belt_record(&epoch, belt, label_of(belt.index()))),
        );
        if let Some(halo) = &self.halo {
            records.push(self.halo_record(&epoch, halo, label_of(halo.halo.index())));
        }
        records.sort_by_key(BodyRecord::index);
        let belts = Section::Ok(self.belts.iter().map(Belt::index).collect());
        let halo = Section::Ok(self.halo().map(CometaryHalo::index));
        SystemSnapshot::new(self.system, t, records)
            .and_then(|snapshot| snapshot.with_populations(belts, halo))
            .expect("a system's records are its own, in index order, at full detail")
    }

    /// The record of body `index` at `t`, at [`DetailLevel::Full`](crate::planetary::record::DetailLevel::Full)
    /// (P14.T30.b).
    ///
    /// - The identity: the kind, the label (P14.T30.c), what the body orbits as its parent (a
    ///   belt's member its belt), and its state at `t` from the fate transform (P14.T28); a moon
    ///   and a ring take their parent's, and a giant-impact moon may be lost first (P14.T18).
    /// - The mass, always: a body not yet formed, destroyed or unbound has the mass it had, and a
    ///   present planet carries any neighbour that collided with it and merged into it (ruling
    ///   71); a belt's mass wears down (P14.T21.a); a halo's is not modelled.
    /// - For a body present at `t`: its orbit at `t` with the time it holds until, its position,
    ///   and its bulk from [`derive_body`] about its hosts at `t` (a moon's by P14.T17.b), with
    ///   the surface [`Section::NotModelled`] ([`Section::NotApplicable`] for a giant). A ring,
    ///   a belt and a halo carry their extent as their population section instead.
    /// - For a body not present: no position, and its orbit, bulk, surface and population
    ///   [`Section::NotApplicable`], since nothing orbits there at `t`.
    /// - A planet's moons and rings [`Section::Ok`], lists that may be empty; every hooks section
    ///   [`Section::NotModelled`] (ruling 34), a population's `NotApplicable`.
    ///
    /// # Errors
    ///
    /// [`ResolveBodyError::NoSuchBody`] if the system holds no body `index`: an unused slot or a
    /// star, whose records are plan 06's.
    ///
    /// # Panics
    ///
    /// As [`snapshot_at`](Self::snapshot_at).
    pub fn body_at(
        &self,
        ctx: &SystemContext,
        index: BodyIndex,
        t: UniverseTime,
    ) -> Result<BodyRecord, ResolveBodyError> {
        let epoch = Epoch::new(self, ctx, t);
        let label = label::label(self, index).ok_or(ResolveBodyError::NoSuchBody)?;
        if let Some(belt) = self.belt(index) {
            return Ok(self.belt_record(&epoch, belt, label));
        }
        if let Some(halo) = self.halo.as_ref().filter(|halo| halo.halo.index() == index) {
            return Ok(self.halo_record(&epoch, halo, label));
        }
        let body = self.body(index).ok_or(ResolveBodyError::NoSuchBody)?;
        match body.host {
            OrbitHost::Body(parent) => {
                let parent = self
                    .body(parent)
                    .expect("a satellite's parent is a body of its system");
                let host = fate_host(ctx, self.zone_of(parent));
                let fate = self.fate_of(parent, &host);
                let parent_label =
                    label::label(self, parent.index).expect("every body of a system has a label");
                let (_, now) = self.primary_record(&epoch, parent, parent_label, &fate);
                Ok(self.satellite_record(&epoch, body, label, &now))
            }
            OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => {
                let host = fate_host(ctx, self.zone_of(body));
                let fate = self.fate_of(body, &host);
                Ok(self.primary_record(&epoch, body, label, &fate).0)
            }
        }
    }

    /// Where body `index` is at `t`, in the system frame from its barycentre (plan 01's `coords`):
    /// its host's position, a star's or a pair's barycentre on plan 11's hierarchy, plus the
    /// body's Kepler state about it, a moon's planet's position plus its own offset, and a ring's
    /// its planet's; `None` for a body not present at `t`, and for a belt or the halo, which have
    /// no single position (P14.T30.b).
    ///
    /// # Errors
    ///
    /// As [`body_at`](Self::body_at).
    ///
    /// # Panics
    ///
    /// As [`snapshot_at`](Self::snapshot_at).
    pub fn position_at(
        &self,
        ctx: &SystemContext,
        index: BodyIndex,
        t: UniverseTime,
    ) -> Result<Option<SystemPosition>, ResolveBodyError> {
        if self.belt(index).is_some() || self.halo().is_some_and(|halo| halo.index() == index) {
            return Ok(None);
        }
        let body = self.body(index).ok_or(ResolveBodyError::NoSuchBody)?;
        let epoch = Epoch::new(self, ctx, t);
        let primary = match body.host {
            OrbitHost::Body(parent) => self
                .body(parent)
                .expect("a satellite's parent is a body of its system"),
            OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => body,
        };
        let zone = self.zone_of(primary);
        let host = fate_host(ctx, zone);
        let fate = self.fate_of(primary, &host).at(t);
        let Some(centre) = fate.orbit().map(|orbit| epoch.position(zone, orbit)) else {
            return Ok(None);
        };
        Ok(match &body.part {
            Part::Planet(_) | Part::Member(_) | Part::Ring(_) => Some(centre),
            Part::Moon(moon) => {
                match moon_state(fate.state(), &moon.satellite, ctx.age_at_epoch(), t) {
                    BodyState::Present => {
                        let orbit = moon.satellite.orbit_at(&moon.parent, ctx.age_at(t));
                        Some(centre.translated(orbit.relative_state_at(t).0))
                    }
                    BodyState::NotYetFormed
                    | BodyState::Destroyed { .. }
                    | BodyState::Unbound { .. } => None,
                }
            }
        })
    }

    /// The habitable zone of the orbit host `host` at `t` (P14.T12.b): Kopparapu et al.'s limits
    /// for the host's stars at `t`, pushed out by the light of the system's other stars, each
    /// seen along the orbit that separates it from the host ([`habitable_zone_of`]).
    ///
    /// `None` if `host` is not one of the system's zones, or its stars have not formed by `t`.
    ///
    /// # Panics
    ///
    /// As [`snapshot_at`](Self::snapshot_at).
    #[must_use]
    pub fn habitable_zone_at(
        &self,
        ctx: &SystemContext,
        host: OrbitHost,
        t: UniverseTime,
    ) -> Option<HabitableZone> {
        let zone = self.zone(host)?;
        let epoch = Epoch::new(self, ctx, t);
        let orbited = epoch.lights(zone)?;
        let companions = epoch.companions(zone)?;
        Some(habitable_zone_of(&orbited, &companions))
    }

    /// The zone of `body`'s host: a planet's or a member's own, a satellite's parent's.
    #[must_use]
    fn zone_of(&self, body: &Body) -> &OrbitZone {
        let mut host = body.host;
        while let OrbitHost::Body(parent) = host {
            host = self
                .body(parent)
                .expect("a satellite's parent is a body of its system")
                .host;
        }
        self.zone(host)
            .expect("every body orbits one of its system's zones")
    }

    /// The history of the planet or member `body` on `host`, its zone's stars (P14.T28): a
    /// planet's among the other planets of its host ([`BodyFate::resolve_among`]), which it meets
    /// only in the scattering after a supernova (ruling 71), and a member's alone.
    #[must_use]
    fn fate_of<'s>(&'s self, body: &'s Body, host: &'s FateHost<'s>) -> BodyFate<'s> {
        match &body.part {
            Part::Member(member) => BodyFate::resolve(&member.fate, host),
            Part::Planet(_) => {
                let siblings: Vec<&Body> = self.planets().filter(|b| b.host == body.host).collect();
                let index = siblings
                    .iter()
                    .position(|b| b.index == body.index)
                    .expect("a planet is among its host's planets");
                let bodies: Vec<&FateBody> = siblings.iter().filter_map(|b| b.fate()).collect();
                BodyFate::resolve_among(&bodies, index, host)
            }
            Part::Moon(_) | Part::Ring(_) => {
                unreachable!("a satellite has no history of its own")
            }
        }
    }

    /// The indices of the moons and of the rings of body `index`.
    #[must_use]
    fn moons_and_rings(&self, index: BodyIndex) -> (Vec<BodyIndex>, Vec<BodyIndex>) {
        let mut moons = Vec::new();
        let mut rings = Vec::new();
        for child in self.children(index) {
            match child.part {
                Part::Moon(_) => moons.push(child.index),
                Part::Ring(_) => rings.push(child.index),
                Part::Planet(_) | Part::Member(_) => {}
            }
        }
        (moons, rings)
    }

    /// The record of the planet or member `body` at the epoch's time, labelled `label`, with its
    /// history `fate`, and what its satellites read of it then.
    #[must_use]
    fn primary_record(
        &self,
        epoch: &Epoch<'_>,
        body: &Body,
        label: BodyLabel,
        fate: &BodyFate<'_>,
    ) -> (BodyRecord, ParentNow) {
        let zone = self.zone_of(body);
        let at = fate.at(epoch.t);
        let identity = BodyIdentity::new(
            self.system,
            body.index,
            body.kind(),
            Some(body.parent()),
            at.state(),
        )
        .with_label(label);
        let (moons, rings) = self.moons_and_rings(body.index);
        let builder = BodyRecord::builder(identity)
            .mass(Section::Ok(at.mass()))
            .moons(Section::Ok(moons))
            .population(Section::NotApplicable);
        let builder = match &body.part {
            Part::Member(_) => builder.rings(Section::NotModelled),
            Part::Planet(_) | Part::Moon(_) | Part::Ring(_) => builder.rings(Section::Ok(rings)),
        };
        let mut now = ParentNow {
            state: at.state(),
            valid_until: at.valid_until(),
            position: None,
            derived: None,
            sky: None,
            nursery: None,
        };
        let builder = match at.state() {
            BodyState::Present => {
                let orbit = at.orbit().expect("a present body has an orbit");
                let section = at
                    .body_orbit()
                    .expect("a present body has an orbit section");
                let section = match &body.part {
                    Part::Member(_) => section.about(body.host),
                    Part::Planet(_) | Part::Moon(_) | Part::Ring(_) => section,
                };
                let placed = Self::placed_now(body, orbit, at.mass());
                let (derived, sky) =
                    self.derive(epoch, body, zone, &placed, fate.host_mass(epoch.t));
                let position = epoch.position(zone, orbit);
                now.position = Some(position);
                now.derived = Some(derived);
                now.sky = sky;
                now.nursery = self.nursery(epoch.ctx, body);
                builder
                    .derived(&derived)
                    .orbit(Section::Ok(section))
                    .position(position)
            }
            BodyState::NotYetFormed | BodyState::Destroyed { .. } | BodyState::Unbound { .. } => {
                builder
                    .orbit(Section::NotApplicable)
                    .bulk(Section::NotApplicable)
                    .surface(Section::NotApplicable)
            }
        };
        let record = builder
            .build()
            .expect("a generated record withholds no section and has a known kind");
        (record, now)
    }

    /// The planet or member `body` on its orbit `orbit` at the time, of mass `mass` then, as
    /// [`derive_body`] reads it.
    #[must_use]
    fn placed_now(body: &Body, orbit: &KeplerElements, mass: EarthMasses) -> PlacedBody {
        let placed = match &body.part {
            Part::Planet(planet) => PlacedBody::new(
                mass,
                *planet.placed.orbit(),
                planet.placed.formation_distance(),
                planet.radius_rank,
            )
            .expect("a placed planet's mass and formation distance are positive"),
            Part::Member(member) => member.member.placed_body(),
            Part::Moon(_) | Part::Ring(_) => unreachable!("a satellite is derived by its own rule"),
        };
        placed.with_orbit_now(*orbit)
    }

    /// Where the moons of `body` formed (P14.T17.b), if it has any.
    #[must_use]
    fn nursery(&self, ctx: &SystemContext, body: &Body) -> Option<MoonNursery> {
        let found = self
            .satellites
            .iter()
            .find(|found| found.parent_index() == body.index)?;
        let parent = found.parent()?;
        let profile = self.host(self.zone_of(body).host())?.disc.profile()?;
        let formation = match &body.part {
            Part::Planet(planet) => planet.placed.formation_distance(),
            Part::Member(member) => member.member.orbit().semi_major_axis(),
            Part::Moon(_) | Part::Ring(_) => return None,
        };
        MoonNursery::new(parent, ctx.composition(), profile, formation).ok()
    }

    /// The record of the moon or ring `body` at the epoch's time, labelled `label`, whose parent
    /// is as `parent` says then.
    #[must_use]
    fn satellite_record(
        &self,
        epoch: &Epoch<'_>,
        body: &Body,
        label: BodyLabel,
        parent: &ParentNow,
    ) -> BodyRecord {
        let t = epoch.t;
        let ctx = epoch.ctx;
        let state = match &body.part {
            Part::Moon(moon) => moon_state(parent.state, &moon.satellite, ctx.age_at_epoch(), t),
            Part::Ring(_) => parent.state,
            Part::Planet(_) | Part::Member(_) => unreachable!("a satellite is a moon or a ring"),
        };
        let identity =
            BodyIdentity::new(self.system, body.index, body.kind(), Some(body.host), state)
                .with_label(label);
        let builder = BodyRecord::builder(identity)
            .mass(Section::Ok(body.mass()))
            .moons(Section::NotApplicable)
            .rings(Section::NotApplicable);
        let present = state == BodyState::Present;
        let builder = match (&body.part, present, parent.position) {
            (Part::Ring(ring), true, Some(centre)) => builder
                .population(Section::Ok(Population::Ring(ring.clone())))
                .position(centre)
                .orbit(Section::NotApplicable)
                .bulk(Section::NotApplicable)
                .surface(Section::NotApplicable)
                .hooks(Section::NotApplicable),
            (Part::Moon(moon), true, Some(centre)) => {
                let orbit = moon.satellite.orbit_at(&moon.parent, ctx.age_at(t));
                let valid_until = earliest(
                    parent.valid_until,
                    moon_lost_at(&moon.satellite, ctx.age_at_epoch())
                        .filter(|&lost| lost > t && ClockWindow::contains(lost)),
                );
                let bulk = Self::moon_bulk(epoch, moon, &orbit, parent);
                builder
                    .orbit(Section::Ok(BodyOrbit::new(orbit, valid_until)))
                    .position(centre.translated(orbit.relative_state_at(t).0))
                    .bulk(bulk)
                    .population(Section::NotApplicable)
            }
            (Part::Ring(_), _, _) => builder
                .population(Section::NotApplicable)
                .orbit(Section::NotApplicable)
                .bulk(Section::NotApplicable)
                .surface(Section::NotApplicable)
                .hooks(Section::NotApplicable),
            (Part::Moon(_), _, _) => builder
                .population(Section::NotApplicable)
                .orbit(Section::NotApplicable)
                .bulk(Section::NotApplicable)
                .surface(Section::NotApplicable),
            (Part::Planet(_) | Part::Member(_), _, _) => {
                unreachable!("a satellite is a moon or a ring")
            }
        };
        builder
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }

    /// The bulk of the present moon `moon` on `orbit` at the epoch's time, whose parent is as
    /// `parent` says (P14.T17.b): a regular moon's from its own derivation, a giant-impact moon's
    /// of its own density and a capture's of its own radius, each with the flux and temperature
    /// of the derivation.
    #[must_use]
    fn moon_bulk(
        epoch: &Epoch<'_>,
        moon: &MoonPart,
        orbit: &KeplerElements,
        parent: &ParentNow,
    ) -> Section<BulkProperties> {
        let (Some(nursery), Some(sky)) = (&parent.nursery, &parent.sky) else {
            // A parent whose satellites' nursery or sky cannot be built (a giant beyond the
            // cooling fit's 13 Jupiter masses, which placement never makes) has moons whose bulk
            // this generator version does not compute.
            return Section::NotModelled;
        };
        let satellite = &moon.satellite;
        let Ok(derived) = derive_moon(
            satellite.mass(),
            orbit,
            &moon.parent,
            nursery,
            sky,
            satellite.radius_rank(),
            epoch.ctx.age_at_epoch(),
            epoch.t,
        ) else {
            // As above: the derivation refuses only inputs the generator never makes.
            return Section::NotModelled;
        };
        let mass = satellite.mass();
        let (radius, density, fractions) = match satellite.moon() {
            SatelliteMoon::Regular(_) => (derived.radius(), derived.density(), derived.fractions()),
            SatelliteMoon::GiantImpact(impact) => {
                let fractions = match (moon.parent.class(), &parent.derived) {
                    (PlanetClass::Icy, Some(planet)) => planet.fractions(),
                    _ => MassFractions::solid(0.0, 1.0, 0.0),
                };
                (
                    sphere_radius(mass, impact.density()),
                    impact.density(),
                    fractions,
                )
            }
            SatelliteMoon::Captured(captured) => {
                let density = sphere_density(mass, captured.radius());
                let ice = ice_fraction_of(density);
                (
                    captured.radius(),
                    density,
                    MassFractions::solid(0.0, 1.0 - ice, ice),
                )
            }
        };
        let r = radius.value();
        let gravity = MetresPerSecondSquared::new(GM_EARTH * mass.value() / (r * r));
        Section::Ok(BulkProperties::new(
            EarthRadii::from(radius),
            density,
            gravity,
            PlanetClass::of(mass, &fractions),
            fractions,
            derived.equilibrium_temperature(),
        ))
    }

    /// The record of `belt` at the epoch's time, labelled `label` (P14.T21, P14.T30.b).
    ///
    /// The belt goes as a circular orbit at the geometric mean of its main component's edges
    /// would, alone about its host (the fate transform, P14.T28): formed at its disc's lifetime,
    /// widened with the host's mass loss, engulfed or unbound with it. Its mass wears down by
    /// collisions (Wyatt et al. 2007a), and its dust shines by its host's light then.
    #[must_use]
    fn belt_record(&self, epoch: &Epoch<'_>, belt: &Belt, label: BodyLabel) -> BodyRecord {
        let ctx = epoch.ctx;
        let zone = self
            .zone(belt.host())
            .expect("a belt's host is one of its system's zones");
        let lifetime = self
            .host(belt.host())
            .and_then(|host| host.disc.profile())
            .map(DiscProfile::lifetime)
            .expect("a belt's host has a disc");
        let plane = self
            .host(belt.host())
            .map(PlanetaryHost::plane)
            .expect("a belt's host is one of its system's hosts");
        let (inner, outer) = (belt.main().inner_edge(), belt.main().outer_edge());
        let middle = Metres::new((inner.value() * outer.value()).sqrt());
        let orientation = Orientation::new(plane.inclination(), plane.node(), Radians::ZERO)
            .expect("a host's plane is a valid orientation");
        let orbit = KeplerElements::from_semi_major_axis(
            middle,
            GravitationalParameter::from_solar_masses(zone.host_mass()),
            Eccentricity::new(0.0).expect("a circle is a bound orbit"),
            orientation,
            Radians::ZERO,
        )
        .expect("a belt's radius and its host's mass are positive");
        let formation =
            Formation::from_draws(belt.initial_mass(), lifetime, &FormationDraws::MEDIAN)
                .expect("a belt's mass and its disc's lifetime are positive");
        let proxy = FateBody::new(
            formation,
            orbit,
            belt.initial_mass(),
            belt.composition().member_density(),
        )
        .expect("a belt's mass and its members' density are positive");
        let host = fate_host(ctx, zone);
        let at = BodyFate::resolve(&proxy, &host).at(epoch.t);
        let identity = BodyIdentity::new(
            self.system,
            belt.index(),
            BodyKind::Belt(belt.kind()),
            Some(belt.host()),
            at.state(),
        )
        .with_label(label);
        let age = ctx.age_at(epoch.t);
        let builder = BodyRecord::builder(identity)
            .orbit(Section::NotApplicable)
            .moons(Section::NotApplicable)
            .rings(Section::NotApplicable)
            .bulk(Section::NotApplicable)
            .surface(Section::NotApplicable)
            .hooks(Section::NotApplicable);
        let builder = match (at.state(), at.orbit()) {
            (BodyState::Present, Some(now)) => {
                let expansion = now.semi_major_axis().value() / middle.value();
                let luminosity = epoch.luminosity(zone);
                let shine = luminosity.map_or(0.0, |l| belt.fractional_luminosity(age, l));
                let members = belt.members().iter().map(BeltMember::index).collect();
                builder
                    .mass(Section::Ok(belt.mass_at(age)))
                    .population(Section::Ok(Population::Belt(BeltRecord::new(
                        belt, expansion, shine, members,
                    ))))
            }
            _ => builder
                .mass(Section::Ok(belt.initial_mass()))
                .population(Section::NotApplicable),
        };
        builder
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }

    /// The record of the halo `halo` at the epoch's time, labelled `label` (P14.T21.d,
    /// P14.T30.b): present from its host's disc's lifetime, with what its host's mass loss has
    /// left of it ([`CometaryHalo::at`]; ruling 84.4), and unbound once nothing is left.
    #[must_use]
    fn halo_record(&self, epoch: &Epoch<'_>, halo: &SystemHalo, label: BodyLabel) -> BodyRecord {
        let ctx = epoch.ctx;
        let zone = self
            .zone(halo.host)
            .expect("a halo's host is one of its system's zones");
        let lifetime = self
            .host(halo.host)
            .and_then(|host| host.disc.profile())
            .map_or(0.0, |profile| Years::from(profile.lifetime()).value());
        let stars: Vec<&StarModel> = zone
            .members()
            .map(|member| &ctx.stars()[usize::from(member)])
            .collect();
        let states: Option<Vec<StarState>> =
            stars.iter().map(|star| star.state_at(epoch.t)).collect();
        let age = ctx.age_at(epoch.t).value();
        let (state, now) = match states {
            Some(states) if age >= lifetime => {
                let mass_now = states
                    .iter()
                    .fold(SolarMasses::ZERO, |sum, state| sum + state.mass());
                let rate = stars
                    .iter()
                    .map(|star| fastest_mass_loss_rate(star, epoch.t).value())
                    .fold(0.0, f64::max);
                let luminosity = states
                    .iter()
                    .fold(0.0, |sum, state| sum + state.luminosity().value());
                match halo.halo.at(mass_now, SolarMassesPerYear::new(rate)) {
                    Some(left) => (
                        BodyState::Present,
                        Some(HaloRecord::new(
                            halo.host,
                            left.inner_edge(),
                            left.outer_edge(),
                            left.comets(),
                            left.comet_rate(SolarLuminosities::new(luminosity)),
                        )),
                    ),
                    None => (
                        BodyState::Unbound {
                            at: first_death(&stars).unwrap_or(epoch.t),
                        },
                        None,
                    ),
                }
            }
            Some(_) | None => (BodyState::NotYetFormed, None),
        };
        let identity = BodyIdentity::new(
            self.system,
            halo.halo.index(),
            BodyKind::CometaryHalo,
            Some(halo.host),
            state,
        )
        .with_label(label);
        BodyRecord::builder(identity)
            .mass(Section::NotModelled)
            .orbit(Section::NotApplicable)
            .moons(Section::NotApplicable)
            .rings(Section::NotApplicable)
            .bulk(Section::NotApplicable)
            .surface(Section::NotApplicable)
            .hooks(Section::NotApplicable)
            .population(now.map_or(Section::NotApplicable, |halo| {
                Section::Ok(Population::CometaryHalo(halo))
            }))
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }

    /// Everything [`derive_body`] computes of the present planet or member `body` of `zone` at
    /// the epoch's time, placed as `placed` says then, about hosts of total mass `host_mass`, and
    /// the sky its moons see (P14.T17.b's [`MoonSky`]).
    #[must_use]
    fn derive(
        &self,
        epoch: &Epoch<'_>,
        body: &Body,
        zone: &OrbitZone,
        placed: &PlacedBody,
        host_mass: SolarMasses,
    ) -> (DerivedBody, Option<MoonSky>) {
        let disc = self
            .host(zone.host())
            .and_then(|host| host.disc.profile())
            .expect("a host with planets has a disc");
        let orbited = epoch
            .lights(zone)
            .expect("a present body's host stars have formed");
        let companions = epoch
            .companions(zone)
            .expect("a system's stars are coeval, so every one has formed");
        let hosts = BodyHosts::new(
            Kilograms::from(host_mass),
            *epoch.ctx.composition(),
            &orbited,
            &companions,
        )
        .expect("a present body orbits a positive mass and at least one star");
        let derived = derive_body(placed, &hosts, disc, epoch.ctx.age_at_epoch(), epoch.t).expect(
            "a present body's system is born, and a placed planet lies below 13 Jupiter masses",
        );
        let has_moons = self.children(body.index).next().is_some();
        let sky = has_moons
            .then(|| MoonSky::new(&derived, placed.orbit_now(), &hosts).ok())
            .flatten();
        (derived, sky)
    }
}

/// What a planet or member is at a time, as its moons and rings read it.
#[derive(Debug, Clone, PartialEq)]
struct ParentNow {
    state: BodyState,
    valid_until: Option<UniverseTime>,
    position: Option<SystemPosition>,
    derived: Option<DerivedBody>,
    sky: Option<MoonSky>,
    nursery: Option<MoonNursery>,
}

/// The state at `t` of the moon `satellite`, in a system aged `age_at_epoch` at the epoch, whose
/// parent's state then is `parent`: its parent's, unless a giant-impact moon has receded past its
/// stability limit and been lost first (P14.T18).
#[must_use]
fn moon_state(
    parent: BodyState,
    satellite: &Satellite,
    age_at_epoch: Years,
    t: UniverseTime,
) -> BodyState {
    match (parent, satellite.moon()) {
        (BodyState::Present, SatelliteMoon::GiantImpact(moon)) => moon.state_at(age_at_epoch, t),
        (BodyState::Present, SatelliteMoon::Regular(_) | SatelliteMoon::Captured(_)) => {
            BodyState::Present
        }
        (other, _) => other,
    }
}

/// When a giant-impact moon is lost, as a time, if it is representable.
#[must_use]
fn moon_lost_at(satellite: &Satellite, age_at_epoch: Years) -> Option<UniverseTime> {
    let SatelliteMoon::GiantImpact(moon) = satellite.moon() else {
        return None;
    };
    time_at_age(moon.lost_at(), age_at_epoch)
}

/// The time at which a system aged `age_at_epoch` at the epoch is `age` old, if representable.
#[must_use]
fn time_at_age(age: Years, age_at_epoch: Years) -> Option<UniverseTime> {
    Span::from_seconds_f64((age.value() - age_at_epoch.value()) * SECONDS_PER_JULIAN_YEAR)
        .and_then(|span| UniverseTime::EPOCH.checked_add(span))
}

/// The earlier of two optional times.
#[must_use]
fn earliest(a: Option<UniverseTime>, b: Option<UniverseTime>) -> Option<UniverseTime> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, None) => a,
        (None, b) => b,
    }
}

/// The radius of a sphere of `mass` at `density`.
#[must_use]
fn sphere_radius(mass: EarthMasses, density: KilogramsPerCubicMetre) -> Metres {
    let volume = Kilograms::from(mass).value() / density.value();
    Metres::new(math::cbrt(3.0 * volume / (4.0 * core::f64::consts::PI)))
}

/// The ice fraction by mass of a two-component sphere of rock and ice of mean density
/// `density`: ruling 83.7's uncompressed rock at 3,300 kg m⁻³ and ice at 940, held to [0, 1].
/// A capture of 1,700 kg m⁻³ (P14.T19) holds 0.375.
#[must_use]
fn ice_fraction_of(density: KilogramsPerCubicMetre) -> f64 {
    let (rock, ice) = (MOON_ROCK_DENSITY.value(), MOON_ICE_DENSITY.value());
    ((1.0 / density.value() - 1.0 / rock) / (1.0 / ice - 1.0 / rock)).clamp(0.0, 1.0)
}

/// Whether a star in `phase` has reached its thermally pulsing asymptotic giant branch or passed
/// it to a white dwarf.
#[must_use]
const fn on_or_past_tpagb(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::ThermallyPulsingAgb
            | Phase::PostAgb
            | Phase::HeliumWhiteDwarf
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf
    )
}

/// Whether a star in `phase` has left its thermally pulsing asymptotic giant branch.
#[must_use]
const fn past_tpagb(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::PostAgb
            | Phase::HeliumWhiteDwarf
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf
    )
}

/// Steps of the bisections of [`fastest_mass_loss_rate`], a fixed count: 2⁻⁴⁸ of a star's age.
const PHASE_BISECTIONS: u32 = 48;

/// The rate of the fastest phase of mass loss `star` has been through by `t`, which the halo's
/// loss reads (P14.T21.d's `CometaryHalo::at`; ruling 84.4): infinite after a sudden death, and
/// otherwise the mean rate over its thermally pulsing asymptotic giant branch so far, its mass
/// lost there over its duration; zero for a star that has not reached it, whose slower winds
/// keep every comet (Veras et al. 2011, the adiabatic regime).
///
/// The branch's ends are found by bisection on the phase in age, a fixed number of steps.
#[must_use]
fn fastest_mass_loss_rate(star: &StarModel, t: UniverseTime) -> SolarMassesPerYear {
    let age_t = star.age_at(t).value();
    if let Some(death) = star.death()
        && death.kind().is_sudden()
        && death.age().value() <= age_t
    {
        return SolarMassesPerYear::new(f64::INFINITY);
    }
    let epoch_age = star.age_at_epoch();
    let state = |age: f64| time_at_age(Years::new(age), epoch_age).and_then(|at| star.state_at(at));
    let phase_at = |age: f64| state(age).map(|s| s.phase());
    let first = |lo: f64, hi: f64, test: fn(Phase) -> bool| {
        let (mut lo, mut hi) = (lo, hi);
        for _ in 0..PHASE_BISECTIONS {
            let mid = f64::midpoint(lo, hi);
            if phase_at(mid).is_some_and(test) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    };
    if !phase_at(age_t).is_some_and(on_or_past_tpagb) {
        return SolarMassesPerYear::new(0.0);
    }
    let start = first(0.0, age_t, on_or_past_tpagb);
    let end = if phase_at(age_t).is_some_and(past_tpagb) {
        first(start, age_t, past_tpagb)
    } else {
        age_t
    };
    let mass = |age: f64| state(age).map_or(0.0, |s| s.mass().value());
    if end > start {
        SolarMassesPerYear::new(((mass(start) - mass(end)) / (end - start)).max(0.0))
    } else {
        SolarMassesPerYear::new(0.0)
    }
}

/// The earliest death among `stars` as a time, if any dies and it is representable.
#[must_use]
fn first_death(stars: &[&StarModel]) -> Option<UniverseTime> {
    stars
        .iter()
        .filter_map(|star| {
            let death = star.death()?;
            time_at_age(death.age(), star.age_at_epoch())
        })
        .min()
}

/// The stars the bodies of `zone` orbit, as the fate transform reads them: its component alone,
/// or every component under its pair, in body order.
#[must_use]
fn fate_host<'c>(ctx: &'c SystemContext, zone: &OrbitZone) -> FateHost<'c> {
    let stars = ctx.stars();
    FateHost::stars(zone.members().map(|member| &stars[usize::from(member)]))
        .expect("a zone has a component, and a system's stars are coeval")
}

/// What the queries at one time share: the context, the time, the stars' states then and the
/// hierarchy's membership.
struct Epoch<'c> {
    ctx: &'c SystemContext,
    t: UniverseTime,
    under: Vec<u32>,
    states: Vec<Option<StarState>>,
}

impl<'c> Epoch<'c> {
    /// The queries of `system`, generated from `ctx`, at `t`.
    ///
    /// # Panics
    ///
    /// If `ctx` is not the context `system` was generated from.
    #[must_use]
    fn new(system: &PlanetarySystem, ctx: &'c SystemContext, t: UniverseTime) -> Self {
        assert_eq!(
            ctx.id(),
            system.system,
            "a system is queried with the context it was generated from"
        );
        Self::of(ctx, t)
    }

    /// The queries of the system of `ctx` at `t`, before it is generated.
    #[must_use]
    fn of(ctx: &'c SystemContext, t: UniverseTime) -> Self {
        Self {
            ctx,
            t,
            under: members_under(ctx.hierarchy()),
            states: ctx.stars().iter().map(|star| star.state_at(t)).collect(),
        }
    }

    /// The summed luminosity of the stars `zone`'s bodies orbit at the time, or `None` before
    /// they form.
    #[must_use]
    fn luminosity(&self, zone: &OrbitZone) -> Option<SolarLuminosities> {
        zone.members()
            .map(|member| self.states[usize::from(member)].map(|state| state.luminosity().value()))
            .sum::<Option<f64>>()
            .map(SolarLuminosities::new)
    }

    /// The light of component `star` at the time, or `None` before it forms (ruling 34: a host's
    /// L, `T_eff` and R from its state).
    #[must_use]
    fn light(&self, star: usize) -> Option<HostLight> {
        let state = self.states[star]?;
        Some(
            HostLight::new(
                state.luminosity(),
                state.effective_temperature(),
                state.radius(),
            )
            .expect("a star's state is finite and not negative, with a surface if it shines"),
        )
    }

    /// The light of the stars `zone`'s bodies orbit, in body order, or `None` before they form.
    #[must_use]
    fn lights(&self, zone: &OrbitZone) -> Option<Vec<HostLight>> {
        zone.members()
            .map(|member| self.light(usize::from(member)))
            .collect()
    }

    /// The light of every other star of the system on `zone`'s bodies, each seen along the orbit
    /// of the lowest pair that holds both it and the zone's host (design note 10, P14.T12.a), or
    /// `None` before they form.
    #[must_use]
    fn companions(&self, zone: &OrbitZone) -> Option<Vec<Illumination>> {
        let h = self.ctx.hierarchy();
        let members = members_of(zone);
        (0..self.states.len())
            .filter(|&star| (members & (1 << star)) == 0)
            .map(|star| {
                let both = members | 1 << star;
                let separation = h
                    .pairs()
                    .filter(|(node, _)| {
                        let under = self.under[usize::from(node.get())];
                        (under & both) == both
                    })
                    .min_by_key(|(node, _)| self.under[usize::from(node.get())].count_ones())
                    .map(|(_, orbit)| orbit)
                    .expect("the root pair holds every star of a multiple system");
                let light = self.light(star)?;
                Some(
                    Illumination::new(
                        light,
                        separation.semi_major_axis(),
                        separation.eccentricity().value(),
                    )
                    .expect("a pair's orbit is bound, with a positive axis"),
                )
            })
            .collect()
    }

    /// The position of the barycentre of the components `members` at the time, on the walk of
    /// plan 11's [`star_positions_at`](crate::stellar::multiplicity::star_positions_at) from the
    /// root down, with its arithmetic: for a single component, its position bit for bit.
    #[must_use]
    fn centre(&self, members: u32) -> SystemPosition {
        let h = self.ctx.hierarchy();
        let mut node = h.root();
        let mut centre = SystemPosition::ORIGIN;
        loop {
            if self.under[usize::from(node.get())] == members {
                return centre;
            }
            let (inner, outer, orbit) = match h.node(node) {
                // A component is reached only when it is the one sought.
                HierarchyNode::Star(_) => return centre,
                HierarchyNode::Pair {
                    inner,
                    outer,
                    orbit,
                } => (inner, outer, orbit),
            };
            let (separation, _) = orbit.relative_state_at(self.t);
            let inner_mass = h.node_mass(*inner).value();
            let outer_mass = h.node_mass(*outer).value();
            let mass = h.node_mass(node).value();
            if self.under[usize::from(inner.get())] & members == members {
                centre = centre.translated(separation * -(outer_mass / mass));
                node = *inner;
            } else {
                centre = centre.translated(separation * (inner_mass / mass));
                node = *outer;
            }
        }
    }

    /// The position at the time of a body of `zone` on `orbit`, its elements then about the zone's
    /// host.
    #[must_use]
    fn position(&self, zone: &OrbitZone, orbit: &KeplerElements) -> SystemPosition {
        let (offset, _) = orbit.relative_state_at(self.t);
        self.centre(members_of(zone)).translated(offset)
    }
}

#[cfg(test)]
pub(crate) mod tests;
