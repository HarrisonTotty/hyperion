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
//! [`PlanetarySystem`] holds only that primordial state, which is what the server may cache ("state
//! at the epoch, never positions at some time").
//!
//! # Queries at a time (P14.T30.b)
//!
//! Everything that depends on time is evaluated lazily from the primordial state and the context,
//! so every query is a pure function of seed, ID and time, and agrees with every other:
//!
//! - [`PlanetarySystem::body_at`]: the fate transform (P14.T28, [`BodyFate`]) gives the body's
//!   state and orbit at the time; a present body is then derived (P14.T16.a, [`derive_body`])
//!   about its hosts' states at the time, and the result is a [`BodyRecord`] (P14.T34) with its
//!   label (P14.T30.c).
//! - [`PlanetarySystem::snapshot_at`]: every body's record, in index order.
//! - [`PlanetarySystem::position_at`]: the host's position from plan 11's hierarchy, the walk of
//!   [`star_positions_at`](crate::stellar::multiplicity::star_positions_at) (bit for bit for a
//!   star, and a pair's barycentre on the same walk), plus the body's Kepler state about its host.
//! - [`PlanetarySystem::habitable_zone_at`]: Kopparapu et al.'s zone of a host's stars at the
//!   time, with the light of the system's other stars (P14.T12.b).
//!
//! # What the vertical slice leaves out
//!
//! Ruling 33's slice builds the planets and nothing else, and says so in every record rather than
//! leaving a gap to be read as "none" (ruling 34):
//!
//! - Satellites (P14.T22.a), belts and the cometary halo (P14.T21), second-generation planets
//!   (P14.T28.e) and the free-floating hosts of P14.T27 are not generated, so [`generate`] is
//!   [`generate_planets`]. Each has its own streams (design note 4) and slots (design note 3), so
//!   adding one moves no planet. A planet's `moons` and `rings`, and the snapshot's `belts` and
//!   `halo`, are [`Section::NotModelled`].
//! - A record's `surface` and `hooks` are [`Section::NotModelled`] (P14.T13, T14, T23–T26), a
//!   giant's surface [`Section::NotApplicable`].
//! - Events on bodies (P14.T31) are not built, so there is no `events_between`.
//!
//! # Streams
//!
//! [`tags::PLANET_RADIUS`], opened with the planet's [`BodyId`]: word 0 is its radius rank, and
//! words 1–7 are reserved ([`RADIUS_WORDS`]). Every other draw is its stage's.

use crate::Seed;
use crate::coords::SystemPosition;
use crate::id::{BodyId, SystemId};
use crate::orbit::KeplerElements;
use crate::planetary::architecture::{
    ArchitectureClass, ClassConstraints, ZoneLimit, class_weights, draw_class,
};
use crate::planetary::context::SystemContext;
use crate::planetary::derive::{
    BodyHosts, DerivedBody, HabitableZone, HostLight, Illumination, PlacedBody, derive_body,
    habitable_zone_of, radius_chen_kipping,
};
use crate::planetary::disc::{self, Disc, Truncation};
use crate::planetary::error::ResolveBodyError;
use crate::planetary::fate::{BodyFate, BodyState, FateBody, FateHost, ScatterDraws};
use crate::planetary::hosts::evolved::Circularisation;
use crate::planetary::hosts::young::Formation;
use crate::planetary::index::{BodyIndex, LAST_PLANET_SLOT};
use crate::planetary::label::{self, BodyLabel};
use crate::planetary::params::SPACING_GIANT_MASS;
use crate::planetary::placement::classes::orbits::{HostPlane, SystemPlane, host_plane};
use crate::planetary::placement::classes::tides::{
    GIANT_TIDAL_Q_PRIME, ROCKY_TIDAL_Q_PRIME, circularisation_time,
};
use crate::planetary::placement::zones::CLOSE_BINARY_SEMI_MAJOR_AXIS;
use crate::planetary::placement::{
    OrbitHost, OrbitZone, PlacedPlanet, PlacementHost, ZoneDiscInputs, core_fallback, place,
};
use crate::planetary::record::{BodyIdentity, BodyKind, BodyRecord, Section, SystemSnapshot};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::StarState;
use crate::stellar::draws::UnitUniform;
use crate::stellar::multiplicity::{HierarchyNode, SystemHierarchy};
use crate::time::UniverseTime;
use crate::units::{
    EarthMasses, Kilograms, KilogramsPerCubicMetre, Megayears, Metres, SolarMasses,
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

/// One body of a generated system, as it was born (design note 1): a planet as placed (P14.T8),
/// with the host it orbits and its own draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    placed: PlacedPlanet,
    host: OrbitHost,
    radius_rank: UnitUniform,
    fate: FateBody,
}

