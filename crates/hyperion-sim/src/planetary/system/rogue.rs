//! A rogue planet's system (plan 14, design note 13, P14.T27.b), from generator version 26
//! (`hosts::substellar`'s `ROGUE_PLANET_HOSTS_VERSION`): the object, body `0x0000`, derived with no
//! star, and its satellites, by P14.T17–T20 with design note 13's ejection cut.
//!
//! A rogue planet runs the satellite path only, as a planet without a star (design note 13). Its
//! context holds no star (`SystemContext`'s module documentation), so its system has no zone,
//! host, disc, planet, belt or halo, and every query of it takes this path.
//!
//! # The object
//!
//! [`RoguePlanet`]: plan 13's mass, the radius rank on its own `planet.radius` stream (body
//! `0x0000`), and its [`BirthOrbit`], drawn on its own `planet.origin`. Its record at a time is
//! P14.T16's derivation with no star (`derive_alone`): its radius and composition fixed at formation
//! on the birth orbit, its metals its cloud's own from one Jupiter mass (it formed alone), its
//! equilibrium temperature the cosmic background's, and its own heat, plan 13's giant cooling from
//! 0.3 Jupiter masses and P14.T24.b's radiogenic and residual heat below, the two blended between
//! 0.3 and 0.414. Its record holds:
//!
//! - its mass, its moons and its rings, and its position, the system's origin;
//! - its bulk, at the cosmic background's equilibrium temperature, 2.725 48 K, and the effective
//!   temperature of its own heat;
//! - its hooks' detail seed, which reads its body ID alone;
//! - its orbit `NotApplicable`: it is its system's root, which orbits nothing in it.
//!
//! The sections that need a star are `NotModelled`, each with its reason:
//!
//! - its surface (a body with a solid surface) or its envelope (one without): the climate and the
//!   air are solved under a host's light (P14.T13, T24, T48), and a star-free climate, Stevenson's
//!   (1999) warm floor under a hydrogen envelope among them, is not built;
//! - its photometry: P14.T47's is of a host's reflected light;
//! - its rotation and figure: P14.T14's law reads its orbit about a host and the host's tides,
//!   and the figure the law.
//!
//! # Its satellites
//!
//! The moon generators read the object as P14.T22.a reads a planet: derived at the generators'
//! time (the epoch, or the clock window's end for a system not born then), on its birth orbit
//! about [`BIRTH_HOST_MASS`](crate::planetary::hosts::substellar::BIRTH_HOST_MASS), with no belt
//! beside it, in the birth orbit's plane, since it holds no rotation law. An ejected planet, under
//! one Jupiter mass, keeps only the moons whose apocentre lies inside a tenth of its birth orbit's
//! Hill radius ([`BirthOrbit::moon_bound`]); a heavier one, formed alone, keeps them all. Its moons
//! are numbered from `0x0100` and its rings from `0x0080` (design note 3).
//!
//! A moon's record holds its mass, its orbit about the object and its position, and a ring's its
//! population and position, as about a planet. A moon's bulk, surface, photometry, rotation and
//! figure are `NotModelled`: its derivation (P14.T17.b) reads its star's light, and its ice
//! fraction its nursery's ice line in its star's disc.
//!
//! Nothing here reads a system's events: a rogue planet's bodies have none (P14.T31.c's impactors
//! are a host's belts', its storms a host's light).

use core::slice;

use super::{
    Body, Part, PlanetarySystem, RecordRules, moon_state, moon_trajectory, parent_time,
    satellite_bodies,
};
use crate::Seed;
use crate::coords::{SystemPosition, SystemVelocity};
use crate::planetary::context::SystemContext;
use crate::planetary::derive::{
    AloneMetals, DeriveBodyError, DerivedAlone, PlacedBody, derive_alone,
};
use crate::planetary::error::ResolveBodyError;
use crate::planetary::fate::BodyState;
use crate::planetary::hooks::{self, BodyHooks};
use crate::planetary::hosts::substellar::{BIRTH_HOST_MASS, BirthOrbit, birth_host, ejected};
use crate::planetary::index::BodyIndex;
use crate::planetary::label::{self, BodyLabel};
use crate::planetary::moons::{MoonParent, MoonParentParts, ParentKind};
use crate::planetary::record::{
    BodyIdentity, BodyKind, BodyOrbit, BodyRecord, BodyRecordBuilder, Hooks, Population, Section,
    SystemSnapshot,
};
use crate::planetary::rings::RingParent;
use crate::planetary::satellites::{SatelliteFrame, Satellites, satellites_inside};
use crate::stellar::draws::UnitUniform;
use crate::time::UniverseTime;
use crate::units::{EarthFluxes, EarthMasses, Kilograms, Metres};

