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
//!    which P14.T17.a's eccentricity halving leaves broken in about one planet in a million.
//! 4. **Rings** (P14.T20, [`generate_rings`]), with the regular moons kept, whose resonances cut
//!    their gaps.
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
//! out, the giant-impact moon, then the captures kept in their ordinals), and its rings `0x80`
//! upward, the dusty ring first ([`generate_rings`]). A belt's member (P14.T21.c), which design
//! note 3 gives no moon block, numbers its one possible moon, a giant-impact moon, `0x80` above
//! its own sub-index in its belt's slot: member k's moon is `Member(0x80 + k)` (this lane's
//! reading, for a ruling).
//!
//! # Frames
//!
//! The generators give each moon's elements about its parent, referred to the parent's equator
//! (regular and giant-impact moons) or its orbital plane (captures). Until P14.T14 gives planets
//! their poles the equator is taken to be the orbital plane, so [`Satellite::orbit_at`] turns
//! both into the parent's body frame, a translation of the system frame with the galactic axes
//! (plan 01's `coords`), where a moon's position is its planet's plus its own offset.

use crate::Seed;
use crate::orbit::KeplerElements;
use crate::planetary::context::SystemContext;
use crate::planetary::index::{BodyIndex, BodySlot, BodySub, LAST_MOON_SUB};
use crate::planetary::moons::{
    CaptureKind, CapturedMoon, ImpactMoon, IrregularPopulation, MoonParent, NearestBelt,
    RegularMoon, captures, giant_impact_moon, regular_moons,
};
use crate::planetary::params::HILL_STABLE_GAP;
use crate::planetary::placement::classes::orbits::{SystemPlane, orientation_in_plane};
use crate::planetary::placement::mutual_hill_radius;
use crate::planetary::record::MoonOrigin;
use crate::planetary::rings::{Ring, RingMoon, RingParent, generate_rings};
use crate::planetary::system::{Body, satellite_parent};
use crate::stellar::draws::UnitUniform;
use crate::time::CLOCK_WINDOW_H;
use crate::units::{EarthMasses, Kilograms, SolarMasses, Years};

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

/// One moon of a planet with its index and its own radius rank (P14.T22.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Satellite {
    index: BodyIndex,
    moon: SatelliteMoon,
    radius_rank: UnitUniform,
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
    /// moon's derivation reads (P14.T17.b).
    #[must_use]
    pub const fn radius_rank(&self) -> UnitUniform {
        self.radius_rank
    }

    /// Its elements about its parent `parent` when the parent is `age` old, as generated:
    /// referred to the parent's equator or orbital plane. Only a giant-impact moon's change with
    /// age, as it recedes (P14.T18).
    #[must_use]
    pub fn local_orbit_at(&self, parent: &MoonParent, age: Years) -> KeplerElements {
        match &self.moon {
            SatelliteMoon::Regular(moon) => *moon.orbit(),
            SatelliteMoon::GiantImpact(moon) => moon.orbit_at(parent, age),
            SatelliteMoon::Captured(moon) => *moon.orbit(),
        }
    }

    /// Its elements about `parent` when the parent is `age` old, in the parent's body frame,
    /// with the galactic axes (see the [module](self) documentation).
    #[must_use]
    pub fn orbit_at(&self, parent: &MoonParent, age: Years) -> KeplerElements {
        in_body_frame(parent, &self.local_orbit_at(parent, age))
    }
}

/// `local`, an orbit about `parent` referred to its orbital plane, in the parent's body frame.
///
/// # Panics
///
/// Never: the orbit is rebuilt from a valid one's own values.
#[must_use]
pub(crate) fn in_body_frame(parent: &MoonParent, local: &KeplerElements) -> KeplerElements {
    let plane = SystemPlane::of_orbit(parent.orbit().orientation());
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
/// It reads nothing but the planet, its host (the context's stars and the zone's disc) and the
/// nearest belt's mass passed in, which
/// [`PlanetarySystem::nearest_belt`](crate::planetary::PlanetarySystem::nearest_belt) finds. A
/// planet with no moons and no rings has an empty set, and a body that is not a planet or a belt
/// member none.
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
        ),
        None => Satellites::none(planet.index()),
    }
}

/// The satellites of `parent`, the body `parent_index`, whose rings (for a giant) read
/// `ring_parent`, next to `belt`, in a system aged `age_at_epoch` at the epoch: the four steps of
/// the [module](self) documentation.
#[must_use]
pub(crate) fn satellites_of(
    seed: Seed,
    parent_index: BodyIndex,
    parent: &MoonParent,
    ring_parent: Option<&RingParent>,
    belt: Option<NearestBelt>,
    age_at_epoch: Years,
) -> Satellites {
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
    let mut moons = Vec::new();
    for (n, moon) in (1_u8..).zip(generated) {
        let Some(index) = moon_index(parent_index, n) else {
            break;
        };
        moons.push(Satellite {
            index,
            moon,
            radius_rank: crate::planetary::system::radius_rank(seed, index.body_id(system)),
        });
    }

    let ring_moons: Vec<RingMoon> = regular
        .moons()
        .iter()
        .map(|moon| RingMoon::new(moon.orbit().semi_major_axis(), Kilograms::from(moon.mass())))
        .collect();
    let rings = ring_parent.map_or_else(Vec::new, |ring_parent| {
        generate_rings(seed, system, ring_parent, &ring_moons)
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
        if outer.orbit().periapsis() - inner.orbit().apoapsis() < hill * HILL_STABLE_GAP {
            break;
        }
        last = outer.ordinal();
    }
    last
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
