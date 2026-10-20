//! A planet's satellites, assembled: its moons of every origin and its rings, numbered by design
//! note 3 (plan 14, P14.T22.a).
//!
//! [`generate_satellites`] runs phase D's pieces for one planet in a fixed order, each on the
//! planet's own streams (design note 4), so that one planet's satellites can be generated alone
//! and equal the same planet's inside [`generate`](crate::planetary::generate):
//!
//! 1. **Regular moons** (P14.T17, [`regular_moons`]).
//! 2. **The giant-impact moon** (P14.T18, [`giant_impact_moon`]).
//! 3. **Captures and their removals** (P14.T19, [`captures`]): a Triton-like capture that fits
//!    inside its planet's retrograde stability limit and moon-mass limit (P14.T15) removes the
//!    regular moons beyond it ([`Captures::prune`](crate::planetary::moons::Captures::prune)); one
//!    that does not is not made. Then, by ruling 83.8, a capture that is not
//!    Triton-like is kept only where its pericentre lies beyond every other moon's apocentre:
//!    the regular moons kept, the giant-impact moon's orbit at the end of the clock window (its
//!    widest there, since it only recedes), a Triton-like capture's, and the planet's own radius
//!    (P14.T22.b; P14.T19 bounds a capture's pericentre by the fluid Roche limit alone, which
//!    about a bloated giant lies inside the planet). Irregulars may cross one
//!    another, as the Himalia group's orbits do; nothing else crosses anything.
//!    The regular moons are then kept as their longest inner run that keeps design note 7's gap,
//!    which P14.T17.a's eccentricity halving leaves broken in about one planet in a million. On
//!    the Laplace surface (P14.T56.a), they are then cut at the first that lies in the
//!    eccentricity band of their parent's Laplace radius ([`laplace`]), dropped with every
//!    regular moon beyond it, before the captures are cleared of them.
//! 4. **Rings** (P14.T20, [`generate_rings`]), with the regular moons kept, whose resonances cut
//!    their gaps; on the Laplace surface none that reaches into the band.
//!
//! # What the parent is
//!
//! The moon generators read the parent as P14.T16.a derives it at the epoch
//! ([`MoonParent::from_derived`]), since moons are primordial like planets (design note 1): its
//! mass, radius and class, its primordial orbit about its host's initial mass, and the heaviest
//! moon tides let survive. A system not yet born at the epoch is read at the end of the clock
//! window, by which every system of a [`SystemContext`] is born; one whose age is still not
//! positive there has no satellites.
//!
//! # Sub-indices
//!
//! A planet's moons take the sub-indices `0x01` upward in the order above (regular moons inside
//! out, the giant-impact moon, then the captures kept in their ordinals), densely, and its rings
//! `0x80` upward, the dusty ring first ([`generate_rings`]). A moon's own draws, its radius rank
//! and its volatile ranks (P14.T56.c), are keyed by its candidate's identity rather than its
//! sub-index (ruling 112.5), so that pruning one moon moves no other's draw: `candidate_key` gives
//! the layout. A belt's member (P14.T21.c) numbers its one possible moon, a giant-impact moon,
//! `0x80` above its own sub-index in its belt's slot: member k's moon is `Member(0x80 + k)`
//! (design note 3's row, ruling 95.1), whose [`BodyIndex::parent`] is member k.
//!
//! # Frames
//!
//! The generators give each moon's elements about its parent against a reference plane they do
//! not orient: the equator for regular and giant-impact moons, the orbital plane for captures.
//! [`Satellite::orbit_at`] turns them into the parent's body frame, a translation of the system
//! frame with the galactic axes (plan 01's `coords`), where a moon's position is its planet's
//! plus its own offset, by referring them to the moon's [`MoonPlane`]:
//!
//! - **On the Laplace surface** (P14.T56.a): every moon, of every origin,
//!   to its parent's local Laplace plane at its semi-major axis ([`laplace`]), the plane its node
//!   precesses about, so that its drawn inclination is its free inclination. The plane follows
//!   from the parent's [`LaplaceFrame`], read once at the parent's time, and the moon's own
//!   Laplace radius, which counts every moon inside it at that time. Its pole is on the parent's
//!   spin's side for regular and giant-impact moons, which form prograde about the spin, and on
//!   the orbit normal's side for captures, whose sense is drawn about the orbit
//!   ([`PoleSense`]). A giant-impact moon's plane follows its receding axis.
//! - **In its parent's orbital plane**, about a parent with no Laplace frame: one with no rotation
//!   law, such as a planet of [`generate_planets`](crate::planetary::generate_planets) alone, and
//!   in the generator's first pass, which finds every parent's satellites before its law, and so
//!   its frame, is derived (`planetary::system`'s step 7: it fixes whether the parent had a giant
//!   impact, which its law reads).

