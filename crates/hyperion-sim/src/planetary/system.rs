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
//!    disc and its reach. A close binary acts through the reach alone: design note 10's flag
//!    ([`OrbitZone::host_multiplicity`]) moves no class weight (decision-p14-d10-bd-companions
//!    §2).
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
//! Then every planet's and member's rotation law, from it its satellites' frame on its Laplace
//! surface, on which they are found again (P14.T56.a), and each moon's law on its plane.
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
//! - [`PlanetarySystem::state_at`]: that position bit for bit with the body's velocity beside it
//!   (added by rendering plan R03's R03.T3).
//! - [`PlanetarySystem::habitable_zone_at`]: Kopparapu et al.'s zone of a host's stars at the
//!   time, with the light of the system's other stars (P14.T12.b).
//!
//! What is not computed is said in every record rather than left to be read as "none" (ruling
//! 34): a gas-envelope body's surface is [`Section::NotApplicable`] and its `envelope`
//! `NotModelled` (P14.T48.e, until P14.T24.d), and a dwarf planet's rings `NotModelled`. Its
//! `hooks` hold the body's detail seed alone (the rendering plans' R09.T1.b): [`Section::Ok`] for
//! a planet, moon or dwarf planet present at the time, `NotApplicable` otherwise. Events on bodies
//! (P14.T31) are not built, so there is no `events_between`.
//!
//! # Streams
//!
//! [`tags::PLANET_RADIUS`], opened with the body's [`BodyId`]: word 0 is its radius rank, and
//! words 1–7 are reserved ([`RADIUS_WORDS`]). A planet and a moon read it; a belt member reads its
//! rank on `belt.member`. Every other draw is its stage's.

use core::f64::consts::TAU;

use crate::Seed;
use crate::coords::{SystemPosition, SystemVelocity};
use crate::id::{BodyId, SystemId};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::planetary::architecture::{
    ArchitectureClass, ClassConstraints, ZoneLimit, class_weights, draw_class,
};
use crate::planetary::belts::{
    Belt, BeltComposition, BeltHost, BeltMember, FIRST_BELT_SLOT, LAST_BELT_SLOT, host_belts,
};
use crate::planetary::chem::inventory::ElementInventory;
use crate::planetary::chem::partition::{Condensates, CrustWater, Partition};
use crate::planetary::chem::{AbundanceDraws, HostAbundances};
use crate::planetary::context::{SystemContext, XuvHistory};
use crate::planetary::derive::atmosphere::{Atmosphere, Gas, SurfaceMaterial, VolatileDraws};
use crate::planetary::derive::figure::{BodyFigure, FigureInputs};
use crate::planetary::derive::irradiation::{BondAlbedo, luminosity_flux};
use crate::planetary::derive::pass_record::{
    PassBulk, PassContext, PassRecord, PassRecordOf, RecordCrust, RecordSpin, WaterGate,
};
use crate::planetary::derive::photometry::area_mix::ShadeBook;
use crate::planetary::derive::photometry::{BodyPhotometry, PhotometryInputs};
use crate::planetary::derive::rotation::{
    BodyRotation, ObliquityLaw, SpinDraws, SpinInputs, SpinState, SpinTides,
};
use crate::planetary::derive::{
    BodyHosts, DerivedBody, DerivedBulk, GivenBulk, GivenFractions, HabitableZone, HostLight,
    Illumination, MassFractions, PassDriver, PlacedBody, PlanetClass, SnowLineSide, SpunPasses,
    derive_body, derive_body_driven, derive_bulk, habitable_zone_of, radius_chen_kipping,
};
use crate::planetary::disc::{self, Disc, DiscProfile, Truncation};
use crate::planetary::drift::{DriftingOrbit, drift_cell};
use crate::planetary::error::ResolveBodyError;
use crate::planetary::fate::{BodyFate, BodyState, FateAt, FateBody, FateHost, ScatterDraws};
use crate::planetary::frames::BodyFixedFrame;
use crate::planetary::halo::{
    ADIABATIC_INDEX_LIMIT, CometaryHalo, HaloBounds, HaloHost, RUNAWAY_INDEX, Scatterer, halo,
};
use crate::planetary::hooks::cold_traps::{self, ColdTrapRules, ColdTrapSky, RecordColdTraps};
use crate::planetary::hooks::crust::{
    CrustComposition, CrustLid, CrustOrigin, CrustRecord, FormationWindow, ParentSurface,
    formation_time,
};
use crate::planetary::hooks::figures::{
    BeltMembership, Continents, CraterInputs, FiguresInputs, HostOrbit, HostOrbiter, ImpactorOrbit,
    ImpactorSources, MoonBirth, PartitionGround, PassFigures, SourceBelt, VolatileHistory,
    VolatileHistoryInputs, WaterIce, WindowAt, WindowEntryInputs, pass_figures, past_outgassing,
    relief, window_entry,
};
use crate::planetary::hooks::surface::{
    ClimateInputs, HourCosines, SeasonalSpin, SeasonsFrom, SurfaceInputs, ZonalInsolation,
};
use crate::planetary::hooks::{
    self, BodyHooks, BulkComposition, GlobalFigures, Subsurface, SurfaceConditions, SurfaceSeed,
};
use crate::planetary::hosts::evolved::{Circularisation, EccentricityFloor};
use crate::planetary::hosts::young::{Formation, FormationDraws};
use crate::planetary::index::{BodyIndex, LAST_PLANET_SLOT};
use crate::planetary::label::{self, BodyLabel};
use crate::planetary::laplace::{LaplaceFrame, LaplaceFrameParts, hydrostatic_j2};
use crate::planetary::moons::regular::{
    DerivedMoon, MOON_ICE_DENSITY, MOON_ROCK_DENSITY, MoonBeforePasses, MoonBulk, MoonInputs,
    MoonNursery, MoonPasses, MoonRecordOf, MoonShape, MoonSky, MoonTideAt, derive_moon_under,
    moon_before_passes,
};
use crate::planetary::moons::regular::{ICY_MOON_ICE_FRACTION, moon_radius};
use crate::planetary::moons::{
    BeltAdjacency, MoonParent, MoonParentParts, NearestBelt, ParentKind,
};
use crate::planetary::params::{GIANT_LOVE_NUMBER, ROCKY_LOVE_NUMBER, SPACING_GIANT_MASS};
use crate::planetary::placement::classes::orbits::{HostPlane, SystemPlane, host_plane};
use crate::planetary::placement::classes::secular::{
    SecularPlanet, resonant_forced, surviving_mode,
};
use crate::planetary::placement::classes::tides::{
    GIANT_TIDAL_Q_PRIME, ROCKY_TIDAL_Q_PRIME, circularisation_time,
};
use crate::planetary::placement::zones::CLOSE_BINARY_SEMI_MAJOR_AXIS;
use crate::planetary::placement::{
    Neighbour, OrbitHost, OrbitZone, PlacedPlanet, PlacementHost, ZoneDiscInputs, place,
};
use crate::planetary::record::{
    BeltRecord, BodyIdentity, BodyKind, BodyOrbit, BodyRecord, BulkProperties, HaloRecord, Hooks,
    MoonOrigin, Population, Section, Surface, SurfaceBasis, SystemSnapshot,
};
use crate::planetary::rings::{Ring, RingParent};
use crate::planetary::satellites::{
    Satellite, SatelliteFrame, SatelliteMoon, Satellites, satellites_of,
};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::stellar::multiplicity::{HierarchyNode, SystemHierarchy};
use crate::stellar::system::StarModel;
use crate::stellar::{Phase, StarState};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::{
    GM_EARTH, GRAVITATIONAL_CONSTANT, METRES_PER_AU, SECONDS_PER_JULIAN_YEAR,
};
use crate::units::{
    Dex, EarthFluxes, EarthMasses, EarthRadii, GravitationalParameter, Kelvin, Kilograms,
    KilogramsPerCubicMetre, Megayears, Metres, MetresPerSecondSquared, Radians, Seconds,
    SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, WattsPerSquareMetre, Years,
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
    /// giant's core in time (D5's second fallback), or `Barren` without a disc or where nothing
    /// was placed (ruling 112.8).
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
    /// The rotation law, derived once by [`generate`] and held (P14.T46.b); `None` for a ring,
    /// for a body not present at [`parent_time`], and for every body [`generate_planets`] makes.
    rotation: Option<BodyRotation>,
    /// A planet's or member's satellites' frame on the Laplace surface, derived once with its
    /// rotation and held (P14.T56.a); `None` for a moon or a ring, and for a body without a
    /// rotation law.
    laplace: Option<LaplaceFrame>,
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
    volatiles: VolatileDraws,
    fate: FateBody,
}

impl PlanetPart {
    /// The planet as [`derive_body`] reads it, of mass `mass` on its primordial orbit: its radius
    /// rank, its volatile inventory's ranks (P14.T13.a) and the end of its young magma ocean
    /// (design note 12, P14.T28.a's `molten_until`).
    #[must_use]
    fn placed_body(&self, mass: EarthMasses) -> PlacedBody {
        let molten_until = Years::from(self.fate.formation().molten_until());
        PlacedBody::new(
            mass,
            *self.placed.orbit(),
            self.placed.formation_distance(),
            self.radius_rank,
        )
        .expect("a placed planet's mass and formation distance are positive")
        .with_volatiles(self.volatiles)
        .with_magma_ocean_until(molten_until)
    }
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
    /// lives `disc_lifetime`, circularising as `circularisation` says: its radius rank, its
    /// formation and its circularisation.
    #[must_use]
    fn planet(
        seed: Seed,
        system: SystemId,
        zone: &OrbitZone,
        placed: PlacedPlanet,
        circularisation: Circularisation,
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
        .with_circularisation(circularisation)
        .with_scatter_draws(ScatterDraws::Stream { seed, body: id });
        Self {
            index: placed.index(),
            host: zone.host(),
            part: Part::Planet(PlanetPart {
                placed,
                radius_rank: radius_rank(seed, id),
                volatiles: VolatileDraws::for_body(seed, id),
                fate,
            }),
            rotation: None,
            laplace: None,
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
            rotation: None,
            laplace: None,
        }
    }

    /// The moon `satellite` of `parent`.
    #[must_use]
    fn moon(satellite: &Satellite, parent: &MoonParent, parent_index: BodyIndex) -> Self {
        Self {
            index: satellite.index(),
            host: OrbitHost::Body(parent_index),
            part: Part::Moon(MoonPart {
                satellite: *satellite,
                parent: *parent,
                orbit: satellite.orbit_at(parent, Years::new(0.0)),
            }),
            rotation: None,
            laplace: None,
        }
    }