/// A rogue planet, body `0x0000` of its system, as generated (P14.T27.b, design note 13): its
/// mass, its radius rank and its birth orbit.
///
/// # Examples
///
/// A rogue planet's system holds the object and its satellites, and no star:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::planetary::{BodyIndex, SystemContext, generate};
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::RoguePlanet, [0, 6_500, 0])?, &mut cell);
/// let record = cell.first().ok_or("layer G is not empty at the solar circle")?;
/// // `from_record` builds its context under every version's rules; `for_system` from 26.
/// let ctx = SystemContext::from_record(&galaxy, record);
/// let system = generate(galaxy.seed(), &ctx);
/// let object = system.rogue_planet().ok_or("a rogue planet's system holds its object")?;
/// assert!(system.zones().is_empty() && ctx.stars().is_empty());
/// assert_eq!(system.indices().first(), Some(&BodyIndex::PRIMARY));
/// assert!(object.birth_orbit().semi_major_axis().value() > 0.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoguePlanet {
    mass: EarthMasses,
    radius_rank: UnitUniform,
    birth: BirthOrbit,
}

impl RoguePlanet {
    /// The rogue planet of `ctx` in the universe of `seed`: its plan 13 mass, word 0 of its own
    /// `planet.radius` stream and its birth orbit ([`BirthOrbit::draw`]).
    ///
    /// # Panics
    ///
    /// For a context that is not a rogue planet's.
    #[must_use]
    fn of(seed: Seed, ctx: &SystemContext) -> Self {
        let mass = ctx
            .rogue_planet_mass()
            .expect("a rogue planet's context holds its mass");
        let object = BodyIndex::PRIMARY.body_id(ctx.id());
        Self {
            mass: EarthMasses::from(mass),
            radius_rank: super::radius_rank(seed, object),
            birth: BirthOrbit::draw(seed, object),
        }
    }

    /// Its mass, plan 13's.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// Its radius rank (design note 8), word 0 of its own `planet.radius` stream.
    #[must_use]
    pub const fn radius_rank(&self) -> UnitUniform {
        self.radius_rank
    }

    /// The orbit it formed on (design note 13).
    #[must_use]
    pub const fn birth_orbit(&self) -> &BirthOrbit {
        &self.birth
    }

    /// The apocentre inside which it keeps its moons: a tenth of its birth orbit's Hill radius
    /// for an ejected planet, none for one that formed alone ([`BirthOrbit::moon_bound`]).
    #[must_use]
    pub fn moon_bound(&self) -> Option<Metres> {
        self.birth.moon_bound(self.mass)
    }

    /// It as its derivation reads it: on its birth orbit, formed there, at its rank.
    #[must_use]
    fn placed(&self) -> PlacedBody {
        PlacedBody::new(
            self.mass,
            self.birth.elements(self.mass),
            self.birth.semi_major_axis(),
            self.radius_rank,
        )
        .expect("a rogue planet's mass and its birth orbit's axis are positive")
    }

    /// Its derivation with no star at `t` (`derive_alone`), in the composition of `ctx`.
    ///
    /// # Errors
    ///
    /// `derive_alone`'s: not yet formed for a system not born at `t`.
    fn derived_at(
        &self,
        ctx: &SystemContext,
        t: UniverseTime,
    ) -> Result<DerivedAlone, DeriveBodyError> {
        // Design note 13: under a Jupiter mass it was ejected from the disc it formed in; from it,
        // it formed alone, a fragment holding its cloud's metals.
        let metals = if ejected(self.mass) {
            AloneMetals::DiscFormed
        } else {
            AloneMetals::Cloud
        };
        derive_alone(
            &self.placed(),
            &birth_host(ctx.composition()),
            ctx.composition(),
            ctx.age_at(t),
            metals,
        )
    }