use crate::Seed;
use crate::orbit::KeplerElements;
use crate::planetary::context::SystemContext;
use crate::planetary::derive::atmosphere::VolatileDraws;
use crate::planetary::index::{BodyIndex, BodySlot, BodySub, LAST_MOON_SUB};
use crate::planetary::laplace::{self, InnerMoon, LaplaceFrame, LocalLaplace, PoleSense};
use crate::planetary::moons::regular::{MoonVolatiles, VolatileSite};
use crate::planetary::moons::{
    CaptureKind, CapturedMoon, ImpactMoon, IrregularPopulation, MoonParent, NearestBelt,
    RegularMoon, captures, giant_impact_moon, regular_moons,
};
use crate::planetary::params::HILL_STABLE_GAP;
use crate::planetary::placement::classes::orbits::{SystemPlane, orientation_in_plane};
use crate::planetary::placement::mutual_hill_radius;
use crate::planetary::record::MoonOrigin;
use crate::planetary::rings::{Ring, RingMoon, RingParent, RingPlane, generate_rings};
use crate::planetary::system::{Body, satellite_parent};
use crate::stellar::draws::UnitUniform;
use crate::time::CLOCK_WINDOW_H;
use crate::units::{EarthMasses, Kilograms, Metres, SolarMasses, Years};

/// The sub-index offset of a belt member's moon above the member's own (see the
/// [module](self) documentation).
pub const MEMBER_MOON_OFFSET: u8 = 0x80;

/// One moon of a planet, of any origin, as generated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SatelliteMoon {
    /// A regular satellite (P14.T17).
    Regular(RegularMoon),
    /// A giant-impact moon (P14.T18).
    GiantImpact(ImpactMoon),
    /// A captured moon (P14.T19).
    Captured(CapturedMoon),
}

/// The plane a moon's local elements are referred to in its parent's body frame (see the
/// [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoonPlane {
    /// The parent's orbital plane, its [`MoonParent::orbit`]'s, about a parent with no Laplace
    /// frame.
    OrbitalPlane,
    /// The parent's local Laplace plane at the moon's semi-major axis (P14.T56.a).
    Laplace(LocalLaplace),
}

/// The planes [`satellites_of`] refers a parent's satellites to (P14.T56.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SatelliteFrame<'f> {
    /// The parent's orbital plane, for a parent with no Laplace frame.
    OrbitalPlane,
    /// The parent's Laplace surface, of this frame.
    Laplace(&'f LaplaceFrame),
}

impl<'f> SatelliteFrame<'f> {
    /// The Laplace surface of `frame` where a parent holds one, and its orbital plane where it
    /// does not.
    #[must_use]
    pub(crate) const fn of(frame: Option<&'f LaplaceFrame>) -> Self {
        match frame {
            Some(frame) => Self::Laplace(frame),
            None => Self::OrbitalPlane,
        }
    }
}