    /// The ring `ring` of the planet `planet`.
    #[must_use]
    fn ring(ring: Ring, planet: BodyIndex) -> Self {
        Self {
            index: ring.index(),
            host: OrbitHost::Body(planet),
            part: Part::Ring(ring),
            rotation: None,
            laplace: None,
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

    /// The frame its satellites lie in on the Laplace surface (P14.T56.a), held by [`generate`]:
    /// a planet's or member's with a rotation law; `None` otherwise.
    #[must_use]
    pub const fn laplace(&self) -> Option<&LaplaceFrame> {
        self.laplace.as_ref()
    }

    /// The record's rotation section (P14.T46.b): the held law's frame, or
    /// [`Section::NotModelled`] where none is held. The caller gives a body not present, a ring,
    /// a belt and the halo [`Section::NotApplicable`].
    #[must_use]
    fn rotation_section(&self) -> Section<BodyFixedFrame> {
        self.rotation.map_or(Section::NotModelled, |rotation| {
            Section::Ok(*rotation.frame())
        })
    }

    /// The record's figure section at `t` (P14.T46.d): the figure of the body's held rotation at
    /// `t` on its bulk `bulk` and mass `mass` then, or [`Section::NotModelled`] where either is
    /// missing. The caller gives a body not present, a ring, a belt and the halo
    /// [`Section::NotApplicable`].
    #[must_use]
    fn figure_section(
        &self,
        bulk: Option<&BulkProperties>,
        mass: EarthMasses,
        t: UniverseTime,
    ) -> Section<BodyFigure> {
        self.figure_of(bulk.map(SpinBulk::from), mass, t)
    }

    /// [`figure_section`](Self::figure_section) on what the figure reads of its bulk, `bulk`.
    #[must_use]
    fn figure_of(
        &self,
        bulk: Option<SpinBulk>,
        mass: EarthMasses,
        t: UniverseTime,
    ) -> Section<BodyFigure> {
        let (Some(rotation), Some(bulk)) = (self.rotation, bulk) else {
            return Section::NotModelled;
        };
        let frame = rotation.frame();
        Section::Ok(BodyFigure::derive(&FigureInputs {
            radius: Metres::from(bulk.radius),
            mass: Kilograms::from(mass),
            class: bulk.class,
            fractions: bulk.fractions,
            rate: frame.rate().rate_at(t),
            state: frame.state_at(t),
            pole: frame.pole(),
        }))
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

/// The photometry of a present body whose bulk is `bulk`, whose last atmosphere pass is
/// `atmosphere` and whose equilibrium temperature was taken at the albedo `albedo` (P14.T47.c):
/// its surface state with the material of its record's composition, and its Bond albedo `albedo`
/// itself, the area mix's, whose passes read the record's composition (P14.T51.b), so that the two
/// agree to the bit by construction; its template's reference is the area mix's
/// ([`TemplateRow::reference_bond`](crate::planetary::derive::photometry::TemplateRow::reference_bond)).
#[must_use]
fn photometry_of(
    albedo: BondAlbedo,
    atmosphere: &Atmosphere,
    bulk: &BulkProperties,
) -> BodyPhotometry {
    let fractions = bulk.fractions();
    BodyPhotometry::derive(&PhotometryInputs {
        state: atmosphere.state(),
        material: SurfaceMaterial::of(&fractions),
        surface_pressure: atmosphere.surface_pressure(),
        cloud_fraction: atmosphere.cloud_fraction(),
        surface_temperature: atmosphere.surface_temperature(),
        equilibrium_temperature: bulk.equilibrium_temperature(),
        surface_gravity: bulk.surface_gravity(),
        class: bulk.class(),
        fractions,
        bond: albedo,
    })
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

/// The bulk of `body` if it is an icy belt's member, derived as `derived` (ruling 112.8): ruling
/// 83.7's rock-and-ice model, which the icy moons take, in place of Chen and Kipping's radius
/// lottery, whose small bodies are rock and iron wherever they formed. Its ice fraction is
/// [`ICY_MOON_ICE_FRACTION`]'s 0.35–0.50 at its radius rank and its radius [`moon_radius`]'s; its
/// temperatures are the derivation's. `None` for anything else, a rocky belt's members included,
/// which stay dry.
#[must_use]
fn icy_member_bulk(body: &Body, derived: &DerivedBody) -> Option<BulkProperties> {
    icy_member_figure(body, derived.mass())
        .map(|bulk| bulk.with_temperature(derived.equilibrium_temperature()))
}

/// [`icy_member_bulk`]'s bulk of `body` at the mass `mass` its derivation gives it, with no
/// derivation's temperatures: what its passes read of it before them (P14.T54.a). `None` for
/// anything but an icy belt's member.
#[must_use]
fn icy_member_figure(body: &Body, mass: EarthMasses) -> Option<FigureBulk> {
    let (radius, fractions) = icy_member_shape(body, mass)?;
    let r = radius.value();
    let volume = 4.0 / 3.0 * core::f64::consts::PI * r * r * r;
    Some(FigureBulk {
        radius: EarthRadii::from(radius),
        density: KilogramsPerCubicMetre::new(Kilograms::from(mass).value() / volume),
        surface_gravity: MetresPerSecondSquared::new(GM_EARTH * mass.value() / (r * r)),
        class: PlanetClass::of(mass, &fractions),
        fractions,
    })
}

/// The radius and mass fractions ruling 112.8 gives `body` of mass `mass` if it is an icy belt's
/// member, [`icy_member_bulk`]'s: [`moon_radius`] at its ice fraction ([`icy_member_ice`]) and its
/// rock and ice. `None` for anything else.
#[must_use]
fn icy_member_shape(body: &Body, mass: EarthMasses) -> Option<(Metres, MassFractions)> {
    let ice = icy_member_ice(body)?;
    Some((
        moon_radius(mass, ice),
        MassFractions::solid(0.0, 1.0 - ice, ice),
    ))
}

/// What the rotation law and the figure read of [`icy_member_bulk`]'s bulk of `body` at the mass
/// `mass` its derivation gives it (lever (e)): the same radius, class and fractions, with no
/// derivation's temperatures. `None` for anything but an icy belt's member.
#[must_use]
fn icy_member_spin_bulk(body: &Body, mass: EarthMasses) -> Option<SpinBulk> {
    let (radius, fractions) = icy_member_shape(body, mass)?;
    Some(SpinBulk {
        radius: EarthRadii::from(radius),
        class: PlanetClass::of(mass, &fractions),
        fractions,
    })
}

/// The ice fraction of `body` if it is an icy belt's member (ruling 112.8): ruling 83.7's
/// [`ICY_MOON_ICE_FRACTION`], 0.35–0.50 at its radius rank, which its record's bulk states
/// ([`icy_member_bulk`]) and, from the 22 → 23 bump, its air and albedo read
/// ([`icy_member_given_bulk`]). `None` for anything else, a rocky belt's members included.
#[must_use]
fn icy_member_ice(body: &Body) -> Option<f64> {
    let Part::Member(member) = &body.part else {
        return None;
    };
    if member.member.composition() != BeltComposition::Icy {
        return None;
    }
    let (lo, hi) = ICY_MOON_ICE_FRACTION;
    Some(lo + member.member.radius_rank().value() * (hi - lo))
}

/// The bulk the derivation of `body`, of mass `mass` before derivation, reads for its inventory,
/// its air and its albedo if it is an icy belt's member (P14.T51.b, "The passes take the record's
/// composition"; decision-triton-and-e101 §1.7): its record's ([`icy_member_bulk`]), the radius
/// [`moon_radius`] gives its ice ([`icy_member_ice`]) at that mass and its rock-and-ice fractions,
/// known before the derivation. `None` for anything else.
#[must_use]
fn icy_member_given_bulk(body: &Body, mass: EarthMasses) -> Option<GivenBulk> {
    let ice = icy_member_ice(body)?;
    Some(GivenBulk::new(
        moon_radius(mass, ice),
        GivenFractions::Exactly(MassFractions::solid(0.0, 1.0 - ice, ice)),
    ))
}

/// `placed`, the placement of `body`: with its record's bulk ([`icy_member_given_bulk`]) for an
/// icy belt's member, so that its air and albedo read the material its bulk section states
/// (P14.T51.b), and as it is otherwise.
#[must_use]
fn with_record_bulk(body: &Body, placed: &PlacedBody) -> PlacedBody {
    match icy_member_given_bulk(body, placed.mass()) {
        Some(bulk) => placed.with_bulk(bulk),
        None => *placed,
    }
}

/// The circularisations of a host's planets `planets`, in slot order, about a host of initial
/// mass `host` in a system aged `age` at the epoch (P14.T8.e; ruling 133.1).
///
/// Each planet damps on P14.T8.e's time at its primordial orbit, with Chen and Kipping's median
/// radius and a giant's modified tidal quality factor from [`SPACING_GIANT_MASS`] (design note 7's
/// giant) and a rocky planet's below (ruling 62.5). It stops at its floor:
///
/// - **Secular:** of every sibling k, the pair's surviving mode ([`surviving_mode`], the planet's
///   Love number `k₂` from the same giant boundary) gives ρ; the dominant neighbour is the one of
///   the largest ρ `e_k`, `e_k` its primordial eccentricity decayed by its own tides to the epoch,
///   which only primordial values fix, so the choice does not depend on order. The floor is
///   ρ `e_k0` at formation and decays with the mode at γ = 1/`τ_k` + Σ `s_j`/`τ_j` over the planets
///   k anchors, whose drain k's own eccentricity also takes.
/// - **Resonant:** a pair the placer snapped to a first-order commensurability holds its
///   near-resonant forced eccentricities ([`resonant_forced`]) for good, the inner planet taken as
///   the nearest inside the outer one.
#[must_use]
pub(crate) fn host_circularisations(
    planets: &[PlacedPlanet],
    host: SolarMasses,
    age: Years,
) -> Vec<Circularisation> {
    let giant = |p: &PlacedPlanet| p.mass() >= EarthMasses::from(SPACING_GIANT_MASS);
    let taus: Vec<Years> = planets
        .iter()
        .map(|p| {
            let q_prime = if giant(p) {
                GIANT_TIDAL_Q_PRIME
            } else {
                ROCKY_TIDAL_Q_PRIME
            };
            let orbit = p.orbit();
            circularisation_time(
                p.mass(),
                primordial_radius(p.mass()),
                host,
                orbit.semi_major_axis(),
                orbit.period(),
                q_prime,
            )
        })
        .collect();
    let secular: Vec<SecularPlanet> = planets
        .iter()
        .map(|p| {
            let love = if giant(p) {
                GIANT_LOVE_NUMBER
            } else {
                ROCKY_LOVE_NUMBER
            };
            SecularPlanet::new(
                p.mass(),
                p.orbit().semi_major_axis(),
                primordial_radius(p.mass()),
                love,
            )
        })
        .collect();
    let e0 = |k: usize| planets[k].orbit().eccentricity().value();
    let at_epoch = age.value().max(0.0);
    // Each planet's dominant neighbour: (its position, ρ, the planet's share of the mode).
    let dominant: Vec<Option<(usize, f64, f64)>> = (0..planets.len())
        .map(|j| {
            let mut best: Option<(usize, f64, f64, f64)> = None;
            for k in (0..planets.len()).filter(|&k| k != j) {
                let Some(mode) = surviving_mode(host, &secular[j], &secular[k]) else {
                    continue;
                };
                let now = mode.ratio() * e0(k) * math::exp(-at_epoch / taus[k].value());
                if best.is_none_or(|(.., held)| now > held) {
                    best = Some((k, mode.ratio(), mode.share(), now));
                }
            }
            best.map(|(k, ratio, share, _)| (k, ratio, share))
        })
        .collect();
    let mut drains = vec![0.0; planets.len()];
    for (j, found) in dominant.iter().enumerate() {
        if let Some((k, _, share)) = *found {
            drains[k] += share / taus[j].value();
        }
    }
    let mut resonant = vec![0.0_f64; planets.len()];
    let mut order: Vec<usize> = (0..planets.len()).collect();
    order.sort_by(|&x, &y| {
        planets[x]
            .orbit()
            .semi_major_axis()
            .value()
            .total_cmp(&planets[y].orbit().semi_major_axis().value())
    });
    for pair in order.windows(2) {
        let (i, o) = (pair[0], pair[1]);
        let Some(resonance) = planets[o].resonance() else {
            continue;
        };
        let body = |k: usize| (planets[k].mass(), planets[k].orbit().semi_major_axis());
        if let Some((inner, outer)) = resonant_forced(
            host,
            body(i),
            body(o),
            resonance.commensurability(),
            resonance.offset(),
        ) {
            resonant[i] = resonant[i].max(inner);
            resonant[o] = resonant[o].max(outer);
        }
    }
    (0..planets.len())
        .map(|j| {
            let (secular_floor, decay) = dominant[j].map_or((0.0, 0.0), |(k, ratio, _)| {
                (ratio * e0(k), 1.0 / taus[k].value() + drains[k])
            });
            Circularisation::new(taus[j])
                .expect("a damping time of positive quantities is positive")
                .with_floor(EccentricityFloor::new(secular_floor, decay, resonant[j]))
                .with_drain(drains[j])
        })
        .collect()
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
    seed: Seed,
    system: SystemId,
    zones: Vec<OrbitZone>,
    hosts: Vec<PlanetaryHost>,
    bodies: Vec<Body>,
    satellites: Vec<Satellites>,
    belts: Vec<Belt>,
    halo: Option<SystemHalo>,
    /// Which rules its records' cold traps follow (P14.T51.f): those in force, which the tests may
    /// change ([`with_record_rules`](Self::with_record_rules)).
    record: RecordRules,
}

/// The rules a system's records' cold traps follow (P14.T51.f): the generator version's, and for
/// the tests the batch's ahead of its bump. P14.T54.b deletes it with the holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordRules {
    /// Its cold traps: their hold at 24.
    pub(crate) cold_traps: ColdTrapRules,
}

impl RecordRules {
    /// The rules this generator version gives.
    #[must_use]
    pub(crate) fn in_force() -> Self {
        Self {
            cold_traps: ColdTrapRules::in_force(),
        }
    }
}

/// A system's cometary halo and the orbit host it surrounds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SystemHalo {
    halo: CometaryHalo,
    host: OrbitHost,
    /// The nearest wide companion's orbit, as its pair was born: the pair's components, a bit per
    /// body index, and the orbit's pericentre (ruling 112.6).
    companion: Option<(u32, KeplerElements)>,
}

/// The system of `ctx` in the universe of `seed`, whole (P14.T30.a): its planets
/// ([`generate_planets`]), then its belts with their members, every planet's and member's
/// satellites, and its cometary halo (the [module](self) documentation, steps 6–8).
///
/// Each stage after the planets has its own streams and slots, so a system's planets are exactly
/// [`generate_planets`]'s.
///
/// Every satellite lies on its parent's Laplace surface (P14.T56.a), so the satellites are found
/// twice. Step 7 finds them in their parents' orbital planes, which fixes whether a planet or
/// member had a giant impact, all its rotation law reads of them. The rotation pass then derives
/// every planet's and member's law, and from it and its record at the moon generators' time (the
/// epoch, or for a system not yet born then the clock window's end) its [`LaplaceFrame`]
/// ([`Body::laplace`]). Each one's satellites are found again on its frame, which drops the
/// regular moons and rings of the eccentricity band and moves no giant-impact moon, so its law
/// stands. Last, each moon's law is derived on its plane, on its own derivation at that time,
/// since its own tides (P14.T56.d) read its air and heat.
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
            &BeltHost::new(zone.host(), profile, &neighbours, host.plane),
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

    // Step 7: satellites of every planet and every member, their parents read at `parent_time`
    // under one book of their shades (lever (d)), which the rotation pass and the moons' laws
    // below share.
    let then = Epoch::new(&system, ctx, parent_time(ctx)).with_shades();
    let mut satellites = Vec::new();
    let mut sites = Vec::new();
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
        let site = SatelliteSite {
            index: parent.index(),
            parent: parent_at_epoch(&then, zone, profile, parent),
            belt,
        };
        satellites.push(site.find(seed, ctx, SatelliteFrame::OrbitalPlane));
        sites.push(site);
    }

    // Step 8: the cometary halo.
    system.halo = system_halo(seed, ctx, &system, scatterer);

    system.bodies.extend(members);
    system.bodies.extend(satellite_bodies(&satellites));
    system.satellites = satellites;
    system.bodies.sort_by_key(Body::index);

    // P14.T46.b: each body's rotation law, derived once and held, so that it is the same at every
    // record time. The pass reads only each body's mass, orbit and bulk at `parent_time` and its
    // elements then; the records it reads have no rotation yet, which is attached after
    // (decision-p14-phase-j A2). Each body's law is a pure function of the system without any
    // law, so the order of the pass changes nothing. What it reads of each record is its
    // derivation's before the passes (lever (e1)), which a snapshot then would state to the bit
    // and which equals `body_at` body by body (decision-p14-phase-j A3).
    let records = system.records_at_parent_time(&then);
    let rotations: Vec<Option<BodyRotation>> = system
        .bodies
        .iter()
        .zip(&records)
        .map(|(body, record)| {
            // A moon's law waits for its plane, which its parent's sets, and for its derivation on
            // it, which its own tides read.
            if matches!(body.part, Part::Moon(_)) {
                return None;
            }
            record
                .as_ref()
                .and_then(|record| system.rotation_then(ctx, body, record))
        })
        .collect();
    for (body, rotation) in system.bodies.iter_mut().zip(rotations) {
        body.rotation = rotation;
    }
    system.place_on_laplace_surfaces(seed, &then, &records, &sites);
    system
}

/// The moons and rings of `found` as bodies, each parent's moons and then its rings.
#[must_use]
fn satellite_bodies(found: &[Satellites]) -> Vec<Body> {
    let mut bodies = Vec::new();
    for satellites in found {
        if let Some(parent) = satellites.parent() {
            bodies.extend(
                satellites
                    .moons()
                    .iter()
                    .map(|moon| Body::moon(moon, parent, satellites.parent_index())),
            );
        }
        bodies.extend(
            satellites
                .rings()
                .iter()
                .map(|ring| Body::ring(ring.clone(), satellites.parent_index())),
        );
    }
    bodies
}

/// What a planet's or member's satellites are found from in [`generate`]'s step 7, kept so
/// that they can be found again on its Laplace surface (P14.T56.a).
#[derive(Debug, Clone)]
struct SatelliteSite {
    /// The parent's index.
    index: BodyIndex,
    /// The parent as the moon generators and its rings read it ([`parent_at_epoch`]), if it can
    /// be read.
    parent: Option<(MoonParent, Option<RingParent>)>,
    /// The belt next to it, which a rocky planet's captures read.
    belt: Option<NearestBelt>,
}

impl SatelliteSite {
    /// The parent's satellites, in the planes `frame` gives ([`satellites_of`]).
    #[must_use]
    fn find(&self, seed: Seed, ctx: &SystemContext, frame: SatelliteFrame<'_>) -> Satellites {
        self.parent.as_ref().map_or_else(
            || Satellites::none(self.index),
            |(moon_parent, ring_parent)| {
                satellites_of(
                    seed,
                    self.index,
                    moon_parent,
                    ring_parent.as_ref(),
                    self.belt,
                    ctx.age_at_epoch(),
                    frame,
                )
            },
        )
    }
}

/// The planets of the system of `ctx` in the universe of `seed`, with no satellites, belts or halo
/// (P14.T30.a): zones, then per zone its disc, class and placement, then each planet's own draws
/// (the [module](self) documentation, steps 1–5).
///
/// It holds no rotation law, so its snapshots derive every body over the passes' stand-in: no
/// rotation law, no basins, the bulk material's regolith; a system [`generate`] makes derives its
/// records under each body's law, its basins and its lid (P14.T54.a). Its records' surfaces are
/// not modelled.
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
        let constraints = ClassConstraints::new(&disc, reach);
        let weights = class_weights(zone.host_mass(), ctx.fe_h());
        let drawn = draw_class(seed, id, zone.host_number(), &weights, &constraints);
        let plane = plane_of(hierarchy, &under, zone);
        let class = if slot > LAST_PLANET_SLOT {
            // Every primordial slot is taken: the host keeps its disc and places nothing, rather
            // than spilling into the second-generation block, and its placed class says so
            // (ruling 112.8).
            ArchitectureClass::Barren
        } else {
            let host = PlacementHost::new(zone.host_number(), *inputs.host(), plane);
            let placement = place(seed, id, &host, limits, &disc, drawn, slot);
            slot = placement.next_slot();
            let circularisations =
                host_circularisations(placement.planets(), zone.host_mass(), ctx.age_at_epoch());
            bodies.extend(placement.planets().iter().zip(circularisations).map(
                |(&planet, circularisation)| {
                    Body::planet(seed, id, zone, planet, circularisation, inputs.lifetime())
                },
            ));
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
        seed,
        system: id,
        zones,
        hosts,
        bodies,
        satellites: Vec::new(),
        belts: Vec::new(),
        halo: None,
        record: RecordRules::in_force(),
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
/// [`parent_time`], `epoch`'s time, on its primordial orbit about its zone's initial mass. `None`
/// for a moon or a ring, and for a body whose system is not born even then.
///
/// At [`generate`]'s step 7 no body holds its law yet, so a planet's derivation here is the passes'
/// stand-in: no rotation law, no basins, the bulk material's regolith; a system's record derives
/// the body under its law, its basins and its lid (P14.T54.a). Its moons' parent reads nothing a
/// pass gives; its rings' parent reads its effective or equilibrium temperature, the stand-in's,
/// which for a giant, whose envelope its passes never read their record through, is its record
/// passes' to the bit (`an_envelope_body_s_passes_over_its_record_are_its_stand_in_s`).
#[must_use]
fn parent_at_epoch(
    epoch: &Epoch<'_>,
    zone: &OrbitZone,
    profile: &DiscProfile,
    body: &Body,
) -> Option<(MoonParent, Option<RingParent>)> {
    let (placed, kind) = match &body.part {
        Part::Planet(planet) => (planet.placed_body(planet.placed.mass()), ParentKind::Planet),
        Part::Member(member) => (member.member.placed_body(), ParentKind::DwarfPlanet),
        Part::Moon(_) | Part::Ring(_) => return None,
    };
    let (ctx, t) = (epoch.ctx, epoch.t);
    debug_assert!(
        t == parent_time(ctx),
        "the moon generators read their parents at `parent_time`"
    );
    let orbited = epoch.lights(zone)?;
    let companions = epoch.companions(zone)?;
    let host_mass = Kilograms::from(zone.host_mass());
    // Each `.ok()` below turns a parent the moon generators cannot read into "no satellites",
    // as for a system not born, and the system's other stages go on; none is an error a caller
    // could act on. `BodyHosts::new` fails only for no orbited light or a zone mass that is not
    // positive, which a zone with lights at `t` never has; `derive_body` and `derive_bulk` for a
    // system whose age at `t` is not positive (born after the clock window's end) or a giant
    // above 13 Jupiter masses, which placement never makes; `MoonParent::new` and
    // `RingParent::new` only for a mass, radius or temperature that is not positive and finite.
    let hosts = BodyHosts::new(host_mass, *ctx.composition(), &orbited, &companions)
        .ok()?
        .with_shades(epoch.shades.as_ref());
    // An icy member's bulk moves only its air, albedo and temperatures, none of which its moons'
    // parent reads (its mass, its bulk's radius and class, its moon limit and its orbit).
    let placed = with_record_bulk(body, &placed);
    let age = ctx.age_at_epoch();
    let id = body.index.body_id(ctx.id());
    match kind {
        // A planet's rings read its effective or equilibrium temperature, which its passes give.
        ParentKind::Planet => {
            let derived = derive_body(&placed, &hosts, profile, age, t).ok()?;
            let bulk = DerivedBulk::from(&derived);
            let parent = moon_parent(body, (id, kind), &bulk, &placed, host_mass)?;
            let rings = RingParent::new(
                body.index,
                derived.class(),
                Kilograms::from(derived.mass()),
                Metres::from(derived.radius()),
                derived
                    .effective_temperature()
                    .unwrap_or_else(|| derived.equilibrium_temperature()),
                derived.flux(),
            )
            .ok();
            Some((parent, rings))
        }
        // A member's moons read nothing a pass gives (lever (e2)): its bulk before its passes.
        ParentKind::DwarfPlanet => {
            let before_passes = || derive_bulk(&placed, &hosts, profile, age, t).ok();
            // The tests' switch reads its whole derivation's instead, or checks the two agree.
            #[cfg(test)]
            let before_passes = || {
                pass_free_read(PassFreeRead::Member, before_passes, || {
                    derive_body(&placed, &hosts, profile, age, t)
                        .ok()
                        .map(|derived| DerivedBulk::from(&derived))
                })
                .unwrap_or_else(before_passes)
            };
            let bulk = before_passes()?;
            Some((
                moon_parent(body, (id, kind), &bulk, &placed, host_mass)?,
                None,
            ))
        }
    }
}

/// The moons' parent `body`, of ID and kind `id_kind`, derived at [`parent_time`] as `bulk` says
/// and placed as `placed` about its zone's initial mass `host_mass`: its mass, its moon limit and
/// its orbit, with its bulk's radius and class, ruling 112.8's rock and ice for an icy belt's
/// member ([`icy_member_spin_bulk`]) and its derivation's otherwise, formed where ruling 112.4
/// says. `None` for a mass or radius that is not positive and finite, which a derivation never
/// gives.
#[must_use]
fn moon_parent(
    body: &Body,
    (id, kind): (BodyId, ParentKind),
    bulk: &DerivedBulk,
    placed: &PlacedBody,
    host_mass: Kilograms,
) -> Option<MoonParent> {
    let (radius, class) = icy_member_spin_bulk(body, bulk.mass()).map_or_else(
        || (Metres::from(bulk.radius()), bulk.class()),
        |icy| (Metres::from(icy.radius), icy.class),
    );
    MoonParent::new(MoonParentParts {
        id,
        kind,
        mass: bulk.mass(),
        radius,
        class,
        orbit: *placed.orbit(),
        host_mass,
        maximum_moon_mass: bulk.maximum_moon_mass(),
    })
    .ok()
    // Ruling 112.4: a migrated giant captured its irregulars where it formed.
    .map(|parent| parent.with_formation_distance(placed.formation_distance()))
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
    let epoch = Epoch::of(ctx, parent_time(ctx)).with_shades();
    parent_at_epoch(&epoch, zone, disc.profile()?, body)
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
        .map(|(node, orbit)| (under[usize::from(node.get())], *orbit));
    let halo_host = HaloHost::new(
        zone.host_mass(),
        host.disc.solid_mass(),
        ctx.strip_radius(),
        companion.map(|(_, orbit)| orbit.periapsis()),
    );
    halo(seed, system.system, &halo_host, scatterer).map(|halo| SystemHalo {
        halo,
        host: zone.host(),
        companion,
    })
}

/// The bounds of `halo`, the halo of the system of `ctx`, at `t` (rulings 112.6 and 133.4): 0.49
/// of the sphere of influence of the system's present mass, the tidal radius scaling as the cube
/// root of the mass, and the nearest wide companion's present pericentre. A star with no state at
/// `t` counts at its initial mass.
///
/// The companion's pericentre is q₀ (M₀/M)^(1−w) of its pair's masses, w = clamp(log(Ψ/0.1) ÷
/// log 30, 0, 1) the regime weight ruling 84.4 gives the halo (ours), Ψ Veras et al.'s (2011,
/// eq. 15) index of the pair's own orbit at its stars' fastest mass-loss rate: exact when the loss
/// is adiabatic (the orbit widens by M₀/M, their eq. 18) and the lower bound when it is impulsive
/// (the pericentre never shrinks, their eq. 21). A supernova in the pair unbinds it when the mass
/// left, M/M₀, falls below r ÷ 2a at the explosion (their eq. 39), r from the orbit's phase then;
/// the pair then cuts nothing. The orbit is the hierarchy's primordial one, and kicks are plan
/// 11's.
#[must_use]
fn present_halo_bounds(ctx: &SystemContext, halo: &SystemHalo, t: UniverseTime) -> HaloBounds {
    let stars = ctx.stars();
    let members = |bits: u32| {
        stars
            .iter()
            .zip(0_u32..32)
            .filter(move |&(_, i)| bits & (1 << i) != 0)
            .map(|(star, _)| star)
    };
    let mass_of = |bits: u32| {
        members(bits).fold((0.0, 0.0), |(initial, now), star| {
            let m0 = star.initial_mass().value();
            let m = star.state_at(t).map_or(m0, |state| state.mass().value());
            (initial + m0, now + m)
        })
    };
    let (initial, now) = mass_of(u32::MAX);
    let strip = if initial > 0.0 && now > 0.0 && now < initial {
        ctx.strip_radius() * math::cbrt(now / initial)
    } else {
        ctx.strip_radius()
    };
    let companion = halo.companion.and_then(|(pair, orbit)| {
        let (initial, now) = mass_of(pair);
        let pericentre = orbit.periapsis();
        if !(now > 0.0 && now < initial) {
            return Some(pericentre);
        }
        let kept = now / initial;
        let a = orbit.semi_major_axis();
        let explosions: Vec<UniverseTime> = members(pair)
            .filter_map(|star| {
                let death = star.death()?;
                let age_now = star.age_at(t).value();
                (death.kind().is_sudden() && death.age().value() <= age_now)
                    .then(|| time_at_age(death.age(), star.age_at_epoch()))
                    .flatten()
            })
            .collect();
        if !explosions.is_empty() {
            let unbound = explosions.iter().any(|&at| {
                let r = orbit.relative_state_at(at).0.length();
                kept < r.value() / (2.0 * a.value())
            });
            return (!unbound).then_some(pericentre);
        }
        let rate = members(pair)
            .map(|star| fastest_mass_loss_rate(star, t).value())
            .fold(0.0, f64::max);
        let a_au = a.value() / METRES_PER_AU;
        let psi = rate * math::powf(a_au, 1.5) * math::powf(initial, -1.5) / TAU;
        Some(present_pericentre(pericentre, kept, psi))
    });
    HaloBounds::new(strip, companion)
}

/// A companion's pericentre `pericentre` after its pair kept `kept` of its mass at Veras et al.'s
/// index `psi` (ruling 133.4): q₀ (M₀/M)^(1−w), w = clamp(log(Ψ/0.1) ÷ log 30, 0, 1).
#[must_use]
fn present_pericentre(pericentre: Metres, kept: f64, psi: f64) -> Metres {
    let w = if psi > 0.0 {
        (math::ln(psi / ADIABATIC_INDEX_LIMIT) / math::ln(RUNAWAY_INDEX / ADIABATIC_INDEX_LIMIT))
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    pericentre * math::powf(1.0 / kept, 1.0 - w)
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

/// The orbit of the lowest pair of `h` that holds both the stars `members` and the star `star`,
/// with `under` each node's stars: the separation along which `star`'s light reaches the bodies
/// that go round `members` (design note 10, P14.T12.a).
///
/// # Panics
///
/// If no pair holds both, which the root pair of a multiple system always does.
#[must_use]
fn companion_separation<'h>(
    h: &'h SystemHierarchy,
    under: &[u32],
    members: u32,
    star: usize,
) -> &'h KeplerElements {
    let both = members | 1 << star;
    h.pairs()
        .filter(|(node, _)| {
            let under = under[usize::from(node.get())];
            (under & both) == both
        })
        .min_by_key(|(node, _)| under[usize::from(node.get())].count_ones())
        .map(|(_, orbit)| orbit)
        .expect("the root pair holds every star of a multiple system")
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

    /// The same system with its records' cold traps under `rules` (P14.T51.i), for
    /// the tests and the cold traps' probe to reach 24's record path ahead of the bump. P14.T54.b
    /// deletes it with the holds.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_record_rules(self, rules: RecordRules) -> Self {
        Self {
            record: rules,
            ..self
        }
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
    /// P14.T22.a): its primordial mass, whether another of the host's planets lies between them,
    /// and its composition, which a rocky planet's captures are born of (science-p14-t24b-laws
    /// §2.4). `None` for a body that is not a planet, or a host with no belt.
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
        NearestBelt::new(belt.initial_mass(), adjacency, belt.composition())
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
        let epoch = Epoch::new(self, ctx, t).with_shades();
        // Once for the whole snapshot, every body's zonal profiles reading them (lever (h)).
        let cosines = HourCosines::new();
        let labels: Vec<(BodyIndex, BodyLabel)> = label::labels(self);
        let label_of = |index: BodyIndex| {
            labels
                .binary_search_by_key(&index, |(at, _)| *at)
                .map(|at| labels[at].1.clone())
                .expect("every body of a system has a label")
        };
        let hosts: Vec<FateHost<'_>> = self.zones.iter().map(|zone| fate_host(ctx, zone)).collect();
        let fates = self.fates(&hosts);
        let mut records = Vec::with_capacity(self.bodies.len() + self.belts.len() + 1);
        let mut parents: Vec<(BodyIndex, ParentNow, &BodyFate<'_>)> = Vec::new();
        for (body, fate) in self.bodies.iter().zip(&fates) {
            let label = label_of(body.index);
            match &body.part {
                Part::Planet(_) | Part::Member(_) => {
                    let fate = fate
                        .as_ref()
                        .expect("every planet and member orbits one of its system's zones");
                    let (record, now) = self.primary_record(&epoch, &cosines, body, label, fate);
                    records.push(record);
                    parents.push((body.index, now, fate));
                }
                Part::Moon(_) | Part::Ring(_) => {
                    let OrbitHost::Body(parent) = body.host else {
                        unreachable!("a moon's and a ring's host is a body")
                    };
                    let (now, fate) = parents
                        .iter()
                        .find(|(at, ..)| *at == parent)
                        .map(|(_, now, fate)| (now, *fate))
                        .expect("a satellite's parent comes before it in index order");
                    records.push(self.satellite_record(&epoch, &cosines, body, label, now, fate));
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

    /// Every body's history about the hosts `hosts` of its system's zones (in zone order), in body
    /// order: each host's planets resolved together, for the scattering after a supernova (ruling
    /// 71), and each exactly once; a member alone about its host; `None` for a moon and a ring,
    /// which have none of their own.
    #[must_use]
    fn fates<'s>(&'s self, hosts: &'s [FateHost<'s>]) -> Vec<Option<BodyFate<'s>>> {
        let mut fates: Vec<Option<BodyFate<'_>>> = vec![None; self.bodies.len()];
        for (zone, host) in self.zones.iter().zip(hosts) {
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
        fates
    }

    /// What every body's record at [`parent_time`], `epoch`'s time, holds for the rotation pass and
    /// the Laplace frames ([`RecordThen`]), in body order: `None` for a moon, a ring and a body not
    /// present then.
    ///
    /// A planet's or a member's comes from its derivation's stage before the passes and its
    /// record's bulk rule, an icy member's included (lever (e1), decision-p14-t54a-passes, "Cost
    /// escalation (after Batch B)" §C.3; [`records_before_passes`](Self::records_before_passes)):
    /// no passes, surfaces or moons' records, and the bits a snapshot then would state.
    #[must_use]
    fn records_at_parent_time(&self, epoch: &Epoch<'_>) -> Vec<Option<RecordThen>> {
        #[cfg(test)]
        if let Some(read) = pass_free_read(
            PassFreeRead::Records,
            || self.records_before_passes(epoch),
            || {
                let snapshot = self.snapshot_at(epoch.ctx, epoch.t);
                self.bodies
                    .iter()
                    .map(|body| match body.part {
                        Part::Planet(_) | Part::Member(_) => RecordThen::of_record(
                            snapshot
                                .body(body.index)
                                .expect("a snapshot holds every body of its system"),
                        ),
                        Part::Moon(_) | Part::Ring(_) => None,
                    })
                    .collect()
            },
        ) {
            return read;
        }
        self.records_before_passes(epoch)
    }

    /// [`records_at_parent_time`](Self::records_at_parent_time)'s records, each planet and member
    /// present then read off its derivation before its passes.
    #[must_use]
    fn records_before_passes(&self, epoch: &Epoch<'_>) -> Vec<Option<RecordThen>> {
        let hosts: Vec<FateHost<'_>> = self
            .zones
            .iter()
            .map(|zone| fate_host(epoch.ctx, zone))
            .collect();
        let fates = self.fates(&hosts);
        self.bodies
            .iter()
            .zip(&fates)
            .map(|(body, fate)| {
                let fate = fate.as_ref()?;
                let at = fate.at(epoch.t);
                match at.state() {
                    BodyState::Present => {}
                    BodyState::NotYetFormed
                    | BodyState::Destroyed { .. }
                    | BodyState::Unbound { .. } => return None,
                }
                let orbit = at.orbit().expect("a present body has an orbit");
                let section = at
                    .body_orbit()
                    .expect("a present body has an orbit section");
                let placed = Self::placed_now(body, orbit, at.mass());
                let derived = self.derive_before_passes(
                    epoch,
                    self.zone_of(body),
                    &placed,
                    fate.host_mass(epoch.t),
                );
                let bulk = icy_member_spin_bulk(body, derived.mass()).unwrap_or(SpinBulk {
                    radius: derived.radius(),
                    class: derived.class(),
                    fractions: derived.fractions(),
                });
                // A present body's record states its derivation's mass, escape's loss taken.
                Some(RecordThen {
                    orbit: *section.elements(),
                    bulk,
                    mass: derived.mass(),
                })
            })
            .collect()
    }

    /// [`derive`](Self::derive)'s derivation of the present planet or member of `zone` at the
    /// epoch's time, placed as `placed` says then, about hosts of total mass `host_mass`, before
    /// its passes and without them ([`derive_bulk`], lever (e1)).
    #[must_use]
    fn derive_before_passes(
        &self,
        epoch: &Epoch<'_>,
        zone: &OrbitZone,
        placed: &PlacedBody,
        host_mass: SolarMasses,
    ) -> DerivedBulk {
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
        .expect("a present body orbits a positive mass and at least one star")
        .with_shades(epoch.shades.as_ref());
        derive_bulk(placed, &hosts, disc, epoch.ctx.age_at_epoch(), epoch.t).expect(
            "a present body's system is born, and a placed planet lies below 13 Jupiter masses",
        )
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
    ///   its surface section (P14.T48.e): its surface conditions and global figures for a body
    ///   with a solid surface, [`Section::NotModelled`] where it has no rotation law, and for a
    ///   gas-envelope body, a sub-Neptune or a giant, [`Section::NotApplicable`] with its
    ///   envelope [`Section::NotModelled`]. A ring, a belt and a halo carry their extent as their
    ///   population section instead.
    /// - For a body not present: no position, and its orbit, bulk, surface, envelope, hooks and
    ///   population [`Section::NotApplicable`], since nothing orbits there at `t`.
    /// - A planet's moons and rings [`Section::Ok`], lists that may be empty. The hooks section of
    ///   a planet, moon or dwarf planet present at `t` is [`Section::Ok`], holding its detail seed
    ///   ([`hooks::detail_seed`]; the rendering plans' R09.T1.b), and a population's
    ///   `NotApplicable`.
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
        let epoch = Epoch::new(self, ctx, t).with_shades();
        let cosines = HourCosines::new();
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
                let (_, now) = self.primary_record(&epoch, &cosines, parent, parent_label, &fate);
                Ok(self.satellite_record(&epoch, &cosines, body, label, &now, &fate))
            }
            OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => {
                let host = fate_host(ctx, self.zone_of(body));
                let fate = self.fate_of(body, &host);
                Ok(self.primary_record(&epoch, &cosines, body, label, &fate).0)
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
        let Some(centre) = fate
            .trajectory()
            .map(|trajectory| epoch.position(zone, trajectory))
        else {
            return Ok(None);
        };
        Ok(match &body.part {
            Part::Planet(_) | Part::Member(_) | Part::Ring(_) => Some(centre),
            Part::Moon(moon) => {
                match moon_state(fate.state(), &moon.satellite, ctx.age_at_epoch(), t) {
                    BodyState::Present => {
                        let (orbit, _) = moon_trajectory(ctx, moon, t, fate.changes_at());
                        Some(centre.translated(orbit.relative_state_at(t).0))
                    }
                    BodyState::NotYetFormed
                    | BodyState::Destroyed { .. }
                    | BodyState::Unbound { .. } => None,
                }
            }
        })
    }

    /// Where body `index` is at `t` and how fast it moves: [`position_at`](Self::position_at)'s
    /// position bit for bit, with the body's velocity relative to the system's barycentre, m/s
    /// along the galactic axes (asked of plan 14 by rendering plan R03, and added by R03.T3).
    ///
    /// The velocity follows the position's construction: the host's barycentric velocity on plan
    /// 11's walk ([`star_states_at`](crate::stellar::multiplicity::star_states_at)), plus the
    /// body's Kepler velocity about it, and for a moon its planet's plus its own relative velocity.
    /// For an evolving orbit it is the model's (P14.T45.a and T45.e): the time derivative of the
    /// model's position, the Kepler velocity at the model's elements then with the mean motion
    /// nᵣ + ṅ Δt plus ȧ ∂r/∂a + ė ∂r/∂e, so that a matched ship or an extrapolated state stays
    /// with the body. That velocity is the position's derivative within a record, and it may step
    /// at the record's `valid_until` where the host's Ṁ jumps, by up to ȧ, a few m/s on a late-AGB
    /// superwind host (P14.T45.e). `None` where `position_at` is.
    ///
    /// # Errors
    ///
    /// As [`body_at`](Self::body_at).
    ///
    /// # Panics
    ///
    /// As [`snapshot_at`](Self::snapshot_at).
    pub fn state_at(
        &self,
        ctx: &SystemContext,
        index: BodyIndex,
        t: UniverseTime,
    ) -> Result<Option<(SystemPosition, SystemVelocity)>, ResolveBodyError> {
        let Some(position) = self.position_at(ctx, index, t)? else {
            return Ok(None);
        };
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
        let trajectory = fate
            .trajectory()
            .expect("a body with a position has its primary's orbit then");
        let centre = epoch.velocity(zone, trajectory);
        let velocity = match &body.part {
            Part::Planet(_) | Part::Member(_) | Part::Ring(_) => centre,
            Part::Moon(moon) => {
                let (orbit, _) = moon_trajectory(ctx, moon, t, fate.changes_at());
                centre + orbit.relative_state_at(t).1
            }
        };
        Ok(Some((position, velocity)))
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

    /// The surface seed of body `index` (P14.T23, [`hooks::surface_seed`]): a planet's, a
    /// moon's or a dwarf planet's, whatever happens to it; `None` for a ring, a belt or the halo,
    /// which have no surface. Server-only (the [`hooks`] module
    /// documentation).
    ///
    /// # Errors
    ///
    /// [`ResolveBodyError::NoSuchBody`] if the system holds no body `index`.
    pub fn surface_seed(&self, index: BodyIndex) -> Result<Option<SurfaceSeed>, ResolveBodyError> {
        if self.belt(index).is_some() || self.halo().is_some_and(|halo| halo.index() == index) {
            return Ok(None);
        }
        let body = self.body(index).ok_or(ResolveBodyError::NoSuchBody)?;
        Ok(match body.part {
            Part::Planet(_) | Part::Moon(_) | Part::Member(_) => {
                Some(hooks::surface_seed(self.seed, index.body_id(self.system)))
            }
            Part::Ring(_) => None,
        })
    }

    /// The hooks of body `index` at `t` (P14.T23, T24.a, T24.b): its surface seed, its bulk
    /// composition, the mass fractions its record gives with the volatile inventory of its
    /// derivation at `t` and the system's \[Fe/H\] and \[α/Fe\], and its surface conditions and
    /// global figures, its record's surface section's ([`record::Surface`](crate::planetary::record::Surface),
    /// P14.T48.e, in the same state); `None` for a ring, a belt or the halo. Server-only, and not
    /// part of the body's record (the [`hooks`] module documentation).
    ///
    /// # Errors
    ///
    /// As [`body_at`](Self::body_at).
    ///
    /// # Panics
    ///
    /// As [`snapshot_at`](Self::snapshot_at).
    pub fn hooks_at(
        &self,
        ctx: &SystemContext,
        index: BodyIndex,
        t: UniverseTime,
    ) -> Result<Option<BodyHooks>, ResolveBodyError> {
        let Some(seed) = self.surface_seed(index)? else {
            return Ok(None);
        };
        let body = self.body(index).ok_or(ResolveBodyError::NoSuchBody)?;
        let epoch = Epoch::new(self, ctx, t).with_shades();
        let primary = match body.host {
            OrbitHost::Body(parent) => self
                .body(parent)
                .expect("a satellite's parent is a body of its system"),
            OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => body,
        };
        let host = fate_host(ctx, self.zone_of(primary));
        let fate = self.fate_of(primary, &host);
        let primary_label =
            label::label(self, primary.index).expect("every body of a system has a label");
        let cosines = HourCosines::new();
        let (primary_record, now) =
            self.primary_record(&epoch, &cosines, primary, primary_label, &fate);
        let (bulk, record) = match &body.part {
            Part::Planet(_) | Part::Member(_) => {
                let bulk = match now.derived {
                    Some(d) => {
                        let properties =
                            icy_member_bulk(body, &d).unwrap_or_else(|| BulkProperties::from(&d));
                        bulk_composition(ctx, properties.fractions(), *d.element_inventory())
                    }
                    None => Section::NotApplicable,
                };
                (bulk, primary_record)
            }
            Part::Moon(moon) => {
                let label = label::label(self, index).expect("every body of a system has a label");
                let record = self.satellite_record(&epoch, &cosines, body, label, &now, &fate);
                let state = moon_state(now.state, &moon.satellite, ctx.age_at_epoch(), t);
                let bulk = if state == BodyState::Present && now.position.is_some() {
                    let orbit = moon.satellite.orbit_at(&moon.parent, ctx.age_at(t));
                    let at = MoonNow {
                        body,
                        moon,
                        orbit,
                        parent: &now,
                        fate: &fate,
                    };
                    let spin = Self::moon_spin(body, &fate, t);
                    match self.moon_derived(&epoch, &cosines, &at, spin, PassDriver::Solve) {
                        Some(derived) => {
                            let properties = Self::moon_bulk_of(moon, &derived, &now);
                            bulk_composition(
                                ctx,
                                properties.fractions(),
                                *derived.element_inventory(),
                            )
                        }
                        None => Section::NotModelled,
                    }
                } else {
                    Section::NotApplicable
                };
                (bulk, record)
            }
            Part::Ring(_) => unreachable!("a ring has no surface seed"),
        };
        let (surface, figures) = match record.into_surface() {
            Section::Ok(surface) => {
                let (conditions, figures) = surface.into_parts();
                (Section::Ok(conditions), Section::Ok(figures))
            }
            Section::NotResolved => (Section::NotResolved, Section::NotResolved),
            Section::NotModelled => (Section::NotModelled, Section::NotModelled),
            Section::NotApplicable => (Section::NotApplicable, Section::NotApplicable),
        };
        Ok(Some(BodyHooks::new(seed, bulk, surface, figures)))
    }

    /// The surface section of the planet, member or moon `body` at the epoch's time
    /// (P14.T24.a–b, T48.a–d, P14.T48.e), whose primary, itself or a moon's planet, goes round its
    /// hosts as `fate` tells, read as `of` describes it ([`published_surface`]), under the spin
    /// `spin` its record computed once for its passes and its surface (P14.T54.a):
    /// [`Section::NotModelled`] for a body with no rotation law, every body [`generate_planets`]
    /// makes, before anything else is read.
    ///
    /// A present body's surface reads its derivation at the time, its bulk section's gravity,
    /// radius and surface material (of its mass fractions, [`SurfaceMaterial::of`]), its spin
    /// under its seasonal orbit then (its own for a planet or a member, its planet's for a moon,
    /// [`SeasonalSpin::of`]) and the context its published pass read (rule (f)). The figures read
    /// its mass, its bulk section's radius, gravity, class and fractions, its atmosphere and its
    /// volatile inventory's water, its sidereal rotation period, the system's \[Fe/H\] and age,
    /// and for a moon the tide its planet raises in it, its derivation's (P14.T17.b), with a
    /// regular moon's subsurface ocean ([`Subsurface::of_regular_moon`], P14.T17.b's flag); its
    /// crater inputs with its primary's host's impactor sources (P14.T48.c, T48.f); and its
    /// continental asymptote, word 0 of its `body.continents` stream (P14.T48.a), with whether it
    /// entered its hosts' water window holding water (P14.T48.b's start condition). The figures'
    /// volatile history (P14.T48.b) reads the conditions' liquid-water flag, the hosts' light over
    /// the primary's past (each star at its state then, along the primary's orbit then for a star
    /// of its zone and the pair's for a companion), and the body's outgassing then, and their
    /// ocean, ice and cloud the conditions. The conditions' climate regime (P14.T48.d) reads the
    /// body's mass, the reservoirs its atmosphere holds, the figures' history and their ice.
    #[must_use]
    fn surface_section(
        &self,
        epoch: &Epoch<'_>,
        cosines: &HourCosines,
        (body, fate): (&Body, &BodyFate<'_>),
        of: SurfaceOf<'_>,
        spin: Option<SeasonalSpin>,
    ) -> Section<Surface> {
        let Some(spin) = spin else {
            return Section::NotModelled;
        };
        let frame = self.surface_frame(epoch, body, fate);
        let time = SurfaceTime {
            past: &frame.past,
            spin,
            seasonal: frame.seasonal.as_ref(),
            fe_h: frame.fe_h,
            age: frame.age,
            cosines,
        };
        let case = match of {
            SurfaceOf::Body { derived, bulk } => SurfaceCase {
                sky: SurfaceSky::of_body(derived),
                bulk,
                interior: SurfaceInterior::of_body(derived),
                craters: self.body_craters(body),
                sources: frame.sources.as_ref(),
                continental_asymptote: frame.continental_asymptote,
                cold_traps: frame.cold_traps,
                record: self.record,
            },
            SurfaceOf::Moon {
                moon,
                derived,
                bulk,
                orbit,
            } => SurfaceCase {
                sky: SurfaceSky::of_moon(derived),
                bulk,
                interior: SurfaceInterior::of_moon(moon, derived),
                craters: self.moon_craters(frame.primary, moon, derived.ice_fraction(), orbit),
                sources: frame.sources.as_ref(),
                continental_asymptote: frame.continental_asymptote,
                cold_traps: frame.cold_traps,
                record: self.record,
            },
        };
        let surface = published_surface(&time, &case);
        // Its callers ask only for a class with a solid surface, which is never in the
        // gas-envelope state (both read the one envelope fraction), so that exactly one of the
        // surface and the envelope applies.
        debug_assert!(
            !matches!(surface, Section::NotApplicable),
            "{:?}'s solid surface is not applicable",
            body.index
        );
        surface
    }

    /// What the surface of `body`, whose primary (itself or a moon's planet) goes round its hosts
    /// as `fate` tells, reads of its system at the epoch's time beyond its derivation and its
    /// spin, its passes' and its record's alike (P14.T54.a): its primary, the hosts' light over
    /// the primary's past ([`PastSky`]), the primary's orbit then, which sets its seasons, its
    /// host's impactor sources (P14.T48.f), its continental asymptote (P14.T48.a), its cold traps'
    /// case (P14.T51.f), and the system's \[Fe/H\] and age then.
    #[must_use]
    fn surface_frame<'s>(
        &'s self,
        epoch: &Epoch<'s>,
        body: &Body,
        fate: &'s BodyFate<'s>,
    ) -> SurfaceFrame<'s> {
        let (ctx, t) = (epoch.ctx, epoch.t);
        let primary = match body.host {
            OrbitHost::Body(parent) => self
                .body(parent)
                .expect("a satellite's parent is a body of its system"),
            OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => self
                .body(body.index)
                .expect("a body is a body of its system"),
        };
        let id = body.index.body_id(self.system);
        SurfaceFrame {
            primary,
            past: PastSky::new(ctx, self.zone_of(primary), fate, t),
            // The primary's orbit at the time sets the seasons: a planet's or a member's own, and
            // its moons'.
            seasonal: fate.at(t).orbit().copied(),
            // The impactors' sources about the primary's host (P14.T48.f), as generated.
            sources: self.impactor_sources(primary.host),
            continental_asymptote: relief::continental_asymptote(self.seed, id),
            cold_traps: ColdTrapCase {
                exposed_fraction: cold_traps::exposed_fraction(self.seed, id),
                star_radius: epoch.brightest_radius(self.zone_of(primary)),
            },
            fe_h: ctx.fe_h(),
            age: ctx.age_at(t),
        }
    }

    /// The rotation of body `index` (P14.T14), as [`generate`] derived and holds it (P14.T46.b):
    /// its spin, drawn on its own `planet.spin` stream, against its orbit, mass, radius, class and
    /// mass fractions at the epoch (or, for a system not yet born then, at the end of the clock
    /// window), about its primary: its host's mass for a planet or a dwarf planet, its planet's
    /// for a moon.
    ///
    /// A planet or dwarf planet with a giant-impact moon (P14.T18) takes the isotropic obliquity,
    /// every other body the Rayleigh one. `None` for a ring, a belt or the halo, for a body not
    /// present at that time, such as one destroyed before the epoch, and for every body of a
    /// system [`generate_planets`] made, which holds no law.
    ///
    /// # Errors
    ///
    /// As [`body_at`](Self::body_at).
    ///
    /// # Panics
    ///
    /// As [`snapshot_at`](Self::snapshot_at).
    pub fn rotation_of(
        &self,
        ctx: &SystemContext,
        index: BodyIndex,
    ) -> Result<Option<BodyRotation>, ResolveBodyError> {
        assert_eq!(
            ctx.id(),
            self.system,
            "a system is queried with the context it was generated from"
        );
        label::label(self, index).ok_or(ResolveBodyError::NoSuchBody)?;
        Ok(self.body(index).and_then(|body| body.rotation))
    }

    /// The rotation of `body` as [`rotation_of`](Self::rotation_of) describes it, derived from its
    /// record at [`parent_time`], `record` (P14.T14, P14.T46.b): what [`generate`] holds, which the
    /// tests derive again from a body's record. The generator reads its planets' and members'
    /// records before their passes ([`records_at_parent_time`](Self::records_at_parent_time)).
    #[cfg(test)]
    #[must_use]
    fn derive_rotation(
        &self,
        ctx: &SystemContext,
        body: &Body,
        record: &BodyRecord,
    ) -> Option<BodyRotation> {
        if matches!(body.part, Part::Ring(_)) {
            return None;
        }
        let then = RecordThen::of_record(record)?;
        // A moon's law reads its derivation then, its own tides (P14.T56.d), which reads its
        // planet's record: the moon arm takes its rounds, as `moon_rotations` does.
        if let Part::Moon(moon) = &body.part {
            let OrbitHost::Body(parent) = body.host else {
                unreachable!("a moon's host is a body")
            };
            let epoch = Epoch::new(self, ctx, parent_time(ctx)).with_shades();
            let cosines = HourCosines::new();
            let primary = self
                .body(parent)
                .expect("a satellite's parent is a body of its system");
            let host = fate_host(ctx, self.zone_of(primary));
            let fate = self.fate_of(primary, &host);
            let now = self.parent_now(&epoch, &cosines, parent, &fate);
            return self
                .moon_law(&epoch, &cosines, body, moon, (&now, &fate))
                .map(|held| held.law);
        }
        self.rotation_then(ctx, body, &then)
    }

    /// The rotation of the planet or member `body` from what its record at [`parent_time`] holds,
    /// `then` (P14.T14, P14.T46.b): what [`generate`] holds, its locking at its class's tides.
    #[must_use]
    fn rotation_then(
        &self,
        ctx: &SystemContext,
        body: &Body,
        then: &RecordThen,
    ) -> Option<BodyRotation> {
        self.rotation_from(
            ctx,
            body,
            &then.orbit,
            then.bulk,
            then.mass,
            SpinTides::OfClass,
        )
    }

    /// The elements of `body` at [`parent_time`] that its rotation law and its satellites' frame
    /// read, where its record's orbit then is `recorded`: the elements at the time itself, as the
    /// derivation reads them, and not the record's, which an evolving orbit states at its drift
    /// cell's start (P14.T45.a).
    #[must_use]
    fn elements_at_parent_time(
        &self,
        ctx: &SystemContext,
        body: &Body,
        recorded: &KeplerElements,
    ) -> KeplerElements {
        let t = parent_time(ctx);
        match &body.part {
            Part::Moon(moon) => moon.satellite.orbit_at(&moon.parent, ctx.age_at(t)),
            Part::Planet(_) | Part::Member(_) | Part::Ring(_) => {
                let host = fate_host(ctx, self.zone_of(body));
                self.fate_of(body, &host)
                    .at(t)
                    .orbit()
                    .copied()
                    .unwrap_or(*recorded)
            }
        }
    }

    /// The rotation of the planet, member or moon `body`, present at [`parent_time`] with its
    /// record's orbit `recorded`, bulk `bulk` and mass `mass` then, whose locking reads the tides
    /// `tides` (its class's, or a moon's own, P14.T56.d): [`rotation_then`]'s law.
    ///
    /// [`rotation_then`]: Self::rotation_then
    #[must_use]
    fn rotation_from(
        &self,
        ctx: &SystemContext,
        body: &Body,
        recorded: &KeplerElements,
        bulk: SpinBulk,
        mass: EarthMasses,
        tides: SpinTides,
    ) -> Option<BodyRotation> {
        let inputs = self.spin_inputs(ctx, body, recorded, (bulk, mass), tides)?;
        // A present body's record has a positive mass and radius and an orbit about a primary of
        // positive mass, so the derivation refuses nothing the generator makes; a refusal would
        // be a record without a rotation to give, which `None` says.
        BodyRotation::derive(&inputs).ok()
    }

    /// What [`rotation_from`](Self::rotation_from)'s law reads of `body` (P14.T14): its draws on
    /// its own `planet.spin` stream, its obliquity's law, its record's bulk `bulk` and mass `mass`
    /// then, its elements at [`parent_time`] from its record's orbit `recorded`, its primary's
    /// mass and its system's age, with the tides `tides`. `None` for a ring.
    #[must_use]
    fn spin_inputs(
        &self,
        ctx: &SystemContext,
        body: &Body,
        recorded: &KeplerElements,
        (bulk, mass): (SpinBulk, EarthMasses),
        tides: SpinTides,
    ) -> Option<SpinInputs> {
        let index = body.index;
        let obliquity_law = match body.part {
            Part::Ring(_) => return None,
            Part::Planet(_) | Part::Member(_) if self.had_giant_impact(index) => {
                ObliquityLaw::Isotropic
            }
            Part::Planet(_) | Part::Member(_) | Part::Moon(_) => ObliquityLaw::Rayleigh,
        };
        let elements = self.elements_at_parent_time(ctx, body, recorded);
        let mass = Kilograms::from(mass);
        let primary =
            elements.gravitational_parameter().value() / GRAVITATIONAL_CONSTANT - mass.value();
        Some(SpinInputs {
            draws: SpinDraws::for_body(self.seed, index.body_id(self.system)),
            obliquity_law,
            class: bulk.class,
            fractions: bulk.fractions,
            mass,
            radius: Metres::from(bulk.radius),
            orbit: elements,
            primary_mass: Kilograms::new(primary),
            age_at_epoch: ctx.age_at_epoch(),
            tides,
        })
    }

    /// Puts every satellite on its parent's Laplace surface (P14.T56.a; [`generate`]'s
    /// last steps), once every planet's and member's rotation law is held: each one's
    /// [`LaplaceFrame`] from its law and its record at [`parent_time`], `epoch`'s time, as
    /// `records` holds it body by body ([`records_at_parent_time`](Self::records_at_parent_time));
    /// its satellites found again from `sites` on it; and each moon's law on its plane.
    fn place_on_laplace_surfaces(
        &mut self,
        seed: Seed,
        epoch: &Epoch<'_>,
        records: &[Option<RecordThen>],
        sites: &[SatelliteSite],
    ) {
        let ctx = epoch.ctx;
        debug_assert_eq!(
            records.len(),
            self.bodies.len(),
            "a record then for every body"
        );
        let frames: Vec<Option<LaplaceFrame>> = self
            .bodies
            .iter()
            .zip(records)
            .map(|(body, record)| self.laplace_frame(ctx, body, record.as_ref()))
            .collect();
        for (body, frame) in self.bodies.iter_mut().zip(frames) {
            body.laplace = frame;
        }
        let before = std::mem::take(&mut self.satellites);
        let found: Vec<Satellites> = before
            .into_iter()
            .zip(sites)
            .map(
                |(found, site)| match self.body(site.index).and_then(Body::laplace) {
                    Some(frame) => site.find(seed, ctx, SatelliteFrame::Laplace(frame)),
                    None => found,
                },
            )
            .collect();
        self.bodies
            .retain(|body| !matches!(body.part, Part::Moon(_) | Part::Ring(_)));
        self.bodies.extend(satellite_bodies(&found));
        self.satellites = found;
        self.bodies.sort_by_key(Body::index);
        let rotations = self.moon_rotations(epoch);
        for (body, rotation) in self.bodies.iter_mut().zip(rotations) {
            body.rotation = rotation;
        }
    }

    /// The frame of the satellites of `body` on its Laplace surface (P14.T56.a), from its held
    /// rotation law and what its record at [`parent_time`] holds, `record`: its spin pole, the
    /// normal of the orbit its law was derived on, its hydrostatic J₂ ([`hydrostatic_j2`]) at its
    /// spin rate then and its figure's equatorial radius and moment of inertia, its orbit, mass and
    /// host's mass then. `None` for a moon or a ring, and for a body with no law or not present
    /// then.
    #[must_use]
    fn laplace_frame(
        &self,
        ctx: &SystemContext,
        body: &Body,
        record: Option<&RecordThen>,
    ) -> Option<LaplaceFrame> {
        if matches!(body.part, Part::Moon(_) | Part::Ring(_)) {
            return None;
        }
        let rotation = body.rotation.as_ref()?;
        let record = record?;
        let t = parent_time(ctx);
        // The figure its record publishes then, whose radius and moment of inertia J₂ reads.
        let Section::Ok(figure) = body.figure_of(Some(record.bulk), record.mass, t) else {
            return None;
        };
        let elements = self.elements_at_parent_time(ctx, body, &record.orbit);
        let mass = Kilograms::from(record.mass);
        let host =
            elements.gravitational_parameter().value() / GRAVITATIONAL_CONSTANT - mass.value();
        let spin = rotation.frame();
        let (rate, state) = (spin.rate().rate_at(t), spin.state_at(t));
        let equatorial_radius = Metres::new(figure.spheroid().equatorial_radius_m);
        let built = LaplaceFrame::new(&LaplaceFrameParts {
            spin_pole: spin.pole(),
            orbit_normal: elements.orientation().normal(),
            j2: hydrostatic_j2(figure.moment_factor(), rate, equatorial_radius, mass, state),
            equatorial_radius,
            parent_orbit: elements,
            parent_mass: mass,
            host_mass: Kilograms::new(host),
            read_at: ctx.age_at(t),
        });
        // A present body's law, figure, mass and orbit are the generator's own, which the frame
        // never refuses; a refusal is a bug, which the tests' debug builds catch, and leaves the
        // body's satellites in its orbital plane.
        debug_assert!(built.is_ok(), "{:?}: {built:?}", body.index);
        built.ok()
    }

    /// Each body's rotation law in index order: a moon's derived at [`parent_time`], `epoch`'s
    /// time, on its plane, in its rounds ([`moon_law`](Self::moon_law)), its planet derived once,
    /// and every other body's as it is held.
    #[must_use]
    fn moon_rotations(&self, epoch: &Epoch<'_>) -> Vec<Option<BodyRotation>> {
        // Once for the call chain, every moon's insolation table reading them (lever (h)).
        let cosines = HourCosines::new();
        let hosts: Vec<FateHost<'_>> = self
            .zones
            .iter()
            .map(|zone| fate_host(epoch.ctx, zone))
            .collect();
        let fates = self.fates(&hosts);
        let mut parents: Vec<(BodyIndex, ParentNow)> = Vec::new();
        self.bodies
            .iter()
            .map(|body| {
                let Part::Moon(moon) = &body.part else {
                    return body.rotation;
                };
                let OrbitHost::Body(parent) = body.host else {
                    unreachable!("a moon's host is a body")
                };
                let fate = self
                    .bodies
                    .binary_search_by_key(&parent, Body::index)
                    .ok()
                    .and_then(|at| fates[at].as_ref())
                    .expect("a moon's parent is a planet or member, which orbits a zone");
                let at = parents.iter().position(|(index, _)| *index == parent);
                let at = at.unwrap_or_else(|| {
                    parents.push((parent, self.parent_now(epoch, &cosines, parent, fate)));
                    parents.len() - 1
                });
                self.moon_law(epoch, &cosines, body, moon, (&parents[at].1, fate))
                    .map(|held| held.law)
            })
            .collect()
    }

    /// The planet or member `parent`, whose history is `fate`, as its satellites read it at the
    /// epoch's time ([`primary_record`](Self::primary_record)'s), with no record built (lever (e3),
    /// decision-p14-t54a-passes, "Cost escalation (after Batch B)" §C.3): none of a record's
    /// sections, its surface and envelope among them, is in it. Its derivation is its record's,
    /// over its law where it holds one, its insolation table over `cosines`.
    #[must_use]
    fn parent_now(
        &self,
        epoch: &Epoch<'_>,
        cosines: &HourCosines,
        parent: BodyIndex,
        fate: &BodyFate<'_>,
    ) -> ParentNow {
        let primary = self
            .body(parent)
            .expect("a satellite's parent is a body of its system");
        let now = || self.primary_now(epoch, cosines, primary, fate, &fate.at(epoch.t));
        #[cfg(test)]
        if let Some(read) = pass_free_read(PassFreeRead::Parent, now, || {
            let label = label::label(self, parent).expect("every body of a system has a label");
            self.primary_record(epoch, &HourCosines::new(), primary, label, fate)
                .1
        }) {
            return read;
        }
        now()
    }

    /// The rotation law the moon `body`, `moon`, holds, whose parent is as `now` says at the
    /// epoch's time ([`parent_time`]) and goes round its hosts as `fate` tells, in at most two
    /// rounds (P14.T56.d; decision-p14-t54a-passes §2.3, Q2 as amended), as
    /// [`rotation_from`](Self::rotation_from) gives each from its record then, whose orbit is
    /// present and whose bulk its derivation's, with the tides its derivation takes, its own
    /// ([`DerivedMoon::spin_tides`]).
    ///
    /// 1. **Round 1** derives the moon with its record's passes under the locked spin: the spin of
    ///    a frame built from its own [`SpinInputs`] with the lock reached by the time
    ///    ([`BodyRotation::locked_frame`]), whose pole, obliquity and locked rate n p ÷ q are any
    ///    law's locked then, its τ not read. A law it gives that is locked then is held: its
    ///    record, derived under it, reads that same spin, so its passes, tides, locking time and
    ///    law are round 1's to the bit, by construction.
    /// 2. **Round 2**, only for a moon not locked after round 1, derives it under round 1's law and
    ///    holds the law its tides give. Unlocked after round 1 and locked after round 2 is an
    ///    oscillation: "locked by its tides while despinning; its present tides' locking time
    ///    exceeds its age". A moon unlocked in both rounds holds round 2's law, whose record's
    ///    tides, under round 2's rate, differ slightly from round 2's, under round 1's.
    ///
    /// At most two derivations, a fixed rule, and no draw: the rounds open only keyed streams.
    /// `None` for a moon not present then, or whose planet has no position or no orbit then.
    #[must_use]
    fn moon_law(
        &self,
        epoch: &Epoch<'_>,
        cosines: &HourCosines,
        body: &Body,
        moon: &MoonPart,
        (now, fate): (&ParentNow, &BodyFate<'_>),
    ) -> Option<HeldMoonLaw> {
        let (ctx, t) = (epoch.ctx, epoch.t);
        let state = moon_state(now.state, &moon.satellite, ctx.age_at_epoch(), t);
        if state != BodyState::Present || now.position.is_none() {
            return None;
        }
        let seasonal = fate.at(t).orbit().copied()?;
        let orbit = moon.satellite.orbit_at(&moon.parent, ctx.age_at(t));
        let case = MoonNow {
            body,
            moon,
            orbit,
            parent: now,
            fate,
        };
        // Its bulk before its passes, the same in both rounds: what its law reads of its record.
        let before = Self::moon_bulk_before_passes(epoch, &case)?;
        let bulk = SpinBulk::from(&Self::moon_figure_of_shape(
            moon,
            &before.shape,
            &before.bulk.fractions(),
            now,
        ));
        let law_of = |spin: SeasonalSpin| {
            let derived =
                self.moon_derived(epoch, cosines, &case, Some(spin), PassDriver::Solve)?;
            self.rotation_from(ctx, body, &orbit, bulk, body.mass(), derived.spin_tides())
        };
        // Round 1, under the locked spin.
        let inputs =
            self.spin_inputs(ctx, body, &orbit, (bulk, body.mass()), SpinTides::OfClass)?;
        let locked = BodyRotation::locked_frame(&inputs).ok()?;
        let locked = SeasonalSpin::of(&locked, t, &seasonal, SeasonsFrom::Planet);
        let first = law_of(locked)?;
        if matches!(first.frame().state_at(t), SpinState::Locked(_)) {
            return Some(HeldMoonLaw {
                law: first,
                round: MoonRound::First,
                locked,
            });
        }
        // Round 2, under round 1's law.
        let second = law_of(SeasonalSpin::of(
            first.frame(),
            t,
            &seasonal,
            SeasonsFrom::Planet,
        ))?;
        Some(HeldMoonLaw {
            law: second,
            round: MoonRound::Second {
                first: Box::new(first),
            },
            locked,
        })
    }

    /// Whether the planet or dwarf planet `index` has a giant-impact moon (P14.T18).
    #[must_use]
    fn had_giant_impact(&self, index: BodyIndex) -> bool {
        self.children(index)
            .any(|child| child.kind() == BodyKind::Moon(MoonOrigin::GiantImpact))
    }

    /// The impactors' sources about the orbit host `host` (P14.T48.f, with P14.T48.c's masses):
    /// its zone's initial host mass, its belts in their generated order with their components'
    /// masses at 4.567 Gyr ([`SourceBelt::from`]), and its planets' masses and primordial
    /// semi-major axes, as the belts read them. `None` for a host with no zone, which no body goes
    /// round.
    #[must_use]
    fn impactor_sources(&self, host: OrbitHost) -> Option<ImpactorSources> {
        let zone = self.zone(host)?;
        let belts = self
            .belts
            .iter()
            .filter(|belt| belt.host() == host)
            .map(SourceBelt::from)
            .collect();
        let planets = self
            .planets()
            .filter(|planet| planet.host == host)
            .filter_map(|planet| Some((planet.mass(), planet.orbit()?.semi_major_axis())));
        // The parts are the generator's own, which the sources never refuse; a refusal is a bug,
        // which the tests' debug builds catch, and leaves the figures not modelled.
        let sources = ImpactorSources::new(
            GravitationalParameter::from_solar_masses(zone.host_mass()),
            belts,
            planets,
        );
        debug_assert!(sources.is_ok(), "{host:?}: {sources:?}");
        sources.ok()
    }

    /// The planet or member `body` as its impactors read it (P14.T48.f): its primordial
    /// semi-major axis, eccentricity and inclination to its host's plane, its mass, and a
    /// member's part and composition; `None` for a moon or a ring.
    ///
    /// The orbit is the primordial one, as the sources' belts and planets are, so that the impact
    /// rate holds through the host's evolution with them.
    #[must_use]
    fn host_orbiter(&self, body: &Body) -> Option<HostOrbiter> {
        let belt = match &body.part {
            Part::Planet(_) => None,
            Part::Member(member) => Some(BeltMembership {
                part: member.member.part(),
                composition: member.member.composition(),
            }),
            Part::Moon(_) | Part::Ring(_) => return None,
        };
        let orbit = body.orbit()?;
        let plane = self.host(body.host)?.plane();
        let p = Orientation::new(plane.inclination(), plane.node(), Radians::ZERO)
            .expect("a host's plane is a valid orientation")
            .normal();
        let n = orbit.orientation().normal();
        // The angle between the normals as atan2(|p × n|, p · n), which keeps a coplanar orbit's
        // inclination accurate where an arccosine of the dot product would not.
        let cross = [
            p[1] * n[2] - p[2] * n[1],
            p[2] * n[0] - p[0] * n[2],
            p[0] * n[1] - p[1] * n[0],
        ];
        let sine = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        let cosine = p[0] * n[0] + p[1] * n[1] + p[2] * n[2];
        Some(HostOrbiter {
            orbit: HostOrbit {
                semi_major_axis: orbit.semi_major_axis(),
                eccentricity: orbit.eccentricity().value(),
                inclination: Radians::new(math::atan2(sine, cosine)),
            },
            mass: body.mass(),
            belt,
        })
    }

    /// What the crater contract of the planet or member `body` reads beyond its bulk and its
    /// sources (P14.T24.b, T48.c): where it goes round, a planet's magma ocean's end
    /// (P14.T28.a's `molten_until`; a member has no draw), and a giant impact at the system's
    /// birth if it has a giant-impact moon (P14.T18).
    #[must_use]
    fn body_craters(&self, body: &Body) -> Option<SurfaceCraters> {
        let molten_until = match &body.part {
            Part::Planet(planet) => Some(Years::from(planet.fate.formation().molten_until())),
            Part::Member(_) | Part::Moon(_) | Part::Ring(_) => None,
        };
        Some(SurfaceCraters {
            orbit: ImpactorOrbit::Host(self.host_orbiter(body)?),
            molten_until,
            last_giant_impact: self.had_giant_impact(body.index).then_some(Years::ZERO),
        })
    }

    /// What the crater contract of the moon `moon` of `parent`, of ice fraction `ice_fraction`
    /// ([`DerivedMoon::ice_fraction`]) on `orbit`, reads beyond its bulk and its sources
    /// (P14.T24.b, T48.c): its parent as its impactors read it (with the parent's mass the moon
    /// goes round), its semi-major axis then, the side of its parent's ice line it formed on
    /// ([`MoonBirth::formed`]; science-p14-t24b-laws §2.4), and for a giant-impact moon its birth
    /// in the impact at the system's birth. No moon has a magma-ocean draw.
    #[must_use]
    fn moon_craters(
        &self,
        parent: &Body,
        moon: &MoonPart,
        ice_fraction: f64,
        orbit: &KeplerElements,
    ) -> Option<SurfaceCraters> {
        let parent_orbiter = HostOrbiter {
            mass: moon.parent.mass(),
            ..self.host_orbiter(parent)?
        };
        let birth = match moon.satellite.moon() {
            SatelliteMoon::Regular(_) => MoonBirth::Regular { ice_fraction },
            SatelliteMoon::GiantImpact(_) => MoonBirth::GiantImpact,
            SatelliteMoon::Captured(captured) => MoonBirth::Captured(captured.kind()),
        };
        let neighbouring = self
            .nearest_belt(parent.index)
            .filter(|belt| belt.adjacency() == BeltAdjacency::Neighbouring)
            .map(|belt| belt.composition());
        Some(SurfaceCraters {
            orbit: ImpactorOrbit::Moon {
                parent: parent_orbiter,
                semi_major_axis: orbit.semi_major_axis(),
                formed: birth.formed(&parent_orbiter, neighbouring),
            },
            molten_until: None,
            last_giant_impact: matches!(birth, MoonBirth::GiantImpact).then_some(Years::ZERO),
        })
    }

    /// The lifetime of the disc `body`'s host formed its bodies in (P14.T28.a), the end of every
    /// body's formation window but a planet's and a giant-impact moon's ([`FormationWindow`]):
    /// `None` for a host with no disc.
    #[must_use]
    fn disc_lifetime(&self, body: &Body) -> Option<Megayears> {
        Some(
            self.host(self.zone_of(body).host())?
                .disc
                .profile()?
                .lifetime(),
        )
    }

    /// The formation time `t_f` of the host orbiter `body` as its crust reads it (P14.T51.c's
    /// `CrustCompositionInputs::formation`): a planet's P14.T28.a's `formed_at`, and a member's word
    /// 0 of its `body.formation` stream over its disc's window ([`formation_time`]). `None` for a
    /// moon or a ring, and for a member whose host has no disc.
    #[must_use]
    fn crust_formation(&self, body: &Body) -> Option<Megayears> {
        match &body.part {
            Part::Planet(planet) => Some(planet.fate.formation().formed_at()),
            Part::Member(_) => {
                let window = FormationWindow::disc(self.disc_lifetime(body)?).ok()?;
                Some(formation_time(
                    self.seed,
                    body.index.body_id(self.system),
                    window,
                ))
            }
            Part::Moon(_) | Part::Ring(_) => None,
        }
    }

    /// What the host of the system of `ctx` gives every body's crust's basis (P14.T51.c): its
    /// Mg/Si by number, its system's `system.abundances` words 0–1 at its \[Fe/H\] (P14.T50.a's
    /// [`HostAbundances`], first read on the record from 23, P14.T54.a), its \[α/Fe\] (0 for none)
    /// and its \[Fe/H\].
    #[must_use]
    fn crust_host(&self, ctx: &SystemContext) -> CrustHost {
        let abundances = HostAbundances::of(
            ctx.fe_h(),
            &AbundanceDraws::for_system(self.seed, self.system),
        );
        CrustHost {
            host_magnesium_to_silicon: abundances.magnesium_to_silicon(),
            alpha_fe: ctx.alpha_fe().unwrap_or(Dex::ZERO),
            fe_h: ctx.fe_h(),
        }
    }

    /// What the record gives the crust's basis of the planet or member `body` beyond its bulk
    /// (P14.T54.a; [`RecordCrust`]): its origin, its formation time and its host's
    /// ([`crust_host`](Self::crust_host)); its side of the snow line is its derivation's. `None`
    /// for a moon or a ring, and for a member whose host has no disc.
    #[must_use]
    fn primary_crust(&self, ctx: &SystemContext, body: &Body) -> Option<CrustOf> {
        let origin = match &body.part {
            Part::Planet(_) => CrustOrigin::Planet,
            Part::Member(_) => CrustOrigin::BeltMember,
            Part::Moon(_) | Part::Ring(_) => return None,
        };
        Some(CrustOf {
            origin,
            formation: self.crust_formation(body)?,
            snow_line_side: None,
            host: self.crust_host(ctx),
        })
    }

    /// What the record gives the crust's basis of the moon `moon` of `primary` beyond its bulk, of
    /// ice fraction `ice_fraction` on `orbit` (P14.T54.a; [`RecordCrust`]):
    ///
    /// - its side of its parent's ice line, its craters' birthplace ([`MoonBirth::formed`]);
    /// - its origin: a regular moon, a giant-impact moon on a parent rocky or icy by that side
    ///   (an icy belt's member's moon formed beyond, as Charon), or a capture;
    /// - its formation time: a regular moon's word 0 of its `body.formation` stream from its
    ///   giant's formation to the disc's lifetime, a capture's over the disc's window, and a
    ///   giant-impact moon's its parent's ([`crust_formation`](Self::crust_formation));
    /// - its host's ([`CrustHost`]).
    ///
    /// `None` where its craters are not computed, its host has no disc, or a regular moon's window
    /// is refused (its giant formed after its disc).
    #[must_use]
    fn moon_crust(
        &self,
        (primary, body, moon): (&Body, &Body, &MoonPart),
        ice_fraction: f64,
        orbit: &KeplerElements,
        host: CrustHost,
    ) -> Option<CrustOf> {
        let craters = self.moon_craters(primary, moon, ice_fraction, orbit)?;
        let ImpactorOrbit::Moon { formed, .. } = craters.orbit else {
            return None;
        };
        let lifetime = self.disc_lifetime(primary)?;
        let drawn = |window: FormationWindow| {
            formation_time(self.seed, body.index.body_id(self.system), window)
        };
        let (origin, formation) = match moon.satellite.moon() {
            SatelliteMoon::Regular(_) => {
                let formed_at = primary.formation()?.formed_at();
                let window = FormationWindow::new(formed_at, lifetime).ok()?;
                (CrustOrigin::RegularMoon, drawn(window))
            }
            SatelliteMoon::GiantImpact(_) => {
                let parent = match formed {
                    SnowLineSide::Beyond => ParentSurface::Icy,
                    SnowLineSide::Inside => ParentSurface::Rocky,
                };
                (
                    CrustOrigin::ImpactMoon(parent),
                    self.crust_formation(primary)?,
                )
            }
            SatelliteMoon::Captured(_) => (
                CrustOrigin::CapturedMoon,
                drawn(FormationWindow::disc(lifetime).ok()?),
            ),
        };
        Some(CrustOf {
            origin,
            formation,
            snow_line_side: Some(formed),
            host,
        })
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
    /// history `fate`, and what its satellites read of it then. Its surface reads the call chain's
    /// hour angles' cosines `cosines` ([`published_surface`]).
    #[must_use]
    fn primary_record(
        &self,
        epoch: &Epoch<'_>,
        cosines: &HourCosines,
        body: &Body,
        label: BodyLabel,
        fate: &BodyFate<'_>,
    ) -> (BodyRecord, ParentNow) {
        let at = fate.at(epoch.t);
        let now = self.primary_now(epoch, cosines, body, fate, &at);
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
        // A present body's derivation and position, which every present body has, and no other.
        let builder = match now.derived.zip(now.position) {
            Some((derived, position)) => {
                let section = at
                    .body_orbit()
                    .expect("a present body has an orbit section");
                let section = match &body.part {
                    Part::Member(_) => section.about(body.host),
                    Part::Planet(_) | Part::Moon(_) | Part::Ring(_) => section,
                };
                let bulk = icy_member_bulk(body, &derived)
                    .unwrap_or_else(|| BulkProperties::from(&derived));
                // P14.T48.e: the surface section of a body with a solid surface, and the envelope
                // of one without.
                let (surface, envelope) = if bulk.class().has_solid_surface() {
                    let of = SurfaceOf::Body {
                        derived: &derived,
                        bulk: &bulk,
                    };
                    (
                        self.surface_section(epoch, cosines, (body, fate), of, now.spin),
                        Section::NotApplicable,
                    )
                } else {
                    (Section::NotApplicable, Section::NotModelled)
                };
                builder
                    .derived(&derived)
                    .bulk(Section::Ok(bulk))
                    .surface(surface)
                    .envelope(envelope)
                    .photometry(Section::Ok(photometry_of(
                        derived.albedo(),
                        derived.atmosphere(),
                        &bulk,
                    )))
                    .figure(body.figure_section(Some(&bulk), derived.mass(), epoch.t))
                    .orbit(Section::Ok(section))
                    .position(position)
                    .rotation(body.rotation_section())
                    .hooks(self.present_hooks(body.index))
            }
            None => builder
                .orbit(Section::NotApplicable)
                .bulk(Section::NotApplicable)
                .surface(Section::NotApplicable)
                .envelope(Section::NotApplicable)
                .photometry(Section::NotApplicable)
                .figure(Section::NotApplicable)
                .rotation(Section::NotApplicable)
                .hooks(Section::NotApplicable),
        };
        let record = builder
            .build()
            .expect("a generated record withholds no section and has a known kind");
        (record, now)
    }

    /// The planet or member `body` as its satellites read it at the epoch's time, where its
    /// history `fate` puts it `at` then: its state and next change, and for a body present then its
    /// position, its derivation, its spin, its moons' sky, nursery and shades' book. It is what
    /// [`primary_record`](Self::primary_record) builds its record on, and what [`parent_now`]
    /// gives without the record (lever (e3)).
    ///
    /// A body with a held law is derived over its record (P14.T54.a): its spin under its law and
    /// its own orbit then, computed once here for its passes and its record's surface, with its
    /// insolation table over the call chain's hour angles' cosines `cosines`. One with none keeps
    /// the passes' stand-in (no rotation law, no basins, the bulk material's regolith).
    ///
    /// [`parent_now`]: Self::parent_now
    #[must_use]
    fn primary_now(
        &self,
        epoch: &Epoch<'_>,
        cosines: &HourCosines,
        body: &Body,
        fate: &BodyFate<'_>,
        at: &FateAt,
    ) -> ParentNow {
        let mut now = ParentNow {
            state: at.state(),
            changes_at: at.changes_at(),
            position: None,
            derived: None,
            spin: None,
            sky: None,
            nursery: None,
            shades: None,
        };
        match at.state() {
            BodyState::Present => {
                let zone = self.zone_of(body);
                let (derived, sky, spin) =
                    self.primary_derived(epoch, cosines, (body, fate), at, PassDriver::Solve);
                let position = epoch.position(
                    zone,
                    at.trajectory().expect("a present body has a trajectory"),
                );
                now.position = Some(position);
                now.derived = Some(derived);
                now.spin = spin;
                // Its moons' shades read the epoch's book with its own glow in it (lever (d)).
                now.shades = sky
                    .as_ref()
                    .zip(epoch.shades.as_ref())
                    .map(|(sky, book)| book.with_host(sky.planet_light().effective_temperature()));
                now.sky = sky;
                now.nursery = self.nursery(epoch.ctx, body);
            }
            BodyState::NotYetFormed | BodyState::Destroyed { .. } | BodyState::Unbound { .. } => {}
        }
        now
    }

    /// The derivation of the planet or member `body`, present at the epoch's time where its
    /// history `fate` puts it `at`, its passes driven by `driver`, with its moons' sky and the
    /// spin it was derived under ([`primary_now`](Self::primary_now)): over its record where it
    /// holds a law, its spin computed once here, and over the passes' stand-in where it holds
    /// none.
    #[must_use]
    fn primary_derived(
        &self,
        epoch: &Epoch<'_>,
        cosines: &HourCosines,
        (body, fate): (&Body, &BodyFate<'_>),
        at: &FateAt,
        driver: PassDriver,
    ) -> (DerivedBody, Option<MoonSky>, Option<SeasonalSpin>) {
        let zone = self.zone_of(body);
        let orbit = at.orbit().expect("a present body has an orbit");
        let placed = Self::placed_now(body, orbit, at.mass());
        let host_mass = fate.host_mass(epoch.t);
        // Its spin, once a body: its held law's under its own orbit then, its seasons' (P14.T54.a).
        let spin = body
            .rotation
            .as_ref()
            .map(|rotation| SeasonalSpin::of(rotation.frame(), epoch.t, orbit, SeasonsFrom::Own));
        let Some(spin) = spin else {
            let (derived, sky) = self.derive(
                epoch,
                body,
                zone,
                &placed,
                host_mass,
                (PassRecord::StandIn, driver),
            );
            return (derived, sky, None);
        };
        let frame = self.surface_frame(epoch, body, fate);
        let table = ZonalInsolation::of(spin.obliquity, cosines);
        let passes = PrimaryPasses {
            frame: &frame,
            spin,
            table: &table,
            body,
            craters: self.body_craters(body),
            crust: self.primary_crust(epoch.ctx, body),
        };
        let (derived, sky) = self.derive(
            epoch,
            body,
            zone,
            &placed,
            host_mass,
            (PassRecord::Record(&passes), driver),
        );
        (derived, sky, Some(spin))
    }

    /// The hooks section of the record of body `index`, a planet, moon or dwarf planet present at
    /// the record's time: its detail seed (the rendering plans' R09.T1.b), word 0 of its
    /// `body.surface.detail` stream, which depends on nothing but the universe seed and the body's
    /// ID. The surface seed stays off the record (the [`hooks`] module documentation).
    #[must_use]
    fn present_hooks(&self, index: BodyIndex) -> Section<Hooks> {
        Section::Ok(Hooks::new(hooks::detail_seed(
            self.seed,
            index.body_id(self.system),
        )))
    }

    /// The planet or member `body` on its orbit `orbit` at the time, of mass `mass` then, as
    /// [`derive_body`] reads it, with its record's bulk for an icy member
    /// ([`with_record_bulk`]).
    #[must_use]
    fn placed_now(body: &Body, orbit: &KeplerElements, mass: EarthMasses) -> PlacedBody {
        let placed = match &body.part {
            Part::Planet(planet) => planet.placed_body(mass),
            Part::Member(member) => member.member.placed_body(),
            Part::Moon(_) | Part::Ring(_) => unreachable!("a satellite is derived by its own rule"),
        };
        with_record_bulk(body, &placed.with_orbit_now(*orbit))
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
        // A nursery refused is a parent beyond the cooling fit's 13 Jupiter masses, which
        // placement never makes: its moons' bulk is not computed (`moon_derived`). Its zone
        // (P14.T56.c) is never refused here, since a placed planet's formation distance and a
        // member's orbit are positive and finite, and so is a disc's snow line.
        MoonNursery::new(parent, ctx.composition(), profile, formation).ok()
    }

    /// The record of the moon or ring `body` at the epoch's time, labelled `label`, whose parent
    /// is as `parent` says then and goes round its hosts as `fate` tells. Its surface reads the
    /// call chain's hour angles' cosines `cosines` ([`published_surface`]).
    #[must_use]
    fn satellite_record(
        &self,
        epoch: &Epoch<'_>,
        cosines: &HourCosines,
        body: &Body,
        label: BodyLabel,
        parent: &ParentNow,
        fate: &BodyFate<'_>,
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
            .rings(Section::NotApplicable)
            .envelope(Section::NotApplicable);
        let present = state == BodyState::Present;
        let builder = match (&body.part, present, parent.position) {
            (Part::Ring(ring), true, Some(centre)) => builder
                .population(Section::Ok(Population::Ring(ring.clone())))
                .position(centre)
                .orbit(Section::NotApplicable)
                .bulk(Section::NotApplicable)
                .surface(Section::NotApplicable)
                .hooks(Section::NotApplicable)
                .photometry(Section::NotApplicable)
                .figure(Section::NotApplicable)
                .rotation(Section::NotApplicable),
            (Part::Moon(moon), true, Some(centre)) => {
                let orbit = moon.satellite.orbit_at(&moon.parent, ctx.age_at(t));
                let (trajectory, valid_until) = moon_trajectory(ctx, moon, t, parent.changes_at);
                let now = MoonNow {
                    body,
                    moon,
                    orbit,
                    parent,
                    fate,
                };
                // Its spin, once a moon: its held law's under its planet's orbit then, which its
                // passes and its surface read (P14.T54.a).
                let spin = Self::moon_spin(body, fate, t);
                let derived = self.moon_derived(epoch, cosines, &now, spin, PassDriver::Solve);
                let bulk = derived
                    .as_ref()
                    .map(|derived| (Self::moon_bulk_of(moon, derived, parent), derived));
                let photometry = bulk.map_or(Section::NotModelled, |(bulk, derived)| {
                    Section::Ok(photometry_of(derived.albedo(), derived.atmosphere(), &bulk))
                });
                // P14.T48.e: a moon's surface section, and its envelope's tag, as a planet's. A
                // moon's fractions hold no envelope, so its class always has a solid surface.
                let (surface, envelope) = bulk.map_or(
                    (Section::NotModelled, Section::NotApplicable),
                    |(bulk, derived)| {
                        if bulk.class().has_solid_surface() {
                            let of = SurfaceOf::Moon {
                                moon,
                                derived,
                                bulk: &bulk,
                                orbit: &orbit,
                            };
                            (
                                self.surface_section(epoch, cosines, (body, fate), of, spin),
                                Section::NotApplicable,
                            )
                        } else {
                            (Section::NotApplicable, Section::NotModelled)
                        }
                    },
                );
                let bulk = bulk.map_or(Section::NotModelled, |(bulk, _)| Section::Ok(bulk));
                builder
                    .orbit(Section::Ok(
                        BodyOrbit::new(*trajectory.elements(), valid_until)
                            .with_drift(trajectory.drift().copied()),
                    ))
                    .position(centre.translated(trajectory.relative_state_at(t).0))
                    .figure(body.figure_section(bulk.ok(), body.mass(), t))
                    .photometry(photometry)
                    .bulk(bulk)
                    .surface(surface)
                    .envelope(envelope)
                    .population(Section::NotApplicable)
                    .rotation(body.rotation_section())
                    .hooks(self.present_hooks(body.index))
            }
            // A ring or a moon not present at the time, or whose planet has no position then: its
            // sections mean nothing then.
            (Part::Ring(_) | Part::Moon(_), _, _) => builder
                .population(Section::NotApplicable)
                .orbit(Section::NotApplicable)
                .bulk(Section::NotApplicable)
                .surface(Section::NotApplicable)
                .hooks(Section::NotApplicable)
                .photometry(Section::NotApplicable)
                .figure(Section::NotApplicable)
                .rotation(Section::NotApplicable),
            (Part::Planet(_) | Part::Member(_), _, _) => {
                unreachable!("a satellite is a moon or a ring")
            }
        };
        builder
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }

    /// The derivation of the moon `body` at the epoch's time as its record takes it, under its
    /// held law, its planet as `now` says then and going round its hosts as `fate` tells: the
    /// tests' ([`moon_derived`](Self::moon_derived)). `None` for a body not a moon, or where its
    /// derivation is not computed.
    #[cfg(test)]
    #[must_use]
    fn record_moon(
        &self,
        epoch: &Epoch<'_>,
        body: &Body,
        (now, fate): (&ParentNow, &BodyFate<'_>),
    ) -> Option<DerivedMoon> {
        self.record_moon_driven(epoch, body, (now, fate), PassDriver::Solve)
    }

    /// [`record_moon`](Self::record_moon), its passes driven by `driver`.
    #[cfg(test)]
    #[must_use]
    fn record_moon_driven(
        &self,
        epoch: &Epoch<'_>,
        body: &Body,
        (now, fate): (&ParentNow, &BodyFate<'_>),
        driver: PassDriver,
    ) -> Option<DerivedMoon> {
        let Part::Moon(moon) = &body.part else {
            return None;
        };
        let orbit = moon
            .satellite
            .orbit_at(&moon.parent, epoch.ctx.age_at(epoch.t));
        let case = MoonNow {
            body,
            moon,
            orbit,
            parent: now,
            fate,
        };
        let spin = Self::moon_spin(body, fate, epoch.t);
        self.moon_derived(epoch, &HourCosines::new(), &case, spin, driver)
    }

    /// The spin at `t` of the moon `body` under its held law and its planet's orbit then, as its
    /// planet's history `fate` puts it ([`SeasonalSpin::of`], [`SeasonsFrom::Planet`]): `None`
    /// for a moon with no law, or a planet with no orbit then.
    #[must_use]
    fn moon_spin(body: &Body, fate: &BodyFate<'_>, t: UniverseTime) -> Option<SeasonalSpin> {
        let rotation = body.rotation.as_ref()?;
        let at = fate.at(t);
        let seasonal = at.orbit()?;
        Some(SeasonalSpin::of(
            rotation.frame(),
            t,
            seasonal,
            SeasonsFrom::Planet,
        ))
    }

    /// The derivation of the present moon `now` describes at the epoch's time (P14.T17.b): its own
    /// volatiles (P14.T56.c), its inventory, atmosphere and tides read at its own bulk
    /// ([`moon_own_bulk`](Self::moon_own_bulk); T56.c's follow-up and P14.T56.d); `None` where
    /// this generator version does not compute it.
    ///
    /// Its passes are over its record under the spin `spin` (P14.T54.a): its held law's, or for
    /// its law's rounds the locked spin or the first round's ([`moon_law`](Self::moon_law)), with
    /// its insolation table over the call chain's hour angles' cosines `cosines`; with no spin,
    /// the passes' stand-in (no rotation law, no basins, the bulk material's regolith). `driver`
    /// drives them.
    #[must_use]
    fn moon_derived(
        &self,
        epoch: &Epoch<'_>,
        cosines: &HourCosines,
        now: &MoonNow<'_>,
        spin: Option<SeasonalSpin>,
        driver: PassDriver,
    ) -> Option<DerivedMoon> {
        let (moon, parent) = (now.moon, now.parent);
        let (Some(nursery), Some(sky)) = (&parent.nursery, &parent.sky) else {
            // A parent whose satellites' nursery or sky cannot be built (a giant beyond the
            // cooling fit's 13 Jupiter masses, which placement never makes) has moons whose bulk
            // this generator version does not compute.
            return None;
        };
        let inputs = Self::moon_inputs(moon, &now.orbit, parent);
        let frame;
        let table;
        let passes;
        let record: Option<&dyn MoonRecordOf> = match spin {
            Some(spin) => {
                frame = self.surface_frame(epoch, now.body, now.fate);
                table = ZonalInsolation::of(spin.obliquity, cosines);
                passes = SatellitePasses {
                    system: self,
                    frame: &frame,
                    spin,
                    table: &table,
                    now,
                    host: self.crust_host(epoch.ctx),
                };
                Some(&passes)
            }
            None => None,
        };
        // As above: the derivation refuses only inputs the generator never makes, so its error
        // carries nothing a caller could act on.
        derive_moon_under(
            &inputs,
            &moon.parent,
            nursery,
            sky,
            (epoch.ctx.age_at_epoch(), epoch.t),
            parent.shades.as_ref(),
            MoonPasses { record, driver },
        )
        .ok()
    }

    /// The moon `now` describes derived before its passes (lever (e); [`moon_before_passes`]):
    /// its shape and its bulk then, which its law's rounds read (P14.T54.a); `None` where this
    /// generator version does not compute it.
    #[must_use]
    fn moon_bulk_before_passes(epoch: &Epoch<'_>, now: &MoonNow<'_>) -> Option<MoonBeforePasses> {
        let (moon, parent) = (now.moon, now.parent);
        let (Some(nursery), Some(sky)) = (&parent.nursery, &parent.sky) else {
            return None;
        };
        moon_before_passes(
            &Self::moon_inputs(moon, &now.orbit, parent),
            &moon.parent,
            nursery,
            sky,
            (epoch.ctx.age_at_epoch(), epoch.t),
            parent.shades.as_ref(),
        )
        .ok()
    }

    /// What the moon `moon` on `orbit`, whose parent is as `parent` says, gives its derivation:
    /// its mass, orbit, radius rank, own volatiles and own bulk.
    #[must_use]
    fn moon_inputs(moon: &MoonPart, orbit: &KeplerElements, parent: &ParentNow) -> MoonInputs {
        let satellite = &moon.satellite;
        MoonInputs {
            mass: satellite.mass(),
            orbit: *orbit,
            radius_rank: satellite.radius_rank(),
            volatiles: satellite.own_volatiles(),
            bulk: Self::moon_own_bulk(moon, parent),
        }
    }

    /// The bulk the moon `moon`, whose parent is as `parent` says, states, at which its inventory,
    /// atmosphere and tides are read (P14.T56.c's follow-up, P14.T56.d): a regular moon's as its
    /// derivation gives it, and a giant-impact moon's or a capture's radius and fractions as its
    /// record's bulk ([`moon_bulk_of`](Self::moon_bulk_of)) states them.
    #[must_use]
    fn moon_own_bulk(moon: &MoonPart, parent: &ParentNow) -> MoonBulk {
        match Self::stated_moon_bulk(moon, parent) {
            Some((radius, _, fractions)) => MoonBulk::Stated { radius, fractions },
            None => MoonBulk::Derived,
        }
    }

    /// The radius, density and mass fractions that the origin of the moon `moon`, whose parent is
    /// as `parent` says, states apart from its derivation: a giant-impact moon's of its own
    /// density, with its parent's fractions if the parent is icy and rock otherwise, and a
    /// capture's of its own radius, with the ice its density holds. `None` for a regular moon,
    /// whose are its derivation's.
    #[must_use]
    fn stated_moon_bulk(
        moon: &MoonPart,
        parent: &ParentNow,
    ) -> Option<(Metres, KilogramsPerCubicMetre, MassFractions)> {
        let satellite = &moon.satellite;
        let mass = satellite.mass();
        match satellite.moon() {
            SatelliteMoon::Regular(_) => None,
            SatelliteMoon::GiantImpact(impact) => {
                let fractions = match (moon.parent.class(), &parent.derived) {
                    (PlanetClass::Icy, Some(planet)) => planet.fractions(),
                    _ => MassFractions::solid(0.0, 1.0, 0.0),
                };
                Some((
                    sphere_radius(mass, impact.density()),
                    impact.density(),
                    fractions,
                ))
            }
            SatelliteMoon::Captured(captured) => {
                let density = sphere_density(mass, captured.radius());
                let ice = ice_fraction_of(density);
                Some((
                    captured.radius(),
                    density,
                    MassFractions::solid(0.0, 1.0 - ice, ice),
                ))
            }
        }
    }

    /// The bulk of the moon `moon` derived as `derived`, whose parent is as `parent` says: a
    /// regular moon's from its own derivation, a giant-impact moon's of its own density and a
    /// capture's of its own radius ([`stated_moon_bulk`](Self::stated_moon_bulk)), each with the
    /// flux and temperature of the derivation.
    #[must_use]
    fn moon_bulk_of(moon: &MoonPart, derived: &DerivedMoon, parent: &ParentNow) -> BulkProperties {
        Self::moon_figure(
            moon,
            (derived.radius(), derived.density(), derived.fractions()),
            parent,
        )
        .with_temperature(derived.equilibrium_temperature())
    }

    /// [`moon_bulk_of`](Self::moon_bulk_of)'s bulk of the moon `moon`, whose parent is as `parent`
    /// says and whose derivation gives it the radius, density and fractions `derived`, with no
    /// derivation's temperatures: a giant-impact moon's and a capture's as their origin states
    /// them, a regular moon's its derivation's. What its passes and its law read of it before
    /// them, from its shape (P14.T54.a).
    #[must_use]
    fn moon_figure(
        moon: &MoonPart,
        derived: (Metres, KilogramsPerCubicMetre, MassFractions),
        parent: &ParentNow,
    ) -> FigureBulk {
        let mass = moon.satellite.mass();
        let (radius, density, fractions) = Self::stated_moon_bulk(moon, parent).unwrap_or(derived);
        let r = radius.value();
        FigureBulk {
            radius: EarthRadii::from(radius),
            density,
            surface_gravity: MetresPerSecondSquared::new(GM_EARTH * mass.value() / (r * r)),
            class: PlanetClass::of(mass, &fractions),
            fractions,
        }
    }

    /// The figure bulk of the moon `moon` of shape `shape`, whose derivation's bulk before its
    /// passes is `bulk` and whose parent is as `parent` says ([`moon_figure`](Self::moon_figure)
    /// at [`DerivedMoon`]'s radius, density and fractions, to the bit).
    #[must_use]
    fn moon_figure_of_shape(
        moon: &MoonPart,
        shape: &MoonShape,
        fractions: &MassFractions,
        parent: &ParentNow,
    ) -> FigureBulk {
        Self::moon_figure(
            moon,
            (shape.radius, shape.density, shape.fractions(fractions)),
            parent,
        )
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
            .envelope(Section::NotApplicable)
            .hooks(Section::NotApplicable)
            .photometry(Section::NotApplicable)
            .figure(Section::NotApplicable)
            .rotation(Section::NotApplicable);
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
                let bounds = present_halo_bounds(ctx, halo, epoch.t);
                match halo
                    .halo
                    .at(mass_now, SolarMassesPerYear::new(rate), bounds)
                {
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
            .envelope(Section::NotApplicable)
            .hooks(Section::NotApplicable)
            .photometry(Section::NotApplicable)
            .figure(Section::NotApplicable)
            .rotation(Section::NotApplicable)
            .population(now.map_or(Section::NotApplicable, |halo| {
                Section::Ok(Population::CometaryHalo(halo))
            }))
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }

    /// Everything [`derive_body`] computes of the present planet or member `body` of `zone` at
    /// the epoch's time, placed as `placed` says then, about hosts of total mass `host_mass`, and
    /// the sky its moons see (P14.T17.b's [`MoonSky`]): its passes over `record`, driven by
    /// `driver` (P14.T54.a).
    #[must_use]
    fn derive(
        &self,
        epoch: &Epoch<'_>,
        body: &Body,
        zone: &OrbitZone,
        placed: &PlacedBody,
        host_mass: SolarMasses,
        (record, driver): (PassRecord<'_>, PassDriver),
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
        .expect("a present body orbits a positive mass and at least one star")
        .with_shades(epoch.shades.as_ref());
        let age = epoch.ctx.age_at_epoch();
        let derived = derive_body_driven(driver, placed, &hosts, disc, age, epoch.t, record)
            .expect(
                "a present body's system is born, and a placed planet lies below 13 Jupiter masses",
            );
        let has_moons = self.children(body.index).next().is_some();
        let sky = has_moons
            .then(|| MoonSky::new(&derived, placed.orbit_now(), &hosts).ok())
            .flatten();
        (derived, sky)
    }
}

/// What a body's rotation law and its figure read of its bulk (P14.T14, P14.T46.d): its radius,
/// class and mass fractions, its record's bulk's ([`BulkProperties`]), or for the rotation pass
/// its derivation's before its passes (lever (e1)).
#[derive(Debug, Clone, Copy, PartialEq)]
struct SpinBulk {
    radius: EarthRadii,
    class: PlanetClass,
    fractions: MassFractions,
}

impl From<&BulkProperties> for SpinBulk {
    fn from(bulk: &BulkProperties) -> Self {
        Self {
            radius: bulk.radius(),
            class: bulk.class(),
            fractions: bulk.fractions(),
        }
    }
}

impl From<&FigureBulk> for SpinBulk {
    fn from(bulk: &FigureBulk) -> Self {
        Self {
            radius: bulk.radius,
            class: bulk.class,
            fractions: bulk.fractions,
        }
    }
}

/// A body's record's bulk with no derivation's temperatures (P14.T54.a): its radius, density,
/// surface gravity, class and mass fractions, as its record's bulk section states them
/// ([`BulkProperties`]), which its passes' figures and climate read before the passes, the record
/// after them, to the bit.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FigureBulk {
    radius: EarthRadii,
    density: KilogramsPerCubicMetre,
    surface_gravity: MetresPerSecondSquared,
    class: PlanetClass,
    fractions: MassFractions,
}

impl FigureBulk {
    /// A planet's or a member's bulk from its derivation's bulk before its passes `bulk`, the
    /// record's rule ([`BulkProperties::from`] of its derivation, an icy member's ruling 112.8
    /// bulk, [`icy_member_figure`]).
    #[must_use]
    fn of_primary(body: &Body, bulk: &PassBulk) -> Self {
        icy_member_figure(body, bulk.mass).unwrap_or(Self {
            radius: bulk.radius,
            density: bulk.density,
            surface_gravity: bulk.surface_gravity,
            class: bulk.class,
            fractions: bulk.fractions,
        })
    }

    /// The record's bulk section of this bulk at the derivation's equilibrium temperature
    /// `equilibrium`, with no effective temperature of its own.
    #[must_use]
    const fn with_temperature(self, equilibrium: Kelvin) -> BulkProperties {
        BulkProperties::new(
            self.radius,
            self.density,
            self.surface_gravity,
            self.class,
            self.fractions,
            equilibrium,
        )
    }

    /// The record's bulk section's figure bulk.
    #[must_use]
    fn of_record(bulk: &BulkProperties) -> Self {
        Self {
            radius: bulk.radius(),
            density: bulk.density(),
            surface_gravity: bulk.surface_gravity(),
            class: bulk.class(),
            fractions: bulk.fractions(),
        }
    }

    /// The spun climate's record of this bulk under the spin `spin` and the insolation table
    /// `table`: its gravity, its radius and its material ([`RecordSpin`]).
    #[must_use]
    fn record_spin<'a>(&self, spin: SeasonalSpin, table: &'a ZonalInsolation) -> RecordSpin<'a> {
        RecordSpin {
            spin,
            table,
            surface_gravity: self.surface_gravity,
            radius: Metres::from(self.radius),
            material: SurfaceMaterial::of(&self.fractions),
        }
    }
}

/// What a body's surface reads of its system at a time beyond its derivation and its spin, its
/// passes' and its record's alike ([`PlanetarySystem::surface_frame`], P14.T54.a).
struct SurfaceFrame<'a> {
    /// Its primary: itself, or a moon's planet.
    primary: &'a Body,
    /// The hosts' light over the primary's past.
    past: PastSky<'a>,
    /// The primary's orbit at the time, which sets the seasons.
    seasonal: Option<KeplerElements>,
    /// The impactors' sources about the primary's host.
    sources: Option<ImpactorSources>,
    /// Its continental asymptote f∞ (P14.T48.a), word 0 of its `body.continents` stream.
    continental_asymptote: f64,
    /// What its cold traps read beyond its sky (P14.T51.f).
    cold_traps: ColdTrapCase,
    /// The system's \[Fe/H\].
    fe_h: Dex,
    /// The system's age at the time.
    age: Years,
}

impl SurfaceFrame<'_> {
    /// What the figures of a body of mass `mass`, record's bulk `bulk` and inventory's water
    /// `inventory_water`, turning at the rotation period `rotation_period`, whose crater contract
    /// reads `craters`, read beyond its air and its tide; `None` with no impactor sources.
    #[must_use]
    fn figures_of(
        &self,
        (mass, bulk, inventory_water): (EarthMasses, FigureBulk, Kilograms),
        rotation_period: Seconds,
        craters: SurfaceCraters,
    ) -> Option<FiguresOf<'_>> {
        Some(FiguresOf {
            mass,
            bulk,
            inventory_water,
            rotation_period,
            fe_h: self.fe_h,
            age: self.age,
            craters,
            sources: self.sources.as_ref()?,
            continental_asymptote: self.continental_asymptote,
            past: &self.past,
        })
    }
}

/// What a body's global figures read beyond its air and its tide (P14.T54.a): its mass, its
/// record's bulk, its inventory's water, its rotation period, the system's \[Fe/H\] and age, its
/// crater inputs with its host's impactor sources, its continental asymptote and its hosts' past
/// light. Its passes' contexts and its record's figures build their [`FiguresInputs`] by the one
/// function ([`inputs`](Self::inputs)), so that the record's figures are its published pass's
/// K(P*) to the bit.
#[derive(Clone, Copy)]
struct FiguresOf<'a> {
    mass: EarthMasses,
    bulk: FigureBulk,
    inventory_water: Kilograms,
    rotation_period: Seconds,
    fe_h: Dex,
    age: Years,
    craters: SurfaceCraters,
    sources: &'a ImpactorSources,
    continental_asymptote: f64,
    past: &'a PastSky<'a>,
}

impl FiguresOf<'_> {
    /// The figures' inputs of the atmosphere `air`, with the tidal heat over the surface and the
    /// subsurface ocean `tide` (a moon's, or none): its continents read whether it entered its
    /// hosts' water window holding water with that air (P14.T48.b's start condition,
    /// [`window_entry`]).
    #[must_use]
    fn inputs<'b>(
        &'b self,
        air: &'b Atmosphere,
        (tidal_heat_flow, subsurface_ocean): (WattsPerSquareMetre, Subsurface),
    ) -> FiguresInputs<'b> {
        let radius = Metres::from(self.bulk.radius);
        let window = |age: Years| self.past.window_at(age);
        let entry = window_entry(&WindowEntryInputs {
            atmosphere: air,
            water: self.inventory_water,
            radius,
            molten_until: self.craters.molten_until,
            age: self.age,
            window: &window,
        });
        FiguresInputs {
            atmosphere: air,
            mass: self.mass,
            radius,
            surface_gravity: self.bulk.surface_gravity,
            class: self.bulk.class,
            fractions: self.bulk.fractions,
            inventory_water: self.inventory_water,
            rotation_period: self.rotation_period,
            fe_h: self.fe_h,
            age: self.age,
            tidal_heat_flow,
            subsurface_ocean,
            craters: CraterInputs {
                orbit: self.craters.orbit,
                density: self.bulk.density,
                sources: self.sources,
                molten_until: self.craters.molten_until,
                last_giant_impact: self.craters.last_giant_impact,
            },
            continents: Continents::Grown {
                asymptote: self.continental_asymptote,
                window: entry,
            },
        }
    }

    /// The figures' first stage of the atmosphere `air` with the tide `tide`
    /// ([`pass_figures`]): `None` for a body with no solid surface or a stage refused, which a
    /// pass's context reads as no figures, C₀'s.
    #[must_use]
    fn first_stage(
        &self,
        air: &Atmosphere,
        tide: (WattsPerSquareMetre, Subsurface),
    ) -> Option<PassFigures> {
        pass_figures(&self.inputs(air, tide)).ok().flatten()
    }
}

/// What a system's host gives every body's crust's basis (P14.T51.c; [`RecordCrust`]): its Mg/Si
/// by number, its \[α/Fe\] and its \[Fe/H\].
#[derive(Debug, Clone, Copy, PartialEq)]
struct CrustHost {
    host_magnesium_to_silicon: f64,
    alpha_fe: Dex,
    fe_h: Dex,
}

/// What a body's record gives its crust's basis beyond its bulk (P14.T54.a; [`RecordCrust`]): its
/// origin, its formation time, a moon's side of its parent's ice line, and its host's.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CrustOf {
    origin: CrustOrigin,
    formation: Megayears,
    snow_line_side: Option<SnowLineSide>,
    host: CrustHost,
}