    /// It as its moon generators and its rings read it, derived at [`parent_time`] on its birth
    /// orbit; `None` where it cannot be derived then, as for a system not born even then, so that
    /// it has no satellites. Its rings read its effective temperature in no flux; one that
    /// radiates nothing has none.
    #[must_use]
    fn moon_parent(&self, ctx: &SystemContext) -> Option<(MoonParent, Option<RingParent>)> {
        let derived = self.derived_at(ctx, parent_time(ctx)).ok()?;
        let radius = Metres::from(derived.radius());
        let parent = MoonParent::new(MoonParentParts {
            id: BodyIndex::PRIMARY.body_id(ctx.id()),
            kind: ParentKind::Planet,
            mass: derived.mass(),
            radius,
            class: derived.class(),
            orbit: self.birth.elements(self.mass),
            host_mass: Kilograms::from(BIRTH_HOST_MASS),
            maximum_moon_mass: derived.maximum_moon_mass(),
        })
        .ok()?;
        let rings = derived.effective_temperature().and_then(|temperature| {
            RingParent::new(
                BodyIndex::PRIMARY,
                derived.class(),
                Kilograms::from(derived.mass()),
                radius,
                temperature,
                EarthFluxes::new(0.0),
            )
            .ok()
        });
        Some((parent, rings))
    }
}

/// [`generate_planets`](super::generate_planets) for a rogue planet's context: its object, and
/// nothing else, since it has no star to place planets about.
#[must_use]
pub(super) fn generate_planets(seed: Seed, ctx: &SystemContext) -> PlanetarySystem {
    PlanetarySystem {
        seed,
        system: ctx.id(),
        zones: Vec::new(),
        hosts: Vec::new(),
        bodies: Vec::new(),
        satellites: Vec::new(),
        belts: Vec::new(),
        halo: None,
        discs: super::SystemDiscs::NONE,
        record: RecordRules::in_force(),
        rogue: Some(RoguePlanet::of(seed, ctx)),
    }
}

/// [`generate`](super::generate) for a rogue planet's context: its object, then its satellites
/// with the ejection cut (see the [module](self) documentation).
#[must_use]
pub(super) fn generate(seed: Seed, ctx: &SystemContext) -> PlanetarySystem {
    let mut system = generate_planets(seed, ctx);
    let object = system
        .rogue
        .expect("a rogue planet's system holds its object");
    let satellites = object.moon_parent(ctx).map_or_else(
        || Satellites::none(BodyIndex::PRIMARY),
        |(parent, rings)| {
            satellites_inside(
                seed,
                BodyIndex::PRIMARY,
                &parent,
                rings.as_ref(),
                (None, ctx.age_at_epoch()),
                SatelliteFrame::OrbitalPlane,
                object.moon_bound(),
            )
        },
    );
    system.bodies = satellite_bodies(slice::from_ref(&satellites));
    system.bodies.sort_by_key(Body::index);
    system.satellites = vec![satellites];
    system
}

/// The object's state at `t`: present once its system is born.
#[must_use]
fn object_state(ctx: &SystemContext, t: UniverseTime) -> BodyState {
    if ctx.age_at(t).value() > 0.0 {
        BodyState::Present
    } else {
        BodyState::NotYetFormed
    }
}

impl PlanetarySystem {
    /// The rogue planet of a rogue planet's system (P14.T27.b), body `0x0000`, which is not among
    /// its [`bodies`](Self::bodies); `None` for every other system.
    #[must_use]
    pub const fn rogue_planet(&self) -> Option<&RoguePlanet> {
        self.rogue.as_ref()
    }

    /// [`body_at`](Self::body_at) in a rogue planet's system.
    pub(super) fn rogue_record(
        &self,
        ctx: &SystemContext,
        object: &RoguePlanet,
        index: BodyIndex,
        t: UniverseTime,
    ) -> Result<BodyRecord, ResolveBodyError> {
        let label = label::label(self, index).ok_or(ResolveBodyError::NoSuchBody)?;
        if index == BodyIndex::PRIMARY {
            return Ok(self.object_record(ctx, object, label, t));
        }
        let body = self.body(index).ok_or(ResolveBodyError::NoSuchBody)?;
        Ok(self.rogue_satellite_record(ctx, body, label, t))
    }

    /// [`snapshot_at`](Self::snapshot_at) in a rogue planet's system: the object's record and
    /// each satellite's, in index order, with its belts and halo `NotApplicable`, since a rogue
    /// planet has no disc to leave either.
    pub(super) fn rogue_snapshot(
        &self,
        ctx: &SystemContext,
        object: &RoguePlanet,
        t: UniverseTime,
    ) -> SystemSnapshot {
        let labels = label::labels(self);
        let label_of = |index: BodyIndex| {
            labels
                .binary_search_by_key(&index, |(at, _)| *at)
                .map(|at| labels[at].1.clone())
                .expect("every body of a system has a label")
        };
        let mut records = Vec::with_capacity(self.bodies.len() + 1);
        records.push(self.object_record(ctx, object, label_of(BodyIndex::PRIMARY), t));
        records.extend(
            self.bodies
                .iter()
                .map(|body| self.rogue_satellite_record(ctx, body, label_of(body.index), t)),
        );
        SystemSnapshot::new(self.system, t, records)
            .and_then(|snapshot| {
                snapshot.with_populations(Section::NotApplicable, Section::NotApplicable)
            })
            .expect("a system's records are its own, in index order, at full detail")
    }