/// One moon of a planet with its index, its own radius rank and volatile ranks, and its plane
/// (P14.T22.a, P14.T56.a, P14.T56.c).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Satellite {
    index: BodyIndex,
    moon: SatelliteMoon,
    radius_rank: UnitUniform,
    volatiles: VolatileDraws,
    plane: MoonPlane,
}

impl Satellite {
    /// The moon's body index.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// The moon as its generator made it.
    #[must_use]
    pub const fn moon(&self) -> &SatelliteMoon {
        &self.moon
    }

    /// How it came to orbit its parent.
    #[must_use]
    pub const fn origin(&self) -> MoonOrigin {
        match self.moon {
            SatelliteMoon::Regular(_) => MoonOrigin::Regular,
            SatelliteMoon::GiantImpact(_) => MoonOrigin::GiantImpact,
            SatelliteMoon::Captured(_) => MoonOrigin::Captured,
        }
    }

    /// Its mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        match &self.moon {
            SatelliteMoon::Regular(moon) => moon.mass(),
            SatelliteMoon::GiantImpact(moon) => moon.mass(),
            SatelliteMoon::Captured(moon) => moon.mass(),
        }
    }

    /// Its radius rank, word 0 of its own `planet.radius` stream (design note 8), which a regular
    /// moon's derivation reads (P14.T17.b): keyed by its candidate's identity, not its sub-index
    /// (ruling 112.5; see the [module](self) documentation).
    #[must_use]
    pub const fn radius_rank(&self) -> UnitUniform {
        self.radius_rank
    }

    /// Its volatile ranks, words 0–5 of its own `planet.volatiles` stream
    /// ([`VolatileDraws::for_body`]), keyed by its candidate's identity as its
    /// [`radius_rank`](Self::radius_rank) is (P14.T56.c, ruling 112.5): each moon's own, as each
    /// planet's is, so that the moons of one planet do not move together.
    #[must_use]
    pub const fn volatiles(&self) -> &VolatileDraws {
        &self.volatiles
    }

    /// Where its volatiles formed (P14.T56.c): a regular moon's in its planet's nursery, a captured
    /// or giant-impact moon's in its parent's zone of the star's disc.
    #[must_use]
    pub const fn volatile_site(&self) -> VolatileSite {
        match self.moon {
            SatelliteMoon::Regular(_) => VolatileSite::Nursery,
            SatelliteMoon::GiantImpact(_) | SatelliteMoon::Captured(_) => VolatileSite::ParentZone,
        }
    }

    /// The volatiles it is derived with: its own [`volatiles`](Self::volatiles) at its
    /// [`volatile_site`](Self::volatile_site) (P14.T56.c).
    #[must_use]
    pub const fn own_volatiles(&self) -> MoonVolatiles {
        MoonVolatiles {
            draws: self.volatiles,
            site: self.volatile_site(),
        }
    }

    /// The plane its elements are referred to (see the [module](self) documentation).
    #[must_use]
    pub const fn plane(&self) -> &MoonPlane {
        &self.plane
    }

    /// Its elements about its parent `parent` when the parent is `age` old, as generated:
    /// referred to its unoriented reference plane (see the [module](self) documentation). Only a
    /// giant-impact moon's change with age, as it recedes (P14.T18).
    #[must_use]
    pub fn local_orbit_at(&self, parent: &MoonParent, age: Years) -> KeplerElements {
        match &self.moon {
            SatelliteMoon::Regular(moon) => *moon.orbit(),
            SatelliteMoon::GiantImpact(moon) => moon.orbit_at(parent, age),
            SatelliteMoon::Captured(moon) => *moon.orbit(),
        }
    }

    /// Its elements about `parent` when the parent is `age` old, in the parent's body frame,
    /// with the galactic axes: [`local_orbit_at`](Self::local_orbit_at) referred to its
    /// [`plane`](Self::plane) at that axis (see the [module](self) documentation).
    #[must_use]
    pub fn orbit_at(&self, parent: &MoonParent, age: Years) -> KeplerElements {
        in_body_frame(parent, &self.plane, &self.local_orbit_at(parent, age))
    }
}