impl CrustOf {
    /// The record's crust inputs with the record's bulk `bulk` at the mass `mass`.
    #[must_use]
    fn with_bulk(&self, bulk: &FigureBulk, mass: EarthMasses) -> RecordCrust {
        RecordCrust {
            origin: self.origin,
            formation: self.formation,
            snow_line_side: self.snow_line_side,
            host_magnesium_to_silicon: self.host.host_magnesium_to_silicon,
            alpha_fe: self.host.alpha_fe,
            fe_h: self.host.fe_h,
            radius: Metres::from(bulk.radius),
            mass,
            fractions: bulk.fractions,
        }
    }
}

/// A planet's or member's passes over its record (P14.T54.a; [`PassRecordOf`]): its spin under its
/// held law and its own orbit then, its insolation table, its record's bulk rule, what its figures
/// read beyond its air, so that each pass's context is K of the pass before, and what its crust's
/// basis reads beyond its bulk.
struct PrimaryPasses<'a> {
    frame: &'a SurfaceFrame<'a>,
    spin: SeasonalSpin,
    table: &'a ZonalInsolation,
    body: &'a Body,
    craters: Option<SurfaceCraters>,
    crust: Option<CrustOf>,
}

impl PassRecordOf for PrimaryPasses<'_> {
    fn spin(&self, bulk: &PassBulk) -> RecordSpin<'_> {
        FigureBulk::of_primary(self.body, bulk).record_spin(self.spin, self.table)
    }

    fn crust(&self, bulk: &PassBulk) -> Option<RecordCrust> {
        let crust = self.crust?;
        Some(crust.with_bulk(&FigureBulk::of_primary(self.body, bulk), bulk.mass))
    }

    fn context(&self, bulk: &PassBulk, air: &Atmosphere, _: Kelvin) -> PassContext {
        let figures = self.craters.and_then(|craters| {
            self.frame
                .figures_of(
                    (
                        bulk.mass,
                        FigureBulk::of_primary(self.body, bulk),
                        bulk.inventory_water,
                    ),
                    self.spin.rotation_period,
                    craters,
                )?
                .first_stage(
                    air,
                    (WattsPerSquareMetre::new(0.0), Subsurface::NotComputed),
                )
        });
        PassContext::of(figures.as_ref(), None)
    }
}