    /// [`position_at`](Self::position_at) and [`state_at`](Self::state_at) in a rogue planet's
    /// system: the object at rest at the origin, a ring with it, and a moon on its trajectory about
    /// it; `None` for a body not present at `t`.
    pub(super) fn rogue_state(
        &self,
        ctx: &SystemContext,
        index: BodyIndex,
        t: UniverseTime,
    ) -> Result<Option<(SystemPosition, SystemVelocity)>, ResolveBodyError> {
        let at_rest = (SystemPosition::ORIGIN, SystemVelocity::ZERO);
        let state = object_state(ctx, t);
        if index == BodyIndex::PRIMARY {
            return Ok((state == BodyState::Present).then_some(at_rest));
        }
        let body = self.body(index).ok_or(ResolveBodyError::NoSuchBody)?;
        Ok(match &body.part {
            Part::Ring(_) => (state == BodyState::Present).then_some(at_rest),
            Part::Moon(moon) => match moon_state(state, &moon.satellite, ctx.age_at_epoch(), t) {
                BodyState::Present => {
                    let (trajectory, _) = moon_trajectory(ctx, moon, t, None);
                    let (position, velocity) = trajectory.relative_state_at(t);
                    Some((SystemPosition::ORIGIN.translated(position), velocity))
                }
                BodyState::NotYetFormed
                | BodyState::Destroyed { .. }
                | BodyState::Unbound { .. } => None,
            },
            Part::Planet(_) | Part::Member(_) => {
                unreachable!("a rogue planet's bodies are its moons and rings")
            }
        })
    }

    /// [`hooks_at`](Self::hooks_at) in a rogue planet's system: the body's surface seed, its bulk
    /// composition `NotModelled` (it reads a derivation's element inventory, which the derivation
    /// with no star does not take), and its record's surface's state; `None` for a ring.
    pub(super) fn rogue_hooks(
        &self,
        ctx: &SystemContext,
        object: &RoguePlanet,
        index: BodyIndex,
        t: UniverseTime,
    ) -> Result<Option<BodyHooks>, ResolveBodyError> {
        let record = self.rogue_record(ctx, object, index, t)?;
        if record.identity().kind() == BodyKind::Ring {
            return Ok(None);
        }
        let seed = hooks::surface_seed(self.seed, index.body_id(self.system));
        let bulk = match record.identity().state() {
            BodyState::Present => Section::NotModelled,
            BodyState::NotYetFormed | BodyState::Destroyed { .. } | BodyState::Unbound { .. } => {
                Section::NotApplicable
            }
        };
        let (surface, figures) = match record.surface() {
            Section::Ok(_) => unreachable!("a rogue planet's surfaces are not modelled"),
            Section::NotResolved => (Section::NotResolved, Section::NotResolved),
            Section::NotModelled => (Section::NotModelled, Section::NotModelled),
            Section::NotApplicable => (Section::NotApplicable, Section::NotApplicable),
        };
        Ok(Some(BodyHooks::new(seed, bulk, surface, figures)))
    }