/// `local`, an orbit about `parent` referred to `plane` at its semi-major axis, in the parent's
/// body frame.
///
/// # Panics
///
/// Never: the orbit is rebuilt from a valid one's own values, about a plane of a finite pole.
#[must_use]
fn in_body_frame(parent: &MoonParent, plane: &MoonPlane, local: &KeplerElements) -> KeplerElements {
    let plane = match plane {
        MoonPlane::OrbitalPlane => SystemPlane::of_orbit(parent.orbit().orientation()),
        MoonPlane::Laplace(laplace) => {
            SystemPlane::from_pole(laplace.pole_at(local.semi_major_axis()))
                .expect("a Laplace plane's pole is a finite direction")
        }
    };
    let orientation = orientation_in_plane(
        &plane,
        local.inclination(),
        local.ascending_node(),
        local.argument_of_periapsis(),
    )
    .expect("a valid orbit's angles make a valid orientation");
    KeplerElements::from_semi_major_axis(
        local.semi_major_axis(),
        local.gravitational_parameter(),
        local.eccentricity(),
        orientation,
        local.mean_anomaly_at_epoch(),
    )
    .expect("a valid orbit's axis and parameter are positive")
}

/// A planet's satellites (P14.T22.a): its moons, its rings, and what stays a count.
#[derive(Debug, Clone, PartialEq)]
pub struct Satellites {
    parent_index: BodyIndex,
    parent: Option<MoonParent>,
    moons: Vec<Satellite>,
    rings: Vec<Ring>,
    moonlets: u32,
    irregulars: Option<IrregularPopulation>,
}

impl Satellites {
    /// No satellites about the body `parent_index`.
    #[must_use]
    pub const fn none(parent_index: BodyIndex) -> Self {
        Self {
            parent_index,
            parent: None,
            moons: Vec::new(),
            rings: Vec::new(),
            moonlets: 0,
            irregulars: None,
        }
    }

    /// The index of the body they orbit.
    #[must_use]
    pub const fn parent_index(&self) -> BodyIndex {
        self.parent_index
    }

    /// The parent as the moon generators read it, if it could be derived.
    #[must_use]
    pub const fn parent(&self) -> Option<&MoonParent> {
        self.parent.as_ref()
    }

    /// The moons, in index order.
    #[must_use]
    pub fn moons(&self) -> &[Satellite] {
        &self.moons
    }

    /// The rings, in index order.
    #[must_use]
    pub fn rings(&self) -> &[Ring] {
        &self.rings
    }

    /// The small inner moonlets of a giant's regular system, a count only (P14.T17.a).
    #[must_use]
    pub const fn moonlets(&self) -> u32 {
        self.moonlets
    }

    /// A giant's irregular population above 2.8 km, a statistical record (P14.T19).
    #[must_use]
    pub const fn irregulars(&self) -> Option<&IrregularPopulation> {
        self.irregulars.as_ref()
    }

    /// Whether there is nothing at all: no moon, no ring.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.moons.is_empty() && self.rings.is_empty()
    }

    /// The bytes the satellites own on the heap (P14.T36.a's byte bound).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.moons.capacity() * size_of::<Satellite>()
            + self.rings.capacity() * size_of::<Ring>()
            + self.rings.iter().map(Ring::heap_bytes).sum::<usize>()
    }
}