/// A moon at the epoch's time as its derivation reads it: the body, its part, its orbit about its
/// planet then, its planet as `parent` says then, and its planet's history, which sets its
/// seasons and its past light.
#[derive(Clone, Copy)]
struct MoonNow<'a> {
    body: &'a Body,
    moon: &'a MoonPart,
    orbit: KeplerElements,
    parent: &'a ParentNow,
    fate: &'a BodyFate<'a>,
}

/// A moon's passes over its record (P14.T54.a; [`MoonRecordOf`]): its spin, its insolation table,
/// its record's bulk rule, what its figures read beyond its air and its tide, and what its crust's
/// basis reads of its host.
struct SatellitePasses<'a> {
    system: &'a PlanetarySystem,
    frame: &'a SurfaceFrame<'a>,
    spin: SeasonalSpin,
    table: &'a ZonalInsolation,
    now: &'a MoonNow<'a>,
    host: CrustHost,
}

impl MoonRecordOf for SatellitePasses<'_> {
    fn spin(&self, shape: &MoonShape, bulk: &PassBulk) -> RecordSpin<'_> {
        PlanetarySystem::moon_figure_of_shape(
            self.now.moon,
            shape,
            &bulk.fractions,
            self.now.parent,
        )
        .record_spin(self.spin, self.table)
    }

    fn crust(&self, shape: &MoonShape, bulk: &PassBulk) -> Option<RecordCrust> {
        let MoonNow {
            body,
            moon,
            parent,
            orbit,
            ..
        } = *self.now;
        let crust = self.system.moon_crust(
            (self.frame.primary, body, moon),
            shape.ice_fraction,
            &orbit,
            self.host,
        )?;
        let figure = PlanetarySystem::moon_figure_of_shape(moon, shape, &bulk.fractions, parent);
        Some(crust.with_bulk(&figure, moon.satellite.mass()))
    }

    fn figures(
        &self,
        shape: &MoonShape,
        bulk: &PassBulk,
        air: &Atmosphere,
        tide: &MoonTideAt,
    ) -> Option<PassFigures> {
        let MoonNow {
            moon,
            parent,
            orbit,
            ..
        } = *self.now;
        let craters =
            self.system
                .moon_craters(self.frame.primary, moon, shape.ice_fraction, &orbit)?;
        let figure = PlanetarySystem::moon_figure_of_shape(moon, shape, &bulk.fractions, parent);
        // A regular moon's ocean is its flag's; no rule publishes another moon's
        // ([`SurfaceInterior::of_moon`]).
        let subsurface = match moon.satellite.moon() {
            SatelliteMoon::Regular(_) => tide.subsurface,
            SatelliteMoon::GiantImpact(_) | SatelliteMoon::Captured(_) => Subsurface::NotComputed,
        };
        self.frame
            .figures_of(
                (moon.satellite.mass(), figure, bulk.inventory_water),
                self.spin.rotation_period,
                craters,
            )?
            .first_stage(air, (tide.heat_flux, subsurface))
    }
}