    /// The record of the object at `t`, labelled `label` (see the [module](self) documentation).
    #[must_use]
    fn object_record(
        &self,
        ctx: &SystemContext,
        object: &RoguePlanet,
        label: BodyLabel,
        t: UniverseTime,
    ) -> BodyRecord {
        let state = object_state(ctx, t);
        let identity = BodyIdentity::new(
            self.system,
            BodyIndex::PRIMARY,
            BodyKind::Planet,
            None,
            state,
        )
        .with_label(label);
        let (moons, rings) = self.moons_and_rings(BodyIndex::PRIMARY);
        let builder = BodyRecord::builder(identity)
            .mass(Section::Ok(object.mass))
            .moons(Section::Ok(moons))
            .rings(Section::Ok(rings))
            .population(Section::NotApplicable);
        let builder = match state {
            BodyState::Present => {
                // A body with a solid surface has a surface and no envelope; one without, an
                // envelope (P14.T48.e's split, on its class, since it has no air solved).
                let (bulk, solid) = match object.derived_at(ctx, t) {
                    Ok(derived) => (
                        Section::Ok(derived.bulk()),
                        Some(derived.class().has_solid_surface()),
                    ),
                    Err(_) => (Section::NotModelled, None),
                };
                let (surface, envelope) = match solid {
                    Some(true) => (Section::NotModelled, Section::NotApplicable),
                    Some(false) => (Section::NotApplicable, Section::NotModelled),
                    None => (Section::NotModelled, Section::NotModelled),
                };
                builder
                    .position(SystemPosition::ORIGIN)
                    .orbit(Section::NotApplicable)
                    .bulk(bulk)
                    .surface(surface)
                    .envelope(envelope)
                    .photometry(Section::NotModelled)
                    .figure(Section::NotModelled)
                    .rotation(Section::NotModelled)
                    .hooks(self.rogue_hooks_section(BodyIndex::PRIMARY))
            }
            BodyState::NotYetFormed | BodyState::Destroyed { .. } | BodyState::Unbound { .. } => {
                not_present(builder)
            }
        };
        builder
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }

    /// The record of the moon or ring `body` of the object at `t`, labelled `label` (see the
    /// [module](self) documentation).
    #[must_use]
    fn rogue_satellite_record(
        &self,
        ctx: &SystemContext,
        body: &Body,
        label: BodyLabel,
        t: UniverseTime,
    ) -> BodyRecord {
        let parent = object_state(ctx, t);
        let state = match &body.part {
            Part::Moon(moon) => moon_state(parent, &moon.satellite, ctx.age_at_epoch(), t),
            Part::Ring(_) => parent,
            Part::Planet(_) | Part::Member(_) => {
                unreachable!("a rogue planet's bodies are its moons and rings")
            }
        };
        let identity =
            BodyIdentity::new(self.system, body.index, body.kind(), Some(body.host), state)
                .with_label(label);
        let builder = BodyRecord::builder(identity)
            .mass(Section::Ok(body.mass()))
            .moons(Section::NotApplicable)
            .rings(Section::NotApplicable)
            .envelope(Section::NotApplicable);
        let builder = match (&body.part, state) {
            (Part::Ring(ring), BodyState::Present) => builder
                .population(Section::Ok(Population::Ring(ring.clone())))
                .position(SystemPosition::ORIGIN)
                .orbit(Section::NotApplicable)
                .bulk(Section::NotApplicable)
                .surface(Section::NotApplicable)
                .hooks(Section::NotApplicable)
                .photometry(Section::NotApplicable)
                .figure(Section::NotApplicable)
                .rotation(Section::NotApplicable),
            (Part::Moon(moon), BodyState::Present) => {
                let (trajectory, valid_until) = moon_trajectory(ctx, moon, t, None);
                builder
                    .orbit(Section::Ok(
                        BodyOrbit::new(*trajectory.elements(), valid_until)
                            .with_drift(trajectory.drift().copied()),
                    ))
                    .position(SystemPosition::ORIGIN.translated(trajectory.relative_state_at(t).0))
                    .population(Section::NotApplicable)
                    .bulk(Section::NotModelled)
                    .surface(Section::NotModelled)
                    .photometry(Section::NotModelled)
                    .figure(Section::NotModelled)
                    .rotation(Section::NotModelled)
                    .hooks(self.rogue_hooks_section(body.index))
            }
            (Part::Ring(_) | Part::Moon(_), _) => {
                not_present(builder.population(Section::NotApplicable))
            }
            (Part::Planet(_) | Part::Member(_), _) => {
                unreachable!("a rogue planet's bodies are its moons and rings")
            }
        };
        builder
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }

    /// The hooks section of the present body `index`: its detail seed, which reads its ID alone.
    #[must_use]
    fn rogue_hooks_section(&self, index: BodyIndex) -> Section<Hooks> {
        Section::Ok(Hooks::new(hooks::detail_seed(
            self.seed,
            index.body_id(self.system),
        )))
    }
}

/// `builder` with the sections of a body not present at the time, which mean nothing then.
#[must_use]
fn not_present(builder: BodyRecordBuilder) -> BodyRecordBuilder {
    builder
        .orbit(Section::NotApplicable)
        .bulk(Section::NotApplicable)
        .surface(Section::NotApplicable)
        .envelope(Section::NotApplicable)
        .photometry(Section::NotApplicable)
        .figure(Section::NotApplicable)
        .rotation(Section::NotApplicable)
        .hooks(Section::NotApplicable)
}