/// The satellites of the planet `planet` of the system of `ctx`, in the universe of `seed`, next
/// to the belt `belt` if it has one (P14.T22.a; see the [module](self) documentation).
///
/// It reads nothing but the planet, its host (the context's stars and the zone's disc), the
/// nearest belt's mass passed in, which
/// [`PlanetarySystem::nearest_belt`](crate::planetary::PlanetarySystem::nearest_belt) finds, and
/// the planet's [`Body::laplace`] frame, which [`generate`](crate::planetary::generate) holds on
/// the Laplace surface (P14.T56.a): with none, its satellites keep its orbital plane. A planet
/// with no moons and no rings has an empty set, and a body that is not a planet or a belt member
/// none.
///
/// # Panics
///
/// If `planet` is not a body of the system of `ctx`, which only a caller's error makes.
#[must_use]
pub fn generate_satellites(
    seed: Seed,
    ctx: &SystemContext,
    planet: &Body,
    belt: Option<NearestBelt>,
) -> Satellites {
    match satellite_parent(seed, ctx, planet) {
        Some((parent, rings)) => satellites_of(
            seed,
            planet.index(),
            &parent,
            rings.as_ref(),
            belt,
            ctx.age_at_epoch(),
            SatelliteFrame::of(planet.laplace()),
        ),
        None => Satellites::none(planet.index()),
    }
}

/// The satellites of `parent`, the body `parent_index`, whose rings (for a giant) read
/// `ring_parent`, next to `belt`, in a system aged `age_at_epoch` at the epoch, in the planes
/// `frame` gives: the four steps of the [module](self) documentation.
#[must_use]
pub(crate) fn satellites_of(
    seed: Seed,
    parent_index: BodyIndex,
    parent: &MoonParent,
    ring_parent: Option<&RingParent>,
    belt: Option<NearestBelt>,
    age_at_epoch: Years,
    frame: SatelliteFrame<'_>,
) -> Satellites {
    let frame = match frame {
        SatelliteFrame::Laplace(frame) => Some(frame),
        SatelliteFrame::OrbitalPlane => None,
    };
    let regular = regular_moons(seed, parent);
    let impact = giant_impact_moon(seed, parent);
    let caught = captures(seed, parent, belt);
    // A Triton-like capture is kept only inside its planet's limits (P14.T15, P14.T22.b): its
    // apocentre inside the retrograde stability limit and its mass inside the heaviest moon tides
    // let survive. P14.T19 places it at 10–20 planetary radii whatever the Hill sphere, which
    // about a close-in ice giant lies outside it; such a capture is not made, and prunes nothing.
    let large_fits = caught.large().is_none_or(|large| {
        let e = large.orbit().eccentricity().value();
        large.orbit().apoapsis() < parent.stability_limit(e, large.sense())
            && large.mass() <= parent.maximum_moon_mass()
    });
    let regular = if large_fits {
        caught.prune(parent, &regular)
    } else {
        regular
    };
    // T22.b's non-crossing among the regular moons, kept as their longest inner run: P14.T17.a
    // halves an outer moon's eccentricity to clear the gap and then sets it to zero, and places
    // the moon even where its inner neighbour's eccentricity alone still breaks the gap (about
    // one planet in a million), so such a moon and those beyond it are dropped from the outside
    // in, as T17.a drops what does not fit.
    let clear_run = clear_run(parent, regular.moons());
    let regular = regular.retaining(|moon| moon.ordinal() <= clear_run);
    // P14.T56.a: on the Laplace surface, the regular moons stop short of the eccentricity band,
    // counting inside each the moons found so far (decision-p14-moons §1.2).
    let regular = match frame {
        Some(frame) => {
            let large = caught.large().filter(|_| large_fits);
            let others: Vec<SatelliteMoon> = impact
                .map(SatelliteMoon::GiantImpact)
                .into_iter()
                .chain(large.map(|&moon| SatelliteMoon::Captured(moon)))
                .collect();
            let stable = stable_run(frame, parent, regular.moons(), &others);
            regular.retaining(|moon| moon.ordinal() <= stable)
        }
        None => regular,
    };

    // Ruling 83.8: captures other than a Triton-like one clear every other moon.
    let end_age = Years::new(age_at_epoch.value() + CLOCK_WINDOW_H.as_julian_years_f64());
    let clear_of = regular
        .moons()
        .iter()
        .map(|moon| moon.orbit().apoapsis())
        .chain(impact.map(|moon| moon.orbit_at(parent, end_age).apoapsis()))
        .chain(
            caught
                .large()
                .filter(|_| large_fits)
                .map(|moon| moon.orbit().apoapsis()),
        )
        // Nor may a capture's pericentre fall inside its planet: P14.T19 holds it outside the
        // fluid Roche limit, which about a giant much less dense than the capture lies inside
        // the planet's own radius.
        .fold(parent.radius(), |far, a| if a > far { a } else { far });
    let kept_captures = caught.moons().iter().filter(|moon| match moon.kind() {
        CaptureKind::Large => large_fits,
        CaptureKind::Irregular | CaptureKind::Small => moon.orbit().periapsis() > clear_of,
    });

    let generated = regular
        .moons()
        .iter()
        .map(|&moon| SatelliteMoon::Regular(moon))
        .chain(impact.map(SatelliteMoon::GiantImpact))
        .chain(kept_captures.map(|&moon| SatelliteMoon::Captured(moon)));
    let system = parent.id().system();
    let moons = numbered(seed, parent_index, parent, frame, generated);

    let ring_moons: Vec<RingMoon> = regular
        .moons()
        .iter()
        .map(|moon| RingMoon::new(moon.orbit().semi_major_axis(), Kilograms::from(moon.mass())))
        .collect();
    let plane = frame.map_or_else(
        || RingPlane::Fixed(parent.orbit().orientation().normal()),
        |frame| RingPlane::Laplace(*frame),
    );
    let rings = ring_parent.map_or_else(Vec::new, |ring_parent| {
        generate_rings(seed, system, ring_parent, &ring_moons, &plane)
    });
    Satellites {
        parent_index,
        parent: Some(*parent),
        moons,
        rings,
        moonlets: regular.moonlets(),
        irregulars: caught.population().copied(),
    }
}