impl Body {
    /// The body `placed` of the host `zone` of `system`, in the universe of `seed`, whose disc
    /// lives `disc_lifetime`: its radius rank, its formation and its circularisation.
    #[must_use]
    fn new(
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
        let r = radius.value();
        let volume = 4.0 / 3.0 * core::f64::consts::PI * (r * r * r);
        let density = KilogramsPerCubicMetre::new(Kilograms::from(mass).value() / volume);
        let fate = FateBody::new(formation, *placed.orbit(), mass, density)
            .expect("a placed planet's mass and its density are positive")
            .with_circularisation(circularisation(&placed, zone.host_mass(), radius))
            .with_scatter_draws(ScatterDraws::Stream { seed, body: id });
        Self {
            placed,
            host: zone.host(),
            radius_rank: radius_rank(seed, id),
            fate,
        }
    }

    /// The body's index in its system.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.placed.index()
    }

    /// What the body is: a planet, every body of this generator version.
    #[must_use]
    pub const fn kind(&self) -> BodyKind {
        BodyKind::Planet
    }

    /// What the body orbits: its zone's host.
    #[must_use]
    pub const fn host(&self) -> OrbitHost {
        self.host
    }

    /// The body as placed (P14.T8): its group, role, origin and the rest.
    #[must_use]
    pub const fn placed(&self) -> &PlacedPlanet {
        &self.placed
    }

    /// The body's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.placed.mass()
    }

    /// The body's primordial orbit about its host, in the system frame, with μ = G (M + m) at the
    /// host's initial mass.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        self.placed.orbit()
    }

    /// The body's radius rank, drawn on its own `planet.radius` stream ([`radius_rank`]).
    #[must_use]
    pub const fn radius_rank(&self) -> UnitUniform {
        self.radius_rank
    }

    /// When the body forms (P14.T28.a).
    #[must_use]
    pub const fn formation(&self) -> &Formation {
        self.fate.formation()
    }

    /// How its orbit circularises (P14.T8.e).
    #[must_use]
    pub const fn circularisation(&self) -> Circularisation {
        self.fate.circularisation()
    }

    /// What the fate transform reads of the body (P14.T28): its formation, primordial orbit, mass,
    /// primordial density and circularisation.
    #[must_use]
    pub const fn fate(&self) -> &FateBody {
        &self.fate
    }
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
/// use hyperion_sim::planetary::system::generate_planets;
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
/// let system = generate_planets(seed, &sun);
/// assert_eq!(system.zones().len(), 1);
/// assert!(system.disc(OrbitHost::Star(0)).is_some());
/// for body in system.bodies() {
///     let record = system.body_at(&sun, body.index(), UniverseTime::EPOCH)?;
///     // A Sun's planets have long formed, and none has been engulfed yet.
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
}

/// The system of `ctx` in the universe of `seed`, whole: its planets, and in time their
/// satellites, belts, cometary halo and second-generation planets (P14.T30.a).
///
/// In the vertical slice (ruling 33) those stages are not built, so this is
/// [`generate_planets`]; each will join with its own streams and slots and move no planet.
#[must_use]
pub fn generate(seed: Seed, ctx: &SystemContext) -> PlanetarySystem {
    generate_planets(seed, ctx)
}