/// A moon's held rotation law, the round of [`PlanetarySystem::moon_law`] that held it, and the
/// locked spin its first round read.
#[derive(Debug, Clone, PartialEq)]
struct HeldMoonLaw {
    law: BodyRotation,
    round: MoonRound,
    locked: SeasonalSpin,
}

/// Which round held a moon's law (decision-p14-t54a-passes §2.3).
#[derive(Debug, Clone, PartialEq)]
enum MoonRound {
    /// Round 1's, locked at the time under the locked spin.
    First,
    /// Round 2's, under round 1's law, which was not locked then.
    Second {
        /// Round 1's law.
        first: Box<BodyRotation>,
    },
}

/// What the rotation pass and the Laplace frames read of a planet's or member's record at
/// [`parent_time`] (P14.T46.b, P14.T56.a): its orbit section's elements, what its bulk gives its
/// law and figure, and its mass.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RecordThen {
    orbit: KeplerElements,
    bulk: SpinBulk,
    mass: EarthMasses,
}

impl RecordThen {
    /// What `record` holds of it: `None` unless its orbit, its bulk and its mass are all there, as
    /// for a body present at its time (the tests' oracle of lever (e1)).
    #[cfg(test)]
    #[must_use]
    fn of_record(record: &BodyRecord) -> Option<Self> {
        let (Section::Ok(orbit), Section::Ok(bulk), Section::Ok(mass)) =
            (record.orbit(), record.bulk(), record.mass())
        else {
            return None;
        };
        Some(Self {
            orbit: *orbit.elements(),
            bulk: SpinBulk::from(bulk),
            mass: *mass,
        })
    }
}