/// The moons `generated` of `parent`, the body `parent_index`, numbered densely in their order up
/// to the layout's last sub-index, each with its own radius rank and, on the Laplace surface of
/// `frame` if one is given, its local plane (see the [module](self) documentation).
#[must_use]
fn numbered(
    seed: Seed,
    parent_index: BodyIndex,
    parent: &MoonParent,
    frame: Option<&LaplaceFrame>,
    generated: impl Iterator<Item = SatelliteMoon>,
) -> Vec<Satellite> {
    let system = parent.id().system();
    let made: Vec<(BodyIndex, SatelliteMoon)> = (1_u8..)
        .zip(generated)
        .map_while(|(n, moon)| moon_index(parent_index, n).map(|index| (index, moon)))
        .collect();
    made.iter()
        .map(|&(index, moon)| {
            // Ruling 112.5: the moon's own draws are keyed by what it is, not by where the moons
            // kept before it leave it in the dense numbering, so that pruning one moon moves no
            // other.
            let key = candidate_key(parent_index, &moon)
                .unwrap_or(index)
                .body_id(system);
            let plane = frame.map_or(MoonPlane::OrbitalPlane, |frame| {
                MoonPlane::Laplace(local_plane(
                    frame,
                    parent,
                    &moon,
                    made.iter().map(|(_, other)| other),
                ))
            });
            Satellite {
                index,
                moon,
                radius_rank: crate::planetary::system::radius_rank(seed, key),
                volatiles: VolatileDraws::for_body(seed, key),
                plane,
            }
        })
        .collect()
}

/// The semi-major axis of `moon` about `parent` when its system is `age` old, as its plane and the
/// Laplace radii of the moons beyond it read it.
#[must_use]
fn axis_at(moon: &SatelliteMoon, parent: &MoonParent, age: Years) -> Metres {
    match moon {
        SatelliteMoon::Regular(moon) => moon.orbit().semi_major_axis(),
        SatelliteMoon::GiantImpact(moon) => moon.orbit_at(parent, age).semi_major_axis(),
        SatelliteMoon::Captured(moon) => moon.orbit().semi_major_axis(),
    }
}