/// The planets of the system of `ctx` in the universe of `seed`, with no satellites (P14.T30.a):
/// zones, then per zone its disc, class and placement, then each planet's own draws (the
/// [module](self) documentation).
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
                    .map(|&planet| Body::new(seed, id, zone, planet, inputs.lifetime())),
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
    }
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

    /// Every body, sorted by index.
    #[must_use]
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// The body `index`, if the system holds it.
    #[must_use]
    pub fn body(&self, index: BodyIndex) -> Option<&Body> {
        self.bodies
            .binary_search_by_key(&index, Body::index)
            .ok()
            .map(|at| &self.bodies[at])
    }

    /// The bytes the system owns on the heap, beyond `size_of::<PlanetarySystem>()`: its zones,
    /// hosts and bodies, by capacity. None of them owns anything further, so the charge grows with
    /// the body count and the host count alone.
    ///
    /// It is what a server charges a cached system against its byte budget (plan 14, P14.T36.a),
    /// beside [`SystemContext::heap_bytes`]; nothing generated reads it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.zones.capacity() * size_of::<OrbitZone>()
            + self.hosts.capacity() * size_of::<PlanetaryHost>()
            + self.bodies.capacity() * size_of::<Body>()
    }

    /// The bodies whose parent is body `index`: a planet's moons and rings, a belt's members. None
    /// in this generator version, which generates planets only.
    pub fn children(&self, index: BodyIndex) -> impl Iterator<Item = &Body> {
        self.bodies
            .iter()
            .filter(move |body| body.index().parent() == Some(index))
    }

    /// Every body's record at `t`, at [`DetailLevel::Full`](crate::planetary::record::DetailLevel::Full),
    /// in index order, with the system's belts and halo [`Section::NotModelled`] (P14.T30.b).
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
        let hosts: Vec<FateHost<'_>> = self.zones.iter().map(|zone| fate_host(ctx, zone)).collect();
        // Each host's bodies are resolved together, for the scattering after a supernova
        // (ruling 71), and each exactly once.
        let mut fates: Vec<Option<BodyFate<'_>>> = vec![None; self.bodies.len()];
        for (zone, host) in self.zones.iter().zip(&hosts) {
            let members: Vec<usize> = (0..self.bodies.len())
                .filter(|&k| self.bodies[k].host == zone.host())
                .collect();
            let bodies: Vec<&FateBody> = members.iter().map(|&k| &self.bodies[k].fate).collect();
            for (k, fate) in members
                .into_iter()
                .zip(BodyFate::resolve_all(&bodies, host))
            {
                fates[k] = Some(fate);
            }
        }
        let records = self
            .bodies
            .iter()
            .zip(label::labels(self))
            .zip(&fates)
            .map(|((body, (_, label)), fate)| {
                let fate = fate
                    .as_ref()
                    .expect("every body orbits one of its system's zones");
                self.record(&epoch, body, label, fate)
            })
            .collect();
        SystemSnapshot::new(self.system, t, records)
            .expect("a system's records are its own, in index order, at full detail")
    }

    /// The record of body `index` at `t`, at [`DetailLevel::Full`](crate::planetary::record::DetailLevel::Full)
    /// (P14.T30.b).
    ///
    /// - The identity: the kind, the label (P14.T30.c), the host the body orbits as its parent,
    ///   and its state at `t` from the fate transform (P14.T28).
    /// - The mass, always: a body not yet formed, destroyed or unbound has the mass it had, and a
    ///   present one carries any neighbour that collided with it and merged into it (ruling 71).
    /// - For a body present at `t`: its orbit at `t` with the time it holds until, its position,
    ///   and its bulk from [`derive_body`] about its hosts at `t`, with the surface
    ///   [`Section::NotModelled`] ([`Section::NotApplicable`] for a giant).
    /// - For a body not present: no position, and its orbit, bulk and surface
    ///   [`Section::NotApplicable`], since nothing orbits there at `t`.
    /// - Its moons, rings and hooks [`Section::NotModelled`] (ruling 34).
    ///
    /// # Errors
    ///
    /// [`ResolveBodyError::NoSuchBody`] if the system holds no body `index`: an unused slot, a
    /// moon or ring of this generator version, or a star, whose records are plan 06's.
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
        let body = self.body(index).ok_or(ResolveBodyError::NoSuchBody)?;
        let label = label::label(self, index).expect("every body of a system has a label");
        let host = fate_host(ctx, self.zone_of(body));
        let fate = self.fate_of(body, &host);
        Ok(self.record(&Epoch::new(self, ctx, t), body, label, &fate))
    }

    /// Where body `index` is at `t`, in the system frame from its barycentre (plan 01's `coords`):
    /// its host's position, a star's or a pair's barycentre on plan 11's hierarchy, plus the
    /// body's Kepler state about it; `None` for a body not present at `t` (P14.T30.b).
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
        let body = self.body(index).ok_or(ResolveBodyError::NoSuchBody)?;
        let epoch = Epoch::new(self, ctx, t);
        let zone = self.zone_of(body);
        let host = fate_host(ctx, zone);
        let fate = self.fate_of(body, &host).at(t);
        Ok(fate.orbit().map(|orbit| epoch.position(zone, orbit)))
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

    /// The zone of `body`'s host.
    #[must_use]
    fn zone_of(&self, body: &Body) -> &OrbitZone {
        self.zone(body.host)
            .expect("every body orbits one of its system's zones")
    }

    /// The history of `body` on `host`, its zone's stars, among the other bodies of its host
    /// (P14.T28, [`BodyFate::resolve_among`]): they meet it only in the scattering after a
    /// supernova (ruling 71).
    #[must_use]
    fn fate_of<'s>(&'s self, body: &Body, host: &'s FateHost<'s>) -> BodyFate<'s> {
        let siblings: Vec<&Body> = self.bodies.iter().filter(|b| b.host == body.host).collect();
        let index = siblings
            .iter()
            .position(|b| b.index() == body.index())
            .expect("a body is among its host's bodies");
        let bodies: Vec<&FateBody> = siblings.iter().map(|b| &b.fate).collect();
        BodyFate::resolve_among(&bodies, index, host)
    }

    /// The record of `body` at the epoch's time, labelled `label`, with its history `fate`.
    #[must_use]
    fn record(
        &self,
        epoch: &Epoch<'_>,
        body: &Body,
        label: BodyLabel,
        fate: &BodyFate<'_>,
    ) -> BodyRecord {
        let zone = self.zone_of(body);
        let at = fate.at(epoch.t);
        let identity = BodyIdentity::new(
            self.system,
            body.index(),
            body.kind(),
            Some(body.host),
            at.state(),
        )
        .with_label(label);
        let builder = BodyRecord::builder(identity).mass(Section::Ok(at.mass()));
        let builder = match at.state() {
            BodyState::Present => {
                let orbit = at.orbit().expect("a present body has an orbit");
                let section = at
                    .body_orbit()
                    .expect("a present body has an orbit section");
                let derived =
                    self.derive(epoch, body, zone, orbit, at.mass(), fate.host_mass(epoch.t));
                builder
                    .derived(&derived)
                    .orbit(Section::Ok(section))
                    .position(epoch.position(zone, orbit))
            }
            BodyState::NotYetFormed | BodyState::Destroyed { .. } | BodyState::Unbound { .. } => {
                builder
                    .orbit(Section::NotApplicable)
                    .bulk(Section::NotApplicable)
                    .surface(Section::NotApplicable)
            }
        };
        builder
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }

    /// Everything [`derive_body`] computes of the present `body` of `zone` at the epoch's time,
    /// on its orbit `orbit` then and of its mass `mass` then (its own, or with the neighbours
    /// merged into it, ruling 71), about hosts of total mass `host_mass`.
    #[must_use]
    fn derive(
        &self,
        epoch: &Epoch<'_>,
        body: &Body,
        zone: &OrbitZone,
        orbit: &KeplerElements,
        mass: EarthMasses,
        host_mass: SolarMasses,
    ) -> DerivedBody {
        let disc = self
            .host(body.host)
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
        let placed = PlacedBody::new(
            mass,
            *body.orbit(),
            body.placed.formation_distance(),
            body.radius_rank,
        )
        .expect("a placed planet's mass and formation distance are positive")
        .with_orbit_now(*orbit);
        derive_body(&placed, &hosts, disc, epoch.ctx.age_at_epoch(), epoch.t).expect(
            "a present body's system is born, and a placed planet lies below 13 Jupiter masses",
        )
    }
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
        Self {
            ctx,
            t,
            under: members_under(ctx.hierarchy()),
            states: ctx.stars().iter().map(|star| star.state_at(t)).collect(),
        }
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