/// How [`generate`] and the moons' parents read the bodies they derive at [`parent_time`], a
/// test-only switch of lever (e) ([`with_pass_free`]).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassFree {
    /// Before the passes, as the generator reads them: (e1) the rotation pass's and the Laplace
    /// frames' records, (e2) a member as its moons' parent, and (e3) a parent as its moons read it,
    /// with no record.
    Taken,
    /// As before the lever: a snapshot at `parent_time`, each member's whole derivation and each
    /// parent's whole record: the unlevered path, the oracle.
    Unlevered,
    /// Both: each reading asserted the unlevered one's to the bit (by its rendering), and counted.
    Checked,
}

/// What [`pass_free_read`] reads.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PassFreeRead {
    /// (e1) A system's records at `parent_time`.
    Records,
    /// (e2) A member as its moons' parent.
    Member,
    /// (e3) A parent as its moons read it.
    Parent,
}

/// What [`PassFree::Checked`] has checked on its thread: the systems' records at `parent_time`
/// (e1), the members (e2) and the parents (e3).
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PassFreeCounts {
    pub(crate) records: u64,
    pub(crate) members: u64,
    pub(crate) parents: u64,
}

#[cfg(test)]
thread_local! {
    /// The switch on this thread, a test's own: tests run on threads of one process.
    static PASS_FREE: core::cell::Cell<PassFree> = const { core::cell::Cell::new(PassFree::Taken) };
    /// What [`PassFree::Checked`] has checked on this thread.
    static PASS_FREE_COUNTS: core::cell::Cell<PassFreeCounts> = const {
        core::cell::Cell::new(PassFreeCounts {
            records: 0,
            members: 0,
            parents: 0,
        })
    };
}