/// The local Laplace plane of `moon` about `parent` in `frame` (see the [module](self)
/// documentation): its Laplace radius counts every moon of `all` inside it at the frame's time,
/// in their order, and its pole is on the spin's side for a regular or giant-impact moon and the
/// orbit normal's for a capture.
#[must_use]
fn local_plane<'m>(
    frame: &LaplaceFrame,
    parent: &MoonParent,
    moon: &SatelliteMoon,
    all: impl IntoIterator<Item = &'m SatelliteMoon>,
) -> LocalLaplace {
    let age = frame.read_at();
    let a = axis_at(moon, parent, age);
    let sense = match moon {
        SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => PoleSense::Spin,
        SatelliteMoon::Captured(_) => PoleSense::Orbit,
    };
    frame.local(laplace_radius_inside(frame, parent, a, all), sense)
}

/// The Laplace radius at `a` about `parent` in `frame`, counting each of `moons` whose axis at
/// the frame's time lies inside `a`, in their order.
#[must_use]
fn laplace_radius_inside<'m>(
    frame: &LaplaceFrame,
    parent: &MoonParent,
    a: Metres,
    moons: impl IntoIterator<Item = &'m SatelliteMoon>,
) -> Metres {
    let age = frame.read_at();
    let parent_mass = frame.parent_mass().value();
    let inside = moons.into_iter().filter_map(|other| {
        let axis = axis_at(other, parent, age);
        (axis < a).then(|| InnerMoon {
            mass_ratio: Kilograms::from(satellite_mass(other)).value() / parent_mass,
            semi_major_axis: axis,
        })
    });
    frame.laplace_radius(inside)
}

/// The mass of `moon`.
#[must_use]
const fn satellite_mass(moon: &SatelliteMoon) -> EarthMasses {
    match moon {
        SatelliteMoon::Regular(moon) => moon.mass(),
        SatelliteMoon::GiantImpact(moon) => moon.mass(),
        SatelliteMoon::Captured(moon) => moon.mass(),
    }
}

/// The ordinal of the last of the regular moons `regular` of `parent` (inside out) before the
/// first that lies in the eccentricity band of `frame`'s Laplace surface, each counting inside it
/// the regular moons before it and those of `others` inside it (decision-p14-moons §1.2: "a
/// regular moon inside the band is dropped, with every regular moon beyond it"); every moon's when
/// none does, and 0 when the first does.
#[must_use]
fn stable_run(
    frame: &LaplaceFrame,
    parent: &MoonParent,
    regular: &[RegularMoon],
    others: &[SatelliteMoon],
) -> u8 {
    // Every moon found so far; each one's radius counts those inside it, which for a regular moon
    // are the regular moons before it (they run inside out) and any other inside it.
    let found: Vec<SatelliteMoon> = regular
        .iter()
        .map(|&moon| SatelliteMoon::Regular(moon))
        .chain(others.iter().copied())
        .collect();
    let mut last = 0;
    for moon in regular {
        let a = moon.orbit().semi_major_axis();
        let rl = laplace_radius_inside(frame, parent, a, &found);
        if laplace::eccentricity_unstable(frame.obliquity(), a.value() / rl.value()) {
            break;
        }
        last = moon.ordinal();
    }
    last
}

/// The ordinal of the last moon of the longest inner run of `moons` (inside out) whose adjacent
/// pairs keep design note 7's gap of 2√3 mutual Hill radii about `parent` between the inner
/// apocentre and the outer pericentre; 0 for none.
#[must_use]
fn clear_run(parent: &MoonParent, moons: &[RegularMoon]) -> u8 {
    let host = SolarMasses::from(parent.mass());
    let mut last = moons.first().map_or(0, RegularMoon::ordinal);
    for pair in moons.windows(2) {
        let (inner, outer) = (&pair[0], &pair[1]);
        let hill = mutual_hill_radius(
            inner.mass(),
            outer.mass(),
            host,
            inner.orbit().semi_major_axis(),
            outer.orbit().semi_major_axis(),
        );
        // Written so that a gap or a Hill radius that is not a number fails the floor.
        let gap = outer.orbit().periapsis() - inner.orbit().apoapsis();
        let floor = hill * HILL_STABLE_GAP;
        if !matches!(
            gap.partial_cmp(&floor),
            Some(core::cmp::Ordering::Greater | core::cmp::Ordering::Equal)
        ) {
            break;
        }
        last = outer.ordinal();
    }
    last
}