/// `run`'s result with the bodies [`generate`] and the moons' parents derive at [`parent_time`]
/// read as `pass_free` says, and what it checked. The switch is set back after.
#[cfg(test)]
#[must_use]
pub(crate) fn with_pass_free<R>(
    pass_free: PassFree,
    run: impl FnOnce() -> R,
) -> (R, PassFreeCounts) {
    let before = PASS_FREE.with(|cell| cell.replace(pass_free));
    PASS_FREE_COUNTS.with(|cell| cell.set(PassFreeCounts::default()));
    let result = run();
    PASS_FREE.with(|cell| cell.set(before));
    (
        result,
        PASS_FREE_COUNTS.with(|cell| cell.replace(PassFreeCounts::default())),
    )
}

/// The reading `what` under the test's switch: `None` under [`PassFree::Taken`], for the caller to
/// read before the passes; `unlevered()` under [`PassFree::Unlevered`]; and under
/// [`PassFree::Checked`] `levered()`, asserted `unlevered()`'s by their renderings (each `f64` at
/// its shortest round trip, so to the bit) and counted.
#[cfg(test)]
fn pass_free_read<T: core::fmt::Debug>(
    what: PassFreeRead,
    levered: impl FnOnce() -> T,
    unlevered: impl FnOnce() -> T,
) -> Option<T> {
    match PASS_FREE.with(core::cell::Cell::get) {
        PassFree::Taken => None,
        PassFree::Unlevered => Some(unlevered()),
        PassFree::Checked => {
            let read = levered();
            let oracle = unlevered();
            assert_eq!(
                format!("{read:?}"),
                format!("{oracle:?}"),
                "{what:?} read before the passes is not the whole derivation's"
            );
            PASS_FREE_COUNTS.with(|cell| {
                let mut counts = cell.get();
                match what {
                    PassFreeRead::Records => counts.records += 1,
                    PassFreeRead::Member => counts.members += 1,
                    PassFreeRead::Parent => counts.parents += 1,
                }
                cell.set(counts);
            });
            Some(read)
        }
    }
}

/// What a planet or member is at a time, as its moons and rings read it.
#[derive(Debug, Clone, PartialEq)]
struct ParentNow {
    state: BodyState,
    /// The next change of its state or segment inside the clock window, or anywhere for a time
    /// before it ([`FateAt::changes_at`]), which its moons' records hold until; not its own drift
    /// cell's end (P14.T45.a).
    changes_at: Option<UniverseTime>,
    position: Option<SystemPosition>,
    derived: Option<DerivedBody>,
    /// The spin its derivation's passes read, its held law's under its own orbit then, which its
    /// record's surface reads too (P14.T54.a): `None` for a body not present or with no law.
    spin: Option<SeasonalSpin>,
    sky: Option<MoonSky>,
    nursery: Option<MoonNursery>,
    /// What its moons' shades share (lever (d)): its epoch's book with its own glow in it, if its
    /// epoch holds one and it has moons.
    shades: Option<ShadeBook>,
}

/// The bulk composition hook of a body of mass fractions `fractions` and element inventory
/// `elements` (P14.T50.a) in the system of `ctx`, at its \[Fe/H\] and \[α/Fe\] (P14.T23).
#[must_use]
fn bulk_composition(
    ctx: &SystemContext,
    fractions: MassFractions,
    elements: ElementInventory,
) -> Section<BulkComposition> {
    Section::Ok(BulkComposition::new(
        fractions,
        elements,
        ctx.fe_h(),
        ctx.alpha_fe(),
    ))
}

/// What a body's surface conditions read of its derivation at a time (P14.T24.a).
#[derive(Debug, Clone, Copy)]
struct SurfaceSky<'a> {
    atmosphere: &'a Atmosphere,
    equilibrium_temperature: Kelvin,
    flux: EarthFluxes,
    xuv_flux: WattsPerSquareMetre,
    /// The bare regolith's Bond albedo its derivation gives (P14.T51.i), which its cold traps read.
    regolith_albedo: f64,
    /// What its passes over its record leave for it (P14.T54.a, rule (f)), `None` over the
    /// stand-in.
    spun: Option<&'a SpunPasses>,
    /// Its published pass's lid, whose row its regolith read (P14.T54.a), from which its record's
    /// crust is built; `None` where its passes took none.
    crust_lid: Option<&'a CrustLid>,
}

/// What a body's global figures read at a time beyond its bulk section, its sky and its spin
/// (P14.T24.b): its mass, its volatile inventory's water, the tide its parent raises in it, and
/// its subsurface ocean as its rule computes it.
#[derive(Debug, Clone, Copy)]
struct SurfaceInterior {
    mass: EarthMasses,
    inventory_water: Kilograms,
    tidal_heat_flow: WattsPerSquareMetre,
    subsurface_ocean: Subsurface,
}

impl SurfaceSky<'_> {
    /// The sky of a planet or a member derived as `derived`.
    #[must_use]
    fn of_body(derived: &DerivedBody) -> SurfaceSky<'_> {
        SurfaceSky {
            atmosphere: derived.atmosphere(),
            equilibrium_temperature: derived.equilibrium_temperature(),
            flux: derived.flux(),
            xuv_flux: derived.xuv_flux(),
            regolith_albedo: derived.regolith_albedo(),
            spun: derived.spun(),
            crust_lid: derived.crust_lid(),
        }
    }

    /// The sky of a moon derived as `derived`.
    #[must_use]
    fn of_moon(derived: &DerivedMoon) -> SurfaceSky<'_> {
        SurfaceSky {
            atmosphere: derived.atmosphere(),
            equilibrium_temperature: derived.equilibrium_temperature(),
            flux: derived.flux(),
            xuv_flux: derived.xuv_flux(),
            regolith_albedo: derived.regolith_albedo(),
            spun: derived.spun(),
            crust_lid: derived.crust_lid(),
        }
    }
}

impl SurfaceInterior {
    /// The interior of a planet or a member derived as `derived`: no tide, and no rule for an
    /// ocean under its surface.
    #[must_use]
    fn of_body(derived: &DerivedBody) -> Self {
        Self {
            mass: derived.mass(),
            inventory_water: derived.inventory().water(),
            tidal_heat_flow: WattsPerSquareMetre::new(0.0),
            subsurface_ocean: Subsurface::NotComputed,
        }
    }

    /// The interior of the moon `moon`, derived as `derived`: its tide is its derivation's,
    /// P14.T17.b's (science-p14-t24b-laws §1.1's `q_tid`) at its own tides (P14.T56.d), and a
    /// regular moon's ocean is its flag's ([`DerivedMoon::subsurface`]: its branch's at its whole
    /// heat flow, decision-p14-moons §F-ocean F3); no rule publishes another moon's ocean, though
    /// its tides read one.
    #[must_use]
    fn of_moon(moon: &MoonPart, derived: &DerivedMoon) -> Self {
        let subsurface_ocean = match moon.satellite.moon() {
            SatelliteMoon::Regular(_) => derived.subsurface(),
            SatelliteMoon::GiantImpact(_) | SatelliteMoon::Captured(_) => Subsurface::NotComputed,
        };
        Self {
            mass: moon.satellite.mass(),
            inventory_water: derived.inventory().water(),
            tidal_heat_flow: derived.heat_flux(),
            subsurface_ocean,
        }
    }
}

/// What a body's surface conditions and global figures read of the time they are asked at
/// ([`published_surface`]): the hosts' light over the past up to it as the primary's orbit sees
/// it, which the volatile history reads (P14.T48.b; a moon's its planet's), the body's spin, which
/// its record computed once for its passes and its surface (P14.T54.a), its seasonal orbit then
/// (`None` where this version does not compute it), the system's \[Fe/H\] and age, and the hour
/// angles' cosines of the call chain that asks, which every body's zonal insolation table reads
/// (lever (h)).
#[derive(Debug, Clone, Copy)]
struct SurfaceTime<'a> {
    past: &'a PastSky<'a>,
    spin: SeasonalSpin,
    seasonal: Option<&'a KeplerElements>,
    fe_h: Dex,
    age: Years,
    cosines: &'a HourCosines,
}

/// What [`PlanetarySystem::surface_section`] reads of a body beyond its record's place: a planet's
/// or a member's derivation and bulk section, or a moon's, with its orbit about its planet then.
#[derive(Debug, Clone, Copy)]
enum SurfaceOf<'a> {
    /// A planet or a member.
    Body {
        derived: &'a DerivedBody,
        bulk: &'a BulkProperties,
    },
    /// A moon.
    Moon {
        moon: &'a MoonPart,
        derived: &'a DerivedMoon,
        bulk: &'a BulkProperties,
        orbit: &'a KeplerElements,
    },
}

/// What a body's surface conditions and global figures read of the body ([`published_surface`]):
/// its sky, its bulk section, its interior, and what its crater contract reads with its host's
/// impactor sources (`None` where this version does not compute them).
#[derive(Debug, Clone, Copy)]
struct SurfaceCase<'a> {
    sky: SurfaceSky<'a>,
    bulk: &'a BulkProperties,
    interior: SurfaceInterior,
    craters: Option<SurfaceCraters>,
    sources: Option<&'a ImpactorSources>,
    /// Its continental asymptote f∞ (P14.T48.a), word 0 of its `body.continents` stream.
    continental_asymptote: f64,
    /// What its cold traps read beyond its sky (P14.T51.f).
    cold_traps: ColdTrapCase,
    /// The rules its cold traps follow.
    record: RecordRules,
}

/// What a body's cold traps read beyond its sky and its spin (P14.T51.f): its exposed fraction,
/// word 0 of its `body.cold_traps` stream, and the radius of the brightest star its zone's bodies
/// orbit at the time, `None` before one forms.
#[derive(Debug, Clone, Copy)]
struct ColdTrapCase {
    exposed_fraction: f64,
    star_radius: Option<SolarRadii>,
}

/// What a body's water window reads of its hosts over its past (P14.T48.b; science-p14-t24b-laws
/// §3.3): the system's stars, each with the orbit its light reaches the body along, and the
/// history of the orbit the body goes round them on, a moon's its planet's.
#[derive(Debug, Clone)]
struct PastSky<'a> {
    ctx: &'a SystemContext,
    fate: &'a BodyFate<'a>,
    /// The present, as the system's age and the time it is asked at: the window at that age reads
    /// the time itself, which the round trip through the age would miss by up to some 30 s.
    now: (Years, UniverseTime),
    /// Each star of the system, with the separation its light is seen along: `None` for a star of
    /// the zone, seen along the body's own orbit then ([`Epoch::lights`]), and the lowest pair's
    /// orbit for a companion ([`companion_separation`]).
    stars: Vec<(usize, Option<(Metres, f64)>)>,
}

impl<'a> PastSky<'a> {
    /// The past light of `ctx`'s stars on the bodies of `zone`, one of which goes round them as
    /// `fate` tells, up to the present `t`.
    #[must_use]
    fn new(
        ctx: &'a SystemContext,
        zone: &OrbitZone,
        fate: &'a BodyFate<'a>,
        t: UniverseTime,
    ) -> Self {
        let h = ctx.hierarchy();
        let under = members_under(h);
        let members = members_of(zone);
        let stars = (0..ctx.stars().len())
            .map(|star| {
                let seen = ((members & (1 << star)) == 0).then(|| {
                    let separation = companion_separation(h, &under, members, star);
                    (
                        separation.semi_major_axis(),
                        separation.eccentricity().value(),
                    )
                });
                (star, seen)
            })
            .collect();
        Self {
            ctx,
            fate,
            now: (ctx.age_at(t), t),
            stars,
        }
    }