/// The sub-index a giant-impact moon's own draws are keyed by (see [`candidate_key`]).
const IMPACT_CANDIDATE_SUB: u8 = 0x10;

/// The sub-index a Triton-like capture's own draws are keyed by, and above which a giant's
/// irregular k is keyed at `0x20 + k` (see [`candidate_key`]).
const LARGE_CAPTURE_CANDIDATE_SUB: u8 = 0x20;

/// The sub-index above which a rocky planet's small capture k is keyed at `0x30 + k` (see
/// [`candidate_key`]).
const SMALL_CAPTURE_CANDIDATE_SUB: u8 = 0x30;

/// The index whose [`BodyId`](crate::id::BodyId) keys the own draws of `moon`, a moon of the body
/// `parent`: its candidate's identity, its kind and its number within the kind, as a sub-index of
/// the parent's slot (ruling 112.5), whichever sub-index the dense numbering gives it.
///
/// A regular moon k is keyed at sub-index k (1–6), the giant-impact moon at `0x10`, a Triton-like
/// capture at `0x20`, a giant's irregular k at `0x20 + k` (k of 1–4) and a rocky planet's small
/// capture k at `0x30 + k`. So a regular moon keeps the key it had while nothing before it is
/// pruned, and no two moons of a planet share a key. A belt member's one possible moon is its
/// only candidate and keeps its own index, as does anything past the layout (`None`).
#[must_use]
fn candidate_key(parent: BodyIndex, moon: &SatelliteMoon) -> Option<BodyIndex> {
    if parent.sub() != BodySub::Primary {
        return None;
    }
    let sub = match moon {
        SatelliteMoon::Regular(moon) => moon.ordinal(),
        SatelliteMoon::GiantImpact(_) => IMPACT_CANDIDATE_SUB,
        SatelliteMoon::Captured(moon) => match moon.kind() {
            CaptureKind::Large => LARGE_CAPTURE_CANDIDATE_SUB,
            CaptureKind::Irregular => LARGE_CAPTURE_CANDIDATE_SUB.checked_add(moon.ordinal())?,
            CaptureKind::Small => SMALL_CAPTURE_CANDIDATE_SUB.checked_add(moon.ordinal())?,
        },
    };
    BodyIndex::new(parent.slot(), BodySub::Moon(sub)).ok()
}

/// The index of the `n`-th moon (from 1) of the body `parent`: sub-index n of a planet's slot, or
/// a belt member's `0x80` above its own; `None` past the layout.
#[must_use]
fn moon_index(parent: BodyIndex, n: u8) -> Option<BodyIndex> {
    match parent.sub() {
        BodySub::Primary if n <= LAST_MOON_SUB => {
            BodyIndex::new(parent.slot(), BodySub::Moon(n)).ok()
        }
        BodySub::Member(k) if n == 1 => match parent.slot() {
            BodySlot::Belt(_) => k
                .checked_add(MEMBER_MOON_OFFSET)
                .and_then(|sub| BodyIndex::new(parent.slot(), BodySub::Member(sub)).ok()),
            BodySlot::Stellar | BodySlot::Planet(_) | BodySlot::SecondGeneration(_) => None,
        },
        BodySub::Primary
        | BodySub::Member(_)
        | BodySub::Component(_)
        | BodySub::Moon(_)
        | BodySub::Ring(_) => None,
    }
}

#[cfg(test)]
mod tests;