    /// Where the body lay against its hosts' water window at the system age `age`: each star's
    /// flux then, at its state then, along the body's orbit then for a star of its zone and along
    /// the pair's for a companion, as [`Epoch::lights`] and [`Epoch::companions`] see them now.
    /// [`WindowAt::DARK`] at an age the clock cannot hold or at which the body is not present, and
    /// nothing from a star not yet formed. At the present's own age it reads the present's time.
    #[must_use]
    fn window_at(&self, age: Years) -> WindowAt {
        let (now, at) = self.now;
        let time = if age.value().total_cmp(&now.value()).is_eq() {
            Some(at)
        } else {
            time_at_age(age, self.ctx.age_at_epoch())
        };
        let Some(t) = time else {
            return WindowAt::DARK;
        };
        let Some(orbit) = self.fate.orbit_at(t) else {
            return WindowAt::DARK;
        };
        let own = (orbit.semi_major_axis(), orbit.eccentricity().value());
        let stars = self.ctx.stars();
        WindowAt::of(self.stars.iter().filter_map(|&(star, seen)| {
            let state = stars[star].state_at(t)?;
            let (a, e) = seen.unwrap_or(own);
            Some((
                luminosity_flux(state.luminosity(), a, e),
                state.effective_temperature(),
            ))
        }))
    }
}

/// What a body's crater contract reads beyond its bulk section and its host's sources
/// (P14.T24.b's surface age, P14.T48.c): where it goes round, and the dates of its magma ocean's
/// end and its last giant impact ([`CraterInputs`]).
#[derive(Debug, Clone, Copy)]
struct SurfaceCraters {
    orbit: ImpactorOrbit,
    molten_until: Option<Years>,
    last_giant_impact: Option<Years>,
}

/// The surface section of the body `case` describes at `time` (P14.T24.a, T24.b, T48.a–e): its
/// surface conditions and global figures, and P14.T51.a's published condensates, which its liquids
/// and ices read ([`with_partition`]).
///
/// [`Section::NotModelled`] with no seasonal orbit, crater inputs or impactor sources, or for
/// inputs the figures refuse; [`Section::NotApplicable`] for a body with no surface; and
/// otherwise [`Section::Ok`], in science-p14-t24b-laws §4.6's order:
/// T48.b's start condition from the past sky, the figures' first stage with the continents it
/// gates, the conditions at the figures' heat flow and ocean weight θ, and the second stage from
/// the conditions: the volatile history from their liquid-water flag (P14.T48.b), the ocean, the
/// ice and the cloud, and then the conditions' climate regime (P14.T48.d), which reads the history
/// and the ice.
///
/// **Rule (f)** (P14.T54.a; decision-p14-t54a-passes, "Cost escalation (after Batch B)"): for a
/// body derived over its record, the conditions, their cold profile, the partition and the
/// condensates are built on the context its published pass read and that pass's θ, carried on its
/// derivation ([`SpunPasses`]), by the one computation the pass's area mix published over
/// ([`PassContext::partition_ground`], [`partition_over`](crate::planetary::hooks::figures::partition_over)),
/// so that they are its last pass's to the bit, but for an ice bulk's crust, which the record
/// lists and the pass reads as its regolith ([`CrustWater`]). The figures section stays its own
/// air's, K(P*), the first stage the solve computed last, reused: for a converged body it agrees
/// with the context read within 10⁻⁹, and a bracketed body's differs by its context residual. Over
/// the passes' stand-in, which no record with a law reads, the figures' own stand in.
///
/// The conditions' zonal profile is read off the body's insolation table, built once from its
/// spin's obliquity over the call chain's hour angles' cosines (the time's), and read by the
/// partition's cold profile and T24.b's ice alike (decision-p14-t54a-passes §C.3, lever (h)).
///
/// **The crust** (P14.T51.c, on the record from P14.T54.a; decision-pre-23-review item 17): the
/// published pass's lid with what only the record reads, the figures' volcanism and continental
/// fraction and the conditions' melt area (`CrustComposition::of`), so that the record's lid is
/// the row its last pass read, to the bit; the conditions' ground's inertia reads the lid the
/// published pass's context holds, as that pass's climate did. Not modelled over the passes'
/// stand-in and for a body whose passes took no lid.
#[must_use]
fn published_surface(time: &SurfaceTime<'_>, case: &SurfaceCase<'_>) -> Section<Surface> {
    let Some(seasonal) = time.seasonal else {
        return Section::NotModelled;
    };
    let (Some(craters), Some(sources)) = (case.craters, case.sources) else {
        return Section::NotModelled;
    };
    let (past, asymptote) = (time.past, case.continental_asymptote);
    let (sky, bulk, interior) = (&case.sky, case.bulk, &case.interior);
    let air = sky.atmosphere;
    let spin = time.spin;
    let window = |age: Years| past.window_at(age);
    let figures_of = FiguresOf {
        mass: interior.mass,
        bulk: FigureBulk::of_record(bulk),
        inventory_water: interior.inventory_water,
        rotation_period: spin.rotation_period,
        fe_h: time.fe_h,
        age: time.age,
        craters,
        sources,
        continental_asymptote: asymptote,
        past,
    };
    // Step 4's start condition (science-p14-t24b-laws §4.6), T48.b's, ahead of the conditions,
    // in the figures' continents.
    let figure_inputs =
        figures_of.inputs(air, (interior.tidal_heat_flow, interior.subsurface_ocean));
    // The record's figures are its own air's K(P*): the first stage its passes' solve computed
    // last, reused, the same bits (rule (f)).
    let reused = sky.spun.and_then(|spun| spun.next().figures());
    #[cfg(test)]
    let reused = reused.filter(|_| !FIRST_STAGE_RECOMPUTED.with(core::cell::Cell::get));
    let derived = match reused {
        Some(first) => Ok(Some(GlobalFigures::of_first_stage(&figure_inputs, first))),
        None => GlobalFigures::derive(&figure_inputs),
    };
    // The generator's bodies are in the figures' range: a refusal is a bug, which the tests'
    // debug builds catch, and it carries nothing a caller could act on, so the body's figures are
    // not modelled, as an uncomputed derivation's.
    debug_assert!(derived.is_ok(), "{derived:?}");
    let figures = match derived {
        Ok(Some(figures)) => figures,
        Ok(None) => return Section::NotApplicable,
        Err(_) => return Section::NotModelled,
    };
    let material = SurfaceMaterial::of(&bulk.fractions());
    let basis = surface_basis(case, &figures, material);
    let inputs = SurfaceInputs {
        atmosphere: air,
        equilibrium_temperature: sky.equilibrium_temperature,
        surface_gravity: bulk.surface_gravity(),
        radius: Metres::from(bulk.radius()),
        material,
        lid: basis.lid,
        stellar_flux: WattsPerSquareMetre::from(sky.flux),
        xuv_flux: sky.xuv_flux,
        spin,
        heat_flow: basis.heat_flow,
        ocean_weight: basis.ocean_weight,
    };
    let Some(conditions) = SurfaceConditions::derive(&inputs) else {
        return Section::NotApplicable;
    };
    let table = ZonalInsolation::of(spin.obliquity, time.cosines);
    let (conditions, (partition, published, ice)) = with_partition(
        &figures,
        case,
        (&spin, seasonal),
        conditions,
        (&table, &basis.ground),
    );
    // The second stage: the volatile history reads the conditions' liquid-water flag, the hosts'
    // light over the body's past and its outgassing then (P14.T48.b), and the ocean and the ice
    // read the conditions (science-p14-t24b-laws §4.6, steps 8–12).
    let history = VolatileHistory::derive(&VolatileHistoryInputs {
        atmosphere: air,
        water: interior.inventory_water,
        radius: Metres::from(bulk.radius()),
        liquid_water: conditions.liquid_water(),
        molten_until: craters.molten_until,
        age: time.age,
        window,
        outgassing: |age: Years| past_outgassing(&figure_inputs, age),
    });
    // T24.b's ice is the partition's cover's, read off the profile it built (lever (a)).
    let figures = figures.with_surface_under(&conditions, history, Some(&published), &table, ice);
    // Step 10, the climate regime (P14.T48.d): it reads the cover, which reads no regime (an ice
    // belt's band is the profile's cold equator, science-p14-t24b-laws §4.3), so it follows the
    // second stage with no loop.
    let reservoirs = partition.reservoirs();
    let conditions = conditions.with_climate(&ClimateInputs {
        mass: interior.mass,
        reservoirs: &reservoirs,
        from_colder: figures.from_colder(),
        ice_fraction: figures.ice_fraction(),
        ice_threshold: figures.ice_threshold(),
    });
    let crust = record_crust(sky.crust_lid, &figures, &conditions);
    Section::Ok(
        Surface::new(air.state(), material, conditions, figures, published)
            .expect(
                "conditions exist only off the gas-envelope state, and `with_climate` gives their \
                 regime",
            )
            .with_basis(basis)
            .with_crust(crust),
    )
}

/// The record's crust of a body whose published pass took the lid `lid`, of figures `figures`
/// and conditions `conditions` (P14.T51.c, on the record from P14.T54.a; item 17's constraint 1):
/// that lid with what only the record reads, the figures' volcanism and continental fraction and
/// the conditions' melt area (`CrustComposition::of`); `None` where the pass took no lid.
#[must_use]
fn record_crust(
    lid: Option<&CrustLid>,
    figures: &GlobalFigures,
    conditions: &SurfaceConditions,
) -> Option<CrustComposition> {
    let crust = CrustComposition::of(
        lid?,
        &CrustRecord {
            volcanism: figures.volcanism(),
            continental_fraction: figures.continental_fraction(),
            surface: conditions,
        },
    );
    // The figures' volcanism and continental fraction lie in range: a refusal is a bug, which the
    // tests' debug builds catch, and the crust is then not modelled.
    debug_assert!(crust.is_ok(), "{crust:?}");
    crust.ok()
}

/// What the conditions and condensates of the body `case` describes, of figures `figures` and bulk
/// material `material`, are built on (rule (f), P14.T54.a; [`SurfaceBasis`]): for a body derived
/// over its record, its published pass's context's ground, heat flow and θ, the ground of its
/// air's own water over that context's basins and share, with an ice bulk's crust listed; over the
/// passes' stand-in, the figures' own.
#[must_use]
fn surface_basis(
    case: &SurfaceCase<'_>,
    figures: &GlobalFigures,
    material: SurfaceMaterial,
) -> SurfaceBasis {
    let (sky, bulk) = (&case.sky, case.bulk);
    let Some(spun) = sky.spun else {
        return SurfaceBasis {
            ground: figures.partition_ground(bulk.surface_gravity()),
            heat_flow: figures.heat_flow(),
            ocean_weight: figures.ocean_weight(),
            lid: None,
        };
    };
    let air = sky.atmosphere;
    let water = WaterGate {
        state: air.state(),
        retained: air.retains_against_escape(Gas::Water),
        water: case.interior.inventory_water,
        radius: Metres::from(bulk.radius()),
        surface_gravity: bulk.surface_gravity(),
    };
    let context = spun.context();
    let ground = context.partition_ground(air, &water, material, CrustWater::of_record(material));
    // The θ its published pass carried is its partition's own, the one expression.
    debug_assert!(
        spun.ocean_weight()
            .get()
            .total_cmp(
                &hooks::surface::OceanWeight::of_potential_ocean(ground.potential_ocean).get()
            )
            .is_eq(),
        "the published pass's θ is its partition's: {spun:?}"
    );
    SurfaceBasis {
        ground,
        heat_flow: context.heat_flow(),
        ocean_weight: spun.ocean_weight(),
        lid: context.lid().map(|lid| lid.lithology),
    }
}

/// The conditions `conditions` of the body `case` describes, of figures `figures`, spin and seasonal
/// orbit `seasons`, with P14.T51.a's partition on the record and T24.b's water ice as its cover read
/// it ([`GlobalFigures::condensates_and_ice_over`]) on the ground `ground` (rule (f), the
/// published pass's context's): the conditions' liquid-water flag, the ocean and ices, and the
/// regime's reservoirs read it. Its cold traps (P14.T51.f) take the sky [`cold_trap_sky`] gives
/// and the bare regolith's albedo its derivation gives (P14.T51.i), under their own hold at 24,
/// which follows the case's rules, those in force. The cold profile reads the body's insolation
/// table `table`.
#[must_use]
fn with_partition(
    figures: &GlobalFigures,
    case: &SurfaceCase<'_>,
    seasons: (&SeasonalSpin, &KeplerElements),
    conditions: SurfaceConditions,
    (table, ground): (&ZonalInsolation, &PartitionGround),
) -> (SurfaceConditions, (Partition, Condensates, WaterIce)) {
    let condensates = figures.condensates_and_ice_over(
        case.sky.atmosphere,
        &conditions,
        ground,
        &RecordColdTraps {
            rules: case.record.cold_traps,
            sky: &cold_trap_sky(case, seasons),
            regolith_albedo: case.sky.regolith_albedo,
        },
        table,
    );
    let conditions = conditions.with_condensates(&condensates.1);
    (conditions, condensates)
}

#[cfg(test)]
thread_local! {
    /// Whether the record's figures compute their first stage again rather than reuse their
    /// published pass's K(P*), on this thread: the tests' oracle of the reuse.
    static FIRST_STAGE_RECOMPUTED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// `run`'s result with every record's figures computing their first stage again
/// ([`GlobalFigures::derive`]) rather than reusing their published pass's K(P*): a test-only
/// switch, set back after.
#[cfg(test)]
#[must_use]
pub(crate) fn with_first_stage_recomputed<R>(run: impl FnOnce() -> R) -> R {
    let before = FIRST_STAGE_RECOMPUTED.with(|cell| cell.replace(true));
    let result = run();
    FIRST_STAGE_RECOMPUTED.with(|cell| cell.set(before));
    result
}

/// What the cold traps of the body `case` describes read of its sky (P14.T51.f), under its spin and
/// seasonal orbit `seasons`: the greatest declination from its obliquity, the flux and the
/// brightest star's angular radius at the seasonal orbit's periapsis ([`ColdTrapSky::at_periapsis`]),
/// and its exposed fraction. The flux is the body's orbit-mean flux from every host, a moon's
/// planet's light and a companion's included, taken to periapsis as its seasonal host's.
#[must_use]
fn cold_trap_sky(
    case: &SurfaceCase<'_>,
    (spin, seasonal): (&SeasonalSpin, &KeplerElements),
) -> ColdTrapSky {
    ColdTrapSky::at_periapsis(
        spin.obliquity,
        WattsPerSquareMetre::from(case.sky.flux),
        (seasonal.semi_major_axis(), seasonal.eccentricity().value()),
        case.cold_traps.star_radius,
        case.cold_traps.exposed_fraction,
    )
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

/// How the present moon `moon` moves about its planet at `t`, and the time its record holds until,
/// inside the clock window: its planet's next change `parent_changes`, its loss, or for a
/// receding giant-impact moon the end of its drift cell (P14.T45.a).
///
/// A receding moon's phase is anchored at the epoch, held between the system's birth and the
/// moon's loss, where its elements are its own then.
#[must_use]
fn moon_trajectory(
    ctx: &SystemContext,
    moon: &MoonPart,
    t: UniverseTime,
    parent_changes: Option<UniverseTime>,
) -> (DriftingOrbit, Option<UniverseTime>) {
    let within = |time: UniverseTime| ClockWindow::contains(time).then_some(time);
    let age_at_epoch = ctx.age_at_epoch();
    let lost = moon_lost_at(&moon.satellite, age_at_epoch).filter(|&lost| lost > t);
    let next = earliest(parent_changes, lost);
    if let SatelliteMoon::GiantImpact(impact) = moon.satellite.moon() {
        let birth = time_at_age(Years::new(0.0), age_at_epoch)
            .expect("a system's birth, under 14 Gyr ago, is a time of the clock")
            .min(t);
        let anchor = lost.map_or(UniverseTime::EPOCH.max(birth), |lost| {
            UniverseTime::EPOCH.clamp(birth, lost.max(birth))
        });
        let anchored = moon.satellite.orbit_at(&moon.parent, ctx.age_at(anchor));
        if let Some(law) = impact.law(anchored, anchor) {
            let cell = drift_cell(&law, t, birth, next);
            return (cell.orbit, cell.holds_until(t, next));
        }
    }
    let orbit = moon.satellite.orbit_at(&moon.parent, ctx.age_at(t));
    (DriftingOrbit::fixed(orbit), next.and_then(within))
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
/// hierarchy's membership, and for a query that derives its bodies their shades' book.
struct Epoch<'c> {
    ctx: &'c SystemContext,
    t: UniverseTime,
    under: Vec<u32>,
    states: Vec<Option<StarState>>,
    xuv: Vec<XuvHistory>,
    /// What the shades of the bodies it derives share (lever (d), decision-p14-t54a-passes, "Cost
    /// escalation (after Batch B)" §C.3), built by the queries that derive
    /// ([`with_shades`](Self::with_shades)); `None` for the others, whose bodies' shades, if any,
    /// are computed body by body, to the same bits.
    shades: Option<ShadeBook>,
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
            xuv: ctx.xuv_histories(),
            shades: None,
        }
    }

    /// The same queries, with the book its bodies' shades share (lever (d)): the shade's
    /// constants, and the light of each star that shines at the time, at its effective
    /// temperature then ([`light`](Self::light)'s).
    #[must_use]
    fn with_shades(self) -> Self {
        let shades = ShadeBook::new(
            self.states
                .iter()
                .flatten()
                .filter(|state| state.luminosity().value() > 0.0)
                .map(StarState::effective_temperature),
        );
        Self {
            shades: Some(shades),
            ..self
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
    /// L, `T_eff` and R from its state), with its past: its X-ray and ultraviolet history
    /// (P14.T1.a) and the largest luminosity it has had (design note 11), which the atmospheres
    /// read (P14.T13).
    #[must_use]
    fn light(&self, star: usize) -> Option<HostLight> {
        let state = self.states[star]?;
        let peak = self.ctx.stars()[star].max_luminosity_until(self.t);
        Some(
            HostLight::new(
                state.luminosity(),
                state.effective_temperature(),
                state.radius(),
            )
            .expect("a star's state is finite and not negative, with a surface if it shines")
            .with_history(self.xuv[star], peak),
        )
    }

    /// The radius of the most luminous of the stars `zone`'s bodies orbit at the time, the first of
    /// equals in the zone's order, or `None` before any forms (P14.T51.f's star's angular radius).
    #[must_use]
    fn brightest_radius(&self, zone: &OrbitZone) -> Option<SolarRadii> {
        zone.members()
            .filter_map(|member| self.states[usize::from(member)])
            .fold(
                None,
                |brightest: Option<StarState>, state| match brightest {
                    Some(b) if b.luminosity() >= state.luminosity() => Some(b),
                    Some(_) | None => Some(state),
                },
            )
            .map(|state| state.radius())
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
                let separation = companion_separation(h, &self.under, members, star);
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

    /// The velocity at the time of the barycentre of the components `members`, relative to the
    /// system's barycentre: [`centre`](Self::centre)'s walk with each pair's relative velocity
    /// shared as its separation is, as plan 11's
    /// [`star_states_at`](crate::stellar::multiplicity::star_states_at) does.
    #[must_use]
    fn centre_velocity(&self, members: u32) -> SystemVelocity {
        let h = self.ctx.hierarchy();
        let mut node = h.root();
        let mut velocity = SystemVelocity::ZERO;
        loop {
            if self.under[usize::from(node.get())] == members {
                return velocity;
            }
            let (inner, outer, orbit) = match h.node(node) {
                HierarchyNode::Star(_) => return velocity,
                HierarchyNode::Pair {
                    inner,
                    outer,
                    orbit,
                } => (inner, outer, orbit),
            };
            let (_, relative) = orbit.relative_state_at(self.t);
            let inner_mass = h.node_mass(*inner).value();
            let outer_mass = h.node_mass(*outer).value();
            let mass = h.node_mass(node).value();
            if self.under[usize::from(inner.get())] & members == members {
                velocity = velocity + relative * -(outer_mass / mass);
                node = *inner;
            } else {
                velocity = velocity + relative * (inner_mass / mass);
                node = *outer;
            }
        }
    }

    /// The velocity at the time of a body of `zone` on `orbit`, relative to the system's
    /// barycentre: [`position`](Self::position)'s derivative at the elements then.
    #[must_use]
    fn velocity(&self, zone: &OrbitZone, orbit: &DriftingOrbit) -> SystemVelocity {
        let (_, relative) = orbit.relative_state_at(self.t);
        self.centre_velocity(members_of(zone)) + relative
    }

    /// The position at the time of a body of `zone` on `orbit`, its elements then about the zone's
    /// host.
    #[must_use]
    fn position(&self, zone: &OrbitZone, orbit: &DriftingOrbit) -> SystemPosition {
        let (offset, _) = orbit.relative_state_at(self.t);
        self.centre(members_of(zone)).translated(offset)
    }
}

#[cfg(test)]
pub(crate) mod tests;
